//! The pointer stack: pointer, keyboard and dial messages in, resolved [`Report`]s out on
//! the frame clock.
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
//! * A pointer message signals that samples are available; it does not carry them. The
//!   samples live in a system-side history ring addressed by pointer id, so a consumer that
//!   reads the ring once per frame keeps the intermediate samples legacy coalescing
//!   discards.
//! * Hover is a per-frame quantity and a manipulation is an integrated one. A hover state
//!   between two presents is not observable, so hover resolves once per frame; every sample
//!   of a manipulation contributes, so all of them are consumed. The split is at the
//!   consumption point, not at the message.
//! * A discrete transition is neither. A press, a release, a cancel, a wheel notch and a
//!   keystroke ask to be serviced on the next pump iteration by posting the pacer's own
//!   message, because waiting for the display would add a frame of latency to each. Motion
//!   waits for the frame clock, because motion is per-frame.
//! * Resolving a contact costs a walk proportional to the node count, which the flat hit
//!   array in [`HitTable`] bounds.
//!
//! A tick is not a frame. Ticks are bounded by the frame clock *plus* the discrete input
//! rate, so anything genuinely per-frame is gated on
//! [`Wake::frames`](windows_window::Wake::frames) rather than on the tick. A driven run
//! measured 145 ticks against 134 display frames and 45 hover resolutions.
//!
//! # There is no legacy mouse path
//!
//! `DefWindowProc` promotes pointer input into legacy mouse messages, so every pointer arm
//! that carries a contact is handled and none falls through. Neither binding filter
//! generates `WM_MOUSEMOVE`, its relatives or `TrackMouseEvent`, so a legacy arm does not
//! compile.
//!
//! # The environment is stated, never held
//!
//! [`Router::tick`] takes an [`Env`] because the display's scale and its output transform
//! belong to the window and its monitor. A router holding its own copy is not told when the
//! window moves to another display, and every contact then resolves against the wrong pixel
//! grid.

mod capability;
mod coords;
mod doorbell;
mod dynamic;
mod focus;
mod inertia;
mod sample;
mod service;

pub use capability::{Capability, Devices, Interaction};
pub use coords::{Coords, PointerSpace, Unit};
pub use doorbell::{
    Doorbell, DoorbellHealth, EventKind, InputEvent, KeyEvent, KeyKind, Mods, PointerEvent,
    PointerFlags, PointerType,
};
pub use dynamic::Late;
pub use focus::{FocusRing, FocusScope, Move, ScopeId};
pub use inertia::Inertia;
pub use sample::{Pen, Reader, Sample};
pub use service::Service;

use crate::bindings::*;
use crate::gesture::{DragUpdate, Events, GestureDecl, Recognised, RecognizerPool};
use crate::rotary::{Rotary, Rotation};
use rustc_hash::FxHashMap;
use std::rc::Rc;
use windows_core::{ComObject, Result};
use windows_scene::{ControlId, Env, HitFlags, HitTable, Point};

/// How many coalesced entries one service reads back.
///
/// A pointer's `historyCount` is bounded by how far behind the consumer fell; 128 is several
/// frames of a 1 kHz digitizer. A deeper batch means the pump never reached the frames that
/// produced it.
const HISTORY_MAX: usize = 128;
use windows_window::{Tick, Wake, Window};

