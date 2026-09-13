//! The app thread: the signal graph, the model and the overlay stack, run on a thread with
//! no clock.
//!
//! It wakes on a write — a producer's `Cell::post`, a batch from the scene thread carrying
//! intents and events, a stop — flushes what the writes implied, and emits one batch to the
//! scene thread. Between wakes it is parked on its doorbell. Nothing here holds a compositor
//! object, and nothing here can stall a gesture: by the time an intent reaches this thread
//! the visual it describes has already happened.

use super::links::{Guard, Links};
use super::AppCtx;
use crate::build::{Host, Mount};
use crate::input::{KeyKind, Report};
use crate::layout;
use crate::overlay::Overlays;
use crate::role::Scope;
use crate::seam::{AppCensus, Down, FocusOp};
use crate::signal::{self, PostWake};
use crate::widget::Intent;
use core::sync::atomic::Ordering;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use windows_core::{Error, Result};
use windows_numerics::Vector2;
use windows_scene::{Env, Model, SceneEvent};
use windows_window::Watch;

/// What the input thread hands the app thread to start with.
pub(super) struct Start {
    pub mount: Box<dyn FnOnce(AppCtx) -> Mount + Send>,
    pub root_scope: Scope,
    pub env: Env,
    pub window_dips: Vector2,
    pub watch: Watch,
}

/// Runs the app thread until told to stop.
pub(super) fn run(links: Arc<Links>, start: Start) -> Result<()> {
    let _guard = Guard {
        links: &links,
        name: "app",
        release: &[],
    };
    let out = Thread::start(&links, start).and_then(|mut thread| thread.run());
    if let Err(error) = &out {
        links.fail(error);
    }
    out
}

struct Thread<'a> {
    links: &'a Links,
    /// Held for the life of the mount: dropping it unmounts the tree.
    mounted: Option<Mount>,
    overlays: Overlays,
    /// Set by the signal waker when a write on this thread gave the graph work, so the loop
    /// runs another pass rather than parking over it.
    dirty: Rc<Cell<bool>>,
    /// Focus edits the overlay stack emitted since the last batch went out.
    focus: Vec<FocusOp>,
    intents: Vec<Intent>,
    /// A batch flushed into but not yet accepted by the scene thread's mailbox. The model is
    /// not flushed again while one is held: a flush swaps the model's pending ops with the
    /// buffer it is given, so flushing into a full buffer would drop what it holds.
    held: Option<Box<Down>>,
    env: Env,
    census: AppCensus,
    /// Whether the first pass is still owed: the mount's own ops are pending in the model
    /// before any write or batch has arrived to say so.
    first: bool,
    _posts: signal::PostGuard,
}

impl<'a> Thread<'a> {
    fn start(links: &'a Links, start: Start) -> Result<Self> {
        // The shaping engine is this thread's, over the ladder the scene thread's rasterizing
        // engine holds: two engines over one ladder agree on every face id.
        let ladder = links
            .ladder
            .get()
            .cloned()
            .ok_or_else(|| Error::new(windows_window::E_HANDLE, "the scene thread never started"))?;
        crate::build::text::install(ladder)?;

        let mut model = Model::new(layout::root());
        model.set_window(start.window_dips);
        // Taken before the model is handed over: the host keeps it privately from here on,
        // and the root is what the application mounts under.
        let root = model.root();
        Host::install(model, start.env, start.root_scope);

        // A write made on this thread between passes marks the graph and returns; this flag
        // is what makes the loop run again rather than park over the marked work. A write
        // from another thread rings the doorbell instead, through the registration below.
        let dirty = Rc::new(Cell::new(false));
        signal::set_waker({
            let dirty = Rc::clone(&dirty);
            move || dirty.set(true)
        });
        let posts = signal::arm_posts(PostWake::Ring(Arc::clone(&links.app_ring)));

        let mounted = (start.mount)(AppCtx {
            root,
            watch: start.watch,
            window_dips: start.window_dips,
        });

        Ok(Self {
            links,
            mounted: Some(mounted),
            overlays: Overlays::new(),
            dirty,
            focus: Vec::new(),
            intents: Vec::new(),
            held: None,
            env: start.env,
            census: AppCensus::default(),
            first: true,
            _posts: posts,
        })
    }

    fn run(&mut self) -> Result<()> {
        loop {
            let ring = &self.links.app_ring;
            ring.arm();
            let stopping = self.links.stop_app.load(Ordering::Acquire);
            if !stopping && !ring.take_pending() && !self.dirty.get() {
                ring.wait();
            } else {
                ring.disarm();
            }
            self.pass();
            if stopping {
                break;
            }
        }
        // The tree comes down on this thread, where the host lives, and its destroys ride
        // one last batch so the scene thread releases every visual and region before it
        // stops.
        self.mounted = None;
        self.overlays = Overlays::new();
        self.emit(true);
        Ok(())
    }

