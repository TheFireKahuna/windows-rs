//! The wake source the window's threads park on: a Windows auto-reset event, and the
//! multi-handle wait that takes several of them.

use crate::bindings::*;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use windows_core::{Error, Result};

/// Releases exactly one waiter per signal: a Windows auto-reset event.
///
/// A manual-reset event left signalled satisfies every subsequent wait immediately, so a
/// waiter on one spins; a wake source is auto-reset for that reason.
pub struct Event(OwnedHandle);

impl Event {
    /// Creates an unsignalled event.
    ///
    /// # Errors
    ///
    /// Fails on resource exhaustion, when the kernel object cannot be created.
    pub fn auto_reset() -> Result<Self> {
        // SAFETY: the call takes flags by value and two null pointers — no security
        // attributes and no name — so nothing has to stay live across it.
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
        // SAFETY: `handle` is non-null by the check above and was created by this call, so
        // no other owner exists and `OwnedHandle` is its sole closer.
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(handle) }))
    }

    /// Releases one waiter.
    pub fn signal(&self) {
        // SAFETY: the handle is owned by this value.
        unsafe {
            _ = SetEvent(self.raw());
        }
    }

    /// Blocks until the event is signalled or `timeout_ms` elapses; pass
    /// [`INFINITE`](crate::clock::INFINITE) for no expiry. A signal and an expiry are
    /// indistinguishable on return, so a caller that must tell them apart carries its own
    /// state.
    pub fn wait(&self, timeout_ms: u32) {
        // SAFETY: as above.
        unsafe {
            WaitForSingleObject(self.raw(), timeout_ms);
        }
    }

    /// Returns whether the event is signalled now, consuming the signal if it is.
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
/// The count comes off the slice rather than from the caller, so the pointer and the length
/// passed to `WaitForMultipleObjects` cannot disagree.
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
