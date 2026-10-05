//! What a target declares, what a contact is doing about it, and the two recognisers
//! underneath.
//!
//! A declaration is looked up per contact; a contact's drag state is the declaration plus its
//! origin plus one phase; the recogniser is what that contact is bound to.
//!
//! # A contact is a slot
//!
//! There are at most [`SLOTS`] contacts, because the doorbell's own slot table bounds them
//! there. So every per-contact datum is a column of that length, a recogniser is minted into
//! its slot on first use and never handed back, and "pooling" is that and nothing more: no
//! map, no free list, no scratch buffer.
//!
//! # Declarations are front-resident
//!
//! No call is made to the application thread to decide whether a gesture applies, because that
//! decision has to be made between a contact arriving and its pixels moving. A control that
//! declares nothing routes press, release and drag through the ordinary paths and costs no
//! recogniser at all.
//!
//! # Allocation
//!
//! `PointerPoint` is a WinRT object **per sample**, so **hover never touches the recogniser
//! path**: it stays on the raw Win32 history and allocates nothing. WinRT points are
//! constructed only between down and up, for the one contact being manipulated, which confines
//! allocation to a bounded, user-initiated interval. Nothing in
//! [`Sample`](crate::input::Sample) can reach here.
//!
//! # The two recognisers
//!
//! | Pointer type | Recogniser |
//! |---|---|
//! | touch, pen, mouse | `Windows.UI.Input.GestureRecognizer` |
//! | precision touchpad | `Windows.UI.Input.PhysicalGestureRecognizer` |
//!
//! Both emit the **same** `Windows.UI.Input` event argument types, so the sink downstream is
//! one code path regardless of device. They differ in the coordinate space of their output —
//! the physical one reports relative to the device rather than in display-independent pixels —
//! and in what they recognise at all: the platform never recognises touchpad input as Tap or
//! Hold.
//!
//! Both are **non-agile** (`MarshalingBehavior(None)`), so both are created and used only on
//! the front thread — the same constraint the compositor and text services already impose.

use crate::bindings::{Point as WinPoint, *};
use core::cell::RefCell;
use core::time::Duration;
use std::rc::Rc;
use windows_collections::IVector;
use windows_core::{EventRevoker, Interface, Result};
use windows_scene::{ControlId, Hit, HitFlags, HitTable, NO_ENTRY, Point};

/// The most contacts that can be live at once, which is the doorbell's slot count: ten
/// fingers, and two spare slots absorb a pen and a mouse arriving alongside a full hand.
pub const SLOTS: usize = 12;

// ---------------------------------------------------------------- what a target declares

/// Everything one target says about how it may be touched.
///
/// `Copy`, and every field a number or a flag: a declaration travels with the widget that made
/// it and is looked up per contact, so it must not own anything.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GestureDecl {
    /// This node's recogniser configuration.
    pub settings: GestureSettings,
    pub hold: HoldTuning,
    /// Whether this target is turned: single-pointer rotation about its own centre.
    ///
    /// A marker and not a number. The [`Pivot`] a turned control rotates about is resolved
    /// from the hit array per contact, so it follows a control that scrolls or moves without
    /// anything being re-declared.
    pub turned: bool,
    /// DIPs added on each side of this target's hit entry, or `None` for the platform's own
    /// ~9 mm guidance. `Some(0.0)` never grows: a dense meter or a curve node field, where
    /// inflation would make adjacent targets indistinguishable.
    pub touch_inflate: Option<f32>,
    /// A drag whose meaning depends on its direction.
    pub drag: Option<DragDecl>,
    /// Rotary interest — resolution and step for `RadialController`.
    pub rotary: Option<RotaryDecl>,
}

/// Every manipulation bit. A declaration carrying one needs a recogniser bound to its contact.
const MANIPULATES: u32 = GestureSettings::ManipulationTranslateX.0
    | GestureSettings::ManipulationTranslateY.0
    | GestureSettings::ManipulationRotate.0
    | GestureSettings::ManipulationScale.0;

impl Default for GestureDecl {
    /// A plain tappable target: tap, right-tap, and hold, so touch reaches the context menu
    /// the secondary button opens.
    fn default() -> Self {
        Self {
            settings: GestureSettings::Tap
                | GestureSettings::RightTap
                | GestureSettings::Hold
                | GestureSettings::HoldWithMouse,
            hold: HoldTuning::default(),
            turned: false,
            touch_inflate: None,
            drag: None,
            rotary: None,
        }
    }
}

impl GestureDecl {
    /// Returns a declaration that recognizes single and double taps.
    #[must_use]
    pub fn double_tap() -> Self {
        Self { settings: GestureSettings::Tap | GestureSettings::DoubleTap, ..Self::default() }
    }

    /// Returns a declaration that reports taps and nothing else.
    #[must_use]
    pub fn tap() -> Self {
        Self {
            settings: GestureSettings::Tap,
            ..Self::default()
        }
    }

    /// Returns a slider's declaration: translation on one axis, no inertia so the value stops
    /// where the contact lifts, and no inflation.
    #[must_use]
    pub fn slider(vertical: bool) -> Self {
        const AXIS: [GestureSettings; 2] = [
            GestureSettings::ManipulationTranslateX,
            GestureSettings::ManipulationTranslateY,
        ];
        Self {
            settings: AXIS[usize::from(vertical)],
            touch_inflate: Some(0.0),
            ..Self::default()
        }
    }

    /// Returns a knob's declaration: single-pointer rotation about the control's own centre.
    ///
    /// The centre and the radius are not stated here. [`pivot_of`] resolves them from the
    /// control's entry in the hit array whenever a contact is bound to it or moves on it.
    #[must_use]
    pub fn knob() -> Self {
        Self {
            settings: GestureSettings::ManipulationRotate
                | GestureSettings::ManipulationTranslateX
                | GestureSettings::ManipulationTranslateY,
            turned: true,
            touch_inflate: Some(0.0),
            ..Self::default()
        }
    }

    /// Adds a two-axis drag policy.
    #[must_use]
    pub const fn with_drag(mut self, drag: DragDecl) -> Self {
        self.drag = Some(drag);
        self
    }

    /// Adds rotary interest, so the dial drives the same value path a drag does.
    #[must_use]
    pub const fn with_rotary(mut self, rotary: RotaryDecl) -> Self {
        self.rotary = Some(rotary);
        self
    }

    /// Returns whether this declaration asks for any manipulation, which is what decides
    /// whether a contact needs a recogniser bound to it.
    #[must_use]
    pub const fn manipulates(&self) -> bool {
        self.settings.0 & MANIPULATES != 0
    }
}

/// How long, how far, and with how many contacts a hold is a hold.
///
/// The defaults are the platform's own tuning: half a second, and a contact that wanders more
/// than a finger's width is a drag.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HoldTuning {
    pub start_delay: Duration,
    pub radius: f32,
    pub min_contacts: u32,
    pub max_contacts: u32,
}

