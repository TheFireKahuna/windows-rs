//! A payload-free "something changed".
//!
//! Pick by consumer, not by taste:
//!
//! | consumer | use | why |
//! |---|---|---|
//! | a retained sink, an automation value, a derived label | [`Cell`](super::Cell) | it wants the value |
//! | a presentation region's frame | `Epoch` | it wants a *wake*; it reads its own data through its own reader |
//!
//! Handing a region a `Cell` would put the producer's data type into a trait the framework
//! has to name, which is how a viz source becomes a union of every facet anything ever
//! wanted. An `Epoch` says only *there is new work*, and the consumer closes over
//! whatever it reads.

use core::sync::atomic::{AtomicU64, Ordering};
use std::os::windows::io::{AsHandle, BorrowedHandle};
use windows_core::Result;
use windows_window::Event;

/// A counter and a wake, both `Send + Sync`.
///
/// The counter is what a consumer already inside its own loop reads — it can miss any
/// number of bumps and still knows it missed them, which a flag cannot express. The event
/// is what a consumer that is parked waits on, and it is signalled on every bump because
/// an auto-reset event coalesces the wakes itself: a burst leaves one waiter release, not
/// one per bump.
pub struct Epoch {
    count: AtomicU64,
    event: Event,
}

impl Epoch {
    /// A new epoch at zero.
    ///
    /// # Errors
    ///
    /// Resource exhaustion creating the event. There is no degraded mode: an epoch that
    /// cannot wake its consumer is a consumer that polls.
    pub fn new() -> Result<Self> {
        Ok(Self {
            count: AtomicU64::new(0),
            event: Event::auto_reset()?,
        })
    }

    /// Publishes: increments the counter and releases a waiter.
    ///
    /// `Release` ordering, paired with [`count`](Self::count)'s `Acquire`, so whatever the
    /// producer wrote before this call is visible to a consumer that observes the new
    /// count. That pairing is the whole synchronization contract — there is no lock.
    pub fn bump(&self) {
        self.count.fetch_add(1, Ordering::Release);
        self.event.signal();
    }

    /// The current count. `Acquire`: see [`bump`](Self::bump).
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Acquire)
    }

    /// Parks until the next bump, or until `timeout_ms` elapses.
    ///
    /// Returns the count observed on waking, which is what a consumer compares against
    /// what it last handled — the wait itself cannot distinguish a signal from an expiry,
    /// and the count is what makes that distinction unnecessary.
    pub fn wait(&self, timeout_ms: u32) -> u64 {
        self.event.wait(timeout_ms);
        self.count()
    }
}

impl AsHandle for Epoch {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.event.as_handle()
    }
}

impl core::fmt::Debug for Epoch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Epoch")
            .field("count", &self.count())
            .finish_non_exhaustive()
    }
}
