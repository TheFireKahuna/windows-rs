//! The pointer stack: pointer, keyboard and dial messages in, resolved [`Report`]s out on the
//! frame clock.
//!
//! ```text
//!  front thread — WndProc                     front thread — service tick
//!  ───────────────────────────────            ────────────────────────────────────────
//!  WM_POINTERDOWN / UP / cancel   ─ring──▶    1. drain ring in order → flat hit test
//!  WM_POINTERWHEEL / HWHEEL       ─ring──▶    2. active contact?
//!  WM_KEY* / WM_CHAR              ─ring──▶         batch → ProcessMoveEvents
//!        └─ and post WM_FRAME now                  (recogniser events → gesture sinks)
//!                                             3. at most once per tick: every RAW
//!  WM_POINTERUPDATE               ─flag──▶       sample in the batch → crossings
//!  WM_POINTERENTER / LEAVE        ─flag──▶    4. drain the dial
//!        └─ and ask for a frame                5. ProcessInertia() per inertial recogniser
//!                                             6. return — the work item publishes
//! ```
//!
//! The split rests on four properties of pointer input:
//!
//! * A pointer message signals that samples are available; it does not carry them.
//! * Hover is a per-frame quantity and a manipulation is an integrated one. The split is at
//!   the consumption point, not at the message.
//! * A discrete transition is neither: it asks to be serviced on the next pump iteration,
//!   because waiting for the display would add a frame of latency to each.
//! * Resolving a contact costs a walk proportional to the node count, which the flat hit array
//!   in [`HitTable`] bounds.
//!
//! A tick is not a frame. Ticks are bounded by the frame clock *plus* the discrete input rate,
//! so anything genuinely per-frame is gated on [`Wake::frames`](windows_window::Wake::frames)
//! rather than on the tick.
//!
//! # There is no legacy mouse path
//!
//! `DefWindowProc` promotes pointer input into legacy mouse messages, so every pointer arm
//! that carries a contact is handled and none falls through. Neither binding filter generates
//! `WM_MOUSEMOVE`, its relatives or `TrackMouseEvent`, so a legacy arm does not compile.
//!
//! # The environment is stated, never held
//!
//! [`Router::tick`] takes an [`Env`] because the display's scale and its output transform
//! belong to the window and its monitor. A router holding its own copy is not told when the
//! window moves to another display, and every contact then resolves against the wrong pixel
//! grid.

mod coords;
mod doorbell;
mod focus;
mod platform;

mod view;
pub use view::HitView;

pub use coords::{Coords, Pen, PointerSpace, Sample, client_origin};
pub use doorbell::{
    Doorbell, DoorbellHealth, EventKind, InputEvent, KeyEvent, KeyKind, Mods, PointerEvent,
    PointerFlags, PointerType,
};
pub use focus::{FocusRing, Move, ScopeId};
pub use platform::{Capability, Inertia, Late, Service};

use crate::bindings::*;
use crate::gesture::{
    Contacts, DragUpdate, Events, Feed, Flags, Recognised, SLOTS, pivot_of, scroll_ancestor,
    scroll_offer,
};
use crate::rotary::{Rotary, Rotation};
use rustc_hash::FxHashMap;
use std::rc::Rc;
use windows_core::Result;
use windows_scene::{ContactKind, ControlId, Env, Hit, HitFlags, HitTable, Point};
use windows_window::{Tick, Wake, Window};

/// How many coalesced entries one service reads back.
///
/// A pointer's `historyCount` is bounded by how far behind the consumer fell; 128 is several
/// frames of a 1 kHz digitizer. A deeper batch means the pump never reached the frames that
/// produced it.
const HISTORY_MAX: usize = 128;

