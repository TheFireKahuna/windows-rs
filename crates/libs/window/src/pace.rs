//! The frame pacer: a counter with RAII guards, not a poll.
//!
//! One thread blocks on the compositor's own clock and posts a message, so a window's
//! frame work happens inside its ordinary pump, in message order with input. **There is no
//! timer.**
//!
//! A tick is requested by, and only by, a live [`Tick`] guard. The **0→1 transition signals
//! the wake event**, so no window exists in which a requester exists and the pacer sleeps.
//! A dropped guard decrements, which makes "zero wakes at idle" a property of ownership.
//!
//! # What the clock wait requires
//!
//! Five behaviours the API does not suggest, each a real failure:
//!
//! - **The event is auto-reset.** Left signalled, it satisfies every subsequent clock wait
//!   instantly and the pacer becomes a busy loop posting frames as fast as a core will go.
//! - **The return value is not a boolean.** Index `0` is our event — a state change, no
//!   frame elapsed; `WAIT_FAILED` is a session with no compositor clock; anything else is a
//!   frame. Posting on all of them spends a frame on every state change.
//! - **The wait is guarded, not infinite.** A stalled clock — display off, mode switch,
//!   locked or remote session — otherwise freezes every paced surface with nothing to
//!   report it. A guard expiry counts as a frame, so motion degrades to a trickle.
//! - **At most one message is in flight.** A pump slower than the display otherwise
//!   accumulates stale frame messages and ticks for each. The gate re-opens *before* the
//!   tick's work, so a frame completing during it is not lost.
//! - **An occluded window parks** — the common case for a minimized app still animating.

use crate::bindings::*;
use crate::Window;
use crate::event::Event;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use windows_core::{Error, Result};

/// The message the pacer posts. `WM_APP` is the base of the range reserved for an
/// application's own messages, so nothing else in the window's stream can collide with it.
pub const WM_APP_FRAME: u32 = WM_APP as u32 + 0x44;

/// How long a clock wait may block before it is treated as a frame.
///
/// This only ever bounds a *stalled* clock. Wide enough that a healthy display never
/// reaches it — six frames at 60 Hz — and short enough that a surface driving itself from
/// the tick keeps moving, slowly, rather than appearing to hang.
const CLOCK_GUARD_MS: u32 = 100;

/// A handle to the frame clock, handed to anything that can ask for a tick.
///
/// `Clone + Send + Sync`, so every requester takes the same guard type: an active pointer
/// contact, a pending realization, a queued patch on the app thread, a scene's outstanding
/// exit animations and non-idle trackers. That is what lets a thread that is not the
/// window's hold a request with no second mechanism.
#[derive(Clone)]
pub struct Wake(Arc<Inner>);

/// One live request for the frame clock. Dropping it releases the request.
///
/// `Send`, and deliberately not `Copy`: the release is the drop.
pub struct Tick(Arc<Inner>);

struct Inner {
    /// Live requesters. The 0→1 and 1→0 edges are the only ones that touch the kernel.
    count: AtomicUsize,
    /// Auto-reset, initially unsignalled: one signal, one worker wake. It rides in the
    /// clock wait's handle list, so a state change is noticed *inside* the wait rather than
    /// one frame later.
    event: Event,
    /// Whether a frame message is already in flight. The whole of the coalescing.
    posted: AtomicBool,
    /// Set while the window cannot be seen. A veto rather than a request, which is why it
    /// is not expressible as a `Tick`.
    occluded: AtomicBool,
    stopping: AtomicBool,
    /// Frames the clock guard produced rather than the clock. Non-zero means the compositor
    /// clock stalled, which is a fact about the session and not about us.
    stalls: AtomicU32,
    /// Set once if the session has no compositor clock at all. Reported rather than worked
    /// around: a second, partially-exercised pacing path is the failure mode this design
    /// exists to remove.
    clockless: AtomicBool,
}

impl Wake {
    /// Requests a tick until the returned guard is dropped.
    #[must_use]
    pub fn tick(&self) -> Tick {
        self.0.acquire();
        Tick(Arc::clone(&self.0))
    }

    /// How many requesters are live. Zero means the pacer is parked and the pump is
    /// blocked — which is what "zero wakes at idle" looks like from outside.
    #[must_use]
    pub fn requesters(&self) -> usize {
        self.0.count.load(Ordering::Acquire)
    }
}

impl Inner {
    fn acquire(&self) {
        // Only the 0→1 edge signals. A second requester while one is already live is one
        // increment and no kernel call at all, which is the steady state during a drag —
        // several guards live, none of them touching the event.
        if self.count.fetch_add(1, Ordering::AcqRel) == 0 {
            self.event.signal();
        }
    }

