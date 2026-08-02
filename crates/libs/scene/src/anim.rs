//! Animation templates, the three expressions, scoped batches and ghosts. **Front half.**
//!
//! Two things here are shared rather than per sink, and both are the difference between an
//! interaction path that allocates and one that does not.
//!
//! **Springs are templates, held once for the process.** Continuity across a retarget is a
//! property of the *target* — a natural-motion animation starts from the property's current
//! value and resets velocity to zero, whichever object drives it — so one instance per
//! (value kind × tuning) serves every sink in the application. Nine objects, against four
//! per sprite.
//!
//! **Key frames are not templates.** A key-frame animation carries its frames and there is
//! no way to clear them, so one is minted per start and released with its scoped batch.
//! That is event rate, not per frame.

use crate::node::Node;
use crate::sink::{Easing, Iterations, Tuning, Value};
use core::cell::Cell as CoreCell;
use core::time::Duration;
use std::rc::Rc;
use windows_composition::{
    Animatable, Animation, BatchKind, CompositionAnimation, CompositionEasingFunction,
    CompositionScopedBatch, Compositor, ExpressionAnimation, SpringScalarNaturalMotionAnimation,
    SpringVector2NaturalMotionAnimation, SpringVector3NaturalMotionAnimation, Visual,
};
use windows_core::{EventRevoker, Result};
use windows_numerics::{Vector2, Vector3};

/// **Scroll carrier.** Derived from the retired CPU spring's stiffness and damping —
/// natural period `2π/√k`, damping ratio `c/(2√k)`. Used where carrying momentum is the
/// point.
pub const SCROLL_PERIOD: f32 = 0.2756;
/// The scroll carrier's damping ratio.
pub const SCROLL_DAMPING: f32 = 0.877;

/// **Control chrome.** Every indicator, ink, pill glide and trim, and nothing else.
///
/// **Tuned by eye, and it has to be:** the textbook settling time for a spring of this
/// period predicts a motion several times shorter than what the compositor actually plays,
/// so `period` here does *not* mean the undamped natural period a second-order model would
/// assume. Do not re-derive it from the scroll carrier's stiffness and damping — that
/// derivation is what made a selection pill travel for what read as a fifth of a second of
/// lag. What *is* dependable is that duration scales linearly with period at fixed damping.
pub const CHROME_PERIOD: f32 = 0.0900;
/// The chrome spring's damping ratio.
pub const CHROME_DAMPING: f32 = 0.900;

/// The travel a chrome spring's period is quoted against, in DIPs.
const CHROME_REF_TRAVEL: f32 = 120.0;

/// The shared animation objects, and the three expression strings.
/// Everything in flight: the shared animation templates, and the subtrees still playing an
/// exit after the model let go of them.
pub(crate) struct Motion {
    pub(crate) templates: Templates,
    pub(crate) ghosts: Vec<Ghost>,
}

impl Motion {
    pub(crate) fn new(compositor: &Compositor) -> Self {
        Self {
            templates: Templates::new(compositor),
            ghosts: Vec::new(),
        }
    }
}

pub(crate) struct Templates {
    scalar: [SpringScalarNaturalMotionAnimation; 2],
    vec2: [SpringVector2NaturalMotionAnimation; 2],
    vec3: [SpringVector3NaturalMotionAnimation; 2],
    /// The whole expression DSL: a tracker's X, its Y, and its scale, each mapped through
    /// `value * m + c`. `m` and `c` are scalar parameters, so the strings themselves are
    /// constants — which is what makes "every expression in the system is authored inside
    /// this crate and covered by tests" a checkable claim rather than an aspiration.
    track: [ExpressionAnimation; 3],
    linear: CompositionEasingFunction,
}

/// The three expression strings, and there are no others.
///
/// The negation on position is not cosmetic: a tracker's position increases for up and left
/// motion, so content bound to it without the sign scrolls backwards.
const TRACK_X: &str = "t.Position.X * m + c";
const TRACK_Y: &str = "t.Position.Y * m + c";
const TRACK_SCALE: &str = "t.Scale * m + c";