/// Reports one outcome of a tick, published in the order it happened.
///
/// Every variant is resolved on the front thread, and the layer above turns a report into
/// pixels before it turns it into an intent. An intent therefore exists only after the visual
/// it belongs to, and can never be the cause of one.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Report {
    /// Offers a touch press to its nearest scrolling ancestor. Scene-thread only.
    Redirect {
        target: ControlId,
        pointer: windows_scene::ManipulationPointer,
    },
    /// A hover boundary was crossed. One tick can publish several, in the order the pointer
    /// crossed them: a fast flick over a toolbar publishes every crossing on the path, and the
    /// layer that owns the chrome decides which of them light anything.
    HoverChanged {
        from: Option<ControlId>,
        to: Option<ControlId>,
        /// Where the crossing happened, in client DIPs.
        at: Point,
        /// The performance-counter value the sample was stamped with, or zero where it carried
        /// none. A dwell is measured against this rather than against a tick count.
        qpc: u64,
    },
    FocusChanged {
        from: Option<ControlId>,
        to: Option<ControlId>,
    },
    Pressed {
        target: ControlId,
        contact: u32,
        /// The contact as it was at the message, not as it is now. Its `raw` is the point the
        /// target was chosen from, and it carries the pen's pressure and tilt and the measured
        /// contact patch.
        sample: Sample,
        buttons: u32,
    },
    /// A bound contact moved. Carries the pen's pressure, tilt and twist, which a recogniser's
    /// own events do not: those carry a position alone.
    Moved {
        target: ControlId,
        contact: u32,
        sample: Sample,
    },
    /// A button changed while the contact stayed down.
    Buttons {
        target: ControlId,
        contact: u32,
        buttons: u32,
    },
    Released {
        target: ControlId,
        contact: u32,
        at: Point,
    },
    /// A contact ended without releasing: the pre-drag value is restored and nothing is
    /// committed.
    Canceled { target: ControlId, contact: u32 },
    Gesture {
        target: ControlId,
        contact: u32,
        event: Recognised,
    },
    Dragged {
        target: ControlId,
        contact: u32,
        update: DragUpdate,
    },
    /// A wheel notch over a target that is not a scroll surface. A scroll container's wheel
    /// does not reach here: `PointerWheelConfig` routes it to that container's tracker on the
    /// compositor side, with no front-thread work.
    Wheel {
        target: Option<ControlId>,
        at: Point,
        /// Notches, signed. One detent is `1.0`.
        notches: f32,
        horizontal: bool,
    },
    Key {
        target: Option<ControlId>,
        event: KeyEvent,
    },
    /// `Esc` reached the innermost focus scope, before any control saw it.
    Escape { scope: Option<ScopeId> },
    /// A press landed on an overlay's blocker. The press is consumed: nothing under the
    /// blocker is pressed and no focus moves.
    Dismiss {
        blocker: ControlId,
        scope: Option<ScopeId>,
    },
    /// The dial turned. Reports a delta, so it lands on the gesture seam and drives the same
    /// value path a knob drag does.
    Rotary {
        target: Option<ControlId>,
        degrees: f64,
        /// `degrees` divided by the target's declared resolution.
        steps: f64,
    },
    RotaryButton {
        target: Option<ControlId>,
        pressed: bool,
    },
    /// Every contact was taken away — the window lost focus.
    CaptureLost,
}

/// Counts what the pointer stack did.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct InputCensus {
    pub ticks: u64,
    /// Samples examined to resolve hover. Bounded by the pointer's report rate rather than by
    /// the frame clock, because every sample in a batch is tested: a crossing between two of
    /// them is an event and not a state.
    pub hover_hits: u64,
    /// Boundary crossings published. Bounded by what a user can see, so a runaway hover path
    /// shows up here.
    pub hover_changes: u64,
    /// The deepest coalesced batch ever read.
    ///
    /// One means the platform never coalesced: the pump kept up and each service saw a single
    /// sample. Greater than one means the pump fell behind and the batch carries samples a
    /// point-sampling consumer would have dropped.
    pub deepest_batch: u32,
    /// Hit tests run to resolve a discrete transition. Bounded by human input rate.
    pub discrete_hits: u64,
    /// Contacts bound to a recogniser.
    pub bindings: u64,
    /// Gestures recognised.
    pub gestures: u64,
    /// Contacts that aborted rather than completing.
    pub aborts: u64,
    /// Contacts the digitizer reported without confidence, which are treated as palms.
    pub rejected: u64,
}

/// Drains a [`Doorbell`] on the frame clock and publishes the resulting [`Report`]s.
///
/// Runs on the front thread: it owns recognisers, which are non-agile, and it resolves through
/// the retained tree's hit array.
pub struct Router {
    bell: Rc<Doorbell>,
    contacts: Contacts,
    /// A clone of the contact table's queue, so draining it does not conflict with writing the
    /// columns beside it.
    events: Events,
    /// Which gestures each target accepts. Resolved on the front thread, so deciding whether a
    /// gesture applies makes no call to the application thread.
    decls: FxHashMap<ControlId, crate::gesture::GestureDecl>,
    focus: FocusRing,
    capability: Capability,
    inertia: Inertia,
    /// The window's dial. `None` means no radial controller is attached.
    rotary: Option<Rotary>,
    hover: Option<ControlId>,
    /// The mouse contact holding explicit Win32 capture. Touch routing is system-owned.
    capture: Option<u32>,
    /// The environment the last tick ran under. Read only by the comparison that decides what
    /// a change to the environment invalidated; never answered as the current scale.
    env: Option<Env>,
    /// Set when the hover answer may have changed without the pointer moving.
    stale: bool,
    /// Held while a gesture or a stale hover is live. The doorbell holds its own for the ring.
    tick: Option<Tick>,
    wake: Wake,
    census: InputCensus,
    // ── scratch, so a frame allocates nothing after the first ─────────────────────
    /// The coalesced-history buffer, allocated once. `POINTER_INFO` is ~100 bytes, so this
    /// holds ~13 KB for the window's life and a contact's motion allocates nothing.
    history: Vec<POINTER_INFO>,
    moved: Vec<u32>,
    recognised: Vec<Recognised>,
    rotations: Vec<Rotation>,
}

