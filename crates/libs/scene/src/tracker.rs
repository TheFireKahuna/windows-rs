//! Interaction trackers, and scroll. **Front half.**
//!
//! A tracker is a value the composition engine drives from a manipulation and from inertia,
//! **in another process**. Every call into it and every callback out of it is asynchronous,
//! and that single fact shapes everything here: the position is read only from a
//! values-changed callback, and a request may be dropped silently by design.
//!
//! **An owner is not free and it is all-or-nothing.** It is supplied at construction and
//! there is no per-callback subscription, so a tracker that needs one event pays for all
//! six — measured at ~19× against an ownerless one over the same fling. So the type answers
//! "which surfaces qualify": a surface that is neither virtualized nor driven by explicit
//! position requests **cannot** be given callbacks it does not read, because it is created
//! as [`Passive`](crate::Passive) and `request` does not accept one.

use crate::sink::{NodeId, TrackerRequest, Tracker as TrackerFamily};
use core::cell::RefCell;
use std::rc::Rc;
use windows_composition::{
    ChainingMode, Clamping, InteractionTracker, RedirectionMode, RequestId, ScaleAnimationPolicy,
    SourceMode, TrackerEvent, VisualInteractionSource, WheelMode,
};
use windows_core::Result;
use windows_numerics::{Vector2, Vector3};

/// What phase a tracker is in, as its last reported transition said.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Interacting,
    Inertia,
    CustomAnimation,
}

/// How many requests can be outstanding before the oldest is forgotten.
///
/// A bounded array and not a map: a mouse drag issues one request per frame and every
/// values-changed clears the set, so it never holds more than a frame or two of latency —
/// and a map there would allocate on the drag path for a collection that never grows. A
/// linear scan over eight entries is both faster and inside the zero-allocation rule.
const PENDING: usize = 8;

/// One tracker and everything known about it on this side.
pub(crate) struct TrackerState {
    pub(crate) tracker: InteractionTracker,
    pub(crate) source: Option<VisualInteractionSource>,
    /// The group this tracker scrolls. The hit array resolves a scroll ancestry through it,
    /// so a reported position has somewhere to land.
    pub(crate) viewport: Option<NodeId>,
    /// The last values reported. **The only trustworthy read**: the tracker runs in another
    /// process and its own getter would answer with whatever was last *set*.
    pub(crate) position: Vector3,
    pub(crate) scale: f32,
    pub(crate) phase: Phase,
    pending: [Option<(i32, TrackerRequest)>; PENDING],
}

impl TrackerState {
    pub(crate) fn new(tracker: InteractionTracker) -> Self {
        Self {
            tracker,
            source: None,
            viewport: None,
            position: Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            scale: 1.0,
            phase: Phase::Idle,
            pending: [None; PENDING],
        }
    }

    /// Issues a request and holds it against its id.
    ///
    /// **Never assume one applied.** A position update arriving while the user is actively
    /// manipulating is documented to be dropped, and the drop is silent.
    pub(crate) fn request(&mut self, request: TrackerRequest) -> Result<RequestId> {
        let id = match request {
            TrackerRequest::To(p) => self.tracker.try_update_position(
                Vector3 {
                    x: p.x,
                    y: p.y,
                    z: 0.0,
                },
                Clamping::Auto,
                // Stated rather than defaulted, because the default silently stops a
                // running custom scale animation.
                ScaleAnimationPolicy::Keep,
            )?,
            TrackerRequest::By(d) => self.tracker.try_update_position_by(
                Vector3 {
                    x: d.x,
                    y: d.y,
                    z: 0.0,
                },
                Clamping::Auto,
            )?,
            TrackerRequest::Fling(v) => {
                self.tracker
                    .try_update_position_with_additional_velocity(Vector3 {
                        x: v.x,
                        y: v.y,
                        z: 0.0,
                    })?
            }
        };
        self.remember(id.0, request);
        Ok(id)
    }

