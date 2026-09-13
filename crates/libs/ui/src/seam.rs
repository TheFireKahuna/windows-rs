//! The thread seams: what crosses between the input, scene and app threads, and how.
//!
//! Every crossing is a single-slot mailbox over a buffer allocated once, and every wait is
//! on a doorbell that rings only on the parked→running edge. No mutex sits on a seam, no
//! thread waits on another, and nothing here allocates once the buffers exist.

// The rows below are written by the thread on one side of a seam and read by the thread on
// the other; this module defines them and reads none of them itself.
#![allow(dead_code)]
// The mailbox moves a `Box` between threads through a raw pointer, which is the one
// mechanism that lets both sides be wait-free.

use crate::gesture::GestureDecl;
use crate::input::{Report, ScopeId};
use crate::layout::{Reveal, ThumbGeom};
use crate::present::{Build, Live};
use crate::uia::Seeds;
use crate::widget::{ChromeRow, Intent};
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::Arc;
use windows_numerics::Vector2;
use windows_present::{Extent, Queue, RegionKey};
use windows_scene::{
    Census, ControlId, Env, HitTable, NodeId, Observed, RegionId, SceneEvent, SinkPatch, SpriteId,
    TrackerId,
};
use windows_window::{CaptionState, Event};

pub(crate) use crate::build::ScrollId;

/// A doorbell one thread parks on and any thread rings.
///
/// `arm` is called by the sleeper before it re-checks its inboxes and waits; `ring` by a
/// producer after it has published. The kernel event is signalled only when the sleeper had
/// armed it, so a producer ringing a running consumer costs one atomic and no system call —
/// and a ring that lands between the sleeper's arm and its wait is not lost, because the
/// event is auto-reset and stays signalled until the wait consumes it.
pub(crate) struct Ring {
    armed: AtomicBool,
    /// Set by every ring and taken by the sleeper after it arms, so a ring that landed while
    /// the sleeper was running is seen without the sleeper inspecting any inbox: the flag is
    /// the one summary of "something was published since you last looked".
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
        // release: the sleeper's inbox reads that follow happen after the arm is visible, so
        // a producer that publishes and then sees `armed` clear knows the sleeper is past its
        // re-check and about to wait — and rings.
        self.armed.store(true, Ordering::Release);
    }

    /// Rings, waking the sleeper if it is parked or about to park. One system call per
    /// parked→running edge, none while the consumer is running.
    pub(crate) fn ring(&self) {
        // release: the publish this ring announces is ordered before the flag the sleeper
        // takes with acquire.
        self.pending.store(true, Ordering::Release);
        // acq_rel: pairs with `arm`; the swap orders this producer's publish before the
        // sleeper's next inbox read.
        if self.armed.swap(false, Ordering::AcqRel) {
            self.event.signal();
        }
    }

    /// Returns whether a ring has landed since the last take, clearing it. The sleeper calls
    /// this after [`arm`](Self::arm): `true` means skip the wait and drain.
    pub(crate) fn take_pending(&self) -> bool {
        // acq_rel: pairs with the release in `ring`, so the publish behind the flag is
        // visible to the drain that follows.
        self.pending.swap(false, Ordering::AcqRel)
    }

    /// Parks until rung. The sleeper disarms on the way out, so a ring that arrives while it
    /// runs signals nothing and the next `arm` starts clean.
    pub(crate) fn wait(&self) {
        self.event.wait(u32::MAX);
        self.armed.store(false, Ordering::Release);
    }

    /// The event, for a sleeper that waits on it alongside other handles rather than through
    /// [`wait`](Self::wait). Such a sleeper disarms itself after the wait returns.
    pub(crate) fn event(&self) -> &Event {
        &self.event
    }

    /// Disarms without waiting: the sleeper found work on its re-check and is not parking.
    pub(crate) fn disarm(&self) {
        self.armed.store(false, Ordering::Release);
    }
}

const _: () = {
    const fn assert<T: Send + Sync>() {}
    assert::<Ring>();
};

// ── the mailbox ──────────────────────────────────────────────────────────────────

