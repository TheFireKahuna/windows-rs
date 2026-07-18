use std::time::Duration;

use super::*;
use bindings::*;

/// RAII timer wrapper; stops and unhooks on drop.
pub struct DispatcherTimer {
    timer: DispatcherQueueTimer,
    _tick_revoker: windows_core::EventRevoker,
}

impl DispatcherTimer {
    pub fn new<F>(interval: Duration, f: F) -> Result<Self>
    where
        F: Fn() + 'static,
    {
        Self::build(interval, true, f)
    }

    pub fn new_one_shot<F>(after: Duration, f: F) -> Result<Self>
    where
        F: Fn() + 'static,
    {
        Self::build(after, false, f)
    }

    fn build<F>(interval: Duration, repeating: bool, f: F) -> Result<Self>
    where
        F: Fn() + 'static,
    {
        let queue = DispatcherQueue::GetForCurrentThread()?;
        let timer = queue.CreateTimer()?;
        timer.SetInterval(duration_to_timespan(interval))?;
        timer.SetIsRepeating(repeating)?;

        let tick_revoker = timer.Tick(move |_, _| {
            fault::catch("timer", &f);
        })?;
        timer.Start()?;
        Ok(Self {
            timer,
            _tick_revoker: tick_revoker,
        })
    }

    pub fn stop(&self) -> Result<()> {
        self.timer.Stop()
    }

    pub fn start(&self) -> Result<()> {
        self.timer.Start()
    }
}

impl Drop for DispatcherTimer {
    fn drop(&mut self) {
        let _ = self.timer.Stop();
    }
}

/// RAII handle for a per-frame subscription; detaches on drop. Backed by the
/// XAML `CompositionTarget::Rendering` event on the WinUI backend, or by the
/// backend frame tick (see [`on_frame_tick`]) when a host frame pump is driving
/// the current thread (the self-hosted DirectComposition backend, which has no
/// XAML `CompositionTarget`).
pub struct Rendering {
    // One of the two pacing sources is kept alive for the subscription's life.
    _xaml: Option<windows_core::EventRevoker>,
    _tick: Option<FrameTick>,
}

/// Subscribe `f` to the current thread's per-frame tick.
///
/// On the WinUI backend this is `CompositionTarget::Rendering`. When a host owns
/// the backend frame pump (the DirectComposition backend installs one via
/// [`set_frame_pump_wake`]), there is no XAML static to subscribe to, so the
/// callback is paced by the backend frame tick instead — transparently, so
/// canvas/viz code (`SurfacePainter`, `animated_canvas`) need not know which
/// backend hosts them.
pub fn on_rendering<F>(f: F) -> Result<Rendering>
where
    F: Fn() + 'static,
{
    // A host frame pump (DComp) being installed is the runtime signal that we are
    // not under XAML; pace via the backend frame tick in that case.
    if FRAME_PUMP_WAKE.with(|w| w.borrow().is_some()) {
        return Ok(Rendering {
            _xaml: None,
            _tick: Some(on_frame_tick(f)),
        });
    }
    let revoker = CompositionTarget::Rendering(move |_, _| {
        fault::catch("rendering", &f);
    })?;
    Ok(Rendering {
        _xaml: Some(revoker),
        _tick: None,
    })
}

fn duration_to_timespan(d: Duration) -> TimeSpan {
    TimeSpan::try_from(d).unwrap_or(TimeSpan::MAX)
}

// ─── Backend frame tick ──────────────────────────────────────────────────
//
// A backend-agnostic per-frame callback registry. On the WinUI backend frames
// come from `CompositionTarget::Rendering` (see `on_rendering`); the self-hosted
// DirectComposition backend has no XAML static, so it drives this registry from
// its compositor-clock frame pacer instead — one wake per display refresh, and
// only while at least one subscriber is live, so an idle window keeps zero CPU.
// Canvas/viz (`SurfacePainter`, `animated_canvas`) can subscribe here to be
// paced by whichever backend is hosting them, without depending on XAML.

use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static FRAME_TICKS: RefCell<Vec<(u64, Rc<dyn Fn()>)>> = const { RefCell::new(Vec::new()) };
    static FRAME_TICK_NEXT_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    // Installed by a host that owns a frame pump (e.g. the DComp timer); called
    // when a subscriber is added so the pump can start ticking.
    static FRAME_PUMP_WAKE: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// RAII handle for a backend frame-tick subscription; unsubscribes on drop.
/// When the last handle drops, a host pump observing [`frame_ticks_active`]
/// stops, returning the thread to idle.
pub struct FrameTick {
    id: u64,
}

impl Drop for FrameTick {
    fn drop(&mut self) {
        FRAME_TICKS.with(|t| {
            t.borrow_mut().retain(|(id, _)| *id != self.id);
        });
    }
}

/// Subscribe `f` to the backend frame tick for the current thread. The returned
/// [`FrameTick`] keeps the subscription alive; dropping it unsubscribes. Adding
/// the first subscriber wakes the host's frame pump (if one is installed).
pub fn on_frame_tick<F>(f: F) -> FrameTick
where
    F: Fn() + 'static,
{
    let id = FRAME_TICK_NEXT_ID.with(|c| {
        let v = c.get();
        c.set(v + 1);
        v
    });
    FRAME_TICKS.with(|t| t.borrow_mut().push((id, Rc::new(f))));
    // Kick the host pump so the new subscriber starts receiving ticks.
    if let Some(wake) = FRAME_PUMP_WAKE.with(|w| w.borrow().clone()) {
        wake();
    }
    FrameTick { id }
}

/// Whether any backend frame-tick subscriber is currently live. A host pump uses
/// this to decide whether to keep ticking or return to idle.
pub fn frame_ticks_active() -> bool {
    FRAME_TICKS.with(|t| !t.borrow().is_empty())
}

/// Invoke every live frame-tick callback once. Called by the host's frame pump
/// (the DComp backend's compositor-clock pacer) per frame.
pub fn drive_frame_ticks() {
    let ticks: Vec<Rc<dyn Fn()>> = FRAME_TICKS.with(|t| t.borrow().iter().map(|(_, f)| f.clone()).collect());
    for f in ticks {
        f();
    }
}

/// Install (or clear) the host frame-pump wake hook. A host that owns a frame
/// pump installs this so [`on_frame_tick`] can start the pump when a subscriber
/// appears while the pump is idle. Returns nothing; pass `None` to clear.
pub fn set_frame_pump_wake(wake: Option<Rc<dyn Fn()>>) {
    FRAME_PUMP_WAKE.with(|w| *w.borrow_mut() = wake);
}
