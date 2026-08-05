//! A value handed from the window procedure to the frame that services it, carrying the frame
//! request that makes that frame happen.
//!
//! The pacer is parked unless something has asked for a frame, so a value written with no
//! [`Tick`] behind it is one nothing comes back for: a resize the window never redraws, a
//! caption hover that lights on the next unrelated frame or not at all.

use crate::{Tick, Wake};
use std::cell::Cell;

/// Holds a value posted from the window procedure, with the frame request that makes it
/// arrive.
///
/// The wake is armed only once the window exists, because a pacer borrows one. A message
/// arriving before then records its value **without** a request; the only window that reaches
/// that state is one changed before `show`, and the first tick reads the value anyway.
pub struct Handoff<T: Copy> {
    value: Cell<Option<T>>,
    tick: Cell<Option<Tick>>,
    wake: Cell<Option<Wake>>,
}

impl<T: Copy> Default for Handoff<T> {
    fn default() -> Self {
        Self {
            value: Cell::new(None),
            tick: Cell::new(None),
            wake: Cell::new(None),
        }
    }
}

impl<T: Copy> Handoff<T> {
    /// Creates an unarmed handoff, which records values and requests no frame.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gives the handoff the frame clock. Until then, [`post`](Self::post) records its value
    /// and requests nothing.
    pub fn arm(&self, wake: Wake) {
        self.wake.set(Some(wake));
    }

    /// Records `value` and requests the frame that will read it.
    ///
    /// The fresh request is taken before the held one is dropped, so the pacer's requester
    /// count never falls to zero across a drag, where posts arrive faster than they are
    /// serviced. Runs per window message and allocates nothing.
    pub fn post(&self, value: T) {
        self.value.set(Some(value));
        let wake = self.wake.take();
        self.tick.set(wake.as_ref().map(Wake::tick));
        self.wake.set(wake);
    }

    /// Takes the value and releases the request that carried it.
    ///
    /// Both in one call: releasing separately allows taking the value while leaving the clock
    /// running, or releasing while leaving the value for a frame nobody asked for.
    pub fn take(&self) -> Option<T> {
        self.tick.take();
        self.value.take()
    }
}
