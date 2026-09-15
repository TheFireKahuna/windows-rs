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

mod published;
pub use published::Published;
/// Versioned window theme. Read only after its version changes.
pub type Theme = Arc<Published<crate::role::Scope>>;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use windows_core::Result;
use windows_present::{
    Bound, Epoch, Extent, Frame, Gpu, Presenter, Queue, RegionInput, RegionKey, RegionParts,
    RegionSpec,
};
use windows_scene::{Backends, ControlId, Env, RegionId, Scene};
use windows_window::Tick;

use crate::build::{Element, Region, Ui};
use crate::layout::{Len, Preset};
use crate::role::Metric;
use crate::seam::RegionOp;
use crate::signal::PostWake;

/// Builds the [`Frame`] a region draws with, on the present thread and with that thread's
/// `Gpu`.
///
/// `FnOnce` and `Send`: it is called once, on the other thread, and the renderer it returns
/// stays there. That is what lets a renderer hold device resources and anything else `!Send`
/// — none of it is ever constructed on this side.
pub type Build = Box<dyn FnOnce(&Gpu, Theme) -> Result<Box<dyn Frame>> + Send>;

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
/// # fn f(ui: &mut windows_ui::build::Ui<'_>, live: &Live) {
/// windows_ui::present::region(ui, Queue::Solo, live, |_gpu, _theme| Ok(spectrum()))
///     .name("Composite response")
///     .grow();
/// # }
/// ```
pub fn region<'a>(
    ui: &'a mut Ui<'_>,
    queue: Queue,
    live: &Live,
    build: impl FnOnce(&Gpu, Theme) -> Result<Box<dyn Frame>> + Send + 'static,
) -> Element<'a, Region> {
    ui.region(queue, live, build)
}

/// Returns the key that names the region painting `sink`.
///
/// Derived from the sink rather than counted separately, so the present thread's registry,
/// the Direct2D tag that names a failed draw and the statistics record all read one
/// identity. Both halves of the id are carried: a slot reused by a later region would
/// otherwise take the key of the one before it.
pub(crate) const fn key_of(sink: RegionId) -> RegionKey {
    RegionKey(((sink.index() as u64) << 32) | sink.generation() as u64)
}

/// One mounted region, as the flush needs it.
pub(crate) struct RegionRow {
    pub theme: Theme,
    pub sink: RegionId,
    pub key: RegionKey,
    pub queue: Queue,
    pub live: Live,
    /// The control this region occupies in the hit array. Construction registers it
    /// directly so pointer and accessibility queries can reach the presented content.
    pub control: Option<ControlId>,
    /// `Some` until the region is mounted on the present thread, which is the first flush
    /// that gives the node a box.
    pub build: Option<Build>,
    /// The extent the buffers were last allocated for, or `None` while the region is still
    /// pending. It is the only record of whether the box moved, so a solve that moved
    /// nothing sends nothing.
    pub extent: Option<Extent>,
}

/// One mounted region, as the half that binds its surface needs it.
pub(crate) struct RegionFront {
    pub theme: Theme,
    pub epoch: Arc<Epoch>,
    pub key: RegionKey,
    pub sink: RegionId,
    /// Whether a surface handle is bound to [`sink`](Self::sink), so a drop clears only a
    /// binding that was made.
    pub bound: bool,
}

/// One binding waiting for the region it names.
///
/// A binding can arrive a batch before the mount that declares its row: the present thread
/// posts the handle as soon as the buffers exist, and the row reaches this side with the
/// batch the app half is still filling.
struct Pending {
    key: RegionKey,
    bound: Bound,
    /// Whether this binding has already waited one batch. A binding whose row is absent
    /// after the batch that follows names a region that never mounted, and is dropped.
    retried: bool,
}

/// Every mounted region, and the bindings not yet resolved against one.
///
/// Held by the tick rather than reached through the app half's tables, so binding a surface
/// costs a scan of a table a settled layout keeps under eight rows long and no hop.
#[derive(Default)]
pub(crate) struct Regions {
    rows: Vec<RegionFront>,
    pending: Vec<Pending>,
}