/// A one-slot mailbox carrying a `Box<T>` from one producer to one consumer.
///
/// Wait-free on both sides: a producer that finds the slot full keeps its buffer and a
/// consumer that finds it empty gets `None`. Neither ever blocks, so a seam can never make
/// one thread wait on another.
///
/// Single-producer, single-consumer. Nothing here bounds how many boxes exist, so the buffer
/// count is the producer's: the disciplines on [`Down`] and [`Up`] are what keep it at one.
pub(crate) struct Mailbox<T>(AtomicPtr<T>);

// SAFETY: the box behind the pointer is owned by exactly one side at a time. A producer owns
// it until `put` publishes the pointer, and after that only `take` can reach it, which
// removes the pointer from the slot before returning the box. No two threads can hold the
// same `T`, so crossing the seam needs only `T: Send`.
unsafe impl<T: Send> Send for Mailbox<T> {}
// SAFETY: as above; the shared state is one pointer and every access to it is atomic.
unsafe impl<T: Send> Sync for Mailbox<T> {}

impl<T> Mailbox<T> {
    /// Returns an empty mailbox.
    pub(crate) const fn empty() -> Self {
        Self(AtomicPtr::new(ptr::null_mut()))
    }

    /// Publishes `buf`, or hands it back when the slot is still full.
    ///
    /// # Errors
    ///
    /// The consumer has not taken the previous buffer. The caller keeps the box and retries
    /// on its next pass.
    pub(crate) fn put(&self, buf: Box<T>) -> Result<(), Box<T>> {
        let raw = Box::into_raw(buf);
        // acq_rel on success: the release half publishes every write the producer made into
        // the box before the pointer becomes visible, and the acquire half pairs with the
        // consumer's `take`, so the slot being refilled is known to have been emptied.
        // acquire on failure: the load that read the occupant orders against the `take` that
        // will clear it, so the next attempt sees a slot no staler than this one did.
        match self
            .0
            .compare_exchange(ptr::null_mut(), raw, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(()),
            // SAFETY: the exchange failed, so the pointer was never published and this thread
            // still owns the allocation `Box::into_raw` handed out above.
            Err(_) => Err(unsafe { Box::from_raw(raw) }),
        }
    }

