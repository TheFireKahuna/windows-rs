//! Interaction trackers, and scroll. **Front half.**
//!
//! A tracker is a value the composition engine drives from a manipulation and from inertia,
//! in another process. Every call into it and every callback out of it is asynchronous: the
//! position is read only from a values-changed callback, and a request may be dropped.
//!
//! An owner is supplied at construction and there is no per-callback subscription, so a
//! tracker that needs one event pays for all six — measured at ~19× the callback cost of an
//! ownerless tracker over the same fling. A surface that is neither virtualized nor driven
//! by explicit position requests is created as [`Passive`](crate::Passive), which `request`
//! does not accept, so it cannot be given callbacks it does not read.

use crate::sink::{NodeId, Tracker as TrackerFamily, TrackerRequest};
use core::cell::RefCell;
use std::rc::Rc;
use windows_composition::{
    ChainingMode, Clamping, InteractionTracker, RedirectionMode, RequestId, ScaleAnimationPolicy,
    SourceMode, TrackerEvent, VisualInteractionSource, WheelMode,
};
use windows_core::Result;
use windows_numerics::{Vector2, Vector3};

/// The phase a tracker's last reported transition put it in.
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
/// The pending set is a fixed array because the drag path allocates nothing: a mouse drag
/// issues one request per frame and every values-changed callback clears the set, so it
/// holds a frame or two of latency and a linear scan over eight entries resolves a reply.
const PENDING: usize = 8;

/// One tracker and everything known about it on this side.
pub(crate) struct TrackerState {
    pub(crate) tracker: InteractionTracker,
    pub(crate) source: Option<VisualInteractionSource>,
    /// The group this tracker scrolls. The hit array resolves a scroll ancestry through it,
    /// so a reported position has somewhere to land.
    pub(crate) viewport: Option<NodeId>,
    /// The last values reported by a values-changed callback, and the only sound read: the
    /// tracker runs in another process and its own getter answers with whatever was last
    /// set.
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
    /// A request is not an assignment: a position update arriving while the user is
    /// manipulating is documented as dropped, and the tracker reports the drop only as an
    /// ignored request.
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
        // A full set means the tracker has not reported in several frames. The oldest entry
        // is the least useful one to keep, and the drag path allocates nothing.
        self.pending.rotate_left(1);
        self.pending[PENDING - 1] = Some((id, request));
    }

    /// Drops a request the system reported as ignored.
    ///
    /// The request is not re-applied: re-applying it would jump a second time once the
    /// user's manipulation ends.
    pub(crate) fn ignored(&mut self, id: i32) {
        for slot in &mut self.pending {
            if slot.is_some_and(|(pending, _)| pending == id) {
                *slot = None;
            }
        }
    }

    /// Records the reported values and clears the pending set.
    pub(crate) fn values_changed(&mut self, position: Vector3, scale: f32) {
        self.position = position;
        self.scale = scale;
        self.pending = [None; PENDING];
    }
}

/// Configures the source a manipulation is collected on.
///
/// The source visual is both the hit-test target and the gesture's coordinate space, so it
/// must not move during the manipulation: it is the scroll container's viewport and never
/// the content scrolling inside it.
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
    // Rails lock a pan to the axis it started on, so a vertical list does not drift
    // sideways. Meaningful only while both axes are live.
    source.set_rails(axes.x && axes.y, axes.x && axes.y);
    // Precision touchpad and wheel arrive without the window's help. Touch and pen need
    // explicit redirection, and mouse cannot be redirected at all, so a mouse drag is driven
    // by explicit position requests.
    source.set_redirection_mode(RedirectionMode::TouchpadAndWheel);
    // Nested scrollers hand off at their bounds.
    source.set_chaining(ChainingMode::Auto, ChainingMode::Auto, ChainingMode::Auto);
    // Wheel drives Y only; X and scale are left to touch and the touchpad. A wheel message
    // over a scroll container then needs no front-thread handling.
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
/// Shared rather than owned because the callback is a COM object the compositor holds past
/// the borrow that created it. Nothing here is `Send`: the callbacks arrive on the thread
/// that created the compositor, which is the thread the tree lives on.
///
/// The queue holds a [`Tick`](windows_window::Tick) while anything is in it. Inertia reports
/// from the compositor with no input behind it, so without the tick the queue would be read
/// at whatever unrelated wake came next — for a virtualized list, a fling landing on rows the
/// list never realized. [`drain`](Events::drain) releases the tick, so a tracker that has
/// stopped reporting parks the clock.
#[derive(Default)]
pub(crate) struct Events {
    queued: Vec<crate::SceneEvent>,
    tick: Option<windows_window::Tick>,
}

impl Events {
    /// Queues an event, taking a tick if the queue was empty.
    pub(crate) fn push(&mut self, event: crate::SceneEvent, wake: &windows_window::Wake) {
        self.queued.push(event);
        if self.tick.is_none() {
            self.tick = Some(wake.tick());
        }
    }

    /// Moves the queued events onto `out` and releases the tick.
    pub(crate) fn drain(&mut self, out: &mut Vec<crate::SceneEvent>) {
        out.append(&mut self.queued);
        self.tick = None;
    }
}

pub(crate) type EventQueue = Rc<RefCell<Events>>;

/// Translates a wrapper tracker event into this crate's, tagged with the tracker it came
/// from.
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
