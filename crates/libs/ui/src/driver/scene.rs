//! The scene thread: owns the compositor and the retained tree, applies what the app thread
//! emits and moves what the input thread reports, and waits on nothing but its mailboxes.
//!
//! The thread has no clock. A patch is applied when it arrives, a report is turned into a
//! retarget when it arrives, and a compositor callback lands through this thread's own
//! message queue, which the wait below serves alongside the doorbell. Each pass ends a work
//! item, and ending the work item is the publish.

use super::links::{Guard, Links};
use crate::caption;
use crate::input::Report;
use crate::layout::{self, ScrollTable};
use crate::present::{self, Regions};
use crate::seam::{AppCensus, InputDown, RegionOp, RegionPick, ToScene, TrackerShadow, Up};
use crate::widget::{Controls, Front};
use core::sync::atomic::Ordering;
use std::os::windows::io::AsHandle;
use std::sync::Arc;
use windows_color::OutputTransform;
use windows_core::Result;
use windows_scene::{BackdropSpec, Backends, Env, Scene, SceneEvent};
use windows_window::qos::{self, Speed};
use windows_window::{Apartment, Pumped, WM_FRAME, Watch, ensure_dispatcher_queue, pump_until};

/// What the input thread hands the scene thread to start with.
pub(super) struct Start {
    pub backends: Box<dyn FnOnce() -> Result<Backends> + Send>,
    pub backdrop: BackdropSpec,
    pub env: Env,
    pub output: OutputTransform,
    /// For the present thread, which parks on it.
    pub watch: Watch,
    /// For this thread's own wait.
    pub scene_watch: Watch,
}

/// Runs the scene thread until told to stop. Never returns while the window is up.
pub(super) fn run(links: Arc<Links>, start: Start) -> Result<()> {
    let _guard = Guard {
        links: &links,
        name: "scene",
        release: &[&links.scene_ready, &links.first_frame],
    };
    let mut thread = match Thread::start(&links, start) {
        Ok(thread) => thread,
        Err(error) => {
            links.fail(&error);
            return Err(error);
        }
    };
    links.scene_ready.signal();
    if let Err(error) = thread.run() {
        links.fail(&error);
        return Err(error);
    }
    Ok(())
}

struct Thread<'a> {
    links: &'a Links,
    backends: Backends,
    scene: Scene,
    controls: Controls,
    regions: Regions,
    scrolls: ScrollTable,
    /// The window commands, as the app thread last published them, for the caption's
    /// hover and press.
    caption: caption::Registry,
    /// The regions the input thread can pick inside, kept in step with the region ops so
    /// the whole list can be sent when it changes.
    picks: Vec<RegionPick>,
    env: Env,
    watch: Watch,
    visible: bool,
    /// What the compositor reported since the last pass.
    events: Vec<SceneEvent>,
    /// The batch being filled for the app thread.
    up: Option<Box<Up>>,
    /// The batch being filled for the input thread.
    to_input: Option<Box<InputDown>>,
    /// The array epoch and tracker count last sent to the input thread.
    hits_epoch: u64,
    trackers_live: u32,
    shadows: Vec<(windows_scene::NodeId, Arc<core::sync::atomic::AtomicU64>)>,
    app: AppCensus,
    wakes: u64,
    applies: u64,
    first: bool,
    /// Whether a patch was applied since the input thread last heard the tallies, so an
    /// observer there reads counts that are current to the last apply and not to the last
    /// structural change.
    counted: bool,
}

impl<'a> Thread<'a> {
    fn start(links: &'a Links, start: Start) -> Result<Self> {
        // Before the compositor, which finds the queue on its thread and refuses a thread
        // without one. The window crate keeps it for the thread's life.
        ensure_dispatcher_queue(Apartment::Asta)?;
        let backends = (start.backends)()?;
        // For the app thread's shaping engine, before `scene_ready` releases the thread that
        // starts it.
        _ = links.ladder.set(backends.ladder().clone());
        let scene = Scene::new_at(links.hwnd, &backends, start.env, start.backdrop)?;
        // The present thread rings this thread for every binding it posts.
        present::install(
            windows_present::Tuning::default(),
            start.output,
            Some(start.watch),
            Arc::clone(&links.scene_ring),
        )?;
        Ok(Self {
            links,
            backends,
            scene,
            controls: Controls::new(),
            regions: Regions::default(),
            scrolls: ScrollTable::default(),
            caption: caption::Registry::default(),
            picks: Vec::new(),
            env: start.env,
            watch: start.scene_watch,
            visible: true,
            events: Vec::new(),
            up: Some(Box::default()),
            to_input: Some(Box::default()),
            hits_epoch: 0,
            trackers_live: 0,
            shadows: Vec::new(),
            app: AppCensus::default(),
            wakes: 0,
            applies: 0,
            first: false,
            counted: true,
        })
    }