impl Router {
    /// Builds a router over a doorbell already installed in `window`.
    ///
    /// Takes no scale and no output transform: both belong to the window and its monitor and
    /// are stated at every [`tick`](Self::tick), so a display change cannot leave the router
    /// resolving contacts against a stale pixel grid.
    ///
    /// # Errors
    ///
    /// `window` is closed, leaving no handle to resolve against.
    pub fn new(bell: &Rc<Doorbell>, window: &Window, wake: Wake) -> Result<Self> {
        if !window.is_open() {
            return Err(windows_core::Error::new(
                windows_core::HRESULT(0x8007_0006_u32 as i32),
                "the window is closed",
            ));
        }
        bell.pace(window, wake.clone());
        let late = bell.late();
        let contacts = Contacts::default();
        Ok(Self {
            events: contacts.events().clone(),
            contacts,
            bell: Rc::clone(bell),
            decls: FxHashMap::default(),
            focus: FocusRing::default(),
            capability: Capability::read(window, &late),
            inertia: Inertia::new(window, late),
            rotary: None,
            hover: None,
            capture: None,
            env: None,
            stale: false,
            tick: None,
            wake,
            census: InputCensus::default(),
            history: vec![POINTER_INFO::default(); HISTORY_MAX],
            moved: Vec::with_capacity(SLOTS),
            recognised: Vec::with_capacity(32),
            rotations: Vec::with_capacity(8),
        })
    }

    /// Returns what the machine reports about its own input devices. Diagnostic: no path
    /// branches on it.
    #[must_use]
    pub const fn capability(&self) -> &Capability {
        &self.capability
    }

    /// Returns the counters this stack has accumulated.
    #[must_use]
    pub const fn census(&self) -> &InputCensus {
        &self.census
    }

    /// Returns the factor the WinRT pointer statics were measured to answer in, which is
    /// `1.0` while unmeasured and exactly `1.0` where the two spaces agreed.
    #[must_use]
    pub fn measured_factor(&self) -> f32 {
        self.bell.space().factor()
    }

    /// Returns the focus ring. Focus order is the hit array's, filtered to `INTERACTIVE`.
    pub const fn focus_mut(&mut self) -> &mut FocusRing {
        &mut self.focus
    }

    /// Attaches the window's radial controller.
    ///
    /// `CreateForWindow` succeeds on a machine with no dial attached and the object then never
    /// raises anything, so a dial plugged in later needs no re-attach.
    ///
    /// # Errors
    ///
    /// The interop factory refused the window, or an event registration failed.
    pub fn attach_rotary(&mut self, window: &Window) -> Result<()> {
        self.rotary = Some(Rotary::new(window, self.bell.service())?);
        Ok(())
    }

    /// Records which gestures `target` accepts. Called as the widget mounts.
    pub fn declare(&mut self, target: ControlId, decl: crate::gesture::GestureDecl) {
        self.decls.insert(target, decl);
    }

    /// Drops `target`'s declaration, on unmount.
    ///
    /// Any contact still bound to it aborts, because a gesture whose target has gone cannot
    /// commit anything, and the abort releases any explicit mouse capture that contact held.
    pub fn forget(&mut self, target: ControlId) {
        self.decls.remove(&target);
        for id in self.contacts.ids() {
            if id != 0
                && self
                    .contacts
                    .bound(id)
                    .is_some_and(|(held, _)| held == target)
            {
                self.drop_contact(id);
            }
        }
        self.focus.forget(target);
    }

    /// Marks the hover answer stale, for a layout change under a stationary pointer.
    ///
    /// Costs one hit test on the next tick and nothing at idle.
    pub fn invalidate_hover(&mut self) {
        self.stale = true;
        self.tick.get_or_insert_with(|| self.wake.tick());
    }

    /// Stops content inertia at the system's request.
    ///
    /// No message arm reaches this: neither inertia message number is in the generated
    /// bindings, so there is no constant to match on.
    pub fn stop_inertia(&mut self) {
        self.bell.stop_inertia();
    }

    /// Consumes one frame of input, appending each outcome to `out`.
    ///
    /// The order is part of the contract: discrete transitions first, in the order they
    /// happened; then every intermediate sample of every active contact; then at most one
    /// hover resolution; then the dial; then inertia. Hover resolves after the presses,
    /// because a hover resolved before them would answer for the previous frame's layout.
    ///
    /// # Errors
    ///
    /// A platform pointer or recogniser call refused. Whatever was resolved before it is
    /// already in `out`.
    pub fn tick(&mut self, hits: &HitTable, env: Env, out: &mut Vec<Report>) -> Result<()> {
        // Re-opened before the drain, so a transition arriving during it asks again rather
        // than being swallowed. A gate re-opened after the drain stays shut over that window,
        // and the symptom is input that silently stops.
        self.bell.service().begin();
        self.census.ticks += 1;
        self.retarget(env);
        while let Some(event) = self.bell.pop() {
            match event {
                InputEvent::Pointer(event) => self.pointer(&event, hits, env, out)?,
                InputEvent::Key(event) => self.key(event, hits, out),
            }
        }
        let hovering_moved = self.feed(hits, env, out)?;
        self.resolve_hover(hits, env, out, hovering_moved);
        self.rotate(hits, out);
        self.pump(hits, out)?;
        self.settle();
        Ok(())
    }

