//! The compositor's own frame clock: one blocking wait, decoded once into [`Observed`].
//!
//! The wait's return is neither a `WAIT_*` value nor a boolean, and the clock's own slot is
//! the one *after* the handles the caller passes in. A caller reading it as a plain wait
//! result takes a signal, an occluded display or a failed wait for a frame.

use crate::bindings::*;
use std::os::windows::io::{BorrowedHandle, RawHandle};

/// Passed as the timeout for no guard: the wait returns only when the clock or one of the
/// caller's own handles answers.
pub const INFINITE: u32 = crate::bindings::INFINITE;

/// Returned when the display cannot show anything. Not a `WAIT_*` value and not an error: the
/// call succeeds and returns **immediately**, so treating it as a frame is a busy loop that
/// appears only when a monitor sleeps.
const STATUS_GRAPHICS_PRESENT_OCCLUDED: u32 = 0xC01E_0006;

/// Reports what one wait on the compositor clock observed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Observed {
    /// A composition frame elapsed.
    Frame,
    /// The caller's own handle at this index fired, and **no frame elapsed**. Doing a frame's
    /// work here would spend one on every state change.
    Signal(u32),
    /// The guard expired with no frame: a stalled clock — a locked session, a sleeping
    /// display, a mode switch. A caller that treats it as a frame keeps motion going; every
    /// timing figure taken meanwhile measures the session rather than the display.
    Stalled,
    /// The display cannot show anything, and the call returned without blocking. The caller
    /// parks: only the clock reports the display coming back.
    Occluded,
    /// The wait failed, which is what a session with no compositor clock — headless, or
    /// remote — answers. Nothing distinguishes that from a transient failure, so a caller
    /// parks and probes again on its next edge rather than retrying in place, which on the
    /// permanent case is a spin.
    NoClock,
}

/// Blocks until a composition frame elapses, one of `handles` is signalled, or `timeout_ms`
/// expires. Pass [`INFINITE`] for no guard.
///
/// The block is required rather than incidental: the graphics system cannot tell whether
/// anyone is waiting on an event, so a real waiter is what keeps the vertical-blank interrupt
/// on, and the absence of one is what switches it off.
#[must_use]
pub fn wait_for_frame(handles: &[BorrowedHandle<'_>], timeout_ms: u32) -> Observed {
    // SAFETY: `BorrowedHandle` is a transparent wrapper over the raw handle, so the slice is
    // the contiguous array the call takes, and the borrows keep every owner alive across it.
    unsafe { wait_for_frame_raw(handles.as_ptr().cast(), handles.len() as u32, timeout_ms) }
}

/// As [`wait_for_frame`], for a caller whose handle list cannot name its owners' lifetime —
/// one rebuilt as a field when its membership changes, rather than per wait.
///
/// # Safety
///
/// `handles` must address `count` handles, each naming a kernel object that stays live for the
/// call.
#[must_use]
pub unsafe fn wait_for_frame_raw(
    handles: *const RawHandle,
    count: u32,
    timeout_ms: u32,
) -> Observed {
    // SAFETY: the caller guarantees the list.
    let result = unsafe { DCompositionWaitForCompositorClock(count, handles.cast(), timeout_ms) };
    // The clock's slot is the one after the caller's handles, not zero, so a result read as
    // nonzero-means-frame takes a caller's own signal for a frame.
    let clock = WAIT_OBJECT_0 as u32 + count;
    match result {
        STATUS_GRAPHICS_PRESENT_OCCLUDED => Observed::Occluded,
        WAIT_FAILED => Observed::NoClock,
        r if r == clock => Observed::Frame,
        r if r < clock => Observed::Signal(r - WAIT_OBJECT_0 as u32),
        // The guard timeout, and an abandoned wait: neither is a frame and both mean the
        // clock stopped answering.
        _ => Observed::Stalled,
    }
}
