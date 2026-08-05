//! Whether anything a window draws can be seen, and the wakes that say it changed.

use crate::bindings::*;
use crate::event::Event;
use core::sync::atomic::{AtomicBool, Ordering};
use std::os::windows::io::{AsHandle, BorrowedHandle};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use windows_core::{Interface, Result};

/// Tracks whether the window itself is on screen, and wakes watchers when that moves.
///
/// This is the half the window thread can determine outright: not shown yet, minimized, or
/// cloaked. The system's occlusion status arrives as a bare change notification carrying no
/// direction, so it only wakes; a consumer pairs that wake with the compositor clock's
/// [`Occluded`](crate::clock::Observed::Occluded) return, which carries the direction. No
/// timer is involved.
///
/// A window fully covered by another window is not detected: a flip-model chain never returns
/// `DXGI_STATUS_OCCLUDED`, and `D3DKMTCheckOcclusion` answers "not occluded" whenever desktop
/// composition is running.
///
/// A thread parks on the wake returned by [`watch`](Self::watch).
pub struct Visibility {
    hidden: AtomicBool,
    /// One wake per watcher rather than one shared by all of them. A wake is auto-reset, so a
    /// signal releases *exactly one* waiter: two threads sharing one would mean a visibility
    /// change woke whichever raced to it and left the other parked, with no second edge coming.
    ///
    /// `Weak`, so a watcher that goes away needs no deregistration.
    wakes: Mutex<Vec<Weak<Event>>>,
}

impl Visibility {
    pub(crate) fn new() -> Self {
        Self {
            // Hidden until a window is evaluated: the opposite default would have every
            // consumer draw a frame before the window has been put on screen even once.
            hidden: AtomicBool::new(true),
            wakes: Mutex::new(Vec::new()),
        }
    }

    /// Returns whether the window itself is off screen — not shown yet, minimized, or cloaked.
    ///
    /// Answers only that half. The display may also be off, which only the compositor clock
    /// reports.
    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.hidden.load(Ordering::Acquire)
    }

    /// Registers a watcher and returns it, carrying a wake of its own for a wait list.
    ///
    /// # Errors
    ///
    /// Fails on resource exhaustion, when the wake event cannot be created.
    pub fn watch(self: &Arc<Self>) -> Result<Watch> {
        let wake = Arc::new(Event::auto_reset()?);
        self.locked_wakes().push(Arc::downgrade(&wake));
        Ok(Watch {
            visibility: Arc::clone(self),
            wake,
        })
    }

    /// Re-evaluates the window's own half. Called from the window procedure, so a consumer
    /// cannot be told late by an application that handles its own messages.
    pub(crate) fn evaluate(&self, hwnd: HWND) {
        // `IsWindowVisible` first: a window built hidden is neither iconic nor cloaked, so
        // without it a window created hidden and shown only after its first commit would
        // report itself visible for the whole of that startup.
        self.publish(!is_visible(hwnd) || is_iconic(hwnd) || is_cloaked(hwnd));
    }

    /// Publishes the window's own half, waking every watcher when the value moves.
    pub(crate) fn publish(&self, hidden: bool) {
        // acq_rel: pairs with the acquire in `is_hidden`, so a watcher woken below reads the
        // state that caused the wake rather than the one before it.
        if self.hidden.swap(hidden, Ordering::AcqRel) != hidden {
            self.wake_all();
        }
    }

    /// Wakes every watcher after the system's occlusion status changes. That notification
    /// carries no direction, so this only wakes: a parked watcher re-probes, and one that is
    /// not parked ignores it.
    pub(crate) fn poke(&self) {
        self.wake_all();
    }

    /// Wakes every live watcher, forgetting the ones that have gone.
    ///
    /// Reached only on a real edge — a minimize, a restore, a cloak, an occlusion-status
    /// change — so the lock is off every path that runs per message or per frame.
    fn wake_all(&self) {
        self.locked_wakes().retain(|wake| {
            wake.upgrade().is_some_and(|wake| {
                wake.signal();
                true
            })
        });
    }

    /// Locks the watcher registry. Poisoning is recovered from rather than propagated: the
    /// only work done under this lock is signalling handles, and a poisoned lock would
    /// otherwise take the window procedure down with it.
    fn locked_wakes(&self) -> std::sync::MutexGuard<'_, Vec<Weak<Event>>> {
        self.wakes.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl core::fmt::Debug for Visibility {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Visibility")
            .field("hidden", &self.is_hidden())
            .finish_non_exhaustive()
    }
}

