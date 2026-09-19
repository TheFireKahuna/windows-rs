//! The input thread's tick: everything the window's thread does per frame, and nothing that
//! draws.
//!
//! The tick runs inside the window procedure, reached from
//! [`WM_FRAME`](windows_window::WM_FRAME), so it keeps running through a nested pump: the
//! system's own resize and move loops, the window menu and `Alt`+`Space` each run a message
//! loop inside `DefWindowProc` and do not return until the gesture ends.
//!
//! It resolves contacts against a copy of the hit array the scene thread sent after its last
//! apply, moves nothing on screen, and hands the reports it produced to the scene thread, which
//! turns them into pixels. A region is the one exception: a pick inside one writes the region's
//! input state directly, because that state is an atomic the present thread reads.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::*};

use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{ControlId, Env, HitFlags, HitTable, NodeId, unpack_offset};
use windows_window::{CaptionHit, CaptionState, Handoff, Tick as Frame, Wake, Window};

use crate::caption;
use crate::input::{Doorbell, Report, Router, client_origin};
use crate::present::Picks;
use crate::role::Scope;
use crate::seam::{AppCensus, FocusOp, InputDown, Row, SceneTally, ToScene};
use crate::text_input::{Reveal, TextInput};
use crate::uia::{self, Uia};

use super::links::Links;

/// Everything one input tick needs, in one place the window procedure can reach.
///
/// A message handler is `'static`, so none of these can be a local in
/// [`UiRuntime::run`](super::UiRuntime::run).
pub(super) struct Tick {
    /// The window, held rather than borrowed: the handler outlives every stack frame in
    /// [`UiRuntime::run`](super::UiRuntime::run). The window's own state holds a weak reference
    /// back to this tick, so the two do not keep each other alive.
    window: Rc<Window>,
    links: Arc<Links>,
    bell: Rc<Doorbell>,
    router: Router,
    /// What the window procedure owns and posts to, because those handlers run inside it and
    /// this tick is what forwards them.
    from_pump: Handoffs,
    /// The hit array as the scene thread last published it. The caption's hit handler reads it
    /// from the window procedure through the same borrow, fallibly.
    hits: HitTable,
    /// Each viewport's live offset, as one atomic word the thread owning the trackers
    /// publishes. Installed on [`hits`](Self::hits) so a hit test resolves scroll without a hop,
    /// and read again where automation publishes the same offsets.
    trackers: Vec<(NodeId, Arc<AtomicU64>)>,
    /// The regions the pointer can be picked inside, with the part copy each is scanned against.
    picks: Picks,
    /// The window commands, as the scene thread last published them.
    caption: [Option<ControlId>; 3],
    /// Focus edits the app thread emitted, applied after the router's own tick so a keyboard
    /// move lands on the reports the front table is about to read.
    focus: Vec<FocusOp>,
    reports: Vec<Report>,
    /// The batch being filled for the scene thread.
    to_scene: Box<ToScene>,
    /// Held while a filled batch could not be handed over, so the pacer keeps ticking until it
    /// is: a batch that waited for the next contact would carry that contact's release to the
    /// scene thread late.
    holding: Option<Frame>,
    /// Set by the window procedure on `WM_SETTINGCHANGE`, so the caret and selection metrics
    /// are re-read once rather than per tick.
    settings_changed: bool,
    /// What this thread last told the scene thread the display was.
    sent: Option<Env>,
    /// What the other threads last reported about themselves, for the observer.
    scene: SceneTally,
    app: AppCensus,
    ticks: u64,
    scope: Scope,
    /// Drained per tick and never held, so a provider that asks for nothing allocates nothing.
    uia_actions: Vec<uia::Action>,
    text_actions: Vec<uia::action::TextAction>,
}

/// What the window procedure owns and the tick forwards.
///
/// A handler attached to the window runs inside the procedure, where the tick may already be
/// borrowed, so each of these posts rather than reaching in. The frame clock is here because
/// arming a handoff is what turns a post into a tick.
pub(super) struct Handoffs {
    pub(super) text: TextInput,
    pub(super) uia: Rc<RefCell<Uia>>,
    pub(super) resized: Rc<Handoff<(i32, i32)>>,
    pub(super) rescaled: Rc<Handoff<()>>,
    pub(super) nonclient: Rc<Handoff<CaptionState>>,
    pub(super) wake: Wake,
}

