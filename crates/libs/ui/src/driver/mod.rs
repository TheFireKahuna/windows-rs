//! Start-up and the threads: the process-wide installs, the window, and the four threads a
//! running window is made of.
//!
//! # The threads
//!
//! ```text
//! input   the window's own thread: the pump, the doorbell, the router, the caption's answers.
//!         Resolves contacts against a copy of the hit array, moves nothing on screen, and
//!         hands its reports to the scene thread. Ticks on the frame clock only while a
//!         contact or an inertia is live.
//! scene   owns the compositor and the retained tree. Applies a patch when it arrives and
//!         turns a report into a retarget when it arrives; has no clock of its own.
//! app     owns the signal graph, the model and the overlay stack; parks on a doorbell and
//!         wakes on a write. Has no clock.
//! present owns the presentation regions and draws them off the compositor clock; posts to
//!         nobody but the scene thread's binder.
//! ```
//!
//! Every crossing between them is a mailbox over a buffer allocated once, and every wait is
//! on a doorbell or the compositor's own clock. An idle window costs no wakes on any of them.
//!
//! # Why the order lives here
//!
//! Within each thread's pass the step order is a correctness rule at every seam, and each
//! ordering comment names what breaks if the line moves. Start-up carries one constraint that
//! is not about order: the shaping engine's font ladder must be the same instance the
//! rasterizing engine holds, because two ladders agree on face 0 and disagree on everything
//! after it. The scene thread builds the [`Backends`] and publishes their ladder before the
//! app thread starts.
//!
//! # What stays the application's
//!
//! The compositor and the GPU. This crate declares into a retained tree and never builds one,
//! which is what keeps `windows-composition` and `windows-d2d` out of its dependencies, so
//! the application constructs the [`Backends`] — on the scene thread, through the closure it
//! hands over.

mod app;
mod input;
mod links;
mod scene;

use crate::build::Mount;
use crate::input::Report;
use crate::role::{AccentId, Density, Palette, Scope};
pub use crate::seam::AppCensus;
use core::sync::atomic::Ordering;
use input::Frame;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use windows_color::OutputTransform;
use windows_core::{Error, Result};
use windows_numerics::Vector2;
use windows_scene::{BackdropSpec, Backends, Census, Env, GroupId};
use windows_window::{CaptionHit, CaptionState, E_HANDLE, Handoff, Watch, Window, WindowBuilder};

/// The process-wide installs, and the root [`Scope`] they produce.
///
/// Constructed before any role resolves: resolving without a palette panics rather than
/// inventing a colour, so a palette that arrives late is a start-up failure with a stack
/// rather than a grey screen.
///
/// The root scope is reachable only through [`Ui::install`], so a caller needing a number
/// before its window exists — a custom caption's band is stated in row heights, and a row
/// height is the palette's answer at the root scope — has necessarily installed the palette
/// already.
#[derive(Copy, Clone)]
pub struct Ui {
    root_scope: Scope,
}

/// What the application's tree builder is handed, on the app thread.
pub struct AppCtx {
    /// The group the application mounts under: a full-client stretching column, so a shell
    /// states `grow` and nothing about the window's own extent.
    pub root: GroupId,
    /// The window's visibility, for a producer that should stop while nobody can see it.
    pub watch: Watch,
    /// The client area the window opened at, in DIPs.
    pub window_dips: Vector2,
}

impl Ui {
    /// Installs `palette` and fixes the root scope. The first call an application makes into
    /// this crate.
    ///
    /// # Panics
    ///
    /// If a different palette is already installed — see [`role::install`](crate::role::install).
    #[must_use]
    pub fn install(palette: &'static dyn Palette, accent: AccentId, density: Density) -> Self {
        crate::role::install(palette);
        Self {
            root_scope: Scope::root(accent, density),
        }
    }

    /// Returns the root scope fixed at [`install`](Self::install), for the numbers an
    /// application needs before its window exists.
    #[must_use]
    pub const fn root_scope(self) -> Scope {
        self.root_scope
    }