    fn run(&mut self) -> Result<()> {
        loop {
            let stopping = self.links.stop_scene.load(Ordering::Acquire);
            let ring = &self.links.scene_ring;
            // Armed, then the flag is taken: a ring that landed during the previous pass is
            // seen here, and one that lands after the arm signals the event the wait holds.
            ring.arm();
            if !stopping && !ring.take_pending() {
                match pump_until(&[ring.event().as_handle(), self.watch.as_handle()]) {
                    Pumped::Signalled(0) | Pumped::Messages => {}
                    Pumped::Signalled(_) => self.visibility_edge(),
                    // A quit, a failed wait, or a result this crate does not know: none of
                    // them is a wake this loop can serve.
                    _ => {
                        ring.disarm();
                        break;
                    }
                }
            }
            ring.disarm();
            self.pass()?;
            if stopping {
                break;
            }
        }
        self.teardown()
    }

    /// Re-tags the thread on a visibility edge: full speed while anything it draws can be
    /// seen, the system's choice otherwise.
    fn visibility_edge(&mut self) {
        let visible = !self.watch.is_hidden();
        if visible != self.visible {
            self.visible = visible;
            qos::set(if visible { Speed::Full } else { Speed::Managed });
        }
    }

    /// One pass: everything that arrived, in the order the seams require.
    fn pass(&mut self) -> Result<()> {
        self.wakes += 1;

        // ⓪ what the compositor reported, before anything else reads it: the thumb reveal
        // below wants tracker phases; only observed scroll state needs them on the app thread.
        self.events.clear();
        self.scene.drain_events(&mut self.events);

        // ⓪′ what the present thread reported. Before the patch, so a region binds in the
        // pass its handle arrived in: the patch can unmount it, and binding a brush over a
        // handle that is already closing is what the ordering rules out.
        present::bind(&mut self.regions, &mut self.scene, &self.backends, self.env)?;

        if let Some(down) = self.links.down.take() {
            self.apply(down)?;
        }

        let inbound = self.links.to_scene.take();
        self.route(inbound.as_deref())?;
        if let Some(mut inbound) = inbound {
            inbound.clear();
            _ = self.links.to_scene_spare.put(inbound);
        }

        self.send_input();
        self.send_up();
        Ok(())
    }

    /// Applies one batch from the app thread and records what the input thread must learn
    /// from it.
    fn apply(&mut self, mut down: Box<crate::seam::Down>) -> Result<()> {
        self.applies += 1;
        self.scene
            .apply(&mut down.patch, &self.backends, self.env)?;

        let regions_changed = !down.regions.is_empty();
        for op in &down.regions {
            match op {
                RegionOp::Mount {
                    key,
                    control: Some(control),
                    live,
                    ..
                } => self.picks.push(RegionPick {
                    key: *key,
                    control: *control,
                    live: live.clone(),
                }),
                RegionOp::Drop { key } => self.picks.retain(|pick| pick.key != *key),
                RegionOp::Mount { control: None, .. } | RegionOp::Resize { .. } => {}
            }
        }
        // The row table before the presenter is told, and a sink cleared before its region
        // is unmounted: both orders live inside `apply`.
        present::apply(
            &mut self.regions,
            &mut down.regions,
            &mut self.scene,
            &self.backends,
            self.env,
        )?;
        {
            let mut front = Front {
                scene: &mut self.scene,
                back: &self.backends,
                env: self.env,
            };
            self.controls.adopt(&down.chrome, &mut front)?;
        }
        self.scrolls.apply_ops(&mut down.scrolls);
        if let Some(ids) = down.caption {
            self.caption = ids.into();
        }

        // What the input thread must learn from this patch, appended to the batch held for
        // it. The array is copied only when its epoch moved, so a patch that changed only
        // values ships no entries.
        if let Some(out) = self.to_input.as_mut() {
            let epoch = self.scene.hits().epoch();
            if epoch != self.hits_epoch {
                out.hits.copy_from(self.scene.hits());
                out.hits_changed = true;
                self.hits_epoch = epoch;
            }
            out.gestures.append(&mut down.gestures);
            out.released.append(&mut down.released);
            out.focus.append(&mut down.focus);
            if down.caption.is_some() {
                out.caption = down.caption;
            }
            if regions_changed {
                out.regions.clear();
                out.regions.extend(self.picks.iter().cloned());
                out.regions_changed = true;
            }
            let trackers_live = self.scene.census().trackers_live;
            if trackers_live != self.trackers_live {
                self.trackers_live = trackers_live;
                self.shadows.clear();
                self.scene.tracker_shadows(&mut self.shadows);
                out.trackers.clear();
                out.trackers
                    .extend(self.shadows.iter().map(|(viewport, shadow)| TrackerShadow {
                        viewport: *viewport,
                        shadow: Arc::clone(shadow),
                    }));
                out.trackers_changed = true;
            }
            out.census = *self.scene.census();
            out.scene_wakes = self.wakes;
            out.scene_applies = self.applies;
            out.app = down.census;
        }
        self.app = down.census;
        self.counted = false;

        down.clear();
        // The spare goes back, and the app thread is rung only if it skipped a flush for want
        // of one: a ring for a spare nobody was waiting for is a wake that finds nothing.
        _ = self.links.down_spare.put(down);
        if self.links.app_wants_down_spare.swap(false, Ordering::AcqRel) {
            self.links.app_ring.ring();
        }

        if !self.first {
            self.first = true;
            self.links.first_frame.signal();
        }
        Ok(())
    }

