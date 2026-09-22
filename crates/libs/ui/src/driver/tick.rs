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

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::*};

use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{ControlId, Env, HitFlags, NodeId};
use windows_window::{CaptionState, Handoff, Tick as Frame, Wake, Window};

use crate::input::{Doorbell, HitView, Report, Router, client_origin};
use super::reentry::Reentry;
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
    router: Router,
    /// What the window procedure owns and posts to, because those handlers run inside it and
    /// this tick is what forwards them.
    from_pump: Handoffs,
    /// The hit array as the scene thread last published it, and the window commands in it.
    ///
    /// Shared with the window procedure rather than held here, so the caption's own hit test is
    /// answered from the one array while a pass is on the stack.
    view: Rc<HitView>,
    /// Each viewport's live offset, as one atomic word the thread owning the trackers
    /// publishes. Installed on [`hits`](Self::hits) so a hit test resolves scroll without a hop,
    /// and read again where automation publishes the same offsets.
    trackers: Vec<(NodeId, Arc<AtomicU64>)>,
    /// The regions the pointer can be picked inside, with the part copy each is scanned against.
    picks: Picks,
    /// Focus edits the app thread emitted, applied after the router's own tick so a keyboard
    /// move lands on the reports the front table is about to read.
    focus: Vec<FocusOp>,
    /// Fields whose owner unmounted, retired after the router has run.
    ///
    /// Retiring one enters TSF, and no call-out may precede the routing a removed key is
    /// offered behind. Adopting the batch records them; the pass spends them.
    retiring: Vec<ControlId>,
    reports: Vec<Report>,
    /// The batch being filled for the scene thread.
    to_scene: Box<ToScene>,
    /// Held while a filled batch could not be handed over, so the pacer keeps ticking until it
    /// is: a batch that waited for the next contact would carry that contact's release to the
    /// scene thread late.
    holding: Option<Frame>,
    /// What this thread last told the scene thread the display was.
    sent: Option<Env>,
    /// What the other threads last reported about themselves, for the observer.
    scene: SceneTally,
    app: AppCensus,
    ticks: u64,
    /// Shaped field geometries adopted so far, for an observer measuring how many fields one
    /// keystroke reshaped.
    field_shapes: u64,
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
    /// Latched by the window procedure on `WM_SETTINGCHANGE`, so the caret and selection
    /// metrics are re-read once rather than per tick.
    pub(super) settings: Rc<Cell<bool>>,
    pub(super) wake: Wake,
}

impl Tick {
    /// Builds the tick: the router over the doorbell the window procedure already answers to.
    ///
    /// # Errors
    ///
    /// The router's recogniser pool or the pacer's wake could not be created.
    pub(super) fn new(
        window: &Rc<Window>,
        links: &Arc<Links>,
        bell: &Rc<Doorbell>,
        view: &Rc<HitView>,
        scope: Scope,
        from_pump: Handoffs,
    ) -> Result<Self> {
        let router = Router::new(bell, window, from_pump.wake.clone())?;
        Ok(Self {
            window: Rc::clone(window),
            links: Arc::clone(links),
            router,
            from_pump,
            view: Rc::clone(view),
            trackers: Vec::new(),
            picks: Picks::default(),
            focus: Vec::new(),
            retiring: Vec::new(),
            reports: Vec::new(),
            to_scene: Box::default(),
            holding: None,
            sent: None,
            scene: SceneTally::default(),
            app: AppCensus::default(),
            ticks: 0,
            field_shapes: 0,
            scope,
            uia_actions: Vec::new(),
            text_actions: Vec::new(),
        })
    }

