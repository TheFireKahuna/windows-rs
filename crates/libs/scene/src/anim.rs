//! Animation templates, the tracker expressions, timed delays and exit ghosts.
//!
//! Springs are templates, one per (value kind × tuning) for the whole process: a
//! natural-motion animation starts from the target property's current value and resets its
//! velocity, whichever object drives it, so retargeting a shared instance plays the same
//! motion as a freshly built one. Nine spring objects serve every sink.
//!
//! A key-frame animation carries its frames and the platform offers no way to clear them, so
//! one is built per start and released when its scoped batch reports. That is once per
//! event, not once per frame.

use crate::node::Node;
use crate::sink::{Easing, Iterations, Tuning, Value};
use core::cell::Cell as CoreCell;
use core::time::Duration;
use std::rc::Rc;
use std::time::Instant;
use windows_composition::{
    Animatable, Animation, BatchKind, CompositionAnimation, CompositionEasingFunction,
    CompositionScopedBatch, Compositor, ExpressionAnimation, SpringScalarNaturalMotionAnimation,
    SpringVector2NaturalMotionAnimation, SpringVector3NaturalMotionAnimation, Visual,
};
use windows_core::{EventRevoker, Result};
use windows_numerics::{Vector2, Vector3};

/// The scroll carrier's spring period, in seconds: the tuning for [`Tuning::Scroll`], which
/// drives motion that carries momentum. Not scaled by how far the value travels.
pub const SCROLL_PERIOD: f32 = 0.2756;
/// The scroll carrier's damping ratio.
pub const SCROLL_DAMPING: f32 = 0.877;

/// The chrome spring's period, in seconds: the tuning for [`Tuning::Chrome`], which drives
/// every indicator, ink, pill glide and trim.
///
/// The compositor plays a spring of this period for several times the settling time a
/// second-order model predicts, so `period` is not that model's undamped natural period and
/// this value cannot be computed from a stiffness and a damping ratio. It is fixed by eye.
/// At fixed damping the motion's duration does scale linearly with the period, which is what
/// [`scaled_period`] relies on.
pub const CHROME_PERIOD: f32 = 0.0900;
/// The chrome spring's damping ratio.
pub const CHROME_DAMPING: f32 = 0.900;

/// The travel [`CHROME_PERIOD`] is quoted against, in DIPs. [`scaled_period`] scales the
/// period by a travel relative to this.
const CHROME_REF_TRAVEL: f32 = 120.0;

/// Holds everything in flight: the shared animation templates, the subtrees still playing an
/// exit after the model let go of them, and the timed reveals waiting to report.
pub(crate) struct Motion {
    pub(crate) templates: Templates,
    pub(crate) ghosts: Vec<Ghost>,
    /// Pending delays, looked up by a linear scan on id. A handful at most — a hovered
    /// submenu, a tooltip — and empty in the steady state.
    pub(crate) delays: Vec<Delay>,
}

impl Motion {
    pub(crate) fn new(compositor: &Compositor) -> Self {
        Self {
            templates: Templates::new(compositor),
            ghosts: Vec::new(),
            delays: Vec::new(),
        }
    }
}

/// Holds the animation objects every sink shares: one spring per (value kind × tuning), one
/// expression per tracker axis, and the linear easing.
pub(crate) struct Templates {
    scalar: [SpringScalarNaturalMotionAnimation; 2],
    vec2: [SpringVector2NaturalMotionAnimation; 2],
    vec3: [SpringVector3NaturalMotionAnimation; 2],
    /// One expression per tracker axis — X, Y and scale — each mapping the tracker through
    /// `value * m + c`. `m` and `c` are scalar parameters, so the strings are constants and
    /// one instance per axis serves every binding.
    track: [ExpressionAnimation; 3],
    linear: CompositionEasingFunction,
}

/// The tracker expression for the X axis. `t` is the tracker, and `m` and `c` are the
/// affine parameters a binding supplies; the other two axes take the same shape.
const TRACK_X: &str = "t.Position.X * m + c";
const TRACK_Y: &str = "t.Position.Y * m + c";
const TRACK_SCALE: &str = "t.Scale * m + c";