    fn remember(&mut self, id: i32, request: TrackerRequest) {
        if let Some(slot) = self.pending.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some((id, request));
            return;
        }
        // Full means the tracker has not reported in several frames, which is itself the
        // signal: the oldest entry matters least, and dropping it is
        // better than growing a buffer on the drag path.
        self.pending.rotate_left(1);
        self.pending[PENDING - 1] = Some((id, request));
    }

    /// Drops a request the system reported as ignored.
    ///
    /// It is **not** re-applied. Re-applying blindly gives a user whose manipulation ends a
    /// double jump, which is the failure this reconciliation exists to prevent.
    pub(crate) fn ignored(&mut self, id: i32) {
        for slot in &mut self.pending {
            if slot.is_some_and(|(pending, _)| pending == id) {
                *slot = None;
            }
        }
    }

    /// Records the one trustworthy read, and clears the pending set.
    pub(crate) fn values_changed(&mut self, position: Vector3, scale: f32) {
        self.position = position;
        self.scale = scale;
        self.pending = [None; PENDING];
    }


}

/// Configures the source a manipulation is collected on.
///
/// The source visual is both the hit-test target and the gesture's coordinate space, so it
/// **must not move during the manipulation** — which is why it is the scroll container's
/// viewport and never the content that scrolls inside it. No visual exists purely for
/// input.
pub(crate) fn configure_source(
    source: &VisualInteractionSource,
    axes: crate::sink::Axes,
) -> Result<()> {
    let mode = |on: bool| {
        if on {
            SourceMode::EnabledWithInertia
        } else {
            SourceMode::Disabled
        }
    };
    source.set_axis_modes(mode(axes.x), mode(axes.y), mode(axes.scale));
    // Rails: a pan started primarily on one axis locks to it. Wanted whenever both are
    // live — a vertical list should not drift sideways — and meaningless when only one is.
    source.set_rails(axes.x && axes.y, axes.x && axes.y);
    // Precision touchpad and wheel arrive without the window's help. Touch and pen must be
    // redirected explicitly, and mouse cannot be redirected at all — which is why a mouse
    // drag is driven by explicit position requests instead.
    source.set_redirection_mode(RedirectionMode::TouchpadAndWheel);
    // Nested scrollers hand off at their bounds with no hand-written plumbing.
    source.set_chaining(ChainingMode::Auto, ChainingMode::Auto, ChainingMode::Auto);
    // Wheel drives Y only; X and scale are left to touch and the touchpad. With this set, a
    // wheel message over a scroll container needs **no front-thread handling whatsoever**.
    source.set_wheel_modes(
        WheelMode::Disabled,
        if axes.y {
            WheelMode::Enabled
        } else {
            WheelMode::Disabled
        },
        WheelMode::Disabled,
    )
}

/// The queue a tracker's owner callbacks push into, drained on the front thread's tick.
///
/// Shared rather than owned because the callback is a COM object the compositor holds, and
/// it outlives the borrow that created it. Nothing here is `Send`: the callbacks arrive on
/// the thread that created the compositor, which is the thread the tree lives on.
pub(crate) type EventQueue = Rc<RefCell<Vec<crate::SceneEvent>>>;

/// Turns a wrapper tracker event into this crate's, tagged with which tracker it came from.
pub(crate) fn translate(
    id: crate::id::Id<TrackerFamily>,
    event: TrackerEvent,
) -> crate::SceneEvent {
    use crate::SceneEvent as Out;
    let flat = |v: Vector3| Vector2 { x: v.x, y: v.y };
    match event {
        TrackerEvent::ValuesChanged {
            position, scale, ..
        } => Out::TrackerValues {
            tracker: id,
            position: flat(position),
            scale,
        },
        TrackerEvent::InteractingStateEntered { .. } => Out::TrackerPhase {
            tracker: id,
            phase: Phase::Interacting,
        },
        TrackerEvent::InertiaStateEntered {
            natural_resting_position,
            modified_resting_position,
            from_impulse,
            ..
        } => Out::InertiaStarting {
            tracker: id,
            natural: flat(natural_resting_position),
            modified: flat(modified_resting_position),
            from_impulse,
        },
        TrackerEvent::IdleStateEntered { .. } => Out::TrackerPhase {
            tracker: id,
            phase: Phase::Idle,
        },
        TrackerEvent::CustomAnimationStateEntered { .. } => Out::TrackerPhase {
            tracker: id,
            phase: Phase::CustomAnimation,
        },
        TrackerEvent::RequestIgnored { request } => Out::RequestIgnored {
            tracker: id,
            request: request.0,
        },
    }
}
