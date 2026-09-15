//! The input thread's tick: everything the window's thread does per frame, and nothing that
//! draws.
//!
//! The tick runs inside the window procedure, reached from
//! [`WM_FRAME`](windows_window::WM_FRAME), so it keeps running through a nested pump: the
//! system's own resize and move loops, the window menu and `Alt`+`Space` each run a message
//! loop inside `DefWindowProc` and do not return until the gesture ends.
//!
//! It resolves contacts against a copy of the hit array the scene thread sent after its last
//! apply, moves nothing on screen, and hands the reports it produced to the scene thread,
//! which turns them into pixels. A region is the one exception: a pick inside one writes the
//! region's input state directly, because that state is an atomic the present thread reads.

use super::links::Links;
use super::{Observed, env_of, observed};
use crate::caption;
use crate::input::{Report, Router};
use crate::present::{self, Picks};
use crate::seam::{AppCensus, FocusOp, ToScene};
use crate::widget::Intent;
use core::sync::atomic::Ordering;
use std::rc::Rc;
use std::sync::Arc;
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{Census, Env, HitTable};
use windows_window::{CaptionState, Handoff, Tick, Wake, Window};

/// Everything one input tick needs, in one place the window procedure can reach.
///
/// A message handler is `'static`, so none of these can be a local in
/// [`UiRuntime::run`](super::UiRuntime::run) borrowed by it.
pub(super) struct Frame {
    pub scope: crate::role::Scope,
    /// The window, held rather than borrowed: the handler outlives every stack frame in
    /// [`UiRuntime::run`](super::UiRuntime::run). The window's own state holds a weak reference back to
    /// this frame, so the two do not keep each other alive.
    pub window: Rc<Window>,
    pub links: Arc<Links>,
    pub router: Router,
    pub text: crate::text_input::TextInput,
    pub settings_changed: Rc<std::cell::Cell<bool>>,
    pub uia: Rc<std::cell::RefCell<crate::uia::Uia>>,
    pub uia_actions: Vec<crate::uia::Action>,
    pub text_actions: Vec<crate::uia::action::TextAction>,
    /// The hit array as the scene thread last published it. The caption's hit handler reads
    /// it from the window procedure through the frame's own cell, fallibly.
    pub hits: HitTable,
    /// The regions the pointer can be picked inside, with the part copy each is scanned
    /// against.
    pub picks: Picks,
    /// The window commands, as the scene thread last published them.
    pub caption: caption::Registry,
    /// Focus edits the app thread emitted, applied after the router's own tick so a
    /// keyboard move lands on the reports the front table is about to read.
    pub focus: Vec<FocusOp>,
    pub reports: Vec<Report>,
    pub intents: Vec<Intent>,
    pub resized: Rc<Handoff<(i32, i32)>>,
    /// Posted when the window's scale changed. Carries nothing: the tick re-reads the
    /// environment and forwards it when it differs from what it last sent.
    pub rescaled: Rc<Handoff<()>>,
    pub nonclient: Rc<Handoff<CaptionState>>,
    /// The batch being filled for the scene thread. `None` only between handing one over
    /// and taking the spare back, which is one statement.
    pub out: Option<Box<ToScene>>,
    /// The frame clock, for the request below.
    pub wake: Wake,
    /// Held while a filled batch could not be handed over, so the pacer keeps ticking until
    /// it is: a batch that waited for the next contact would carry that contact's release
    /// to the scene thread late.
    pub holding: Option<Tick>,
    /// What this thread last told the scene thread the display was.
    pub sent_env: Option<Env>,
    /// What the scene thread last reported about itself, for the observer.
    pub census: Census,
    pub scene_wakes: u64,
    pub scene_applies: u64,
    pub app: AppCensus,
    pub ticks: u64,
}