    /// Runs one input tick.
    ///
    /// `phase` is marked serviced once the router has run and text focus has moved, which is
    /// what a removed key taken by a nested pump is offered to TSF behind.
    ///
    /// # Errors
    ///
    /// The router's own tick, a text action or the caret publication failed.
    pub(super) fn run(&mut self, phase: &Reentry) -> Result<()> {
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
        // ① input, against the array above. The array is read for the call and released, so
        // nothing downstream holds it across a call-out.
        self.reports.clear();
        let Self {
            view,
            router,
            reports,
            ..
        } = self;
        view.with(|hits| router.tick(hits, env, reports))?;
        // Focus requests must reach this pass's focus application before an idle tick parks.
        self.automation();
        // ② the focus edits the app thread emitted, into the same report list: a keyboard move
        // and a pointer move reach the front table the same way.
        self.apply_focus();
        // A control that stopped being interactive under the focus it holds keeps the ring
        // pointing at something no contact can reach, so focus is dropped rather than stranded.
        let focused = self.router.focus_mut().current();
        if focused.is_some_and(|id| {
            self.view
                .entry(id)
                .is_none_or(|e| !e.flags.contains(HitFlags::INTERACTIVE))
        }) {
            self.focus.push(FocusOp::Focus(None));
            self.apply_focus();
        }
        // From here the pass makes call-outs: TSF, the clipboard, the touch view, automation.
        // Everything above routed input and moved nothing outside this thread, so a removed key
        // taken by a nested pump from here on is offered to TSF behind input that has been
        // routed — which is what the boundary means.
        phase.serviced();
        // A field whose owner unmounted gives up its storage, and its focus with it. First,
        // because the rest of the pass reads what holds focus.
        let text = &mut self.from_pump.text;
        for id in self.retiring.drain(..) {
            text.forget(id)?;
        }
        // The caret's box, in the same space the array is scanned in, and whatever the editor
        // has to say about it. Both are the input thread's: TSF is pump-bound.
        text.geometry(&self.view, env.scale());
        text.reports(&mut self.reports, &self.view, env.scale())?;
        // Where a text service opens its own message pump: the notification this delivers is
        // what a TIP answers by asking for a lock, and it answers on this stack.
        #[cfg(feature = "test-support")]
        super::testing::at_call_out();
        text.flush(&mut self.to_scene.fields.updates);
        // An occlusion that changed is reported whether or not a field still holds focus,
        // because the extent it asked for is the scroll containers' to give back. A focus move
        // reports only where there is a field to bring into view.
        let occluded = text.touch.take();
        if occluded
            || (text.focused().is_some()
                && self
                    .reports
                    .iter()
                    .any(|r| matches!(r, Report::FocusChanged { .. })))
        {
            self.to_scene.reveals.push(Reveal {
                id: text.focused(),
                occlusion: text.touch.docked,
            });
        }
        if self.from_pump.settings.replace(false) {
            self.from_pump.text.settings_changed();
        }
        // A provider snapshot can precede disable, hide or unmount, so what one asked for is
        // executed against the adopted array's eligibility, just like physical input.
        self.text_automation();
        // ③ a contact inside a region writes that region's input and bumps its epoch here, on
        // this thread: the present thread reads both, and no other thread is in the way.
        let Self {
            view,
            reports,
            picks,
            to_scene,
            ..
        } = self;
        view.with(|hits| crate::present::pick(reports, hits, picks, &mut to_scene.intents));
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
        super::tallies(
            self.scene,
            self.app,
            &self.reports,
            self.ticks,
            self.field_shapes,
            self.from_pump.text.focused_box(),
        );
        Ok(())
    }

    /// Applies the focus edits held, raising one report where the ring moved.
    fn apply_focus(&mut self) {
        if self.focus.is_empty() {
            return;
        }
        let from = self.router.focus_mut().current();
        let Self {
            view,
            router,
            focus,
            ..
        } = self;
        if view.with(|hits| router.focus_mut().apply(focus, hits)) {
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
            self.view.replace(&down.hits, &self.trackers);
        }
        if down.trackers_changed {
            self.trackers.clear();
            self.trackers.append(&mut down.trackers);
            self.view.set_shadows(&self.trackers);
            // Before the publish below, so the tree this batch carries binds the trackers this
            // batch declared rather than the ones the batch before it did.
            self.from_pump.uia.borrow_mut().set_trackers(&self.trackers);
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
        self.field_shapes += down.fields.layouts.len() as u64;
        let declared = &mut down.declared;
        for (id, decl) in declared.gestures.drain(..) {
            self.router.declare(id, decl);
        }
        // This mailbox also accumulates batches. Retirement wins over an earlier declaration,
        // including a text source whose owner closed while input waited.
        for id in declared.released.drain(..) {
            self.router.forget(id);
            self.retiring.push(id);
            self.from_pump.uia.borrow_mut().release(id);
        }
        for peer in declared.peers.drain(..) {
            self.from_pump.uia.borrow_mut().watch_region(peer);
        }
        self.focus.append(&mut declared.focus);
        if declared.caption != [None; 3] {
            self.view.set_caption(declared.caption);
        }
        self.app = declared.census;
        if down.tallies {
            self.scene = down.scene;
        }
        if !declared.uia.entries.is_empty() {
            self.from_pump.uia.borrow_mut().publish(&declared.uia);
        }
        // After the publish: a control that moved and then republished reports the number the
        // publish stated, and `set_value` drops one equal to what it already holds.
        if !declared.intents.is_empty() {
            self.from_pump.uia.borrow_mut().observe(&declared.intents);
            declared.intents.clear();
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
                    self.to_scene.reveals.push(Reveal {
                        id: Some(id),
                        occlusion,
                    });
                }
                action => self.to_scene.automation.push(action),
            }
        }
    }

    /// Applies editor commands after focus and text-service reports have settled.
    fn text_automation(&mut self) {
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
                .view
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
        // The renderers' own geometry, joined against what this side says it means. One
        // acquire load per watched region where nothing moved, so it sits on the tick.
        uia.sync_regions();
        uia.flush();
    }
}
