//! Asks for a tick **now** rather than at the next composition frame.
//!
//! A press, a release, a wheel notch, a keystroke and a dial detent do not batch and are not
//! per-frame quantities, so making any of them wait for the display buys a frame of latency
//! and nothing else. They ask for the tick instead of waiting for it.
//!
//! Motion does not come here, and neither does anything the frame clock is *for*: a hover
//! state nobody can observe between two presents, and a manipulation whose samples are read
//! from history at whatever instant the tick runs.
//!
//! The message posted is the **pacer's own**, so this is not a second consumption path: the
//! tick that services the ring is the same code either way, drains in the same order and
//! publishes the same way. Only *when* it runs differs.

use crate::bindings::{HWND, PostMessageW};
use core::cell::Cell;

/// Gates one window's requests for an immediate tick, shared by every producer with
/// something latency-critical to hand over.
#[derive(Default)]
pub struct Service {
    /// The window requests are posted to, null until [`Service::attach`] names it. A doorbell
    /// is installed into the window builder, so it predates the window it serves; anything
    /// arriving before the attach is recorded and consumed by the first tick.
    target: Cell<HWND>,
    /// Whether a request is already in flight. This is the whole of the coalescing: a burst
    /// of contacts lifting together asks once.
    posted: Cell<bool>,
}

impl Service {
    /// Creates a gate with no window attached.
    #[must_use]
    pub fn new() -> Self {
        Self {
            target: Cell::new(core::ptr::null_mut()),
            posted: Cell::new(false),
        }
    }

    /// Names the window that requests are posted to.
    pub fn attach(&self, hwnd: HWND) {
        self.target.set(hwnd);
    }

    /// Asks for a tick on the next pump iteration. Does nothing when a request is already in
    /// flight or no window is attached.
    pub fn now(&self) {
        if self.posted.replace(true) {
            return;
        }
        let target = self.target.get();
        if target.is_null() {
            return;
        }
        // Reusing the pacer's message also re-opens the pacer's own post gate, so the display
        // may post one extra frame after this. That tick finds nothing pending, does nothing
        // and releases its guard.
        //
        // SAFETY: posting is callable from any thread and resolves the handle itself; a
        // window that has gone refuses the post, which is why the result is dropped. The tick
        // re-opens the gate, so a refusal cannot wedge it shut.
        unsafe {
            _ = PostMessageW(target, windows_window::WM_FRAME, 0, 0);
        }
    }

    /// Re-opens the gate.
    ///
    /// The tick calls this **before** it drains, so a transition arriving during the drain
    /// asks again rather than being swallowed.
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
