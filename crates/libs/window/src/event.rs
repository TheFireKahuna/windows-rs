//! A waitable event, and the one rule that makes it usable as a wake source.

use crate::bindings::*;
use std::os::windows::io::{AsHandle, BorrowedHandle};
use windows_core::{Error, Result};

/// An auto-reset event, closed on drop.
///
/// Auto-reset is not a detail. A manual-reset event left signalled satisfies every
/// subsequent wait immediately, so the waiter becomes a busy loop — which is the
/// difference between a thread that parks at idle and one that spends a core reporting
/// that nothing happened. Both users of this type are wake sources
/// ([`Pacer`](crate::Pacer) and the signal scheduler's cross-thread write inbox), and both
/// depend on that.
///
/// The handle is a kernel object reference and signal, wait and close are all documented as
/// safe from any thread, so this is `Send + Sync` with nothing thread-affine in it.
pub struct Event(*mut core::ffi::c_void);

// SAFETY: an event handle is a kernel object reference, and signal, wait and close are all
// documented as safe from any thread. Nothing about it is thread-affine.
unsafe impl Send for Event {}
unsafe impl Sync for Event {}

impl Event {
    /// A new, unsignalled, auto-reset event.
    ///
    /// # Errors
    ///
    /// Resource exhaustion, which has no fallback: without an event a waiter cannot be
    /// interrupted, and an uninterruptible waiter is worse than none.
    pub fn auto_reset() -> Result<Self> {
        // SAFETY: no security attributes and no name; auto-reset, initially unsignalled.
        let handle = unsafe {
            CreateEventW(
                core::ptr::null(),
                false.into(),
                false.into(),
                windows_core::PCWSTR::null(),
            )
        };
        if handle.is_null() {
            return Err(Error::from_thread());
        }
        Ok(Self(handle))
    }

    /// The raw handle, for a wait that takes a handle list.
    ///
    /// Borrowed rather than copied out, so it cannot outlive the event that owns it.
    #[must_use]
    pub fn raw(&self) -> *mut core::ffi::c_void {
        self.0
    }

    /// Signals the event, releasing exactly one waiter.
    pub fn signal(&self) {
        // SAFETY: the handle is owned by this value and is live for its whole lifetime.
        unsafe {
            let _ = SetEvent(self.0);
        }
    }

    /// Waits up to `timeout_ms`, or [`INFINITE`]. Returns on signal or on expiry, and does
    /// not distinguish them: a caller that needs to knows from its own state.
    pub fn wait(&self, timeout_ms: u32) {
        // SAFETY: as above.
        unsafe {
            WaitForSingleObject(self.0, timeout_ms);
        }
    }

    /// Whether the event is signalled right now, consuming the signal if it is.
    #[must_use]
    pub fn take(&self) -> bool {
        // SAFETY: as above. A zero timeout polls rather than blocks.
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 as u32 }
    }
}

/// The borrowed form, for a consumer that puts this in a wait list next to handles it does
/// not own. It carries the borrow the raw pointer cannot, so a handle cannot outlive the
/// event it came from.
impl AsHandle for Event {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        // SAFETY: the handle is owned by this value, live for the whole of the borrow, and
        // closed only by the `Drop` below.
        unsafe { BorrowedHandle::borrow_raw(self.0) }
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: closed exactly once, from the sole owner.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