impl Templates {
    /// Builds one spring per (value kind × tuning), one expression per tracker axis, and
    /// the shared linear easing.
    pub(crate) fn new(compositor: &Compositor) -> Self {
        let spring_scalar = |tuning: Tuning| {
            let spring = compositor.create_spring_scalar_animation();
            let (period, damping) = tuning_of(tuning);
            spring.set_period(period);
            spring.set_damping_ratio(damping);
            spring
        };
        let spring_vec2 = |tuning: Tuning| {
            let spring = compositor.create_spring_vector2_animation();
            let (period, damping) = tuning_of(tuning);
            spring.set_period(period);
            spring.set_damping_ratio(damping);
            spring
        };
        let spring_vec3 = |tuning: Tuning| {
            let spring = compositor.create_spring_vector3_animation();
            let (period, damping) = tuning_of(tuning);
            spring.set_period(period);
            spring.set_damping_ratio(damping);
            spring
        };
        Self {
            scalar: [spring_scalar(Tuning::Chrome), spring_scalar(Tuning::Scroll)],
            vec2: [spring_vec2(Tuning::Chrome), spring_vec2(Tuning::Scroll)],
            vec3: [spring_vec3(Tuning::Chrome), spring_vec3(Tuning::Scroll)],
            track: [
                compositor.create_expression_animation(TRACK_X),
                compositor.create_expression_animation(TRACK_Y),
                compositor.create_expression_animation(TRACK_SCALE),
            ],
            linear: compositor.create_linear_easing_function(),
        }
    }

    /// Retargets the shared scalar spring for `tuning` to `to` and returns it, ready to
    /// start.
    ///
    /// `travel` is how far the value has to move, in DIPs; it scales the chrome period so a
    /// long slide is not instantaneous and a short one is not sluggish, which is why a
    /// caller states a tuning and never a period. The spring is shared, so its settings hold
    /// only until the next retarget.
    pub(crate) fn spring_scalar(
        &self,
        tuning: Tuning,
        to: f32,
        travel: f32,
    ) -> &SpringScalarNaturalMotionAnimation {
        let spring = &self.scalar[index_of(tuning)];
        spring.set_period(scaled_period(tuning, travel));
        spring.set_final_value(to);
        spring
    }

    /// Retargets the shared `Vector2` spring for `tuning` to `to` and returns it, ready to
    /// start. `travel` scales the period as it does for [`Templates::spring_scalar`].
    pub(crate) fn spring_vec2(
        &self,
        tuning: Tuning,
        to: Vector2,
        travel: f32,
    ) -> &SpringVector2NaturalMotionAnimation {
        let spring = &self.vec2[index_of(tuning)];
        spring.set_period(scaled_period(tuning, travel));
        spring.set_final_value(to);
        spring
    }

    /// Retargets the shared `Vector3` spring for `tuning` to `to` and returns it, ready to
    /// start. `travel` scales the period as it does for [`Templates::spring_scalar`].
    pub(crate) fn spring_vec3(
        &self,
        tuning: Tuning,
        to: Vector3,
        travel: f32,
    ) -> &SpringVector3NaturalMotionAnimation {
        let spring = &self.vec3[index_of(tuning)];
        spring.set_period(scaled_period(tuning, travel));
        spring.set_final_value(to);
        spring
    }

    /// Binds the shared expression for `axis` to `tracker` as `value * m + c` and returns
    /// it, ready to start. The expression is shared, so its parameters hold only until the
    /// next binding on the same axis.
    pub(crate) fn track(
        &self,
        axis: crate::sink::TrackerAxis,
        tracker: &impl Animatable,
        m: f32,
        c: f32,
    ) -> &ExpressionAnimation {
        use crate::sink::TrackerAxis::{PositionX, PositionY, Scale};
        let expression = match axis {
            PositionX => &self.track[0],
            PositionY => &self.track[1],
            Scale => &self.track[2],
        };
        expression.set_reference_parameter("t", tracker);
        expression.set_scalar_parameter("m", m);
        expression.set_scalar_parameter("c", c);
        expression
    }

