//! The presentation manager, and what one present costs.
//!
//! Presents are queued per manager, so a group is the unit of "one present shows everything
//! that moved, and the regions that did not move cost nothing". Regions own their buffers,
//! because a buffer is allocated at one size; the manager is what they share.
//!
//! A present that rebinds more than one surface cannot be an independent flip, and that is a
//! property of the queue rather than of the surfaces. Measured with two opaque regions each
//! reaching independent flip on a queue of its own, folding them onto one queue saves this
//! process 1.8% of a core and costs `dwm.exe` ten, because sharing trades independent flip
//! for overlay scanout and DWM then runs a composition frame per present to program the
//! plane. Statistics read `composed 0` in both cases, so `composed 0` does not report a
//! flip; the tell is DWM's frame rate.
//!
//! A queue is therefore shared only for regions that are composed anyway. Regions that share
//! a clock merge into one region with parts, which keeps the plane, rather than onto one
//! queue, which does not.

use super::*;
use core::cell::Cell;

/// Names the present queue a region asks for.
///
/// A region names a queue rather than a group object, so the present thread owns the group
/// set and a consumer states only its timing requirement.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Queue {
    /// A queue of this region's own, guaranteed. For the region whose plane the layout
    /// protects, and which must never have another region's timing in front of it.
    Solo,
    /// The named queue, shared with every region that asks for it.
    ///
    /// One present shows every region that drew and the ones that did not cost nothing — at
    /// the price of the plane, for every region in the queue, whenever it has more than one
    /// member. With a single member it is [`Solo`](Self::Solo) in all but name.
    ///
    /// An extra member on a shared queue costs that queue's plane outright, while an extra
    /// `Solo` queue that gets no plane costs one present, about 100 µs. The system chooses
    /// which surfaces get planes, so a further `Solo` candidate competes with the region
    /// whose plane the layout protects; one shared queue confines the loss to its own
    /// members.
    Shared(&'static str),
}

/// Selects whether a present wakes the CPU when the display shows it.
///
/// The interrupt is how the CPU learns a present was shown: it runs kernel code that updates
/// the buffer-available events, the retiring fence and the statistics queue. Nothing reads
/// any of that for the earlier frames of a batch before the next wake, so they can be
/// reported lazily — about 26 µs of CPU per interrupt, some 14% of a present — which lets
/// the GPU run the flip queue with the CPU out of it.
///
/// Correctness does not depend on this. Deferred feedback makes a wait take longer rather
/// than return the wrong answer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Interrupt {
    /// Wake the CPU. For the one present of a batch that the next pass waits behind.
    Raise,
    /// Report lazily. For every present the producer will not look at before it sleeps.
    Defer,
}

/// Reports how one present reached one display.
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

/// Reports what happened to one issued present.
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

/// Carries one record dequeued from the manager's statistics queue.
#[derive(Copy, Clone, Debug)]
pub enum PresentStatistic {
    /// Whether this producer is being skipped, for the present `present_id`.
    Status { present_id: u64, outcome: Outcome },
    /// Whether DWM composed the buffer or the display scanned it out.
    Frame {
        /// The present this record describes.
        present_id: u64,
        /// How the buffer reached the display.
        instance: Instance,
        /// Whether the buffer had to be copied to another adapter to be displayed — a
        /// reason to reallocate on the display's adapter.
        cross_adapter: bool,
    },
    /// The present reached a display plane, bypassing the compositor.
    ///
    /// The only report such a present makes: composition frames are not issued for flipped
    /// presents, so a caller reading only those sees silence and cannot tell success from a
    /// surface displaying nothing.
    Flip { present_id: u64 },
}

/// Counts what a group's presents did, folded from the records the statistics queue yields.
///
/// `flipped` and `composed` come from two different record kinds and never both describe one
/// present.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PresentTally {
    /// Presents the queue accepted for display.
    pub queued: u32,
    /// Presents superseded by a later one before they could be shown.
    pub skipped: u32,
    /// Presents cancelled.
    pub canceled: u32,
    /// Presents that reached a display plane, from independent-flip records.
    pub flipped: u32,
    /// Presents DWM composed, into its back buffer or into an intermediate.
    pub composed: u32,
    /// Presents the display controller scanned out, from composition-frame records.
    pub scanout: u32,
}

