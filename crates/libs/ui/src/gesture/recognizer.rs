//! The two recognisers, behind one shape.
//!
//! | Pointer type | Recogniser |
//! |---|---|
//! | touch, pen, mouse | `Windows.UI.Input.GestureRecognizer` |
//! | precision touchpad | `Windows.UI.Input.PhysicalGestureRecognizer` |
//!
//! Both emit the **same** `Windows.UI.Input` event argument types, so the sink downstream is
//! one code path regardless of device. They differ in the coordinate space of their output —
//! the physical one reports relative to the device rather than in display-independent pixels
//! — and in what they recognise at all: the platform never recognises touchpad input as Tap
//! or Hold.
//!
//! Both are **non-agile** (`MarshalingBehavior(None)`), so both are created and used only on
//! the front thread — the same constraint the compositor and text services already impose,
//! and the reason they are held in a [`FrontHandle`](crate::FrontHandle).

use super::decl::{GestureDecl, PivotDecl};
use crate::bindings::{Point as WinPoint, *};
use std::cell::RefCell;
use std::rc::Rc;
use windows_collections::IVector;
use windows_core::{EventRevoker, Interface, Result};
use windows_scene::Point;

/// A manipulation's translation, scale, rotation and expansion, in the space the hit array
/// is built in.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Manip {
    pub translation: Point,
    pub scale: f32,
    /// Degrees, positive clockwise: the platform's convention, carried unconverted so a
    /// value from the system reads the same here as in its own documentation.
    pub rotation: f32,
    pub expansion: f32,
}

impl From<ManipulationDelta> for Manip {
    fn from(delta: ManipulationDelta) -> Self {
        Self {
            translation: Point {
                x: delta.translation.x,
                y: delta.translation.y,
            },
            scale: delta.scale,
            rotation: delta.rotation,
            expansion: delta.expansion,
        }
    }
}

/// How fast a manipulation was going when it ended, which is what inertia is started from.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Velocities {
    pub linear: Point,
    pub angular: f32,
    pub expansion: f32,
}