    /// Builds a key-frame animation over `frames`, each entry a progress in `0..=1`, the
    /// value at it, and the easing into it.
    ///
    /// Frames that are all [`Value::Scalar`] build a scalar animation; any other mix builds
    /// a `Vector3` one, splatting a scalar across all three components and giving a
    /// [`Value::Vec2`] a zero third. `duration_ms` is the length of one run and `iterations`
    /// how many times it runs.
    ///
    /// Built per call rather than shared: a key-frame animation carries its frames and the
    /// platform offers no way to clear them.
    pub(crate) fn frames(
        &self,
        compositor: &Compositor,
        frames: &[(f32, Value, Easing)],
        duration_ms: u32,
        iterations: Iterations,
    ) -> Option<CompositionAnimation> {
        let scalar = frames.iter().all(|(_, v, _)| matches!(v, Value::Scalar(_)));
        if scalar {
            let animation = compositor.create_scalar_key_frame_animation();
            for &(at, value, easing) in frames {
                let Value::Scalar(v) = value else { continue };
                animation.insert_key_frame_with_easing(at, v, &self.easing(compositor, easing));
            }
            animation.set_duration(Duration::from_millis(u64::from(duration_ms)));
            apply_iterations(
                iterations,
                |n| animation.set_iteration_count(n),
                || {
                    animation.set_iterate_forever();
                },
            );
            return Some(animation.as_animation());
        }
        let animation = compositor.create_vector3_key_frame_animation();
        for &(at, value, easing) in frames {
            let v = match value {
                Value::Scalar(v) => Vector3 { x: v, y: v, z: v },
                Value::Vec2(v) => Vector3 {
                    x: v.x,
                    y: v.y,
                    z: 0.0,
                },
            };
            animation.insert_key_frame_with_easing(at, v, &self.easing(compositor, easing));
        }
        animation.set_duration(Duration::from_millis(u64::from(duration_ms)));
        apply_iterations(
            iterations,
            |n| animation.set_iteration_count(n),
            || {
                animation.set_iterate_forever();
            },
        );
        Some(animation.as_animation())
    }

    /// Returns the easing object for one key-frame segment.
    ///
    /// Linear is one shared instance, since every linear segment is the same curve; a cubic
    /// carries its own control points and is built per call. Returned owned for that reason
    /// — the key frame it is inserted into takes its own reference, so the one built here
    /// lives exactly as long as the call.
    fn easing(&self, compositor: &Compositor, easing: Easing) -> CompositionEasingFunction {
        match easing {
            Easing::Linear => self.linear.clone(),
            Easing::Cubic { c1, c2 } => compositor.create_cubic_bezier_easing_function(c1, c2),
        }
    }
}

/// Applies an iteration behaviour to an animation. A count the platform's `i32` cannot hold
/// saturates rather than wrapping into a shorter animation than was asked for.
fn apply_iterations(iterations: Iterations, count: impl FnOnce(i32), forever: impl FnOnce()) {
    match iterations {
        Iterations::Count(n) => count(i32::try_from(n).unwrap_or(i32::MAX)),
        Iterations::Forever => forever(),
    }
}

fn index_of(tuning: Tuning) -> usize {
    match tuning {
        Tuning::Chrome => 0,
        Tuning::Scroll => 1,
    }
}

fn tuning_of(tuning: Tuning) -> (Duration, f32) {
    match tuning {
        Tuning::Chrome => (secs(CHROME_PERIOD), CHROME_DAMPING),
        Tuning::Scroll => (secs(SCROLL_PERIOD), SCROLL_DAMPING),
    }
}

/// Returns the spring period for `tuning`.
///
/// The chrome period is scaled by `travel` relative to [`CHROME_REF_TRAVEL`], with the
/// factor clamped to `0.7..=1.4`; a non-finite `travel` leaves it unscaled. The scroll
/// carrier is not scaled at all: it carries momentum, which does not depend on how far the
/// content is from a bound.
fn scaled_period(tuning: Tuning, travel: f32) -> Duration {
    let (period, _) = tuning_of(tuning);
    match tuning {
        Tuning::Scroll => period,
        Tuning::Chrome => {
            let factor = if travel.is_finite() && CHROME_REF_TRAVEL > 0.0 {
                (travel.abs() / CHROME_REF_TRAVEL).clamp(0.7, 1.4)
            } else {
                1.0
            };
            secs(CHROME_PERIOD * factor)
        }
    }
}

fn secs(seconds: f32) -> Duration {
    Duration::from_secs_f64(f64::from(seconds.max(0.0)))
}