impl Default for HoldTuning {
    fn default() -> Self {
        Self {
            start_delay: Duration::from_millis(500),
            radius: 10.0,
            min_contacts: 1,
            max_contacts: 1,
        }
    }
}

/// Single-pointer rotation about a resolved centre — which is what a knob is.
///
/// Both values are in client DIPs, the space the samples fed to a recogniser are stated in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pivot {
    pub radius: f32,
    pub center: Point,
}

/// Returns the pivot `target` rotates about, or `None` where it is not turned or has no entry.
///
/// The centre is the entry box's own centre and the radius half its shorter side, both moved
/// by the live scroll offset of the entry's scrolling ancestor: layout places content
/// unscrolled and the compositor applies the offset, so a dial inside a viewport moves under
/// the finger with no solve and nothing else would correct it.
#[must_use]
pub fn pivot_of(hits: &HitTable, target: ControlId, decl: GestureDecl) -> Option<Pivot> {
    if !decl.turned {
        return None;
    }
    let entry = hits.entry(target)?;
    let offset = if entry.flags.contains(HitFlags::UNSCROLLED) {
        Point::zero()
    } else {
        hits.offset(entry.scroll_src)
    };
    let offset = offset - hits.translation(target);
    Some(Pivot {
        radius: (entry.x1 - entry.x0).min(entry.y1 - entry.y0) * 0.5,
        center: Point {
            x: (entry.x0 + entry.x1) * 0.5 - offset.x,
            y: (entry.y0 + entry.y1) * 0.5 - offset.y,
        },
    })
}

/// Rotary interest — what the dial does to this target.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RotaryDecl {
    /// Degrees of dial rotation per detent. The dial's haptics are driven from this, so it is
    /// the step the user *feels* as well as the one they get.
    pub resolution_degrees: f64,
    /// How much the value moves per detent, in the target's own units.
    pub step: f64,
    pub haptics: bool,
}

impl Default for RotaryDecl {
    fn default() -> Self {
        Self {
            resolution_degrees: 10.0,
            step: 1.0,
            haptics: true,
        }
    }
}

// ---------------------------------------------------------------- the two-axis drag policy

/// A drag whose meaning depends on its direction.
///
/// Moving a row within its list and moving it into a different one are the same physical
/// gesture on the same object, separated by axis: vertical is order, horizontal is scope. One
/// policy holds for every two-axis drag in the application:
///
/// 1. **Nothing is decided before the threshold.** Below it the gesture has no axis and no
///    meaning, so a nudge while clicking is a click.
/// 2. **The first axis past the threshold owns the drag for its whole duration.** The lock is
///    never revisited, so a drag cannot change meaning mid-flight.
/// 3. **The locked axis is named on screen**, beside the pointer, for as long as the lock
///    holds. This module reports the axis; the overlay layer anchors the label.
/// 4. **Commit on release is the default, and cancel aborts.** A canceled contact restores the
///    pre-drag value — for a reorder that means the row returns to its original index, not to
///    wherever it was hovering.
/// 5. **Displacement is a compositor animation, not a per-frame write.** The dragged row
///    follows the contact on the frame clock, and the rows it displaces move by a retargeted
///    chrome spring started when the insertion index changes. The widget owns that; this
///    module reports *when* the index changed.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DragDecl {
    /// Which axes may mean something. A cleared axis is left out of the decision entirely.
    pub vertical: bool,
    pub horizontal: bool,
    /// DIPs before any meaning is assigned.
    pub threshold: f32,
    /// Whether the first axis past the threshold owns the drag. Clear it for a free pan, where
    /// both axes stay live.
    pub lock: bool,
    /// Whether the value follows the contact rather than landing on release.
    pub live: bool,
}

impl Default for DragDecl {
    /// A reorderable list's drag: both axes, locked, committed on release.
    fn default() -> Self {
        Self {
            vertical: true,
            horizontal: true,
            threshold: 6.0,
            lock: true,
            live: false,
        }
    }
}

/// How far a drag has been decided, and onto what.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Below the threshold. No axis, no meaning — and still a click if it ends here.
    #[default]
    Undecided,
    Vertical,
    Horizontal,
    /// Both axes live. Only ever reached with the lock cleared.
    Free,
}

/// What one sample did to a drag.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DragUpdate {
    pub phase: Phase,
    /// Displacement from the contact's origin, in DIPs, **projected onto the locked axis**. A
    /// locked drag reports zero on the axis it does not own, so a consumer cannot accidentally
    /// act on the other one.
    pub delta: Point,
    /// Where the contact went down, in client DIPs.
    pub from: Point,
    /// Where it is now, in client DIPs. Not projected: a consumer that resolves a position
    /// against a box — which lane of a channel graph the pointer is over, which row it is
    /// between — needs the point the hit array is scanned in, and a displacement cannot answer
    /// that without knowing where it started.
    pub at: Point,
    /// Whether this sample is the one that decided the axis, which is when the label appears.
    pub decided: bool,
}

/// Advances a drag with one sample at `at`, in client DIPs.
///
/// Every sample of the contact's path must be fed, in order: the axis is decided by a
/// threshold crossing on the path, so a caller that skips to the newest sample of a batch can
/// lock an axis the contact has already left.
fn advance(decl: DragDecl, from: Point, phase: &mut Phase, at: Point) -> DragUpdate {
    let raw = Point {
        x: at.x - from.x,
        y: at.y - from.y,
    };
    let mut decided = false;
    if *phase == Phase::Undecided {
        let past_x = decl.horizontal && raw.x.abs() >= decl.threshold;
        let past_y = decl.vertical && raw.y.abs() >= decl.threshold;
        // The larger displacement wins where one sample crosses on both axes at once, which is
        // what "first past" means when the frame clock delivers the crossing as a single batch
        // rather than as two samples.
        *phase = match (past_x, past_y, decl.lock) {
            (false, false, _) => Phase::Undecided,
            (_, _, false) => Phase::Free,
            (true, true, _) if raw.x.abs() >= raw.y.abs() => Phase::Horizontal,
            (true, true, _) | (false, true, _) => Phase::Vertical,
            (true, false, _) => Phase::Horizontal,
        };
        decided = *phase != Phase::Undecided;
    }
    let delta = match *phase {
        Phase::Undecided => Point { x: 0.0, y: 0.0 },
        Phase::Horizontal => Point { x: raw.x, y: 0.0 },
        Phase::Vertical => Point { x: 0.0, y: raw.y },
        Phase::Free => raw,
    };
    DragUpdate {
        phase: *phase,
        delta,
        from,
        at,
        decided,
    }
}

// ---------------------------------------------------------------- scroll arbitration

/// Returns the nearest scrolling viewport containing `hit`.
///
/// An overlay blocker has no parent, so a gesture outside a flyout cannot fall through into
/// the document behind it.
#[must_use]
pub fn scroll_ancestor(hits: &HitTable, hit: Hit) -> Option<ControlId> {
    let mut at = hit.index;
    while at != NO_ENTRY {
        let entry = hits.entries().get(at as usize)?;
        if entry.flags.contains(HitFlags::SCROLL) {
            return Some(entry.id);
        }
        at = entry.parent;
    }
    None
}