    /// Brings the router up to date with `env` and keeps it as the next tick's watermark.
    ///
    /// Any change to the environment makes the hover answer stale. A scale change additionally
    /// discards the measured recogniser factor, which was never derived from the scale and has
    /// no arithmetic that carries it to a new one.
    fn retarget(&mut self, env: Env) {
        let Some(last) = self.env.replace(env) else {
            return;
        };
        if last == env {
            return;
        }
        if last.scale() != env.scale() {
            self.bell.space().forget();
            if let Some(rotary) = self.rotary.as_ref() {
                rotary.rescale(env.scale());
            }
        }
        self.stale = true;
    }

    /// Takes or drops the frame request covering everything this tick left running.
    fn settle(&mut self) {
        if self.contacts.live() > 0 || self.stale {
            self.tick.get_or_insert_with(|| self.wake.tick());
        } else {
            self.tick = None;
            self.bell.settle();
        }
        // A window whose content is moving must say so, or a touchpad tap lands on whatever
        // was moving under it. A refusal is retried by the next tick, so the result is not
        // acted on here.
        _ = self.inertia.set(self.contacts.any_inertial());
    }

    // ── 1. the ring, in order ─────────────────────────────────────────────────────

    fn pointer(
        &mut self,
        p: &PointerEvent,
        hits: &HitTable,
        env: Env,
        out: &mut Vec<Report>,
    ) -> Result<()> {
        match p.kind {
            EventKind::Down => return self.press(p, hits, env, out),
            EventKind::Up => return self.up(p, hits, env, out),
            EventKind::Wheel => return self.wheel(p, hits, env, out),
            EventKind::Cancel => {
                // A window-level capture change names no pointer, so it ends whatever held the
                // explicit capture and nothing where none did.
                let contact = if p.id == 0 { self.capture } else { Some(p.id) };
                if let Some(id) = contact {
                    self.abort(id, out);
                }
            }
            EventKind::CaptureLost => {
                self.contacts.release_all(true);
                self.capture = None;
                out.push(Report::CaptureLost);
                let from = self.focus.keyboard();
                self.focus.window_focus(false, hits);
                self.focus.report(from, out);
            }
            EventKind::FocusGained => {
                let from = self.focus.keyboard();
                self.focus.window_focus(true, hits);
                self.focus.report(from, out);
            }
            EventKind::Button => {
                if let Some((target, _)) = self.contacts.bound(p.id) {
                    out.push(Report::Buttons {
                        target,
                        contact: p.id,
                        buttons: p.buttons,
                    });
                }
            }
        }
        Ok(())
    }

    /// Returns the one report a contact's end produces.
    ///
    /// A release carries where it happened; an abort carries nothing, because a canceled
    /// contact restores the pre-drag value and commits none.
    fn ended(target: ControlId, contact: u32, at: Option<Point>) -> Report {
        match at {
            Some(at) => Report::Released {
                target,
                contact,
                at,
            },
            None => Report::Canceled { target, contact },
        }
    }

    /// Ends the contact bound to `id` without committing anything, reporting it once.
    fn abort(&mut self, id: u32, out: &mut Vec<Report>) {
        if let Some((target, _)) = self.contacts.bound(id) {
            out.push(Self::ended(target, id, None));
        }
        self.drop_contact(id);
    }

    /// The same teardown for an OS cancellation, an unmounted target, or a failed press.
    fn drop_contact(&mut self, id: u32) {
        self.contacts.release(id, true);
        self.census.aborts += 1;
        if self.capture == Some(id) {
            self.capture = None;
            // SAFETY: `ReleaseCapture` takes no argument and writes through no pointer.
            unsafe {
                _ = ReleaseCapture();
            }
        }
        self.bell.release(id);
    }

