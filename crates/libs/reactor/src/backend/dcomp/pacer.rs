//! Vsync frame pacer for the backend frame ticks (canvas/viz subscribers).
//!
//! The DComp backend has no XAML `CompositionTarget::Rendering` to pace
//! app-drawn canvases, and the `WM_TIMER` it used to use is the wrong clock:
//! ~15.6 ms granularity, lowest-priority message (an input flood starves it,
//! which reads as drag lag), and unaligned with the display. Instead a
//! dedicated worker thread (spawned on the first subscriber, so a window that
//! never animates a canvas never pays for it) blocks on
//! `DCompositionWaitForCompositorClock` —
//! one wake per DWM composition frame — and posts a coalesced
//! [`WM_APP_FRAME`] to the UI thread, whose WndProc drives
//! [`crate::drive_frame_ticks`]. While no subscriber is live (or the window is
//! minimized) the worker parks on a condvar, so an idle window costs zero
//! wakes on both threads.
//!
//! A kernel auto-reset event rides in the clock wait's handle list: state
//! changes (park, hide, quit) signal it, so the worker reacts within the wait
//! instead of one frame later.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::system_bindings::{
    CloseHandle, CreateEventW, CreateWaitableTimerExW, DCompositionWaitForCompositorClock,
    PostMessageW, SetEvent, SetWaitableTimerEx, WaitForMultipleObjects, HANDLE, HWND,
    TIMER_ALL_ACCESS, WAIT_FAILED, WM_APP,
};

/// App message posted (coalesced — at most one in flight) by the pacer worker
/// once per compositor frame while frame-tick subscribers are live.
pub(crate) const WM_APP_FRAME: u32 = WM_APP + 0x44;

/// Guard timeout for the compositor-clock wait. State changes interrupt the
/// wait via the wake event, so this only bounds a *stalled* DWM clock (display
/// off / mode switch): a guard expiry is treated as a frame, so animations
/// degrade to a slow trickle rather than freezing.
const CLOCK_GUARD_MS: u32 = 100;

/// Tick period when `DCompositionWaitForCompositorClock` is unavailable
/// (headless / remote session): the old timer cadence.
const FALLBACK_TICK: Duration = Duration::from_millis(15);

/// Coalescing tolerance for the fallback timer: lets the kernel merge our
/// expiry with nearby timers (fewer distinct wakeups) without visibly
/// changing the tick cadence.
const FALLBACK_TOLERANCE_MS: u32 = 8;

/// `INFINITE` for the fallback wait — the armed timer (or the wake event)
/// always ends it.
const INFINITE_MS: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// No frame-tick subscribers: the worker sleeps on the condvar.
    Parked,
    /// Subscribers live: the worker posts one [`WM_APP_FRAME`] per frame.
    Running,
    /// Host gone: the worker exits.
    Quit,
}

struct State {
    phase: Phase,
    /// Window visibility gate: while minimized, a `Running` pacer still parks —
    /// rasterizing frames nobody can see is pure waste. Subscriptions survive;
    /// restoring resumes ticks.
    visible: bool,
}

struct Shared {
    state: Mutex<State>,
    cv: Condvar,
    /// Post gate: set by the worker before posting, cleared by the WndProc as
    /// it handles the message. Keeps a busy UI thread from accumulating a
    /// backlog of stale frame messages — ticks coalesce instead.
    tick_pending: AtomicBool,
    hwnd: isize,
    /// Auto-reset kernel event pulsed on every state change, so a worker
    /// blocked in the clock wait re-checks immediately (prompt park/hide/quit
    /// instead of "noticed at the next vsync"). `0` if creation failed — the
    /// worker then just reacts at the next clock tick.
    wake_evt: isize,
    /// Whether the worker thread exists yet — spawned lazily on the first
    /// wake, so a window that never animates a canvas never pays for the
    /// thread. Atomic because a wake may arrive from any thread (the
    /// frame-pump hook fires wherever a subscriber registers): the swap
    /// elects exactly one spawner.
    spawned: AtomicBool,
}

impl Drop for Shared {
    fn drop(&mut self) {
        if self.wake_evt != 0 {
            // SAFETY: the handle was created by us and both referents (UI-side
            // FramePacer, the worker) are gone once Shared drops.
            unsafe {
                let _ = CloseHandle(self.wake_evt as HANDLE);
            }
        }
    }
}