impl Frame {
    /// Runs one input tick.
    ///
    /// # Errors
    ///
    /// The router's own tick failed.
    pub(super) fn tick(&mut self) -> Result<()> {
        self.ticks += 1;

        // ⓪ what the scene thread published since the last tick: the array a contact
        // resolves against, and the rows the router and the pick table are declared from.
        // Taken before the router runs, so a press lands on the geometry that is on screen
        // rather than the geometry that was.
        if let Some(mut inbound) = self.links.input_down.take() {
            if let Some(scope) = inbound.scope {
                self.scope = scope;
            }
            if inbound.text_geometry_changed {
                self.text.tsf.layout_changed();
            }
            if inbound.hits_changed {
                self.hits.copy_from(&inbound.hits);
            }
            for &(target, decl) in &inbound.gestures {
                self.router.declare(target, decl);
            }
            if inbound.regions_changed {
                self.picks.sync(&inbound.regions);
            }
            if let Some(ids) = inbound.caption {
                self.caption = ids.into();
            }
            if inbound.trackers_changed {
                let shadows = self.router.shadows_mut();
                shadows.clear();
                for tracker in &inbound.trackers {
                    shadows.insert(tracker.viewport, Arc::clone(&tracker.shadow));
                }
            }
            for source in &inbound.field_sources {
                self.text.source(source);
            }
            for layout in &inbound.field_layouts {
                self.text.layout(layout);
            }
            // This mailbox also accumulates batches. Retirement wins over an earlier
            // declaration, including a text source whose owner closed while input waited.
            for &target in &inbound.released {
                self.router.forget(target);
                self.text.forget(target)?;
            }
            if let Some(seeds) = inbound.seeds.as_ref() {
                self.uia.borrow_mut().publish(self.hits.entries(), seeds);
            }
            self.focus.append(&mut inbound.focus);
            self.census = inbound.census;
            self.scene_wakes = inbound.scene_wakes;
            self.scene_applies = inbound.scene_applies;
            self.app = inbound.app;
            inbound.clear();
            // The spare goes back, and the scene thread is rung only if it was holding a
            // batch for want of one.
            _ = self.links.input_down_spare.put(inbound);
            if self
                .links
                .scene_wants_input_spare
                .swap(false, Ordering::AcqRel)
            {
                self.links.scene_ring.ring();
            }
        }

        let Some(env) = env_of(&self.window, self.scope) else {
            return Ok(());
        };

        if self.uia.borrow().listening() && !self.links.uia_listening.swap(true, Ordering::AcqRel) {
            self.links.uia_requested.store(true, Ordering::Release);
            self.links.app_ring.ring();
        }
        self.uia.borrow_mut().drain(&mut self.uia_actions);
        for action in self.uia_actions.drain(..) {
            match action {
                crate::uia::Action::Focus(id) => self.focus.push(FocusOp::Focus(Some(id))),
                crate::uia::Action::Reveal(id) => {
                    if let Some(out) = self.out.as_mut() {
                        out.reveals.push(crate::text_input::Reveal {
                            id,
                            occlusion: self.text.touch.docked,
                        });
                    }
                }
                action => {
                    if let Some(out) = self.out.as_mut() {
                        out.automation.push(action);
                    }
                }
            }
        }
        self.uia.borrow().text_actions(&mut self.text_actions);
        for action in self.text_actions.drain(..) {
            let id = match &action {
                crate::uia::action::TextAction::Replace(id, ..)
                | crate::uia::action::TextAction::Select(id, ..) => *id,
            };
            // A provider snapshot can precede disable/hide/unmount. Execution uses the
            // adopted hit generation and eligibility, just like physical input.
            if self
                .hits
                .entry(id)
                .is_some_and(|entry| entry.flags.contains(windows_scene::HitFlags::INTERACTIVE))
            {
                self.text.automation(action);
            }
        }

        if self.settings_changed.replace(false) {
            crate::text_input::settings::refresh();
            if let Some(editor) = self.text.docs.borrow_mut().active_mut() {
                editor.publish(false, false);
            }
        }
        // ① input, against the array above.
        self.reports.clear();
        self.router.tick(&self.hits, env, &mut self.reports)?;

        // ② the focus edits the app thread emitted, into the same report list: a keyboard
        // move and a pointer move reach the front table the same way.
        if !self.focus.is_empty() {
            self.router
                .focus_mut()
                .apply(&self.focus, &self.hits, &mut self.reports);
            self.focus.clear();
        }

        if self.router.focus_mut().current().is_some_and(|id| {
            !self
                .hits
                .entries()
                .iter()
                .any(|e| e.id == id && e.flags.contains(windows_scene::HitFlags::INTERACTIVE))
        }) {
            self.router
                .focus_mut()
                .apply(&[FocusOp::Focus(None)], &self.hits, &mut self.reports);
        }
        self.text
            .geometry(&self.hits, self.router.shadows(), env.scale());
        self.text.reports(
            &mut self.reports,
            &self.hits,
            self.router.shadows(),
            env.scale(),
        )?;
        let occlusion_changed = self.text.touch.take();
        if let Some(out) = self.out.as_mut() {
            self.text.flush(&mut out.text);
            if occlusion_changed
                || self
                    .reports
                    .iter()
                    .any(|r| matches!(r, Report::FocusChanged { .. }))
            {
                if let Some(id) = self.text.docs.borrow().active {
                    out.reveals.push(crate::text_input::Reveal {
                        id,
                        occlusion: self.text.touch.docked,
                    });
                }
            }
        }

        // ③ a contact inside a region writes that region's input and bumps its epoch here,
        // on this thread: the present thread reads both, and no other thread is in the way.
        self.intents.clear();
        present::pick(
            &self.reports,
            &self.hits,
            &mut self.picks,
            &mut self.intents,
        );

        // ④ what the scene thread turns into pixels, and the window facts that arrived with
        // it. Appended to the batch this thread holds; handed over only when the spare is
        // back, otherwise carried to the next tick in the order it happened.
        let resized = self.resized.take();
        let rescaled = self.rescaled.take().is_some();
        let nonclient = self.nonclient.take();
        let env_moved = rescaled || self.sent_env != Some(env);
        let quiet = self.out.as_ref().is_none_or(|out| {
            out.text.is_empty() && out.reveals.is_empty() && out.automation.is_empty()
        }) && self.reports.is_empty()
            && self.intents.is_empty()
            && resized.is_none()
            && nonclient.is_none()
            && !env_moved;
        if !quiet {
            let Some(out) = self.out.as_mut() else {
                return Ok(());
            };
            out.reports.extend_from_slice(&self.reports);
            out.intents.extend_from_slice(&self.intents);
            if let Some(state) = nonclient {
                out.nonclient = Some(state);
            }
            if let Some((width, height)) = resized {
                let scale = env.scale();
                out.window = Some(Vector2 {
                    x: width as f32 / scale,
                    y: height as f32 / scale,
                });
            }
            if env_moved {
                out.env = Some(env);
                self.sent_env = Some(env);
            }
            self.hand_over();
        }

        let Some(env) = env_of(&self.window, self.scope) else {
            return Ok(());
        };

        if self.uia.borrow().listening() {
            let mut uia = self.uia.borrow_mut();
            uia.set_focus(self.router.focus_mut().current());
            if let Some(origin) = crate::input::Coords::new(self.window.hwnd()).origin() {
                uia.set_window(origin, env.scale());
            }
            for entry in self.hits.entries() {
                uia.set_scroll(
                    entry.scroll_src,
                    windows_scene::ScrollOffsets::offset(self.router.shadows(), entry.scroll_src),
                );
            }
            uia.flush();
        }
        // Last, so what an observer is handed is what the whole tick settled on.
        observed(Observed {
            reports: &self.reports,
            census: self.census,
            scene_wakes: self.scene_wakes,
            scene_applies: self.scene_applies,
            app: self.app,
            ticks: self.ticks,
        });
        Ok(())
    }

    /// Hands the filled batch to the scene thread if the spare is back, and rings it.
    ///
    /// With no spare back the batch stays here and grows; the next tick tries again. Order
    /// is kept either way: the batch in the mailbox precedes the one held.
    fn hand_over(&mut self) {
        let Some(spare) = self.links.to_scene_spare.take() else {
            self.holding.get_or_insert_with(|| self.wake.tick());
            return;
        };
        let Some(filled) = self.out.replace(spare) else {
            return;
        };
        match self.links.to_scene.put(filled) {
            Ok(()) => {
                self.holding = None;
                self.links.scene_ring.ring();
            }
            // Occupied: the scene thread has not taken the previous batch. Keep the filled
            // one and return the spare, so nothing is lost and nothing is reordered.
            Err(filled) => {
                if let Some(spare) = self.out.replace(filled) {
                    _ = self.links.to_scene_spare.put(spare);
                }
                self.holding.get_or_insert_with(|| self.wake.tick());
            }
        }
    }
}
