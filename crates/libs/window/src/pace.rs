use crate::Window;
use crate::bindings::*;
use crate::clock::{self, Observed};
use crate::event::{Event, wait_any};
use crate::visibility::Watch;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::os::windows::io::AsHandle;
use std::sync::Arc;
use windows_core::{Error, Result};

/// The message the pacer posts, once per composition frame, while something wants one.
///
/// `WM_USER` and not `WM_APP`: this crate registers the window class, and `WM_APP` upwards
/// belongs to the application.
pub const WM_FRAME: u32 = WM_USER as u32 + 0x44;

/// How long a clock wait may block before it is treated as a frame.
///
/// Only ever bounds a *stalled* clock — a locked session, a sleeping display, a mode
/// switch. Six frames at 60 Hz: wide enough that a healthy display never reaches it, short
/// enough that a surface driving itself from the tick keeps moving rather than hanging.
const CLOCK_GUARD_MS: u32 = 100;

/// A handle to the frame clock, for anything that can ask for a tick.
///
/// `Clone + Send + Sync`, so a pointer contact, a queued patch, a scene's outstanding exit
/// animations and a thread that is not the window's all take the same guard type.
#[derive(Clone)]
pub struct Wake(Arc<Clock>);

const _: () = {
    const fn assert<T: Send + Sync + Clone>() {}
    assert::<Wake>();
};

/// One live request for the frame clock. Dropping it releases the request.
pub struct Tick(Arc<Clock>);

pub(crate) struct Clock {
    /// Live requesters. Only the 0→1 and 1→0 edges touch the kernel.
    count: AtomicUsize,
    /// Rides in the clock wait's handle list, so a state change is noticed *inside* the
    /// wait rather than one frame later.
    event: Event,
    /// Whether a frame message is already in flight. The whole of the coalescing: a pump
    /// slower than the display would otherwise accumulate stale frame messages.
    posted: AtomicBool,
    /// The window's own half of "can anything drawn be seen". A veto rather than a request,
    /// which is why it is not expressible as a `Tick`.
    watch: Watch,
    stopping: AtomicBool,
    stalls: AtomicU32,
    clockless: AtomicBool,
    /// Composition frames this pacer has posted for. Incremented **by the pacer thread**, so
    /// it counts elapsed display frames and not window messages — a consumer that posts
    /// [`WM_FRAME`] itself to be serviced sooner does not move it.
    frames: AtomicU64,
}

impl Wake {
    /// Requests a tick until the returned guard is dropped.
    #[must_use]
    pub fn tick(&self) -> Tick {
        self.0.acquire();
        Tick(Arc::clone(&self.0))
    }

    /// How many requesters are live. Zero means the pacer is parked and the pump is blocked.
    #[must_use]
    pub fn requesters(&self) -> usize {
        self.0.count.load(Ordering::Acquire)
    }

    /// How many composition frames have elapsed.
    ///
    /// **Diagnostic.** [`WM_FRAME`] means "service what is pending", and a consumer with
    /// something latency-critical may post it itself rather than wait for the display — so
    /// services outnumber frames, and the gap between this and a consumer's own service count
    /// is the measure of how much never waited. A consumer that has to *gate* on it is
    /// usually about to sample something it should be folding instead.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.0.frames.load(Ordering::Relaxed)
    }
}

impl Clock {
    fn acquire(&self) {
        // Only the 0→1 edge signals, so the steady state during a drag is several live
        // guards and no kernel call at all.
        if self.count.fetch_add(1, Ordering::AcqRel) == 0 {
            self.event.signal();
        }
    }