/// Carries one watcher's view of whether a window can be seen: the state, and the wake that
/// says it moved.
///
/// `Send + Sync`, so a producer on another thread parks on it directly. Every watcher holds a
/// wake of its own, which is what lets one window have several — a frame pacer and a present
/// thread both park on the same window, and a change reaches both.
///
/// [`as_handle`](AsHandle::as_handle) returns that wake, for a wait list.
pub struct Watch {
    visibility: Arc<Visibility>,
    wake: Arc<Event>,
}

impl Watch {
    /// Returns whether the window itself is off screen, as [`Visibility::is_hidden`] does.
    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.visibility.is_hidden()
    }
}

impl AsHandle for Watch {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.wake.as_handle()
    }
}

impl core::fmt::Debug for Watch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Watch")
            .field("hidden", &self.is_hidden())
            .finish_non_exhaustive()
    }
}

fn is_visible(hwnd: HWND) -> bool {
    // SAFETY: `hwnd` is the window this belongs to, live for the call.
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

fn is_iconic(hwnd: HWND) -> bool {
    // SAFETY: as above.
    unsafe { IsIconic(hwnd).as_bool() }
}

/// Returns whether DWM is hiding the window while still composing it — a virtual-desktop
/// switch or a shell cloak, which nothing else reports.
fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: `hwnd` is live; the destination is a stack local of the stated size.
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        )
    };
    hr.is_ok() && cloaked != 0
}

/// Holds the system's occlusion-status registration, unregistering it on drop.
///
/// The message form rather than the event form: the window thread has a pump, where an event
/// would need a wait slot on a thread that does not wait.
pub(crate) struct OcclusionStatus {
    factory: IDXGIFactory2,
    cookie: u32,
}

impl OcclusionStatus {
    /// Registers `hwnd` for occlusion-status changes, posting `message` to it on each.
    ///
    /// `None` when the platform declines. What is lost is the edge that says to look again;
    /// the window's own state and the compositor clock both still answer.
    pub(crate) fn register(hwnd: HWND, message: u32) -> Option<Self> {
        // SAFETY: the out-parameter is a stack local; ownership transfers on success.
        let factory: IDXGIFactory2 = unsafe {
            let mut out = core::ptr::null_mut();
            CreateDXGIFactory2(0, &IDXGIFactory2::IID, &mut out)
                .ok()
                .ok()?;
            IDXGIFactory2::from_raw(out)
        };
        // SAFETY: `factory` is live and owned here; `hwnd` belongs to this process.
        let cookie = unsafe { factory.RegisterOcclusionStatusWindow(hwnd, message).ok()? };
        Some(Self { factory, cookie })
    }
}

impl Drop for OcclusionStatus {
    fn drop(&mut self) {
        // SAFETY: the cookie came from the matching registration on this factory.
        unsafe { self.factory.UnregisterOcclusionStatus(self.cookie) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One change reaches every watcher: a pacer and a present thread park on the same window,
    /// and a single shared auto-reset wake would release only one of them.
    #[test]
    fn every_watcher_sees_a_change() {
        let visibility = Arc::new(Visibility::new());
        let first = visibility.watch().expect("an event is available");
        let second = visibility.watch().expect("an event is available");
        visibility.publish(false);
        assert!(
            first.wake.take(),
            "the first watcher slept through the change"
        );
        assert!(second.wake.take(), "the second watcher slept through it");
    }

    /// Auto-reset, so a watcher that has already observed a change does not wake again on it.
    #[test]
    fn a_wake_is_consumed_once() {
        let visibility = Arc::new(Visibility::new());
        let watch = visibility.watch().expect("an event is available");
        visibility.publish(false);
        assert!(watch.wake.take());
        assert!(!watch.wake.take(), "one change woke the watcher twice");
    }

    #[test]
    fn only_a_real_change_wakes() {
        let visibility = Arc::new(Visibility::new());
        let watch = visibility.watch().expect("an event is available");
        visibility.publish(true);
        assert!(
            !watch.wake.take(),
            "a repeat of the current state woke a watcher"
        );
        // The occlusion status carries no direction, so it wakes unconditionally.
        visibility.poke();
        assert!(watch.wake.take());
    }

    #[test]
    fn a_dropped_watcher_leaves_no_entry() {
        let visibility = Arc::new(Visibility::new());
        drop(visibility.watch().expect("an event is available"));
        let live = visibility.watch().expect("an event is available");
        visibility.publish(false);
        assert!(live.wake.take());
        assert_eq!(
            visibility.locked_wakes().len(),
            1,
            "the dropped watcher is still registered"
        );
    }
}