    fn release(&self) {
        // The 1→0 edge signals too, so a pacer blocked in the clock wait parks on this
        // frame rather than posting one more into a window that has nothing to draw.
        if self.count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.event.signal();
        }
    }

    /// Whether the pacer should be asleep: nothing wants a frame, or nothing can see one.
    fn parked(&self) -> bool {
        self.count.load(Ordering::Acquire) == 0 || self.occluded.load(Ordering::Acquire)
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

/// What one blocking wait observed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Observed {
    /// A composition frame elapsed — or the guard expired on a stalled clock, which is
    /// treated the same so that motion degrades rather than stopping.
    Frame,
    /// Our own event fired: something changed, and **no frame elapsed**. Re-check and post
    /// nothing.
    StateChange,
    /// The session has no compositor clock.
    NoClock,
}

/// How the pacer reports what it could not do.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PacerHealth {
    /// Frames produced by the stalled-clock guard rather than by the display. Steady growth
    /// means the compositor clock is not running — a locked session, a sleeping display, a
    /// mode switch — and every timing measurement taken during it is meaningless.
    pub stalls: u32,
    /// The session has no compositor clock at all, so nothing is paced. Named rather than
    /// worked around.
    pub clockless: bool,
}

/// The pacer thread, tied to the window it posts to.
///
/// The borrow is the safety: a pacer cannot outlive its window, so the handle it posts to
/// cannot be destroyed and its value recycled underneath it. `Drop` joins the thread.
pub struct Pacer<'w> {
    inner: Arc<Inner>,
    thread: Option<std::thread::JoinHandle<()>>,
    window: PhantomData<&'w Window>,
}

impl Window {
    /// Starts a frame pacer for this window.
    ///
    /// The pacer parks until something asks for a tick, then posts [`WM_APP_FRAME`] once
    /// per composition frame. The window's message arm calls
    /// [`begin_tick`](Pacer::begin_tick) before doing the frame's work.
    pub fn pacer(&self) -> Result<Pacer<'_>> {
        let inner = Arc::new(Inner {
            count: AtomicUsize::new(0),
            event: Event::auto_reset()?,
            posted: AtomicBool::new(false),
            occluded: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            stalls: AtomicU32::new(0),
            clockless: AtomicBool::new(false),
        });
        let worker = Arc::clone(&inner);
        let target = Target(self.hwnd());
        let thread = std::thread::Builder::new()
            .name("frame-pacer".into())
            .spawn(move || run(&worker, &target))
            .map_err(|e| Error::new(windows_core::HRESULT(E_FAIL), e.to_string()))?;
        Ok(Pacer {
            inner,
            thread: Some(thread),
            window: PhantomData,
        })
    }
}

impl Pacer<'_> {
    /// A handle to the frame clock, for anything that can ask for a tick.
    #[must_use]
    pub fn wake(&self) -> Wake {
        Wake(Arc::clone(&self.inner))
    }

    /// Re-opens the post gate.
    ///
    /// Called by the window's frame-message arm **before** it does the tick's work, so a
    /// frame that completes during that work is posted rather than swallowed.
    pub fn begin_tick(&self) {
        self.inner.posted.store(false, Ordering::Release);
    }

    /// Whether the window can be seen. Hiding parks the pacer even with live requesters,
    /// because pacing a window nobody can see is pure waste.
    pub fn set_occluded(&self, occluded: bool) {
        if self.inner.occluded.swap(occluded, Ordering::AcqRel) != occluded {
            // Both edges: hiding interrupts an in-flight clock wait so the park is
            // immediate, and showing ends the park.
            self.inner.event.signal();
        }
    }

    /// What the pacer could not do: a stalled compositor clock, or a session without one.
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
    }
}

/// The window handle, moved into the pacer thread.
///
/// A raw pointer is not `Send`, and a window handle genuinely is not a pointer to anything
/// this thread may dereference — it is an opaque token the system resolves.
struct Target(*mut core::ffi::c_void);

// SAFETY: the value is never dereferenced here; it is passed straight back to
// `PostMessageW`, which is documented as callable from any thread and which resolves the
// handle itself. Its lifetime is discharged by `Pacer` borrowing the window and joining
// this thread on drop.
unsafe impl Send for Target {}
unsafe impl Sync for Target {}

