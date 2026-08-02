//! Asking to be serviced **now** rather than at the next composition frame.
//!
//! **Frame-limiting input is the opposite of a low-latency design.** A press, a release, a
//! wheel notch, a keystroke and a dial detent do not batch and are not per-frame quantities,
//! so making any of them wait for the display buys a frame of latency and nothing else. They
//! ask for the tick instead of waiting for it.
//!
//! Motion deliberately does not come here, and neither does anything the frame clock is
//! actually *for*: a hover state nobody can observe between two presents, and a manipulation
//! whose samples are read from history at whatever instant the tick runs.
//!
//! The message posted is the **pacer's own**. That is what keeps this from being a second
//! consumption path: the tick that services the ring is the same code either way, drains in
//! the same order and publishes the same way. Only *when* it runs differs.

use crate::bindings::{HWND, PostMessageW};
use core::cell::Cell;

/// One window's request-for-service gate, shared by every producer that has something
/// latency-critical to hand over.
#[derive(Default)]
pub struct Service {
    /// Absent until the window exists — a doorbell is installed into the builder, so it
    /// necessarily predates the window it serves. Anything arriving before then is recorded
    /// and consumed by the first tick.
    target: Cell<HWND>,
    /// Whether a request is already in flight. The whole of the coalescing: a burst of
    /// contacts lifting together asks once.
    posted: Cell<bool>,
}

impl Service {
    /// A gate with no window yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            target: Cell::new(core::ptr::null_mut()),
            posted: Cell::new(false),
        }
    }

    /// Names the window to ask.
    pub fn attach(&self, hwnd: HWND) {
        self.target.set(hwnd);
    }

    /// Asks for a tick on the next pump iteration.
    pub fn now(&self) {
        if self.posted.replace(true) {
            return;
        }
        let target = self.target.get();
        if target.is_null() {
            return;
        }
        // Reusing the pacer's message also re-opens the pacer's own post gate, so the display
        // may post one extra frame after this. That costs a tick with nothing pending, which
        // does nothing and releases its guard — the alternative, a second message and a
        // second service path, is the thing this design exists to not have.
        //
        // SAFETY: posting is callable from any thread and resolves the handle itself; a
        // window that has gone simply refuses the post, which is why the result is dropped.
        // The gate is re-opened by the tick, so a refusal cannot wedge it shut.
        unsafe {
            _ = PostMessageW(target, windows_window::WM_FRAME, 0, 0);
        }
    }

    /// Re-opens the gate.
    ///
    /// Called by the tick **before** it drains, so a transition arriving during the drain
    /// asks again rather than being swallowed — the same discipline, and the same reason, as
    /// the pacer's own post gate having no `begin_tick` for a caller to forget.
    pub fn begin(&self) {
        self.posted.set(false);
    }
}

impl core::fmt::Debug for Service {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Service")
            .field("attached", &!self.target.get().is_null())
            .field("posted", &self.posted.get())
            .finish()
    }
}