    fn press(
        &mut self,
        p: &PointerEvent,
        hits: &HitTable,
        env: Env,
        out: &mut Vec<Report>,
    ) -> Result<()> {
        // Built from the point the doorbell recorded at the message, not from wherever the
        // contact has since moved to.
        let sample = self.bell.coords().at_transition(p, env);
        self.census.discrete_hits += 1;
        let Some(hit) = hits.hit(sample.raw, sample.kind()) else {
            // A press on nothing still takes focus away, so clicking the background dismisses
            // a text caret.
            let from = self.focus.keyboard();
            self.focus.focus(None);
            self.focus.report(from, out);
            return Ok(());
        };
        // An overlay's blocker consumes the press outright. Nothing under it is pressed, no
        // focus moves, and the overlay's owner decides what closing means.
        if hit.flags.contains(HitFlags::BLOCKER) {
            out.push(Report::Dismiss {
                blocker: hit.id,
                scope: self.focus.innermost(),
            });
            return Ok(());
        }
        let from = self.focus.keyboard();
        self.focus.focus(Some(hit.id));
        self.focus.report(from, out);
        out.push(Report::Pressed {
            target: hit.id,
            contact: p.id,
            sample,
            buttons: p.buttons,
        });
        // A contact the digitizer is not confident about is a palm. It is still tracked — its
        // up has to be accounted for — but nothing is fed to a recogniser, so it can never
        // start a gesture.
        let rejected = p.ptype == PointerType::Touch && !p.flags.confident();
        if rejected {
            self.census.rejected += 1;
        }
        // A contact routes to its down-window for its life. Mouse is the one device that can
        // leave the window without lifting, and the call is what makes the window own the
        // cursor: a window that does not pays a hit test and a cursor resolution on every
        // sample rather than two resynchronisations.
        if p.ptype == PointerType::Mouse {
            self.capture = Some(p.id);
            // SAFETY: `SetCapture` takes the window handle by value and writes through no
            // pointer; a handle whose window has been destroyed fails the call rather than
            // being dereferenced.
            unsafe {
                _ = SetCapture(self.bell.coords().hwnd());
            }
        }
        match self.start(p, hit, hits, sample.raw, rejected, out) {
            Ok(()) => Ok(()),
            // A press that was reported and could not be started still ends, and ends once.
            Err(error) => {
                out.push(Self::ended(hit.id, p.id, None));
                self.drop_contact(p.id);
                Err(error)
            }
        }
    }

    /// Binds the contact, feeds its recogniser the down, and offers a touch press to the
    /// nearest scroll container.
    ///
    /// # Errors
    ///
    /// No slot was free, the platform refused to configure a recogniser, or the down sample
    /// the doorbell retained could not be taken or fed.
    fn start(
        &mut self,
        p: &PointerEvent,
        hit: Hit,
        hits: &HitTable,
        origin: Point,
        rejected: bool,
        out: &mut Vec<Report>,
    ) -> Result<()> {
        // A contact is bound whatever its target declared: a declaration says which gestures a
        // press can become, not whether the press happened. Skipping the bind for a target
        // that declared nothing would leave it with a press and no release, which latches its
        // press wash, holds its slot for the life of the window, and loses the tap, because a
        // tap is a press and a release on one control.
        let decl = self.decls.get(&hit.id).copied().unwrap_or_default();
        // Ordinary content touches are offered to the nearest scroll ancestor. The local
        // recogniser stays until capture is actually lost: a tap may never become a
        // manipulation.
        let scroll = (p.ptype == PointerType::Touch && !rejected)
            .then(|| scroll_offer(hits, hit, decl))
            .flatten();
        self.contacts
            .bind(p.id, hit.id, decl, p.ptype.is_touchpad(), origin, rejected)?;
        self.contacts
            .set(p.id, Flags::SCROLL_TOUCH, scroll.is_some());
        // Stated at the down and not first at the manipulation, so the rotation the platform
        // reports covers the whole gesture rather than starting one update late.
        self.restate_pivot(p.id, hit.id, hits);
        self.census.bindings += 1;
        if !rejected && let Some(point) = p.point() {
            self.contacts.feed(p.id, Feed::Down(point?))?;
            self.collect(p.id, hits, out);
        }
        if let Some(target) = scroll
            && let Some(pointer) = p.manipulation()
        {
            out.push(Report::Redirect {
                target,
                pointer: pointer?,
            });
        }
        Ok(())
    }

    fn up(&mut self, p: &PointerEvent, hits: &HitTable, env: Env, out: &mut Vec<Report>) -> Result<()> {
        let at = self.bell.coords().client(env, p.id, p.x_px, p.y_px);
        if self.capture == Some(p.id) {
            // Releasing capture posts `WM_CAPTURECHANGED` back at us; clearing the owner first
            // is what keeps that from ending a gesture that has just completed.
            self.capture = None;
            if p.ptype == PointerType::Mouse {
                // SAFETY: `ReleaseCapture` takes no argument and writes through no pointer.
                unsafe {
                    _ = ReleaseCapture();
                }
            }
        }
        let Some((target, _)) = self.contacts.bound(p.id) else {
            self.bell.release(p.id);
            return Ok(());
        };
        let flags = self.contacts.flags(p.id);
        let released = !flags.has(Flags::REJECTED);
        // Ordinary releases are unconditional. Scrollable touch content waits for the
        // recogniser's tap decision below, so that it ends exactly once either way.
        if !flags.has(Flags::SCROLL_TOUCH) {
            out.push(Self::ended(target, p.id, released.then_some(at)));
        }
        let fed = match p.point() {
            Some(point) if released => {
                point.and_then(|point| self.contacts.feed(p.id, Feed::Up(point)))
            }
            _ => Ok(()),
        };
        self.collect(p.id, hits, out);
        // A quick swipe may finish before the compositor takes capture. Only an actual
        // recogniser tap can invoke the child control in that case.
        if flags.has(Flags::SCROLL_TOUCH) {
            let tapped = self.contacts.flags(p.id).has(Flags::TAPPED);
            out.push(Self::ended(target, p.id, tapped.then_some(at)));
        }
        // Inertia keeps the binding alive: the contact is gone but its motion is not, and the
        // recogniser running that motion is the one being pumped.
        let inertial = self.contacts.still_inertial(p.id);
        self.contacts.set(p.id, Flags::INERTIAL, inertial);
        if !inertial {
            self.contacts.release(p.id, false);
        }
        self.bell.release(p.id);
        // Returned last, so a refused sample reaches the caller only after the contact has
        // been fully ended rather than leaving this stack still holding it.
        fed
    }

