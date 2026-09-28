//! The two worker threads, over one loop.
//!
//! Each wakes on a doorbell, drains every inbox, does one pass and publishes; neither has a
//! clock. What differs is only how it parks and what one pass is, so the loop is written once
//! and each thread supplies those two bodies.
//!
//! * **app** — the signal graph, the tree and the overlay stack. It wakes on a write, a batch
//!   from the scene thread, or a stop, flushes what the writes implied, and emits one batch.
//!   Nothing here holds a compositor object, and nothing here can stall a gesture: by the time
//!   an intent reaches this thread the visual it describes has already happened.
//! * **scene** — the compositor and the retained tree. A patch is applied when it arrives, a
//!   report is turned into a retarget when it arrives, and a compositor callback lands through
//!   this thread's own message queue, which the wait below serves alongside the doorbell. Each
//!   changed pass requests a compositor commit before servicing the next wake.

use std::cell::Cell;
use std::ops::ControlFlow;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering::*;
use std::thread::JoinHandle;

use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{BackdropSpec, Backends, ControlId, Env, HitFlags, Scene, SceneEvent};
use windows_window::{Apartment, Pumped, WM_FRAME, Watch, qos};

use crate::build::{Host, Mount, Ui};
use crate::caption;
use crate::input::{KeyKind, Report};
use crate::layout::ScrollTable;
use crate::overlay::Overlays;
use crate::present;
use crate::role::Scope;
use crate::seam::{AppCensus, Down, FocusOp, InputDown, Ring, Row, SceneTally, ToScene, Up};
use crate::signal::{self, PostWake};
use crate::widget::{Controls, Front, Intent, ModelState, UiaRole, What};

use super::links::{Guard, Links};

/// The virtual keys this module reads, in the width [`KeyEvent::key`] carries them in.
///
/// [`KeyEvent::key`]: crate::input::KeyEvent::key
const ESCAPE: u16 = crate::VK_ESCAPE as u16;
const RETURN: u16 = crate::VK_RETURN as u16;
const SPACE: u16 = crate::VK_SPACE as u16;

/// What a worker thread does with one wake.
trait Worker {
    /// Parks until something this thread serves has arrived. `Break` leaves the loop.
    fn park(&mut self) -> ControlFlow<()>;
    /// Drains every inbox, does one pass, publishes.
    fn pass(&mut self) -> Result<()>;
    /// Services thread-affine callbacks after a pass, including under continuous input.
    fn dispatch(&mut self) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
    /// Brings the thread's own state down, on the thread that owns it.
    fn finish(self);
}

/// Runs a worker until the window's pump has returned.
///
/// Arming before the re-check is what makes a ring that lands between them wake the thread
/// rather than being lost, and taking the flag is what makes a ring that landed during the
/// previous pass seen without inspecting any inbox. Each worker drains once more before
/// leaving, so the last batch is applied rather than abandoned.
fn run<W: Worker>(links: &Links, bell: &Ring, mut worker: W) {
    loop {
        bell.arm();
        let rung = bell.take_pending();
        // Read before the park, so the pass below is the extra drain a stop is owed.
        let stopping = links.stop.load(Acquire);
        if !rung && !stopping && worker.park().is_break() {
            bell.disarm();
            break;
        }
        bell.disarm();
        if let Err(e) = worker.pass() {
            links.fail(&e);
            break;
        }
        if worker.dispatch().is_break() {
            break;
        }
        if stopping {
            break;
        }
    }
    worker.finish();
}

// ── the app thread ───────────────────────────────────────────────────────────────

struct App {
    links: Arc<Links>,
    /// Owns the root effects and every dynamically mounted branch beneath them.
    owner: Option<signal::Owner>,
    /// Held for the life of the mount: retiring it unmounts the tree.
    root: Option<Mount>,
    focus_outline: Option<windows_scene::NodeId>,
    preview_done: Option<u64>,
    overlays: Overlays,
    /// Focus edits the overlay stack emitted since the last batch went out.
    focus: Vec<FocusOp>,
    /// What controls did since the last batch went out, for automation to report. Empty while
    /// no client is listening.
    uia_intents: Vec<Intent>,
    /// A batch flushed into but not yet accepted by the scene thread's mailbox. The tree is
    /// not flushed again while one is held: a flush swaps the host's pending ops with the
    /// buffer it is given, so flushing into a full buffer would drop what it holds.
    held: Option<Box<Down>>,
    /// Whether a flush is owed because one could not be made: the host still holds those ops,
    /// and the wake that returns the spare brings no work of its own to ask for them.
    owed: bool,
    /// Set by the signal waker when a write on this thread gave the graph work, so the loop
    /// runs another pass rather than parking over it.
    woken: Rc<Cell<bool>>,
    /// Whether the first pass is still owed: the mount's own ops are pending before any write
    /// or batch has arrived to say so.
    first: bool,
    census: AppCensus,
    /// Routes a producer thread's staged write to this thread's doorbell for as long as this
    /// thread is the graph's owner.
    _posts: signal::PostGuard,
}