impl Shared {
    /// Signal the wake event (no-op when creation failed).
    fn pulse(&self) {
        if self.wake_evt != 0 {
            unsafe {
                let _ = SetEvent(self.wake_evt as HANDLE);
            }
        }
    }

    /// Start vsync ticks — a frame-tick subscriber appeared. Idempotent while
    /// already running; callable from any thread. The first call spawns the
    /// worker (the swap elects one spawner; on spawn failure the flag reopens
    /// so a later wake retries). The load in front keeps the steady state
    /// read-only: once the worker exists, a wake never writes the shared line
    /// — the swap runs only in the race window before anyone has spawned.
    fn wake(self: &Arc<Self>) {
        if !self.spawned.load(Ordering::Acquire) && !self.spawned.swap(true, Ordering::AcqRel) {
            let worker = Arc::clone(self);
            // Detached on purpose: joining could block the UI thread. Quit is
            // signalled via `Phase::Quit` + the wake event on drop, so the
            // worker exits promptly.
            if std::thread::Builder::new()
                .name("reactor-frame-pacer".into())
                .spawn(move || run_worker(&worker))
                .is_err()
            {
                self.spawned.store(false, Ordering::Release);
                return;
            }
        }
        let mut st = self.state.lock().unwrap();
        if st.phase == Phase::Parked {
            st.phase = Phase::Running;
            self.cv.notify_all();
        }
    }
}

/// A `Send + Sync` wake handle for the frame-pump hook: wherever a subscriber
/// registers, this reaches the worker through the shared state alone.
pub(crate) struct PacerWake {
    shared: Arc<Shared>,
}

impl PacerWake {
    pub fn wake(&self) {
        self.shared.wake();
    }
}

/// Owning handle held by the host; dropping it tells the worker to exit.
pub(crate) struct FramePacer {
    shared: Arc<Shared>,
}

impl FramePacer {
    pub fn new(hwnd: isize) -> Self {
        // Auto-reset, initially unsignalled: one SetEvent = one worker wake.
        let wake_evt = unsafe {
            CreateEventW(core::ptr::null(), false.into(), false.into(), windows_core::PCWSTR::null())
        } as isize;
        let shared = Arc::new(Shared {
            state: Mutex::new(State { phase: Phase::Parked, visible: true }),
            cv: Condvar::new(),
            tick_pending: AtomicBool::new(false),
            hwnd,
            wake_evt,
            spawned: AtomicBool::new(false),
        });
        Self { shared }
    }