    fn wheel(
        &mut self,
        p: &PointerEvent,
        hits: &HitTable,
        env: Env,
        out: &mut Vec<Report>,
    ) -> Result<()> {
        let at = self.bell.coords().client(env, p.id, p.x_px, p.y_px);
        self.census.discrete_hits += 1;
        let hit = hits.hit(at, p.ptype.contact());
        // A scroll surface's wheel belongs to its tracker: the source's `PointerWheelConfig`
        // takes it, and handling it front-side here would be a second scroll path.
        if hit.is_some_and(|hit| {
            (hit.flags.contains(HitFlags::SCROLL) || !hit.flags.contains(HitFlags::WHEEL))
                && scroll_ancestor(hits, hit).is_some()
        }) {
            return Ok(());
        }
        if let Some(hit) = hit
            && self
                .contacts
                .bound(p.id)
                .is_some_and(|(target, _)| target == hit.id)
            && let Some(point) = p.point()
        {
            // A pointer wheel message packs the pointer id where the legacy one packs the
            // modifier keys, so neither shift nor control is readable from the record.
            self.contacts
                .feed(p.id, Feed::Wheel(point?, false, false))?;
            self.collect(p.id, hits, out);
            return Ok(());
        }
        out.push(Report::Wheel {
            target: hit.map(|hit| hit.id),
            at,
            notches: p.wheel as f32 / WHEEL_DELTA as f32,
            horizontal: p.horizontal,
        });
        Ok(())
    }

    fn key(&mut self, event: KeyEvent, hits: &HitTable, out: &mut Vec<Report>) {
        if !self.focus.window_focused() {
            return;
        }
        // Tab and Esc are taken before any control sees them: focus order has one authority,
        // and an open overlay closes from the keyboard wherever the pointer is.
        if event.kind == KeyKind::Down {
            match event.key as i32 {
                VK_TAB => {
                    let from = self.focus.keyboard();
                    match self.focus.step(hits, !event.mods.shift) {
                        Move::To => self.focus.report(from, out),
                        // Off the end of a scope that does not trap: dismiss it and let the
                        // owner step again outside.
                        Move::Left => out.push(Report::Escape {
                            scope: self.focus.innermost(),
                        }),
                        Move::None => {}
                    }
                    return;
                }
                // With no scope open, `Esc` is an ordinary key and reaches the focused control.
                VK_ESCAPE if self.focus.innermost().is_some() => {
                    out.push(Report::Escape {
                        scope: self.focus.innermost(),
                    });
                    return;
                }
                _ => {}
            }
        }
        out.push(Report::Key {
            target: self.focus.keyboard(),
            event,
        });
    }

    // ── 2. every intermediate sample of every active contact ──────────────────────