/// Returns the scroll container a touch press is offered to, or `None` where the target keeps
/// it.
///
/// Editing gestures retain their target. An ordinary button can still become a pan after the
/// recogniser crosses its manipulation threshold.
#[must_use]
pub fn scroll_offer(hits: &HitTable, hit: Hit, decl: GestureDecl) -> Option<ControlId> {
    let retains = hit.flags.contains(HitFlags::TEXT)
        || decl.drag.is_some()
        || decl.turned
        || decl.manipulates();
    if retains {
        None
    } else {
        scroll_ancestor(hits, hit)
    }
}

// ---------------------------------------------------------------- what the recogniser said

/// A manipulation's translation, scale, rotation and expansion, in the space the hit array is
/// built in.
///
/// Rotation is degrees, positive clockwise: the platform's convention, carried unconverted so
/// a value from the system reads the same here as in its own documentation.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Manip {
    pub translation: Point,
    pub scale: f32,
    pub rotation: f32,
    pub expansion: f32,
}

impl From<ManipulationDelta> for Manip {
    fn from(d: ManipulationDelta) -> Self {
        Self {
            translation: Point {
                x: d.translation.x,
                y: d.translation.y,
            },
            scale: d.scale,
            rotation: d.rotation,
            expansion: d.expansion,
        }
    }
}

/// What the recogniser said. One vocabulary for both of them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Recognised {
    Tapped {
        at: Point,
        count: u32,
    },
    RightTapped {
        at: Point,
    },
    Holding {
        at: Point,
        state: HoldingState,
    },
    Dragging {
        at: Point,
        state: DraggingState,
    },
    ManipulationStarted {
        at: Point,
    },
    ManipulationUpdated {
        at: Point,
        delta: Manip,
        cumulative: Manip,
    },
    /// The contact has lifted and the motion continues. **The only place inertia can be
    /// tuned** — the platform documents those settings as unchangeable after this event.
    InertiaStarting {
        at: Point,
    },
    ManipulationCompleted {
        at: Point,
        cumulative: Manip,
    },
}

/// Where a recogniser's events land.
///
/// Shared by every slot and drained by the router immediately after each feed, because the
/// platform raises these **synchronously** from inside `ProcessDownEvent` and friends. So the
/// binding is always the one the router just fed.
#[derive(Clone, Default)]
pub struct Events(Rc<RefCell<Vec<Recognised>>>);

impl Events {
    /// Appends one event, dropping it if the queue is already borrowed.
    ///
    /// A re-entrant raise would otherwise panic and unwind across the COM boundary, so the
    /// drop loses a gesture rather than the process.
    fn push(&self, event: Recognised) {
        if let Ok(mut queue) = self.0.try_borrow_mut() {
            queue.push(event);
        }
    }

    /// Appends everything raised since the last drain to `out`.
    pub fn drain(&self, out: &mut Vec<Recognised>) {
        if let Ok(mut queue) = self.0.try_borrow_mut() {
            out.append(&mut queue);
        }
    }

    /// Discards whatever is queued.
    ///
    /// What a canceled contact does, so an abort delivers no part of the gesture it aborted.
    pub fn clear(&self) {
        if let Ok(mut queue) = self.0.try_borrow_mut() {
            queue.clear();
        }
    }
}

fn point(p: WinPoint) -> Point {
    Point { x: p.x, y: p.y }
}

/// Registers one handler, reads the argument properties it names, and queues what they build.
///
/// A failed property read queues nothing rather than unwinding across the COM boundary.
macro_rules! raises {
    ($src:expr, $sink:expr, $event:ident, ($($read:ident),*) => |$($held:ident),*| $make:expr) => {{
        let sink = $sink.clone();
        $src.$event(move |_, args| {
            if let Some(args) = args.as_ref()
                && let ($(Ok($held),)*) = ($(args.$read(),)*)
            {
                sink.push($make);
            }
        })?
    }};
}

// ---------------------------------------------------------------- the recogniser wrapper

/// One platform recogniser and the handler registrations that outlive every binding it takes.
struct Recognizer {
    kind: Kind,
    /// Held for their `Drop`, which revokes. A slot's recogniser is configured again rather
    /// than re-subscribed, so this is set up once, when the object is minted.
    _revokers: Vec<EventRevoker>,
}

/// Which of the two a contact's device needs.
enum Kind {
    Gesture(GestureRecognizer),
    Physical(PhysicalGestureRecognizer),
}