    fn release(&self) {
        // The 1→0 edge signals too, so a pacer blocked in the clock wait parks on this frame
        // rather than posting one more into a window with nothing to draw.
        if self.count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.event.signal();
        }
    }

    /// Re-opens the post gate. Called from the window procedure *before* the application
    /// sees [`WM_FRAME`], so a frame completing during the tick's work is not swallowed.
    pub(crate) fn begin_frame(&self) {
        self.posted.store(false, Ordering::Release);
    }

    /// Whether the pacer should be asleep: nothing wants a frame, or nothing can see one.
    fn parked(&self) -> bool {
        self.count.load(Ordering::Acquire) == 0 || self.watch.is_hidden()
    }

    /// Parks until a requester appears or the window can be seen again.
    ///
    /// Deliberately no clock in the list: a parked pacer has nothing to do on a frame.
    fn park(&self) {
        wait_any(&[self.event.as_handle(), self.watch.as_handle()]);
    }
}

impl Clone for Tick {
    fn clone(&self) -> Self {
        self.0.acquire();
        Self(Arc::clone(&self.0))
    }
}

impl Drop for Tick {
    fn drop(&mut self) {
        self.0.release();
    }
}

impl core::fmt::Debug for Wake {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Wake")
            .field("requesters", &self.requesters())
            .finish()
    }
}

impl core::fmt::Debug for Tick {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Tick")
    }
}

/// What the pacer could not do.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PacerHealth {
    /// Frames produced by the stalled-clock guard rather than by the display. Steady growth
    /// means every timing measurement taken meanwhile is measuring the session.
    pub stalls: u32,
    /// The clock wait is failing, so nothing is paced. What a session with no compositor
    /// clock — headless, or remote — answers. Cleared by the first frame that arrives, so a
    /// clock that comes back is reported as back.
    pub clockless: bool,
}

/// The pacer thread, tied to the window it posts to.
///
/// The borrow is the safety: handle values are recycled, so a pacer that outlived its window
/// would post into whatever now answers to that value. `Drop` joins the thread.
pub struct Pacer<'w> {
    inner: Arc<Clock>,
    thread: Option<std::thread::JoinHandle<()>>,
    window: &'w Window,
}

impl Window {
    /// Starts a frame pacer for this window.
    ///
    /// The pacer parks until something asks for a tick, then posts [`WM_FRAME`] once per
    /// composition frame. Nothing else is required of the caller: the window procedure
    /// re-opens the pacer's post gate itself, ahead of the application's frame work.
    ///
    /// # Errors
    ///
    /// The window is closed, or already has a pacer.
    pub fn pacer(&self) -> Result<Pacer<'_>> {
        let inner = Arc::new(Clock {
            count: AtomicUsize::new(0),
            event: Event::auto_reset()?,
            posted: AtomicBool::new(false),
            watch: self.watch()?,
            stopping: AtomicBool::new(false),
            stalls: AtomicU32::new(0),
            clockless: AtomicBool::new(false),
            frames: AtomicU64::new(0),
        });
        // A second pacer would post frames the window's gate does not account for.
        if !self.claim_frame_gate(Arc::clone(&inner)) {
            return Err(Error::new(E_HANDLE, "the window already has a pacer"));
        }
        let worker = Arc::clone(&inner);
        let target = Target(self.hwnd());
        let thread = std::thread::Builder::new()
            .name("frame-pacer".into())
            .spawn(move || run(&worker, &target))
            .inspect_err(|_| self.release_frame_gate())
            .map_err(|e| Error::new(E_FAIL, e.to_string()))?;
        Ok(Pacer {
            inner,
            thread: Some(thread),
            window: self,
        })
    }
}

impl Pacer<'_> {
    /// A handle to the frame clock, for anything that can ask for a tick.
    #[must_use]
    pub fn wake(&self) -> Wake {
        Wake(Arc::clone(&self.inner))
    }

    /// A stalled compositor clock, or a session without one.
    #[must_use]
    pub fn health(&self) -> PacerHealth {
        PacerHealth {
            stalls: self.inner.stalls.load(Ordering::Relaxed),
            clockless: self.inner.clockless.load(Ordering::Relaxed),
        }
    }
}

impl Drop for Pacer<'_> {
    fn drop(&mut self) {
        self.inner.stopping.store(true, Ordering::Release);
        self.inner.event.signal();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.window.release_frame_gate();
    }
}

