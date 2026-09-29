//! Presentation regions, declared as nodes.
//!
//! A region is the one per-frame path in the system: a rectangle whose pixels are drawn on the
//! present thread and handed to the compositor as one texture, rather than as a tree of sprites
//! it has to walk. Everything else is retained and costs nothing between edits.
//!
//! This module is the join between the three crates that own the halves. `windows-present` owns
//! the thread, the buffers and the [`Frame`] trait; `windows-scene` owns `Paint::Presented` and
//! the brush behind it; what is here is the declaration, the lifecycle that follows the node,
//! the two crossings between the threads, and the input thread's picking inside one.
//!
//! # The extent is a solve output
//!
//! A region's buffers are allocated for its box, so they cannot be allocated until the box
//! exists. Mounting is therefore **deferred, not done against zero**: a region declared inside
//! a hidden subtree solves to no area and stays pending, and the flush that gives it a box is
//! the flush that mounts it. That is the same rule a scroll container's tracker follows.
//!
//! # The two crossings
//!
//! - **Out**, at every flush: mount, resize and unmount, through [`Presenter`]. Each is a
//!   channel send and none of them waits.
//! - **In**, once per region: the surface handle, as [`Bound`]. It arrives on the present
//!   thread, where no composition object may be touched, so the binder sends it here and rings
//!   the thread that will apply it.
//!
//! Nothing crosses per published frame. A region that draws every refresh posts nothing here.
//!
//! # Picking inside one
//!
//! A region's contents are pixels, so nothing inside one is a visual or an entry in the hit
//! array. The region is **one hit entry**, like any control, so capture, cancel, the recogniser
//! pool and inertia are unchanged; what is added is only *which part* won, resolved after the
//! region's entry did, against the geometry the renderer published. The decision is written
//! straight into the region's [`RegionInput`] and its [`Epoch`] is bumped on the input thread.
//! The next present carries the new pixels — one display frame, the same latency the retained
//! path has — and a busy app thread cannot stall a gesture. The application is told afterwards,
//! through the ordinary intent queue.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering::*};
use std::sync::{Arc, Mutex};

use windows_color::OutputTransform;
use windows_core::Result;
use windows_numerics::Vector2;
use windows_present::{
    Bound, Epoch, Extent, Frame, Gpu, Part, Presenter, Queue, RegionInput, RegionKey, RegionParts,
    RegionSpec, SubId, Tuning,
};
use windows_scene::{ControlId, HitTable, NodeId, RegionId};
use windows_window::Watch;

use crate::build::{Element, Host, Region};
use crate::input::Report;
use crate::layout::{Len, Preset};
use crate::role::{Metric, Scope};
use crate::seam::{RegionOp, Ring};
use crate::widget::{Front, Intent, What};

// ── what a region publishes about itself ────────────────────────────────────────────

/// A value one thread publishes and a renderer reads by version.
///
/// The reader compares [`seq`](Self::seq) first and takes the lock only on the pass where it
/// moved, so a frame that draws the same value as the last one does one acquire load and
/// touches no lock.
pub struct Published<T> {
    seq: AtomicU64,
    value: Mutex<T>,
}

impl<T: Clone> Published<T> {
    pub fn new(value: T) -> Self {
        Self {
            seq: AtomicU64::new(0),
            value: Mutex::new(value),
        }
    }

    /// Publishes `value`. Callable from any thread.
    pub fn set(&self, value: T) {
        *self.value.lock().unwrap_or_else(|p| p.into_inner()) = value;
        // release: pairs with the acquire in `seq`, so the value is in place before the version
        // advertising it becomes visible.
        self.seq.fetch_add(1, Release);
    }

    /// Returns the published version. No kernel call and no lock.
    #[must_use]
    pub fn seq(&self) -> u64 {
        // acquire: pairs with the release in `set`.
        self.seq.load(Acquire)
    }

