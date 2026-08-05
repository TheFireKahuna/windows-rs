//! A payload-free "something changed": a counter and a wake, for a consumer that reads its
//! own data.
//!
//! The choice follows from what the consumer needs:
//!
//! | consumer | use | why |
//! |---|---|---|
//! | a retained sink, an automation value, a derived label | [`Cell`](super::Cell) | it wants the value |
//! | a presentation region's frame | [`Epoch`] | it wants a *wake*; it reads its own data through its own reader |
//!
//! An `Epoch` carries no payload, so the producer's data type stays out of the traits the
//! framework names and the consumer closes over whatever it reads.

use core::sync::atomic::{AtomicU64, Ordering};
use std::os::windows::io::{AsHandle, BorrowedHandle};
use windows_core::Result;
use windows_window::Event;

/// A monotonic counter and a wake event, both `Send + Sync`.
///
/// A consumer already inside its own loop reads the counter: it may miss any number of
/// bumps and still sees that it missed them, which a flag cannot express. A parked consumer
/// waits on the event, which is signalled on every bump; the event is auto-reset, so a
/// burst releases one waiter rather than one per bump.
pub struct Epoch {
    count: AtomicU64,
    event: Event,
}

impl Epoch {
    /// Creates an epoch with its count at zero.
    ///
    /// # Errors
    ///
    /// Fails where the wake event cannot be created, which is resource exhaustion. There is
    /// no degraded mode: an epoch that cannot wake its consumer leaves it polling.
    pub fn new() -> Result<Self> {
        Ok(Self {
            count: AtomicU64::new(0),
            event: Event::auto_reset()?,
        })
    }

    /// Increments the counter and releases a waiter.
    ///
    /// `Release`: pairs with the `Acquire` in [`count`](Self::count), publishing whatever
    /// the producer wrote before this call to a consumer that observes the new count. That
    /// pairing is the only synchronization between the two; there is no lock.
    pub fn bump(&self) {
        self.count.fetch_add(1, Ordering::Release);
        self.event.signal();
    }

    /// Returns the current count.
    ///
    /// `Acquire`: pairs with the `Release` in [`bump`](Self::bump), so a consumer observing
    /// a new count also observes the writes the producer made before that bump.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Acquire)
    }

    /// Parks until the next bump or until `timeout_ms` elapses, and returns the count
    /// observed on waking.
    ///
    /// The wait does not distinguish a signal from an expiry; a consumer compares the
    /// returned count against the one it last handled.
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
