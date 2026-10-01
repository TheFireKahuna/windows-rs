//! The thread seams: what crosses between the input, scene and app threads, and how.
//!
//! Every crossing is a single-slot mailbox over a buffer allocated once, and every wait is on
//! a doorbell that rings only on the parked→running edge. No mutex sits on a seam, no thread
//! waits on another, and nothing here allocates once the buffers exist.
//!
//! This module declares the rows and the two mechanisms that carry them. It reads none of
//! them: every column below names the reader that owns it.

use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering::*};
use std::sync::Arc;

use windows_numerics::Vector2;
use windows_present::{Extent, Queue};
use windows_scene::{
    Census, ControlId, Env, HitTable, NodeId, RegionId, SceneEvent, SinkPatch, SpriteId, TrackerId,
};
use windows_window::{CaptionState, Event};

use crate::gesture::GestureDecl;
use crate::input::{Report, ScopeId};
use crate::layout::ThumbGeom;
use crate::present::{Build, Live, Theme};
use crate::role::Scope;
use crate::text_input;
use crate::widget::{ChromeRow, Intent, ValueRow};

// ── the doorbell ────────────────────────────────────────────────────────────────────

/// A doorbell one thread parks on and any thread rings.
///
/// `arm` is called by the sleeper before it re-checks its inboxes and waits; `ring` by a
/// producer after it has published. The kernel event is signalled only when the sleeper had
/// armed it, so a producer ringing a running consumer costs one atomic and no system call —
/// and a ring that lands between the sleeper's arm and its wait is not lost, because the event
/// is auto-reset and stays signalled until the wait consumes it.
///
/// One doorbell serves one sleeper. An auto-reset event is unicast, so two threads parked on
/// the same doorbell would leave one of them asleep through the change that woke the other.
pub(crate) struct Ring {
    armed: AtomicBool,
    /// Set by every ring and taken by the sleeper after it arms, so a ring that landed while
    /// the sleeper was running is seen without inspecting any inbox.
    pending: AtomicBool,
    event: Event,
}

impl Ring {
    /// Creates a doorbell.
    ///
    /// # Errors
    ///
    /// The kernel event could not be created.
    pub(crate) fn new() -> windows_core::Result<Self> {
        Ok(Self {
            armed: AtomicBool::new(false),
            pending: AtomicBool::new(false),
            event: Event::auto_reset()?,
        })
    }

    /// Arms the doorbell. The sleeper calls this, then re-checks every inbox, then
    /// [`wait`](Self::wait): a ring between the re-check and the wait still signals the event.
    pub(crate) fn arm(&self) {
        // release: the sleeper's inbox reads that follow happen after the arm is visible, so a
        // producer that publishes and then sees `armed` clear knows the sleeper is past its
        // re-check and about to wait — and rings.
        self.armed.store(true, Release);
    }

    /// Rings, waking the sleeper if it is parked or about to park. One system call per
    /// parked→running edge, none while the consumer is running.
    pub(crate) fn ring(&self) {
        // release: the publish this ring announces is ordered before the flag the sleeper
        // takes with acquire.
        self.pending.store(true, Release);
        // acq_rel: pairs with `arm`; the swap orders this producer's publish before the
        // sleeper's next inbox read.
        if self.armed.swap(false, AcqRel) {
            self.event.signal();
        }
    }

    /// Returns whether a ring has landed since the last take, clearing it. `true` means skip
    /// the wait and drain.
    pub(crate) fn take_pending(&self) -> bool {
        // acq_rel: pairs with the release in `ring`, so the publish behind the flag is visible
        // to the drain that follows.
        self.pending.swap(false, AcqRel)
    }

    /// Parks until rung. The sleeper disarms on the way out, so a ring that arrives while it
    /// runs signals nothing and the next `arm` starts clean.
    pub(crate) fn wait(&self) {
        self.event.wait(windows_window::clock::INFINITE);
        self.disarm();
    }

    /// The event, for a sleeper that waits on it alongside other handles rather than through
    /// [`wait`](Self::wait). Such a sleeper disarms itself after the wait returns.
    pub(crate) fn event(&self) -> &Event {
        &self.event
    }

    /// Disarms without waiting: the sleeper found work on its re-check and is not parking.
    pub(crate) fn disarm(&self) {
        self.armed.store(false, Release);
    }
}

const _: () = {
    const fn crosses<T: Send + Sync>() {}
    crosses::<Ring>();
};

// ── the mailbox ─────────────────────────────────────────────────────────────────────

