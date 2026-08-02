use crate::bindings::*;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use windows_core::{Error, Result};

/// An auto-reset event: one signal releases exactly one waiter.
///
/// Auto-reset is the only correct mode for a wake source. A manual-reset event left
/// signalled satisfies every subsequent wait immediately, so the waiter spins.
pub struct Event(OwnedHandle);

impl Event {
    /// A new, unsignalled event.
    ///
    /// # Errors
    ///
    /// Resource exhaustion. There is no degraded mode: a waiter that cannot be interrupted
    /// is worse than no waiter.
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
        // SAFETY: a fresh non-null handle, owned here and closed by `OwnedHandle`.
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(handle) }))
    }

    /// Releases one waiter.
    pub fn signal(&self) {
        // SAFETY: the handle is owned by this value.
        unsafe {
            _ = SetEvent(self.raw());
        }
    }

    /// Waits up to `timeout_ms`, or [`INFINITE`]. A signal and an expiry are
    /// indistinguishable; a caller that must tell them apart carries its own state.
    pub fn wait(&self, timeout_ms: u32) {
        // SAFETY: as above.
        unsafe {
            WaitForSingleObject(self.raw(), timeout_ms);
        }
    }

    /// Whether the event is signalled now, consuming the signal if it is.
    #[must_use]
    pub fn take(&self) -> bool {
        // SAFETY: as above. A zero timeout polls.
        unsafe { WaitForSingleObject(self.raw(), 0) == WAIT_OBJECT_0 as u32 }
    }

    fn raw(&self) -> HANDLE {
        self.0.as_raw_handle()
    }
}

impl AsHandle for Event {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.0.as_handle()
    }
}

impl core::fmt::Debug for Event {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Event")
    }
}

/// Blocks until one of `handles` is signalled.
///
/// The count comes off the slice rather than from the caller, so the two cannot disagree —
/// which is the one way to get `WaitForMultipleObjects` wrong that the compiler will not
/// catch.
pub(crate) fn wait_any(handles: &[BorrowedHandle<'_>]) {
    // SAFETY: `BorrowedHandle` is a transparent wrapper over the raw handle, so the slice is
    // the contiguous array the call takes, and the borrows keep every owner alive across it.
    unsafe {
        WaitForMultipleObjects(
            handles.len() as u32,
            handles.as_ptr().cast(),
            false.into(),
            INFINITE,
        );
    }
}