/// One thing fed to a recogniser.
///
/// Every call the platform takes from us is a row here, so the device dispatch happens once
/// instead of once per call.
pub enum Feed<'a> {
    Down(&'a PointerPoint),
    /// `ProcessMoveEvents` takes the intermediate points for a frame, which is what the
    /// frame-clock batch holds, so the recogniser sees the whole path rather than its newest
    /// point.
    Moves(&'a IVector<PointerPoint>),
    Up(&'a PointerPoint),
    /// Pumped from the pacer tick, so inertia, springs and a system stop request all resolve
    /// on one clock.
    Inertia,
    /// **What a cancel does**, and not an up: no value is committed.
    Complete,
}

impl Recognizer {
    /// Mints the recogniser `physical` selects and wires every event it raises into `events`.
    ///
    /// # Errors
    ///
    /// The recogniser could not be constructed, or one of its handlers could not be
    /// registered.
    fn new(physical: bool, events: &Events) -> Result<Self> {
        if physical {
            let src = PhysicalGestureRecognizer::new()?;
            // Tap and Hold are not wired: the platform never recognises touchpad input as
            // either, by design, because touchpad input only arrives in scenarios where those
            // are no longer possible.
            let revokers = vec![
                raises!(src, events, ManipulationStarted, (Position) => |at|
                    Recognised::ManipulationStarted { at: point(at) }),
                raises!(src, events, ManipulationUpdated, (Position, Delta, Cumulative)
                    => |at, delta, cumulative| Recognised::ManipulationUpdated {
                        at: point(at), delta: delta.into(), cumulative: cumulative.into() }),
                raises!(src, events, ManipulationCompleted, (Position, Cumulative)
                    => |at, cumulative| Recognised::ManipulationCompleted {
                        at: point(at), cumulative: cumulative.into() }),
            ];
            return Ok(Self {
                kind: Kind::Physical(src),
                _revokers: revokers,
            });
        }
        let src = GestureRecognizer::new()?;
        let revokers = vec![
            raises!(src, events, Tapped, (Position, TapCount) => |at, count|
                Recognised::Tapped { at: point(at), count }),
            raises!(src, events, RightTapped, (Position) => |at|
                Recognised::RightTapped { at: point(at) }),
            raises!(src, events, Holding, (Position, HoldingState) => |at, state|
                Recognised::Holding { at: point(at), state }),
            raises!(src, events, Dragging, (Position, DraggingState) => |at, state|
                Recognised::Dragging { at: point(at), state }),
            raises!(src, events, ManipulationStarted, (Position) => |at|
                Recognised::ManipulationStarted { at: point(at) }),
            raises!(src, events, ManipulationUpdated, (Position, Delta, Cumulative)
                => |at, delta, cumulative| Recognised::ManipulationUpdated {
                    at: point(at), delta: delta.into(), cumulative: cumulative.into() }),
            raises!(src, events, ManipulationInertiaStarting, (Position) => |at|
                Recognised::InertiaStarting { at: point(at) }),
            raises!(src, events, ManipulationCompleted, (Position, Cumulative)
                => |at, cumulative| Recognised::ManipulationCompleted {
                    at: point(at), cumulative: cumulative.into() }),
        ];
        Ok(Self {
            kind: Kind::Gesture(src),
            _revokers: revokers,
        })
    }

    /// Feeds one thing.
    ///
    /// The physical recogniser has no inertia of its own, because the system continues a
    /// touchpad manipulation itself and reports it through the inertia messages.
    ///
    /// # Errors
    ///
    /// The platform refused the sample, the batch, or the call.
    fn feed(&self, op: Feed<'_>) -> Result<()> {
        match (&self.kind, op) {
            (Kind::Gesture(r), Feed::Down(p)) => r.ProcessDownEvent(p),
            (Kind::Physical(r), Feed::Down(p)) => r.ProcessDownEvent(p),
            (Kind::Gesture(r), Feed::Moves(b)) => r.ProcessMoveEvents(b),
            (Kind::Physical(r), Feed::Moves(b)) => r.ProcessMoveEvents(b),
            (Kind::Gesture(r), Feed::Up(p)) => r.ProcessUpEvent(p),
            (Kind::Physical(r), Feed::Up(p)) => r.ProcessUpEvent(p),
            (Kind::Gesture(r), Feed::Complete) => r.CompleteGesture(),
            (Kind::Physical(r), Feed::Complete) => r.CompleteGesture(),
            (Kind::Gesture(r), Feed::Inertia) => r.ProcessInertia(),
            (Kind::Physical(_), Feed::Inertia) => Ok(()),
        }
    }

    /// Configures the recogniser from a target's declaration.
    ///
    /// The physical recogniser supports a subset of `GestureSettings` — `Tap`, `Hold`, the
    /// translate and rails flags, `ManipulationRotate`, `ManipulationScale` and
    /// `ManipulationMultipleFingerPanning` — so anything else is masked off rather than
    /// offered and rejected.
    ///
    /// # Errors
    ///
    /// The platform refused one of the settings, or the second tuning interface was not
    /// available on this recogniser.
    fn configure(&self, decl: &GestureDecl) -> Result<()> {
        let src = match &self.kind {
            Kind::Physical(src) => {
                src.SetGestureSettings(GestureSettings(decl.settings.0 & PHYSICAL_SETTINGS))?;
                src.SetHoldStartDelay(timespan(decl.hold.start_delay))?;
                return src.SetHoldRadius(decl.hold.radius);
            }
            Kind::Gesture(src) => src,
        };
        src.SetGestureSettings(decl.settings)?;
        // This stack draws its own feedback everywhere; the system's would be a second
        // affordance for the same contact.
        src.SetShowGestureFeedback(false)?;
        // Inertia is advanced by the frame tick rather than by a clock of the recogniser's
        // own — there is no fourth clock.
        src.SetAutoProcessInertia(false)?;
        // Hold tuning and the contact-count bounds live on the class's second interface,
        // reached by cast. A knob that must not be claimed by a two-finger contact needs the
        // maxima, not just the delay.
        let tuning: IGestureRecognizer2 = src.cast()?;
        tuning.SetHoldStartDelay(timespan(decl.hold.start_delay))?;
        tuning.SetHoldRadius(decl.hold.radius)?;
        tuning.SetHoldMinContactCount(decl.hold.min_contacts)?;
        tuning.SetHoldMaxContactCount(decl.hold.max_contacts)?;
        // Cleared rather than stated: the centre is a fact about where the control is now,
        // which this declaration does not carry, and a slot's recogniser otherwise still
        // carries the last knob's radius. The caller states it against the hit array.
        self.pivot(None)
    }

    /// States the pivot, or turns single-pointer rotation off.
    ///
    /// Zero radius is what turns it off, and a slot's recogniser still carries the last knob's
    /// radius.
    ///
    /// # Errors
    ///
    /// The platform refused the centre or the radius.
    fn pivot(&self, pivot: Option<Pivot>) -> Result<()> {
        let Kind::Gesture(src) = &self.kind else {
            return Ok(());
        };
        let Some(p) = pivot else {
            return src.SetPivotRadius(0.0);
        };
        src.SetPivotCenter(WinPoint {
            x: p.center.x,
            y: p.center.y,
        })?;
        src.SetPivotRadius(p.radius)
    }

    /// Returns whether inertia is still running, which is what keeps a tick requested.
    fn inertial(&self) -> bool {
        matches!(&self.kind, Kind::Gesture(r) if r.IsInertial().unwrap_or(false))
    }
}

/// The `GestureSettings` bits the precision-touchpad recogniser supports.
const PHYSICAL_SETTINGS: u32 = GestureSettings::Tap.0
    | GestureSettings::Hold.0
    | GestureSettings::ManipulationTranslateX.0
    | GestureSettings::ManipulationTranslateY.0
    | GestureSettings::ManipulationTranslateRailsX.0
    | GestureSettings::ManipulationTranslateRailsY.0
    | GestureSettings::ManipulationRotate.0
    | GestureSettings::ManipulationScale.0
    | GestureSettings::ManipulationMultipleFingerPanning.0;

/// Returns `duration` as the platform's 100-nanosecond count.
fn timespan(duration: Duration) -> windows_time::TimeSpan {
    windows_time::TimeSpan {
        duration: (duration.as_nanos() / 100).min(i64::MAX as u128) as i64,
    }
}

// ---------------------------------------------------------------- the contact table

/// What one contact has done, packed because the flags are read together and written one at a
/// time.
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct Flags(u8);

impl Flags {
    /// A touch offered to a scroll ancestor invokes its child only after a recognised tap.
    pub const SCROLL_TOUCH: Self = Self(1);
    pub const TAPPED: Self = Self(2);
    /// The contact has lifted and its motion is still being pumped.
    pub const INERTIAL: Self = Self(4);
    /// The contact arrived without the digitizer's confidence. Such a contact **never starts a
    /// gesture** — nothing is fed to its recogniser — and that is the whole of palm rejection
    /// on this stack.
    pub const REJECTED: Self = Self(8);
    /// The contact is a precision-touchpad one, so it is bound through the physical
    /// recogniser.
    pub const PHYSICAL: Self = Self(16);

    /// Returns whether `flag` is set.
    #[must_use]
    pub const fn has(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
}

/// Every live contact, as columns indexed by the doorbell's own contact slot.
///
/// A recogniser is minted into a slot on first use and kept for the window's life, so a slot
/// that has held a mouse contact answers the next one without constructing anything.
#[derive(Default)]
pub struct Contacts {
    /// Zero means the slot is free.
    id: [u32; SLOTS],
    target: [ControlId; SLOTS],
    decl: [GestureDecl; SLOTS],
    /// The contact's down point, in client DIPs, **raw** — a press target is a discrete
    /// decision and an extrapolated origin makes every later delta wrong by the same amount.
    origin: [Point; SLOTS],
    phase: [Phase; SLOTS],
    flags: [Flags; SLOTS],
    /// Minted lazily per slot and per device kind, because a slot that held a touchpad contact
    /// may hold a mouse one next.
    gesture: [Option<Recognizer>; SLOTS],
    physical: [Option<Recognizer>; SLOTS],
    /// Free slot retaining the preceding tap's recognizer until the next press or cancel.
    tap: Option<usize>,
    events: Events,
}

impl Contacts {
    /// Returns the queue every recogniser raises into, drained by the router after each feed.
    #[must_use]
    pub const fn events(&self) -> &Events {
        &self.events
    }

    /// Returns the slot holding contact `id`.
    ///
    /// A linear scan of twelve `u32`s, in one cache line.
    #[must_use]
    pub fn at(&self, id: u32) -> Option<usize> {
        (id != 0)
            .then(|| self.id.iter().position(|held| *held == id))
            .flatten()
    }

    /// Returns every slot's contact id, zero where the slot is free.
    ///
    /// A copy of the column, so a caller walks the contacts while it mutates them and
    /// allocates nothing doing it.
    #[must_use]
    pub const fn ids(&self) -> [u32; SLOTS] {
        self.id
    }

    /// Returns how many contacts are bound.
    #[must_use]
    pub fn live(&self) -> usize {
        self.id.iter().filter(|held| **held != 0).count()
    }

    /// Returns whether anything is still in inertia, which is what keeps a frame requested.
    #[must_use]
    pub fn any_inertial(&self) -> bool {
        self.flags.iter().any(|f| f.has(Flags::INERTIAL))
    }

    /// Returns what contact `id` is on, and its declaration.
    #[must_use]
    pub fn bound(&self, id: u32) -> Option<(ControlId, GestureDecl)> {
        let i = self.at(id)?;
        Some((self.target[i], self.decl[i]))
    }

    /// Returns contact `id`'s flags, or none where it is not bound.
    #[must_use]
    pub fn flags(&self, id: u32) -> Flags {
        self.at(id).map_or(Flags::default(), |i| self.flags[i])
    }

    /// Sets or clears one flag on contact `id`. Does nothing where it is not bound.
    pub fn set(&mut self, id: u32, flag: Flags, on: bool) {
        let Some(i) = self.at(id) else { return };
        self.flags[i] = Flags(if on {
            self.flags[i].0 | flag.0
        } else {
            self.flags[i].0 & !flag.0
        });
    }

    /// Binds a contact to a target, configured from that target's declaration.
    ///
    /// `rejected` says the contact arrived without the digitizer's confidence. It is still
    /// bound — so that its up and its cancel are accounted for — but nothing is fed to the
    /// recogniser, so it can never start a gesture.
    ///
    /// # Errors
    ///
    /// Every slot is taken, which is the doorbell's overflow rather than a state this can be
    /// in on its own; or the platform refused to construct or configure a recogniser.
    pub fn bind(
        &mut self,
        id: u32,
        target: ControlId,
        decl: GestureDecl,
        physical: bool,
        origin: Point,
        rejected: bool,
    ) -> Result<()> {
        let continued = self.tap.filter(|&i| {
            !physical && !rejected && self.at(id).is_none()
                && self.target[i] == target && self.decl[i] == decl
        });
        if continued.is_some() {
            self.tap = None;
        } else {
            self.cancel_tap();
        }
        let Some(i) = self
            .at(id)
            .or(continued)
            .or_else(|| self.id.iter().position(|held| *held == 0))
        else {
            return Err(windows_core::Error::empty());
        };
        let held = if physical {
            &mut self.physical[i]
        } else {
            &mut self.gesture[i]
        };
        let recognizer = match held {
            Some(held) => held,
            none => none.insert(Recognizer::new(physical, &self.events)?),
        };
        if continued.is_none() {
            recognizer.configure(&decl)?;
        }
        (self.id[i], self.target[i], self.decl[i], self.origin[i]) = (id, target, decl, origin);
        self.phase[i] = Phase::Undecided;
        self.flags[i] =
            Flags(u8::from(rejected) * Flags::REJECTED.0 + u8::from(physical) * Flags::PHYSICAL.0);
        Ok(())
    }

    /// Feeds contact `id`'s recogniser, and drops the feed where the contact was rejected.
    ///
    /// # Errors
    ///
    /// The platform refused what was fed.
    pub fn feed(&self, id: u32, op: Feed<'_>) -> Result<()> {
        let Some(i) = self.at(id) else { return Ok(()) };
        if self.flags[i].has(Flags::REJECTED) {
            return Ok(());
        }
        self.recognizer(i).map_or(Ok(()), |r| r.feed(op))
    }

    /// States contact `id`'s pivot, or turns single-pointer rotation off with `None`.
    ///
    /// Called on **every** `ManipulationUpdated` rather than once at down: the platform
    /// documents both values as ones to update regularly during the interaction, and a stale
    /// centre makes a knob drift under a finger that has not moved. `pivot` is resolved by the
    /// caller against the hit array, because the centre is where the control is on screen and
    /// the declaration carries no numbers.
    ///
    /// # Errors
    ///
    /// The platform refused the centre or the radius.
    pub fn restate_pivot(&self, id: u32, pivot: Option<Pivot>) -> Result<()> {
        let Some(i) = self.at(id) else { return Ok(()) };
        self.recognizer(i).map_or(Ok(()), |r| r.pivot(pivot))
    }

    /// Advances contact `id`'s drag with one sample, where its target declared one.
    pub fn drag(&mut self, id: u32, at: Point) -> Option<DragUpdate> {
        let i = self.at(id)?;
        let decl = self.decl[i].drag?;
        Some(advance(decl, self.origin[i], &mut self.phase[i], at))
    }

    /// Returns whether contact `id`'s recogniser is still running inertia.
    ///
    /// Asked of the platform rather than of [`Flags::INERTIAL`], which says only that the
    /// contact lifted with motion left: the recogniser is what knows when that motion stopped.
    #[must_use]
    pub fn still_inertial(&self, id: u32) -> bool {
        self.at(id)
            .and_then(|i| self.recognizer(i))
            .is_some_and(Recognizer::inertial)
    }

    /// Ends a contact and frees its slot.
    ///
    /// An abort completes recognition and discards queued events. An ordinary recognized
    /// tap retains its recognizer state when the target accepts double taps.
    pub fn release(&mut self, id: u32, abort: bool) {
        if abort {
            self.cancel_tap();
        }
        if let Some(i) = self.at(id) {
            self.free(i, abort);
        }
    }

    /// Ends every contact. What a lost capture and a window losing focus both do.
    pub fn release_all(&mut self, abort: bool) {
        for i in 0..SLOTS {
            if self.id[i] != 0 {
                self.free(i, abort);
            }
        }
        self.cancel_tap();
    }

    /// Frees slot `i`, retaining only a recognized double-tap candidate's platform state.
    fn free(&mut self, i: usize, abort: bool) {
        let retain = !abort && !self.flags[i].has(Flags::REJECTED)
            && !self.flags[i].has(Flags::PHYSICAL)
            && self.flags[i].has(Flags::TAPPED)
            && self.decl[i].settings.contains(GestureSettings::DoubleTap);
        if retain {
            self.cancel_tap();
            self.tap = Some(i);
        } else if let Some(r) = self.recognizer(i) {
            _ = r.feed(Feed::Complete);
        }
        if abort {
            self.events.clear();
        }
        self.id[i] = 0;
        self.flags[i] = Flags::default();
    }

    /// Ends the pending tap without allocating or scheduling its expiry.
    pub(crate) fn cancel_tap(&mut self) {
        if let Some(i) = self.tap.take()
            && let Some(r) = &self.gesture[i]
        {
            _ = r.feed(Feed::Complete);
        }
    }

    /// Discards a retiring target's pending tap without affecting other targets.
    pub(crate) fn forget_tap(&mut self, target: ControlId) {
        if self.tap.is_some_and(|i| self.target[i] == target) {
            self.cancel_tap();
        }
    }

    /// Returns the recogniser slot `i`'s current contact is bound through.
    fn recognizer(&self, i: usize) -> Option<&Recognizer> {
        if self.flags[i].has(Flags::PHYSICAL) {
            self.physical[i].as_ref()
        } else {
            self.gesture[i].as_ref()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use windows_scene::{HitEntry, NodeId, pack_offset};

    fn at(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    /// A 40 x 60 DIP knob whose top-left is at (40, 60) in layout DIPs, `scrolled` DIPs down
    /// inside a viewport when that is non-zero.
    fn knob_entry(id: ControlId, w: f32, h: f32, scrolled: f32) -> HitEntry {
        HitEntry {
            x0: 40.0,
            y0: 60.0 + scrolled,
            x1: 40.0 + w,
            y1: 60.0 + scrolled + h,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: NO_ENTRY,
            flags: HitFlags::INTERACTIVE | HitFlags::GESTURE,
            scroll_src: if scrolled == 0.0 {
                NodeId::NONE
            } else {
                NodeId::raw(3, 1)
            },
            id,
        }
    }

    /// Advances a reorderable drag from (100, 100) through `path`, returning the last update.
    fn walk(decl: DragDecl, path: &[Point]) -> DragUpdate {
        let from = at(100.0, 100.0);
        let mut phase = Phase::default();
        let mut last = advance(decl, from, &mut phase, from);
        for point in path {
            last = advance(decl, from, &mut phase, *point);
        }
        last
    }

    #[test]
    fn nothing_is_decided_before_the_threshold_so_a_nudge_is_a_click() {
        let update = walk(DragDecl::default(), &[at(104.0, 103.0)]);
        assert_eq!(update.phase, Phase::Undecided);
        assert_eq!(
            update.delta,
            at(0.0, 0.0),
            "an undecided drag reports travel"
        );
    }

    #[test]
    fn the_first_axis_past_the_threshold_owns_the_drag_for_its_whole_duration() {
        let decl = DragDecl::default();
        let from = at(100.0, 100.0);
        let mut phase = Phase::default();
        assert!(advance(decl, from, &mut phase, at(100.0, 110.0)).decided);
        assert_eq!(phase, Phase::Vertical);

        // A long horizontal excursion afterwards must not re-decide it, and must not leak into
        // the delta either.
        let update = advance(decl, from, &mut phase, at(400.0, 130.0));
        assert_eq!(update.phase, Phase::Vertical);
        assert_eq!(update.delta, at(0.0, 30.0));
        assert!(!update.decided, "the lock revisited itself");
    }

    #[test]
    fn a_batch_that_crosses_both_axes_at_once_locks_to_the_larger() {
        let update = walk(DragDecl::default(), &[at(120.0, 108.0)]);
        assert_eq!(update.phase, Phase::Horizontal);
        assert_eq!(update.delta, at(20.0, 0.0));
    }

    #[test]
    fn a_single_axis_declaration_is_never_decided_by_the_other_one() {
        let decl = DragDecl {
            horizontal: false,
            ..DragDecl::default()
        };
        let from = at(100.0, 100.0);
        let mut phase = Phase::default();
        assert_eq!(
            advance(decl, from, &mut phase, at(600.0, 100.0)).phase,
            Phase::Undecided
        );
        assert_eq!(
            advance(decl, from, &mut phase, at(600.0, 108.0)).phase,
            Phase::Vertical
        );
    }

    #[test]
    fn an_unlocked_drag_reports_both_axes() {
        let decl = DragDecl {
            lock: false,
            ..DragDecl::default()
        };
        let update = walk(decl, &[at(120.0, 109.0)]);
        assert_eq!(update.phase, Phase::Free);
        assert_eq!(update.delta, at(20.0, 9.0));
    }

    #[test]
    fn folding_the_batch_locks_to_the_axis_that_actually_crossed_first() {
        // This path goes out horizontally and comes back while descending, so the newest
        // sample alone is level and below the origin and locks vertical. The crossing happened
        // on the way out and was horizontal, and only walking every sample sees it.
        let path = [at(112.0, 102.0), at(108.0, 104.0), at(100.0, 112.0)];
        assert_eq!(walk(DragDecl::default(), &path).phase, Phase::Horizontal);
        assert_eq!(
            walk(DragDecl::default(), &path[2..]).phase,
            Phase::Vertical,
            "the newest sample alone reports the wrong axis, which is what folding prevents"
        );
    }

    #[test]
    fn the_default_declaration_gives_touch_the_context_menu_mouse_gets_free() {
        let decl = GestureDecl::default();
        assert!(decl.settings.contains(GestureSettings::RightTap));
        assert!(decl.settings.contains(GestureSettings::Hold));
        // Press-and-hold raises RightTapped, so touch reaches the same context-menu routing
        // the secondary button does.
        assert!(!decl.manipulates());
    }

    #[test]
    fn a_slider_manipulates_on_one_axis_and_never_inflates() {
        let decl = GestureDecl::slider(false);
        assert!(decl.manipulates());
        assert!(
            decl.settings
                .contains(GestureSettings::ManipulationTranslateX)
        );
        assert!(
            !decl
                .settings
                .contains(GestureSettings::ManipulationTranslateY)
        );
        assert_eq!(decl.touch_inflate, Some(0.0));
    }

    #[test]
    fn a_knob_is_a_marker_and_its_pivot_comes_from_the_array() {
        let decl = GestureDecl::knob();
        assert!(decl.settings.contains(GestureSettings::ManipulationRotate));
        assert!(decl.turned);
        // A knob is single-pointer rotation, so the hold that would claim the same contact is
        // bounded to one finger.
        assert_eq!(decl.hold.max_contacts, 1);

        let target = ControlId::FIRST;
        let mut hits = HitTable::default();
        hits.replace(&[knob_entry(target, 40.0, 60.0, 0.0)], &[(target, 0)]);
        let pivot = pivot_of(&hits, target, decl).expect("a turned control resolves a pivot");
        assert_eq!(pivot.center, at(60.0, 90.0));
        // Half the shorter side, so the whole control is inside the pivot and a contact on its
        // edge still rotates.
        assert!((pivot.radius - 20.0).abs() < 1e-6);
        // Rotation is not supported at radius zero, so a pivot that resolves at all is one the
        // platform acts on.
        assert!(pivot.radius > 0.0);
        assert_eq!(pivot_of(&hits, target, GestureDecl::tap()), None);
    }

    #[test]
    fn a_scrolled_dial_resolves_a_centre_that_includes_the_offset() {
        let target = ControlId::FIRST;
        let viewport = NodeId::raw(3, 1);
        let word = Arc::new(AtomicU64::new(pack_offset(0.0, 0.0)));
        let mut hits = HitTable::default();
        hits.replace(&[knob_entry(target, 40.0, 60.0, 200.0)], &[(target, 0)]);
        hits.set_shadows(&[(viewport, Arc::clone(&word))]);
        let decl = GestureDecl::knob();

        let unscrolled = pivot_of(&hits, target, decl).expect("a pivot");
        assert_eq!(unscrolled.center, at(60.0, 290.0));
        // Release, pairing with the acquire the table's own read does, so the centre resolved
        // below is a position this write finished.
        word.store(pack_offset(0.0, 200.0), Ordering::Release);
        let scrolled = pivot_of(&hits, target, decl).expect("a pivot");
        assert_eq!(
            scrolled.center,
            at(60.0, 90.0),
            "the dial moved under the finger and the centre did not follow it"
        );
        assert_eq!(scrolled.radius, unscrolled.radius);
    }

    #[test]
    fn the_default_hold_is_the_platforms_own_feel() {
        // Half a second, and ten DIPs of slack before a hold becomes a drag.
        let hold = HoldTuning::default();
        assert_eq!(hold.start_delay, Duration::from_millis(500));
        assert!((hold.radius - 10.0).abs() < 1e-6);
        assert_eq!((hold.min_contacts, hold.max_contacts), (1, 1));
    }

    #[test]
    fn a_duration_crosses_as_hundreds_of_nanoseconds() {
        assert_eq!(timespan(Duration::from_millis(500)).duration, 5_000_000);
        // Saturating rather than wrapping: a delay nobody would ever wait out is still a
        // delay, and a negative one would fire immediately.
        assert!(timespan(Duration::MAX).duration > 0);
    }

    #[test]
    fn the_touchpad_subset_drops_what_that_recogniser_cannot_do() {
        // `DoubleTap`, `RightTap`, `Drag` and `CrossSlide` are not in the subset, and the
        // masking is what keeps a declaration from being rejected rather than ignored.
        let asked = GestureSettings::Tap
            | GestureSettings::DoubleTap
            | GestureSettings::RightTap
            | GestureSettings::ManipulationTranslateX;
        let given = GestureSettings(asked.0 & PHYSICAL_SETTINGS);
        assert!(given.contains(GestureSettings::Tap));
        assert!(given.contains(GestureSettings::ManipulationTranslateX));
        assert!(!given.contains(GestureSettings::DoubleTap));
        assert!(!given.contains(GestureSettings::RightTap));
    }

    #[test]
    fn a_manipulation_delta_crosses_without_losing_an_axis() {
        let delta = ManipulationDelta {
            translation: WinPoint { x: 3.0, y: -4.0 },
            scale: 1.5,
            rotation: 30.0,
            expansion: 12.0,
        };
        let manip: Manip = delta.into();
        assert_eq!(manip.translation, Point { x: 3.0, y: -4.0 });
        assert_eq!(manip.scale, 1.5);
        assert_eq!(manip.rotation, 30.0);
        assert_eq!(manip.expansion, 12.0);
    }

    #[test]
    fn the_event_queue_drains_in_order_and_a_cancel_empties_it() {
        let events = Events::default();
        events.push(Recognised::RightTapped {
            at: Point { x: 1.0, y: 2.0 },
        });
        events.push(Recognised::ManipulationStarted {
            at: Point { x: 3.0, y: 4.0 },
        });
        let mut out = Vec::new();
        events.drain(&mut out);
        assert_eq!(out.len(), 2);
        assert!(matches!(out[0], Recognised::RightTapped { .. }));

        events.push(Recognised::Tapped {
            at: Point { x: 0.0, y: 0.0 },
            count: 1,
        });
        events.clear();
        out.clear();
        events.drain(&mut out);
        assert!(out.is_empty(), "an aborted gesture still delivered");
    }

    /// Binds one mouse contact to the first minted control.
    fn bind(contacts: &mut Contacts, id: u32) {
        contacts
            .bind(
                id,
                ControlId::FIRST,
                GestureDecl::default(),
                false,
                Point::default(),
                false,
            )
            .expect("a recogniser could not be constructed");
    }

    #[test]
    fn double_tap_reuses_its_recognizer_before_an_earlier_free_slot() -> Result<()> {
        let mut contacts = Contacts::default();
        bind(&mut contacts, 7);
        let decl = GestureDecl::double_tap();
        contacts.bind(8, ControlId::FIRST, decl, false, Point::default(), false)?;
        let slot = contacts.at(8).unwrap();
        let Kind::Gesture(recognizer) = &contacts.gesture[slot].as_ref().unwrap().kind else {
            panic!("mouse contact requires GestureRecognizer");
        };
        let recognizer = recognizer.clone();
        let settings = decl.settings | GestureSettings::Hold;
        recognizer.SetGestureSettings(settings)?;
        contacts.set(8, Flags::TAPPED, true);
        contacts.release(8, false);
        contacts.release(7, false);
        assert_eq!(contacts.live(), 0);
        assert!(!contacts.any_inertial());
        assert_eq!(contacts.tap, Some(slot));
        contacts.bind(9, ControlId::FIRST, decl, false, Point::default(), false)?;
        assert_eq!(contacts.at(9), Some(slot));
        let Kind::Gesture(reused) = &contacts.gesture[slot].as_ref().unwrap().kind else {
            panic!("mouse contact requires GestureRecognizer");
        };
        assert_eq!(recognizer.as_raw(), reused.as_raw());
        assert_eq!(reused.GestureSettings()?, settings, "continuation must not reconfigure");
        assert!(contacts.tap.is_none());
        contacts.set(9, Flags::TAPPED, true);
        contacts.release(9, false);
        contacts.release_all(true);
        assert!(contacts.tap.is_none(), "focus loss also cancels an unbound tap");
        contacts.bind(10, ControlId::FIRST, decl, false, Point::default(), false)?;
        let Kind::Gesture(reset) = &contacts.recognizer(contacts.at(10).unwrap()).unwrap().kind else {
            panic!("mouse contact requires GestureRecognizer");
        };
        assert_eq!(reset.GestureSettings()?, decl.settings);
        Ok(())
    }

    #[test]
    fn double_tap_history_rejects_changed_targets_settings_and_canceled_contacts() -> Result<()> {
        let mut ids = windows_scene::Ids::default();
        let target = ids.mint();
        let other = ids.mint();
        let decl = GestureDecl::double_tap();
        for (next_target, next_decl, rejected) in [
            (other, decl, false),
            (target, GestureDecl::tap(), false),
            (target, decl, true),
        ] {
            let mut contacts = Contacts::default();
            contacts.bind(1, target, decl, false, Point::default(), false)?;
            contacts.set(1, Flags::TAPPED, true);
            contacts.release(1, false);
            assert!(contacts.tap.is_some());
            contacts.bind(2, next_target, next_decl, false, Point::default(), rejected)?;
            assert!(contacts.tap.is_none());
            let slot = contacts.at(2).unwrap();
            let Kind::Gesture(recognizer) = &contacts.gesture[slot].as_ref().unwrap().kind else {
                panic!("mouse contact requires GestureRecognizer");
            };
            assert_eq!(recognizer.GestureSettings()?, next_decl.settings);
            contacts.set(2, Flags::TAPPED, true);
            contacts.release(2, true);
            assert!(contacts.tap.is_none());
        }
        let mut contacts = Contacts::default();
        contacts.bind(1, target, decl, false, Point::default(), false)?;
        contacts.release(1, false);
        assert!(contacts.tap.is_none(), "an unrecognized release cannot retain tap history");
        contacts.bind(2, target, decl, false, Point::default(), false)?;
        contacts.set(2, Flags::TAPPED, true);
        contacts.release(2, false);
        contacts.forget_tap(other);
        assert!(contacts.tap.is_some(), "unrelated retirement must preserve the candidate");
        contacts.forget_tap(target);
        assert!(contacts.tap.is_none());
        Ok(())
    }

    #[test]
    fn a_slot_is_taken_on_bind_and_free_on_release() {
        let mut contacts = Contacts::default();
        bind(&mut contacts, 7);
        assert_eq!(contacts.live(), 1);
        assert_eq!(
            contacts.bound(7).map(|(target, _)| target),
            Some(ControlId::FIRST)
        );
        // The id column and the count answer the same question, so a walk over the column and
        // a `live()` guard cannot disagree about which slots are taken.
        assert_eq!(
            contacts.ids().iter().filter(|id| **id != 0).count(),
            contacts.live()
        );
        contacts.release(7, false);
        assert_eq!(contacts.live(), 0);
        assert!(contacts.at(7).is_none());
        assert_eq!(contacts.ids(), [0; SLOTS]);
        // The slot is reusable, and its recogniser was never handed back to anything.
        bind(&mut contacts, 8);
        assert_eq!(contacts.live(), 1);
    }

    #[test]
    fn a_knobs_pivot_is_stated_at_bind_and_restated_per_update() {
        let mut contacts = Contacts::default();
        let decl = GestureDecl::knob();
        assert!(decl.settings.contains(GestureSettings::ManipulationRotate));
        contacts
            .bind(1, ControlId::FIRST, decl, false, Point::default(), false)
            .expect("a knob's declaration is one the platform accepts");
        // Both values have to stay current through the interaction, so restating them is a
        // call the recogniser accepts as often as it is asked.
        let pivot = Some(Pivot {
            radius: 24.0,
            center: at(24.0, 24.0),
        });
        contacts.restate_pivot(1, pivot).expect("a pivot was refused");
        contacts
            .restate_pivot(1, pivot)
            .expect("a second statement was refused");

        // A target that declares no pivot turns single-pointer rotation off rather than
        // leaving the last knob's radius on the slot's recogniser.
        contacts.release(1, false);
        contacts
            .bind(
                1,
                ControlId::FIRST,
                GestureDecl::tap(),
                false,
                Point::default(),
                false,
            )
            .expect("a plain target reuses the slot");
        contacts
            .restate_pivot(1, None)
            .expect("turning the pivot off was refused");
    }

    #[test]
    fn a_contact_whose_recogniser_stopped_is_no_longer_inertial() {
        let mut contacts = Contacts::default();
        bind(&mut contacts, 7);
        contacts.set(7, Flags::INERTIAL, true);
        assert!(contacts.any_inertial());
        // The flag says the contact lifted with motion left; the recogniser, which was never
        // fed, says the motion is over. The second is what releases the slot.
        assert!(!contacts.still_inertial(7));
        contacts.release(7, false);
        assert!(!contacts.any_inertial());
    }

    #[test]
    fn content_touch_uses_the_nearest_mounted_scroll_but_editing_keeps_ownership() {
        use crate::build::{Host, tests::fixture};
        use crate::layout::{Len, scroll};

        let mut patch = fixture();
        let _held = crate::build::Ui::mount_root(|ui| {
            scroll(ui, |ui| {
                scroll(ui, |ui| {
                    crate::widget::button(ui, "band");
                })
                .height(Len::times(crate::role::Metric::RowH, 6.0));
            })
            .height(Len::times(crate::role::Metric::RowH, 12.0));
        });
        let mut down = crate::seam::Down::default();
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        let mut hits = HitTable::default();
        hits.replace(patch.hits(patch.hits_span()), patch.index(patch.index_span()));
        // The button is the one control carrying a wash, so it is the content touch's target.
        let (id, _) = down
            .chrome
            .iter()
            .find(|(_, row)| !row.wash.0.is_none())
            .expect("the button declared no wash");
        let index = hits
            .entries()
            .iter()
            .position(|entry| entry.id == *id)
            .unwrap();
        let entry = &hits.entries()[index];
        let hit = Hit {
            index: index as u32,
            id: entry.id,
            flags: entry.flags,
            local: Point::default(),
        };
        let scrolls: Vec<_> = hits
            .entries()
            .iter()
            .filter(|entry| entry.flags.contains(HitFlags::SCROLL))
            .collect();
        assert_eq!(scrolls.len(), 2);
        assert_eq!(
            scroll_offer(&hits, hit, GestureDecl::tap()),
            Some(scrolls[1].id)
        );
        assert_eq!(scroll_offer(&hits, hit, GestureDecl::slider(true)), None);
        assert_eq!(
            scroll_offer(&hits, hit, GestureDecl::knob()),
            None
        );
        let text = Hit {
            flags: hit.flags | HitFlags::TEXT,
            ..hit
        };
        assert_eq!(scroll_offer(&hits, text, GestureDecl::tap()), None);
    }
}