/// A one-slot mailbox carrying a `Box<T>` from one producer to one consumer.
///
/// Wait-free on both sides: a producer that finds the slot full keeps its buffer and a
/// consumer that finds it empty gets `None`. Neither ever blocks, so a seam can never make one
/// thread wait on another.
struct Slot<T>(AtomicPtr<T>);

// SAFETY: the box behind the pointer is owned by exactly one side at a time. A producer owns
// it until `put` publishes the pointer, and after that only `take` can reach it, which removes
// the pointer from the slot before returning the box. No two threads can hold the same `T`, so
// crossing the seam needs only `T: Send`.
unsafe impl<T: Send> Send for Slot<T> {}
// SAFETY: as above; the shared state is one pointer and every access to it is atomic.
unsafe impl<T: Send> Sync for Slot<T> {}

impl<T> Slot<T> {
    const fn empty() -> Self {
        Self(AtomicPtr::new(null_mut()))
    }

    fn put(&self, buf: Box<T>) -> Result<(), Box<T>> {
        let raw = Box::into_raw(buf);
        // acq_rel on success: the release half publishes every write the producer made into
        // the box before the pointer becomes visible, and the acquire half pairs with the
        // consumer's `take`, so the slot being refilled is known to have been emptied.
        // acquire on failure: the load that read the occupant orders against the `take` that
        // will clear it, so the next attempt sees a slot no staler than this one did.
        match self.0.compare_exchange(null_mut(), raw, AcqRel, Acquire) {
            Ok(_) => Ok(()),
            // SAFETY: the exchange failed, so the pointer was never published and this thread
            // still owns the allocation `Box::into_raw` handed out above.
            Err(_) => Err(unsafe { Box::from_raw(raw) }),
        }
    }

    fn take(&self) -> Option<Box<T>> {
        // acq_rel: the acquire half pairs with the producer's `put`, so every write into the
        // box happens before the consumer reads it; the release half publishes the emptied
        // slot, so the producer's next `put` cannot observe the slot as full after this swap.
        let raw = self.0.swap(null_mut(), AcqRel);
        // SAFETY: a non-null pointer in the slot was put there by `Box::into_raw` in `put`, and
        // the swap removed it, so this thread is now its only owner.
        (!raw.is_null()).then(|| unsafe { Box::from_raw(raw) })
    }
}

impl<T> Drop for Slot<T> {
    fn drop(&mut self) {
        // The mailbox is being destroyed, so both sides are gone and a plain read suffices.
        let raw = *self.0.get_mut();
        if !raw.is_null() {
            // SAFETY: as in `take` — the pointer came from `Box::into_raw` in `put`, and
            // nothing else can reach it now.
            drop(unsafe { Box::from_raw(raw) });
        }
    }
}

// ── the seam ────────────────────────────────────────────────────────────────────────

/// What every seam row answers: whether it carries work, and how to empty itself.
pub(crate) trait Row: Default + Send {
    fn is_empty(&self) -> bool;
    fn clear(&mut self);
}

/// What a row's column is emptied by, so the macro's `clear` is one call whatever the column
/// holds. A column keeps its allocation; only its contents go.
trait Column {
    fn empty(&mut self);
}

impl<T> Column for Vec<T> {
    fn empty(&mut self) {
        self.clear();
    }
}

impl Column for SinkPatch {
    fn empty(&mut self) {
        self.clear();
    }
}

impl Column for HitTable {
    fn empty(&mut self) {
        self.replace(&[], &[]);
    }
}

impl Column for Declared {
    fn empty(&mut self) {
        self.clear();
    }
}

impl Column for Fields {
    fn empty(&mut self) {
        self.clear();
    }
}

/// One seam between two threads: the buffer going across, the spare coming back, and the flag
/// that says the producer is waiting for one.
///
/// Two buffers serve a pair of threads for the life of the window: the producer fills the one
/// it holds, puts it, and takes the spare back; the consumer drains, clears and returns it.
/// Steady-state allocation is zero.
pub(crate) struct Link<T> {
    across: Slot<T>,
    back: Slot<T>,
    /// Set by a producer that had a batch to send and no spare to send it in; the consumer
    /// rings only while this is set, so a returned spare nobody was waiting for wakes nobody.
    wanted: AtomicBool,
}

impl<T: Row> Link<T> {
    /// Creates the seam with the spare allocated. The producer's own first buffer is made
    /// where the producer starts.
    pub(crate) fn new() -> Self {
        let link = Self {
            across: Slot::empty(),
            back: Slot::empty(),
            wanted: AtomicBool::new(false),
        };
        // A put into an empty mailbox cannot fail.
        _ = link.back.put(Box::default());
        link
    }

