//! The presentation manager, and what one present costs.
//!
//! Presents are queued **per manager**, so a group is the unit of "one present shows
//! everything that moved, and the regions that did not move cost nothing". Regions own
//! their buffers, because a buffer is allocated at one size; the manager is what they
//! share, and where the saving is.
//!
//! That saving is real and it is usually not worth taking. A present that rebinds more
//! than one surface **cannot be an independent flip** — a property of the queue, not of
//! the surfaces. Measured with both regions opaque and each independently reaching
//! independent flip: folding them onto one queue saves this process **1.8% of a core**
//! and costs `dwm.exe` **ten**, because sharing trades independent flip for *overlay
//! scanout*, where DWM runs a composition frame for every one of our presents to program
//! the plane. Statistics still read `composed 0` throughout, so `composed 0` is not
//! evidence of a flip; the tell is DWM's frame *rate*.
//!
//! So: share a queue for regions that are composed anyway, never to save presents, and
//! **merge regions that share a clock into one region with parts** rather than onto one
//! queue — a merged region keeps its plane and a shared queue does not.

use super::*;
use core::cell::Cell;

/// Which present queue a region asks for.
///
/// A region names the queue rather than the group object, so the group set is this
/// crate's business and a consumer states only its timing requirement.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Queue {
    /// A queue of this region's own, guaranteed. For the region whose plane the layout
    /// protects, and which must never have another region's timing in front of it.
    Solo,
    /// The named queue, shared with every region that asks for it.
    ///
    /// One present shows every region that drew and the ones that did not cost nothing —
    /// at the price of the plane, for every region in the queue, **whenever it has more
    /// than one member**. With a single member it is `Solo` in all but name, which is how
    /// a per-frame card gets a plane in the common case and degrades predictably when a
    /// second one mounts.
    ///
    /// The reason a shared card queue exists at all rather than a queue per card is the
    /// asymmetry in how the two shapes fail: an extra `Solo` queue that does not get a
    /// plane costs one present, ~100 µs, linear and ours — while an extra member on a
    /// `Shared` queue costs that queue's plane outright. We do not choose which surfaces
    /// get planes, so a third `Solo` candidate competes against the hero and can cost
    /// *it* the plane. One shared queue firewalls the degradation to the cards.
    Shared(&'static str),
}

/// Whether a present wakes the CPU when the display shows it.
///
/// The interrupt is how the CPU learns a present was shown: it runs kernel code that
/// updates the buffer-available events, the retiring fence and the statistics queue.
/// Nothing reads any of that for the earlier frames of a batch before the next wake, so
/// they can be reported lazily — **~26 µs of our own CPU per interrupt, about 14% of a
/// present**, and it is what lets the GPU run the flip queue with the CPU out of it.
///
/// Correctness never depends on this. Deferred feedback makes a wait take longer, not
/// return the wrong answer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Interrupt {
    /// Wake the CPU. For the one present of a batch that the next pass waits behind.
    Raise,
    /// Report lazily. For every present the producer will not look at before it sleeps.
    Defer,
}

/// How one present reached one display.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Instance {
    /// DWM composed the buffer into its back buffer. The expensive case, and the only
    /// one available without displayable buffers.
    Composed,
    /// The display controller scanned the buffer out: an overlay plane or independent
    /// flip. DWM did no drawing for it.
    Scanout,
    /// DWM composed the buffer into an intermediate, because something above it in the
    /// visual tree needed an effect pass.
    Intermediate,
}

/// What happened to one issued present.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Queued for display.
    Queued,
    /// Superseded by a later present before it could be shown. A steady stream of these
    /// means the producer is presenting faster than the display can show.
    Skipped,
    /// Cancelled.
    Canceled,
}

/// One record dequeued from the manager's statistics queue.
#[derive(Copy, Clone, Debug)]
pub enum PresentStatistic {
    /// Whether this producer is being skipped.
    Status { present_id: u64, outcome: Outcome },
    /// Whether DWM composed the buffer or the display scanned it out.
    Frame {
        present_id: u64,
        instance: Instance,
        /// Whether the buffer had to be copied to another adapter to be displayed — a
        /// reason to reallocate on the display's adapter.
        cross_adapter: bool,
    },
    /// The present reached a display plane, bypassing the compositor.
    ///
    /// The **only** report such a present makes at all: composition frames are not issued
    /// for flipped presents, so a caller reading only those sees silence and cannot tell
    /// success from a surface displaying nothing.
    Flip { present_id: u64 },
}

/// A running count of what a group's presents did, drained from the statistics queue.
///
/// Lives here rather than in a consumer because every consumer wants the same five
/// numbers and the decode is not obvious — in particular that `flipped` and `composed`
/// come from two different record kinds and never both describe one present.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PresentTally {
    pub queued: u32,
    pub skipped: u32,
    pub canceled: u32,
    pub flipped: u32,
    pub composed: u32,
    pub scanout: u32,
}