    /// Turns the input thread's reports into pixels and records what the app thread must
    /// learn from them.
    fn route(&mut self, inbound: Option<&ToScene>) -> Result<()> {
        let reports: &[Report] = inbound.map_or(&[], |inbound| &inbound.reports);
        let Some(up) = self.up.as_mut() else {
            return Ok(());
        };
        if let Some(inbound) = inbound {
            if let Some(window) = inbound.window {
                // A geometry change ends every exit snapshot: a ghost captured for one extent
                // would fade at the wrong place in the next.
                self.scene.cancel_exits();
                up.window = Some(window);
            }
            if let Some(env) = inbound.env {
                self.env = env;
                up.env = Some(env);
            }
        }
        let mut front = Front {
            scene: &mut self.scene,
            back: &self.backends,
            env: self.env,
        };
        // The window's own band before the client's hover: while a command holds the
        // pointer the client's hover is stale, and when it gives the pointer back the
        // client's reports below light whatever is underneath.
        if let Some(state) = inbound.and_then(|inbound| inbound.nonclient) {
            let (hover, pressed) = caption::controls(&self.caption, state);
            self.controls.nonclient(hover, pressed, &mut front)?;
        }
        // The pixels those reports move, and only then what the application is asked to do.
        // No intent causes a visual: by the time one exists, the visual has happened.
        self.controls.tick(reports, &mut front, &mut up.intents)?;
        // The thumb's reveal and a thumb being dragged, against the array the last patch
        // published, and against this pass's tracker phases.
        layout::scroll_front(&self.events, reports, &mut self.scrolls, &mut front)?;

        if let Some(inbound) = inbound {
            up.intents.extend_from_slice(&inbound.intents);
            // The per-sample reports stay here: the front table has consumed them, and an
            // intent per sample would put the app thread on the pointer's report rate.
            up.reports.extend(
                inbound
                    .reports
                    .iter()
                    .filter(|report| {
                        !matches!(
                            report,
                            Report::Moved { .. }
                                | Report::Dragged { .. }
                                | Report::Buttons { .. }
                                | Report::Wheel { .. }
                        )
                    })
                    .copied(),
            );
        }
        up.events.extend(
            self.events
                .drain(..)
                .filter(|event| self.scrolls.app_observes(event)),
        );
        Ok(())
    }

    /// Hands the input thread its batch if it carries anything and the spare is back, and
    /// asks the window for a tick to take it.
    fn send_input(&mut self) {
        let carries = self.to_input.as_ref().is_some_and(|out| {
            out.hits_changed
                || !out.gestures.is_empty()
                || !out.released.is_empty()
                || !out.focus.is_empty()
                || out.caption.is_some()
                || out.regions_changed
                || out.trackers_changed
        }) || !self.counted;
        if !carries {
            return;
        }
        let Some(spare) = self.links.input_down_spare.take() else {
            self.links
                .scene_wants_input_spare
                .store(true, Ordering::Release);
            return;
        };
        let Some(filled) = self.to_input.replace(spare) else {
            return;
        };
        match self.links.input_down.put(filled) {
            // A frame message, not a pacer request: the window's thread takes the batch on
            // its next tick, and one post is one tick whether or not the pacer is running.
            Ok(()) => {
                self.counted = true;
                self.links.hwnd.post(WM_FRAME, 0, 0);
            }
            Err(filled) => {
                if let Some(spare) = self.to_input.replace(filled) {
                    _ = self.links.input_down_spare.put(spare);
                }
                self.links
                    .scene_wants_input_spare
                    .store(true, Ordering::Release);
            }
        }
    }

    /// Hands the app thread its batch if it carries anything and the spare is back.
    fn send_up(&mut self) {
        let carries = self.up.as_ref().is_some_and(|up| {
            !up.events.is_empty()
                || !up.intents.is_empty()
                || !up.reports.is_empty()
                || up.window.is_some()
                || up.env.is_some()
        });
        if !carries {
            return;
        }
        let Some(spare) = self.links.up_spare.take() else {
            self.links
                .scene_wants_up_spare
                .store(true, Ordering::Release);
            return;
        };
        let Some(filled) = self.up.replace(spare) else {
            return;
        };
        match self.links.up.put(filled) {
            Ok(()) => self.links.app_ring.ring(),
            Err(filled) => {
                if let Some(spare) = self.up.replace(filled) {
                    _ = self.links.up_spare.put(spare);
                }
                self.links
                    .scene_wants_up_spare
                    .store(true, Ordering::Release);
            }
        }
    }

    /// Releases every region still mounted, then the present thread, then — by drop order —
    /// the scene and the backends, all on this thread.
    fn teardown(&mut self) -> Result<()> {
        let mut drops = Vec::new();
        self.regions.drops_into(&mut drops);
        present::apply(
            &mut self.regions,
            &mut drops,
            &mut self.scene,
            &self.backends,
            self.env,
        )?;
        present::uninstall();
        Ok(())
    }
}