    /// Takes the spare, recording that one is owed where there is none.
    pub(crate) fn spare(&self) -> Option<Box<T>> {
        self.back.take().or_else(|| {
            // release: the consumer reads this after its own `back.put`, so the flag must not
            // become visible before the take above failed.
            self.wanted.store(true, Release);
            // Armed, then re-checked: the consumer may have returned the spare between the
            // first take and the store, and either this take gets it or that return rings.
            self.back.take()
        })
    }

    /// Publishes a filled buffer, handing it back when the consumer is behind.
    ///
    /// # Errors
    ///
    /// The consumer has not taken the previous batch. The caller keeps the buffer and retries.
    pub(crate) fn put(&self, buf: Box<T>) -> Result<(), Box<T>> {
        self.across.put(buf)
    }

    /// Takes the published batch. `None` when nothing was published since the last take.
    pub(crate) fn take(&self) -> Option<Box<T>> {
        self.across.take()
    }

    /// Hands `held` across when it carries work and a spare is back, and reports whether the
    /// consumer must be rung.
    ///
    /// **An empty batch never crosses, and a returned buffer is not work.** Where the spare is
    /// not back or the consumer is behind, the buffer stays here and grows; order is kept
    /// either way, because the batch in the mailbox precedes the one held.
    pub(crate) fn send(&self, held: &mut Box<T>) -> bool {
        if held.is_empty() {
            return false;
        }
        let Some(spare) = self.spare() else {
            return false;
        };
        let filled = core::mem::replace(held, spare);
        match self.across.put(filled) {
            Ok(()) => true,
            // Occupied: the consumer has not taken the previous batch. Keep the filled one and
            // return the spare, so nothing is lost and nothing is reordered.
            Err(filled) => {
                let spare = core::mem::replace(held, filled);
                _ = self.back.put(spare);
                self.wanted.store(true, Release);
                false
            }
        }
    }

    /// Returns a drained buffer as the spare, and reports whether the producer must be rung.
    ///
    /// Rings only where the producer set `wanted`, so a returned spare nobody was waiting for
    /// wakes nobody.
    pub(crate) fn give(&self, mut buf: Box<T>) -> bool {
        buf.clear();
        // acq_rel: the swap must not be reordered before the spare is back in the slot, or the
        // producer this rings would wake and find nothing.
        self.back.put(buf).is_ok() && self.wanted.swap(false, AcqRel)
    }
}

