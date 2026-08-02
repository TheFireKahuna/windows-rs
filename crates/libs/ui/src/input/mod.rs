//! The pointer stack.
//!
//! ```text
//!  front thread — WndProc                     front thread — service tick
//!  ───────────────────────────────            ────────────────────────────────────────
//!  WM_POINTERDOWN / UP / cancel   ─ring──▶    1. drain ring in order → flat hit test
//!  WM_POINTERWHEEL / HWHEEL       ─ring──▶    2. active contact?
//!  WM_KEY* / WM_CHAR              ─ring──▶         batch → ProcessMoveEvents
//!        └─ and post WM_FRAME now                  (recogniser events → gesture sinks)
//!                                             3. once per DISPLAY frame: newest RAW
//!  WM_POINTERUPDATE               ─flag──▶       sample → exactly ONE hover hit test
//!  WM_POINTERENTER / LEAVE        ─flag──▶    4. drain the dial
//!        └─ and ask for a frame                5. ProcessInertia() per inertial recogniser
//!                                             6. return — the work item publishes
//! ```
//!
//! Four insights this rests on:
//!
//! * **A pointer message is a doorbell, not a datum.** The samples live in a system-side
//!   history ring addressed by pointer id, so a consumer that reads it once per frame loses
//!   nothing and *gains* the intermediate samples legacy coalescing discards.
//! * **Hover is a per-frame quantity; a manipulation is an integrated one.** No user can
//!   observe a hover state between two presents, so resolving hover more than once per frame
//!   is provably wasted. A manipulation is the opposite — every sample contributes. That is
//!   why the split is at the *consumption point* and not at the message.
//! * **A discrete transition is neither.** It does not batch and it is not per-frame, so
//!   making a press wait for the display is a frame of latency bought for nothing —
//!   frame-limiting input is the opposite of a low-latency design. A press, a release, a
//!   cancel, a wheel notch and a keystroke therefore ask to be serviced on the **next pump
//!   iteration**: they post the pacer's own message rather than waiting for it. Motion does
//!   not, because motion *is* per-frame.
//! * **The remaining cost is a data-structure problem.** No API makes an O(nodes) walk
//!   cheaper; the flat hit array does.
//!
//! One consequence is worth stating because it is easy to get wrong: **a tick is no longer
//! the same thing as a frame.** Ticks are bounded by the frame clock *plus* discrete input
//! rate, so anything genuinely per-frame is gated on
//! [`Wake::frames`](windows_window::Wake::frames) rather than on the tick. Measured over a
//! driven run: 145 ticks against 134 display frames, and 45 hover resolutions.
//!
//! # There is no legacy mouse path
//!
//! `DefWindowProc` is what promotes pointer input into legacy mouse messages, so suppressing
//! them is an act rather than an omission: every pointer arm that carries a contact is
//! handled and none falls through. That is not a rule to remember — neither binding filter
//! generates `WM_MOUSEMOVE`, its relatives or `TrackMouseEvent`, so a legacy arm is a
//! compile error.
//!
//! # The environment is stated, never held
//!
//! [`Router::tick`] takes an [`Env`] because the display's scale and its output transform
//! belong to the window and its monitor. A router that cached them could be *not told* when
//! the window hops a display — silently, for the rest of the session — and every contact
//! would resolve against the wrong pixel grid. Same rule, same reason, as the scene's.

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
/// frames of a 1 kHz digitizer. A batch deeper than this is one the pump never reached, and
/// the oldest entries are the ones that matter least.
const HISTORY_MAX: usize = 128;
use windows_window::{Tick, Wake, Window};

