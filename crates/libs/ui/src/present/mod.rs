//! Presentation regions, declared as nodes.
//!
//! A region is the one per-frame path in the system: a rectangle whose pixels are drawn on
//! the present thread and handed to the compositor as one texture, rather than as a tree of
//! sprites it has to walk. Everything else is retained and costs nothing between edits.
//!
//! This module is the join between the three crates that own the halves. `windows-present`
//! owns the thread, the buffers and the [`Frame`] trait; `windows-scene` owns
//! `Paint::Presented` and the brush behind it; what is here is the declaration, the
//! lifecycle that follows the node, and the two crossings between the threads.
//!
//! # The extent is a solve output
//!
//! A region's buffers are allocated for its box, so they cannot be allocated until the box
//! exists. Mounting is therefore **deferred, not done against zero**: a region declared
//! inside a hidden subtree solves to no area and stays pending, and the flush that gives it
//! a box is the flush that mounts it. That is the same rule a scroll container's tracker
//! follows, for the same reason.
//!
//! # The two crossings
//!
//! The app thread declares and the present thread draws, and exactly two messages pass
//! between them.
//!
//! - **Out**, at every flush: mount, resize and unmount, through [`Presenter`]. Each is a
//!   channel send and none of them waits.
//! - **In**, once per region: the surface handle, as [`Bound`]. It arrives on the present
//!   thread, where no composition object may be touched, so the binder sends it here and
//!   asks for the frame that will apply it.
//!
//! Nothing crosses per published frame. A region that draws every refresh posts nothing to
//! this side at all.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use windows_core::Result;
use windows_present::{
    Bound, Epoch, Extent, Frame, Gpu, Presenter, Queue, RegionInput, RegionKey, RegionParts,
    RegionSpec,
};
use windows_scene::{ControlId, RegionId};
use windows_window::{Tick, Wake};

use crate::build::{El, Region};
use crate::layout::{Len, Preset};
use crate::role::Metric;

/// Builds the [`Frame`] a region draws with, on the present thread and with that thread's
/// `Gpu`.
///
/// `FnOnce` and `Send`: it is called once, on the other thread, and the renderer it returns
/// stays there. That is what lets a renderer hold device resources and anything else `!Send`
/// — none of it is ever constructed on this side.
pub type Build = Box<dyn FnOnce(&Gpu) -> Result<Box<dyn Frame>> + Send>;

/// The three handles a region shares with its producer, its renderer and its front half.
///
/// One type rather than three parameters, because they are minted together and each is
/// useless without the others: an epoch nothing reads paces no thread, an input nothing
/// writes leaves a gesture invisible to the renderer, and parts nothing publishes leave the
/// pointer picking nothing. The application holds it for as long as the surface exists and
/// hands the same value to whatever produces the data.
///
/// `Arc` throughout, because the three cross different seams: the epoch is bumped from
/// whichever thread the data arrives on, the input is written here and read on the present
/// thread, and the parts are published there and read here.
#[derive(Clone)]
pub struct Live {
    /// Bumped by whatever produces the region's data. The present thread parks on it, so a
    /// region whose data has stopped arriving costs nothing.
    pub epoch: Arc<Epoch>,
    /// What the pointer is doing over the region, written by [`pick`] and read inside
    /// [`Frame::should_draw`]. No input is routed to the present thread as a message.
    pub input: Arc<RegionInput>,
    /// What is pickable inside the region, in region-local DIPs, published by the renderer
    /// whenever its mapping moves.
    pub parts: Arc<RegionParts>,
}

impl Live {
    /// Mints the three handles.
    ///
    /// # Errors
    ///
    /// The epoch's wake event could not be created.
    pub fn new() -> Result<Self> {
        Ok(Self {
            epoch: Arc::new(Epoch::new()?),
            input: Arc::new(RegionInput::new()),
            parts: Arc::new(RegionParts::new()),
        })
    }
}