    /// Feeds each moved contact its batch of samples.
    ///
    /// Returns whether the hovering pointer was among them, which decides whether hover is
    /// resolved this tick.
    ///
    /// # Errors
    ///
    /// The platform refused a batch, or a recogniser refused the samples in one.
    fn feed(&mut self, hits: &HitTable, env: Env, out: &mut Vec<Report>) -> Result<bool> {
        self.bell.moved_into(&mut self.moved);
        let hovering = self.bell.hovering();
        let coords = self.bell.coords();
        let mut hovering_moved = false;
        for index in 0..self.moved.len() {
            let id = self.moved[index];
            if Some(id) == hovering && !self.bell.is_down(id) {
                hovering_moved = true;
            }
            let Some((target, _)) = self.contacts.bound(id) else {
                continue;
            };
            if self.contacts.flags(id).has(Flags::REJECTED) {
                continue;
            }
            // `ProcessMoveEvents` takes the intermediate points, so a drag consumes every
            // sample in the batch, in order, rather than the one a message happened to carry.
            let batch = PointerPoint::GetIntermediatePointsTransformed(id, self.bell.transform())?;
            self.contacts.feed(id, Feed::Moves(&batch))?;

            // The drag policy folds the whole batch into one report. The axis a two-axis drag
            // locks to is a threshold crossing on the path, so deciding it from the newest
            // sample alone can lock to the wrong axis when an earlier sample crossed the other
            // way. Displacement is a state, so the fold reports it once.
            //
            // The predicted position is fed, not the raw one: continuous motion carries the
            // system's latency compensation.
            let count = coords.batch(id, &mut self.history);
            self.census.deepest_batch = self.census.deepest_batch.max(count as u32);
            let mut update: Option<DragUpdate> = None;
            let mut newest = None;
            for entry in 0..count {
                let sample = coords.sample(&self.history[entry], env);
                if let Some(step) = self.contacts.drag(id, sample.at) {
                    // `decided` is sticky across the fold: the tick that contains the crossing
                    // is the tick that reports it, whichever sample crossed.
                    update = Some(DragUpdate {
                        decided: step.decided || update.is_some_and(|held| held.decided),
                        ..step
                    });
                }
                newest = Some(sample);
            }
            if let Some(update) = update {
                out.push(Report::Dragged {
                    target,
                    contact: id,
                    update,
                });
            }
            // Pressure, tilt, twist and the contact patch reach a gesture sink here and
            // nowhere else: a manipulation's own events carry a position alone. They are
            // state, so the newest reading is the whole answer.
            if let Some(sample) = newest {
                out.push(Report::Moved {
                    target,
                    contact: id,
                    sample,
                });
            }
            self.collect(id, hits, out);
        }
        Ok(hovering_moved)
    }

    /// States the pivot contact `id` rotates about, resolved from `target`'s entry.
    ///
    /// A refusal leaves the recogniser on its last pivot, which is the previous frame's
    /// centre: the next update states it again, so a refused call costs one frame of drift
    /// rather than a gesture.
    fn restate_pivot(&self, id: u32, target: ControlId, hits: &HitTable) {
        let Some((_, decl)) = self.contacts.bound(id) else {
            return;
        };
        _ = self.contacts.restate_pivot(id, pivot_of(hits, target, decl));
    }

    /// Drains the events the recogniser raised for `id` and appends them to `out`.
    ///
    /// Called immediately after each feed: the platform raises these synchronously from inside
    /// `ProcessDownEvent` and its siblings, so the binding is the one just fed and no event
    /// has to carry its own routing.
    fn collect(&mut self, id: u32, hits: &HitTable, out: &mut Vec<Report>) {
        self.events.drain(&mut self.recognised);
        let Some((target, _)) = self.contacts.bound(id) else {
            self.recognised.clear();
            return;
        };
        // Applied after the walk, because a completion and a start can both be in one batch
        // and the last one is what the contact is left in.
        let mut inertial = None;
        for index in 0..self.recognised.len() {
            let event = self.recognised[index];
            match event {
                Recognised::Tapped { .. } => self.contacts.set(id, Flags::TAPPED, true),
                // Restated per update rather than set once at down: the platform requires both
                // pivot values to stay current through the interaction, and the control the
                // contact is on moves under it whenever its viewport scrolls.
                Recognised::ManipulationStarted { .. } | Recognised::ManipulationUpdated { .. } => {
                    self.restate_pivot(id, target, hits);
                }
                Recognised::InertiaStarting { .. } => inertial = Some(true),
                Recognised::ManipulationCompleted { .. } => inertial = Some(false),
                _ => {}
            }
            self.census.gestures += 1;
            out.push(Report::Gesture {
                target,
                contact: id,
                event,
            });
        }
        self.recognised.clear();
        if let Some(on) = inertial {
            self.contacts.set(id, Flags::INERTIAL, on);
        }
    }

    // ── 3. one hover resolution ───────────────────────────────────────────────────

    /// Resolves hover across every sample the pointer produced, in order, at most once per
    /// tick.
    ///
    /// Runs only when the hovering pointer moved or the layout changed under it, and never
    /// while a contact is down: a contact owns the pointer while it is down, so hover chrome
    /// must not chase a drag.
    ///
    /// Hover is the accumulated result of enter and leave events, which are boundary crossings
    /// on a path. A path that crosses a target between two samples has a real enter and a real
    /// leave that point-sampling cannot see, and once dropped here they cannot be recovered
    /// above. So the batch is walked and every crossing published.
    ///
    /// Each target is chosen from the sample's raw position rather than its predicted one, so
    /// an extrapolated point cannot select the wrong target. Nothing here constructs a
    /// `PointerPoint`, so this per-frame path allocates nothing.
    fn resolve_hover(&mut self, hits: &HitTable, env: Env, out: &mut Vec<Report>, moved: bool) {
        // The contact table covers a bound contact; `is_down` covers one already handed to a
        // scroll surface's tracker.
        if self.contacts.live() > 0 {
            return;
        }
        if self.bell.hovering().is_some_and(|id| self.bell.is_down(id)) {
            return;
        }
        let Some(id) = self.bell.hovering() else {
            if let Some(from) = self.hover.take() {
                self.census.hover_changes += 1;
                out.push(Report::HoverChanged {
                    from: Some(from),
                    to: None,
                    at: Point { x: 0.0, y: 0.0 },
                    qpc: 0,
                });
            }
            self.stale = false;
            return;
        };
        if !moved && !self.stale && self.hover.is_some() {
            return;
        }
        self.stale = false;
        let coords = self.bell.coords();
        let count = coords.batch(id, &mut self.history);
        self.census.deepest_batch = self.census.deepest_batch.max(count as u32);
        for entry in 0..count {
            let sample = coords.sample(&self.history[entry], env);
            self.cross(hits, &sample, out);
        }
        // A pointer whose history has aged out still has a current position, so an empty batch
        // falls back to the newest sample rather than dropping the hover for this tick.
        if count == 0
            && let Some(sample) = coords.newest(id, env)
        {
            self.cross(hits, &sample, out);
        }
    }