/// Declares one seam row: its columns, its emptiness test, and the assertion that every field
/// on it is `Send`.
///
/// `columns` are emptied keeping their allocation; `scalars` are reset. The emptiness test is
/// written per row because what counts as work differs per consumer.
macro_rules! row {
    (
        $(#[$m:meta])* $name:ident,
        empty: |$b:ident| $empty:expr,
        columns { $($(#[$cm:meta])* $c:ident: $ct:ty,)* }
        scalars { $($(#[$sm:meta])* $s:ident: $st:ty,)* }
    ) => {
        $(#[$m])*
        #[derive(Default)]
        pub(crate) struct $name {
            $($(#[$cm])* pub(crate) $c: $ct,)*
            $($(#[$sm])* pub(crate) $s: $st,)*
        }

        impl Row for $name {
            fn is_empty(&self) -> bool {
                let $b = self;
                $empty
            }

            fn clear(&mut self) {
                $(Column::empty(&mut self.$c);)*
                $(self.$s = Default::default();)*
            }
        }

        // Fails to compile if a row ever gains a field that is not `Send`.
        const _: () = {
            const fn crosses<T: Send>() {}
            crosses::<$name>();
        };
    };
}

// ── the shapes two seams share ──────────────────────────────────────────────────────

/// What the app thread declares and the input thread consumes, carried through the scene
/// thread untouched.
///
/// One shape rather than the same six fields restated on both seams: the scene thread reads
/// `released` on its way past and relays the whole of it with [`absorb`](Self::absorb).
#[derive(Default)]
pub(crate) struct Declared {
    /// Read by `Router::declare`.
    pub(crate) gestures: Vec<(ControlId, GestureDecl)>,
    /// Read by `Controls::adopt` on the way past, and by `Router::forget` at the end.
    pub(crate) released: Vec<ControlId>,
    /// Read by `FocusRing::apply`, after the router's own tick so a keyboard move lands on the
    /// reports the front table is about to read.
    pub(crate) focus: Vec<FocusOp>,
    /// Read by `Uia::publish`. Filled only on a pass a listening provider asked for.
    pub(crate) uia: crate::uia::Snapshot,
    /// Read by `Uia::observe`: what a control did between publishes, so a moved number and a
    /// completed action reach a client without republishing the tree. Filled only while a
    /// provider is listening, which is what keeps a drag free of it.
    pub(crate) intents: Vec<Intent>,
    /// Read by `Uia::watch_region`: what a presentation region's pixels mean. Declared where
    /// the region is mounted, and joined with the renderer's own geometry on the tick.
    pub(crate) peers: Vec<crate::uia::RegionPeer>,
    /// The window commands, in the order minimise, maximise, close. Read by `caption::hit`.
    pub(crate) caption: [Option<ControlId>; 3],
    /// Read by the observer on the input thread.
    pub(crate) census: AppCensus,
}

/// The table that declares no window command, which is what a buffer holds when the flush it
/// carried had no caption change to report.
const NO_CAPTION: [Option<ControlId>; 3] = [None; 3];

impl Declared {
    /// Takes everything `from` carries into this one, appending rather than replacing: the
    /// batch held for the input thread accumulates across the patches that arrive between its
    /// ticks.
    pub(crate) fn absorb(&mut self, from: &mut Self) {
        self.gestures.append(&mut from.gestures);
        self.released.append(&mut from.released);
        self.focus.append(&mut from.focus);
        self.intents.append(&mut from.intents);
        self.peers.append(&mut from.peers);
        // The snapshot travels with the publish rather than with the buffer: it is swapped out
        // of the drained batch before that buffer is recycled.
        if !from.uia.entries.is_empty() {
            core::mem::swap(&mut self.uia, &mut from.uia);
        }
        // A table of three absences carries no declaration, and a fill writes the table only
        // where it changed, so copying one would clear a registry the last batch established.
        if from.caption != NO_CAPTION {
            self.caption = from.caption;
        }
        self.census = from.census;
    }

    fn is_empty(&self) -> bool {
        self.gestures.is_empty()
            && self.released.is_empty()
            && self.focus.is_empty()
            && self.uia.entries.is_empty()
            && self.intents.is_empty()
            && self.peers.is_empty()
            && self.caption == NO_CAPTION
    }

    fn clear(&mut self) {
        self.gestures.clear();
        self.released.clear();
        self.focus.clear();
        self.uia.clear();
        self.intents.clear();
        self.peers.clear();
        self.caption = NO_CAPTION;
        self.census = AppCensus::default();
    }
}

/// The text-field rows crossing a seam: what a field is, where its lines landed, what it
/// committed, and what its editor changed.
///
/// One shape on all three seams, because the same vectors travel app → scene → input and the
/// edits travel back.
#[derive(Default)]
pub(crate) struct Fields {
    pub(crate) sources: Vec<text_input::Source>,
    pub(crate) layouts: Vec<text_input::Layout>,
    pub(crate) commits: Vec<text_input::Commit>,
    pub(crate) updates: Vec<text_input::Update>,
}

impl Fields {
    /// Takes the declarations `from` carries, which are the two the input thread reads.
    pub(crate) fn absorb(&mut self, from: &mut Self) {
        self.sources.append(&mut from.sources);
        self.layouts.append(&mut from.layouts);
    }

    fn is_empty(&self) -> bool {
        self.sources.is_empty()
            && self.layouts.is_empty()
            && self.commits.is_empty()
            && self.updates.is_empty()
    }

    fn clear(&mut self) {
        self.sources.clear();
        self.layouts.clear();
        self.commits.clear();
        self.updates.clear();
    }
}

// ── the four rows ───────────────────────────────────────────────────────────────────

row! {
    /// What the app thread hands the scene thread each flush.
    ///
    /// **A deferring producer.** The app thread flushes only into a spare it has taken,
    /// because a flush swaps the host's pending ops into the buffer it is given and flushing
    /// into a full one would drop what it holds. A skipped flush is counted and the ops stay
    /// put.
    Down,
    empty: |b| b.patch.is_empty()
        && b.chrome.is_empty()
        && b.correlations.is_empty()
        && b.translations.is_empty()
        && b.previews.is_empty()
        && b.reorders.is_empty()
        && b.values.is_empty()
        && b.regions.is_empty()
        && b.scrolls.is_empty()
        && b.fields.is_empty()
        && b.theme.is_none()
        && b.focus_outline.is_none()
        && b.preview_done.is_none()
        && b.declared.is_empty(),
    columns {
        /// Read by `Scene::apply`: the solve as `Bind` ops, the hit array as `Op::Hits`, the
        /// env it was solved under.
        patch: SinkPatch,
        /// Read by `Controls::adopt` on the scene thread.
        chrome: Vec<(ControlId, ChromeRow)>,
        correlations: Vec<crate::correlation::Route>,
        translations: Vec<(ControlId, NodeId, windows_scene::Translation)>,
        previews: Vec<(ControlId, NodeId)>,
        reorders: Vec<crate::widget::ReorderRow>,
        /// Read by `Controls::adopt` on the scene thread.
        values: Vec<(ControlId, ValueRow)>,
        /// Read by `present::apply` here, and relayed as pick rows to `Picks::sync`.
        regions: Vec<RegionOp>,
        /// Read by `ScrollTable::apply_ops`, and relayed as front rows to the router.
        scrolls: Vec<ScrollOp>,
        /// Read by `TextInput`: a field's source, its solved line boxes, and the text a commit
        /// carries back to its handler.
        fields: Fields,
        /// Read by `Controls::adopt` on the way past, and relayed whole.
        declared: Declared,
    }
    scalars {
        /// A new palette and the backdrop it implies, read by `Scene::set_backdrop` and by
        /// every region's own scope.
        theme: Option<(Scope, windows_scene::BackdropSpec)>,
        /// The window-owned outline, installed once before interaction.
        focus_outline: Option<NodeId>,
        /// Preview identity and the control whose reorder the application accepted.
        preview_done: Option<(u64, ControlId)>,
    }
}

row! {
    /// What the scene thread reports back to the app thread.
    ///
    /// **An appending producer.** The scene thread never defers: it keeps appending into its
    /// held buffer and retries on its next pass, so a batch the app thread has not drained
    /// grows rather than losing its oldest rows.
    Up,
    empty: |b| b.events.is_empty()
        && b.intents.is_empty()
        && b.reports.is_empty()
        && b.fields.is_empty()
        && b.size.is_none()
        && b.env.is_none()
        && b.preview_done.is_none(),
    columns {
        /// Read by `Host::field_update` and `deliver_field_commits`.
        fields: Fields,
        /// Read by `Overlays::scene` and by the device-loss arm.
        events: Vec<SceneEvent>,
        /// Read by `Host::dispatch`.
        intents: Vec<Intent>,
        /// Read by `Overlays::keys` — the discrete reports only. An intent per sample would
        /// put the app thread on the pointer's report rate.
        reports: Vec<Report>,
    }
    scalars {
        /// The client size in DIPs, where it changed. Read by `Host::set_window`.
        size: Option<Vector2>,
        /// Read by `Host::set_env`: the DPI every layout is solved against and the transform
        /// every colour passes.
        env: Option<Env>,
        /// Preview held until the app returns this identity with the callback's patch.
        preview_done: Option<u64>,
    }
}

row! {
    /// What the input thread hands the scene thread.
    ///
    /// **An appending producer**, as [`Up`] is: a deferred contact is a lost contact.
    ToScene,
    empty: |b| b.reports.is_empty()
        && !b.correlation_changed
        && b.springs_enabled.is_none()
        && b.intents.is_empty()
        && b.caption.is_none()
        && b.fields.is_empty()
        && b.reveals.is_empty()
        && b.automation.is_empty()
        && b.size.is_none()
        && b.env.is_none(),
    columns {
        /// Read by `Controls::tick` and the scene's retargets.
        reports: Vec<Report>,
        /// Edits the editor made, relayed to the app thread's own document.
        fields: Fields,
        /// Read by `ScrollTable::reveal_field`: a caret or a provider asked for a box to be
        /// brought into view.
        reveals: Vec<text_input::Reveal>,
        /// Read by `Controls::automation`: what a provider asked a control to do.
        automation: Vec<crate::uia::Action>,
        /// What a pick inside a region committed, carried to the application through the scene
        /// thread's own batch.
        intents: Vec<Intent>,
    }
    scalars {
        /// Read by `Controls::nonclient`: what the pointer is doing to a window command whose
        /// input the system took.
        caption: Option<CaptionState>,
        /// A correlation selection moved; retained reveal targets need a scene pass.
        correlation_changed: bool,
        /// Controls whether subsequent compositor springs animate or snap to their targets.
        springs_enabled: Option<bool>,
        /// The client size in DIPs, where it changed.
        size: Option<Vector2>,
        /// The display the window is on, where it changed.
        env: Option<Env>,
    }
}

row! {
    /// What the input thread needs to route a contact without asking another thread.
    ///
    /// **An appending producer**, as [`ToScene`] is: the scene thread retries rather than
    /// dropping a hit array the router has not seen.
    InputDown,
    empty: |b| !b.hits_changed
        && !b.trackers_changed
        && !b.regions_changed
        && !b.correlations_changed
        && b.fields.is_empty()
        && b.scope.is_none()
        && !b.scroll_changed
        && !b.translation_changed
        && !b.text_geometry_changed
        && b.declared.is_empty()
        && !b.tallies,
    columns {
        /// Read by `Router::tick`, `caption::hit` and `present::pick`. Neither `Clone` nor
        /// `Copy`, so it is filled through `HitTable::copy_from`, which reuses this
        /// allocation.
        hits: HitTable,
        /// Read by `HitTable::set_shadows` on the tick's own copy: each tracker's offset as
        /// one atomic word, so the router resolves a hit inside a viewport without a hop to
        /// the thread that owns the tracker.
        trackers: Vec<(NodeId, Arc<AtomicU64>)>,
        /// Read by `Picks::sync`: every region the pointer can be picked inside.
        regions: Vec<(ControlId, Live)>,
        correlations: Vec<crate::correlation::Route>,
        /// Read by `TextInput::source` and `TextInput::layout`.
        fields: Fields,
        /// What the app thread declared, carried through the scene thread untouched.
        declared: Declared,
    }
    scalars {
        /// Whether `hits` is a new array. A batch carrying only gesture edits costs the router
        /// no rebuild.
        hits_changed: bool,
        /// Whether `trackers` is a full listing to install, replacing the set held.
        trackers_changed: bool,
        /// Whether `regions` is a full listing to install, replacing the set held.
        regions_changed: bool,
        correlations_changed: bool,
        /// A new theme, so the input thread resolves its own metrics through the same scope.
        scope: Option<Scope>,
        /// Whether a tracker moved under a focused field, which is a layout change TSF must
        /// hear about even though no box was re-solved.
        text_geometry_changed: bool,
        scroll_changed: bool,
        translation_changed: bool,
        /// Whether the tallies below moved since the last batch, so the observer reads counts
        /// current to the last apply rather than to the last structural change.
        tallies: bool,
        /// The scene's own counts as of this batch, for the observer on the input thread.
        scene: SceneTally,
    }
}

// ── the tallies and the ops ─────────────────────────────────────────────────────────

/// How many times the scene thread has woken, how many patches it applied, and what its tree
/// holds.
#[derive(Copy, Clone, Default)]
pub struct SceneTally {
    pub wakes: u64,
    pub applies: u64,
    /// When the scene thread finished applying its last patch.
    ///
    /// Read where the apply completed rather than where this batch is taken, so measuring how
    /// long an input took to reach the compositor does not also measure how long the input
    /// thread took to hear about it. Absent until the first apply.
    pub applied_at: Option<std::time::Instant>,
    pub census: Census,
}

/// Counts what the app thread's seams did.
#[derive(Copy, Clone, Default)]
pub struct AppCensus {
    pub flushes: u64,
    /// Flushes that published nothing because the scene thread had not drained the previous
    /// buffer.
    pub skipped_flushes: u64,
    /// Batches taken off the [`Up`] seam.
    pub up_batches: u64,
}

/// One edit to the focus order, applied in the order the app thread emitted it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum FocusOp {
    /// Opens a scope. `trap` keeps the tab order inside it, `from` is the control it was
    /// opened from, and `restore_to` where focus lands when it closes.
    Push {
        id: ScopeId,
        trap: bool,
        from: Option<ControlId>,
        restore_to: Option<ControlId>,
    },
    Pop(ScopeId),
    Focus(Option<ControlId>),
    /// Sets a tab position; a negative index permits direct focus only.
    TabIndex(ControlId, i32),
    /// Moves one step in the tab order.
    Step {
        forward: bool,
    },
    /// Moves to an end of the tab order: `last` for the end, otherwise the start.
    End {
        last: bool,
    },
}

/// One edit to the set of presentation regions.
pub(crate) enum RegionOp {
    Active { sink: RegionId, active: bool },
    /// Registers a region and hands over the builder that makes its renderer. The builder is
    /// carried rather than called, because it runs on the present thread.
    Mount {
        sink: RegionId,
        size_node: Option<NodeId>,
        control: ControlId,
        queue: Queue,
        live: Live,
        build: Build,
        extent: Extent,
        /// The lexical scope the region was declared under, resolved where the mount is
        /// emitted: the renderer draws on a thread that has none.
        theme: Theme,
    },
    /// The region's box moved, so its buffers are reallocated for `extent`.
    Resize {
        sink: RegionId,
        extent: Extent,
    },
    Drop {
        sink: RegionId,
    },
}

/// One edit to the set of scroll containers.
pub(crate) enum ScrollOp {
    /// A container arrived. `front` is what the router reads; the other three decide whether the
    /// application observes the offset and how the thumb reveals, and have no other path across.
    Add {
        front: ScrollFront,
        thumb: Option<SpriteId>,
        reveal: crate::layout::Reveal,
        observe: bool,
    },
    /// The solve moved the thumb, which is the part of a container that changes after it is
    /// added.
    Thumb {
        viewport: NodeId,
        geom: ThumbGeom,
    },
    /// Where a list asked the tracker to put its content, so a row outside the realized window
    /// can be brought into view.
    Reveal {
        viewport: NodeId,
        to: Vector2,
    },
    Drop {
        viewport: NodeId,
    },
}

/// One scroll container, as the thread routing a contact over it needs it.
#[derive(Copy, Clone)]
pub(crate) struct ScrollFront {
    pub(crate) viewport: NodeId,
    pub(crate) tracker: TrackerId,
    /// The viewport's own control, which is what a hover names.
    pub(crate) hover: ControlId,
    /// The rail's, which is what a grab names.
    pub(crate) grab: ControlId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_signals_only_when_armed() {
        let ring = Ring::new().expect("an event is available");
        ring.ring();
        assert!(!ring.event.take(), "an unarmed ring reached the kernel");
        ring.arm();
        ring.ring();
        assert!(ring.event.take(), "an armed ring did not signal");
        ring.ring();
        assert!(
            !ring.event.take(),
            "a second ring signalled without a second arm"
        );
    }

    #[test]
    fn a_ring_between_arm_and_wait_is_not_lost() {
        let ring = Ring::new().expect("an event is available");
        ring.arm();
        // The producer rings after the sleeper armed but before it waited.
        ring.ring();
        // The wait returns at once on the auto-reset event left signalled.
        ring.wait();
        assert!(!ring.armed.load(Acquire));
    }

    #[test]
    fn disarm_leaves_the_event_clear() {
        let ring = Ring::new().expect("an event is available");
        ring.arm();
        ring.disarm();
        ring.ring();
        assert!(!ring.event.take(), "a disarmed ring signalled");
    }

    #[test]
    fn a_slot_round_trips_a_box() {
        let post = Slot::<u32>::empty();
        assert!(post.put(Box::new(7)).is_ok());
        assert_eq!(post.take().map(|b| *b), Some(7));
    }

    #[test]
    fn an_occupied_slot_hands_the_box_back() {
        let post = Slot::<u32>::empty();
        assert!(post.put(Box::new(1)).is_ok());
        let returned = post.put(Box::new(2)).expect_err("the slot was occupied");
        assert_eq!(*returned, 2, "the caller lost its buffer");
        assert_eq!(
            post.take().map(|b| *b),
            Some(1),
            "the occupant was replaced"
        );
    }

    #[test]
    fn an_empty_slot_takes_nothing() {
        let post = Slot::<u32>::empty();
        assert!(post.take().is_none());
        assert!(post.put(Box::new(3)).is_ok());
        assert!(post.take().is_some());
        assert!(post.take().is_none(), "a second take found the slot full");
    }

    /// Counts its own drops, so a leaked or double-freed occupant is visible.
    struct Counted(&'static AtomicU64);

    impl Drop for Counted {
        fn drop(&mut self) {
            // relaxed: the test joins nothing and reads the counter on this thread, so the
            // count needs no ordering against anything else.
            self.0.fetch_add(1, Relaxed);
        }
    }

    #[test]
    fn dropping_a_slot_frees_its_occupant() {
        static DROPS: AtomicU64 = AtomicU64::new(0);
        {
            let post = Slot::empty();
            assert!(post.put(Box::new(Counted(&DROPS))).is_ok());
            assert_eq!(DROPS.load(Relaxed), 0);
        }
        assert_eq!(DROPS.load(Relaxed), 1, "the occupant was not freed");

        let post = Slot::empty();
        assert!(post.put(Box::new(Counted(&DROPS))).is_ok());
        drop(post.take());
        assert_eq!(DROPS.load(Relaxed), 2);
        drop(post);
        assert_eq!(DROPS.load(Relaxed), 2, "an emptied slot freed twice");
    }

    #[test]
    fn a_slot_carries_a_sequence_between_two_threads() {
        const COUNT: u64 = 1000;
        let post = Arc::new(Slot::<u64>::empty());
        let producer = {
            let post = Arc::clone(&post);
            std::thread::spawn(move || {
                for n in 0..COUNT {
                    let mut buf = Box::new(n);
                    // The producer that must not wait: an occupied slot means the consumer is
                    // behind, so the buffer stays with this side and is offered again.
                    while let Err(back) = post.put(buf) {
                        buf = back;
                        std::hint::spin_loop();
                    }
                }
            })
        };

        let mut seen = 0;
        let mut last = None;
        while seen < COUNT {
            if let Some(value) = post.take() {
                if let Some(previous) = last {
                    assert!(*value > previous, "{value} arrived after {previous}");
                }
                last = Some(*value);
                seen += 1;
            } else {
                std::hint::spin_loop();
            }
        }
        producer.join().expect("the producer ran to completion");
        assert_eq!(last, Some(COUNT - 1));
        assert!(post.take().is_none(), "the slot was left occupied");
    }

    // ── the seam's producer discipline ───────────────────────────────────────────

    /// A returned buffer is not work, and a spare nobody asked for wakes nobody.
    #[test]
    fn a_returned_spare_rings_only_a_producer_that_asked_for_one() {
        let link = Link::<ToScene>::new();
        // The producer's first buffer is its own; the seam's spare is what it swaps for.
        let mut held = Box::<ToScene>::default();

        // An empty batch never crosses, whatever the seam's state.
        assert!(!link.send(&mut held), "an empty batch crossed");
        assert!(link.take().is_none());

        held.reports.push(Report::CaptureLost);
        assert!(link.send(&mut held), "a filled batch did not cross");
        let first = link.take().expect("the batch is in the mailbox");
        // Nobody is waiting: the producer took its spare at the start and never went without.
        assert!(!link.give(first), "a spare nobody wanted rang the producer");

        // Now fill and send twice without a drain, so the second send finds no spare.
        held.reports.push(Report::CaptureLost);
        assert!(link.send(&mut held));
        held.reports.push(Report::CaptureLost);
        assert!(
            !link.send(&mut held),
            "a second batch crossed over the first"
        );
        let second = link.take().expect("the first batch is still there");
        assert!(link.give(second), "the waiting producer was not rung");
        // The flag the ring answered is spent: taking that spare and handing it straight back
        // rings nobody, because nothing asked for it in between.
        let spare = link.spare().expect("the returned spare");
        assert!(!link.give(spare), "a spent flag rang the producer again");
        let spare = link
            .take()
            .map_or_else(|| link.spare().expect("spare"), |b| b);
        assert!(!link.give(spare), "a spare nobody wanted rang the producer");
    }

    /// Every row's generated emptiness test agrees with its generated `clear`.
    #[test]
    fn a_row_cleared_is_a_row_empty() {
        fn cleared_is_empty<T: Row>(mut row: T, fill: impl FnOnce(&mut T)) {
            fill(&mut row);
            assert!(!row.is_empty(), "a filled row reported empty");
            row.clear();
            assert!(row.is_empty(), "a cleared row reported work");
        }

        cleared_is_empty(Down::default(), |b| {
            b.declared.caption = [Some(ControlId::NONE), None, None];
        });
        cleared_is_empty(Down::default(), |b| b.focus_outline = Some(NodeId::NONE));
        cleared_is_empty(Up::default(), |b| b.size = Some(Vector2 { x: 8.0, y: 8.0 }));
        cleared_is_empty(ToScene::default(), |b| {
            b.reports.push(Report::CaptureLost);
        });
        cleared_is_empty(ToScene::default(), |b| b.springs_enabled = Some(false));
        cleared_is_empty(InputDown::default(), |b| b.hits_changed = true);
    }

    /// A column keeps its allocation across a clear, which is what makes a steady-state batch
    /// cost nothing.
    #[test]
    fn clearing_a_row_keeps_its_capacity() {
        let mut down = Down::default();
        down.chrome.reserve(8);
        down.values.reserve(8);
        down.regions.reserve(8);
        down.scrolls.reserve(8);
        down.declared.released.push(ControlId::NONE);
        down.declared.focus.push(FocusOp::Step { forward: true });

        let before = (
            down.chrome.capacity(),
            down.values.capacity(),
            down.regions.capacity(),
            down.scrolls.capacity(),
            down.declared.released.capacity(),
            down.declared.focus.capacity(),
        );
        Row::clear(&mut down);

        assert!(down.declared.released.is_empty() && down.declared.focus.is_empty());
        assert_eq!(
            before,
            (
                down.chrome.capacity(),
                down.values.capacity(),
                down.regions.capacity(),
                down.scrolls.capacity(),
                down.declared.released.capacity(),
                down.declared.focus.capacity(),
            )
        );
    }
}