/// Declares a presentation region: one sprite painting a buffer the present thread draws.
///
/// The node is an ordinary element in every other respect — it takes a size from its
/// container, it can be placed in a grid, and chrome may sit **over** it. Nothing may sit
/// under it: a region that lets the ground show through its own box is composed every frame
/// instead of flipping, which is the whole of what it was mounted for.
///
/// `queue` is the presentation queue this region asks for.
/// [`Queue::Solo`](windows_present::Queue::Solo) is for the one surface whose plane the
/// layout protects; everything else shares one named queue, so a second per-frame surface
/// degrades its own company and never that one.
///
/// `live` is the triple the region shares with its producer and its renderer.
///
/// `build` runs on the present thread and returns the renderer.
///
/// The node carries a hit entry, so a contact resolves to the region through the one hit
/// array like any control and the recogniser pool, capture, cancel and inertia are
/// unchanged. What is new is only *which part* of it the contact landed on, which [`pick`]
/// resolves after the region has won.
///
/// ```no_run
/// # use windows_present::{Frame, Gpu, Queue};
/// # use windows_ui::present::Live;
/// # fn spectrum() -> Box<dyn Frame> { unimplemented!() }
/// # fn f(live: &Live) -> windows_ui::build::View {
/// windows_ui::present::region(Queue::Solo, live, |_gpu| Ok(spectrum()))
///     .name("Composite response")
///     .grow()
///     .erase()
/// # }
/// ```
#[must_use]
pub fn region(
    queue: Queue,
    live: &Live,
    build: impl FnOnce(&Gpu) -> Result<Box<dyn Frame>> + Send + 'static,
) -> El<Region> {
    // Minted here rather than at mount, because the sprite that paints it is seeded in this
    // same call and a sprite names its paint at mint. The key is the sink's own number, so
    // the registry, the Direct2D tag and the statistics record all read one identity.
    let sink = crate::build::region_sink();
    let seed = RegionSeed {
        sink,
        key: key_of(sink),
        queue,
        live: live.clone(),
        build: Some(Box::new(build)),
    };
    El::<Region>::region_seed(seed)
}

/// Returns the key that names the region painting `sink`.
///
/// Derived from the sink rather than counted separately, so the present thread's registry,
/// the Direct2D tag that names a failed draw and the statistics record all read one
/// identity. Both halves of the id are carried: a slot reused by a later region would
/// otherwise take the key of the one before it.
const fn key_of(sink: RegionId) -> RegionKey {
    RegionKey(((sink.index() as u64) << 32) | sink.generation() as u64)
}

/// One region under construction, held out of line because a `Slot` is `Copy` and a
/// renderer's builder is a boxed closure.
pub(crate) struct RegionSeed {
    pub sink: RegionId,
    pub key: RegionKey,
    pub queue: Queue,
    pub live: Live,
    /// Taken by the mount walk. A chain that is built and then discarded — a `switch` arm
    /// that lost — drops it when the arena is cleared, so nothing is left registered for a
    /// node that never existed.
    pub build: Option<Build>,
}

/// One mounted region, as the flush needs it.
pub(crate) struct RegionRow {
    pub node: windows_scene::NodeId,
    pub sink: RegionId,
    pub key: RegionKey,
    pub queue: Queue,
    pub live: Live,
    /// The control this region occupies in the hit array, filled by the mount walk that
    /// mints it. `None` for a region declared with no hit entry, which is nothing today: the
    /// seed always mints one, because a surface the pointer cannot reach cannot be picked
    /// inside either.
    pub control: Option<ControlId>,
    /// The part copy this side picks against, and the version it holds.
    pub picked: Picked,
    /// `Some` until the region is mounted on the present thread, which is the first flush
    /// that gives the node a box.
    pub build: Option<Build>,
    /// The extent the buffers were last allocated for, or `None` while the region is still
    /// pending. It is the only record of whether the box moved, so a solve that moved
    /// nothing sends nothing.
    pub extent: Option<Extent>,
}

/// What the present thread told this side about one region, and the frame it asked for.
///
/// The [`Tick`] rides with the message rather than being taken when it is read: the request
/// has to outlive the post, or the pacer's count falls to zero and the frame that would
/// apply the binding is never served.
struct Binding {
    key: RegionKey,
    bound: Bound,
    _tick: Option<Tick>,
}