    /// Resolves one sample against the hit array, publishing a crossing where the target
    /// differs from the current hover.
    fn cross(&mut self, hits: &HitTable, sample: &Sample, out: &mut Vec<Report>) {
        self.census.hover_hits += 1;
        let to = hits
            .hit(sample.raw, sample.kind())
            .map(|hit| hit.id);
        if to == self.hover {
            return;
        }
        let from = self.hover;
        self.hover = to;
        self.census.hover_changes += 1;
        out.push(Report::HoverChanged {
            from,
            to,
            at: sample.raw,
            qpc: sample.qpc,
        });
    }

    // ── 4. the dial ───────────────────────────────────────────────────────────────

    /// Drains the dial's rotations and publishes them.
    ///
    /// A dial contact routes through the same hit array a finger does: an on-screen dial
    /// resting over a knob targets that knob, and a dial with no screen contact targets
    /// whatever has focus. The rotary path is therefore not a second routing authority.
    fn rotate(&mut self, hits: &HitTable, out: &mut Vec<Report>) {
        let Some(rotary) = self.rotary.as_ref() else {
            return;
        };
        rotary.drain(&mut self.rotations);
        for index in 0..self.rotations.len() {
            let rotation = self.rotations[index];
            let target = match rotation {
                Rotation::Turned { at: Some(at), .. } | Rotation::Clicked { at: Some(at) } => hits
                    .hit(at, ContactKind::Touch)
                    .map(|hit| hit.id),
                _ => self.focus.keyboard(),
            };
            match rotation {
                Rotation::Turned { degrees, .. } => {
                    let decl = target
                        .and_then(|id| self.decls.get(&id))
                        .and_then(|decl| decl.rotary);
                    // Restated per target rather than once at attach: the dial is one device
                    // serving every knob on the screen, and the resolution is the step the
                    // user feels as well as the one they get.
                    if let Some(decl) = decl {
                        _ = rotary.tune(&decl);
                    }
                    let steps = match decl {
                        Some(decl) if decl.resolution_degrees.abs() > f64::EPSILON => {
                            degrees / decl.resolution_degrees
                        }
                        _ => 0.0,
                    };
                    out.push(Report::Rotary {
                        target,
                        degrees,
                        steps,
                    });
                }
                Rotation::Button { pressed } => out.push(Report::RotaryButton { target, pressed }),
                Rotation::Clicked { .. } => {
                    out.push(Report::RotaryButton {
                        target,
                        pressed: true,
                    });
                    out.push(Report::RotaryButton {
                        target,
                        pressed: false,
                    });
                }
            }
        }
        self.rotations.clear();
    }

    // ── 5. inertia, on the same clock as everything else ──────────────────────────

    /// Advances inertia one frame on every contact still in it, releasing the ones that
    /// stopped.
    ///
    /// # Errors
    ///
    /// A recogniser refused to advance its inertia.
    fn pump(&mut self, hits: &HitTable, out: &mut Vec<Report>) -> Result<()> {
        if self.bell.take_stop_inertia() {
            // A system stop request ends every running motion without committing what it was
            // on its way to.
            for id in self.contacts.ids() {
                if id != 0 && self.contacts.flags(id).has(Flags::INERTIAL) {
                    self.contacts.release(id, true);
                    self.census.aborts += 1;
                }
            }
            return Ok(());
        }
        for id in self.contacts.ids() {
            if id == 0 || !self.contacts.flags(id).has(Flags::INERTIAL) {
                continue;
            }
            self.contacts.feed(id, Feed::Inertia)?;
            self.collect(id, hits, out);
            // A recogniser whose inertia has run out has nothing left to pump; the contact
            // behind it lifted earlier.
            if !self.contacts.still_inertial(id) {
                self.contacts.release(id, false);
            }
        }
        Ok(())
    }
}

impl core::fmt::Debug for Router {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Router")
            .field("census", &self.census)
            .field("factor", &self.measured_factor())
            .field("hover", &self.hover)
            .field("capture", &self.capture)
            .field("inertia", &self.inertia)
            .finish_non_exhaustive()
    }
}