/// What one frame of input resolved to, in the order it happened.
///
/// These are **front-thread facts**, and the layer above turns them into pixels before it
/// turns any of them into an [`Intent`](crate::gesture::Intent). That ordering is what makes
/// "no intent may be the cause of a visual" structural rather than a rule to remember: by the
/// time an intent exists, the visual has already happened.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Report {
    /// A boundary was crossed. **More than one of these can arrive from a single service**,
    /// in the order the pointer crossed them: a fast flick over a toolbar produces the whole
    /// sequence, and the layer that owns the chrome decides what a three-millisecond
    /// traversal means. Nothing above can make that decision if this layer samples.
    HoverChanged {
        from: Option<ControlId>,
        to: Option<ControlId>,
        /// Where the crossing happened, in client DIPs.
        at: Point,
        /// The performance counter the crossing was stamped with; zero where the sample
        /// carried none. What a dwell is measured against, rather than counting services.
        qpc: u64,
    },
    FocusChanged {
        from: Option<ControlId>,
        to: Option<ControlId>,
    },
    Pressed {
        target: ControlId,
        contact: u32,
        /// The contact as it was **at the message**, not as it is now. Its `raw` is what the
        /// target was chosen from, and it carries the pen's pressure and tilt and the honest
        /// contact patch — a press is where a sink learns what pressed it.
        sample: Sample,
        buttons: u32,
    },
    /// A bound contact moved. Where pen pressure, tilt and twist reach a gesture sink: the
    /// recogniser's own events carry a position and nothing about the thing making it.
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
    /// **Not a release.** The pre-drag value is restored and nothing is committed.
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
    /// A wheel notch over a target that is **not** a scroll surface. A scroll container's
    /// wheel never reaches here: `PointerWheelConfig` routes it to its tracker, compositor
    /// side, with no front-thread work at all.
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
    /// A press landed on an overlay's blocker. **The press is consumed**: dismiss-and-act
    /// would make an accidental menu open cost an unintended edit.
    Dismiss {
        blocker: ControlId,
        scope: Option<ScopeId>,
    },
    /// The dial turned. A delta source, so it lands on the gesture seam and drives the same
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

/// What the pointer stack did. The counters contract tests 3.2, 3.3 and 3.6 read.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct InputCensus {
    pub ticks: u64,
    /// Samples examined to resolve hover. Bounded by the pointer's own report rate rather
    /// than by the frame clock — every sample is looked at, because a crossing between two
    /// of them is an event and not a state.
    pub hover_hits: u64,
    /// Boundary crossings published. **This** is the count bounded by what a user can see,
    /// and it is where a runaway hover path shows up.
    pub hover_changes: u64,
    /// The deepest coalesced batch ever read.
    ///
    /// One means the platform never coalesced — the pump kept up, so each service saw a
    /// single sample and folding cost nothing and gained nothing. Greater than one means the
    /// pump fell behind and the batch is carrying samples a point-sampling consumer would
    /// have dropped on the floor. **Injected input rarely reaches it**: `SendInput` does not
    /// outrun a healthy pump. A 1 kHz mouse against a loaded front thread does.
    pub deepest_batch: u32,
    /// Hit tests run to resolve a discrete transition. Bounded by human input rate.
    pub discrete_hits: u64,
    /// Contacts bound to a recogniser.
    pub bindings: u64,
    /// Gestures recognised.
    pub gestures: u64,
    /// Contacts that aborted rather than completing.
    pub aborts: u64,
    /// Contacts refused for want of the digitizer's confidence — palms.
    pub rejected: u64,
}

/// The frame-clock half of the pointer stack.
///
/// Front-thread by construction: it owns recognisers, which are non-agile, and it resolves
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
    /// What each target says about the gestures it accepts. Front-resident: no call is made
    /// to the application thread to decide whether a gesture applies.
    decls: FxHashMap<ControlId, GestureDecl>,
    hover: Option<ControlId>,
    /// Set when the hover answer may have changed without the pointer moving — a layout
    /// change under a stationary cursor.
    hover_stale: bool,
    /// The environment the last tick ran under. A **watermark**, not an authority: its only
    /// reader is the comparison that decides what a move invalidated, and nothing ever asks
    /// this router what the DPI is.
    env: Option<Env>,
    /// The window's dial, where there is one. Absent on every machine without one, which is
    /// most — and absent is not a degraded path, it is a device that is not attached.
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
    /// The coalesced-history buffer, preallocated once. `POINTER_INFO` is ~100 bytes, so this
    /// is ~13 KB held for the window's life rather than pushed onto the stack every time a
    /// contact moves.
    history: Vec<POINTER_INFO>,
    census: InputCensus,
}