/// A timed reveal in flight: a deadline compared against the frame clock.
///
/// A delay costs an instant and a clock request — no property set, no animation, no scoped
/// batch, no subscription. Nothing fires it: the deadline is read on a frame the scene is
/// already servicing, so a delay is observed at a frame boundary and adds no clock of its
/// own.
///
/// Unlike a [`Ghost`], a delay has no animation whose completion matters, so it carries no
/// scoped batch — only elapsed time decides it.
pub(crate) struct Delay {
    pub(crate) id: crate::sink::DelayId,
    /// Monotonic, so nothing a user or a time service does to the wall clock moves it.
    due: Instant,
    /// Keeps the frame clock awake until the delay is dropped, so the deadline is reached
    /// on a frame rather than waited for.
    _tick: windows_window::Tick,
}

impl Delay {
    /// Returns whether `now` has reached the deadline.
    pub(crate) fn elapsed(&self, now: Instant) -> bool {
        now >= self.due
    }
}

impl crate::Scene {
    /// Starts a timed reveal under `id`, due `ms` from now.
    ///
    /// Any delay already registered under `id` is dropped first, so a tooltip swapping
    /// between targets neither reports the old deadline nor waits a second time.
    pub(crate) fn start_delay(&mut self, id: crate::sink::DelayId, ms: u32) {
        self.cancel_delay(id);
        self.motion.delays.push(Delay {
            id,
            due: Instant::now() + Duration::from_millis(u64::from(ms)),
            _tick: self.wake.tick(),
        });
    }

    /// Cancels the delay registered under `id`. It holds only a deadline and a clock
    /// request, so dropping it is the whole unwind and a cancelled delay never reports.
    pub(crate) fn cancel_delay(&mut self, id: crate::sink::DelayId) {
        self.motion.delays.retain(|delay| delay.id != id);
    }
}

/// A dying subtree, held on screen only long enough to play its exit.
///
/// The subtree is flattened into one capture and that capture is mounted as a top-level
/// sprite, so the original visuals unparent at once and a dying panel of sixty visuals fades
/// as one. The capture's brush chain keeps the detached source alive while it plays.
pub(crate) struct Ghost {
    /// The flattened capture, on screen for as long as the exit plays. Held because nothing
    /// else does: the ghost is unparented from the model's tree by construction.
    #[expect(dead_code, reason = "the only owner of the detached capture")]
    pub(crate) visual: Visual,
    /// Set by the scoped batch's completion signal, which is the only report that the exit
    /// has actually played.
    done: Rc<CoreCell<bool>>,
    /// Held so the completion subscription outlives the animation. Dropping it early means
    /// the completion never arrives and the ghost leaks for the compositor's lifetime.
    _revoker: EventRevoker,
    /// Held so the batch is not collected before it reports.
    _batch: CompositionScopedBatch,
    /// Keeps the frame clock awake while the exit is in flight.
    _tick: windows_window::Tick,
}

impl Ghost {
    /// Returns whether the compositor has reported this exit complete.
    ///
    /// Polled by the sweep that drops finished ghosts. Nothing outside the scene observes a
    /// release: the ghost is unparented from the model's tree by construction and this type
    /// is its only owner.
    pub(crate) fn finished(&self) -> bool {
        self.done.get()
    }
}

