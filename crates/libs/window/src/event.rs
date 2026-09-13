//! The wake source the window's threads park on: a Windows auto-reset event, the
//! multi-handle wait that takes several of them, and the wait a message-pumping thread makes
//! so its queue is served alongside them.

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

/// What [`pump_until`] returned for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Pumped {
    /// The handle at this index into the caller's slice was signalled. No message was pumped.
    Signalled(u32),
    /// Messages arrived and were dispatched. Nothing the caller passed fired.
    Messages,
    /// A quit message was dispatched. The caller's loop ends.
    Quit,
    /// The wait itself failed: an invalid handle in the slice. The caller's loop ends rather
    /// than spinning on a wait that will fail again.
    Failed,
}

/// Blocks until one of `handles` is signalled or this thread has messages, and dispatches
/// the messages when it does.
///
/// What a thread that owns a compositor but no window blocks in: a dispatcher queue publishes
/// at the end of a work item and delivers compositor callbacks through the thread's own
/// message queue, so that queue has to be served whenever it is non-empty, and the thread's
/// own wake sources have to interrupt the same wait rather than be seen a message later.
///
/// `MWMO_INPUTAVAILABLE` makes a message that arrived before this call count, so a message
/// posted between two calls is not left waiting for the next unrelated wake.
pub fn pump_until(handles: &[BorrowedHandle<'_>]) -> Pumped {
    let count = handles.len() as u32;
    // SAFETY: `BorrowedHandle` is a transparent wrapper over the raw handle, so the slice is
    // the contiguous array the call takes, and the borrows keep every owner alive across it.
    let woke = unsafe {
        MsgWaitForMultipleObjectsEx(
            count,
            handles.as_ptr().cast(),
            INFINITE,
            QS_ALLINPUT as u32,
            MWMO_INPUTAVAILABLE as u32,
        )
    };
    // The message slot is the one after the caller's handles, as the clock's is for the
    // compositor clock wait.
    let messages = WAIT_OBJECT_0 as u32 + count;
    match woke {
        w if w == messages => {
            if crate::pump() {
                Pumped::Messages
            } else {
                Pumped::Quit
            }
        }
        w if w < messages => Pumped::Signalled(w - WAIT_OBJECT_0 as u32),
        _ => Pumped::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signalled_handle_is_reported_by_index_without_pumping() {
        let first = Event::auto_reset().expect("an event is available");
        let second = Event::auto_reset().expect("an event is available");
        second.signal();
        assert_eq!(
            pump_until(&[first.as_handle(), second.as_handle()]),
            Pumped::Signalled(1)
        );
        assert!(!second.take(), "the wait did not consume the auto-reset signal");
    }

    #[test]
    fn a_quit_message_ends_the_loop() {
        let event = Event::auto_reset().expect("an event is available");
        // SAFETY: takes no pointer; posts to this thread's own queue.
        unsafe { PostQuitMessage(0) };
        assert_eq!(pump_until(&[event.as_handle()]), Pumped::Quit);
        // `pump` reposts the quit it removed, so a second wait answers the same. Consume it
        // so no later test on this thread inherits it.
        assert_eq!(pump_until(&[event.as_handle()]), Pumped::Quit);
        let mut message = MSG::default();
        // SAFETY: the destination is a stack local.
        _ = unsafe { PeekMessageW(&mut message, core::ptr::null_mut(), 0, 0, PM_REMOVE as u32) };
    }
}