/// Reports one outcome of a tick, published in the order it happened.
///
/// Every variant is resolved on the front thread, and the layer above turns a report into
/// pixels before it turns it into an [`Intent`](crate::gesture::Intent). An intent therefore
/// exists only after the visual it belongs to, and can never be the cause of one.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Report {
    /// A hover boundary was crossed. One tick can publish several, in the order the pointer
    /// crossed them: a fast flick over a toolbar publishes every crossing on the path, and
    /// the layer that owns the chrome decides which of them light anything.
    HoverChanged {
        from: Option<ControlId>,
        to: Option<ControlId>,
        /// Where the crossing happened, in client DIPs.
        at: Point,
        /// The performance-counter value the sample was stamped with, or zero where it
        /// carried none. A dwell is measured against this rather than against a tick count.
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
        /// target was chosen from, and it carries the pen's pressure and tilt and the
        /// measured contact patch.
        sample: Sample,
        buttons: u32,
    },
    /// A bound contact moved. Carries the pen's pressure, tilt and twist, which a
    /// recogniser's own events do not: those carry a position alone.
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
    /// does not reach here: `PointerWheelConfig` routes it to that container's tracker on
    /// the compositor side, with no front-thread work.
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
    /// Every contact was taken away — a lost capture, or the window losing focus.
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
    /// point-sampling consumer would have dropped. `SendInput` does not outrun a healthy
    /// pump, so injected input rarely raises it; a 1 kHz mouse against a loaded front thread
    /// does.
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
/// Runs on the front thread: it owns recognisers, which are non-agile, and it resolves
/// through the retained tree's hit array.
pub struct Router {
    bell: Rc<Doorbell>,
    hwnd: HWND,
    coords: Coords,
    reader: Reader,
    /// The space the recogniser is asked to report in, and the interface handle it is handed.
    space: ComObject<PointerSpace>,
    transform: IPointerPointTransform,
    pool: RecognizerPool,
    /// A clone of the pool's queue, so draining it does not conflict with iterating the pool.
    events: Events,
    focus: FocusRing,
    /// Which gestures each target accepts. Resolved on the front thread, so deciding whether
    /// a gesture applies makes no call to the application thread.
    decls: FxHashMap<ControlId, GestureDecl>,
    hover: Option<ControlId>,
    /// Set when the hover answer may have changed without the pointer moving — a layout
    /// change under a stationary cursor.
    hover_stale: bool,
    /// The environment the last tick ran under. Read only by the comparison that decides what
    /// a change to the environment invalidated; it is never answered as the current scale.
    env: Option<Env>,
    /// The window's dial. `None` means no radial controller is attached.
    rotary: Option<Rotary>,
    rotations: Vec<Rotation>,
    /// The contact that owns input while it is down.
    capture: Option<u32>,
    inertia: Inertia,
    capability: Capability,
    wake: Wake,
    /// Held while a gesture or inertia is live. The doorbell holds its own for the ring.
    running: Option<Tick>,
    // ── scratch, so a frame allocates nothing after the first ─────────────────────
    moved: Vec<u32>,
    recognised: Vec<Recognised>,
    /// The coalesced-history buffer, allocated once. `POINTER_INFO` is ~100 bytes, so this
    /// holds ~13 KB for the window's life and a contact's motion allocates nothing.
    history: Vec<POINTER_INFO>,
    census: InputCensus,
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
    /// Returns an error if `window` is closed, leaving no handle to resolve against.
    pub fn new(bell: &Rc<Doorbell>, window: &Window, wake: Wake) -> Result<Self> {
        if !window.is_open() {
            return Err(windows_core::Error::new(
                windows_core::HRESULT(0x8007_0006u32 as i32),
                "the window is closed",
            ));
        }
        let hwnd = window.hwnd();
        let late = Late::resolve();
        let space = ComObject::new(PointerSpace::new());
        let transform = space.to_interface();
        let pool = RecognizerPool::new();
        // The queue every recogniser in the pool is wired to. Cloned so that draining it and
        // iterating the pool do not borrow one field twice; the queue is an `Rc` inside, so
        // both names refer to it.
        let events = pool.events().clone();
        bell.pace(window, wake.clone());
        Ok(Self {
            bell: Rc::clone(bell),
            hwnd,
            coords: Coords::new(hwnd),
            reader: Reader::new(late),
            space,
            transform,
            pool,
            events,
            focus: FocusRing::default(),
            decls: FxHashMap::default(),
            hover: None,
            hover_stale: false,
            env: None,
            rotary: None,
            rotations: Vec::with_capacity(8),
            capture: None,
            inertia: Inertia::new(window, late),
            capability: Capability::read(window, &late),
            wake,
            running: None,
            moved: Vec::with_capacity(16),
            recognised: Vec::with_capacity(32),
            history: vec![POINTER_INFO::default(); HISTORY_MAX],
            census: InputCensus::default(),
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

    /// Attaches the window's radial controller.
    ///
    /// `Ok(true)` means the controller object exists, not that a dial is present:
    /// `CreateForWindow` succeeds on a machine with no wheel attached and the object then
    /// never raises anything, so a dial plugged in later needs no re-attach.
    ///
    /// # Errors
    ///
    /// Returns an error if the interop factory refused the window.
    pub fn attach_rotary(&mut self, window: &Window) -> Result<bool> {
        match Rotary::new(window, self.bell.service()) {
            Ok(rotary) => {
                self.rotary = Some(rotary);
                Ok(true)
            }
            // `CreateForWindow` answers this when no controller is available. Distinguished
            // from a real refusal so a missing dial does not read as a broken one.
            Err(error) if error.code() == windows_core::HRESULT(0x8007_0490u32 as i32) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Returns `true` once a radial controller is attached.
    #[must_use]
    pub const fn has_rotary(&self) -> bool {
        self.rotary.is_some()
    }

    /// Returns the unit the WinRT pointer statics were measured to answer in.
    #[must_use]
    pub fn measured_unit(&self) -> Unit {
        self.space.unit()
    }

    /// Returns the focus ring. Focus order is the hit array's, filtered to `INTERACTIVE`.
    #[must_use]
    pub const fn focus(&self) -> &FocusRing {
        &self.focus
    }

    /// Returns the focus ring for mutation, as an overlay opens or closes a scope.
    pub const fn focus_mut(&mut self) -> &mut FocusRing {
        &mut self.focus
    }

    /// Records which gestures `target` accepts. Called as the widget mounts.
    pub fn declare(&mut self, target: ControlId, decl: GestureDecl) {
        self.decls.insert(target, decl);
    }

    /// Drops `target`'s declaration, on unmount. Any contact still bound to it aborts,
    /// because a gesture whose target has gone cannot commit anything.
    pub fn forget(&mut self, target: ControlId) {
        self.decls.remove(&target);
        if self.pool.holds(target) {
            let ids: Vec<u32> = self
                .pool
                .iter_mut()
                .filter(|(_, bound)| bound.target == target)
                .map(|(id, _)| id)
                .collect();
            for id in ids {
                self.pool.release(id, true);
                self.census.aborts += 1;
            }
        }
        self.focus.clear_tab_index(target);
    }

    /// Marks the hover answer stale, for a layout change under a stationary pointer.
    ///
    /// Costs one hit test on the next tick and nothing at idle. Content that moves under a
    /// resting pointer changes its hover only through this call.
    pub fn invalidate_hover(&mut self) {
        self.hover_stale = true;
    }

    /// Stops content inertia at the system's request.
    ///
    /// No message arm reaches this: `WM_STOPINERTIA` is absent from the SDK the bindings are
    /// generated from, so there is no constant to match on. [`Inertia`] drives it instead.
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
    /// Returns an error propagated from a platform pointer or recogniser call. Whatever was
    /// resolved before it is already in `out`.
    pub fn tick(&mut self, hits: &HitTable, env: Env, out: &mut Vec<Report>) -> Result<()> {
        self.census.ticks += 1;
        self.sync(env);
        // Re-opened before the drain, so a transition arriving during it asks for another
        // service rather than being swallowed. A gate re-opened after the drain stays shut
        // over that window, and the symptom is input that silently stops.
        self.bell.begin();
        self.drain(hits, env, out)?;
        let hover_moved = self.feed(hits, env, out)?;
        self.resolve_hover(hits, env, out, hover_moved);
        self.rotate(hits, env, out);
        self.pump(out)?;
        self.settle();
        Ok(())
    }

    /// Brings the router up to date with `env` and keeps it as the next tick's watermark.
    ///
    /// Any change to the environment makes the hover answer stale. A scale change also puts
    /// every contact on a different pixel grid, so the measured recogniser factor is
    /// discarded and the next contact measures it again.
    fn sync(&mut self, env: Env) {
        let Some(last) = self.env.replace(env) else {
            return;
        };
        if last == env {
            return;
        }
        self.hover_stale = true;
        if last.scale() != env.scale() {
            self.space.forget();
        }
    }

    // ── 1. the ring, in order ─────────────────────────────────────────────────────

    fn drain(&mut self, hits: &HitTable, env: Env, out: &mut Vec<Report>) -> Result<()> {
        while let Some(event) = self.bell.pop() {
            match event {
                InputEvent::Pointer(event) => self.pointer(event, hits, env, out)?,
                InputEvent::Key(event) => self.key(event, hits, out),
            }
        }
        Ok(())
    }

    fn pointer(
        &mut self,
        event: PointerEvent,
        hits: &HitTable,
        env: Env,
        out: &mut Vec<Report>,
    ) -> Result<()> {
        match event.kind {
            EventKind::Down => self.down(event, hits, env, out),
            EventKind::Button => {
                if let Some(bound) = self.pool.get(event.id) {
                    out.push(Report::Buttons {
                        target: bound.target,
                        contact: event.id,
                        buttons: event.buttons,
                    });
                }
                Ok(())
            }
            EventKind::Up => self.up(event, env, out),
            EventKind::Cancel => {
                self.abort(event.id, out);
                Ok(())
            }
            // Releasing capture on an up posts this message back, so a loss with nothing
            // bound is ignored: treating it as a cancel would abort the gesture that had
            // just completed normally.
            EventKind::CaptureLost if self.pool.live() > 0 || self.capture.is_some() => {
                self.pool.release_all(true);
                self.census.aborts += 1;
                self.capture = None;
                self.events.clear();
                out.push(Report::CaptureLost);
                Ok(())
            }
            EventKind::CaptureLost => Ok(()),
            EventKind::Wheel => self.wheel(event, hits, env, out),
        }
    }

    fn down(
        &mut self,
        event: PointerEvent,
        hits: &HitTable,
        env: Env,
        out: &mut Vec<Report>,
    ) -> Result<()> {
        // Built from the point the doorbell recorded at the message, not from wherever the
        // contact has since moved to.
        let pressed = self.reader.at_transition(&event, &self.coords, env);
        let at = pressed.raw;
        self.census.discrete_hits += 1;
        let Some(hit) = hits.hit(at, event.ptype.contact()) else {
            // A press on nothing still takes focus away, so clicking the background dismisses
            // a text caret.
            if let Some((from, to)) = self.focus.focus(None) {
                out.push(Report::FocusChanged { from, to });
            }
            return Ok(());
        };

        // An overlay's blocker consumes the press outright. Nothing under it is pressed, no
        // focus moves, and the overlay's owner decides what closing means.
        if hit.flags.contains(HitFlags::BLOCKER) {
            out.push(Report::Dismiss {
                blocker: hit.id,
                scope: self.focus.innermost().map(|(id, _)| id),
            });
            return Ok(());
        }

        if let Some((from, to)) = self.focus.focus(Some(hit.id)) {
            out.push(Report::FocusChanged { from, to });
        }

        out.push(Report::Pressed {
            target: hit.id,
            contact: event.id,
            sample: pressed,
            buttons: event.buttons,
        });

        // A contact the digitizer is not confident about is a palm. It is still tracked — its
        // up has to be accounted for — but nothing is fed to a recogniser, so it can never
        // start a gesture.
        let rejected = event.ptype == PointerType::Touch && !event.flags.confident();
        if rejected {
            self.census.rejected += 1;
        }

        // A contact is bound whatever its target declared: a declaration says which gestures
        // a press can become, not whether the press happened. Skipping the bind for a target
        // that declared nothing would leave it with a press and no release, which latches its
        // press wash, holds its pool slot for the life of the window, and loses the tap,
        // because a tap is a press and a release on one control.
        let decl = self.decls.get(&hit.id).copied().unwrap_or_default();
        // A scroll surface hands touch to its `InteractionTracker`, which keeps a fling
        // running while the front thread is busy. The contact leaves this stack, so no up
        // arrives here for it; `resolve_hover` reads the doorbell's own down state to cover
        // that case.
        if decl.redirect {
            return Ok(());
        }

        self.capture = Some(event.id);
        // A contact routes to its down-window for its life. Mouse is the one device that can
        // leave the window without lifting, so capture is what keeps it routed here.
        if event.ptype == PointerType::Mouse {
            // SAFETY: `SetCapture` takes the window handle by value and writes through no
            // pointer; a handle whose window has been destroyed fails the call rather than
            // being dereferenced.
            unsafe {
                _ = SetCapture(self.hwnd);
            }
        }

        self.pool
            .bind(event.id, event.ptype, hit.id, decl, at, rejected)?;
        self.census.bindings += 1;
        if !rejected {
            // Measured before the point that is fed, so the first gesture of a session is
            // transformed by a factor that was read rather than assumed. Both readings are of
            // one contact: `raw` through the platform's space and `at` through this crate's
            // screen-to-client conversion, so their ratio is the conversion between the two
            // spaces and nothing else.
            if self.space.unit() == Unit::Unmeasured
                && let Ok(raw) =
                    PointerPoint::GetCurrentPoint(event.id).and_then(|point| point.RawPosition())
            {
                self.space.calibrate(Point { x: raw.x, y: raw.y }, at);
            }
            let point = PointerPoint::GetCurrentPointTransformed(event.id, &self.transform)?;
            if let Some(bound) = self.pool.get(event.id) {
                bound.recognizer().down(&point)?;
            }
            self.collect(event.id, out);
        }
        Ok(())
    }

    fn up(&mut self, event: PointerEvent, env: Env, out: &mut Vec<Report>) -> Result<()> {
        let at = self.coords.client(env, event.id, event.x_px, event.y_px);
        if self.capture == Some(event.id) {
            self.capture = None;
            if event.ptype == PointerType::Mouse {
                // SAFETY: `ReleaseCapture` takes no argument and writes through no pointer.
                unsafe {
                    _ = ReleaseCapture();
                }
            }
        }
        let Some(bound) = self.pool.get(event.id) else {
            self.bell.release(event.id);
            return Ok(());
        };
        let (target, rejected) = (bound.target, bound.rejected);
        // Published before the recogniser is told, as the press is. The release clears the
        // press wash, frees the binding and completes a tap, so a recogniser that refuses the
        // final sample must not be able to suppress it.
        out.push(Report::Released {
            target,
            contact: event.id,
            at,
        });
        let fed = if rejected {
            Ok(())
        } else {
            PointerPoint::GetCurrentPointTransformed(event.id, &self.transform).and_then(|point| {
                self.pool
                    .get(event.id)
                    .map_or(Ok(()), |bound| bound.recognizer().up(&point))
            })
        };
        self.collect(event.id, out);
        // Inertia keeps the binding alive: the contact is gone but its motion is not, and the
        // recogniser running that motion is the one being pumped.
        let inertial = self
            .pool
            .get(event.id)
            .is_some_and(|bound| bound.recognizer().is_inertial());
        if let Some(bound) = self.pool.get_mut(event.id) {
            bound.inertial = inertial;
        }
        if !inertial {
            self.pool.release(event.id, false);
        }
        self.bell.release(event.id);
        // Returned last, so a refused sample reaches the caller only after the contact has
        // been fully ended rather than leaving this stack still holding it.
        fed
    }

    /// Aborts the contact bound to `id`: the recogniser completes and nothing is committed.
    fn abort(&mut self, id: u32, out: &mut Vec<Report>) {
        if let Some(bound) = self.pool.get(id) {
            out.push(Report::Canceled {
                target: bound.target,
                contact: id,
            });
        }
        self.pool.release(id, true);
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

    fn wheel(
        &mut self,
        event: PointerEvent,
        hits: &HitTable,
        env: Env,
        out: &mut Vec<Report>,
    ) -> Result<()> {
        let at = self.coords.client(env, event.id, event.x_px, event.y_px);
        self.census.discrete_hits += 1;
        let hit = hits.hit(at, event.ptype.contact());
        // A scroll surface's wheel belongs to its tracker: the source's `PointerWheelConfig`
        // takes it, and handling it front-side here would be a second scroll path.
        if hit.is_some_and(|hit| hit.flags.contains(HitFlags::SCROLL)) {
            return Ok(());
        }
        if let Some(hit) = hit
            && let Some(bound) = self.pool.get(event.id)
            && bound.target == hit.id
        {
            let point = PointerPoint::GetCurrentPointTransformed(event.id, &self.transform)?;
            bound
                .recognizer()
                .wheel(&point, event.flags.buttons() & 2 != 0, false)?;
            self.collect(event.id, out);
            return Ok(());
        }
        out.push(Report::Wheel {
            target: hit.map(|hit| hit.id),
            at,
            notches: event.wheel as f32 / WHEEL_DELTA as f32,
            horizontal: event.horizontal,
        });
        Ok(())
    }

    fn key(&mut self, event: KeyEvent, hits: &HitTable, out: &mut Vec<Report>) {
        // Tab and Esc are taken before any control sees them: focus order has one authority,
        // and an open overlay closes from the keyboard wherever the pointer is.
        if event.kind == KeyKind::Down {
            match event.key as i32 {
                VK_TAB => {
                    match self.focus.step(hits, !event.mods.shift) {
                        Move::To { from, to } => {
                            out.push(Report::FocusChanged { from, to: Some(to) })
                        }
                        // Off the end of a scope that does not trap: dismiss it and let the
                        // owner step again outside.
                        Move::Left => out.push(Report::Escape {
                            scope: self.focus.innermost().map(|(id, _)| id),
                        }),
                        Move::Nowhere => {}
                    }
                    return;
                }
                VK_ESCAPE if self.focus.depth() > 0 => {
                    out.push(Report::Escape {
                        scope: self.focus.innermost().map(|(id, _)| id),
                    });
                    return;
                }
                _ => {}
            }
        }
        out.push(Report::Key {
            target: self.focus.current(),
            event,
        });
    }

    // ── 2. every intermediate sample of every active contact ──────────────────────

    /// Feeds each moved contact its batch of samples.
    ///
    /// Returns whether the hovering pointer was among them, which decides whether hover is
    /// resolved this tick.
    fn feed(&mut self, _hits: &HitTable, env: Env, out: &mut Vec<Report>) -> Result<bool> {
        let mut moved = core::mem::take(&mut self.moved);
        self.bell.take_moved(&mut moved);
        let hovering = self.bell.hovering();
        let mut hover_moved = false;

        for &id in &moved {
            if Some(id) == hovering {
                hover_moved = true;
            }
            let Some(bound) = self.pool.get(id) else {
                continue;
            };
            if bound.rejected {
                continue;
            }
            // `ProcessMoveEvents` takes the intermediate points, so a drag consumes every
            // sample in the batch, in order, rather than the one a message happened to carry.
            let batch = PointerPoint::GetIntermediatePointsTransformed(id, &self.transform)?;
            bound.recognizer().moves(&batch)?;

            // The drag policy folds the whole batch into one report. The axis a two-axis drag
            // locks to is a threshold crossing on the path, so deciding it from the newest
            // sample alone can lock to the wrong axis when an earlier sample crossed the
            // other way. Displacement is a state, so the fold reports it once.
            //
            // The predicted position is fed, not the raw one: continuous motion carries the
            // system's latency compensation.
            let mut history = core::mem::take(&mut self.history);
            let count = self.reader.batch(id, &mut history);
            self.census.deepest_batch = self.census.deepest_batch.max(count as u32);
            let mut update: Option<DragUpdate> = None;
            let mut newest = None;
            for info in &history[..count] {
                let sample = self.reader.sample(info, &self.coords, env);
                if let Some(bound) = self.pool.get_mut(id)
                    && let Some(drag) = bound.drag.as_mut()
                {
                    let step = drag.update(sample.at);
                    // `decided` is sticky across the fold: the tick that contains the
                    // crossing is the tick that reports it, whichever sample crossed.
                    update = Some(match update {
                        Some(previous) => DragUpdate {
                            decided: previous.decided || step.decided,
                            ..step
                        },
                        None => step,
                    });
                }
                newest = Some(sample);
            }
            self.history = history;

            if let (Some(update), Some(bound)) = (update, self.pool.get(id)) {
                out.push(Report::Dragged {
                    target: bound.target,
                    contact: id,
                    update,
                });
            }
            // Pressure, tilt, twist and the contact patch reach a gesture sink here and
            // nowhere else: a manipulation's own events carry a position alone. They are
            // state, so the newest reading is the whole answer.
            if let (Some(sample), Some(bound)) = (newest, self.pool.get(id)) {
                out.push(Report::Moved {
                    target: bound.target,
                    contact: id,
                    sample,
                });
            }
            self.collect(id, out);
        }

        moved.clear();
        self.moved = moved;
        Ok(hover_moved)
    }

    /// Drains the events the recogniser raised for `id` and appends them to `out`.
    ///
    /// Called immediately after each feed: the platform raises these synchronously from
    /// inside `ProcessDownEvent` and its siblings, so the binding is the one just fed and no
    /// event has to carry its own routing.
    fn collect(&mut self, id: u32, out: &mut Vec<Report>) {
        let mut recognised = core::mem::take(&mut self.recognised);
        self.events.drain(&mut recognised);
        let Some(bound) = self.pool.get_mut(id) else {
            recognised.clear();
            self.recognised = recognised;
            return;
        };
        let target = bound.target;
        for event in recognised.drain(..) {
            match event {
                Recognised::ManipulationStarted { .. } => bound.manipulating = true,
                Recognised::ManipulationUpdated { .. } => {
                    // Restated per update rather than set once at down: the platform requires
                    // both pivot values to stay current through the interaction.
                    if let Some(pivot) = bound.decl.pivot {
                        _ = bound.recognizer().pivot(pivot);
                    }
                }
                Recognised::InertiaStarting { .. } => bound.inertial = true,
                Recognised::ManipulationCompleted { .. } => {
                    bound.manipulating = false;
                    bound.inertial = false;
                }
                _ => {}
            }
            self.census.gestures += 1;
            out.push(Report::Gesture {
                target,
                contact: id,
                event,
            });
        }
        self.recognised = recognised;
    }

    // ── 3. one hover resolution ───────────────────────────────────────────────────

    /// Resolves hover across every sample the pointer produced, in order, at most once per
    /// tick.
    ///
    /// Runs only when the hovering pointer moved or the layout changed under it, and never
    /// while a contact is down.
    ///
    /// Hover is the accumulated result of enter and leave events, which are boundary
    /// crossings on a path. A path that crosses a target between two samples has a real enter
    /// and a real leave that point-sampling cannot see, and once dropped here they cannot be
    /// recovered above. So the batch is walked and every crossing published; whether a
    /// three-millisecond traversal lights anything is decided by the layer that owns the
    /// chrome.
    ///
    /// The cost is one history read in place of one current read, plus a memo-bounded scan of
    /// the flat hit array per entry.
    ///
    /// Each target is chosen from the sample's raw position rather than its predicted one, so
    /// an extrapolated point cannot select the wrong target. Nothing here constructs a
    /// `PointerPoint`, so this per-frame path allocates nothing.
    fn resolve_hover(&mut self, hits: &HitTable, env: Env, out: &mut Vec<Report>, moved: bool) {
        // A contact owns the pointer while it is down: hover chrome must not chase a drag.
        // `capture` covers a contact this router bound; `is_down` covers one it did not — a
        // touch contact redirected to a scroll surface's tracker.
        if self.capture.is_some() {
            return;
        }
        if self.bell.hovering().is_some_and(|id| self.bell.is_down(id)) {
            return;
        }
        let Some(id) = self.bell.hovering() else {
            if let Some(from) = self.hover.take() {
                out.push(Report::HoverChanged {
                    from: Some(from),
                    to: None,
                    at: Point { x: 0.0, y: 0.0 },
                    qpc: 0,
                });
                self.census.hover_changes += 1;
            }
            self.hover_stale = false;
            return;
        };
        if !moved && !self.hover_stale && self.hover.is_some() {
            return;
        }
        self.hover_stale = false;

        let mut history = core::mem::take(&mut self.history);
        let count = self.reader.batch(id, &mut history);
        self.census.deepest_batch = self.census.deepest_batch.max(count as u32);
        for entry in &history[..count] {
            let sample = self.reader.sample(entry, &self.coords, env);
            self.cross(hits, &sample, out);
        }
        // A pointer whose history has aged out still has a current position, so an empty
        // batch falls back to the newest sample rather than dropping the hover for this tick.
        if count == 0
            && let Some(sample) = self.reader.newest(id, &self.coords, env)
        {
            self.cross(hits, &sample, out);
        }
        self.history = history;
    }

    /// Resolves one sample against the hit array, publishing a [`Report::HoverChanged`] when
    /// the target differs from the current hover.
    fn cross(&mut self, hits: &HitTable, sample: &Sample, out: &mut Vec<Report>) {
        self.census.hover_hits += 1;
        let to = hits.hit(sample.raw, sample.kind()).map(|hit| hit.id);
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

    /// Drains the dial's rotations and publishes them.
    ///
    /// A dial contact routes through the same hit array a finger does: an on-screen dial
    /// resting over a knob targets that knob, and a dial with no screen contact targets
    /// whatever has focus. The rotary path is therefore not a second routing authority — it
    /// resolves through the same array and falls back to the same focus ring.
    fn rotate(&mut self, hits: &HitTable, _env: Env, out: &mut Vec<Report>) {
        let Some(rotary) = self.rotary.as_ref() else {
            return;
        };
        let mut rotations = core::mem::take(&mut self.rotations);
        rotary.events().drain(&mut rotations);

        for rotation in rotations.drain(..) {
            let (target, contact) = match rotation {
                Rotation::Turned { contact, .. }
                | Rotation::Button { contact, .. }
                | Rotation::Clicked { contact } => (contact, contact),
                Rotation::Contact { at } => (at, at),
                // Acquiring and losing the controller reports the device arriving and
                // leaving; nothing is targeted by it.
                Rotation::Control { .. } => continue,
            };
            let target = match target {
                Some(at) => hits
                    .hit(at, windows_scene::ContactKind::Touch)
                    .map(|h| h.id),
                None => self.focus.current(),
            };
            _ = contact;

            match rotation {
                Rotation::Turned { degrees, .. } => {
                    // The step is the target's declared resolution, so a knob whose detents
                    // are two units apart moves by two per click and the haptics match.
                    let decl = target
                        .and_then(|id| self.decls.get(&id))
                        .and_then(|d| d.rotary);
                    let steps = match decl {
                        Some(decl) if decl.resolution_degrees.abs() > f64::EPSILON => {
                            degrees / decl.resolution_degrees
                        }
                        _ => 0.0,
                    };
                    if let Some(decl) = decl {
                        // Restated per target rather than once at attach: the dial is one
                        // device serving every knob on the screen.
                        _ = rotary.tune(&decl);
                    }
                    out.push(Report::Rotary {
                        target,
                        degrees,
                        steps,
                    });
                }
                Rotation::Button { pressed, .. } => {
                    out.push(Report::RotaryButton { target, pressed });
                }
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
                Rotation::Contact { .. } | Rotation::Control { .. } => {}
            }
        }

        rotations.clear();
        self.rotations = rotations;
    }

    // ── 4. inertia, on the same clock as everything else ──────────────────────────

    fn pump(&mut self, out: &mut Vec<Report>) -> Result<()> {
        if self.bell.take_stop_inertia() {
            // A system stop request ends every running motion without committing what it was
            // on its way to.
            let ids: Vec<u32> = self
                .pool
                .iter_mut()
                .filter(|(_, bound)| bound.inertial)
                .map(|(id, _)| id)
                .collect();
            for id in ids {
                self.pool.release(id, true);
                self.census.aborts += 1;
            }
        }

        let inertial: Vec<u32> = self
            .pool
            .iter_mut()
            .filter(|(_, bound)| bound.inertial)
            .map(|(id, _)| id)
            .collect();
        for id in inertial {
            if let Some(bound) = self.pool.get(id) {
                bound.recognizer().inertia()?;
            }
            self.collect(id, out);
            // A recogniser whose inertia has run out has nothing left to pump; the contact
            // behind it lifted earlier.
            let done = self
                .pool
                .get(id)
                .is_some_and(|bound| !bound.recognizer().is_inertial());
            if done {
                self.pool.release(id, false);
            }
        }
        Ok(())
    }

    // ── 5. requesting the next tick ───────────────────────────────────────────────

    fn settle(&mut self) {
        let busy = self.pool.live() > 0;
        if busy {
            if self.running.is_none() {
                self.running = Some(self.wake.tick());
            }
        } else {
            self.running = None;
        }
        if self.bell.idle() {
            self.bell.settle();
        }
        // A window whose content is moving must say so, or a touchpad tap lands on whatever
        // was moving under it. A refusal is retried by the next tick, so the result is not
        // acted on here.
        _ = self.inertia.set(self.pool.any_inertial());
    }
}

impl core::fmt::Debug for Router {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Router")
            .field("census", &self.census)
            .field("unit", &self.measured_unit())
            .field("hover", &self.hover)
            .field("capture", &self.capture)
            .field("inertia", &self.inertia)
            .finish_non_exhaustive()
    }
}