fn run(s: &Arc<Inner>, target: &Target) {
    loop {
        // ── parked ────────────────────────────────────────────────────────────────────
        // Zero wakes until a requester appears or the window is shown again. This is the
        // whole of "zero cost at idle": a presentation region is *not* a requester, because
        // it presents on its own thread and the compositor picks its buffers up without us.
        while s.parked() {
            if s.stopping.load(Ordering::Acquire) {
                return;
            }
            s.event.wait(INFINITE);
        }
        if s.stopping.load(Ordering::Acquire) {
            return;
        }

        // ── paced ─────────────────────────────────────────────────────────────────────
        let observed = wait_frame(s);
        if s.stopping.load(Ordering::Acquire) {
            return;
        }
        match observed {
            // A state change is why we woke; the loop re-evaluates whether to park. No
            // frame elapsed, so nothing is posted — posting here spends a frame on every
            // guard that is taken or dropped.
            Observed::StateChange => continue,
            Observed::NoClock => {
                // Latched and reported. There is deliberately no second pacing path: a
                // timer here would be a partially-exercised mechanism that only ever runs
                // where nobody is looking.
                s.clockless.store(true, Ordering::Relaxed);
                s.event.wait(INFINITE);
                continue;
            }
            Observed::Frame => {}
        }

        // One message in flight. The swap is the gate: if one is already pending the pump
        // has not reached it yet, and a second would only make it do the same work twice.
        if !s.posted.swap(true, Ordering::AcqRel) {
            // SAFETY: posting to a window handle is documented as callable from any thread;
            // the handle outlives this thread because `Pacer` borrows the window.
            let posted = unsafe { PostMessageW(target.0, WM_APP_FRAME, 0, 0) };
            if !posted.as_bool() {
                // A full queue, or a window already gone. Re-open the gate and try the next
                // frame rather than wedging the pacer shut for the rest of its life.
                s.posted.store(false, Ordering::Release);
            }
        }
    }
}

/// One blocking wait on the compositor clock, with our event in the handle list.
fn wait_frame(s: &Inner) -> Observed {
    let handles = [s.event.raw()];
    // SAFETY: the handle array outlives the call and the count matches its length.
    let result = unsafe { DCompositionWaitForCompositorClock(1, handles.as_ptr(), CLOCK_GUARD_MS) };
    // The wait answers with the index of what woke it: our event is the one handle we
    // passed, and the clock is the implicit one after it.
    const OURS: u32 = 0;
    const CLOCK: u32 = 1;
    match result {
        // A requester appeared or left, or the window was hidden.
        OURS => Observed::StateChange,
        WAIT_FAILED => Observed::NoClock,
        CLOCK => Observed::Frame,
        // The guard timeout, deliberately treated as a frame so a stalled clock degrades
        // motion rather than freezing it — but counted, because a run of them means every
        // timing figure taken meanwhile is measuring a session rather than an application.
        _ => {
            s.stalls.fetch_add(1, Ordering::Relaxed);
            Observed::Frame
        }
    }
}

const E_FAIL: i32 = -2147467259;

#[cfg(test)]
mod tests {
    use super::*;

    fn inner() -> Arc<Inner> {
        Arc::new(Inner {
            count: AtomicUsize::new(0),
            event: Event::auto_reset().expect("an event is available"),
            posted: AtomicBool::new(false),
            occluded: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            stalls: AtomicU32::new(0),
            clockless: AtomicBool::new(false),
        })
    }

    /// Whether the event is signalled right now, consuming the signal if it is.
    fn signalled(shared: &Inner) -> bool {
        // SAFETY: the handle is owned by `shared` and outlives the call.
        shared.event.take()
    }

    #[test]
    fn a_dropped_guard_releases_its_request() {
        let wake = Wake(inner());
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
        let wake = Wake(inner());
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
        // The steady state during a drag is several live guards and no signalling at all.
        let shared = inner();
        let wake = Wake(Arc::clone(&shared));
        let first = wake.tick();
        assert!(signalled(&shared), "the 0→1 edge must wake the pacer");

        let second = wake.tick();
        let third = wake.tick();
        assert!(!signalled(&shared), "an interior acquire signalled the event");
        drop(third);
        drop(second);
        assert!(!signalled(&shared), "an interior release signalled the event");
        drop(first);
        assert!(
            signalled(&shared),
            "the 1→0 edge must interrupt the clock wait so the pacer parks at once"
        );
    }

    #[test]
    fn occlusion_parks_even_with_live_requesters() {
        let shared = inner();
        let wake = Wake(Arc::clone(&shared));
        let _tick = wake.tick();
        assert!(!shared.parked());
        shared.occluded.store(true, Ordering::Release);
        assert!(shared.parked(), "a window nobody can see is still being paced");
    }

    #[test]
    fn a_tick_can_be_held_across_threads() {
        // A thread that is not the window's holds one for work it has queued, which is the
        // whole reason this is `Send` rather than a pump-thread-only handle.
        let wake = Wake(inner());
        let tick = wake.tick();
        std::thread::spawn(move || drop(tick))
            .join()
            .expect("the holder ran");
        assert_eq!(wake.requesters(), 0);
    }

    #[test]
    fn the_post_gate_coalesces_and_reopens() {
        let shared = inner();
        // The first post claims the gate; a second while one is in flight is refused.
        assert!(!shared.posted.swap(true, Ordering::AcqRel));
        assert!(shared.posted.swap(true, Ordering::AcqRel));
        // The tick re-opens it before doing its work, so a frame completed during that
        // work is not lost.
        shared.posted.store(false, Ordering::Release);
        assert!(!shared.posted.swap(true, Ordering::AcqRel));
    }
}