impl core::fmt::Debug for Pacer<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pacer")
            .field("health", &self.health())
            .finish_non_exhaustive()
    }
}

/// The window handle, moved into the pacer thread. Never dereferenced here — it is an
/// opaque token `PostMessageW` resolves itself.
struct Target(*mut core::ffi::c_void);

// SAFETY: `PostMessageW` is callable from any thread, and the handle outlives this thread
// because `Pacer` borrows the window and joins on drop. It can still name a *destroyed*
// window — `WM_DESTROY` parks the pacer, but a wait that returned a frame just before it is
// already past the check — so the post is allowed to fail and nothing is read back through it.
unsafe impl Send for Target {}
unsafe impl Sync for Target {}

fn run(s: &Arc<Clock>, target: &Target) {
    // The display's half of "can anything be seen". Latched by the clock's own occluded
    // return and cleared by one probe after an edge, because the clock is the only thing
    // that can say the display is back.
    let mut dark = false;
    loop {
        // Zero wakes until a requester appears or the window is shown again.
        while s.parked() || dark {
            if s.stopping.load(Ordering::Acquire) {
                return;
            }
            s.park();
            dark = false;
        }
        if s.stopping.load(Ordering::Acquire) {
            return;
        }

        let observed = wait_frame(s);
        if s.stopping.load(Ordering::Acquire) {
            return;
        }
        match observed {
            // No frame elapsed. Posting here would spend one on every guard taken or dropped.
            Observed::Signal(_) => continue,
            Observed::Occluded => {
                dark = true;
                continue;
            }
            Observed::NoClock => {
                // Reported, and parked rather than retried in place. There is no second
                // pacing path: a timer here would be a partially-exercised mechanism that
                // only runs where nobody looks. Parking probes again on the next edge, so a
                // clock that comes back is picked up without a poll.
                s.clockless.store(true, Ordering::Relaxed);
                s.park();
                continue;
            }
            // The guard fired on a stalled clock. Counted, because a run of them invalidates
            // every timing figure taken meanwhile, then treated as a frame so a surface
            // driving itself from the tick degrades rather than freezes.
            Observed::Stalled => {
                s.stalls.fetch_add(1, Ordering::Relaxed);
            }
            Observed::Frame => s.clockless.store(false, Ordering::Relaxed),
        }

        // Counted here rather than on the message, so it counts *display* frames: a
        // consumer that posts the frame message itself to be serviced sooner must not be
        // able to make a per-frame quantity resolve twice in one frame.
        s.frames.fetch_add(1, Ordering::Relaxed);

        // One message in flight: if one is pending the pump has not reached it yet, and a
        // second would only make it do the same work twice.
        if !s.posted.swap(true, Ordering::AcqRel) {
            // SAFETY: see `Target`.
            let posted = unsafe { PostMessageW(target.0, WM_FRAME, 0, 0) };
            if !posted.as_bool() {
                // A full queue, or a window already gone. Re-open rather than wedging the
                // pacer shut for the rest of its life.
                s.posted.store(false, Ordering::Release);
            }
        }
    }
}

