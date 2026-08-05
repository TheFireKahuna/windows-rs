//! A value handed from the window procedure to the tick that services it.
//!
//! The obvious shape — a `Cell` the procedure writes and the loop reads — does not work, and
//! the reason is the pacer's contract rather than anything about the value. The pacer is
//! parked unless something has asked for a frame, so a write with no [`Tick`] behind it is a
//! value nothing will ever come back for: a resize the window never redraws, a caption hover
//! that lights on the next unrelated frame or not at all.
//!
//! So the request is not an optional extra here. It is what makes the handoff a handoff.

use crate::{Tick, Wake};
use std::cell::Cell;

/// A value posted from the window procedure, with the frame request that makes it arrive.
///
/// The wake is armed after the window exists, because a pacer borrows one. A message arriving
/// before then is recorded **without** a request, which is correct rather than a hole: the
/// only window that can hit it is one changed before `show`, and the first tick reads the
/// value anyway.
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
    /// An unarmed handoff.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gives it the frame clock. Until this, a post records its value and asks for nothing.
    pub fn arm(&self, wake: Wake) {
        self.wake.set(Some(wake));
    }

    /// Records `value` and asks for the frame that will read it.
    ///
    /// Replacing a held request with a fresh one rather than keeping the first is what keeps
    /// the clock running across a drag, where these arrive faster than they are serviced.
    pub fn post(&self, value: T) {
        self.value.set(Some(value));
        let wake = self.wake.take();
        self.tick.set(wake.as_ref().map(Wake::tick));
        self.wake.set(wake);
    }

    /// The value, and the release of the request that carried it.
    ///
    /// Both at once, deliberately: a taker that released the request separately could take
    /// the value and leave the clock running, or release and leave the value for a frame
    /// nobody asked for.
    pub fn take(&self) -> Option<T> {
        self.tick.take();
        self.value.take()
    }
}
