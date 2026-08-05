//! Start-up and the frame: the process-wide installs, the window, and the tick that runs
//! inside its window procedure.
//!
//! # Why the order lives here
//!
//! A tick is twelve steps and the order is a correctness rule at every seam. Input runs
//! after the patch is applied, or a press resolves against the previous frame's geometry.
//! The front table runs before the overlay service, or a menu opens before the press that
//! opened it has lit its button. The scene's borrow is released before the application is
//! dispatched to, or a handler asking the caption what is under a point re-enters a borrow
//! it already holds. None of that is visible in a signature.
//!
//! Start-up carries one constraint that is not about order: the shaping engine's font ladder
//! must be the same instance the rasterizing half holds, because two ladders agree on face 0
//! and disagree on everything after it, and the symptom is a read-out drawn in the wrong face
//! rather than an error. [`Ui::run`] reads the ladder off the [`Backends`] it is given.
//!
//! # What stays the application's
//!
//! The compositor and the GPU. This crate declares into a retained tree and never builds one,
//! which is what keeps `windows-composition` and `windows-d2d` out of its dependencies, so
//! the application constructs the [`Backends`] and hands it over.
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
//! Nothing here polls. The pacer wakes on the compositor clock, and only while something has
//! asked for a frame, so an idle window costs no wakes. A fling asks for its own frames: the
//! queue a tracker callback pushes into holds the clock open until it is drained.

mod tick;