/// One blocking wait on the compositor clock, with our own wake sources in the handle list.
///
/// Both are in the *same* list as the clock, so a state change is noticed inside the wait
/// rather than a frame later.
fn wait_frame(s: &Clock) -> Observed {
    clock::wait_for_frame(&[s.event.as_handle(), s.watch.as_handle()], CLOCK_GUARD_MS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visibility::Visibility;
    use std::os::windows::io::BorrowedHandle;

    /// A clock for a window that is on screen, and the visibility it watches.
    fn inner() -> (Arc<Clock>, Arc<Visibility>) {
        let visibility = Arc::new(Visibility::new());
        // A `Visibility` starts hidden, because nothing has been shown yet.
        visibility.publish(false);
        let shared = Arc::new(Clock {
            count: AtomicUsize::new(0),
            event: Event::auto_reset().expect("an event is available"),
            posted: AtomicBool::new(false),
            watch: visibility.watch().expect("an event is available"),
            stopping: AtomicBool::new(false),
            stalls: AtomicU32::new(0),
            clockless: AtomicBool::new(false),
            frames: AtomicU64::new(0),
        });
        (shared, visibility)
    }

    /// Which of `handles` is signalled now, consuming that signal. `None` if none is. The same
    /// wait the pacer makes, with a zero timeout so a test does not block on it.
    fn signalled(handles: &[BorrowedHandle<'_>]) -> Option<u32> {
        // SAFETY: the handles are borrowed from live kernel objects for the call.
        let result = unsafe {
            WaitForMultipleObjects(
                handles.len() as u32,
                handles.as_ptr().cast(),
                false.into(),
                0,
            )
        };
        (result != WAIT_TIMEOUT as u32).then(|| result - WAIT_OBJECT_0 as u32)
    }

    #[test]
    fn a_dropped_guard_releases_its_request() {
        let wake = Wake(inner().0);
        assert!(wake.0.parked());
        {
            let _first = wake.tick();
            assert!(!wake.0.parked());
            let _second = wake.tick();
            assert_eq!(wake.requesters(), 2);
        }
        assert_eq!(wake.requesters(), 0, "a guard leaked its request");
        assert!(wake.0.parked());
    }

    #[test]
    fn a_cloned_guard_is_a_second_request_and_not_a_shared_one() {
        let wake = Wake(inner().0);
        let first = wake.tick();
        let second = first.clone();
        assert_eq!(wake.requesters(), 2);
        drop(first);
        assert_eq!(wake.requesters(), 1);
        drop(second);
        assert_eq!(wake.requesters(), 0);
    }

    #[test]
    fn only_the_edges_touch_the_kernel() {
        let (shared, _visibility) = inner();
        let wake = Wake(Arc::clone(&shared));
        let first = wake.tick();
        assert!(shared.event.take(), "the 0→1 edge must wake the pacer");

        let second = wake.tick();
        let third = wake.tick();
        assert!(
            !shared.event.take(),
            "an interior acquire signalled the event"
        );
        drop(third);
        drop(second);
        assert!(
            !shared.event.take(),
            "an interior release signalled the event"
        );
        drop(first);
        assert!(
            shared.event.take(),
            "the 1→0 edge must interrupt the clock wait so the pacer parks at once"
        );
    }

    #[test]
    fn a_hidden_window_parks_even_with_live_requesters() {
        let (shared, visibility) = inner();
        let wake = Wake(Arc::clone(&shared));
        let _tick = wake.tick();
        assert!(!shared.parked());
        visibility.publish(true);
        assert!(
            shared.parked(),
            "a window nobody can see is still being paced"
        );
        visibility.publish(false);
        assert!(
            !shared.parked(),
            "the window came back and the pacer did not"
        );
    }

    #[test]
    fn hiding_the_window_interrupts_an_in_flight_clock_wait() {
        // The pacer's wake sources ride in the *same* handle list as the clock, so the park
        // happens inside the wait rather than a frame later.
        let (shared, visibility) = inner();
        let handles = [shared.event.as_handle(), shared.watch.as_handle()];
        visibility.publish(true);
        assert_eq!(
            signalled(&handles),
            Some(1),
            "hiding the window did not reach the pacer's own wait"
        );
        assert_eq!(
            signalled(&handles),
            None,
            "one change woke the pacer twice, which is a parked pacer spinning"
        );
    }

    #[test]
    fn a_tick_can_be_held_across_threads() {
        let wake = Wake(inner().0);
        let tick = wake.tick();
        std::thread::spawn(move || drop(tick))
            .join()
            .expect("the holder ran");
        assert_eq!(wake.requesters(), 0);
    }

    #[test]
    fn the_post_gate_coalesces_and_reopens() {
        let (shared, _visibility) = inner();
        assert!(!shared.posted.swap(true, Ordering::AcqRel));
        assert!(shared.posted.swap(true, Ordering::AcqRel));
        shared.begin_frame();
        assert!(!shared.posted.swap(true, Ordering::AcqRel));
    }
}