/// Starts the app thread, against the ladder the scene thread published.
///
/// # Errors
///
/// The thread could not be started.
pub(super) fn spawn_app<M>(
    links: &Arc<Links>,
    mount: M,
    env: Env,
    size: Vector2,
    root: Scope,
    visibility: Watch,
) -> Result<JoinHandle<()>>
where
    M: FnOnce(&mut Ui<'_>, super::AppCtx) + Send + 'static,
{
    let links = Arc::clone(links);
    std::thread::Builder::new()
        .name("ui-app".into())
        .spawn(move || {
            let guard = Guard {
                links: &links,
                promised: &[],
            };
            let start = (|| {
                // The shaping engine is this thread's, over the ladder the scene thread's
                // rasterizing engine holds: two engines over one ladder agree on every face id.
                let ladder = links
                    .ladder
                    .lock()
                    .ok()
                    .and_then(|held| held.clone())
                    .ok_or_else(super::closed)?;
                Host::install(env, root);
                // The mount solves against the real client extent, so its first publication
                // is the window's geometry rather than a zero box the next resize springs out of.
                Host::with(|h| h.set_window(size));
                // The shaping engine is the text table's, and the table is the host's: this is
                // the one seam where the ladder the other thread built is handed over.
                Host::with(|h| h.text.install(ladder))?;
                let woken = Rc::new(Cell::new(false));
                // A write made on this thread between passes marks the graph and returns; this
                // flag is what makes the loop run again rather than park over the marked work.
                signal::set_waker({
                    let woken = Rc::clone(&woken);
                    move || woken.set(true)
                });
                // A write from another thread is staged and rings the doorbell instead.
                let posts = signal::arm_posts(PostWake::Ring(Arc::clone(&links.app_bell)));
                let (owner, root) = signal::Owner::scope(|| {
                    Ui::mount_root(|ui| mount(ui, super::AppCtx { visibility, size }))
                });
                let focus_outline = Some(Host::with(Host::focus_outline));
                Ok(App {
                    links: Arc::clone(&links),
                    owner: Some(owner),
                    root: Some(root),
                    focus_outline,
                    preview_done: None,
                    overlays: Overlays::new(),
                    focus: Vec::new(),
                    uia_intents: Vec::new(),
                    held: None,
                    owed: false,
                    woken,
                    first: true,
                    census: AppCensus::default(),
                    _posts: posts,
                })
            })();
            match start {
                Ok(app) => run(&links, &links.app_bell, app),
                Err(e) => links.fail(&e),
            }
            drop(guard);
        })
        .map_err(|_| super::closed())
}

impl Worker for App {
    fn park(&mut self) -> ControlFlow<()> {
        // Pending work alone never spins the app loop: a write on this thread sets the flag
        // the pass takes, and a write from elsewhere rings. An owed flush is not a reason to
        // skip the wait — what it waits for is the spare coming back, and returning that
        // spare is itself a ring.
        if !self.woken.get() && !self.first {
            self.links.app_bell.wait();
        }
        ControlFlow::Continue(())
    }

    /// One pass: what arrived, what it implied, and one batch out.
    fn pass(&mut self) -> Result<()> {
        // Everything the writes since the last pass implied. Taken before the flush, so a
        // write the flush itself makes — a probe publishing — asks for another pass rather
        // than being folded into this one and forgotten.
        let mut work = self.first || self.woken.replace(false);
        // A provider that asked for a tree is work: nothing else on this thread has a reason
        // to run the walk that builds one.
        work |= self.links.uia_requested.load(Acquire);
        while let Some(mut up) = self.links.up.take() {
            work = true;
            self.absorb(&mut up);
            if self.links.up.give(up) {
                self.links.scene_bell.ring();
            }
        }
        // A wake that brought no work — a spare handed back for a flush nobody owes — solves
        // nothing and sends nothing.
        if !work && !self.owed && self.held.is_none() {
            return Ok(());
        }
        if work {
            self.first = false;
            reconcile(&mut self.overlays, &mut self.focus);
        }
        self.flush();
        Ok(())
    }

    /// The tree comes down on this thread, where the host lives, and its destroys ride one
    /// last batch so the scene thread releases every visual and region before it stops.
    fn finish(mut self) {
        _ = Host::try_with(|host| {
            self.overlays.retire(host);
            if let Some(root) = &mut self.root {
                root.retire(host);
            }
        });
        self.root = None;
        self.owner = None;
        self.owed = true;
        self.flush();
    }
}

impl App {
    /// Applies one batch from the scene thread.
    fn absorb(&mut self, up: &mut Up) {
        for update in &up.fields.updates {
            Host::with(|h| h.field_update(update));
        }
        deliver_field_commits(&up.fields.commits);
        // Geometry facts first, so the solve below runs on the extent and the display the
        // reports came from.
        Host::with(|h| {
            if let Some(size) = up.size {
                h.set_window(size);
            }
            if let Some(env) = up.env {
                // A scale change re-rasterizes every run: the glyphs were shaped at the old
                // one and a blit of them lands soft.
                let rescaled = env.scale() != h.env.scale();
                h.set_env(env);
                if rescaled {
                    h.reemit_text();
                }
            }
        });
        // What the compositor reported: the realization window a position moved, the overlay a
        // dwell opened, the runs a grid change invalidated.
        crate::layout::scroll_observe(&up.events);
        self.overlays.scene(&up.events, &mut self.focus);
        if up.events.iter().any(|e| {
            matches!(
                e,
                SceneEvent::ScaleChanged { .. } | SceneEvent::DeviceRebuilt
            )
        }) {
            Host::with(Host::reemit_text);
        }
        // The menu vocabulary first, because it appends to the intents: a row activated with
        // `Enter` reaches the handler a click reaches, through the one dispatch point.
        key_intents(
            &mut self.overlays,
            &up.reports,
            &mut self.focus,
            &mut up.intents,
        );
        // After the front table has consumed them, which it did on the scene thread before
        // they were forwarded: the press that opens an overlay here has already lit its button.
        Host::dispatch(&up.intents);
        if up.preview_done.is_some() { self.preview_done = up.preview_done.take(); }
        // A client is owed the number a drag settled on and the fact that an action completed,
        // and neither restales the tree. Collected only while one is listening, so a drag with
        // nothing attached copies nothing.
        if self.links.uia_listening.load(Acquire) {
            self.uia_intents.extend_from_slice(&up.intents);
        }
        // Overlay scopes turn Escape into their own report before it reaches this fallback, so
        // one press closes the overlay or the screen's inspector.
        for report in &up.reports {
            if matches!(report, Report::Key { event, .. }
                if event.kind == KeyKind::Down && event.key == ESCAPE)
                && let Some(handler) = Host::with(|h| h.escape_handler())
            {
                handler();
            }
        }
        self.overlays
            .settle(&up.reports, &up.intents, &mut self.focus);
        // After the dispatch: a menu option's handler lives in the very overlay the choice
        // closes, so closing it any earlier would dispose the control the intent names.
        self.overlays.after_dispatch(&mut self.focus);
        self.census.up_batches += 1;
    }

    /// Flushes the tree into a batch and hands it to the scene thread.
    ///
    /// A batch already held is retried first. With no buffer to flush into, the pass ends
    /// without a flush and the host keeps its pending ops for the next one; the scene thread
    /// rings when it returns the spare, and `owed` is what makes that ring flush rather than
    /// find no work. A flush that produced nothing hands the buffer straight back rather than
    /// crossing with it.
    fn flush(&mut self) {
        if let Some(held) = self.held.take() {
            match self.links.down.put(held) {
                Ok(()) => self.links.scene_bell.ring(),
                Err(held) => {
                    self.held = Some(held);
                    self.owed = true;
                    return;
                }
            }
        }
        let Some(mut down) = self.links.down.spare() else {
            self.census.skipped_flushes += 1;
            self.owed = true;
            return;
        };
        self.owed = false;
        // Outside the borrow: `Host::flush` takes the host itself, and taking it twice is the
        // re-entry the borrow panics on.
        Host::flush(&mut down.patch);
        down.focus_outline = self.focus_outline.take();
        down.preview_done = self.preview_done.take();
        Host::with(|h| {
            h.fill(&mut down);
            if self.links.uia_listening.load(Acquire)
                && (h.uia_stale() || self.links.uia_requested.swap(false, AcqRel))
            {
                h.uia_entries(&mut down.declared.uia);
                h.uia_published();
            }
        });
        down.declared.focus.append(&mut self.focus);
        down.declared.intents.append(&mut self.uia_intents);
        if down.is_empty() {
            _ = self.links.down.give(down);
            return;
        }
        self.census.flushes += 1;
        down.declared.census = self.census;
        match self.links.down.put(down) {
            Ok(()) => self.links.scene_bell.ring(),
            Err(down) => self.held = Some(down),
        }
    }
}

/// The declaration pass shared by the app thread and headless layout tests.
pub(super) fn reconcile(overlays: &mut Overlays, focus: &mut Vec<FocusOp>) {
    signal::flush();
    overlays.sync(focus);
}

/// Delivers a field's committed text to its handler, one revision at a time.
///
/// A revision already delivered is skipped, so a commit that crossed the seam twice edits the
/// document once. Effects are deferred by the signal graph, so this drains each callback's
/// source writes while its causal revision is still installed, before a later callback can
/// overwrite the source or inherit the wrong revision.
///
/// The handler is cloned out before it runs: running application code under the host's borrow
/// would re-enter it.
pub(super) fn deliver_field_commits(commits: &[crate::text_input::Commit]) {
    for commit in commits {
        let callback = Host::with(|h| {
            let at = h.control(commit.id)?.handlers;
            let row = h.fields.get_mut(commit.id)?;
            if row
                .delivered_revision
                .is_some_and(|revision| revision >= commit.revision)
            {
                return None;
            }
            row.delivered_revision = Some(commit.revision);
            row.callback_revision = Some(commit.revision);
            h.handlers.get(at)?.commit.clone()
        });
        if let Some(callback) = callback {
            callback(&commit.text);
            signal::flush();
        }
        Host::with(|h| {
            if let Some(row) = h.fields.get_mut(commit.id) {
                row.callback_revision = None;
            }
        });
    }
}

/// Turns `Enter` and `Space` on a focused control into the intent a tap produces.
///
/// The menu vocabulary answers first, because a row activated with `Enter` reaches the handler
/// a click reaches through the one dispatch point; only a key the overlays left unclaimed
/// falls through to the control under focus. A repeat, or either modifier, is not an
/// activation.
fn key_intents(
    overlays: &mut Overlays,
    reports: &[Report],
    focus: &mut Vec<FocusOp>,
    intents: &mut Vec<Intent>,
) {
    for report in reports {
        if let Report::Key {
            target: Some(target),
            event,
            ..
        } = *report
            && event.kind == KeyKind::Down
            && !event.mods.ctrl
            && !event.mods.alt
            && let Some(next) = Host::with(|h| h.choice_neighbor(target, event.key))
        {
            focus.push(FocusOp::Focus(Some(next)));
            intents.push(Intent::invoke_focused(next));
            continue;
        }
        let activating = matches!(report, Report::Key { event, .. }
            if event.kind == KeyKind::Down
                && matches!(event.key, RETURN | SPACE)
                && !event.repeat
                && !event.mods.ctrl
                && !event.mods.alt);
        let before = intents.len();
        overlays.keys(core::slice::from_ref(report), focus, intents);
        if activating
            && intents.len() == before
            && let Report::Key {
                target: Some(target),
                ..
            } = *report
            && Host::with(|h| {
                h.control(target).is_some_and(|row| {
                    // A control that answers a keyboard activation at all.
                    row.state != ModelState::Disabled
                        && matches!(
                            row.uia,
                            UiaRole::Button
                                | UiaRole::CheckBox
                                | UiaRole::RadioButton
                                | UiaRole::ComboBox
                                | UiaRole::TabItem
                        )
                })
            })
        {
            intents.push(Intent {
                target,
                what: What::Tapped,
            });
        }
    }
}

/// Whether a report is one the application hears about.
///
/// The per-sample reports stay on the scene thread: the front table has consumed them, and an
/// intent per sample would put the app thread on the pointer's report rate.
fn discrete(report: &Report) -> bool {
    !matches!(
        report,
        Report::Moved { .. }
            | Report::Dragged { .. }
            | Report::Buttons { .. }
            | Report::Wheel { .. }
            | Report::Redirect { .. }
    )
}

pub(super) fn apply_control_patch(controls: &mut Controls, down: &mut Down, front: &mut Front<'_>) -> Result<()> {
    if let Some(epoch) = down.preview_done { controls.finish_reorder(epoch, front)?; }
    front.scene.apply(&mut down.patch, front.back, front.env)?;
    if let Some(epoch) = down.preview_done.take() { front.scene.finish_drag_preview(epoch); }
    if let Some(outline) = down.focus_outline.take() {
        controls.set_ring(outline);
    }
    controls.adopt(
        &down.chrome,
        &down.values,
        &down.declared.released,
        front,
    )?;
    controls.adopt_reorders(&down.reorders, &down.declared.released);
    controls.adopt_translations(
        &down.translations, &down.declared.released, front,
    )?;
    controls.adopt_previews(&down.previews);
    controls.validate_reorder(front)?;
    Ok(())
}

// ── the scene thread ─────────────────────────────────────────────────────────────

struct SceneThread {
    links: Arc<Links>,
    scene: Scene,
    back: Backends,
    controls: Controls,
    regions: present::Regions,
    scrolls: ScrollTable,
    scope: Scope,
    /// The control a caret belongs to, so only a field's own geometry change reaches TSF.
    text_focused: Option<ControlId>,
    /// The window commands, as the app thread last published them, for the caption's hover and
    /// press.
    caption: [Option<ControlId>; 3],
    events: Vec<SceneEvent>,
    /// The batch being filled for the app thread.
    up: Box<Up>,
    /// The batch being filled for the input thread.
    down: Box<InputDown>,
    /// The array epoch and tracker count last sent to the input thread.
    sent: (u64, u32),
    env: Env,
    tally: SceneTally,
    /// Whether a patch was applied since the input thread last heard the tallies, so an
    /// observer there reads counts current to the last apply and not to the last structural
    /// change.
    applied: bool,
    hidden: bool,
    watch: Watch,
}

/// Starts the scene thread and waits for nothing: the caller waits on `scene_ready`.
///
/// `watch` is this thread's own and `present_watch` the present thread's. Each watcher holds
/// its own wake, and an auto-reset event is unicast, so one watch shared between two sleepers
/// would leave one of them asleep through the change that woke the other.
///
/// # Errors
///
/// The thread could not be started.
pub(super) fn spawn_scene<B>(
    links: &Arc<Links>,
    backends: B,
    backdrop: BackdropSpec,
    env: Env,
    scope: Scope,
    watch: Watch,
    present_watch: Watch,
) -> Result<JoinHandle<()>>
where
    B: FnOnce() -> Result<Backends> + Send + 'static,
{
    let links = Arc::clone(links);
    std::thread::Builder::new()
        .name("ui-scene".into())
        .spawn(move || {
            let guard = Guard {
                links: &links,
                promised: &[&links.scene_ready, &links.first_patch],
            };
            let start = (|| {
                // Before the compositor, which finds the queue on its thread and refuses a
                // thread without one. The window crate keeps it for the thread's life.
                windows_window::ensure_dispatcher_queue(Apartment::Asta)?;
                let back = backends()?;
                // For the app thread's shaping engine, before `scene_ready` releases the
                // thread that starts it.
                if let Ok(mut held) = links.ladder.lock() {
                    *held = Some(back.ladder().clone());
                }
                let scene = Scene::new_at(links.window, &back, env, backdrop)?;
                // The present thread rings this thread for every binding it posts, and the
                // registry it installs is this thread's local.
                present::install(env.output(), present_watch, Arc::clone(&links.scene_bell))?;
                Ok(SceneThread {
                    links: Arc::clone(&links),
                    scene,
                    back,
                    controls: Controls::new(),
                    regions: present::Regions::default(),
                    scrolls: ScrollTable::default(),
                    scope,
                    text_focused: None,
                    caption: [None; 3],
                    events: Vec::new(),
                    up: Box::default(),
                    down: Box::default(),
                    sent: (u64::MAX, u32::MAX),
                    env,
                    tally: SceneTally::default(),
                    applied: false,
                    hidden: false,
                    watch,
                })
            })();
            match start {
                Ok(scene) => {
                    links.scene_ready.signal();
                    run(&links, &links.scene_bell, scene);
                }
                Err(e) => links.fail(&e),
            }
            drop(guard);
        })
        .map_err(|_| super::closed())
}

impl Worker for SceneThread {
    fn dispatch(&mut self) -> ControlFlow<()> {
        if windows_window::pump() {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
    /// Waits on the doorbell, the window's visibility watch and this thread's own message
    /// queue, which is how a compositor callback reaches it. No timer.
    fn park(&mut self) -> ControlFlow<()> {
        use std::os::windows::io::AsHandle;
        let handles = [
            self.links.scene_bell.event().as_handle(),
            self.watch.as_handle(),
        ];
        match windows_window::pump_until(&handles) {
            Pumped::Signalled(0) | Pumped::Messages => ControlFlow::Continue(()),
            Pumped::Signalled(_) => {
                self.retag();
                ControlFlow::Continue(())
            }
            // A quit or a failed wait: neither is a wake this loop can serve.
            // `Pumped` is non-exhaustive; an outcome this loop does not know ends it as a failed
            // wait does.
            Pumped::Quit | Pumped::Failed | _ => ControlFlow::Break(()),
        }
    }

    /// One pass: everything that arrived, in the order the seams require.
    fn pass(&mut self) -> Result<()> {
        let before = *self.scene.census();
        // ⓪ what the compositor reported, before anything else reads it: the thumb reveal
        // below wants tracker phases, and only observed scroll state needs them on the app
        // thread.
        self.events.clear();
        self.scene.drain_events(&mut self.events);
        self.down.scroll_changed |= self.links.uia_listening.load(Acquire)
            && self
                .events
                .iter()
                .any(|e| matches!(e, SceneEvent::TrackerValues { .. }));
        // ⓪′ what the present thread reported. Before the patch, so a region binds in the pass
        // its handle arrived in: the patch can unmount it, and binding a brush over a handle
        // that is already closing is what the ordering rules out.
        let mut front = Front {
            scene: &mut self.scene,
            back: &self.back,
            env: self.env,
        };
        present::bind(&mut self.regions, &mut front)?;
        if let Some(mut down) = self.links.down.take() {
            self.apply(&mut down)?;
            if self.links.down.give(down) {
                self.links.app_bell.ring();
            }
        }
        if let Some(mut to) = self.links.to_scene.take() {
            self.route(&mut to)?;
            if self.links.to_scene.give(to) {
                self.links.window.post(WM_FRAME, 0, 0);
            }
        }
        self.regions.visibility(self.scene.hits());
        let scrolls = &self.scrolls;
        self.up
            .events
            .extend(self.events.drain(..).filter(|e| scrolls.app_observes(e)));
        self.tally.wakes += 1;
        self.tally.census = *self.scene.census();
        self.to_input();
        // The spare goes back, and the app thread is rung only if it skipped a flush for want
        // of one: a ring for a spare nobody was waiting for is a wake that finds nothing.
        if self.links.up.send(&mut self.up) {
            self.links.app_bell.ring();
        }
        if self.scene.census().changed_since(&before) {
            self.back.request_commit()?;
        }
        Ok(())
    }

    /// Drains the app thread's last batch, releases every region still mounted, then the
    /// present thread, then — by drop order — the scene and the backends, all on this thread.
    ///
    /// The extra drain is what makes one stop flag safe for both workers: the app thread's
    /// retirement rides a final batch, and this is where that batch is applied however the two
    /// loops interleaved on the way out.
    fn finish(mut self) {
        while let Some(mut down) = self.links.down.take() {
            _ = self.apply(&mut down);
        }
        let mut ops = Vec::new();
        self.regions.drops_into(&mut ops);
        let mut front = Front {
            scene: &mut self.scene,
            back: &self.back,
            env: self.env,
        };
        _ = present::apply(&mut self.regions, &mut ops, &mut front);
        present::uninstall();
    }
}

impl SceneThread {
    /// Re-tags the thread on a visibility edge: full speed while anything it draws can be
    /// seen, the system's choice otherwise.
    fn retag(&mut self) {
        let hidden = self.watch.is_hidden();
        if hidden != self.hidden {
            self.hidden = hidden;
            qos::set(if hidden {
                qos::Speed::Eco
            } else {
                qos::Speed::Full
            });
        }
    }

    /// Applies one batch from the app thread and records what the input thread must learn from
    /// it.
    fn apply(&mut self, down: &mut Down) -> Result<()> {
        // A theme transaction rebases every region's lexical scope, re-anchors the output
        // transform's content peak on the new palette, and re-seeds the backdrop. It runs
        // before the patch, because the patch's own colours were resolved under it.
        if let Some((scope, backdrop)) = down.theme.take() {
            self.scope = scope;
            let out = self.env.output();
            self.env = Env::new(
                self.env.dpi(),
                out.with_content_peak_nits(crate::role::content_peak_nits(&out.gamut(), scope)),
            );
            present::relight(self.env.output());
            self.scene.set_backdrop(backdrop, &self.back, self.env)?;
            self.regions.retheme(scope);
            self.down.scope = Some(scope);
        }
        let mut front = Front {
            scene: &mut self.scene,
            back: &self.back,
            env: self.env,
        };
        // The row table before the presenter is told, and a sink cleared before its region is
        // unmounted: both orders live inside `present::apply`.
        let regions_changed = !down.regions.is_empty();
        present::apply(&mut self.regions, &mut down.regions, &mut front)?;
        apply_control_patch(&mut self.controls, down, &mut front)?;
        self.scrolls.apply_ops(&mut down.scrolls);
        // A restated geometry replaces the map the thumb is bound through, so a container
        // holding an occlusion's extent is bound again from the extended one. Here, because the
        // patch that restated it has just been applied.
        self.scrolls.rebind_extents(&mut front)?;
        self.caption = down.declared.caption;
        self.tally.applies += 1;
        self.tally.applied_at = Some(std::time::Instant::now());
        self.applied = true;
        self.links.first_patch.signal();

        // What the input thread must learn from this patch, appended to the batch held for it.
        let out = &mut self.down;
        if regions_changed {
            self.regions.picks_into(&mut out.regions);
            out.regions_changed = true;
        }
        out.declared.absorb(&mut down.declared);
        out.fields.absorb(&mut down.fields);
        self.up.fields.commits.append(&mut down.fields.commits);
        Ok(())
    }

    /// Turns the input thread's reports into pixels and records what the app thread must learn
    /// from them.
    fn route(&mut self, to: &mut ToScene) -> Result<()> {
        if let Some(enabled) = to.springs_enabled {
            self.scene.set_springs_enabled(enabled);
        }
        self.up.size = to.size.or(self.up.size);
        // The ground's grain is the one layer with an extent, and this is where the extent
        // arrives. Cheap on every size but the ones that outgrow the allocation.
        if let Some(size) = to.size {
            self.scene.set_ground_extent(size, &self.back, self.env)?;
        }
        if let Some(env) = to
            .env
            .map(|e| {
                let out = e.output();
                Env::new(
                    e.dpi(),
                    out.with_content_peak_nits(crate::role::content_peak_nits(
                        &out.gamut(),
                        self.scope,
                    )),
                )
            })
            .filter(|e| *e != self.env)
        {
            self.env = env;
            // A geometry change ends every exit snapshot: a ghost captured for one extent
            // would fade at the wrong place in the next.
            self.scene.cancel_exits();
            present::relight(env.output());
            self.up.env = Some(env);
        }
        // A field's caret box is a function of the solved rect and the scroll offset under it,
        // so a tracker that moved while a field holds focus is a layout change TSF must hear.
        for report in &to.reports {
            if let Report::FocusChanged { to, .. } = report {
                self.text_focused = to.filter(|id| {
                    self.scene
                        .hits()
                        .entry(*id)
                        .is_some_and(|e| e.flags.contains(HitFlags::TEXT))
                });
            }
        }
        self.down.text_geometry_changed |= self.text_focused.is_some()
            && self
                .events
                .iter()
                .any(|e| matches!(e, SceneEvent::TrackerValues { .. }));
        let caption = self.caption;
        let mut front = Front {
            scene: &mut self.scene,
            back: &self.back,
            env: self.env,
        };
        if let Some(size) = to.size {
            self.controls.set_viewport(size, &mut front)?;
        }
        // The window's own band before the client's hover: while a command holds the pointer
        // the client's hover is stale, and when it gives the pointer back the client's reports
        // below light whatever is underneath.
        if let Some(state) = to.caption.take() {
            let (hover, press) = caption::controls(state, caption);
            self.controls.nonclient(hover, press, &mut front)?;
        }
        // The pixels those reports move, and only then what the application is asked to do. No
        // intent causes a visual: by the time one exists, the visual has happened.
        self.controls
            .tick(&to.reports, &mut front, &mut self.up.intents)?;
        if let Some(epoch) = self.controls.take_preview_release() {
            self.up.preview_done = Some(epoch);
        }
        // A container's own move, before the controls see the batch: nothing in the front table
        // knows what a viewport is, and the two never name the same control.
        for action in &to.automation {
            if let crate::uia::Action::ScrollTo(id, x, y) = *action {
                self.scrolls.scroll_to(id, Vector2 { x, y }, &mut front)?;
            }
        }
        self.controls
            .automation(&to.automation, &mut front, &mut self.up.intents)?;
        for &reveal in &to.reveals {
            self.scrolls.reveal_field(reveal, &mut front)?;
        }
        // The thumb's reveal and a thumb being dragged, against the array the last patch
        // published, and against this pass's tracker phases.
        crate::layout::scroll_front(&self.events, &to.reports, &mut self.scrolls, &mut front)?;
        self.up.fields.updates.append(&mut to.fields.updates);
        self.up.intents.append(&mut to.intents);
        self.up
            .reports
            .extend(to.reports.drain(..).filter(discrete));
        Ok(())
    }

    /// Hands the input thread its batch if it carries anything and the spare is back, and asks
    /// the window for a tick to take it.
    ///
    /// The array is copied only when its epoch moved, so a patch that changed only values
    /// ships no entries; the shadows only when the tracker count did.
    fn to_input(&mut self) {
        let shifted = self.controls.take_translation_changed();
        self.down.translation_changed |= shifted;
        self.down.text_geometry_changed |= shifted && self.text_focused.is_some();
        let epoch = self.scene.hits().epoch();
        if epoch != self.sent.0 {
            self.down.hits.copy_from(self.scene.hits());
            self.down.hits_changed = true;
            self.sent.0 = epoch;
        }
        let trackers = self.scene.census().trackers_live;
        if trackers != self.sent.1 {
            self.scene.tracker_shadows(&mut self.down.trackers);
            self.down.trackers_changed = true;
            self.sent.1 = trackers;
        }
        if self.applied {
            self.down.scene = self.tally;
            self.down.tallies = true;
            self.applied = false;
        }
        // A frame message, not a pacer request: the window's thread takes the batch on its
        // next tick, and one post is one tick whether or not the pacer is running.
        if self.links.input.send(&mut self.down) {
            self.links.window.post(WM_FRAME, 0, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyEvent, Mods};

    #[test]
    fn released_preview_ack_follows_callback_patch_and_survives_backpressure() {
        use crate::gesture::{DragDecl, DragUpdate, Phase};
        use crate::layout::Preset;
        use crate::widget::Gesturing;
        use windows_scene::{Op, Point};

        let _patch = crate::build::tests::fixture();
        let window = windows_window::Window::new("drag handoff").hidden().create().unwrap();
        let links = Arc::new(Links::new(&window).unwrap());
        let posts = signal::arm_posts(Arc::clone(&links.app_bell));
        let calls = Rc::new(Cell::new(0));
        let mut target = ControlId::NONE;
        let (owner, (mount, remove)) = signal::Owner::scope(|| {
            let shown = signal::Cell::new(true);
            let remove = signal::Cell::new(false);
            let mount = Ui::mount_root(|ui| {
                let calls = calls.clone();
                target = ui.node(Preset::Layer).name("Tile")
                    .on_drag(DragDecl::default(), move |event| {
                        if matches!(event, Gesturing::Committed(_)) {
                            calls.set(calls.get() + 1);
                            if remove.get() { shown.set(false); }
                        }
                    }).control_id();
                ui.when(shown, |ui| { ui.node(Preset::Layer); });
            });
            (mount, remove)
        });
        let woken = Rc::new(Cell::new(false));
        signal::set_waker({ let woken = woken.clone(); move || woken.set(true) });
        let mut app = App {
            links: links.clone(), owner: Some(owner), root: Some(mount), focus_outline: None,
            preview_done: None, overlays: Overlays::new(), focus: Vec::new(),
            uia_intents: Vec::new(), held: None, owed: false, woken, first: true,
            census: AppCensus::default(), _posts: posts,
        };
        app.pass().unwrap();
        links.down.give(links.down.take().expect("initial declaration"));
        app.pass().unwrap();
        assert!(links.down.take().is_none());

        for (epoch, edits) in [(1, false), (2, true)] {
            remove.set(edits);
            let spare = links.down.spare().expect("withhold the scene's spare");
            let mut up = links.up.spare().unwrap();
            up.preview_done = Some(epoch);
            up.intents.push(Intent { target, what: What::DragEnded(Some(DragUpdate {
                phase: Phase::Free, delta: Point { x: 24.0, y: 0.0 },
                from: Point::default(), at: Point { x: 24.0, y: 0.0 }, decided: false,
            })) });
            assert!(links.up.put(up).is_ok());
            app.pass().unwrap();
            assert_eq!(calls.get(), epoch);
            assert_eq!(app.preview_done, Some(epoch));
            assert!(app.owed);
            assert!(links.down.take().is_none());
            assert!(links.down.give(spare));
            app.pass().unwrap();
            let down = links.down.take().expect("no-op drops must also return an acknowledgement");
            assert_eq!(down.preview_done, Some(epoch));
            assert_eq!(down.patch.ops().iter().any(|op| matches!(op, Op::Drop { .. })), edits);
            assert_eq!(app.preview_done, None);
            links.down.give(down);
            let recycled = links.down.spare().unwrap();
            assert_eq!(recycled.preview_done, None);
            links.down.give(recycled);
            app.pass().unwrap();
            let flushes = app.census.flushes;
            app.pass().unwrap();
            assert_eq!(app.census.flushes, flushes);
            assert!(links.down.take().is_none());
        }
    }

    fn key(target: ControlId, key: u16, kind: KeyKind, repeat: bool, mods: Mods) -> Report {
        Report::Key {
            target: Some(target),
            event: KeyEvent {
                key,
                kind,
                repeat,
                mods,
            },
        }
    }

    /// `Enter` and `Space` activate a focused button once, and nothing else.
    ///
    /// A repeat, a key-up, a translated character and a modified press are not activations; a
    /// disabled control, a field and a scalar answer none of them; and an unmounted control's
    /// generation makes the lookup a miss rather than a write to whatever took its slot.
    #[test]
    fn button_keys_activate_once_without_claiming_field_or_scalar_input() {
        use crate::widget::{button, field, knob};
        let _patch = crate::build::tests::fixture();
        let (_owner, mounted) = signal::Owner::scope(|| {
            Ui::mount_root(|ui| {
                button(ui, "Action").key("button");
                button(ui, "Disabled").disabled(true).key("disabled");
                field(ui, "Draft").key("field");
                knob(ui, signal::Cell::new(0.5), crate::widget::Range::UNIT).key("scalar");
            })
        });
        Host::flush(&mut windows_scene::SinkPatch::default());
        let named = |name| {
            Host::with(|h| {
                h.controls
                    .iter()
                    .find(|(_, row)| row.key.as_deref() == Some(name))
                    .expect("the control was declared")
                    .0
            })
        };
        let target = named("button");
        let mut overlays = Overlays::new();
        let mut focus = Vec::new();
        let mut intents = Vec::new();
        for value in [RETURN, SPACE] {
            key_intents(
                &mut overlays,
                &[
                    key(target, value, KeyKind::Down, false, Mods::default()),
                    key(target, value, KeyKind::Down, true, Mods::default()),
                    key(target, value, KeyKind::Up, false, Mods::default()),
                    key(target, value, KeyKind::Char, false, Mods::default()),
                    key(
                        target,
                        value,
                        KeyKind::Down,
                        false,
                        Mods {
                            ctrl: true,
                            ..Mods::default()
                        },
                    ),
                ],
                &mut focus,
                &mut intents,
            );
        }
        assert_eq!(
            intents,
            [Intent {
                target,
                what: What::Tapped
            }; 2]
        );
        intents.clear();
        for target in [named("disabled"), named("field"), named("scalar")] {
            key_intents(
                &mut overlays,
                &[key(target, RETURN, KeyKind::Down, false, Mods::default())],
                &mut focus,
                &mut intents,
            );
        }
        assert!(intents.is_empty());
        drop(mounted);
        key_intents(
            &mut overlays,
            &[key(target, SPACE, KeyKind::Down, false, Mods::default())],
            &mut focus,
            &mut intents,
        );
        assert!(intents.is_empty());
    }

    /// A per-sample report stays on the scene thread and a discrete one goes up.
    #[test]
    fn only_discrete_reports_reach_the_application() {
        let id = ControlId::NONE;
        assert!(!discrete(&Report::Buttons {
            target: id,
            contact: 0,
            buttons: 1
        }));
        assert!(discrete(&Report::Released {
            target: id,
            contact: 0,
            at: windows_scene::Point { x: 0.0, y: 0.0 }
        }));
        assert!(discrete(&key(
            id,
            RETURN,
            KeyKind::Down,
            false,
            Mods::default()
        )));
    }
}