use crate::build::{Host, Mount};
use crate::input::Report;
use crate::role::{AccentId, Density, Palette, Scope};
use crate::signal;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tick::Frame;
use windows_color::OutputTransform;
use windows_core::{Error, Result};
use windows_numerics::Vector2;
use windows_scene::{
    BackdropSpec, Backends, Census, Env, GroupId, Model, Scene, SceneEvent, SinkPatch,
};
use windows_window::{CaptionHit, CaptionState, E_HANDLE, Handoff, Tick, Window, WindowBuilder};

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

    /// Creates the window, brings up the retained tree, mounts, and pumps until quit.
    ///
    /// `backends` is called after the window exists, because both halves of it require
    /// what the window's creation set up: a system compositor needs a dispatcher queue on the
    /// calling thread, and the GPU is chosen for the display the window opened on.
    ///
    /// `mount` is called with the root group once everything it could reach exists, and the
    /// [`Mount`] it returns is held until this call returns — dropping one unmounts its tree.
    /// It mounts into a full-client stretching column
    /// ([`layout::root`](crate::layout::root)), so a shell states `grow` and nothing about
    /// the window's own extent.
    ///
    /// `on_resize`, `on_caption_hit` and `on_caption_state` are attached here, after `window`
    /// is configured, so those three handlers are the driver's. `on_message` is **chained**:
    /// a caller's own handler survives and answers first, and the tick and the doorbell see
    /// whatever it returned `None` for. Everything else about the window is the caller's.
    ///
    /// # Errors
    ///
    /// The window could not be created, the backends could not be built, the scene could not
    /// be brought up, or a tick failed. A tick runs inside the window procedure and has no
    /// call stack this side owns to return up, so its failure is recorded there, the loop is
    /// stopped, and the error surfaces here.
    pub fn run(
        self,
        window: WindowBuilder,
        backends: impl FnOnce() -> Result<Backends>,
        backdrop: BackdropSpec,
        mount: impl FnOnce(GroupId) -> Mount,
    ) -> Result<()> {
        let bell = Rc::new(crate::input::Doorbell::new());
        // The client extent, in pixels, posted whenever the system changes it and taken by
        // the next tick, which restates the layout's window from it. A tick that never
        // learns it solves forever against the size the window opened at.
        let resized: Rc<Handoff<(i32, i32)>> = Rc::new(Handoff::new());
        // The tick, reachable from the window procedure. Empty until everything it needs
        // exists, which is after the window whose handler reaches it. That handler holds a
        // weak reference: the frame owns the window, and two strong ones would be a cycle
        // that never lets either go.
        let frame: Rc<RefCell<Option<Frame>>> = Rc::new(RefCell::new(None));
        // Where a failed tick lands. It has no call stack this side owns to return up.
        let failed: Rc<RefCell<Option<Error>>> = Rc::new(RefCell::new(None));
        // The frame request the signal graph raises when a write gives it work to do.
        let pending: Rc<Cell<Option<Tick>>> = Rc::new(Cell::new(None));

        let window = window
            // The tick, and the doorbell for every other message. `WM_FRAME` is answered
            // inside the window procedure rather than after the pump returns, so a
            // drag-resize keeps drawing: the system's sizing loop pumps this message and
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
                        // A tick that failed this way fails again next frame, so the loop
                        // stops and the error is carried out of `run`.
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
        // The shaping engine is this thread's, and takes the ladder the rasterizing half
        // already holds rather than a second one built the same way. Two ladders agree on
        // face 0 and disagree on everything after it, and the symptom is a run drawn in the
        // wrong face.
        crate::build::text::install(backends.ladder().clone())?;

        let pacer = window.pacer()?;
        resized.arm(pacer.wake());
        // Shared, because the caption's hit test runs inside the window procedure and has to
        // reach the one hit array from there. The tick takes the borrow and drops it before
        // returning, so the two never overlap; the `try_borrow` in that handler answers
        // `Drag` rather than panicking on a re-entry.
        // Both queries answer for the window's current display, so a window closed under
        // start-up fails here rather than bringing the scene up against invented numbers.
        let env = env_of(&window).ok_or_else(closed)?;
        let scene = Rc::new(RefCell::new(Scene::new(
            &window,
            &backends,
            pacer.wake(),
            env,
            backdrop,
        )?));
        let router = crate::input::Router::new(&bell, &window, pacer.wake())?;

        let mut model = Model::new(crate::layout::root());
        model.set_window(client_dips(&window).ok_or_else(closed)?);
        // Taken before the model is handed over: the host keeps it privately from here on,
        // and the root is what the application mounts under.
        let root = model.root();
        Host::install(model, env, self.root_scope);

        // What is at a point in the caption band, answered from the hit array the last patch
        // built, so the drag strip is whatever the bar's controls leave over rather than a
        // second rect stated beside them. The result is discarded because a window with no
        // custom caption has no band to answer for.
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

        // `Cell::set` marks the graph and returns; nothing schedules a frame on its own,
        // since the pump is blocked and the pacer is parked unless something has asked for
        // one. This waker is that request, raised on the edge only — once per burst of
        // writes, and never from inside a flush.
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

        // Composed before the window is shown: `ShowWindow` on an empty tree shows a frame
        // of whatever has not been painted yet. Called directly rather than posted, because
        // no pump is running yet to deliver a frame.
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

/// The error a closed window answers with, for the two start-up queries that need it.
fn closed() -> Error {
    Error::new(E_HANDLE, "the window is closed")
}

type Observer = Box<dyn FnMut(&[SceneEvent], &[Report], Census)>;

thread_local! {
    /// What every tick reports to, where a caller installed one.
    static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

/// Installs a function run at the end of every tick, with what that tick saw.
///
/// The sixth process-wide install, and the only one that is optional. It exists so that a
/// census, a harness or a profile reads the **real** tick rather than a copy of it: the step
/// order in [`Frame::tick`] is a correctness rule at every seam, and a second loop written to
/// observe it drifts from the one that ships without either side failing.
///
/// The arguments are the tick's own buffers and are not held past the call, so an observer
/// that counts allocates nothing. Installing a second replaces the first.
pub fn observe(f: impl FnMut(&[SceneEvent], &[Report], Census) + 'static) {
    OBSERVER.with(|slot| *slot.borrow_mut() = Some(Box::new(f)));
}

/// Reports one tick, where an observer is installed and is not already running.
pub(crate) fn observed(events: &[SceneEvent], reports: &[Report], census: Census) {
    // Fallibly, so an observer reaching back into the tick is a dropped report rather than a
    // panic inside the window procedure.
    let _ = OBSERVER.try_with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(observer) = slot.as_mut()
        {
            observer(events, reports, census);
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
/// Built per tick and never held: the window and its monitor own both, so a cached copy is
/// one a display hop leaves stale. The content peak comes from the installed palette rather
/// than a parameter, because it is a property of the authored table.
///
/// Both queries fail closed rather than substituting 96 DPI and `Sdr`. A window is the only
/// thing that answers for its display, so a default here is an invented measurement — every
/// DIP laid out against it and every colour transformed through it would be wrong in a way
/// nothing downstream can detect.
pub(crate) fn env_of(window: &Window) -> Option<Env> {
    Some(Env::new(
        window.metrics()?.dpi as f32,
        OutputTransform::for_display(window.color_capability()?, crate::role::content_peak_nits()),
    ))
}