impl PresentTally {
    /// Folds one record in.
    pub fn record(&mut self, stat: PresentStatistic) {
        match stat {
            PresentStatistic::Status { outcome, .. } => match outcome {
                Outcome::Queued => self.queued += 1,
                Outcome::Skipped => self.skipped += 1,
                Outcome::Canceled => self.canceled += 1,
            },
            PresentStatistic::Frame { instance, .. } => match instance {
                Instance::Scanout => self.scanout += 1,
                Instance::Composed | Instance::Intermediate => self.composed += 1,
            },
            PresentStatistic::Flip { .. } => self.flipped += 1,
        }
    }
}

struct Inner {
    manager: IPresentationManager,
    /// Signalled when the manager has failed unrecoverably. **Peeked** before each
    /// present rather than waited on, because the manager can be lost without our having
    /// presented at all — so a caller waiting for `Present` to report it can wait
    /// forever.
    lost_event: HANDLE,
    statistics_event: HANDLE,
    statistics: bool,
    /// Latched. A lost group draws and presents nothing until its owner rebuilds it; it
    /// does not error on every call.
    lost: Cell<bool>,
    /// Regions that have bound a freshly drawn buffer since the last present. Zero makes
    /// a present a no-op rather than a re-present of what is already on screen.
    pending: Cell<u32>,
}

/// A presentation manager and the regions presenting through it.
///
/// Thread-affine like the device inside it. [`Clone`] shares the manager — a region holds
/// its group, and cloning is how it does that.
#[derive(Clone)]
pub struct PresentationGroup(Rc<Inner>);

impl PresentationGroup {
    pub(crate) fn new(factory: &IPresentationFactory, statistics: bool) -> Result<Self> {
        // SAFETY: `factory` is live for the call; every out-parameter is a stack local.
        let (manager, lost_event, statistics_event) = unsafe {
            let manager = factory.CreatePresentationManager()?;
            let lost = manager.GetLostEvent()?;
            let stats = manager.GetPresentStatisticsAvailableEvent()?;
            (manager, lost, stats)
        };
        if statistics {
            // All three kinds, or none. They answer different halves of one question and
            // the gap between them is a trap — see `PresentStatistic::Flip`.
            for kind in [
                PresentStatisticsKind_PresentStatus,
                PresentStatisticsKind_CompositionFrame,
                PresentStatisticsKind_IndependentFlipFrame,
            ] {
                // SAFETY: `manager` is live and owned here.
                unsafe { manager.EnablePresentStatisticsKind(kind, 1).ok()? };
            }
        }
        Ok(Self(Rc::new(Inner {
            manager,
            lost_event,
            statistics_event,
            statistics,
            lost: Cell::new(false),
            pending: Cell::new(0),
        })))
    }

    /// Whether the group has failed and must be rebuilt, along with every region in it.
    #[must_use]
    pub fn is_lost(&self) -> bool {
        if self.0.lost.get() {
            return true;
        }
        if peek(self.0.lost_event) {
            self.0.lost.set(true);
            return true;
        }
        false
    }

    /// Shows every region that has bound a buffer since the last call, no earlier than
    /// `at` — a system-interrupt time from [`interrupt_time_now`], or `0` for as early as
    /// the system can.
    ///
    /// A run of presents at increasing times is a *queue*: each occupies its own refresh
    /// and the GPU works through them with the CPU out of it. **A run at `0` is not**,
    /// because `0` supersedes anything still queued and the system does work to resolve
    /// that — about 10% of a present. So a batch always states its slots, and `0` is for
    /// the one-off case where there is nothing queued to supersede.
    ///
    /// `Ok(false)` when nothing was bound or the group is lost.
    pub fn present_at(&self, at: u64, interrupt: Interrupt) -> Result<bool> {
        let inner = &*self.0;
        if inner.pending.replace(0) == 0 {
            return Ok(false);
        }
        if self.is_lost() {
            return Ok(false);
        }
        // A statistic describes a present the CPU was woken for, so a group reporting
        // them forces the interrupt on rather than letting a caller defer the feedback it
        // is about to read. Folded in here, and not a setter, because a mode set before
        // one present and read by the next is exactly the state a caller forgets.
        let raise = inner.statistics || interrupt == Interrupt::Raise;
        // SAFETY: `manager` is live and owned by this group; none of these retains a
        // borrow past its return.
        unsafe {
            // No error path worth surfacing: it cannot fail in a way that changes what
            // the caller does, and a manager that has gone is caught by the present below.
            _ = inner.manager.ForceVSyncInterrupt(u8::from(raise));
            inner
                .manager
                .SetTargetTime(SystemInterruptTime { value: at })
                .ok()?;
            let hr = inner.manager.Present();
            if hr == PRESENTATION_ERROR_LOST {
                inner.lost.set(true);
                return Ok(false);
            }
            hr.ok()?;
        }
        Ok(true)
    }