impl crate::Scene {
    /// Detaches the subtree at `id` and keeps it on screen for the length of `exit`.
    ///
    /// The subtree is flattened into one capture, mounted as a single top-level sprite, so
    /// the original visuals unparent at once. Returns `Ok(None)` when `exit` is
    /// [`Exit::None`](crate::sink::Exit::None), when `id` names no node, or when the node's
    /// box is empty — there is then nothing to capture and nothing to fade.
    ///
    /// # Errors
    ///
    /// Fails if the completion subscription cannot be made or the batch cannot be sealed.
    pub(crate) fn ghost(
        &mut self,
        id: crate::sink::NodeId,
        exit: crate::sink::Exit,
        back: &crate::Backends,
        env: crate::Env,
    ) -> Result<Option<Ghost>> {
        use crate::sink::Exit;
        if exit == Exit::None {
            return Ok(None);
        }
        let Some(source) = self.nodes.get(id).map(|n| n.visual.clone()) else {
            return Ok(None);
        };
        let size = self
            .nodes
            .get(id)
            .map_or(Vector2 { x: 0.0, y: 0.0 }, Node::size);
        if size.x <= 0.0 || size.y <= 0.0 {
            // Nothing to capture, so nothing to fade: a zero-size ghost would be a visual
            // and a batch held open for an animation with no pixels in it.
            return Ok(None);
        }

        // A ghost is a snapshot of a subtree already being destroyed, so its region never
        // moves: the brush holds the surface and nothing will ask it to resize.
        let captured = back.compositor.capture(&source, size, env.scale());
        let sprite = back.compositor.create_sprite_visual();
        sprite.set_brush(&captured.brush);
        sprite.set_size(size.x, size.y);
        let offset = source.offset();
        sprite.set_offset(offset.x, offset.y, offset.z);
        // A ghost outlives the node it was captured from, so it hangs in the window's own
        // container rather than in the subtree being torn down, and at the top because it
        // plays over whatever replaced it.
        let visual = crate::base_of_sprite(&sprite);
        self.overlay_children().insert_at_top(&visual);

        let done = Rc::new(CoreCell::new(false));
        let signal = Rc::clone(&done);
        let batch = back.compositor.create_scoped_batch(BatchKind::Animation);
        match exit {
            Exit::None => unreachable!("returned above"),
            Exit::Fade { ms } => {
                let animation = back.compositor.create_scalar_key_frame_animation();
                animation.insert_key_frame(0.0, sprite.opacity());
                animation.insert_key_frame(1.0, 0.0);
                animation.set_duration(Duration::from_millis(u64::from(ms)));
                sprite.start_animation("Opacity", &animation);
            }
            Exit::Scale { to, ms } => {
                let animation = back.compositor.create_vector3_key_frame_animation();
                animation.insert_key_frame(0.0, sprite.scale());
                animation.insert_key_frame(
                    1.0,
                    Vector3 {
                        x: to,
                        y: to,
                        z: 1.0,
                    },
                );
                animation.set_duration(Duration::from_millis(u64::from(ms)));
                sprite.set_center_point(Vector3 {
                    x: size.x * 0.5,
                    y: size.y * 0.5,
                    z: 0.0,
                });
                sprite.start_animation("Scale", &animation);
            }
        }
        // A batch subscribed to but never sealed keeps swallowing later animations, and one
        // sealed with no subscriber never reports. The pair is armed together or not at all.
        let revoker = batch.on_completed(move || signal.set(true))?;
        batch.try_end()?;
        self.census.animations += 1;

        Ok(Some(Ghost {
            visual,
            done,
            _revoker: revoker,
            _batch: batch,
            _tick: self.wake.tick(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_tunings_are_independent_values() {
        // Both sides are constants, so the ordering is proved at compile time: a chrome
        // period derived from the scroll tuning comes out several times too long.
        const _: () = assert!(CHROME_PERIOD < SCROLL_PERIOD);
        assert_ne!(CHROME_DAMPING, SCROLL_DAMPING);
    }

    #[test]
    fn chrome_period_scales_with_travel_and_is_bounded_at_both_ends() {
        let short = scaled_period(Tuning::Chrome, 1.0);
        let reference = scaled_period(Tuning::Chrome, CHROME_REF_TRAVEL);
        let long = scaled_period(Tuning::Chrome, 10_000.0);
        assert!(short < reference && reference < long);
        assert!(short >= secs(CHROME_PERIOD * 0.7));
        assert!(long <= secs(CHROME_PERIOD * 1.4));
    }

    #[test]
    fn the_scroll_carrier_does_not_scale_with_distance() {
        assert_eq!(
            scaled_period(Tuning::Scroll, 1.0),
            scaled_period(Tuning::Scroll, 10_000.0)
        );
    }

    #[test]
    fn there_are_exactly_three_expression_strings_and_each_maps_affinely() {
        for expression in [TRACK_X, TRACK_Y, TRACK_SCALE] {
            assert!(expression.contains("* m + c"), "{expression}");
            assert!(expression.starts_with("t."), "{expression}");
        }
    }

    #[test]
    fn a_non_finite_travel_does_not_produce_a_non_finite_period() {
        for travel in [f32::NAN, f32::INFINITY, -0.0] {
            let period = scaled_period(Tuning::Chrome, travel);
            assert!(period >= secs(CHROME_PERIOD * 0.7));
            assert!(period <= secs(CHROME_PERIOD * 1.4));
        }
    }
}