    /// Takes the buffer, leaving the slot empty. `None` when the producer has published
    /// nothing since the last take.
    pub(crate) fn take(&self) -> Option<Box<T>> {
        // acq_rel: the acquire half pairs with the producer's `put`, so every write into the
        // box happens before the consumer reads it; the release half publishes the emptied
        // slot, so the producer's next `put` cannot observe the slot as full after this swap.
        let raw = self.0.swap(ptr::null_mut(), Ordering::AcqRel);
        // SAFETY: a non-null pointer in the slot was put there by `Box::into_raw` in `put`,
        // and the swap removed it, so this thread is now its only owner.
        (!raw.is_null()).then(|| unsafe { Box::from_raw(raw) })
    }
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T> Drop for Mailbox<T> {
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

// ── app thread → scene thread ────────────────────────────────────────────────────

/// What the app thread hands the scene thread each flush.
///
/// **A deferring producer.** The app thread holds one `Down` and produces into it. With the
/// mailbox still full it has no spare buffer, so it skips the flush and counts it in
/// [`AppCensus::skipped_flushes`]: what it would have published is still in the model, and
/// the next flush carries it. Every buffer here is allocated once and recycled by
/// [`clear`](Self::clear).
#[derive(Default)]
pub(crate) struct Down {
    pub field_sources: Vec<crate::text_input::Source>,
    pub field_layouts: Vec<crate::text_input::Layout>,
    pub field_commits: Vec<crate::text_input::Commit>,
    pub patch: SinkPatch,
    pub chrome: Vec<ChromeRow>,
    pub gestures: Vec<(ControlId, GestureDecl)>,
    pub released: Vec<ControlId>,
    /// The caption buttons, in the order minimise, maximise, close.
    pub caption: Option<[Option<ControlId>; 3]>,
    pub regions: Vec<RegionOp>,
    pub scrolls: Vec<ScrollOp>,
    pub focus: Vec<FocusOp>,
    pub seeds: Option<Seeds>,
    pub census: AppCensus,
}

impl Down {
    /// Returns whether the batch carries nothing the scene thread would act on.
    pub(crate) fn is_empty(&self) -> bool {
        self.field_sources.is_empty()
            && self.field_layouts.is_empty()
            && self.field_commits.is_empty()
            && self.patch.is_empty()
            && self.chrome.is_empty()
            && self.gestures.is_empty()
            && self.released.is_empty()
            && self.caption.is_none()
            && self.regions.is_empty()
            && self.scrolls.is_empty()
            && self.focus.is_empty()
            && self.seeds.is_none()
    }

    /// Empties every buffer, keeping its allocation, and resets the scalars.
    pub(crate) fn clear(&mut self) {
        self.field_sources.clear();
        self.field_layouts.clear();
        self.field_commits.clear();
        self.patch.clear();
        self.chrome.clear();
        self.gestures.clear();
        self.released.clear();
        self.caption = None;
        self.regions.clear();
        self.scrolls.clear();
        self.focus.clear();
        // The seed blob travels with the publish rather than with this buffer: a producer
        // that pools it takes it out of the drained `Down` before recycling.
        self.seeds = None;
        self.census = AppCensus::default();
    }
}

/// Fails to compile if a row ever gains a field that is not `Send`.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<Down>();
    assert_send::<RegionOp>();
    assert_send::<ScrollOp>();
};

// ── scene thread → app thread ────────────────────────────────────────────────────

/// What the scene thread reports back to the app thread.
///
/// **An appending producer.** The scene thread never defers: it keeps appending into its
/// held buffer and retries [`Mailbox::put`] on its next pass, so a batch the app thread has
/// not drained grows rather than losing its oldest rows. [`AppCensus::up_batches`] counts
/// what the app thread took.
#[derive(Default)]
pub(crate) struct Up {
    pub text: Vec<crate::text_input::Update>,
    pub field_commits: Vec<crate::text_input::Commit>,
    pub events: Vec<SceneEvent>,
    pub intents: Vec<Intent>,
    pub reports: Vec<Report>,
    /// The client size in DIPs, where it changed.
    pub window: Option<Vector2>,
    pub env: Option<Env>,
}

impl Up {
    /// Empties every buffer, keeping its allocation, and resets the scalars.
    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.field_commits.clear();
        self.events.clear();
        self.intents.clear();
        self.reports.clear();
        self.window = None;
        self.env = None;
    }
}

/// Fails to compile if a row ever gains a field that is not `Send`.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<Up>();
};

// ── input thread → scene thread ──────────────────────────────────────────────────

/// What the input thread hands the scene thread.
///
/// **An appending producer**, as [`Up`] is: a deferred contact is a lost contact, so the
/// held buffer keeps growing until a [`Mailbox::put`] lands.
#[derive(Default)]
pub(crate) struct ToScene {
    pub automation: Vec<crate::uia::Action>,
    pub reveals: Vec<crate::text_input::Reveal>,
    pub text: Vec<crate::text_input::Update>,
    pub reports: Vec<Report>,
    /// What a pick inside a region committed, queued on the input thread and carried to the
    /// application through the scene thread's own batch.
    pub intents: Vec<Intent>,
    pub nonclient: Option<CaptionState>,
    /// The client size in DIPs, where it changed.
    pub window: Option<Vector2>,
    pub env: Option<Env>,
}

impl ToScene {
    /// Empties every buffer, keeping its allocation, and resets the scalars.
    pub(crate) fn clear(&mut self) {
        self.automation.clear();
        self.reveals.clear();
        self.text.clear();
        self.reports.clear();
        self.intents.clear();
        self.nonclient = None;
        self.window = None;
        self.env = None;
    }
}

/// Fails to compile if a row ever gains a field that is not `Send`.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<ToScene>();
};

// ── scene thread → input thread ──────────────────────────────────────────────────