    /// Creates the window, starts the scene and app threads, and pumps until quit.
    ///
    /// `backends` runs on the scene thread once the window exists: a system compositor needs
    /// a dispatcher queue on the calling thread, and the scene thread has made its own.
    ///
    /// `mount` runs on the app thread once the scene exists. The [`Mount`] it returns is held
    /// there until this call returns — dropping one unmounts its tree.
    ///
    /// `on_resize`, `on_scale_changed`, `on_caption_hit` and `on_caption_state` are attached
    /// here, after `window` is configured, so those four handlers are the driver's.
    /// `on_message` is **chained**: a caller's own handler survives and answers first, and the
    /// tick and the doorbell see whatever it returned `None` for. Everything else about the
    /// window is the caller's.
    ///
    /// # Errors
    ///
    /// The window could not be created, a thread could not be started, the backends or the
    /// scene could not be brought up, or a thread failed. A failure on a worker thread closes
    /// the window, so the pump returns, and surfaces here after both workers are joined.
    pub fn run(
        self,
        window: WindowBuilder,
        backends: impl FnOnce() -> Result<Backends> + Send + 'static,
        backdrop: BackdropSpec,
        mount: impl FnOnce(AppCtx) -> Mount + Send + 'static,
    ) -> Result<()> {
        let bell = Rc::new(crate::input::Doorbell::new());
        // The client extent, in pixels, posted whenever the system changes it and taken by
        // the next tick, which forwards it in DIPs. Likewise a scale change, which carries
        // nothing: the tick re-reads the display and forwards what differs.
        let resized: Rc<Handoff<(i32, i32)>> = Rc::new(Handoff::new());
        let rescaled: Rc<Handoff<()>> = Rc::new(Handoff::new());
        // The tick, reachable from the window procedure. Empty until everything it needs
        // exists, which is after the window whose handler reaches it. That handler holds a
        // weak reference: the frame owns the window, and two strong ones would be a cycle
        // that never lets either go.
        let frame: Rc<RefCell<Option<Frame>>> = Rc::new(RefCell::new(None));
        // Where a failed tick lands. It has no call stack this side owns to return up.
        let failed: Rc<RefCell<Option<Error>>> = Rc::new(RefCell::new(None));

        let window = window
            // The tick, and the doorbell for every other message. `WM_FRAME` is answered
            // inside the window procedure rather than after the pump returns, so a
            // drag-resize keeps routing: the system's sizing loop pumps this message and
            // does not return until the contact lifts.
            //
            // Chained, so a caller's own handler survives being handed to this method and
            // answers first. Replacing it would discard it without a diagnostic.
            .chain_message({
                let bell = Rc::clone(&bell);
                let frame = Rc::downgrade(&frame);
                let failed = Rc::clone(&failed);
                move |_, message, wparam, lparam| {
                    if message != windows_window::WM_FRAME {
                        return bell.wndproc(message, wparam, lparam);
                    }
                    // A frame arriving while one is running is skipped rather than nested:
                    // `try_borrow_mut` fails, and the pacer's gate reopens so the next
                    // frame serves whatever this one missed.
                    if let Some(cell) = frame.upgrade()
                        && let Ok(mut slot) = cell.try_borrow_mut()
                        && let Some(frame) = slot.as_mut()
                        && let Err(error) = frame.tick()
                    {
                        *failed.borrow_mut() = Some(error);
                        windows_window::quit();
                    }
                    Some(0)
                }
            })
            .on_resize({
                let resized = Rc::clone(&resized);
                move |width, height| resized.post((width, height))
            })
            .on_scale_changed({
                let rescaled = Rc::clone(&rescaled);
                move |_| rescaled.post(())
            })
            .create()?;
        // Shared with the tick, which outlives every stack frame here.
        let window = Rc::new(window);

        let pacer = window.pacer()?;
        resized.arm(pacer.wake());
        rescaled.arm(pacer.wake());
        // Every query below answers for the window's current display, so a window closed
        // under start-up fails here rather than starting threads against invented numbers.
        let env = env_of(&window).ok_or_else(closed)?;
        let output = output_of(&window).ok_or_else(closed)?;
        let window_dips = client_dips(&window).ok_or_else(closed)?;
        let links = Arc::new(links::Links::new(window.handle())?);

        // The scene thread first: it builds the backends whose font ladder the app thread's
        // shaping engine is made over, and it publishes that ladder before it signals ready.
        let scene_thread = std::thread::Builder::new()
            .name("ui-scene".into())
            .spawn({
                let links = Arc::clone(&links);
                let start = scene::Start {
                    backends: Box::new(backends),
                    backdrop,
                    env,
                    output,
                    watch: window.watch()?,
                    scene_watch: window.watch()?,
                };
                move || scene::run(links, start)
            })
            .map_err(spawn_failed)?;
        links.scene_ready.wait(windows_window::clock::INFINITE);
        if let Some(error) = links.failure() {
            _ = scene_thread.join();
            return Err(error);
        }

        let app_thread = std::thread::Builder::new()
            .name("ui-app".into())
            .spawn({
                let links = Arc::clone(&links);
                let start = app::Start {
                    mount: Box::new(mount),
                    root_scope: self.root_scope,
                    env,
                    window_dips,
                    watch: window.watch()?,
                };
                move || app::run(links, start)
            })
            .map_err(spawn_failed)?;

        let router = crate::input::Router::new(&bell, &window, pacer.wake())?;

        // What is at a point in the caption band, answered from the array copy this thread
        // holds, so the drag strip is whatever the bar's controls leave over rather than a
        // second rect stated beside them. The result is discarded because a window with no
        // custom caption has no band to answer for.
        let _ = window.on_caption_hit({
            let frame = Rc::downgrade(&frame);
            move |x, y| {
                // Fallible: a tick holds the frame while it runs, and a re-entrant question
                // answers `Drag` rather than panicking in the window procedure.
                match frame.upgrade().as_ref().map(|cell| cell.try_borrow()) {
                    Some(Ok(slot)) => slot.as_ref().map_or(CaptionHit::Drag, |frame| {
                        crate::caption::hit(
                            &frame.hits,
                            frame.router.shadows(),
                            &frame.caption,
                            x,
                            y,
                        )
                    }),
                    _ => CaptionHit::Drag,
                }
            }
        });
        // Hover and press over a window command, which the router never sees: once the hit
        // test names one, its pointer stream is the system's. Recorded here and forwarded by
        // the tick, because this runs inside the window procedure.
        let nonclient: Rc<Handoff<CaptionState>> = Rc::new(Handoff::new());
        nonclient.arm(pacer.wake());
        let _ = window.on_caption_state({
            let nonclient = Rc::clone(&nonclient);
            move |state| nonclient.post(state)
        });

        *frame.borrow_mut() = Some(Frame {
            window: Rc::clone(&window),
            links: Arc::clone(&links),
            router,
            hits: windows_scene::HitTable::default(),
            picks: crate::present::Picks::default(),
            caption: crate::caption::Registry::default(),
            focus: Vec::new(),
            reports: Vec::new(),
            intents: Vec::new(),
            resized,
            rescaled,
            nonclient,
            out: Some(Box::default()),
            wake: pacer.wake(),
            holding: None,
            sent_env: Some(env),
            census: Census::default(),
            scene_wakes: 0,
            scene_applies: 0,
            app: AppCensus::default(),
            ticks: 0,
        });

        // Shown once the scene thread has applied the first patch: `ShowWindow` over an
        // empty tree shows a frame of whatever has not been painted yet. Bounded, so an
        // application whose first build never emits shows its empty window rather than
        // nothing at all.
        links.first_frame.wait(FIRST_FRAME_MS);
        window.show();

        // Parked, on `GetMessage`. Every wake is a message somebody posted: the pacer's
        // `WM_FRAME`, an input contact, a system question about the window, the scene
        // thread's nudge to take a batch.
        windows_window::run();

        // The app thread first: its tree comes down on its own thread and the destroys ride
        // one last batch. Then the scene thread, which applies that batch, releases every
        // region and stops the present thread before its compositor goes.
        links.stop_app.store(true, Ordering::Release);
        links.app_ring.ring();
        _ = app_thread.join();
        links.stop_scene.store(true, Ordering::Release);
        links.scene_ring.ring();
        _ = scene_thread.join();
        drop(frame);

        if let Some(error) = links.failure() {
            return Err(error);
        }
        match failed.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

/// How long the window waits for the first patch before showing anyway.
const FIRST_FRAME_MS: u32 = 5_000;

/// The error a closed window answers with, for the start-up queries that need it.
fn closed() -> Error {
    Error::new(E_HANDLE, "the window is closed")
}

fn spawn_failed(error: std::io::Error) -> Error {
    Error::new(E_HANDLE, error.to_string())
}

/// What one input tick settled on, and the latest each other thread reported about itself.
#[derive(Copy, Clone)]
pub struct Observed<'a> {
    /// The reports this tick produced, in order.
    pub reports: &'a [Report],
    /// The scene's tallies as of its last batch to the input thread.
    pub census: Census,
    /// How many times the scene thread has woken and how many patches it has applied.
    pub scene_wakes: u64,
    pub scene_applies: u64,
    /// The app thread's tallies as of its last batch.
    pub app: AppCensus,
    /// Input ticks so far, this one included.
    pub ticks: u64,
}

type Observer = Box<dyn FnMut(Observed<'_>)>;

thread_local! {
    /// What every input tick reports to, where a caller installed one.
    static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

/// Installs a function run at the end of every input tick, with what that tick saw.
///
/// The one optional process-wide install. It exists so that a census, a harness or a profile
/// reads the **real** tick rather than a copy of it. Installed on the thread that will own the
/// window, before [`Ui::run`].
///
/// The arguments are the tick's own buffers and are not held past the call, so an observer
/// that counts allocates nothing. Installing a second replaces the first.
pub fn observe(f: impl FnMut(Observed<'_>) + 'static) {
    OBSERVER.with(|slot| *slot.borrow_mut() = Some(Box::new(f)));
}

/// Reports one tick, where an observer is installed and is not already running.
pub(crate) fn observed(seen: Observed<'_>) {
    // Fallibly, so an observer reaching back into the tick is a dropped report rather than a
    // panic inside the window procedure.
    let _ = OBSERVER.try_with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(observer) = slot.as_mut()
        {
            observer(seen);
        }
    });
}

/// The client area in DIPs, which is the space every layout is stated in. `None` once the
/// window is closed.
fn client_dips(window: &Window) -> Option<Vector2> {
    let scale = window.scale()?;
    let (w, h) = window.client_size()?;
    Some(Vector2 {
        x: w as f32 / scale,
        y: h as f32 / scale,
    })
}

/// Returns the window's account of the display it is on: its DPI, and the output transform
/// for the display's colour capability. `None` once the window is closed.
///
/// Read per tick and never held: the window and its monitor own both, so a cached copy is
/// one a display hop leaves stale. The content peak comes from the installed palette rather
/// than a parameter, because it is a property of the authored table.
///
/// Both queries fail closed rather than substituting 96 DPI and `Sdr`. A window is the only
/// thing that answers for its display, so a default here is an invented measurement — every
/// DIP laid out against it and every colour transformed through it would be wrong in a way
/// nothing downstream can detect.
pub(crate) fn env_of(window: &Window) -> Option<Env> {
    Some(Env::new(window.metrics()?.dpi as f32, output_of(window)?))
}

/// Returns the transform that carries authored light to the display the window is on.
///
/// Split out because the present thread takes one directly: a region draws through the same
/// transform the retained side does, and reaching it from an [`Env`] would mean the number
/// travelling through a type that also carries a DPI the present thread has no use for.
pub(crate) fn output_of(window: &Window) -> Option<OutputTransform> {
    let cap = window.color_capability()?;
    Some(OutputTransform::for_display(
        cap,
        crate::role::content_peak_nits(&cap.gamut()),
    ))
}