    /// Returns the published value. Called only on a pass where [`seq`](Self::seq) moved.
    #[must_use]
    pub fn get(&self) -> T {
        self.value
            .lock()
            .map(|held| held.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    /// Refreshes `out` from the published value, for the same pass [`get`](Self::get) is called
    /// on.
    ///
    /// `out` is the reader's own, so a `T` whose `clone_from` reuses what it already holds keeps
    /// those buffers rather than taking fresh ones.
    pub fn read_into(&self, out: &mut T) {
        match self.value.lock() {
            Ok(held) => out.clone_from(&held),
            Err(poisoned) => out.clone_from(&poisoned.into_inner()),
        }
    }
}

/// Versioned window theme. Read only after its version changes.
pub type Theme = Arc<Published<Scope>>;

/// Builds the [`Frame`] a region draws with, on the present thread and with that thread's `Gpu`.
///
/// `FnOnce` and `Send`: it is called once, on the other thread, and the renderer it returns
/// stays there. That is what lets a renderer hold device resources and anything else `!Send` —
/// none of it is ever constructed on this side.
pub type Build = Box<dyn FnOnce(&Gpu, Theme) -> Result<Box<dyn Frame>> + Send>;

/// The three handles a region shares with its producer, its renderer and its front half.
///
/// One type rather than three parameters, because they are minted together and each is useless
/// without the others: an epoch nothing reads paces no thread, an input nothing writes leaves a
/// gesture invisible to the renderer, and parts nothing publishes leave the pointer picking
/// nothing.
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

/// Returns the key that names the region painting `sink`.
///
/// Derived from the sink rather than counted separately, so the present thread's registry, the
/// Direct2D tag that names a failed draw and the statistics record all read one identity. Both
/// halves of the id are carried: a slot reused by a later region would otherwise take the key
/// of the one before it.
pub(crate) const fn key_of(sink: RegionId) -> RegionKey {
    RegionKey(((sink.index() as u64) << 32) | sink.generation() as u64)
}

/// The preset a region's node starts from: a plain box that neither grows nor shrinks on its
/// own, so its extent is entirely its container's decision.
pub(crate) const PRESET: Preset = Preset::Layer;

// ── the app half: declaration and the emit gate ─────────────────────────────────────

/// One mounted region, as the flush needs it. A row on the host, headed by its node.
pub(crate) struct RegionRow {
    /// The node whose solved box the buffers are allocated for.
    pub(crate) node: NodeId,
    pub(crate) sink: RegionId,
    /// The control this region occupies in the hit array, so pointer and accessibility queries
    /// reach the presented content.
    pub(crate) control: ControlId,
    pub(crate) queue: Queue,
    pub(crate) live: Live,
    /// The scope the renderer resolves its colours through, minted where the region is
    /// declared: the thread it draws on has none, and a theme change rebases this rather than
    /// re-declaring the region.
    pub(crate) theme: Theme,
    /// `Some` until the region is mounted on the present thread, which is the first flush that
    /// gives the node a box.
    pub(crate) build: Option<Build>,
    /// The extent the buffers were last allocated for, or `None` while the region is still
    /// pending. It is the only record of whether the box moved, so a solve that moved nothing
    /// sends nothing.
    pub(crate) extent: Option<Extent>,
    pub(crate) atlas: Option<Box<Atlas>>,
    pub(crate) active: bool,
    /// The set a region lays its parts out from, where it declared one, the mirror the
    /// present thread reads it through, and the revision of the set last mirrored.
    pub(crate) layout: Option<(crate::layout::Anchors, Arc<Published<crate::layout::Table>>, u64)>,
}

pub(crate) struct Atlas {
    pub(crate) size: Vector2,
    pub(crate) views: Vec<windows_scene::RegionView>,
}

impl Element<'_, Region> {
    /// Publishes settled child rectangles and scope in the region's coordinate space.
    pub fn layout_parts(self, anchors: crate::layout::Anchors, output: &Arc<Published<crate::layout::Table>>) -> Self {
        let node = self.node;
        let (_, row) = self.ui.host.regions.iter_mut().find(|(_, row)| row.node == node).expect("live region");
        row.layout = Some((anchors, Arc::clone(output), u64::MAX));
        self.anchors_origin(anchors)
    }

    /// Rounds the region's own corners.
    ///
    /// The mask is this side's, not the renderer's: the buffer is a rectangle and the compositor
    /// is what clips it, so a region on a card takes the card's radius without the renderer
    /// knowing what shape it is drawing into.
    #[must_use]
    pub fn radius(self, radius: Metric) -> Self {
        self.region_radius(Len::from(radius))
    }

    /// Declares what the parts the renderer publishes mean, so pixels inside the region become
    /// nameable elements.
    ///
    /// The renderer owns where a part is and restates it through `geometry` whenever its
    /// mapping moves; this side owns what it is, which does not move when a rectangle does.
    /// Declaration order is the order a client reads the parts in, and a part the renderer has
    /// not published yet is left out rather than reported at the origin.
    #[must_use]
    pub fn parts(mut self, geometry: &Arc<RegionParts>, parts: &[crate::uia::PartDecl]) -> Self {
        let id = self.control_id();
        let geometry = Arc::clone(geometry);
        self.host().region_peer(id, move |peer| {
            peer.geometry = geometry;
            peer.parts.clear();
            peer.parts.extend_from_slice(parts);
        });
        self
    }

    /// Binds the slots the region's parts report their numbers from, indexed by `SubId`.
    ///
    /// One allocation for the whole region, read at query time, so a band whose gain moves
    /// while its rectangle does not still reports the number it holds now.
    #[must_use]
    pub fn part_values(mut self, values: &Arc<[AtomicU64]>) -> Self {
        let id = self.control_id();
        let values = Arc::clone(values);
        self.host().region_peer(id, move |peer| {
            peer.values = Some(values);
        });
        self
    }

    /// Requests coalesced part-value notifications only while a client subscribes.
    #[must_use]
    pub fn part_updates(mut self, updates: &Arc<crate::uia::PartUpdates>) -> Self {
        let id = self.control_id();
        let updates = Arc::clone(updates);
        self.host().region_peer(id, move |peer| peer.updates = Some(updates));
        self
    }

    /// Binds the slot the region itself reports its number from, and the bounds it moves
    /// between.
    ///
    /// A presented read-out has no control row to hold a value, so its number lives beside the
    /// tree and is written by whichever thread owns it; the bounds travel with the tree, and
    /// are stated here because a range pattern that cannot answer `Minimum` is not one. Pair
    /// it with [`live_region`](Element::live_region) to have a change announced.
    #[must_use]
    pub fn reading(mut self, value: &Arc<AtomicU64>, range: crate::widget::Range) -> Self {
        let id = self.control_id();
        let value = Arc::clone(value);
        self.host().region_peer(id, move |peer| {
            peer.value = Some(value);
        });
        self.host().enrol_value(id);
        if let Some(row) = self.host().control_mut(id) {
            row.value = Some(crate::widget::ValueRow {
                min: range.min,
                span: range.max - range.min,
                step: range.step,
                ..crate::widget::ValueRow::default()
            });
        }
        self
    }

    /// Formats the region's producer-owned reading on demand for read-only value queries.
    ///
    /// The producer must encode absence as [`crate::uia::MISSING_READING`]. Other
    /// bit patterns, including measured nonfinite values, reach `format` unchanged.
    #[must_use]
    pub fn reading_format(mut self, format: fn(Option<f64>) -> String) -> Self {
        let id = self.control_id();
        self.host().region_peer(id, move |peer| peer.format = Some(format));
        self
    }
}

/// Emits this flush's region edits from the solve.
///
/// A region emits one mount when it first has a box and a resize only when that box moves; the
/// drop rides the node's own retirement. The gate is the whole of what makes a region's buffers
/// correct: a second mount would hand the present thread a builder it has already consumed, and
/// a resize per flush would reallocate the buffers of every region on every frame.
///
/// A box with no area is not ready — a region inside a hidden subtree is laid out at zero, and
/// buffers allocated against that would be one texel across for the life of the window — so the
/// mount defers to the flush that reveals it.
pub(crate) fn emit(host: &mut Host, out: &mut Vec<RegionOp>) {
    let Host {
        regions, tree, env, ..
    } = host;
    for (_, row) in regions.iter_mut() {
        if !tree.is_live(row.node) {
            continue;
        }
        // Compared by revision: the mirror sits behind a lock and reading it clones the whole
        // table, which a flush where the set did not move has no reason to pay.
        if let Some((anchors, output, seen)) = &mut row.layout {
            anchors.with(|table| {
                if table.revision() != *seen {
                    *seen = table.revision();
                    if output.get() != *table {
                        output.set(table.clone());
                        row.live.epoch.invalidate();
                    }
                }
            });
        }
        let size = tree.c.geom[row.node.index()].size;
        let active = size.x > 0.0 && size.y > 0.0;
        if row.extent.is_some() && row.active != active {
            out.push(RegionOp::Active { sink: row.sink, active });
        }
        row.active = active;
        if !active { continue; }
        let size = row.atlas.as_ref().map_or(size, |atlas| atlas.size);
        let extent = Extent::new(size.x, size.y, env.dpi());
        match (row.build.take(), row.extent) {
            (Some(build), _) => out.push(RegionOp::Mount {
                sink: row.sink,
                size_node: (row.atlas.is_none() && row.layout.is_none()).then_some(row.node),
                control: row.control,
                queue: row.queue,
                live: row.live.clone(),
                build,
                extent,
                theme: Arc::clone(&row.theme),
            }),
            (None, Some(was)) if was != extent => out.push(RegionOp::Resize {
                sink: row.sink,
                extent,
            }),
            (None, _) => continue,
        }
        row.extent = Some(extent);
    }
}

// ── the scene half: the presenter, the table and the bindings ───────────────────────

/// One binding waiting for the region it names.
///
/// A binding can arrive a batch before the mount that declares its row: the present thread posts
/// the handle as soon as the buffers exist, and the row reaches this side with the batch the app
/// half is still filling.
struct Waiting {
    key: RegionKey,
    bound: Bound,
    /// Whether this binding has already waited one batch. A binding whose row is absent after
    /// the batch that follows names a region that never mounted, and is dropped.
    waited: bool,
}

/// One mounted region, as the half that binds its surface holds it.
struct Mounted {
    retiring: Option<NodeId>,
    size_node: Option<NodeId>,
    geometry: Option<Arc<windows_present::RegionGeometry>>,
    dpi: std::rc::Rc<std::cell::Cell<f32>>,
    observed: bool,
    layout_active: bool,
    active: bool,
    sink: RegionId,
    /// What a hover over the region names, for the pick rows the input thread is handed.
    control: ControlId,
    live: Live,
    theme: Theme,
    /// Whether a surface handle is bound to [`sink`](Self::sink), so a drop clears only a
    /// binding that was made.
    bound: bool,
}

/// Every mounted region and the bindings not yet resolved against one, as the scene thread holds
/// them.
///
/// A settled layout keeps this under eight rows, so a scan is the lookup.
#[derive(Default)]
pub(crate) struct Regions {
    rows: Vec<Mounted>,
    waiting: Vec<Waiting>,
}

impl Regions {
    pub(crate) fn retire_exits(&mut self, front: &mut Front<'_>) -> Result<()> {
        let mut first = Ok(());
        let mut at = 0;
        while at < self.rows.len() {
            if !self.rows[at].retiring.is_some_and(|node| !front.scene.collapsing(node)) {
                at += 1;
                continue;
            }
            let row = self.rows.swap_remove(at);
            if row.bound {
                let step = front.scene.clear_region(row.sink, front.back, front.env);
                if first.is_ok() { first = step; }
            }
            with(|p, _| p.unmount(key_of(row.sink)));
        }
        first
    }

