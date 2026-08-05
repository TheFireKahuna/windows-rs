//! Start-up and the frame: the two things every application on this stack used to write
//! for itself, and the two it must not get wrong.
//!
//! # Why this is the framework's and not the application's
//!
//! A tick is twelve steps and **the order is a correctness rule at every seam**. Input runs
//! after the patch is applied, or a press resolves against the previous frame's geometry.
//! The front table runs before the overlay service, or a menu opens before the press that
//! opened it has lit its button. The scene's borrow is released before the application is
//! dispatched to, or a handler asking the caption what is under a point re-enters a borrow
//! it already holds. None of those is discoverable from a signature, and an application that
//! transcribes them inherits each one by transcription.
//!
//! Start-up is the same problem in a smaller space. Five things install, process-wide, in an
//! order where one of the constraints is not even about ordering: the shaping engine's font
//! ladder must be the **same instance** the rasterizing half holds, because two ladders agree
//! on face 0 and disagree on everything after it — and the symptom is a read-out drawn in the
//! wrong face rather than an error. [`Ui`] takes the [`Backends`] and reads the ladder off it,
//! so there is nothing to get wrong.
//!
//! # What stays the application's
//!
//! The compositor and the GPU. This crate declares into a retained tree and never builds one
//! — that is what keeps `windows-composition` and `windows-d2d` out of its dependencies —
//! so the application constructs the [`Backends`] and hands it over. Everything after that
//! point is here.
//!
//! # What happens per wake
//!
//! ```text
//! WM_FRAME ──▶ Scene::drain_events  what the trackers and the batches reported
//!          ──▶ scroll_observe       reported positions become signals            (app side)
//!          ──▶ Overlays::scene      a dwell that came due opens what it owed
//!          ──▶ signal::flush()      resolve memos, run effects
//!          ──▶ Host::flush(patch)   structure + solve + emit ops
//!          ──▶ Scene::apply(patch)  ops become composition writes                (front side)
//!          ──▶ Router::tick(hits)   input, against the array the patch just built
//!          ──▶ Overlays::keys       the menu vocabulary, into the same buffers
//!          ──▶ scroll_front         the thumb's reveal, and a thumb being dragged
//!          ──▶ Overlays::service    what this tick's reports mean for the stack
//! ```
//!
//! Nothing here polls. The pacer wakes on the compositor clock and only while something asked
//! for a frame, so an idle window costs no wakes at all — and a fling asks for its own frames,
//! because the queue a tracker callback pushes into holds the clock open until it is drained.

mod tick;

use crate::build::{Host, Mount};
use crate::role::{AccentId, Density, Palette, Scope};
use crate::signal;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tick::Frame;
use windows_color::{DisplayCapability, OutputTransform};
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{BackdropSpec, Backends, Env, GroupId, Model, Scene, SinkPatch, taffy};
use windows_window::{CaptionHit, CaptionState, Handoff, Tick, Window, WindowBuilder};

/// The process-wide installs, in the one order that is correct.
///
/// Constructed before anything resolves a role, which is the whole of its first job: this
/// crate panics rather than inventing a colour, so a palette that arrives late is a start-up
/// failure with a stack rather than a grey screen.
///
/// It hands back the root [`Scope`] because an application needs one before its window
/// exists — a custom caption's band is stated in row heights, and a row height is the
/// palette's answer at the root scope. That is the ordering this type exists to make
/// unrepresentable: you cannot ask for the scope without having installed the palette,
/// because the scope comes *from* the call that installs it.
#[derive(Copy, Clone)]
pub struct Ui {
    root_scope: Scope,
}

impl Ui {
    /// Installs `palette` and fixes the root scope.
    ///
    /// The first line of an application.
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

    /// The window's own scope, for the handful of numbers an application needs before its
    /// window exists.
    #[must_use]
    pub const fn root_scope(self) -> Scope {
        self.root_scope
    }