impl Templates {
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

    /// Retargets the scalar spring for `tuning` and returns it, ready to start.
    ///
    /// `travel` scales the chrome period so a long slide is not instantaneous and a short
    /// one is not sluggish. It is computed here, where the property's current value is
    /// known — which is why a caller states a tuning and never a period.
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

    /// The tracker expression for an axis, bound to `tracker` and mapped by `m` and `c`.
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

    /// Builds a key-frame animation over `frames`.
    ///
    /// Minted per start rather than cached, because a key-frame animation carries its
    /// frames and the platform offers no way to clear them.
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

    /// The easing object for one segment.
    ///
    /// Linear is one shared instance because every linear segment is the same curve; a
    /// cubic is not, since it carries its own control points. Returned owned rather than
    /// borrowed for that reason — the key frame it is inserted into takes its own
    /// reference, so the one built here lives exactly as long as the call.
    fn easing(&self, compositor: &Compositor, easing: Easing) -> CompositionEasingFunction {
        match easing {
            Easing::Linear => self.linear.clone(),
            Easing::Cubic { c1, c2 } => compositor.create_cubic_bezier_easing_function(c1, c2),
        }
    }
}

/// Applies an iteration behaviour, saturating a count that cannot be expressed rather than
/// wrapping it into a shorter animation than asked for.
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

/// The chrome period scaled by how far the value has to travel, clamped so neither end is
/// silly. The scroll carrier is not scaled: its whole job is to carry momentum, and
/// momentum does not depend on how far the content happens to be from a bound.
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

/// A dying subtree, kept alive only long enough to play its exit.
///
/// The subtree is **flattened into one capture** and that capture is mounted as a top-level
/// sprite, so the original visuals unparent immediately and a dying panel of sixty visuals
/// fades as one. The capture's own brush chain is what keeps the detached source alive.
pub(crate) struct Ghost {
    /// The flattened capture, on screen for as long as the exit plays. Held because
    /// nothing else does: the ghost is unparented from the model's tree by construction.
    #[expect(dead_code, reason = "the only owner of the detached capture")]
    pub(crate) visual: Visual,
    /// Set by the batch's own completion signal — **never by a timer or an estimated
    /// deadline**. That is the difference between a ghost released when its animation ended
    /// and one released when we guessed it had.
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
    /// Whether the compositor has reported this exit complete.
    pub(crate) fn finished(&self) -> bool {
        self.done.get()
    }
}

impl crate::Scene {
    /// Detaches a dying subtree and keeps it on screen for the length of its exit.
    ///
    /// The subtree is **flattened into one capture** and that capture is mounted as a single
    /// top-level sprite, so the original visuals unparent immediately and a dying panel of
    /// sixty visuals fades as one. The capture's own brush chain is what keeps the detached
    /// source alive while it plays.
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
            // Nothing to capture, so nothing to fade: a zero-size ghost is a
            // visual and a batch held open for an animation with no pixels in it.
            return Ok(None);
        }

        let captured = back.compositor.capture(&source, size, env.scale());
        let sprite = back.compositor.create_sprite_visual();
        sprite.set_brush(&captured);
        sprite.set_size(size.x, size.y);
        let offset = source.offset();
        sprite.set_offset(offset.x, offset.y, offset.z);
        let visual = crate::base_of_sprite(&sprite);
        if let Some(children) = self
            .nodes
            .get(self.root().node())
            .and_then(|n| n.visual.as_container())
            .map(|c| c.children())
        {
            children.insert_at_top(&visual);
        }

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
        // Not a tautology: the failure this guards is someone deriving one from the other's
        // stiffness and damping, which produces a chrome period several times too long.
        // A compile-time proof, because both sides are constants: the failure it guards is
        // someone deriving one tuning from the other, which produces a chrome period
        // several times too long and reads as lag rather than as a wrong constant.
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
