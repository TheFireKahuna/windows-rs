//! One frame: everything the writes since the last one implied, and nothing else.
//!
//! Every ordering comment in `tick` is a defect that was found rather than a preference. Read
//! them before moving a line.

use super::env_of;
use crate::build::Host;
use crate::caption;
use crate::input::{Report, Router};
use crate::layout;
use crate::overlay::Overlays;
use crate::signal;
use crate::widget::{Controls, Front, Intent};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{Backends, Scene, SceneEvent, SinkPatch};
use windows_window::{CaptionState, Handoff, Tick, Window};

/// Everything one tick needs, in one place the window procedure can reach.
///
/// A struct rather than a set of locals in [`Ui::run`](super::Ui::run) for one reason: **the
/// tick runs inside the window procedure.** That is what makes it survive a *nested* pump —
/// the system's own resize and move loops, the window menu, `Alt`+`Space` — each of which
/// runs a message loop of its own inside `DefWindowProc` and does not return until the
/// gesture ends. A tick sitting after `DispatchMessageW` in an outer loop is starved for the
/// whole of one, which is a window whose content freezes while its frame is being dragged. A
/// tick reached from [`WM_FRAME`](windows_window::WM_FRAME) is not.
///
/// A message handler is `'static`, so none of this can be a local borrowed by it.
pub(super) struct Frame {
    /// Held rather than borrowed, for the same reason: the handler outlives every stack frame
    /// in `run`. The window's own state holds a **weak** reference back to this, so the two do
    /// not keep each other alive.
    pub window: Rc<Window>,
    /// Shared with the caption's hit authority, which answers from the window procedure and
    /// may therefore ask while a tick is in progress. It takes the borrow fallibly for exactly
    /// that reason.
    pub scene: Rc<RefCell<Scene>>,
    pub backends: Backends,
    pub router: Router,
    pub controls: Controls,
    /// The open overlay stack. Held for the life of the window: it owns its slot roots, the
    /// subtrees under them and any dwell still counting down, and dropping it closes all of
    /// them.
    pub overlays: Overlays,
    pub patch: SinkPatch,
    /// What the front half reported since the last tick. Held for the life of the window, like
    /// the other three, so a fling's per-frame drain allocates nothing.
    pub events: Vec<SceneEvent>,
    pub reports: Vec<Report>,
    pub intents: Vec<Intent>,
    pub resized: Rc<Handoff<(i32, i32)>>,
    pub nonclient: Rc<Handoff<CaptionState>>,
    /// The frame the signal graph asked for, released by the tick that serves it.
    pub pending: Rc<Cell<Option<Tick>>>,
}

impl Frame {
    /// One frame: everything the writes since the last one implied, and nothing else.
    pub(super) fn tick(&mut self) -> Result<()> {
        // Released first, before any of the work. A write made *later* in this tick — an
        // intent handler's, at the very end — then asks for the next frame instead of being
        // folded into the request already being served and forgotten with it.
        self.pending.take();

        let env = env_of(&self.window);
        let mut scene = self.scene.borrow_mut();

        // Before the solve, because a solve that ran on the old extent would put the caption's
        // commands past the right edge for a frame.
        //
        // The layout's window is all this sets. The retained side has no half to keep in step:
        // its root is the composition target's, sized from the window by the compositor
        // itself, so the ground tracks a drag-resize whether or not this runs.
        if let Some((width, height)) = self.resized.take() {
            let scale = env.scale();
            Host::with(|h| {
                h.set_window(Vector2 {
                    x: width as f32 / scale,
                    y: height as f32 / scale,
                })
            });
        }

        // ⓪ what the front half reported. Before the flush, so everything it implies — the
        // realization window a position moved, the overlay a dwell opened, the runs a grid
        // change invalidated — lands in the tick it arrived in rather than the next.
        self.events.clear();
        scene.drain_events(&mut self.events);
        layout::scroll_observe(&self.events);
        self.overlays.scene(&self.events, self.router.focus_mut());
        // Neither of these moves a DIP, so the ordinary publish's width gate answers "nothing
        // moved" for exactly the case where every coverage tile is rasterized for a grid that
        // is gone.
        if self.events.iter().any(|event| {
            matches!(
                event,
                SceneEvent::ScaleChanged { .. } | SceneEvent::DeviceRebuilt
            )
        }) {
            Host::with(Host::reemit_text);
        }

        // ① everything the writes since the last tick implied, ② the structure and geometry
        // that fell out of it, ③ the composition writes that realize it.
        signal::flush();
        Host::with(|h| h.flush(&mut self.patch));
        scene.apply(&mut self.patch, &self.backends, env)?;
        self.patch.clear();

        // ④ what the mount minted, moved to the side that draws it. The rows carry resolved
        // numbers and ids and nothing else, which is what keeps the patch's `Send` intact.
        let (chrome, released, gestures) =
            Host::with(|h| (h.take_chrome(), h.take_released(), h.take_gestures()));
        for (target, decl) in gestures {
            self.router.declare(target, decl);
        }
        {
            let mut front = Front {
                scene: &mut scene,
                back: &self.backends,
                env,
            };
            self.controls.adopt(&chrome, &mut front)?;
            // The window's own band, before the router's hover test below: while a command
            // holds the pointer the client's hover is stale, and when it gives the pointer
            // back that test is what lights whatever is underneath.
            if let Some(state) = self.nonclient.take() {
                let (hover, pressed) = caption::controls(state);
                self.controls.nonclient(hover, pressed, &mut front)?;
            }
        }
        for target in released {
            self.controls.release(target);
            self.router.forget(target);
        }

        // ⑤ input, against the hit array the patch above just rebuilt. After the apply, so a
        // press never resolves against the previous frame's geometry.
        self.reports.clear();
        self.router.tick(scene.hits(), env, &mut self.reports)?;

        // ⑥ the pixels those reports move, and only then what the application is asked to do.
        // No intent may be the cause of a visual: by the time one exists, it has happened.
        self.intents.clear();
        // Before the front table, because this *appends* to both buffers: a menu row activated
        // with `Enter` reaches the handler a click reaches, through the one dispatch point
        // rather than a keyboard-shaped second one.
        self.overlays.keys(
            &mut self.reports,
            scene.hits(),
            self.router.focus_mut(),
            &mut self.intents,
        );
        let mut front = Front {
            scene: &mut scene,
            back: &self.backends,
            env,
        };
        self.controls
            .tick(&self.reports, &mut front, &mut self.intents)?;
        // The thumb's reveal and a thumb being dragged, against the array the patch above
        // published — so a grab resolves on this frame's geometry rather than the last's.
        layout::scroll_front(&self.events, &self.reports, &mut front)?;

        // After the front table, so by the time an overlay opens here the press that opened it
        // has already lit its button. Dwell starts and dismisses both resolve from the same
        // reports the wash did.
        self.overlays.service(
            &self.reports,
            &self.intents,
            scene.hits(),
            self.router.focus_mut(),
        );

        // The scene's borrow released before the application runs. A handler is free to do
        // anything a window's thread can do, including ask the caption what is under a point —
        // which reaches this same scene. `front` holds the borrow only to its last use above.
        drop(scene);
        Host::with(|h| h.dispatch(&self.intents));
        Ok(())
    }
}