impl Router {
    /// Builds the frame-clock half over a doorbell already installed in `window`.
    ///
    /// It takes no scale and no output transform. Both are the window's and its monitor's,
    /// they are stated at every [`tick`](Self::tick), and a router that cached them could be
    /// *not told* when the window hops a display — silently, for the rest of the session,
    /// with every contact resolving against the wrong pixel grid.
    ///
    /// # Errors
    ///
    /// The window is closed, so there is no handle to resolve against.
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
        // The queue every recogniser in the pool is wired to. Kept as a clone so draining it
        // and iterating the pool do not borrow the same field twice — it is an `Rc` inside,
        // so the two names are one queue.
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

    /// What the machine reports about itself. Diagnostic: nothing branches on it.
    #[must_use]
    pub const fn capability(&self) -> &Capability {
        &self.capability
    }

    /// What this stack has done.
    #[must_use]
    pub const fn census(&self) -> &InputCensus {
        &self.census
    }

    /// Attaches the window's radial controller.
    ///
    /// **`Ok(true)` means the controller exists, not that a dial is present.**
    /// `CreateForWindow` succeeds on a machine with no wheel attached — measured — and the
    /// object simply never raises anything. That is the right shape: a dial arriving later
    /// needs no re-attach, and there is no capability to branch on.
    ///
    /// # Errors
    ///
    /// The interop factory refused the window.
    pub fn attach_rotary(&mut self, window: &Window) -> Result<bool> {
        match Rotary::new(window, self.bell.service()) {
            Ok(rotary) => {
                self.rotary = Some(rotary);
                Ok(true)
            }
            // `CreateForWindow` answers this when nothing is attached. Distinguished from a
            // real refusal so a missing dial cannot be read as a broken one.
            Err(error) if error.code() == windows_core::HRESULT(0x8007_0490u32 as i32) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Whether a dial is attached.
    #[must_use]
    pub const fn has_rotary(&self) -> bool {
        self.rotary.is_some()
    }

    /// Which unit the WinRT statics turned out to answer in.
    #[must_use]
    pub fn measured_unit(&self) -> Unit {
        self.space.unit()
    }

    /// The focus ring. Focus order is the hit array's, filtered to `INTERACTIVE`.
    #[must_use]
    pub const fn focus(&self) -> &FocusRing {
        &self.focus
    }

    /// The focus ring, for an overlay opening or closing a scope.
    pub const fn focus_mut(&mut self) -> &mut FocusRing {
        &mut self.focus
    }

    /// States what a target accepts. Called as the widget mounts.
    pub fn declare(&mut self, target: ControlId, decl: GestureDecl) {
        self.decls.insert(target, decl);
    }

    /// Forgets a target, on unmount. Any contact still bound to it is aborted, because a
    /// gesture whose target has gone cannot commit anything.
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

    /// Marks the hover answer as possibly stale — a layout change under a stationary pointer.
    ///
    /// Costs one hit test on the next tick and nothing at idle, which is the trade: without
    /// it, content that moves under a resting cursor lights nothing until the user twitches.
    pub fn invalidate_hover(&mut self) {
        self.hover_stale = true;
    }

    /// Honours a system request to stop content inertia.
    ///
    /// Not reached from a message arm: `WM_STOPINERTIA` is redacted from the platform floor's
    /// SDK, so there is no constant to match on. See [`inertia`].
    pub fn stop_inertia(&mut self) {
        self.bell.stop_inertia();
    }

    /// Consumes one frame of input.
    ///
    /// The order is the whole of the contract: discrete transitions first, in the order they
    /// happened; then every intermediate sample of every active contact; then **one** hover
    /// hit test; then inertia. A hover resolved before the presses that changed the tree
    /// would answer for the previous frame's layout.
    pub fn tick(&mut self, hits: &HitTable, env: Env, out: &mut Vec<Report>) -> Result<()> {
        self.census.ticks += 1;
        self.sync(env);
        // Re-opened **before** the drain, so a transition arriving during it asks for another
        // service rather than being swallowed. Exactly the pacer's own gate discipline, and
        // for exactly the reason `Pacer` has no `begin_tick`: a caller that had to remember
        // this would wedge it shut and the symptom would be input that silently stops.
        self.bell.begin();
        self.drain(hits, env, out)?;
        let hover_moved = self.feed(hits, env, out)?;
        self.resolve_hover(hits, env, out, hover_moved);
        self.rotate(hits, env, out);
        self.pump(out)?;
        self.settle();
        Ok(())
    }

    /// Brings the router up to date with `env`.
    ///
    /// The environment is stated, never held, so the only thing kept is a watermark and the
    /// only thing done with it is deciding what a move invalidated. A scale change puts every
    /// contact on a different pixel grid, so the hover answer is stale and the measured
    /// recogniser factor — which was never derived from the scale — cannot be carried across
    /// and is discarded for the next contact to measure again.
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
            // Only a loss that takes something away is a loss. Releasing capture ourselves
            // on an up posts this message back at us, and treating that as a cancel would
            // abort the gesture that had just completed normally.
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
        // contact has since moved to — which is the whole reason the doorbell pays for a
        // syscall there.
        let pressed = self.reader.at_transition(&event, &self.coords, env);
        let at = pressed.raw;
        self.census.discrete_hits += 1;
        let Some(hit) = hits.hit(at, event.ptype.contact()) else {
            // A press on nothing still takes focus away, which is what makes clicking the
            // background dismiss a text caret.
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

        let Some(decl) = self.decls.get(&hit.id).copied() else {
            return Ok(());
        };
        // A scroll surface hands touch to its `InteractionTracker` instead, which is what
        // keeps a fling running when the front thread is busy.
        if decl.redirect {
            return Ok(());
        }

        self.capture = Some(event.id);
        // Retained for mouse-as-pointer drags: a contact routes to its down-window for its
        // life, and this is what makes that true for the one device that can leave the window
        // without lifting.
        if event.ptype == PointerType::Mouse {
            // SAFETY: `hwnd` is live for the call; the previous capture is not needed.
            unsafe {
                _ = SetCapture(self.hwnd);
            }
        }

        self.pool
            .bind(event.id, event.ptype, hit.id, decl, at, rejected)?;
        self.census.bindings += 1;
        if !rejected {
            // Measured **before** the point that is fed, so even the first gesture of a
            // session is transformed by a factor that was read rather than assumed. The two
            // readings are of the same contact in two spaces — one through this crate's own
            // screen-to-client conversion, one through the platform's — so their ratio is the
            // conversion between them and nothing else.
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
                // SAFETY: takes no handle and writes through no pointer.
                unsafe {
                    _ = ReleaseCapture();
                }
            }
        }
        let Some(bound) = self.pool.get(event.id) else {
            self.bell.release(event.id);
            return Ok(());
        };
        let target = bound.target;
        if !bound.rejected {
            let point = PointerPoint::GetCurrentPointTransformed(event.id, &self.transform)?;
            bound.recognizer().up(&point)?;
        }
        out.push(Report::Released {
            target,
            contact: event.id,
            at,
        });
        self.collect(event.id, out);
        // Inertia keeps the binding alive: the contact is gone but its motion is not, and the
        // recogniser it is running on is the one being pumped.
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
        Ok(())
    }

    /// A canceled contact **aborts**: `CompleteGesture` and no commit.
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
            // SAFETY: takes no handle and writes through no pointer.
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
        // A scroll surface's wheel is the tracker's. Reaching here at all would mean the
        // source's `PointerWheelConfig` did not take it, and handling it front-side would be
        // a second scroll path.
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
        // Tab and Esc are the framework's before they are anyone's: focus order is one
        // authority and an open overlay has to be able to close from the keyboard whether or
        // not the pointer is anywhere near it.
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

    /// Feeds each moved contact its batch. Answers whether the hovering pointer moved, which
    /// is what decides whether the one hover hit test is worth running.
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
            // **Exactly the shape `ProcessMoveEvents` wants.** The platform's own sample
            // feeds it `GetIntermediatePoints`, and the frame-clock batch *is* that — so a
            // drag consumes every intermediate sample, in order, rather than the one the
            // message happened to carry.
            let batch = PointerPoint::GetIntermediatePointsTransformed(id, &self.transform)?;
            bound.recognizer().moves(&batch)?;

            // The drag policy **folds the whole batch** and reports the fold. Which axis a
            // two-axis drag locks to is a threshold crossing — an event on the path — so
            // deciding it from the newest sample alone can lock to the wrong one: the
            // crossing may have happened on an earlier sample that went the other way. The
            // *displacement* it reports is a state, so one report carries the fold.
            //
            // Predicted, not raw: continuous motion is where the system's own latency
            // compensation belongs.
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
            // Pressure, tilt, twist and the honest contact patch reach a gesture sink here
            // and nowhere else — a manipulation's own events carry a position and nothing
            // about the thing making it. State, so the newest reading is the whole answer.
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

    /// Drains whatever the recogniser raised for `id` and reports it.
    ///
    /// Called immediately after each feed, because the platform raises these **synchronously**
    /// from inside `ProcessDownEvent` and its siblings — so the binding is always the one just
    /// fed, and no event has to carry its own routing.
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
                    // **Restated per update, not set once at down**: the platform documents
                    // both pivot values as ones to keep current during the interaction.
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

    // ── 3. one hover hit test ─────────────────────────────────────────────────────

    /// Resolves hover — **at most once per tick**, whatever the device did.
    /// Resolves hover across **every sample the pointer produced**, in order.
    ///
    /// Hover reads as a per-frame quantity and is not one. "What is under the pointer" is
    /// state and could be point-sampled; *hover* is the accumulated result of enter and leave
    /// events, and those are boundary crossings on a path. A path that crosses a target
    /// between two samples has a real enter and a real leave that sampling cannot see — and
    /// once this layer has dropped them, no policy above can recover them.
    ///
    /// So the batch is walked and every crossing published. Whether a three-millisecond
    /// traversal should light anything is a **design** decision, and it belongs to the layer
    /// that owns the chrome — which already has the right instrument for it, because a
    /// retargeted spring swallows a sub-frame excursion at about eight percent of its ramp.
    /// Sampling is a second and cruder filter doing the same job worse, and doing it where
    /// the information cannot be got back.
    ///
    /// It stays **cheap** rather than staying *rare*: one history read in place of one
    /// current read, and a memo-bounded scan of the flat array per entry. The cost this used
    /// to be afraid of was two unpruned tree walks and a syscall per move, and the flat array
    /// is what fixed that. The publish is still one per service, because a service is one
    /// work item and a work item is one publish.
    ///
    /// **The raw position, per sample.** A target chosen from an extrapolated point is a
    /// mis-hover, not a smoother one. Nothing here constructs a `PointerPoint`, so the
    /// always-on path allocates nothing.
    fn resolve_hover(&mut self, hits: &HitTable, env: Env, out: &mut Vec<Report>, moved: bool) {
        // A contact owns the pointer while it is down: hover chrome must not chase a drag.
        // `capture` covers a contact this router bound; `is_down` covers one it deliberately
        // did not — a touch contact redirected to a scroll surface's tracker.
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
        // A pointer whose history has aged out still has a current position, and losing the
        // hover for a service because one batch read failed is a worse answer than a coarser
        // one.
        if count == 0
            && let Some(sample) = self.reader.newest(id, &self.coords, env)
        {
            self.cross(hits, &sample, out);
        }
        self.history = history;
    }

    /// Resolves one sample against the one authority, publishing a crossing if it is one.
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

    /// Drains the dial.
    ///
    /// **The contact is routed through the one hit array**, exactly as a finger is: an
    /// on-screen dial resting over a knob targets that knob, and a dial with no screen
    /// contact targets whatever has focus. That is the whole of why the rotary path is not a
    /// second routing authority — it resolves through the same array and falls back to the
    /// same focus ring.
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
                // Acquiring and losing the dial is not an input event; it is the device
                // arriving and leaving, and nothing is targeted by it.
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
                    // The step is the *target's* declared resolution, so a knob whose
                    // detents are two units apart moves by two per click and the haptics
                    // match — which is fidelity no other device on this machine produces.
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
            // A system request to stop: end every running motion, and do not commit anything
            // it was on its way to.
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
            // A recogniser whose inertia has run out has nothing left to pump, and the
            // contact behind it lifted long ago.
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

    // ── 5. what the next frame is for ─────────────────────────────────────────────

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
        // Told on the edge, and told at all: a window whose content is moving and has not
        // said so turns a touchpad tap into a click on whatever was moving under it.
        self.inertia.set(self.pool.any_inertial());
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