/// What the input thread needs to route a contact without asking another thread.
///
/// **An appending producer**, as [`ToScene`] is: the scene thread retries its
/// [`Mailbox::put`] rather than dropping a hit array the router has not seen.
///
/// [`HitTable`] is neither `Clone` nor `Copy`, so filling `hits` from another table goes
/// through [`HitTable::replace`] over that table's [`entries`](HitTable::entries), which
/// reuses this one's allocation. [`hits_changed`](Self::hits_changed) says whether the array
/// in this batch is a new one, so a batch carrying only gesture edits costs the router no
/// rebuild.
#[derive(Default)]
pub(crate) struct InputDown {
    pub text_geometry_changed: bool,
    pub seeds: Option<Seeds>,
    pub field_sources: Vec<crate::text_input::Source>,
    pub field_layouts: Vec<crate::text_input::Layout>,
    pub hits: HitTable,
    pub hits_changed: bool,
    pub gestures: Vec<(ControlId, GestureDecl)>,
    pub released: Vec<ControlId>,
    pub regions: Vec<RegionPick>,
    /// The caption buttons, in the order minimise, maximise, close.
    pub caption: Option<[Option<ControlId>; 3]>,
    pub trackers: Vec<TrackerShadow>,
    /// Whether `trackers` is a full listing to install, replacing the set held. `false`
    /// leaves the held set alone.
    pub trackers_changed: bool,
    /// The app thread's tallies as of the batch this one followed, for the observer.
    pub app: AppCensus,
    /// Whether `regions` is a full listing to install, replacing the set held.
    pub regions_changed: bool,
    pub focus: Vec<FocusOp>,
    /// The scene's tallies as of this batch, for the observer on the input thread.
    pub census: Census,
    /// How many times the scene thread has woken and how many patches it has applied.
    pub scene_wakes: u64,
    pub scene_applies: u64,
}

impl InputDown {
    /// Empties every buffer, keeping its allocation, and resets the scalars.
    pub(crate) fn clear(&mut self) {
        self.text_geometry_changed = false;
        self.seeds = None;
        self.field_sources.clear();
        self.field_layouts.clear();
        self.hits.replace(&[]);
        self.hits_changed = false;
        self.gestures.clear();
        self.released.clear();
        self.regions.clear();
        self.caption = None;
        self.trackers.clear();
        self.trackers_changed = false;
        self.regions_changed = false;
        self.focus.clear();
    }
}

/// Fails to compile if a row ever gains a field that is not `Send`.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<InputDown>();
};

// ── the rows ─────────────────────────────────────────────────────────────────────

/// One edit to the focus order, applied in the order the app thread emitted it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum FocusOp {
    /// Opens a scope. `trap` keeps the tab order inside it, `from` is the control it was
    /// opened from, and `restore_to` where focus lands when it closes.
    PushScope {
        scope: ScopeId,
        trap: bool,
        from: ControlId,
        restore_to: Option<ControlId>,
    },
    PopScope(ScopeId),
    Focus(Option<ControlId>),
    /// Moves one step in the tab order.
    Step {
        forward: bool,
    },
    /// Moves to an end of the tab order: `last` for the end, otherwise the start.
    StepToEnd {
        last: bool,
    },
}

/// One edit to the set of presentation regions.
pub(crate) enum RegionOp {
    /// Registers a region and hands over the builder that makes its renderer. The builder is
    /// carried rather than called, because it runs on the present thread.
    Mount {
        key: RegionKey,
        sink: RegionId,
        control: Option<ControlId>,
        live: Live,
        extent: Extent,
        queue: Queue,
        build: Build,
    },
    /// The region's box moved, so its buffers are reallocated for `extent`.
    Resize {
        key: RegionKey,
        extent: Extent,
    },
    Drop {
        key: RegionKey,
    },
}

/// One edit to the set of scroll containers.
pub(crate) enum ScrollOp {
    Add(ScrollFront),
    /// The solve moved the thumb, which is the part of a container that changes after it is
    /// added.
    Geom {
        id: ScrollId,
        geom: ThumbGeom,
    },
    Drop(ScrollId),
}