impl Regions {
    pub(crate) fn retheme(&mut self, root: crate::role::Scope) {
        for row in &self.rows {
            row.theme.set(row.theme.get().in_theme(root));
            row.epoch.bump();
        }
    }
    fn row(&mut self, key: RegionKey) -> Option<&mut RegionFront> {
        self.rows.iter_mut().find(|row| row.key == key)
    }

    /// Appends a drop for every region still mounted, for the thread shutting down to apply
    /// through [`apply`] so each is released in the order a mount's own drop would take.
    pub(crate) fn drops_into(&self, out: &mut Vec<RegionOp>) {
        out.extend(self.rows.iter().map(|row| RegionOp::Drop { key: row.key }));
    }
}

/// Applies one batch of region edits, in the order the app half emitted them.
///
/// A mount inserts the row **before** the present thread is asked to build the renderer, so
/// the binding that build produces finds a row to resolve against. A drop clears the sink
/// **before** the region is unmounted, because the region owns the surface handle behind the
/// brush this side paints with, and the handle must not close under a bound brush.
///
/// The first failure is returned and the rest of the batch still runs: an edit refused for
/// one region must not leave another's mount unsent.
///
/// # Errors
///
/// The compositor refused to release a surface, or a sprite painting with a dropped region
/// could not be rebound.
pub(crate) fn relight(output: windows_color::OutputTransform) {
    with(|reg| reg.presenter.set_output_transform(output));
}

pub(crate) fn apply(
    regions: &mut Regions,
    ops: &mut Vec<RegionOp>,
    scene: &mut Scene,
    back: &Backends,
    env: Env,
) -> Result<()> {
    let mut failed: Option<windows_core::Error> = None;
    for op in ops.drain(..) {
        match op {
            RegionOp::Mount {
                key,
                sink,
                live,
                extent,
                queue,
                build,
                theme,
                ..
            } => {
                regions.rows.push(RegionFront {
                    theme: theme.clone(),
                    epoch: live.epoch.clone(),
                    key,
                    sink,
                    bound: false,
                });
                let spec = RegionSpec { key, queue, extent };
                let (epoch, input) = (Arc::clone(&live.epoch), Arc::clone(&live.input));
                // The build closure is consumed whether or not a present thread is
                // installed, so a region declared without one is inert rather than mounted
                // by the next batch.
                with(|reg| {
                    reg.presenter
                        .mount(spec, epoch, input, move |gpu: &Gpu| build(gpu, theme))
                });
            }
            RegionOp::Resize { key, extent } => {
                // In place: the surface handle survives a resize, so the binding this side
                // already applied is untouched and no frame is dropped. Destroy-and-create
                // would reallocate the buffers and re-issue a handle that is already bound.
                with(|reg| reg.presenter.resize(key, extent));
            }
            RegionOp::Drop { key } => {
                if let Some(at) = regions.rows.iter().position(|row| row.key == key) {
                    let row = &regions.rows[at];
                    if row.bound
                        && let Err(error) = scene.clear_region(row.sink, back, env)
                    {
                        failed.get_or_insert(error);
                    }
                    regions.rows.remove(at);
                }
                with(|reg| reg.presenter.unmount(key));
            }
        }
    }
    failed.map_or(Ok(()), Err)
}

/// What the present thread told this side about one region, and the frame it asked for.
///
/// Where the binder wakes a pacer, its [`Tick`] rides with the message rather than being
/// taken when it is read: the request has to outlive the post, or the pacer's count falls to
/// zero and the frame that would apply the binding is never served. Where it rings a
/// doorbell, the ring itself is the whole request and nothing rides.
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
/// `wake` is how the thread that applies bindings is woken: a window thread's frame clock,
/// which the binding then holds a frame request on, or the doorbell of a thread parked on
/// its own mailboxes.
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
    wake: impl Into<PostWake>,
) -> Result<()> {
    let (tx, inbox) = channel::<Binding>();
    let wake = wake.into();
    let presenter = Presenter::spawn(
        tuning,
        out,
        visibility,
        Box::new(move |key, bound| post(&tx, &wake, key, bound)),
    )?;
    REGISTRY.with(|slot| *slot.borrow_mut() = Some(Registry { presenter, inbox }));
    Ok(())
}