impl Tick {
    /// Builds the tick: the doorbell and the router, both on the window's own thread.
    ///
    /// # Errors
    ///
    /// The router's recogniser pool or the pacer's wake could not be created.
    pub(super) fn new(
        window: &Rc<Window>,
        links: &Arc<Links>,
        scope: Scope,
        from_pump: Handoffs,
    ) -> Result<Self> {
        let bell = Rc::new(Doorbell::new());
        let router = Router::new(&bell, window, from_pump.wake.clone())?;
        Ok(Self {
            window: Rc::clone(window),
            links: Arc::clone(links),
            bell,
            router,
            from_pump,
            hits: HitTable::default(),
            trackers: Vec::new(),
            picks: Picks::default(),
            caption: [None; 3],
            focus: Vec::new(),
            reports: Vec::new(),
            to_scene: Box::default(),
            holding: None,
            settings_changed: false,
            sent: None,
            scene: SceneTally::default(),
            app: AppCensus::default(),
            ticks: 0,
            scope,
            uia_actions: Vec::new(),
            text_actions: Vec::new(),
        })
    }

    /// Answers every message but the frame: the settings latch, then the pointer doorbell.
    pub(super) fn message(&mut self, msg: u32, w: usize, l: isize) -> Option<isize> {
        if msg == super::WM_SETTINGCHANGE {
            self.settings_changed = true;
        }
        self.bell.wndproc(msg, w, l)
    }

    /// Resolves a point in the caption band against the one hit array.
    pub(super) fn caption_hit(&self, x: f32, y: f32) -> CaptionHit {
        caption::hit(x, y, &self.hits, self.caption)
    }

    /// Runs one input tick.
    ///
    /// # Errors
    ///
    /// The router's own tick, a text action or the caret publication failed.
    pub(super) fn run(&mut self) -> Result<()> {
        self.ticks += 1;
        // ⓪ what the scene thread published since the last tick: the array a contact resolves
        // against, and the rows the router and the pick table are declared from. Taken before
        // the router runs, so a press lands on the geometry that is on screen rather than the
        // geometry that was.
        while let Some(mut down) = self.links.input.take() {
            self.absorb(&mut down);
            // The spare goes back, and the scene thread is rung only if it was holding a batch
            // for want of one.
            if self.links.input.give(down) {
                self.links.scene_bell.ring();
            }
        }
        // A window closed under a queued frame answers for no display. The tick ends rather
        // than routing against an invented DPI.
        let Some(env) = super::env_of(&self.window, self.scope) else {
            return Ok(());
        };
        // ① input, against the array above.
        self.reports.clear();
        self.router.tick(&self.hits, env, &mut self.reports)?;
        // ② the focus edits the app thread emitted, into the same report list: a keyboard move
        // and a pointer move reach the front table the same way.
        self.apply_focus();
        // A control that stopped being interactive under the focus it holds keeps the ring
        // pointing at something no contact can reach, so focus is dropped rather than stranded.
        if self.router.focus_mut().current().is_some_and(|id| {
            self.hits
                .entry(id)
                .is_none_or(|e| !e.flags.contains(HitFlags::INTERACTIVE))
        }) {
            self.focus.push(FocusOp::Focus(None));
            self.apply_focus();
        }
        // The caret's box, in the same space the array is scanned in, and whatever the editor
        // has to say about it. Both are the input thread's: TSF is pump-bound.
        let text = &mut self.from_pump.text;
        text.geometry(&self.hits, env.scale());
        text.reports(&mut self.reports, &self.hits, env.scale())?;
        text.flush(&mut self.to_scene.fields.updates);
        if (text.touch.take()
            || self
                .reports
                .iter()
                .any(|r| matches!(r, Report::FocusChanged { .. })))
            && let Some(id) = text.focused()
        {
            let occlusion = text.touch.docked;
            self.to_scene.reveals.push(Reveal { id, occlusion });
        }
        if core::mem::take(&mut self.settings_changed) {
            self.from_pump.text.settings_changed();
        }
        // A provider snapshot can precede disable, hide or unmount, so what one asked for is
        // executed against the adopted array's eligibility, just like physical input.
        self.automation();
        // ③ a contact inside a region writes that region's input and bumps its epoch here, on
        // this thread: the present thread reads both, and no other thread is in the way.
        crate::present::pick(
            &self.reports,
            &self.hits,
            &mut self.picks,
            &mut self.to_scene.intents,
        );
        // ④ what the scene thread turns into pixels, and the window facts that arrived with it.
        // Appended to the batch this thread holds; handed over only when the spare is back,
        // otherwise carried to the next tick in the order it happened.
        // Copied rather than moved: the observer below is handed what the whole tick settled
        // on, and the next tick clears this buffer before it routes anything.
        self.to_scene.reports.extend_from_slice(&self.reports);
        if let Some(state) = self.from_pump.nonclient.take() {
            self.to_scene.caption = Some(state);
        }
        if let Some((w, h)) = self.from_pump.resized.take() {
            let scale = env.scale();
            self.to_scene.size = Some(Vector2 {
                x: w as f32 / scale,
                y: h as f32 / scale,
            });
        }
        if self.from_pump.rescaled.take().is_some() || self.sent != Some(env) {
            self.to_scene.env = Some(env);
            self.sent = Some(env);
        }
        if self.links.to_scene.send(&mut self.to_scene) {
            self.links.scene_bell.ring();
        }
        self.holding = match self.to_scene.is_empty() {
            true => None,
            false => Some(self.from_pump.wake.tick()),
        };
        self.publish_automation(env);
        // Last, so what an observer is handed is what the whole tick settled on.
        super::tallies(self.scene, self.app, &self.reports, self.ticks);
        Ok(())
    }

