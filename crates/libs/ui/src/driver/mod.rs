//! Start-up and the threads: the process-wide installs, the window, and the four threads a
//! running window is made of.
//!
//! ```text
//! input   the window's own thread: the pump, the doorbell, the router, the caption's answers.
//!         Resolves contacts against a copy of the hit array, moves nothing on screen, and
//!         hands its reports to the scene thread. Ticks on the frame clock only while a
//!         contact or an inertia is live.
//! scene   owns the compositor and the retained tree. Applies a patch when it arrives and
//!         turns a report into a retarget when it arrives; has no clock of its own.
//! app     owns the signal graph, the tree and the overlay stack; parks on a doorbell and
//!         wakes on a write. Has no clock.
//! present owns the presentation regions and draws them off the compositor clock; posts to
//!         nobody but the scene thread's binder.
//! ```
//!
//! Every crossing between them is a mailbox over a buffer allocated once, and every wait is on
//! a doorbell or the compositor's own clock. An idle window costs no wakes on any of them.
//!
//! # Why the order lives here
//!
//! Within each thread's pass the step order is a correctness rule at every seam, and each
//! ordering comment names what breaks if the line moves. Start-up carries one constraint that
//! is not about order: the shaping engine's font ladder must be the same instance the
//! rasterizing engine holds, because two ladders agree on face 0 and disagree on everything
//! after it. The scene thread builds the [`Backends`] and publishes that ladder before the app
//! thread starts.
//!
//! # What stays the application's
//!
//! The compositor and the GPU. This crate declares into a retained tree and never builds one,
//! which is what keeps `windows-composition` and `windows-d2d` out of its dependencies, so the
//! application constructs the [`Backends`] — on the scene thread, through the closure it hands
//! over.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering::*;

use windows_color::OutputTransform;
use windows_core::{Error, Result};
use windows_numerics::Vector2;
use windows_scene::{BackdropSpec, Backends, Census, Env};
use windows_window::{CaptionState, E_HANDLE, Handoff, WM_FRAME, Window, WindowBuilder};

use crate::input::Report;
use crate::role::{AccentId, Density, Palette, Scope};
use crate::seam::SceneTally;

mod links;
mod pass;
mod reentry;
#[cfg(feature = "test-support")]
pub mod testing;
mod tick;

pub use crate::seam::AppCensus;
use links::Links;
use reentry::Reentry;

/// The input pass, and the re-entrancy state the window procedure reads it through.
///
/// The tick is borrowed for the whole of a pass, and the system's nested pumps run inside one.
/// Everything the procedure has to answer while a pass is on the stack — a frame wake, a
/// pointer message, a removed key — is answered off [`Frame::phase`] and the doorbell instead,
/// so no borrow decides whether an event is recorded.
#[derive(Default)]
struct Frame {
    tick: RefCell<Option<tick::Tick>>,
    phase: Reentry,
}


impl Frame {
    /// Runs input passes until none is owed.
    ///
    /// A wake taken by a nested pump is recorded rather than dropped: the pass on the stack
    /// sees it and runs again before it returns. A dropped wake leaves the gate its sender
    /// closed latched shut, and the field that asked for it is never serviced.
    ///
    /// # Errors
    ///
    /// A pass failed. Whatever was still owed is abandoned with it.
    fn run(&self) -> Result<()> {
        self.phase.passes(|| match self.tick.borrow_mut().as_mut() {
            Some(tick) => tick.run(&self.phase),
            None => Ok(()),
        })
    }
}

/// The system's own questions this crate answers, by number. None of them has a constant in
/// either binding filter.
const WM_DESTROY: u32 = 0x0002;
const WM_MOVE: u32 = 0x0003;
const WM_SETTINGCHANGE: u32 = 0x001a;
const WM_GETOBJECT: u32 = 0x003d;

/// How long the window waits for the first patch before showing anyway, in milliseconds.
///
/// Bounded, so an application whose first build never emits shows its empty window rather than
/// nothing at all.
const FIRST_PATCH_MS: u32 = 5_000;