    /// A wake handle for [`crate::set_frame_pump_wake`] — safe to fire from
    /// any thread.
    pub fn wake_handle(&self) -> PacerWake {
        PacerWake {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Re-open the post gate; the WndProc calls this as it handles a
    /// [`WM_APP_FRAME`], before driving the subscribers, so a frame completed
    /// during the drive is not lost.
    pub fn begin_tick(&self) {
        self.shared.tick_pending.store(false, Ordering::Release);
    }

    /// Stop ticking — the last frame-tick subscriber is gone. The wake event
    /// interrupts an in-flight clock wait, so the worker parks immediately.
    pub fn park(&self) {
        let mut st = self.shared.state.lock().unwrap();
        if st.phase == Phase::Running {
            st.phase = Phase::Parked;
            drop(st);
            self.shared.pulse();
        }
    }

    /// Window visibility edge (minimize / restore). Hiding parks the worker
    /// even with live subscribers; restoring resumes ticks for them.
    pub fn set_visible(&self, visible: bool) {
        let mut st = self.shared.state.lock().unwrap();
        if st.visible == visible {
            return;
        }
        st.visible = visible;
        drop(st);
        if visible {
            self.shared.cv.notify_all();
        } else {
            self.shared.pulse();
        }
    }
}

impl Drop for FramePacer {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap().phase = Phase::Quit;
        self.shared.cv.notify_all();
        self.shared.pulse();
    }
}

/// What one blocking wait observed.
enum Wake {
    /// The compositor clock ticked (or the stalled-clock guard expired).
    Frame,
    /// The wake event fired: state changed — re-check, nothing to post.
    StateChange,
    /// The clock wait is unavailable on this session.
    Failed,
}

fn wait_frame(s: &Shared) -> Wake {
    if s.wake_evt != 0 {
        let handles = [s.wake_evt as HANDLE];
        let r = unsafe { DCompositionWaitForCompositorClock(1, handles.as_ptr(), CLOCK_GUARD_MS) };
        match r {
            0 => Wake::StateChange,
            WAIT_FAILED => Wake::Failed,
            _ => Wake::Frame, // index 1 (the clock) or WAIT_TIMEOUT (guard)
        }
    } else {
        let r = unsafe { DCompositionWaitForCompositorClock(0, core::ptr::null(), CLOCK_GUARD_MS) };
        if r == WAIT_FAILED { Wake::Failed } else { Wake::Frame }
    }
}

/// The fallback path's one-shot kernel timer, created on first use (i.e. only
/// on sessions where the compositor clock is unavailable). One-shot re-arm per
/// tick keeps its lifecycle trivial: nothing fires while the worker is parked.
#[derive(Default)]
struct FallbackTimer {
    handle: isize,
    tried: bool,
}

impl Drop for FallbackTimer {
    fn drop(&mut self) {
        if self.handle != 0 {
            // SAFETY: created by us; the worker (sole user) is exiting.
            unsafe {
                let _ = CloseHandle(self.handle as HANDLE);
            }
        }
    }
}

/// One fallback tick: arm the one-shot timer (with coalescing tolerance) and
/// wait on it alongside the wake event, so state changes still interrupt
/// promptly. Degrades to a plain condvar timeout if the kernel timer is
/// unavailable.
fn wait_fallback(s: &Shared, t: &mut FallbackTimer) -> Wake {
    if !t.tried {
        t.tried = true;
        t.handle = unsafe {
            CreateWaitableTimerExW(
                core::ptr::null(),
                windows_core::PCWSTR::null(),
                0,
                TIMER_ALL_ACCESS,
            )
        } as isize;
    }
    let armed = t.handle != 0 && {
        // Relative due time, negative, in 100 ns units.
        let due = -((FALLBACK_TICK.as_nanos() / 100) as i64);
        unsafe {
            SetWaitableTimerEx(
                t.handle as HANDLE,
                &due,
                0,
                None,
                core::ptr::null(),
                core::ptr::null(),
                FALLBACK_TOLERANCE_MS,
            )
        }
        .as_bool()
    };
    if !armed {
        let st = s.state.lock().unwrap();
        let _ = s.cv.wait_timeout(st, FALLBACK_TICK).unwrap();
        return Wake::Frame;
    }
    let handles = [t.handle as HANDLE, s.wake_evt as HANDLE];
    let count = if s.wake_evt != 0 { 2 } else { 1 };
    let r = unsafe { WaitForMultipleObjects(count, handles.as_ptr(), false.into(), INFINITE_MS) };
    match r {
        0 => Wake::Frame,       // the timer
        1 => Wake::StateChange, // the wake event
        _ => {
            // Wait failure — sleep the tick out instead of spinning.
            let st = s.state.lock().unwrap();
            let _ = s.cv.wait_timeout(st, FALLBACK_TICK).unwrap();
            Wake::Frame
        }
    }
}

fn run_worker(s: &Shared) {
    // Latched false the first time the clock wait fails (headless / remote
    // session); pacing then falls back to the tolerant kernel timer.
    let mut clock_ok = true;
    let mut fallback = FallbackTimer::default();
    'park: loop {
        // Idle: zero wakes until the host wakes/restores or quits us.
        {
            let mut st = s.state.lock().unwrap();
            loop {
                match st.phase {
                    Phase::Quit => return,
                    Phase::Running if st.visible => break,
                    _ => st = s.cv.wait(st).unwrap(),
                }
            }
        }
        // Paced: one coalesced post per DWM composition frame.
        loop {
            let wake = if clock_ok {
                wait_frame(s)
            } else {
                wait_fallback(s, &mut fallback)
            };
            {
                let st = s.state.lock().unwrap();
                match st.phase {
                    Phase::Quit => return,
                    Phase::Parked => continue 'park,
                    Phase::Running if !st.visible => continue 'park,
                    Phase::Running => {}
                }
            }
            match wake {
                Wake::Failed => {
                    clock_ok = false;
                    continue;
                }
                // State re-checked above and unchanged (a stale pulse): no frame
                // elapsed, nothing to post.
                Wake::StateChange => continue,
                Wake::Frame => {}
            }
            if !s.tick_pending.swap(true, Ordering::AcqRel) {
                let posted = unsafe { PostMessageW(s.hwnd as HWND, WM_APP_FRAME, 0, 0) };
                if !posted.as_bool() {
                    // Queue full or window gone — re-open the gate and retry
                    // next frame rather than wedging the pacer.
                    s.tick_pending.store(false, Ordering::Release);
                }
            }
        }
    }
}