impl PresentTally {
    /// Folds one record into the running counts.
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
    /// Signalled when the manager has failed unrecoverably. Peeked before each present
    /// rather than waited on: the manager can be lost with no present outstanding, so a
    /// caller waiting for `Present` to report the loss can wait forever.
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

/// Owns a presentation manager and issues the presents for the regions on it.
///
/// Thread-affine, like the device it was created from. [`Clone`] shares the manager: a
/// region holds a clone of its group.
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
            // All three kinds, or none: a flipped present issues no composition frame and a
            // composed one issues no flip record, so a subset cannot separate a present
            // that reached a plane from one that displayed nothing.
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

    /// Reports whether the group has failed and must be rebuilt, along with every region in
    /// it.
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
    /// A run of presents at increasing times forms a queue: each occupies its own refresh
    /// and the GPU works through them with the CPU out of it. A run at `0` does not, because
    /// `0` supersedes anything still queued and the system does about 10% of a present's
    /// work to resolve that. A batch therefore states its slots, and `0` is for the one-off
    /// case with nothing queued to supersede.
    ///
    /// Returns `Ok(false)` when nothing was bound or the group is lost.
    ///
    /// # Errors
    ///
    /// Fails when the manager rejects the target time or the present itself. A manager that
    /// reports loss here is latched instead and returns `Ok(false)`.
    pub fn present_at(&self, at: u64, interrupt: Interrupt) -> Result<bool> {
        let inner = &*self.0;
        if inner.pending.replace(0) == 0 {
            return Ok(false);
        }
        if self.is_lost() {
            return Ok(false);
        }
        // A statistic describes a present the CPU was woken for, so a group that reports
        // them forces the interrupt on rather than letting a caller defer the feedback it is
        // about to read. Decided per present rather than through a setter, so the mode
        // cannot be left set from an earlier one.
        let raise = inner.statistics || interrupt == Interrupt::Raise;
        // SAFETY: `manager` is live and owned by this group; none of these retains a
        // borrow past its return.
        unsafe {
            // Discarded: no failure here changes what the caller does, and a manager that
            // has gone is caught by the present below.
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

    /// Reports whether this group was built to report statistics.
    #[must_use]
    pub fn reports_statistics(&self) -> bool {
        self.0.statistics
    }

    /// Folds every pending record into `tally` and returns how many it read.
    ///
    /// Call this ahead of issuing the pass's presents, as the API's guidance asks: the queue
    /// holds a few seconds and retires its oldest entries when it fills, so a producer that
    /// presents first and reads later reads a queue that has already dropped the answer. A
    /// group that enables statistics and never drains makes the compositor fill a queue that
    /// only overflows.
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
            // A record this crate does not decode still counts as read. Treating it as an
            // empty queue would end the drain at the first one and leave every record behind
            // it unread, which reports a healthy present count and no composition frames.
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

/// Decodes one statistics record, or returns `None` for a kind this crate does not read.
fn decode(item: &IPresentStatistics) -> Option<PresentStatistic> {
    // SAFETY: `item` is live for the rest of this function.
    let (present_id, kind) = unsafe { (item.GetPresentId(), item.GetKind()) };
    // The generated kinds are bare `i32` constants, so these are comparisons rather than
    // match arms: an arm naming one would bind a fresh variable and match everything.
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
        let mut instances: *mut CompositionFrameDisplayInstance = core::ptr::null_mut();
        // SAFETY: `frame` is live, both destinations are stack locals that outlive the
        // call, and the array `instances` comes to point at is owned by `frame` and
        // valid until it drops — which is after the read below.
        unsafe {
            frame.GetDisplayInstanceArray(&raw mut count, &raw mut instances);
        }
        if count == 0 || instances.is_null() {
            return None;
        }
        // Only the first instance is decoded: it is the display whose plane the layout is
        // protecting.
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

/// Reports whether `handle` is signalled right now, without blocking.
///
/// A zero timeout polls. Called only for the two manager events, both of which the manager
/// resets, so this never consumes a signal another reader needed.
fn peek(handle: HANDLE) -> bool {
    // SAFETY: `handle` is owned by the manager and outlives this call; the list is a
    // stack local of the stated length.
    unsafe { WaitForMultipleObjects(1, &handle, false.into(), 0) == WAIT_OBJECT_0 as u32 }
}