/// Window runtime configuration and the root [`Scope`] it owns.
#[derive(Copy, Clone)]
pub struct UiRuntime {
    root_scope: Scope,
}

/// What the application's tree builder is handed, on the app thread.
pub struct AppCtx {
    /// The window's visibility, for a producer that should stop while nobody can see it.
    pub visibility: windows_window::Watch,
    /// The client area the window opened at, in DIPs.
    pub size: Vector2,
}

impl UiRuntime {
    /// Starts an independent window with explicitly selected theme axes.
    #[must_use]
    pub const fn from_scope(root_scope: Scope) -> Self {
        Self { root_scope }
    }

    /// Selects the palette and default dark root scope for this window.
    #[must_use]
    pub fn new(palette: &'static dyn Palette, accent: AccentId, density: Density) -> Self {
        Self::from_scope(crate::role::install(palette, accent, density))
    }

    /// Returns the root scope fixed at [`new`](Self::new), for the numbers an application needs
    /// before its window exists.
    #[must_use]
    pub const fn root_scope(self) -> Scope {
        self.root_scope
    }

    /// Creates the window, starts the scene and app threads, and pumps until quit.
    ///
    /// `backends` runs on the scene thread once the window exists: a system compositor needs a
    /// dispatcher queue on the calling thread, and the scene thread has made its own.
    ///
    /// `mount` runs on the app thread with a borrowed authoring context once the scene exists.
    /// The runtime owns the declared content and retires it during shutdown.
    ///
    /// `on_resize`, `on_scale_changed`, `on_caption_hit` and `on_caption_state` are attached
    /// here, after `window` is configured, so those four handlers are the driver's.
    /// `on_message` is **chained**: a caller's own handler survives and answers first, and the
    /// tick and the doorbell see whatever it returned `None` for.
    ///
    /// # Errors
    ///
    /// The window could not be created, a thread could not be started, the backends or the
    /// scene could not be brought up, or a thread failed. A failure on a worker thread closes
    /// the window, so the pump returns, and surfaces here after both workers are joined.
    pub fn run<B, M>(
        self,
        window: WindowBuilder,
        backends: B,
        backdrop: BackdropSpec,
        mount: M,
    ) -> Result<()>
    where
        B: FnOnce() -> Result<Backends> + Send + 'static,
        M: FnOnce(&mut crate::build::Ui<'_>, AppCtx) + Send + 'static,
    {
        let _apartment = windows_window::initialize_sta()?;
        // The tick, reachable from the window procedure. Empty until everything it needs
        // exists, which is after the window whose handler reaches it. That handler holds a weak
        // reference: the tick owns the window, and two strong ones would be a cycle that never
        // lets either go.
        let frame: Rc<Frame> = Rc::default();
        // Built before the window, because the procedure answers through it from the first
        // message. What arrives before the router exists is recorded and consumed by the first
        // pass.
        let bell = Rc::new(crate::input::Doorbell::new());
        let settings: Rc<Cell<bool>> = Rc::default();
        // The one hit array, shared with the pass rather than owned by it: the caption's hit
        // test is answered from the window procedure while a pass is on the stack.
        let view = Rc::new(crate::input::HitView::default());
        // Posted whenever the system changes the client extent or the scale, and taken by the
        // next tick, which forwards what differs. A scale change carries nothing: the tick
        // re-reads the display. Hover and press over a window command land the same way,
        // because the handler runs inside the window procedure.
        let resized: Rc<Handoff<(i32, i32)>> = Rc::default();
        let rescaled: Rc<Handoff<()>> = Rc::default();
        let nonclient: Rc<Handoff<CaptionState>> = Rc::default();
        let uia = Rc::new(RefCell::new(crate::uia::Uia::new()));
        // Where a failed tick lands. It has no call stack this side owns to return up.
        let failed: Rc<RefCell<Option<Error>>> = Rc::default();

        let window = window
            // The tick. `WM_FRAME` is answered inside the window procedure rather than after the
            // pump returns, so a drag-resize keeps routing: the system's sizing loop pumps this
            // message and does not return until the contact lifts.
            //
            // Chained, so a caller's own handler survives being handed to this method and
            // answers first. Replacing it would discard it without a diagnostic.
            .chain_message({
                let (frame, failed) = (Rc::downgrade(&frame), Rc::clone(&failed));
                let (rescaled, settings) = (Rc::clone(&rescaled), Rc::clone(&settings));
                let uia = Rc::clone(&uia);
                move |_, msg, w, l| {
                    // A settings change and a move both re-read the display, and `WM_GETOBJECT`
                    // is a synchronous cross-process call.
                    match msg {
                        WM_SETTINGCHANGE => {
                            settings.set(true);
                            rescaled.post(());
                        }
                        WM_MOVE => rescaled.post(()),
                        WM_GETOBJECT => return uia.borrow_mut().get_frame_object(w, l),
                        _ => {}
                    }
                    if msg != WM_FRAME {
                        return None;
                    }
                    if let Err(e) = frame.upgrade()?.run() {
                        _ = failed.borrow_mut().get_or_insert(e);
                        windows_window::quit();
                    }
                    Some(0)
                }
            })
            // The content window takes every pointer and key message and is what automation
            // asks for its provider; the scene's target is bound to it, because a target on the
            // top-level window gets no input sink and its trackers never see a wheel or a
            // touchpad gesture.
            .content_window({
                let (uia, bell) = (Rc::clone(&uia), Rc::clone(&bell));
                move |_, msg, w, l| {
                    // `WM_GETOBJECT` is a synchronous cross-process call, and `WM_DESTROY`
                    // releases the provider while its handle is still valid.
                    match msg {
                        WM_GETOBJECT => return uia.borrow_mut().get_object(w, l),
                        WM_DESTROY => uia.borrow_mut().detach(),
                        _ => {}
                    }
                    // The doorbell answers without the tick, so a pointer or key message the
                    // system's nested pump dispatches while a pass is on the stack is recorded
                    // in the order it arrived rather than dropped for want of a borrow.
                    bell.wndproc(msg, w, l)
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
            .hidden()
            .create()?;
        // Shared with the tick, which outlives every stack frame here.
        let window = Rc::new(window);
        let content = window.content().ok_or_else(closed)?;

        uia.borrow_mut().attach(content.raw(), window.hwnd());
        let text = crate::text_input::TextInput::new(&window)?;
        // The one seam a harness drives a docked occlusion through, published where the window's
        // own text input is built so what it reaches is the production mailbox.
        #[cfg(feature = "test-support")]
        testing::publish_occluder(text.occluder());
        // Removed-key pretranslation also runs in the system's nested pumps. Drain earlier
        // discrete input before offering this key, so TSF sees the click or Tab's focus.
        // Release the tick before calling a TIP: it can synchronously enter our store.
        let key_filter = window.key_filter({
            let (held, failed, tsf) = (
                Rc::downgrade(&frame),
                Rc::clone(&failed),
                Rc::clone(&text.tsf),
            );
            move |message| {
                let Some(frame) = held.upgrade() else {
                    return false;
                };
                // The key is offered behind the input in front of it. Outside a pass that means
                // running one, so an earlier click or Tab has moved focus; inside a pass that
                // has already routed its input it means offering directly, which is the state
                // every nested pump a text service, the clipboard or automation opens runs in.
                //
                // The remaining case is a pump opened before this pass routed anything, which
                // no call-out of the pass can produce. It is answered rather than assumed away:
                // the key is declined, and the message left in the queue reaches the router
                // exactly once through `TranslateMessage` and the doorbell.
                if !frame.phase.may_offer() {
                    if frame.phase.running() {
                        return false;
                    }
                    if let Err(e) = frame.run() {
                        _ = failed.borrow_mut().get_or_insert(e);
                        windows_window::quit();
                        return true;
                    }
                }
                tsf.filter(message)
            }
        })?;
        let pacer = window.pacer()?;
        resized.arm(pacer.wake());
        rescaled.arm(pacer.wake());
        nonclient.arm(pacer.wake());

        // Every query below answers for the window's current display, so a window closed under
        // start-up fails here rather than starting threads against invented numbers.
        let env = env_of(&window, self.root_scope).ok_or_else(closed)?;
        let size = client_size(&window).ok_or_else(closed)?;
        let links = Arc::new(Links::new(&window)?);

        // The scene thread first: it builds the backends whose font ladder the app thread's
        // shaping engine is made over, and it publishes that ladder before it signals ready.
        //
        // Two watches, not one: each watcher holds its own wake and an auto-reset event is
        // unicast, so the scene thread's wait and the present thread's park cannot share one.
        let scene = pass::spawn_scene(
            &links,
            backends,
            backdrop,
            env,
            self.root_scope,
            window.watch()?,
            window.watch()?,
        )?;
        links.scene_ready.wait(windows_window::clock::INFINITE);
        if let Err(e) = links.failure() {
            _ = scene.join();
            return Err(e);
        }
        let app = pass::spawn_app(&links, mount, env, size, self.root_scope, window.watch()?)?;

        *frame.tick.borrow_mut() = Some(tick::Tick::new(
            &window,
            &links,
            &bell,
            &view,
            self.root_scope,
            tick::Handoffs {
                text,
                uia,
                resized,
                rescaled,
                nonclient: Rc::clone(&nonclient),
                settings,
                wake: pacer.wake(),
            },
        )?);

        // What is at a point in the caption band, answered from the array copy this thread
        // holds, so the drag strip is whatever the bar's controls leave over rather than a
        // second rect stated beside them. The result is discarded because a window with no
        // custom caption has no band to answer for.
        _ = window.on_caption_hit({
            let view = Rc::clone(&view);
            move |x, y| view.caption_hit(x, y)
        });
        // Hover and press over a window command, which the router never sees: once the hit test
        // names one, its pointer stream is the system's. Posted here and forwarded by the tick,
        // because this runs inside the window procedure.
        _ = window.on_caption_state(move |state| nonclient.post(state));

        // Shown once the scene thread has applied the first patch: `ShowWindow` over an empty
        // tree shows a frame of whatever has not been painted yet.
        links.first_patch.wait(FIRST_PATCH_MS);
        window.show();

        // Parked, on `GetMessage`. Every wake is a message somebody posted: the pacer's
        // `WM_FRAME`, an input contact, a system question about the window, the scene thread's
        // nudge to take a batch.
        windows_window::run();

        // The app thread first: its tree comes down on its own thread and the destroys ride one
        // last batch. The scene thread drains that batch as it leaves, releases every region
        // and stops the present thread before its compositor goes.
        links.stop.store(true, Release);
        links.app_bell.ring();
        _ = app.join();
        links.scene_bell.ring();
        _ = scene.join();
        drop(key_filter);
        drop(frame);

        links.failure()?;
        match failed.borrow_mut().take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// The error a closed window answers with, for the start-up queries that need it.
pub(crate) fn closed() -> Error {
    Error::new(E_HANDLE, "the window is closed")
}

/// What one input tick settled on, and the latest each other thread reported about itself.
pub struct Observed<'a> {
    /// The reports this tick produced, in order.
    pub reports: &'a [Report],
    /// The scene's tallies as of its last batch to the input thread.
    pub census: Census,
    pub scene_wakes: u64,
    pub scene_applies: u64,
    /// When the scene thread finished its last apply.
    ///
    /// The scene's own reading, carried down with its tallies, so the interval from an input to
    /// the frame it produced is measured at the end that produced it.
    pub scene_applied_at: Option<std::time::Instant>,
    /// The app thread's tallies as of its last batch.
    pub app: AppCensus,
    /// Input ticks so far, this one included.
    pub ticks: u64,
    /// The focused field's box in screen pixels, or `None` while no field holds focus.
    ///
    /// The number an occlusion has to clear, read where the editor publishes it rather than
    /// derived a second time from the array.
    pub focused_field: Option<crate::layout::Rect>,
    /// Shaped field geometries taken off the app thread's seam so far.
    ///
    /// One per field the app shaped, counted where the tick adopts them. A field the app left
    /// alone publishes none, so the difference across a keystroke is how many fields that
    /// keystroke reshaped.
    pub field_shapes: u64,
}

thread_local! {
    /// What every input tick reports to, where a caller installed one.
    static OBSERVER: RefCell<Option<Box<dyn FnMut(Observed<'_>)>>> = const { RefCell::new(None) };
}

/// Installs a function run at the end of every input tick, with what that tick saw.
///
/// The one optional process-wide install. It exists so that a census, a harness or a profile
/// reads the **real** tick rather than a copy of it. Installed on the thread that will own the
/// window, before [`UiRuntime::run`].
///
/// The arguments are the tick's own buffers and are not held past the call, so an observer that
/// counts allocates nothing. Installing a second replaces the first.
pub fn observe(f: impl FnMut(Observed<'_>) + 'static) {
    OBSERVER.with(|o| *o.borrow_mut() = Some(Box::new(f)));
}

/// Reports one tick, where an observer is installed and is not already running.
fn observed(seen: Observed<'_>) {
    // Fallibly, so an observer reaching back into the tick is a dropped report rather than a
    // panic inside the window procedure.
    _ = OBSERVER.try_with(|o| {
        if let Ok(mut o) = o.try_borrow_mut()
            && let Some(f) = o.as_mut()
        {
            f(seen);
        }
    });
}

/// Reports the scene and app tallies the tick last took off its inbox.
pub(crate) fn tallies(
    scene: SceneTally,
    app: AppCensus,
    reports: &[Report],
    ticks: u64,
    field_shapes: u64,
    focused_field: Option<crate::layout::Rect>,
) {
    observed(Observed {
        reports,
        census: scene.census,
        scene_wakes: scene.wakes,
        scene_applies: scene.applies,
        scene_applied_at: scene.applied_at,
        app,
        ticks,
        focused_field,
        field_shapes,
    });
}

/// The client area in DIPs, which is the space every layout is stated in. `None` once the
/// window is closed.
pub(crate) fn client_size(window: &Window) -> Option<Vector2> {
    let (w, h) = window.client_size()?;
    let scale = window.scale()?;
    Some(Vector2 {
        x: w as f32 / scale,
        y: h as f32 / scale,
    })
}

/// Returns the window's account of the display it is on: its DPI, and the output transform for
/// the display's colour capability. `None` once the window is closed.
///
/// Read per tick and never held: the window and its monitor own both, so a cached copy is one a
/// display hop leaves stale. The content peak comes from the installed palette rather than a
/// parameter, because it is a property of the authored table.
///
/// Both queries fail closed rather than substituting 96 DPI and `Sdr`. A window is the only
/// thing that answers for its display, so a default here is an invented measurement — every DIP
/// laid out against it and every colour transformed through it would be wrong in a way nothing
/// downstream can detect.
pub(crate) fn env_of(window: &Window, scope: Scope) -> Option<Env> {
    Some(Env::new(
        window.metrics()?.dpi as f32,
        output_of(window, scope)?,
    ))
}

/// Returns the transform that carries authored light to the display the window is on.
///
/// Split out because the present thread takes one directly: a region draws through the same
/// transform the retained side does, and reaching it from an [`Env`] would mean the number
/// travelling through a type that also carries a DPI the present thread has no use for.
pub(crate) fn output_of(window: &Window, scope: Scope) -> Option<OutputTransform> {
    let cap = window.color_capability()?;
    Some(OutputTransform::for_display(
        cap,
        crate::role::content_peak_nits(&cap.gamut(), scope),
    ))
}