/// One scroll container, as the thread routing a contact over it needs it.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ScrollFront {
    pub viewport: NodeId,
    /// Application state is consumed by virtualization or an explicit observer.
    pub observe: bool,
    pub id: ScrollId,
    pub tracker: TrackerId<Observed>,
    pub thumb: Option<SpriteId>,
    /// The viewport's own control, which is what a hover names.
    pub control: Option<ControlId>,
    /// The rail's, which is what a grab names.
    pub grab: Option<ControlId>,
    pub reveal: Reveal,
    pub last: ThumbGeom,
}

/// One region the pointer can be picked inside, with the handles that resolve the pick.
#[derive(Clone)]
pub(crate) struct RegionPick {
    pub key: RegionKey,
    pub control: ControlId,
    pub live: Live,
}

/// A tracker's offset as the router reads it, without a hop to the thread that owns the
/// tracker.
///
/// The compositor evaluates a tracker in another process, so the scene thread writes what it
/// last observed into the shared word and the router reads it whenever it resolves a hit
/// inside the viewport.
#[derive(Clone)]
pub(crate) struct TrackerShadow {
    pub viewport: NodeId,
    /// Two `f32` offsets in one word: x in the high 32 bits, y in the low.
    pub shadow: Arc<AtomicU64>,
}

impl TrackerShadow {
    /// Returns the word holding offsets `x` and `y`.
    pub(crate) const fn pack(x: f32, y: f32) -> u64 {
        ((x.to_bits() as u64) << 32) | y.to_bits() as u64
    }

    /// Returns the offsets `word` holds, as `(x, y)`.
    pub(crate) const fn unpack(word: u64) -> (f32, f32) {
        (
            f32::from_bits((word >> 32) as u32),
            f32::from_bits(word as u32),
        )
    }
}