    /// Appends a drop for every region still mounted, for the thread shutting down to apply
    /// through [`apply`] so each is released in the order a mount's own drop would take.
    pub(crate) fn drops_into(&self, out: &mut Vec<RegionOp>) {
        out.extend(
            self.rows
                .iter()
                .map(|row| RegionOp::Drop { sink: row.sink }),
        );
    }

    /// Publishes activity transitions after layout or tracker events.
    pub(crate) fn visibility(&mut self, scene: &mut windows_scene::Scene, hidden: bool) -> Result<()> {
        for row in &mut self.rows {
            if row.retiring.is_some() { continue; }
            let active = !hidden && row.layout_active && scene.hits().visible(row.control);
            if active != row.active {
                row.active = active;
                if let Some(node) = row.size_node { scene.observe_size_active(node, active)?; }
                with(|p, _| p.set_active(key_of(row.sink), active));
            }
        }
        Ok(())
    }

    /// Rebases every mounted region's scope on a new theme and wakes its renderer, which
    /// invalidates whatever it cached against the old one.
    pub(crate) fn retheme(&mut self, root: Scope) {
        for row in &self.rows {
            row.theme.set(row.theme.get().in_theme(root));
            row.live.epoch.invalidate();
        }
    }

    /// Restates every region the pointer can be picked inside, for the input thread.
    pub(crate) fn picks_into(&self, out: &mut Vec<(ControlId, Live)>) {
        out.clear();
        out.extend(self.rows.iter().filter(|row| row.retiring.is_none())
            .map(|row| (row.control, row.live.clone())));
    }
}

// The present thread and the inbox its binder writes to.
//
// Installed by the scene thread, which is the thread that applies a binding: a `Presenter`
// needs the window's visibility watch and the display's output transform, and neither exists
// before the window does. A thread-local, because there is one present thread per scene thread
// and a second would be a second answer to which surfaces are flipping.
thread_local! {
    static PRESENT: RefCell<Option<(Presenter, Arc<Mutex<Vec<Waiting>>>)>> =
        const { RefCell::new(None) };
}

/// Starts the present thread and installs it for the calling thread.
///
/// Called once, by the scene thread. Installing a second replaces the first, which stops the
/// thread the first was running.
///
/// # Errors
///
/// The present thread or its wake event could not be created.
pub(crate) fn install(out: OutputTransform, visibility: Watch, bell: Arc<Ring>) -> Result<()> {
    let inbox: Arc<Mutex<Vec<Waiting>>> = Arc::default();
    let posted = Arc::clone(&inbox);
    // Runs on the present thread. A closed inbox means the applying thread is gone, so the send
    // is dropped rather than reported: there is nothing left to report it to. The ring follows
    // the send, so a sleeper that wakes finds the binding already in its inbox.
    let presenter = Presenter::spawn(
        Tuning::default(),
        out,
        Some(visibility),
        Box::new(move |key, bound| {
            if let Ok(mut held) = posted.lock() {
                held.push(Waiting {
                    key,
                    bound,
                    waited: false,
                });
            }
            bell.ring();
        }),
    )?;
    PRESENT.with(|p| *p.borrow_mut() = Some((presenter, inbox)));
    Ok(())
}

/// Stops the present thread. Called by the scene thread as the window goes away, so the regions
/// are torn down before the scene that binds them.
pub(crate) fn uninstall() {
    PRESENT.with(|p| *p.borrow_mut() = None);
}

/// Runs `f` against the installed presenter, if there is one.
///
/// Fallibly on both counts: a region can be declared in a test with no present thread installed,
/// and this runs from the apply, which the registry's own drop can re-enter while the thread's
/// locals are being torn down.
fn with<R>(f: impl FnOnce(&Presenter, &Mutex<Vec<Waiting>>) -> R) -> Option<R> {
    PRESENT
        .try_with(|p| {
            let held = p.try_borrow().ok()?;
            let (presenter, inbox) = held.as_ref()?;
            Some(f(presenter, inbox))
        })
        .ok()
        .flatten()
}

/// Replaces the transform every region draws through, after a display-capability change.
pub(crate) fn relight(out: OutputTransform) {
    with(|p, _| p.set_output_transform(out));
}

/// Applies one batch of region edits, in the order the app half emitted them.
///
/// A mount inserts the row **before** the present thread is asked to build the renderer, so the
/// binding that build produces finds a row to resolve against. A drop clears the sink **before**
/// the region is unmounted, because the region owns the surface handle behind the brush this
/// side paints with, and the handle must not close under a bound brush.
///
/// The first failure is returned and the rest of the batch still runs: an edit refused for one
/// region must not leave another's mount unsent.
///
/// # Errors
///
/// The compositor refused to release a surface, or a sprite painting with a dropped region could
/// not be rebound.
pub(crate) fn apply(
    regions: &mut Regions,
    ops: &mut Vec<RegionOp>,
    front: &mut Front<'_>,
    patch: Option<&windows_scene::SinkPatch>,
) -> Result<()> {
    let mut first = Ok(());
    // Drained rather than borrowed: a mount carries the builder that makes the renderer, and a
    // builder is called once, so it has to be moved out of the batch rather than read from it.
    for op in ops.drain(..) {
        let step = match op {
            RegionOp::Mount {
                sink,
                size_node,
                control,
                queue,
                live,
                build,
                extent,
                theme,
            } => {
                regions.rows.push(Mounted {
                    retiring: None,
                    size_node,
                    geometry: None,
                    dpi: std::rc::Rc::new(std::cell::Cell::new(extent.dpi)),
                    observed: false,
                    layout_active: true,
                    active: true,
                    sink,
                    control,
                    live: live.clone(),
                    theme: Arc::clone(&theme),
                    bound: false,
                });
                let spec = RegionSpec {
                    key: key_of(sink),
                    queue,
                    extent,
                };
                let (epoch, input) = (Arc::clone(&live.epoch), Arc::clone(&live.input));
                // The build closure is consumed whether or not a present thread is installed,
                // so a region declared without one is inert rather than mounted by a later
                // batch.
                with(|p, _| {
                    let geometry = p.mount(spec, epoch, input, move |gpu: &Gpu| build(gpu, theme));
                    regions.rows.last_mut().unwrap().geometry = Some(geometry);
                });
                Ok(())
            }
            // In place: the surface handle survives a resize, so the binding this side already
            // applied is untouched and no frame is dropped. Destroy-and-create would reallocate
            // the buffers and re-issue a handle that is already bound.
            RegionOp::Resize { sink, extent } => {
                let observed = regions.rows.iter_mut().find(|row| row.sink == sink).is_some_and(|row| {
                    row.dpi.set(extent.dpi);
                    if row.geometry.as_ref().is_some_and(|geometry| geometry.set_geometry_dpi(extent.dpi)) { row.live.epoch.invalidate(); }
                    row.observed
                });
                if !observed { with(|p, _| p.resize(key_of(sink), extent)); }
                Ok(())
            }
            RegionOp::Active { sink, active } => {
                if let Some(row) = regions.rows.iter_mut().find(|row| row.sink == sink) {
                    row.layout_active = active;
                }
                Ok(())
            }
            RegionOp::Drop { sink } => {
                if let Some(root) = patch.and_then(|patch| front.scene.collapsing_region(sink, patch))
                    && let Some(row) = regions.rows.iter_mut().find(|row| row.sink == sink)
                {
                    row.retiring = Some(root);
                    row.active = false;
                    if let Some(node) = row.size_node { front.scene.forget_size(node); }
                    with(|p, _| p.set_active(key_of(sink), false));
                    continue;
                }
                let bound = regions
                    .rows
                    .iter()
                    .position(|row| row.sink == sink)
                    .map(|at| {
                        let row = regions.rows.swap_remove(at);
                        if let Some(node) = row.size_node { front.scene.forget_size(node); }
                        row.bound
                    });
                let step = match bound {
                    Some(true) => front.scene.clear_region(sink, front.back, front.env),
                    _ => Ok(()),
                };
                with(|p, _| p.unmount(key_of(sink)));
                step
            }
        };
        if first.is_ok() {
            first = step;
        }
    }
    first
}

/// Applies what the present thread reported since the last pass.
///
/// Called at the top of the pass, before [`apply`] takes that pass's batch. It lives here and
/// not in the driver because binding a surface handle is the one `unsafe` call the region path
/// makes, and the driver denies unsafe outright.
///
/// A binding whose row is absent is kept and retried once; still unresolved after the batch that
/// follows it names a region that never mounted, or one already dropped — the two threads tear a
/// region down in opposite orders — and it is dropped rather than asserted on.
///
/// # Errors
///
/// The compositor refused the handle, or a sprite painting with the region could not be rebound.
pub(crate) fn bind(regions: &mut Regions, front: &mut Front<'_>) -> Result<()> {
    // Taken out of the table, because resolving a binding writes the row it names. The buffer is
    // empty in the steady state, so the swap allocates nothing per pass.
    let mut held = core::mem::take(&mut regions.waiting);
    with(|_, inbox| {
        if let Ok(mut posted) = inbox.lock() {
            held.append(&mut posted);
        }
    });
    let mut first = Ok(());
    for mut waiting in held {
        let Some(row) = regions
            .rows
            .iter_mut()
            .find(|r| key_of(r.sink) == waiting.key)
        else {
            // Kept for one batch, then dropped: the mount that would declare this row rides the
            // batch applied straight after this call.
            if !waiting.waited {
                waiting.waited = true;
                regions.waiting.push(waiting);
            }
            continue;
        };
        let step = match waiting.bound {
            // SAFETY: the handle is a composition surface handle the region owns, and it stays
            // live until that region unmounts — which asks this side to release the binding
            // first, through `Released`.
            Bound::Surface { handle, .. } => unsafe {
                let step =
                    front
                        .scene
                        .set_region(row.sink, handle as *mut _, front.back, front.env);
                row.bound = step.is_ok();
                if row.bound && !row.observed && row.retiring.is_none() {
                    if let Some(node) = row.size_node {
                        let input = row.geometry.as_ref().expect("mounted geometry").clone();
                        let epoch = row.live.epoch.clone();
                        let dpi = row.dpi.clone();
                        front.scene.observe_size(node, Some(row.sink), move |origin, size, target, moving| {
                            let scale = dpi.get() / 96.0;
                            let x = (origin.x * scale).round();
                            let y = (origin.y * scale).round();
                            let extent = Extent::new(((origin.x+size.x)*scale).round()/scale-x/scale, ((origin.y+size.y)*scale).round()/scale-y/scale, dpi.get());
                            let reserve = Extent::new(target.x, target.y, dpi.get());
                            if input.set_bounds(extent, reserve, moving, Some([x/scale,y/scale])) { epoch.invalidate(); }
                        }, front.back)?;
                        row.observed = true;
                        front.scene.observe_size_active(node, row.active)?;
                    }
                }
                step
            },
            // Both mean the handle is about to close, and both leave the region drawing nothing
            // rather than sampling a brush over a dead one. They are distinct to the producer
            // and identical here.
            Bound::Released | Bound::Failed => {
                if let Some(node) = row.size_node { front.scene.forget_size(node); }
                row.observed = false;
                row.bound = false;
                front.scene.clear_region(row.sink, front.back, front.env)
            }
        };
        if first.is_ok() {
            first = step;
        }
    }
    first
}

// ── the input half: picking a part ──────────────────────────────────────────────────

/// One region the pointer can be picked inside, with the part copy this side scans.
///
/// The copy is kept per region rather than rebuilt per pick: a drag over a band publishes a new
/// mapping every frame, and a copy that reached its high-water mark once allocates nothing after
/// that.
struct Pick {
    control: ControlId,
    live: Live,
    /// The version `parts` was copied at. [`u64::MAX`] until the first copy, which is a version
    /// a publish counter cannot reach, so a renderer that published before this side ever looked
    /// is copied rather than skipped.
    seen: u64,
    parts: Vec<Part>,
}

/// Every region the pointer can be picked inside, as the thread routing a contact holds them.
#[derive(Default)]
pub(crate) struct Picks(Vec<Pick>);

impl Picks {
    /// Makes the table hold exactly `rows`: regions that are gone are removed, new ones are
    /// added, and a region already held keeps the part copy it has scanned.
    pub(crate) fn sync(&mut self, rows: &[(ControlId, Live)]) {
        self.0.retain(|pick| {
            let retained = rows.iter().any(|row| row.0 == pick.control);
            if !retained && (pick.live.input.hover().is_some()
                || pick.live.input.active().is_some() || pick.live.input.cursor().is_some()) {
                pick.live.input.set_hover(None);
                pick.live.input.set_active(None);
                pick.live.input.set_cursor(None);
                pick.live.epoch.invalidate();
            }
            retained
        });
        for (control, live) in rows {
            if !self.0.iter().any(|pick| pick.control == *control) {
                self.0.push(Pick {
                    control: *control,
                    live: live.clone(),
                    seen: u64::MAX,
                    parts: Vec::new(),
                });
            }
        }
    }