    /// Whether this group was built to report statistics.
    #[must_use]
    pub fn reports_statistics(&self) -> bool {
        self.0.statistics
    }

    /// Folds every pending record into `tally`, and answers how many it read.
    ///
    /// Call it **ahead of issuing this pass's presents**, as the API's own guidance asks:
    /// the queue holds a few seconds and retires its oldest entries when it fills, so a
    /// producer that presents first and reads later reads a queue that has already
    /// dropped the answer. A group that enables statistics and never drains is making the
    /// compositor generate work for a queue that only ever overflows.
    pub fn drain_statistics(&self, tally: &mut PresentTally) -> u32 {
        if !self.0.statistics {
            return 0;
        }
        let mut read = 0;
        while peek(self.0.statistics_event) {
            // SAFETY: `manager` is live and owned here.
            let Ok(item) = (unsafe { self.0.manager.GetNextPresentStatistics() }) else {
                break;
            };
            read += 1;
            // A record this build does not understand is still a record that was read.
            // Folding "decoded to nothing" back into "the queue is empty" would end the
            // drain at the first one and leave every record behind it unread — which is
            // how a run reports a healthy present count and no composition frames at all.
            if let Some(stat) = decode(&item) {
                tally.record(stat);
            }
        }
        read
    }

    pub(crate) fn manager(&self) -> &IPresentationManager {
        &self.0.manager
    }

    pub(crate) fn lost_event(&self) -> HANDLE {
        self.0.lost_event
    }

    pub(crate) fn note_bound(&self) {
        self.0.pending.set(self.0.pending.get() + 1);
    }
}

/// One statistics record, or `None` when it is a kind this build does not read.
fn decode(item: &IPresentStatistics) -> Option<PresentStatistic> {
    // SAFETY: `item` is live for the rest of this function.
    let (present_id, kind) = unsafe { (item.GetPresentId(), item.GetKind()) };
    // The generated kinds are bare `i32` constants, so these are comparisons rather
    // than match arms — an arm naming one would bind a fresh variable and match
    // everything, which is a bug that compiles and reads correctly.
    if kind == PresentStatisticsKind_PresentStatus {
        let status = item.cast::<IPresentStatusPresentStatistics>().ok()?;
        // SAFETY: `status` is a live interface pointer.
        let status = unsafe { status.GetPresentStatus() };
        let outcome = if status == PresentStatus_Skipped {
            Outcome::Skipped
        } else if status == PresentStatus_Canceled {
            Outcome::Canceled
        } else {
            Outcome::Queued
        };
        return Some(PresentStatistic::Status {
            present_id,
            outcome,
        });
    }
    if kind == PresentStatisticsKind_IndependentFlipFrame {
        return Some(PresentStatistic::Flip { present_id });
    }
    if kind == PresentStatisticsKind_CompositionFrame {
        let frame = item.cast::<ICompositionFramePresentStatistics>().ok()?;
        let mut count = 0u32;
        let mut instances: *const CompositionFrameDisplayInstance = core::ptr::null();
        // Both are written by the callee, and the array parameter is generated as
        // `*const *const` — the outer indirection loses its `mut` because nothing in
        // the metadata marks it as an out-parameter. Taken from a mutable place and
        // then cast, rather than passed as `&instances`, so the pointer the callee
        // writes through carries write provenance.
        // SAFETY: `frame` is live, both destinations are stack locals that outlive the
        // call, and the array `instances` comes to point at is owned by `frame` and
        // valid until it drops — which is after the read below.
        unsafe {
            frame.GetDisplayInstanceArray(&raw mut count, (&raw mut instances).cast_const());
        }
        if count == 0 || instances.is_null() {
            return None;
        }
        // One display is the question worth answering: the first instance is the one
        // whose plane the layout is protecting.
        // SAFETY: `count` is non-zero, so element 0 is inside the array.
        let first = unsafe { *instances };
        let instance = if first.instanceKind == CompositionFrameInstanceKind_ScanoutOnScreen {
            Instance::Scanout
        } else if first.instanceKind == CompositionFrameInstanceKind_ComposedToIntermediate {
            Instance::Intermediate
        } else {
            Instance::Composed
        };
        return Some(PresentStatistic::Frame {
            present_id,
            instance,
            cross_adapter: first.requiredCrossAdapterCopy != 0,
        });
    }
    None
}

/// Whether `handle` is signalled right now, without blocking.
///
/// A zero timeout polls. Used for the two manager events, both of which are the manager's
/// to reset, so this never consumes a signal another reader needed.
fn peek(handle: HANDLE) -> bool {
    // SAFETY: `handle` is owned by the manager and outlives this call; the list is a
    // stack local of the stated length.
    unsafe { WaitForMultipleObjects(1, &handle, false.into(), 0) == WAIT_OBJECT_0 as u32 }
}