/// Counts what the app thread's seams did.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct AppCensus {
    pub flushes: u64,
    /// Flushes that published nothing because the scene thread had not drained the previous
    /// buffer.
    pub skipped_flushes: u64,
    /// Batches taken off the [`Up`] seam.
    pub up_batches: u64,
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
        assert!(!ring.armed.load(Ordering::Acquire));
    }

    #[test]
    fn disarm_leaves_the_event_clear() {
        let ring = Ring::new().expect("an event is available");
        ring.arm();
        ring.disarm();
        ring.ring();
        assert!(!ring.event.take(), "a disarmed ring signalled");
    }

    // ── the mailbox ──────────────────────────────────────────────────────────────

    #[test]
    fn a_mailbox_round_trips_a_box() {
        let post = Mailbox::<u32>::empty();
        assert!(post.put(Box::new(7)).is_ok());
        assert_eq!(post.take().map(|b| *b), Some(7));
    }

    #[test]
    fn an_occupied_slot_hands_the_box_back() {
        let post = Mailbox::<u32>::empty();
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
        let post = Mailbox::<u32>::empty();
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
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn dropping_a_mailbox_frees_its_occupant() {
        static DROPS: AtomicU64 = AtomicU64::new(0);
        {
            let post = Mailbox::empty();
            assert!(post.put(Box::new(Counted(&DROPS))).is_ok());
            assert_eq!(DROPS.load(Ordering::Relaxed), 0);
        }
        assert_eq!(
            DROPS.load(Ordering::Relaxed),
            1,
            "the occupant was not freed"
        );

        let post = Mailbox::empty();
        assert!(post.put(Box::new(Counted(&DROPS))).is_ok());
        drop(post.take());
        assert_eq!(DROPS.load(Ordering::Relaxed), 2);
        drop(post);
        assert_eq!(
            DROPS.load(Ordering::Relaxed),
            2,
            "an emptied slot freed twice"
        );
    }

    #[test]
    fn a_mailbox_carries_a_sequence_between_two_threads() {
        const COUNT: u64 = 1000;
        let post = Arc::new(Mailbox::<u64>::empty());
        let producer = {
            let post = Arc::clone(&post);
            std::thread::spawn(move || {
                for n in 0..COUNT {
                    let mut buf = Box::new(n);
                    // The producer that must not wait: an occupied slot means the consumer
                    // is behind, so the buffer stays with this side and is offered again.
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

    // ── the rows ─────────────────────────────────────────────────────────────────

    #[test]
    fn a_tracker_offset_round_trips_through_one_word() {
        for (x, y) in [
            (0.0_f32, 0.0_f32),
            (1.0, -1.0),
            (-0.5, 1024.75),
            (f32::MIN, f32::MAX),
            (f32::INFINITY, f32::NEG_INFINITY),
        ] {
            let word = TrackerShadow::pack(x, y);
            assert_eq!(TrackerShadow::unpack(word), (x, y));
        }
        // x occupies the high half, so a y of zero leaves the low half clear.
        assert_eq!(
            TrackerShadow::pack(1.0, 0.0),
            (1.0_f32.to_bits() as u64) << 32
        );
    }

    #[test]
    fn clearing_a_down_keeps_its_capacity() {
        let mut down = Down::default();
        down.chrome.reserve(8);
        down.gestures.reserve(8);
        down.released.push(ControlId::NONE);
        down.regions.reserve(8);
        down.scrolls.reserve(8);
        down.focus.push(FocusOp::Step { forward: true });
        down.caption = Some([None, None, None]);
        down.seeds = Some(Seeds::default());
        down.census.flushes = 3;

        let before = (
            down.chrome.capacity(),
            down.gestures.capacity(),
            down.released.capacity(),
            down.regions.capacity(),
            down.scrolls.capacity(),
            down.focus.capacity(),
        );
        down.clear();

        assert!(down.released.is_empty() && down.focus.is_empty());
        assert_eq!(down.caption, None);
        assert!(down.seeds.is_none());
        assert_eq!(down.census, AppCensus::default());
        assert_eq!(
            before,
            (
                down.chrome.capacity(),
                down.gestures.capacity(),
                down.released.capacity(),
                down.regions.capacity(),
                down.scrolls.capacity(),
                down.focus.capacity(),
            )
        );
    }

    #[test]
    fn clearing_an_up_keeps_its_capacity() {
        let mut up = Up::default();
        up.events.reserve(8);
        up.intents.reserve(8);
        up.reports.reserve(8);
        up.window = Some(Vector2 { x: 4.0, y: 5.0 });

        let before = (
            up.events.capacity(),
            up.intents.capacity(),
            up.reports.capacity(),
        );
        up.clear();

        assert!(up.events.is_empty() && up.intents.is_empty() && up.reports.is_empty());
        assert_eq!(up.window, None);
        assert_eq!(
            before,
            (
                up.events.capacity(),
                up.intents.capacity(),
                up.reports.capacity()
            )
        );
    }

    #[test]
    fn clearing_a_to_scene_keeps_its_capacity() {
        let mut to = ToScene::default();
        to.reports.reserve(8);
        to.window = Some(Vector2 { x: 1.0, y: 2.0 });
        to.nonclient = Some(CaptionState::default());

        let before = to.reports.capacity();
        to.clear();

        assert!(to.reports.is_empty());
        assert_eq!(to.window, None);
        assert_eq!(to.nonclient, None);
        assert_eq!(before, to.reports.capacity());
    }

    #[test]
    fn clearing_an_input_down_keeps_its_capacity() {
        let mut down = InputDown::default();
        down.gestures.reserve(8);
        down.released.push(ControlId::NONE);
        down.regions.reserve(8);
        down.trackers.reserve(8);
        down.focus.push(FocusOp::StepToEnd { last: true });
        down.caption = Some([None, None, None]);
        down.hits_changed = true;

        let before = (
            down.gestures.capacity(),
            down.released.capacity(),
            down.regions.capacity(),
            down.trackers.capacity(),
            down.focus.capacity(),
        );
        down.clear();

        assert!(down.hits.entries().is_empty());
        assert!(!down.hits_changed);
        assert_eq!(down.caption, None);
        assert_eq!(
            before,
            (
                down.gestures.capacity(),
                down.released.capacity(),
                down.regions.capacity(),
                down.trackers.capacity(),
                down.focus.capacity(),
            )
        );
    }
}
