//! The window handle as a token another thread may hold.

use crate::Window;
use crate::bindings::*;

/// Names a window to a thread that does not own it.
///
/// A window handle is an identifier the system resolves on every call, not a pointer into
/// this process, so it crosses threads as a value. What it cannot promise is that the window
/// is still the same one: handle values are recycled after a window is destroyed. A holder
/// therefore either ends before the [`Window`] does — the pacer joins its thread while it
/// still borrows the window — or accepts that a call may fail or reach nothing, which every
/// method here does. Nothing is ever read back through it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Hwnd(*mut core::ffi::c_void);

// SAFETY: the value is an opaque handle the system resolves per call and this type never
// dereferences it; every method is a thread-safe user32 call that fails harmlessly on a
// destroyed window.
unsafe impl Send for Hwnd {}
// SAFETY: as above; a shared reference exposes only those same calls.
unsafe impl Sync for Hwnd {}

impl Hwnd {
    pub(crate) const fn new(raw: *mut core::ffi::c_void) -> Self {
        Self(raw)
    }

    /// Returns the raw handle, for a call this crate does not wrap.
    ///
    /// The lifetime contract above travels with the value: the caller must not use it after
    /// the window it names has closed, other than through calls that tolerate a stale handle.
    #[must_use]
    pub const fn raw(self) -> *mut core::ffi::c_void {
        self.0
    }

    /// Posts `message` to the window's queue from any thread. `false` if the queue is full or
    /// the window is gone; nothing is read back either way.
    pub fn post(self, message: u32, wparam: usize, lparam: isize) -> bool {
        // SAFETY: `PostMessageW` is callable from any thread and takes no pointer; a handle
        // that no longer names a window makes it fail rather than fault.
        unsafe { PostMessageW(self.0, message, wparam, lparam).as_bool() }
    }

    /// Asks the window to close, from any thread. The window's own thread answers `WM_CLOSE`
    /// exactly as it would a click on the close button.
    pub fn close(self) -> bool {
        self.post(WM_CLOSE as u32, 0, 0)
    }
}

impl Window {
    /// Returns this window's handle as a token another thread may hold.
    #[must_use]
    pub fn handle(&self) -> Hwnd {
        Hwnd::new(self.hwnd())
    }
}

const _: () = {
    const fn assert<T: Send + Sync + Copy>() {}
    assert::<Hwnd>();
};
