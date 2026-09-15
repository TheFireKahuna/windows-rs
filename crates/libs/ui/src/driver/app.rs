//! The app thread: the signal graph, the model and the overlay stack, run on a thread with
//! no clock.
//!
//! It wakes on a write — a producer's `Cell::post`, a batch from the scene thread carrying
//! intents and events, a stop — flushes what the writes implied, and emits one batch to the
//! scene thread. Between wakes it is parked on its doorbell. Nothing here holds a compositor
//! object, and nothing here can stall a gesture: by the time an intent reaches this thread
//! the visual it describes has already happened.

use super::AppCtx;
use super::links::{Guard, Links};
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
    pub mount: Box<dyn FnOnce(&mut crate::build::Ui<'_>, AppCtx) + Send>,
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
    /// Owns the root effects and every dynamically mounted branch beneath them.
    owner: Option<signal::Owner>,
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
    /// Work retained in the host until a spare lets it cross to the scene thread.
    pending_flush: bool,
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
        let ladder = links.ladder.get().cloned().ok_or_else(|| {
            Error::new(windows_window::E_HANDLE, "the scene thread never started")
        })?;

        let mut model = Model::new(layout::root());
        model.set_window(start.window_dips);
        Host::install(model, start.env, start.root_scope);
        Host::install_text(ladder)?;

        // A write made on this thread between passes marks the graph and returns; this flag
        // is what makes the loop run again rather than park over the marked work. A write
        // from another thread rings the doorbell instead, through the registration below.
        let dirty = Rc::new(Cell::new(false));
        signal::set_waker({
            let dirty = Rc::clone(&dirty);
            move || dirty.set(true)
        });
        let posts = signal::arm_posts(PostWake::Ring(Arc::clone(&links.app_ring)));

        let (owner, mounted) = signal::Owner::scope(|| {
            crate::build::Ui::mount_root(|ui| {
                (start.mount)(
                    ui,
                    AppCtx {
                        watch: start.watch,
                        window_dips: start.window_dips,
                    },
                )
            })
        });

        Ok(Self {
            links,
            mounted: Some(mounted),
            owner: Some(owner),
            overlays: Overlays::new(),
            dirty,
            focus: Vec::new(),
            intents: Vec::new(),
            held: None,
            pending_flush: false,
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
            if !stopping && !self.first && !ring.take_pending() && !self.dirty.get() {
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
        Host::with(|host| {
            self.overlays.retire(host);
            if let Some(mount) = &mut self.mounted {
                mount.retire(host);
            }
        });
        self.overlays = Overlays::new();
        self.mounted = None;
        self.owner = None;
        self.emit(true);
        Ok(())
    }

    /// One pass: what arrived, what it implied, and one batch out.
    fn pass(&mut self) {
        let mut work = self.first || self.links.uia_requested.load(Ordering::Acquire);
        self.first = false;
        if let Some(mut up) = self.links.up.take() {
            self.inbound(&mut up);
            up.clear();
            _ = self.links.up_spare.put(up);
            // Rung only if the scene thread was holding a batch for want of this spare.
            if self
                .links
                .scene_wants_up_spare
                .swap(false, Ordering::AcqRel)
            {
                self.links.scene_ring.ring();
            }
            work = true;
        }
        // Everything the writes since the last pass implied. Cleared before the flush, so a
        // write the flush itself makes — a probe publishing — asks for another pass rather
        // than being folded into this one and forgotten.
        work |= self.dirty.replace(false);
        work |= reconcile(&mut self.overlays, &mut self.focus);
        // A wake that brought no work — a spare handed back, a ring for a write the flush
        // found already applied — solves nothing and sends nothing.
        self.emit(work);
    }

    /// Applies one batch from the scene thread.
    fn inbound(&mut self, up: &mut crate::seam::Up) {
        for update in &up.text {
            Host::with(|h| h.field_update(update));
        }
        deliver_field_commits(&up.field_commits);
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
        Host::dispatch(&self.intents);
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
        self.pending_flush |= work;
        if let Some(held) = self.held.take() {
            match self.links.down.put(held) {
                Ok(()) => self.links.scene_ring.ring(),
                Err(held) => {
                    self.held = Some(held);
                    return;
                }
            }
        }
        if !self.pending_flush {
            return;
        }
        let down = self.links.down_spare.take().or_else(|| {
            // Arm before rechecking: the scene may return the spare between the
            // first take and this store. Either this take gets it or its return
            // rings the app. Pending work alone never spins the app loop.
            self.links
                .app_wants_down_spare
                .store(true, Ordering::Release);
            self.links.down_spare.take()
        });
        let Some(mut down) = down else {
            self.census.skipped_flushes += 1;
            return;
        };
        self.links
            .app_wants_down_spare
            .store(false, Ordering::Release);
        self.pending_flush = false;
        self.census.flushes += 1;
        Host::flush(&mut down.patch);
        Host::with(|h| {
            h.fill(&mut down);
            if self.links.uia_listening.load(Ordering::Acquire)
                && (h.uia_stale() || self.links.uia_requested.swap(false, Ordering::AcqRel))
            {
                let seeds = down.seeds.get_or_insert_with(Default::default);
                h.uia_seeds(seeds);
                h.uia_published();
            }
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

/// The declaration pass shared by the app thread and headless layout tests.
pub(super) fn reconcile(overlays: &mut Overlays, focus: &mut Vec<FocusOp>) -> bool {
    let work = signal::flush();
    overlays.sync(focus);
    work | !focus.is_empty()
}

pub(super) fn deliver_field_commits(commits: &[crate::text_input::Commit]) {
    for commit in commits {
        let callback = Host::with(|h| {
            let row = h.fields.get_mut(commit.id)?;
            if row
                .delivered_revision
                .is_some_and(|revision| revision >= commit.revision)
            {
                return None;
            }
            row.delivered_revision = Some(commit.revision);
            row.callback_revision = Some(commit.revision);
            row.callback.clone()
        });
        if let Some(callback) = callback {
            callback(&commit.text);
            // Effects are deferred by the signal graph. Drain this callback's source
            // writes while its causal revision is still installed, before a later
            // callback can overwrite the source or inherit the wrong revision.
            signal::flush();
        }
        Host::with(|h| {
            if let Some(row) = h.fields.get_mut(commit.id) {
                row.callback_revision = None;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_returned_spare_flushes_pending_geometry_without_another_edit() {
        let _patch = crate::build::tests::fixture();
        let window = windows_window::Window::new("pending geometry witness")
            .hidden()
            .create()
            .expect("window");
        let links = Links::new(window.handle()).expect("mailboxes");
        let spare = links.down_spare.take().expect("initial spare");
        let mut thread = Thread {
            links: &links,
            mounted: None,
            owner: None,
            overlays: Overlays::new(),
            dirty: Rc::new(Cell::new(false)),
            focus: Vec::new(),
            intents: Vec::new(),
            held: None,
            pending_flush: false,
            env: Host::with(|h| h.env),
            census: AppCensus::default(),
            first: false,
            _posts: signal::arm_posts(PostWake::Ring(Arc::clone(&links.app_ring))),
        };
        let (owner, _) = signal::Owner::scope(|| {
            crate::build::Ui::mount_root(|ui| {
                ui.geometry(&[]);
            })
        });
        thread.emit(true);
        assert!(thread.pending_flush);
        assert!(links.app_wants_down_spare.load(Ordering::Acquire));
        assert!(links.down.take().is_none());

        assert!(links.down_spare.put(spare).is_ok());
        thread.emit(false); // The return brings no new model or signal work.
        let mut down = links.down.take().expect("the deferred geometry was sent");
        assert!(down.patch.ops().iter().any(|op| matches!(
            op,
            windows_scene::Op::Res {
                op: windows_scene::ResOp::Geom { .. },
                ..
            }
        )));
        assert!(!thread.pending_flush);
        assert!(!links.app_wants_down_spare.load(Ordering::Acquire));
        down.clear();
        assert!(links.down_spare.put(down).is_ok());
        thread.emit(false);
        assert!(links.down.take().is_none(), "idle emits nothing");
        drop(owner);
    }
    #[test]
    fn field_callback_effect_keeps_its_causal_revision_and_deduplicates() {
        let _patch = crate::build::tests::fixture();
        let window = windows_window::Window::new("field callback witness")
            .hidden()
            .create()
            .unwrap();
        let links = Links::new(window.handle()).unwrap();
        let mut thread = Thread {
            links: &links,
            mounted: None,
            owner: None,
            overlays: Overlays::new(),
            dirty: Rc::new(Cell::new(false)),
            focus: Vec::new(),
            intents: Vec::new(),
            held: None,
            pending_flush: false,
            env: Host::with(|h| h.env),
            census: AppCensus::default(),
            first: false,
            _posts: signal::arm_posts(PostWake::Ring(Arc::clone(&links.app_ring))),
        };
        let calls = Rc::new(Cell::new(0));
        let root = Host::with(|h| h.model().root());
        let (owner, mounted) = signal::Owner::scope({
            let calls = calls.clone();
            move || {
                let source = signal::Cell::new(String::new());
                crate::build::Ui::mount_at(root, None, crate::build::root_scope(), None, |ui| {
                    crate::widget::field(
                        ui,
                        crate::widget::TextSource::Dynamic(Box::new(move |out| {
                            source.with(|s| out.push_str(s))
                        })),
                    )
                    .on_commit(move |text| {
                        calls.set(calls.get() + 1);
                        source.set(text.to_uppercase());
                    });
                })
            }
        });
        signal::flush();
        let id = Host::with(|h| {
            let id = h.field_sources[0].id;
            h.field_sources.clear();
            h.fields.get_mut(id).unwrap().revision = 2;
            id
        });
        let mut up = crate::seam::Up::default();
        up.field_commits.push(crate::text_input::Commit {
            id,
            revision: 1,
            text: Arc::from("a"),
        });
        thread.inbound(&mut up);
        Host::with(|h| {
            let replacement = &h.field_sources[0];
            assert_eq!(
                replacement.based_on, 1,
                "deferred effects must not inherit revision 2"
            );
            assert_eq!(&*replacement.text, &[65]);
        });
        thread.inbound(&mut up);
        assert_eq!(calls.get(), 1);
        drop(mounted);
        thread.inbound(&mut up);
        assert_eq!(
            calls.get(),
            1,
            "unmounted generations cannot receive callbacks"
        );
        drop(owner);
    }
}