/// The present thread and the inbox its binder writes to.
///
/// Installed by the driver once the window exists, because a [`Presenter`] needs the
/// window's visibility watch and the display's output transform, and neither exists earlier.
/// A thread-local, like the style and text tables: there is one present thread per UI
/// thread, and a second would be a second answer to which surfaces are flipping.
struct Registry {
    presenter: Presenter,
    inbox: Receiver<Binding>,
}

thread_local! {
    static REGISTRY: RefCell<Option<Registry>> = const { RefCell::new(None) };
}

/// Starts the present thread and installs it for this UI thread.
///
/// `wake` is the frame clock: a binding arriving from the present thread has to be applied
/// on this one, so it asks for the frame that will apply it.
///
/// Called once, by the driver. Installing a second replaces the first, which stops the
/// thread the first was running.
///
/// # Errors
///
/// The present thread or its wake event could not be created.
pub(crate) fn install(
    tuning: windows_present::Tuning,
    out: windows_color::OutputTransform,
    visibility: Option<windows_window::Watch>,
    wake: Wake,
) -> Result<()> {
    let (tx, inbox) = channel::<Binding>();
    let presenter = Presenter::spawn(
        tuning,
        out,
        visibility,
        Box::new(move |key, bound| post(&tx, &wake, key, bound)),
    )?;
    REGISTRY.with(|slot| *slot.borrow_mut() = Some(Registry { presenter, inbox }));
    Ok(())
}

/// Sends one binding and asks for the frame that will apply it.
///
/// Runs on the present thread. A closed inbox means the UI thread is gone, so the send is
/// dropped rather than reported: there is nothing left to report it to.
fn post(tx: &Sender<Binding>, wake: &Wake, key: RegionKey, bound: Bound) {
    let _ = tx.send(Binding {
        key,
        bound,
        _tick: Some(wake.tick()),
    });
}

/// Stops the present thread. Called by the driver as the window goes away, so the regions
/// are torn down before the scene that binds them.
pub(crate) fn uninstall() {
    REGISTRY.with(|slot| slot.borrow_mut().take());
}

/// Runs `f` against the installed registry, if there is one.
///
/// Fallibly on both counts: a region can be declared in a test with no present thread
/// installed, and this runs from the flush, which the registry's own drop can re-enter while
/// the thread's locals are being torn down.
fn with<R>(f: impl FnOnce(&Registry) -> R) -> Option<R> {
    REGISTRY
        .try_with(|slot| slot.try_borrow().ok().and_then(|slot| slot.as_ref().map(f)))
        .ok()
        .flatten()
}

/// Mounts `row` if it is pending and now has a box, or resizes it if its box moved.
///
/// Returns nothing and re-solves nothing: a region's extent is read from the solve and never
/// feeds back into it, so this cannot make the flush's sequence fail to terminate.
///
/// A box with no area is not ready. A region inside a subtree `when` or `hide_below` has
/// made `Display::None` is laid out at zero, and buffers allocated against that would be one
/// texel across for the life of the window — the extent gate is what defers it to the flush
/// that reveals the subtree, since revealing it is a style change.
pub(crate) fn publish(row: &mut RegionRow, size: windows_numerics::Vector2, dpi: f32) {
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let extent = Extent::new(size.x, size.y, dpi);
    if let Some(build) = row.build.take() {
        let spec = RegionSpec {
            key: row.key,
            queue: row.queue,
            extent,
        };
        let (epoch, input) = (Arc::clone(&row.live.epoch), Arc::clone(&row.live.input));
        // The build closure is consumed whether or not a present thread is installed, so a
        // region declared without one is inert rather than mounted on the next flush.
        with(|reg| reg.presenter.mount(spec, epoch, input, build));
        row.extent = Some(extent);
        return;
    }
    if row.extent == Some(extent) {
        return;
    }
    // In place: the surface handle survives a resize, so the binding this side already
    // applied is untouched and no frame is dropped. Destroy-and-create would reallocate the
    // buffers and re-issue a handle that is already bound.
    with(|reg| reg.presenter.resize(row.key, extent));
    row.extent = Some(extent);
}