    /// Creates the window, brings up the retained tree, mounts, and pumps until quit.
    ///
    /// `backends` is called **after** the window exists, because both halves of it require
    /// what the window's creation set up: a system compositor needs a dispatcher queue on the
    /// calling thread, and the GPU is chosen for the display the window opened on.
    ///
    /// `mount` is called with the root group once everything it could reach exists, and the
    /// [`Mount`] it returns is held for the life of the process — dropping one unmounts its
    /// tree. It mounts into a full-client stretching column ([`layout::root`](crate::layout::root)),
    /// so a shell states `grow` and nothing about the window's own extent.
    ///
    /// The driver attaches its own `on_message`, `on_resize`, `on_caption_hit` and
    /// `on_caption_state` after `window` is configured, so an application cannot replace the
    /// tick by accident. Everything else about the window is the caller's.
    ///
    /// # Errors
    ///
    /// The window could not be created, the backends could not be built, the scene could not
    /// be brought up, or a tick failed — a tick runs inside the window procedure and has no
    /// call stack this side owns to return up, so its failure is recorded there, the loop is
    /// stopped, and it surfaces here.
    pub fn run(
        self,
        window: WindowBuilder,
        backends: impl FnOnce() -> Result<Backends>,
        backdrop: BackdropSpec,
        mount: impl FnOnce(GroupId) -> Mount,
    ) -> Result<()> {
        let bell = Rc::new(crate::input::Doorbell::new());
        // The client extent, in pixels, whenever the system changes it. Nothing else notices:
        // the layout's window and the scene's grid are both set from it, and a tick that never
        // learns it solves forever against the size the window opened at.
        let resized: Rc<Handoff<(i32, i32)>> = Rc::new(Handoff::new());
        // The tick, reachable from the window procedure. Empty until everything it needs
        // exists — which is after the window, whose own handler is what reaches it. The
        // handler holds a **weak** reference: the frame owns the window, and two strong ones
        // would be a cycle that never lets either go.
        let frame: Rc<RefCell<Option<Frame>>> = Rc::new(RefCell::new(None));
        // Where a failed tick lands. It has no call stack this side owns to return up.
        let failed: Rc<RefCell<Option<windows_core::Error>>> = Rc::new(RefCell::new(None));
        // The frame request the signal graph raises when a write gives it work to do.
        let pending: Rc<Cell<Option<Tick>>> = Rc::new(Cell::new(None));

        let window = window
            // The tick, and the doorbell for everything that is not one. `WM_FRAME` is
            // answered here rather than after the pump returns, which is the whole of why a
            // drag-resize keeps drawing: the system's sizing loop pumps this message and
            // never returns until the contact lifts.
            .on_message({
                let bell = Rc::clone(&bell);
                let frame = Rc::downgrade(&frame);
                let failed = Rc::clone(&failed);
                move |_, message, wparam, lparam| {
                    if message != windows_window::WM_FRAME {
                        return bell.wndproc(message, wparam, lparam);
                    }
                    // A frame arriving while one is running is **skipped, not nested**: the
                    // borrow is what makes that structural rather than a rule to remember,
                    // and the pacer's gate reopens so the next one serves whatever this
                    // missed.
                    if let Some(cell) = frame.upgrade()
                        && let Ok(mut slot) = cell.try_borrow_mut()
                        && let Some(frame) = slot.as_mut()
                        && let Err(error) = frame.tick()
                    {
                        // A tick that failed this way fails again next frame. Stop, rather
                        // than repeat it at the clock's rate with nobody to tell.
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
            .create()?;
        // Shared with the tick, which outlives every stack frame here.
        let window = Rc::new(window);

        let backends = backends()?;
        // The shaping engine is this thread's, and its ladder comes from the one the
        // rasterizing half already holds rather than from a second call to whatever built it.
        // Two ladders agree on face 0 and disagree on everything after it, and the symptom is
        // a run drawn in the wrong face.
        crate::build::text::install(backends.ladder().clone())?;

        let pacer = window.pacer()?;
        resized.arm(pacer.wake());
        // Shared, because the caption's hit test runs inside the window procedure and has to
        // reach the one hit array from there. The tick takes the borrow and drops it before
        // returning, so the two never overlap; `try_borrow` in the authority below makes even
        // a surprise re-entry answer `Drag` rather than panic.
        let scene = Rc::new(RefCell::new(Scene::new(
            &window,
            &backends,
            pacer.wake(),
            env_of(&window),
            backdrop,
        )?));
        let router = crate::input::Router::new(&bell, &window, pacer.wake())?;

        let mut model = Model::new(crate::layout::root());
        model.set_window(client_dips(&window));
        // Taken before the model is handed over: the host keeps it privately from here on,
        // and the root is what the application mounts under.
        let root = model.root();
        Host::install(model, env_of(&window), self.root_scope);

        // What is at a point in the band. Answered from the hit array the last mount built,
        // so the drag strip is whatever the bar's controls leave over rather than a second
        // rect stated beside them. Silently skipped where the window has no custom caption,
        // which is the case where there is no band to answer for.
        let _ = window.on_caption_hit({
            let scene = Rc::clone(&scene);
            move |x, y| {
                scene
                    .try_borrow()
                    .map_or(CaptionHit::Drag, |s| crate::caption::hit(s.hits(), x, y))
            }
        });
        // Hover and press over a window command, which the router never sees: once the hit
        // test names one, its pointer stream is the system's. Recorded here and applied in the
        // tick, because this runs inside the window procedure where nothing that draws is
        // reachable.
        let nonclient: Rc<Handoff<CaptionState>> = Rc::new(Handoff::new());
        nonclient.arm(pacer.wake());
        let _ = window.on_caption_state({
            let nonclient = Rc::clone(&nonclient);
            move |state| nonclient.post(state)
        });

        // Held for the life of the process: dropping it unmounts the tree.
        let _mounted = mount(root);

        // A write with nobody watching schedules nothing. `Cell::set` marks the graph and
        // returns; the pump is blocked, and the pacer is parked unless something has asked for
        // a frame. This is the ask, and the graph raises it on the edge only — once per burst
        // of writes, and never from inside a flush.
        signal::set_waker({
            let pending = Rc::clone(&pending);
            let wake = pacer.wake();
            move || pending.set(Some(wake.tick()))
        });

        *frame.borrow_mut() = Some(Frame {
            window: Rc::clone(&window),
            scene,
            backends,
            router,
            // Held for the life of the window rather than made per tick, which is what keeps
            // the input path allocation-free once the high-water mark is reached.
            controls: crate::widget::Controls::new(),
            overlays: crate::overlay::Overlays::new(),
            patch: SinkPatch::new(),
            events: Vec::new(),
            reports: Vec::new(),
            intents: Vec::new(),
            resized,
            nonclient,
            pending,
        });

        // Composed before the window is shown, not after. `ShowWindow` on an empty tree is a
        // flash of whatever has not been painted yet, and the ground exists precisely so the
        // first composited frame is not that. A direct call rather than a posted frame,
        // because there is no pump running yet to deliver one.
        if let Some(frame) = frame.borrow_mut().as_mut() {
            frame.tick()?;
        }
        window.show();

        // Parked, on `GetMessage`. Every wake is a message somebody posted: the pacer's
        // `WM_FRAME`, an input contact, a system question about the window. Nothing here
        // polls, and an idle window costs no wakes at all.
        windows_window::run();

        match failed.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

/// The client area in DIPs, which is the space every layout is stated in.
fn client_dips(window: &Window) -> Vector2 {
    let scale = window.scale().unwrap_or(1.0);
    let (w, h) = window.client_size().unwrap_or((0, 0));
    Vector2 {
        x: w as f32 / scale,
        y: h as f32 / scale,
    }
}

/// The window's own account of the display it is on.
///
/// Stated per tick and never held: the window and its monitor own the DPI and the colour
/// capability, and a cached copy is one a display hop leaves stale. The content peak comes
/// from the installed palette rather than from a parameter, because it is an authoring
/// decision about the palette's own speculars and there is nowhere else it could honestly
/// live.
pub(crate) fn env_of(window: &Window) -> Env {
    Env::new(
        window.metrics().map_or(96.0, |m| m.dpi as f32),
        OutputTransform::for_display(
            window.color_capability().unwrap_or(DisplayCapability::Sdr),
            crate::role::content_peak_nits(),
        ),
    )
}