/// Sends one binding and wakes the thread that applies it.
///
/// Runs on the present thread. A closed inbox means the applying thread is gone, so the send
/// is dropped rather than reported: there is nothing left to report it to. The ring follows
/// the send, so a sleeper that wakes finds the binding already in its inbox.
fn post(tx: &Sender<Binding>, wake: &PostWake, key: RegionKey, bound: Bound) {
    let tick = match wake {
        PostWake::Pacer(pacer) => Some(pacer.tick()),
        PostWake::Ring(_) => None,
    };
    let _ = tx.send(Binding {
        key,
        bound,
        _tick: tick,
    });
    if let PostWake::Ring(ring) = wake {
        ring.ring();
    }
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

/// Applies what the present thread reported since the last tick.
///
/// Called by the driver at the top of the tick, before [`apply`] takes that tick's batch. It
/// lives here and not there because binding a surface handle is the one `unsafe` call the
/// region path makes, and the driver denies unsafe outright.
///
/// A binding whose row is absent is kept and retried once: the present thread posts a handle
/// as soon as the buffers exist, which can be a batch before the mount that declares the row
/// reaches this side. A binding still unresolved after the batch that follows names a region
/// that never mounted, or one already dropped — the two threads tear a region down in
/// opposite orders — and is dropped rather than asserted on.
///
/// The first failure is returned and the rest of the batch still runs.
///
/// # Errors
///
/// The compositor refused the handle, or a sprite painting with the region could not be
/// rebound.
pub(crate) fn bind(
    regions: &mut Regions,
    scene: &mut Scene,
    back: &Backends,
    env: Env,
) -> Result<()> {
    with(|reg| {
        while let Ok(binding) = reg.inbox.try_recv() {
            regions.pending.push(Pending {
                key: binding.key,
                bound: binding.bound,
                retried: false,
            });
        }
    });
    if regions.pending.is_empty() {
        return Ok(());
    }
    let mut failed: Option<windows_core::Error> = None;
    // Taken out of the table, because resolving a binding writes the row it names. The
    // buffer is empty in the steady state, so the swap allocates nothing per tick.
    let mut pending = core::mem::take(&mut regions.pending);
    pending.retain_mut(|entry| {
        let Some(row) = regions.row(entry.key) else {
            // Kept for one batch, then dropped: the mount that would declare this row rides
            // the batch applied straight after this call.
            let keep = !entry.retried;
            entry.retried = true;
            return keep;
        };
        let result = match entry.bound {
            // SAFETY: the handle is a composition surface handle the region owns, and it
            // stays live until that region unmounts — which asks this side to release the
            // binding first, through `Released`.
            Bound::Surface { handle, .. } => unsafe {
                let result = scene.set_region(row.sink, handle as *mut _, back, env);
                row.bound = result.is_ok();
                result
            },
            // Both mean the handle is about to close, and both leave the region drawing
            // nothing rather than sampling a brush over a dead one. They are distinct to the
            // producer and identical here.
            Bound::Released | Bound::Failed => {
                row.bound = false;
                scene.clear_region(row.sink, back, env)
            }
        };
        if let Err(error) = result {
            failed.get_or_insert(error);
        }
        false
    });
    // The buffer goes back with the entries still waiting in it, keeping its allocation.
    regions.pending = pending;
    failed.map_or(Ok(()), Err)
}

impl Element<'_, Region> {
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

pub(crate) use pick::{Picks, pick};

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
    use super::{Picks, RegionRow};
    use crate::build::{Host, tests::fixture};
    use crate::seam::{Down, RegionOp};
    use windows_numerics::Vector2;
    use windows_scene::ControlId;

    /// Returns what the host has produced since the last call, as one batch.
    fn filled() -> Down {
        let mut down = Down::default();
        Host::with(|h| h.fill(&mut down));
        down
    }

    /// Names each edit, so a failure says which op the batch carried.
    fn kinds(ops: &[RegionOp]) -> Vec<&'static str> {
        ops.iter()
            .map(|op| match op {
                RegionOp::Mount { .. } => "mount",
                RegionOp::Resize { .. } => "resize",
                RegionOp::Drop { .. } => "drop",
            })
            .collect()
    }

    /// A region emits one mount when it first has a box, a resize only when that box moves,
    /// and a drop when it unmounts.
    ///
    /// The gate is the whole of what makes a region's buffers correct. A second mount would
    /// hand the present thread a builder it has already consumed, and a resize per flush
    /// would reallocate the buffers of every region on every frame.
    #[test]
    fn a_region_emits_one_mount_then_a_resize_only_when_its_box_moves() {
        let mut patch = fixture();
        let live = super::Live::new().expect("the epoch's wake event");
        // Collapsed before the mount, so the first solve is the one with no area to give.
        Host::with(|h| h.set_window(Vector2 { x: 0.0, y: 0.0 }));
        let held = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                super::region(ui, windows_present::Queue::Solo, &live, |_, _| {
                    unreachable!("no present thread is installed in a fixture")
                })
                .grow();
            },
        );

        Host::flush(&mut patch);
        assert!(
            filled().regions.is_empty(),
            "a region with no box asked for buffers"
        );

        Host::with(|h| h.set_window(Vector2 { x: 800.0, y: 600.0 }));
        Host::flush(&mut patch);
        let mounted = filled();
        assert_eq!(kinds(&mounted.regions), ["mount"]);

        Host::flush(&mut patch);
        assert!(
            filled().regions.is_empty(),
            "a solve that moved nothing re-sent an extent"
        );

        Host::with(|h| h.set_window(Vector2 { x: 400.0, y: 300.0 }));
        Host::flush(&mut patch);
        assert_eq!(kinds(&filled().regions), ["resize"]);

        drop(held);
        Host::flush(&mut patch);
        assert_eq!(kinds(&filled().regions), ["drop"]);
    }

    /// The pick table follows the same edits: a region the pointer can reach goes in when it
    /// mounts and out when it drops.
    #[test]
    fn the_pick_table_follows_the_region_edits() {
        let mut patch = fixture();
        let live = super::Live::new().expect("the epoch's wake event");
        let held = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                super::region(ui, windows_present::Queue::Solo, &live, |_, _| {
                    unreachable!("no present thread is installed in a fixture")
                })
                .grow();
            },
        );
        Host::flush(&mut patch);

        let mut picks = Picks::default();
        picks.apply(&filled().regions);
        assert_eq!(picks.len(), 1, "the mounted region cannot be picked inside");

        drop(held);
        Host::flush(&mut patch);
        picks.apply(&filled().regions);
        assert_eq!(picks.len(), 0, "the dropped region is still pickable");
    }

    /// Returns how many regions are mounted, and how many of those are still waiting for a
    /// box.
    ///
    /// Both counts, because "every region is pending" is vacuously true of an empty table
    /// and would pass for a region that never mounted at all. Read off the rows rather than
    /// off the present thread, because a fixture installs none: what is under test is the
    /// gate this side applies.
    pub(crate) fn census(host: &Host) -> (usize, usize) {
        host.regions_count(RegionRow::is_pending)
    }

    /// Returns the control the one mounted region occupies in the hit array.
    ///
    /// The id is minted by the mount walk and never handed to the application, so a test
    /// driving a report at a region has no other way to name its target.
    pub(crate) fn control(host: &Host) -> Option<ControlId> {
        host.first_region_control()
    }
}