/// Destroys the region `row` names, after the front thread has released its binding.
pub(crate) fn drop_region(row: &RegionRow) {
    with(|reg| reg.presenter.unmount(row.key));
}

/// Applies what the present thread reported since the last tick.
///
/// Called by the driver at the top of the tick. It lives here and not there because binding
/// a surface handle is the one `unsafe` call the region path makes, and the driver denies
/// unsafe outright.
///
/// A binding can arrive for a region this side has already dropped — the two threads tear a
/// region down in opposite orders — so an unresolvable key is skipped rather than asserted
/// on.
///
/// # Errors
///
/// The compositor refused the handle, or a sprite painting with the region could not be
/// rebound.
pub(crate) fn bind(
    scene: &mut windows_scene::Scene,
    back: &windows_scene::Backends,
    env: windows_scene::Env,
) -> Result<()> {
    thread_local! {
        /// One buffer per thread, reused. A binding arrives once per region and the buffer
        /// is empty in the steady state, so this allocates once and never again.
        static DRAINED: RefCell<Vec<(RegionKey, Bound)>> = const { RefCell::new(Vec::new()) };
    }
    // Drained into a buffer before any of it is applied: the inbox's borrow is the
    // registry's, and applying a binding reaches the host, which can unmount a region and
    // re-enter that borrow.
    let mut taken = DRAINED.take();
    taken.clear();
    with(|reg| {
        while let Ok(binding) = reg.inbox.try_recv() {
            taken.push((binding.key, binding.bound));
        }
    });
    let result = apply(&taken, scene, back, env);
    DRAINED.set(taken);
    result
}

fn apply(
    bindings: &[(RegionKey, Bound)],
    scene: &mut windows_scene::Scene,
    back: &windows_scene::Backends,
    env: windows_scene::Env,
) -> Result<()> {
    for &(key, bound) in bindings {
        let Some(sink) = crate::build::Host::with(|h| h.region_sink(key)) else {
            continue;
        };
        match bound {
            // SAFETY: the handle is a composition surface handle the region owns, and it
            // stays live until that region unmounts — which asks this side to release the
            // binding first, through `Released`.
            Bound::Surface { handle, .. } => unsafe {
                scene.set_region(sink, handle as *mut _, back, env)?;
            },
            // Both mean the handle is about to close, and both leave the region drawing
            // nothing rather than sampling a brush over a dead one. They are distinct to the
            // producer and identical here.
            Bound::Released | Bound::Failed => scene.clear_region(sink, back, env)?,
        }
    }
    Ok(())
}

impl El<Region> {
    /// Rounds the region's own corners.
    ///
    /// The mask is this side's, not the renderer's: the buffer is a rectangle and the
    /// compositor is what clips it, so a region on a card takes the card's radius without the
    /// renderer knowing what shape it is drawing into.
    #[must_use]
    pub fn radius(self, radius: Metric) -> Self {
        self.region_radius(Len::Metric(radius))
    }
}

pub use pick::{Picked, pick};

mod pick;

/// The preset a region's node starts from: a plain box that neither grows nor shrinks on its
/// own, so its extent is entirely its container's decision.
pub(crate) const PRESET: Preset = Preset::Bare;

impl RegionRow {
    /// Returns whether this region is still waiting for the box its buffers are taken from.
    #[cfg(test)]
    pub(crate) const fn is_pending(&self) -> bool {
        self.build.is_some()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::RegionRow;

    /// Returns how many regions are mounted, and how many of those are still waiting for a
    /// box.
    ///
    /// Both counts, because "every region is pending" is vacuously true of an empty table
    /// and would pass for a region that never mounted at all. Read off the rows rather than
    /// off the present thread, because a fixture installs none: what is under test is the
    /// gate this side applies.
    pub(crate) fn census(host: &crate::build::Host) -> (usize, usize) {
        host.regions_count(RegionRow::is_pending)
    }

    /// Returns the control the one mounted region occupies in the hit array.
    ///
    /// The id is minted by the mount walk and never handed to the application, so a test
    /// driving a report at a region has no other way to name its target.
    pub(crate) fn control(host: &crate::build::Host) -> Option<windows_scene::ControlId> {
        host.first_region_control()
    }
}