    /// Returns the row the control `id` names, or `None` where that control is not a region.
    ///
    /// A miss is the common case — most reports name an ordinary control — so this is a scan of
    /// a table a settled layout keeps under eight rows long.
    fn row(&mut self, id: ControlId) -> Option<&mut Pick> {
        self.0.iter_mut().find(|pick| pick.control == id)
    }
}

impl Pick {
    /// Returns the part at `local`, refreshing the copy first if the renderer republished.
    ///
    /// `local` is in the region's own DIPs, which is the space the renderer publishes in.
    ///
    /// **Last match wins.** The renderer publishes in whatever order its mapping produced, and a
    /// later part is drawn over an earlier one, so the topmost is the last that contains the
    /// point — the same rule the hit array resolves overlap by.
    fn at(&mut self, local: Vector2) -> Option<SubId> {
        if self.live.parts.version() != self.seen {
            self.seen = self.live.parts.read_into(&mut self.parts);
        }
        self.parts
            .iter()
            .rev()
            .find(|part| {
                let r = part.rect;
                local.x >= r.left && local.x <= r.right && local.y >= r.top && local.y <= r.bottom
            })
            .map(|part| part.id)
    }
}

/// Converts a client-DIP point into the region's own space.
///
/// The origin is the region's hit entry, which is where layout put it in the same unscrolled,
/// pixel-snapped space the array is scanned in — so a region inside a scrolled container
/// resolves against the box the contact was already matched to, and nothing here re-derives a
/// scroll offset.
///
/// `None` where the entry has gone: a report can outlive one tick's array by a frame, and a
/// point resolved against a missing box would land at the window's origin rather than nowhere.
fn local(hits: &HitTable, id: ControlId, at: Vector2) -> Option<Vector2> {
    let entry = hits.entry(id)?;
    let scroll = if entry.flags.contains(windows_scene::HitFlags::UNSCROLLED) {
        Vector2::zero()
    } else { hits.offset(entry.scroll_src) };
    let by = hits.translation(id) - scroll;
    Some(Vector2 {
        x: at.x - entry.x0 - by.x,
        y: at.y - entry.y0 - by.y,
    })
}

/// Applies this tick's pointer reports to whichever regions they landed on.
///
/// Called from the tick after the front table has moved its pixels, so a region's own publish
/// sits beside every other control's rather than ahead of it.
///
/// `out` receives one intent per part a gesture finished on, which is how the application learns
/// what was edited. Nothing is queued for a hover: a hover changes pixels and no document, and
/// an intent per pointer sample would put the app thread on the frame clock.
pub(crate) fn pick(reports: &[Report], hits: &HitTable, picks: &mut Picks, out: &mut Vec<Intent>) {
    for report in reports {
        match *report {
            // Both edges, in one pass. Leaving the region clears its hover, and a fast flick
            // across two regions publishes the leave before the enter, because the reports
            // arrive in the order the pointer crossed them.
            Report::HoverChanged { from, to, at, .. } => {
                // The cursor goes with the hover: a read-out drawn at the last position the
                // pointer held while it is somewhere else entirely states a measurement that is
                // not being taken.
                if let Some(from) = from
                    && let Some(row) = picks.row(from)
                {
                    row.live.input.set_hover(None);
                    row.live.input.set_cursor(None);
                    row.live.epoch.invalidate();
                }
                if let Some(to) = to {
                    hover(picks, to, at, hits);
                }
            }
            Report::Moved {
                target, ref sample, ..
            } => hover(picks, target, sample.raw, hits),
            Report::Pressed {
                target, ref sample, ..
            } => {
                let Some(at) = local(hits, target, sample.raw) else {
                    continue;
                };
                let Some(row) = picks.row(target) else {
                    continue;
                };
                let part = row.at(at);
                row.live.input.set_active(part);
                row.live.input.set_cursor(Some((at.x, at.y)));
                row.live.epoch.invalidate();
            }
            // A release commits: the part under the contact stops being active, and the
            // application is told which part the gesture finished on.
            Report::Released { target, at, .. } => {
                let Some(at) = local(hits, target, at) else {
                    continue;
                };
                let Some(row) = picks.row(target) else {
                    continue;
                };
                let part = row.at(at);
                row.live.input.set_active(None);
                row.live.epoch.invalidate();
                if let Some(part) = part {
                    out.push(Intent {
                        target,
                        what: What::Part(part),
                    });
                }
            }
            // A cancel restores and commits nothing — the same contract a slider's canceled drag
            // has.
            Report::Canceled { target, .. } => {
                if let Some(row) = picks.row(target) {
                    row.live.input.set_active(None);
                    row.live.epoch.invalidate();
                }
            }
            _ => {}
        }
    }
}

/// Publishes the hovered part and the cursor for the region `id` names.
fn hover(picks: &mut Picks, id: ControlId, at: Vector2, hits: &HitTable) {
    let Some(at) = local(hits, id, at) else {
        return;
    };
    let Some(row) = picks.row(id) else {
        return;
    };
    let part = row.at(at);
    row.live.input.set_hover(part);
    row.live.input.set_cursor(Some((at.x, at.y)));
    row.live.epoch.invalidate();
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Published;

    #[test]
    fn retiring_a_pick_clears_input_before_the_live_handle_is_reused() {
        use super::{Live, Picks};
        use windows_present::SubId;
        use windows_scene::ControlId;

        let live = Live::new().unwrap();
        let rows = [(ControlId::FIRST, live.clone())];
        let mut picks = Picks::default();
        picks.sync(&rows);
        live.input.set_cursor(Some((42.0, 17.0)));
        live.input.set_hover(Some(SubId(3)));
        live.input.set_active(Some(SubId(3)));
        let seq = live.epoch.seq();
        picks.sync(&rows);
        assert_eq!(live.epoch.seq(), seq);
        assert_eq!(live.input.cursor(), Some((42.0, 17.0)));
        picks.sync(&[]);
        assert_eq!(live.input.cursor(), None);
        assert_eq!(live.input.hover(), None);
        assert_eq!(live.input.active(), None);
        assert_ne!(live.epoch.seq(), seq);
        let seq = live.epoch.seq();
        picks.sync(&[]);
        picks.sync(&rows);
        assert_eq!(live.epoch.seq(), seq);
        assert_eq!(live.input.cursor(), None);
        assert_eq!(picks.0.len(), 1);
        picks.sync(&[]);
        assert_eq!(live.epoch.seq(), seq, "an empty retired input needs no wake");
    }

    #[test]
    fn region_hover_routes_local_coordinates_without_application_intents() {
        use super::{Live, Picks, pick};
        use crate::input::Report;
        use windows_numerics::Vector2;
        use windows_present::{Part, SubId};
        use windows_scene::{ControlId, HitEntry, HitFlags, HitTable, NodeId, NO_ENTRY};

        let live = Live::new().unwrap();
        live.parts.publish(&[Part { id: SubId(7), rect: windows_d2d::Rect::new(0.0, 0.0, 100.0, 80.0) }]);
        let id = ControlId::FIRST;
        let mut picks = Picks::default();
        picks.sync(&[(id, live.clone())]);
        let entry = HitEntry {
            x0: 100.0, y0: 200.0, x1: 400.0, y1: 500.0,
            touch_inflate: 0.0, clip_parent: NO_ENTRY, parent: NO_ENTRY,
            flags: HitFlags::INTERACTIVE, scroll_src: NodeId::FIRST, id,
        };
        let mut hits = HitTable::default();
        hits.replace(&[entry], &[(id, 0)]);
        hits.set_scroll(NodeId::FIRST, Vector2::new(0.0, 30.0));
        let mut intents = Vec::new();
        let seq = live.epoch.seq();
        pick(&[Report::HoverChanged {
            from: None, to: Some(id), at: Vector2::new(140.0, 190.0), qpc: 0,
        }], &hits, &mut picks, &mut intents);
        assert_eq!(live.input.cursor(), Some((40.0, 20.0)));
        assert_eq!(live.input.hover(), Some(SubId(7)));
        assert_ne!(live.epoch.seq(), seq);
        assert!(intents.is_empty());
        live.parts.publish(&[]);
        pick(&[Report::HoverChanged {
            from: Some(id), to: Some(id), at: Vector2::new(150.0, 200.0), qpc: 0,
        }], &hits, &mut picks, &mut intents);
        assert_eq!(live.input.cursor(), Some((50.0, 30.0)));
        assert_eq!(live.input.hover(), None, "a passive region still publishes its cursor");
        assert!(intents.is_empty());
        let seq = live.epoch.seq();
        pick(&[Report::HoverChanged {
            from: Some(id), to: None, at: Vector2::new(450.0, 190.0), qpc: 0,
        }], &hits, &mut picks, &mut intents);
        assert_eq!(live.input.cursor(), None);
        assert_eq!(live.input.hover(), None);
        assert_ne!(live.epoch.seq(), seq);
        assert!(intents.is_empty());
    }

    /// A reader sees the version move with the value, and both reads answer the same one.
    ///
    /// The version is what every reader gates on, so a value published without it moving is a
    /// frame drawn against the value before it.
    #[test]
    fn a_value_published_from_another_thread_arrives_with_its_version() {
        use std::sync::Arc;
        let published = Arc::new(Published::new(Vec::<u32>::new()));
        let seq = published.seq();
        let producer = Arc::clone(&published);
        std::thread::spawn(move || producer.set(vec![4, 5, 6]))
            .join()
            .expect("the producer finished");
        assert_ne!(published.seq(), seq, "the version stood still");
        assert_eq!(published.get(), [4, 5, 6]);

        let mut out = Vec::with_capacity(8);
        let held = out.as_ptr();
        published.read_into(&mut out);
        assert_eq!(out, [4, 5, 6]);
        assert_eq!(out.as_ptr(), held, "the reader's buffer was replaced");
    }
}