    /// One pass: what arrived, what it implied, and one batch out.
    fn pass(&mut self) {
        let mut work = self.first;
        self.first = false;
        if let Some(mut up) = self.links.up.take() {
            self.inbound(&mut up);
            up.clear();
            _ = self.links.up_spare.put(up);
            // Rung only if the scene thread was holding a batch for want of this spare.
            if self.links.scene_wants_up_spare.swap(false, Ordering::AcqRel) {
                self.links.scene_ring.ring();
            }
            work = true;
        }
        // Everything the writes since the last pass implied. Cleared before the flush, so a
        // write the flush itself makes — a probe publishing — asks for another pass rather
        // than being folded into this one and forgotten.
        work |= self.dirty.replace(false);
        work |= signal::flush();
        self.overlays.sync(&mut self.focus);
        work |= !self.focus.is_empty();
        // A wake that brought no work — a spare handed back, a ring for a write the flush
        // found already applied — solves nothing and sends nothing.
        self.emit(work);
    }

    /// Applies one batch from the scene thread.
    fn inbound(&mut self, up: &mut crate::seam::Up) {
        // Geometry facts first, so the solve below runs on the extent and the display the
        // reports came from.
        if let Some(window) = up.window {
            Host::with(|h| h.set_window(window));
        }
        if let Some(env) = up.env {
            let rescaled = env.scale() != self.env.scale();
            self.env = env;
            Host::with(|h| {
                h.set_env(env);
                if rescaled {
                    h.reemit_text();
                }
            });
        }
        // What the compositor reported: the realization window a position moved, the overlay
        // a dwell opened, the runs a grid change invalidated.
        layout::scroll_observe(&up.events);
        self.overlays.scene(&up.events, &mut self.focus);
        if up.events.iter().any(|event| {
            matches!(
                event,
                SceneEvent::ScaleChanged { .. } | SceneEvent::DeviceRebuilt
            )
        }) {
            Host::with(Host::reemit_text);
        }

        // The menu vocabulary first, because it appends to the intents: a row activated with
        // `Enter` reaches the handler a click reaches, through the one dispatch point.
        self.intents.clear();
        self.intents.extend_from_slice(&up.intents);
        self.overlays
            .keys(&up.reports, &mut self.focus, &mut self.intents);
        Host::with(|h| h.dispatch(&self.intents));
        // Overlay scopes turn Escape into their own report before it reaches this fallback,
        // so one press closes the overlay or the screen's inspector.
        for report in &up.reports {
            if matches!(report, Report::Key { event, .. }
                if event.kind == KeyKind::Down && event.key as i32 == crate::VK_ESCAPE)
                && let Some(handler) = Host::with(|h| h.escape_handler())
            {
                handler();
            }
        }
        // After the front table has consumed them, which it did on the scene thread before
        // they were forwarded: the press that opens an overlay here has already lit its
        // button.
        self.overlays
            .service(&up.reports, &self.intents, &mut self.focus);
        // After the dispatch: a menu option's handler lives in the very overlay the choice
        // closes, so closing it any earlier would dispose the control the intent names.
        self.overlays.after_dispatch(&mut self.focus);
    }

    /// Flushes the model into a batch and hands it to the scene thread, when `work` says the
    /// pass gave it something to flush.
    ///
    /// A batch already held is retried first. With no buffer to flush into, the pass ends
    /// without a flush and the model keeps its pending ops for the next one; the scene
    /// thread rings when it returns the spare. A flush that produced nothing hands the buffer
    /// straight back rather than crossing with it.
    fn emit(&mut self, work: bool) {
        if let Some(held) = self.held.take() {
            match self.links.down.put(held) {
                Ok(()) => self.links.scene_ring.ring(),
                Err(held) => {
                    self.held = Some(held);
                    return;
                }
            }
        }
        if !work {
            return;
        }
        let Some(mut down) = self.links.down_spare.take() else {
            self.census.skipped_flushes += 1;
            // release: the flag follows this pass's reads of the mailbox, so the scene thread
            // returning the spare and then reading the flag sees it.
            self.links
                .app_wants_down_spare
                .store(true, Ordering::Release);
            return;
        };
        self.census.flushes += 1;
        Host::with(|h| {
            h.flush(&mut down.patch);
            h.fill(&mut down);
        });
        down.focus.append(&mut self.focus);
        down.census = self.census;
        if down.is_empty() {
            _ = self.links.down_spare.put(down);
            return;
        }
        match self.links.down.put(down) {
            Ok(()) => self.links.scene_ring.ring(),
            Err(down) => self.held = Some(down),
        }
    }
}