    /// Applies the focus edits held, raising one report where the ring moved.
    fn apply_focus(&mut self) {
        if self.focus.is_empty() {
            return;
        }
        let from = self.router.focus_mut().current();
        if self.router.focus_mut().apply(&self.focus, &self.hits) {
            let to = self.router.focus_mut().current();
            self.reports.push(Report::FocusChanged { from, to });
        }
        self.focus.clear();
    }

    /// Applies one batch from the scene thread.
    fn absorb(&mut self, down: &mut InputDown) {
        if let Some(scope) = down.scope {
            self.scope = scope;
        }
        if down.text_geometry_changed {
            self.from_pump.text.tsf.layout_changed();
        }
        if down.hits_changed {
            self.hits.copy_from(&down.hits);
            // A fresh array carries no offsets, so the shadows are re-installed on the copy
            // whether or not the tracker set moved.
            self.hits.set_shadows(&self.trackers);
        }
        if down.trackers_changed {
            self.trackers.clear();
            self.trackers.append(&mut down.trackers);
            self.hits.set_shadows(&self.trackers);
        }
        if down.regions_changed {
            self.picks.sync(&down.regions);
        }
        for source in &down.fields.sources {
            self.from_pump.text.source(source);
        }
        for layout in &down.fields.layouts {
            self.from_pump.text.layout(layout);
        }
        let declared = &mut down.declared;
        for (id, decl) in declared.gestures.drain(..) {
            self.router.declare(id, decl);
        }
        // This mailbox also accumulates batches. Retirement wins over an earlier declaration,
        // including a text source whose owner closed while input waited.
        for id in declared.released.drain(..) {
            self.router.forget(id);
            _ = self.from_pump.text.forget(id);
        }
        self.focus.append(&mut declared.focus);
        if declared.caption != [None; 3] {
            self.caption = declared.caption;
        }
        self.app = declared.census;
        if down.tallies {
            self.scene = down.scene;
        }
        if !declared.uia.entries.is_empty() {
            self.from_pump.uia.borrow_mut().publish(&declared.uia);
        }
    }

    /// Turns what a provider asked for into the same reports and focus edits physical input
    /// produces, and asks the app thread for a tree the first time one is listening.
    fn automation(&mut self) {
        if self.from_pump.uia.borrow().listening() && !self.links.uia_listening.swap(true, AcqRel) {
            self.links.uia_requested.store(true, Release);
            self.links.app_bell.ring();
        }
        self.from_pump.uia.borrow_mut().drain(&mut self.uia_actions);
        for action in self.uia_actions.drain(..) {
            match action {
                uia::Action::Focus(id) => self.focus.push(FocusOp::Focus(Some(id))),
                // A reveal is the scroll containers' to answer, so it rides to the scene thread
                // beside the caret's own.
                uia::Action::Reveal(id) => {
                    let occlusion = self.from_pump.text.touch.docked;
                    self.to_scene.reveals.push(Reveal { id, occlusion });
                }
                action => self.to_scene.automation.push(action),
            }
        }
        self.from_pump
            .uia
            .borrow()
            .text_actions(&mut self.text_actions);
        for action in self.text_actions.drain(..) {
            let target = match &action {
                uia::action::TextAction::Replace(id, ..)
                | uia::action::TextAction::Select(id, ..) => *id,
            };
            if self
                .hits
                .entry(target)
                .is_some_and(|e| e.flags.contains(HitFlags::INTERACTIVE))
            {
                self.from_pump.text.automation(action);
            }
        }
    }

    /// Publishes what the provider answers positional and scroll queries from.
    ///
    /// After the reports, because a provider reads the focus and the offsets this tick settled
    /// on; only where something is listening, because nothing else reads any of it.
    fn publish_automation(&mut self, env: Env) {
        if !self.from_pump.uia.borrow().listening() {
            return;
        }
        let mut uia = self.from_pump.uia.borrow_mut();
        uia.set_focus(self.router.focus_mut().current());
        if let Some(origin) = client_origin(self.window.hwnd()) {
            uia.set_window(origin, env.scale());
        }
        for (viewport, shadow) in &self.trackers {
            // acquire: pairs with the release the tracker's own thread publishes the word with,
            // so both axes read here belong to one reported position.
            let (x, y) = unpack_offset(shadow.load(Acquire));
            uia.set_scroll(*viewport, Vector2 { x, y });
        }
        uia.flush();
    }
}