impl From<ManipulationVelocities> for Velocities {
    fn from(v: ManipulationVelocities) -> Self {
        Self {
            linear: Point {
                x: v.linear.x,
                y: v.linear.y,
            },
            angular: v.angular,
            expansion: v.expansion,
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
    /// tuned** — the platform documents these settings as unchangeable after this event.
    InertiaStarting {
        at: Point,
        velocities: Velocities,
    },
    ManipulationCompleted {
        at: Point,
        cumulative: Manip,
        velocities: Velocities,
    },
}

/// Where a recogniser's events land.
///
/// Shared by every recogniser in the pool and drained by the router immediately after each
/// feed, because the platform raises these **synchronously** from inside `ProcessDownEvent`
/// and friends. So the binding is always the one the router just fed.
#[derive(Clone, Default)]
pub struct Events(Rc<RefCell<Vec<Recognised>>>);

impl Events {
    /// Returns an empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Discards whatever is queued. What a canceled contact does, so an abort delivers no
    /// part of the gesture it aborted.
    pub fn clear(&self) {
        if let Ok(mut queue) = self.0.try_borrow_mut() {
            queue.clear();
        }
    }
}

fn point(p: WinPoint) -> Point {
    Point { x: p.x, y: p.y }
}

/// Which of the two the contact's device needs.
enum Kind {
    Gesture(GestureRecognizer),
    Physical(PhysicalGestureRecognizer),
}

/// One recogniser, configured per contact and returned to the pool when the contact ends.
pub struct Recognizer {
    kind: Kind,
    /// Held for their `Drop`, which revokes. The handlers outlive every individual binding —
    /// a pooled recogniser is configured again rather than re-subscribed — so this is set up
    /// once, when the object is minted.
    _revokers: Vec<EventRevoker>,
}

impl Recognizer {
    /// Returns a recogniser for touch, pen and mouse, wired to `events`.
    ///
    /// # Errors
    ///
    /// The recogniser could not be constructed, or one of its handlers could not be
    /// registered.
    pub fn gesture(events: &Events) -> Result<Self> {
        let inner = GestureRecognizer::new()?;
        let mut revokers = Vec::with_capacity(8);
        let sink = events.clone();
        revokers.push(inner.Tapped(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(count)) = (args.Position(), args.TapCount())
            {
                sink.push(Recognised::Tapped {
                    at: point(at),
                    count,
                });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.RightTapped(move |_, args| {
            if let Some(args) = args.as_ref()
                && let Ok(at) = args.Position()
            {
                sink.push(Recognised::RightTapped { at: point(at) });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.Holding(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(state)) = (args.Position(), args.HoldingState())
            {
                sink.push(Recognised::Holding {
                    at: point(at),
                    state,
                });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.Dragging(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(state)) = (args.Position(), args.DraggingState())
            {
                sink.push(Recognised::Dragging {
                    at: point(at),
                    state,
                });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.ManipulationStarted(move |_, args| {
            if let Some(args) = args.as_ref()
                && let Ok(at) = args.Position()
            {
                sink.push(Recognised::ManipulationStarted { at: point(at) });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.ManipulationUpdated(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(delta), Ok(cumulative)) =
                    (args.Position(), args.Delta(), args.Cumulative())
            {
                sink.push(Recognised::ManipulationUpdated {
                    at: point(at),
                    delta: delta.into(),
                    cumulative: cumulative.into(),
                });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.ManipulationInertiaStarting(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(velocities)) = (args.Position(), args.Velocities())
            {
                sink.push(Recognised::InertiaStarting {
                    at: point(at),
                    velocities: velocities.into(),
                });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.ManipulationCompleted(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(cumulative), Ok(velocities)) =
                    (args.Position(), args.Cumulative(), args.Velocities())
            {
                sink.push(Recognised::ManipulationCompleted {
                    at: point(at),
                    cumulative: cumulative.into(),
                    velocities: velocities.into(),
                });
            }
        })?);
        Ok(Self {
            kind: Kind::Gesture(inner),
            _revokers: revokers,
        })
    }

    /// Returns a recogniser for precision-touchpad contacts, wired to `events`.
    ///
    /// It carries no inertia of its own — there is no `ProcessInertia` on it — because the
    /// system continues a touchpad manipulation itself and reports it through the inertia
    /// messages.
    ///
    /// # Errors
    ///
    /// The recogniser could not be constructed, or one of its handlers could not be
    /// registered.
    pub fn physical(events: &Events) -> Result<Self> {
        let inner = PhysicalGestureRecognizer::new()?;
        let mut revokers = Vec::with_capacity(5);
        let sink = events.clone();
        revokers.push(inner.ManipulationStarted(move |_, args| {
            if let Some(args) = args.as_ref()
                && let Ok(at) = args.Position()
            {
                sink.push(Recognised::ManipulationStarted { at: point(at) });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.ManipulationUpdated(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(delta), Ok(cumulative)) =
                    (args.Position(), args.Delta(), args.Cumulative())
            {
                sink.push(Recognised::ManipulationUpdated {
                    at: point(at),
                    delta: delta.into(),
                    cumulative: cumulative.into(),
                });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.ManipulationCompleted(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(cumulative), Ok(velocities)) =
                    (args.Position(), args.Cumulative(), args.Velocities())
            {
                sink.push(Recognised::ManipulationCompleted {
                    at: point(at),
                    cumulative: cumulative.into(),
                    velocities: velocities.into(),
                });
            }
        })?);
        // Wired although the platform documents that touchpad input is never recognised as
        // either: a device that starts producing them is a device this stack should route,
        // not one it should silently drop.
        let sink = events.clone();
        revokers.push(inner.Tapped(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(count)) = (args.Position(), args.TapCount())
            {
                sink.push(Recognised::Tapped {
                    at: point(at),
                    count,
                });
            }
        })?);
        let sink = events.clone();
        revokers.push(inner.Holding(move |_, args| {
            if let Some(args) = args.as_ref()
                && let (Ok(at), Ok(state)) = (args.Position(), args.HoldingState())
            {
                sink.push(Recognised::Holding {
                    at: point(at),
                    state,
                });
            }
        })?);
        Ok(Self {
            kind: Kind::Physical(inner),
            _revokers: revokers,
        })
    }

    /// Returns whether this is the precision-touchpad recogniser.
    #[must_use]
    pub const fn is_physical(&self) -> bool {
        matches!(self.kind, Kind::Physical(_))
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
    pub fn configure(&self, decl: &GestureDecl) -> Result<()> {
        match &self.kind {
            Kind::Gesture(inner) => {
                inner.SetGestureSettings(decl.settings)?;
                // This stack draws its own feedback everywhere; the system's would be a
                // second affordance for the same contact.
                inner.SetShowGestureFeedback(false)?;
                // Inertia is advanced by the frame tick rather than by a clock of the
                // recogniser's own — there is no fourth clock.
                inner.SetAutoProcessInertia(false)?;
                // Hold tuning and the contact-count bounds live on the class's second
                // interface, reached by cast. A knob that must not be claimed by a
                // two-finger contact needs the maxima, not just the delay.
                let tuning: IGestureRecognizer2 = inner.cast()?;
                tuning.SetHoldStartDelay(timespan(decl.hold.start_delay))?;
                tuning.SetHoldRadius(decl.hold.radius)?;
                tuning.SetHoldMinContactCount(decl.hold.min_contacts)?;
                tuning.SetHoldMaxContactCount(decl.hold.max_contacts)?;
                match decl.pivot {
                    Some(pivot) => self.pivot(pivot)?,
                    // Zero is what turns single-pointer rotation off, and a recogniser
                    // coming back from the pool still carries the last knob's radius.
                    None => inner.SetPivotRadius(0.0)?,
                }
                Ok(())
            }
            Kind::Physical(inner) => {
                inner.SetGestureSettings(GestureSettings(decl.settings.0 & PHYSICAL_SETTINGS))?;
                inner.SetHoldStartDelay(timespan(decl.hold.start_delay))?;
                inner.SetHoldRadius(decl.hold.radius)
            }
        }
    }

    /// Restates the pivot centre and radius.
    ///
    /// Must be called on **every** `ManipulationUpdated` rather than once at down: the
    /// platform documents both values as ones to update regularly during the interaction,
    /// and a stale centre makes a knob drift under a finger that has not moved. Does nothing
    /// on the physical recogniser, which has no pivot.
    ///
    /// # Errors
    ///
    /// The platform refused the centre or the radius.
    pub fn pivot(&self, pivot: PivotDecl) -> Result<()> {
        let Kind::Gesture(inner) = &self.kind else {
            return Ok(());
        };
        inner.SetPivotCenter(WinPoint {
            x: pivot.center.x,
            y: pivot.center.y,
        })?;
        inner.SetPivotRadius(pivot.radius)
    }

    /// Sets the translation, rotation and expansion decelerations inertia runs down at.
    ///
    /// Must be called from inside the `InertiaStarting` handler: the platform rejects a
    /// change to these after that event. Does nothing on the physical recogniser, which runs
    /// no inertia of its own.
    ///
    /// # Errors
    ///
    /// The platform refused one of the decelerations.
    pub fn decelerate(&self, translation: f32, rotation: f32, expansion: f32) -> Result<()> {
        let Kind::Gesture(inner) = &self.kind else {
            return Ok(());
        };
        inner.SetInertiaTranslationDeceleration(translation)?;
        inner.SetInertiaRotationDeceleration(rotation)?;
        inner.SetInertiaExpansionDeceleration(expansion)
    }

    /// Feeds the contact's down sample.
    ///
    /// # Errors
    ///
    /// The platform refused the sample.
    pub fn down(&self, p: &PointerPoint) -> Result<()> {
        match &self.kind {
            Kind::Gesture(inner) => inner.ProcessDownEvent(p),
            Kind::Physical(inner) => inner.ProcessDownEvent(p),
        }
    }

    /// Feeds the frame's batch of move samples, oldest first.
    ///
    /// `ProcessMoveEvents` takes the intermediate points for a frame, which is what the
    /// frame-clock batch holds, so the recogniser sees the whole path rather than its
    /// newest point.
    ///
    /// # Errors
    ///
    /// The platform refused the batch.
    pub fn moves(&self, batch: &IVector<PointerPoint>) -> Result<()> {
        match &self.kind {
            Kind::Gesture(inner) => inner.ProcessMoveEvents(batch),
            Kind::Physical(inner) => inner.ProcessMoveEvents(batch),
        }
    }

    /// Feeds the contact's up sample.
    ///
    /// # Errors
    ///
    /// The platform refused the sample.
    pub fn up(&self, p: &PointerPoint) -> Result<()> {
        match &self.kind {
            Kind::Gesture(inner) => inner.ProcessUpEvent(p),
            Kind::Physical(inner) => inner.ProcessUpEvent(p),
        }
    }

    /// Feeds a wheel notch over a target that declared gesture interest.
    ///
    /// A scroll surface never reaches here: its wheel drives the tracker compositor-side,
    /// with no front-thread work. Does nothing on the physical recogniser.
    ///
    /// # Errors
    ///
    /// The platform refused the notch.
    pub fn wheel(&self, p: &PointerPoint, shift: bool, ctrl: bool) -> Result<()> {
        let Kind::Gesture(inner) = &self.kind else {
            return Ok(());
        };
        inner.ProcessMouseWheelEvent(p, shift, ctrl)
    }

    /// Advances inertia by one frame.
    ///
    /// Pumped from the pacer tick, so inertia, springs and a system stop request all resolve
    /// on one clock. Does nothing on the physical recogniser, which the system pumps itself.
    ///
    /// # Errors
    ///
    /// The platform refused to advance inertia.
    pub fn inertia(&self) -> Result<()> {
        let Kind::Gesture(inner) = &self.kind else {
            return Ok(());
        };
        inner.ProcessInertia()
    }

    /// Returns whether inertia is still running, which is what keeps a tick requested.
    #[must_use]
    pub fn is_inertial(&self) -> bool {
        match &self.kind {
            Kind::Gesture(inner) => inner.IsInertial().unwrap_or(false),
            Kind::Physical(_) => false,
        }
    }

    /// Returns whether a gesture is in progress at all.
    #[must_use]
    pub fn is_active(&self) -> bool {
        match &self.kind {
            Kind::Gesture(inner) => inner.IsActive().unwrap_or(false),
            Kind::Physical(inner) => inner.IsActive().unwrap_or(false),
        }
    }

    /// Ends whatever is in progress, discarding it.
    ///
    /// **What a cancel does**, and not an up: no value is committed.
    ///
    /// # Errors
    ///
    /// The platform refused to complete the gesture.
    pub fn complete(&self) -> Result<()> {
        match &self.kind {
            Kind::Gesture(inner) => inner.CompleteGesture(),
            Kind::Physical(inner) => inner.CompleteGesture(),
        }
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

/// A duration as the platform's 100-nanosecond count.
fn timespan(duration: core::time::Duration) -> windows_time::TimeSpan {
    windows_time::TimeSpan {
        duration: (duration.as_nanos() / 100).min(i64::MAX as u128) as i64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_crosses_as_hundreds_of_nanoseconds() {
        assert_eq!(
            timespan(core::time::Duration::from_millis(500)).duration,
            5_000_000
        );
        // Saturating rather than wrapping: a delay nobody would ever wait out is still a
        // delay, and a negative one would fire immediately.
        assert!(timespan(core::time::Duration::MAX).duration > 0);
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
        let events = Events::new();
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
}
