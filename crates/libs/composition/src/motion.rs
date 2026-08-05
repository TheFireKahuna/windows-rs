//! Compositor-evaluated motion: springs and expressions (feature `system`).
//!
//! A key-frame animation plays a curve the app authored; the two forms here do not. A
//! **spring** is retargeted mid-flight by reassigning its final value, so a gesture
//! redirects without the discontinuity a restarted key-frame animation shows. An
//! **expression** is re-evaluated every vblank against the objects it references, so its
//! value is a function of live state that the app thread never computes.

use super::*;
use std::time::Duration;
use windows_time::TimeSpan;

// Durations too large for a WinRT `TimeSpan` saturate rather than wrap.
fn to_time_span(duration: Duration) -> TimeSpan {
    TimeSpan::try_from(duration).unwrap_or(TimeSpan::MAX)
}

/// An expression the compositor evaluates every frame.
///
/// The expression text names parameters bound with
/// [`set_reference_parameter`](Self::set_reference_parameter) and the scalar/vector
/// setters — `"tracker.Position.Y * -1.0"`, `"set.Progress"` — and, unlike a key-frame
/// animation, **it is validated only when the animation is started**. A typo is a
/// failure at `start_animation`, not at construction.
#[derive(Clone)]
pub struct ExpressionAnimation(pub(crate) bindings::ExpressionAnimation);

impl ExpressionAnimation {
    /// Replaces the expression text.
    pub fn set_expression(&self, expression: &str) {
        self.0.SetExpression(expression).unwrap();
    }

    /// Binds `target` into the expression under `name`, so the expression can read its
    /// properties — `"tracker.Position"` for a tracker bound as `"tracker"`.
    pub fn set_reference_parameter(&self, name: &str, target: &impl Animatable) {
        let animation: bindings::ICompositionAnimation = self.0.cast().unwrap();
        animation
            .SetReferenceParameter(name, &target.as_object().0)
            .unwrap();
    }

    /// Binds a constant scalar under `name`.
    ///
    /// **Value parameters are copied into the expression when the animation is started**,
    /// so changing one afterwards does nothing to a running animation — that is what a
    /// [`CompositionPropertySet`](crate::CompositionPropertySet) bound with
    /// [`set_reference_parameter`](Self::set_reference_parameter) is for: its keys are read
    /// by the compositor every frame, and writing one updates every expression referencing
    /// it.
    pub fn set_scalar_parameter(&self, name: &str, value: f32) {
        let animation: bindings::ICompositionAnimation = self.0.cast().unwrap();
        animation.SetScalarParameter(name, value).unwrap();
    }

    /// Binds a constant [`Vector2`] under `name`.
    pub fn set_vector2_parameter(&self, name: &str, value: Vector2) {
        let animation: bindings::ICompositionAnimation = self.0.cast().unwrap();
        animation.SetVector2Parameter(name, value).unwrap();
    }

    /// Binds a constant [`Vector3`] under `name`.
    pub fn set_vector3_parameter(&self, name: &str, value: Vector3) {
        let animation: bindings::ICompositionAnimation = self.0.cast().unwrap();
        animation.SetVector3Parameter(name, value).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit animation.
    pub fn set_target(&self, target: &str) {
        let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
        animation.SetTarget(target).unwrap();
    }
}

impl Sealed for ExpressionAnimation {}

impl Animation for ExpressionAnimation {
    fn as_animation(&self) -> CompositionAnimation {
        CompositionAnimation(self.0.cast().unwrap())
    }
}

/// A key-frame animation over a [`Vector2`] property, so a size or a two-axis offset
/// animates as one value rather than as two scalar animations that can drift apart.
#[derive(Clone)]
pub struct Vector2KeyFrameAnimation(pub(crate) bindings::Vector2KeyFrameAnimation);

impl Vector2KeyFrameAnimation {
    /// Inserts a key frame at `progress` (in `0.0..=1.0`) with the given value.
    pub fn insert_key_frame(&self, progress: f32, value: Vector2) {
        self.0.InsertKeyFrame(progress, value).unwrap();
    }

    /// Inserts a key frame at `progress` that eases to `value` along `easing`.
    pub fn insert_key_frame_with_easing(
        &self,
        progress: f32,
        value: Vector2,
        easing: &CompositionEasingFunction,
    ) {
        self.0
            .InsertKeyFrameWithEasingFunction(progress, value, &easing.0)
            .unwrap();
    }

    /// Sets how long one iteration takes.
    pub fn set_duration(&self, duration: Duration) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation.SetDuration(to_time_span(duration)).unwrap();
    }

    /// Sets how long to wait before the animation starts.
    pub fn set_delay(&self, delay: Duration) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation.SetDelayTime(to_time_span(delay)).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit animation.
    pub fn set_target(&self, target: &str) {
        let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
        animation.SetTarget(target).unwrap();
    }
}

impl Sealed for Vector2KeyFrameAnimation {}

impl Animation for Vector2KeyFrameAnimation {
    fn as_animation(&self) -> CompositionAnimation {
        CompositionAnimation(self.0.cast().unwrap())
    }
}

impl ScalarKeyFrameAnimation {
    /// Inserts a key frame at `progress` (in `0.0..=1.0`) with the given value, using
    /// the animation's default interpolation.
    pub fn insert_key_frame(&self, progress: f32, value: f32) {
        self.0.InsertKeyFrame(progress, value).unwrap();
    }
}

macro_rules! spring {
    ($name:ident, $binding:ident, $value:ty, $final_value:ident, $spring:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// Retarget a running spring with `set_final_value(v)` followed by
        /// `start_animation(prop, anim)` on the **same cached object**: that allocates
        /// nothing on the interaction path and carries whatever velocity the property
        /// already had. A value that must land immediately, with no motion, takes
        /// `stop_animation` plus a plain property set.
        ///
        /// As an implicit animation a spring is **not** handed the target value
        /// automatically: set the final value explicitly at start, or it animates toward
        /// zero.
        #[derive(Clone)]
        pub struct $name(pub(crate) bindings::$binding);

        impl $name {
            /// Sets the damping ratio: below `1.0` overshoots and rings, `1.0` is
            /// critically damped, above `1.0` crawls in without overshoot.
            pub fn set_damping_ratio(&self, ratio: f32) {
                let spring: bindings::$spring = self.0.cast().unwrap();
                spring.SetDampingRatio(ratio).unwrap();
            }

            /// Sets the spring's period.
            ///
            /// The period is **not** the undamped natural period of a second-order model:
            /// the motion the compositor plays for a given period runs several times
            /// longer than that model predicts. Duration does scale linearly with period
            /// at fixed damping, so a period measured by eye scales while one derived from
            /// a stiffness constant does not hold.
            pub fn set_period(&self, period: Duration) {
                let spring: bindings::$spring = self.0.cast().unwrap();
                spring.SetPeriod(to_time_span(period)).unwrap();
            }

            /// Sets the value the spring settles at. Assigning it while the spring is
            /// running is what retargets it.
            pub fn set_final_value(&self, value: $value) {
                // The property is an `IReference<T>` whose documented default is null,
                // meaning "use the ending value of the property being animated". A spring
                // left null as an implicit animation on this stack animates toward zero
                // instead, so the target is always supplied and the null state unreachable.
                let motion: bindings::$final_value = self.0.cast().unwrap();
                motion.SetFinalValue(Some(value)).unwrap();
            }

            /// Sets the velocity the spring starts with, per second.
            ///
            /// A released gesture continues at the speed the user was moving when its
            /// velocity measured at release is handed over here. A spring already running
            /// keeps the property's current velocity when it is retargeted, so this
            /// applies to the handoff from app-driven motion to the compositor's.
            pub fn set_initial_velocity(&self, velocity: $value) {
                let motion: bindings::$final_value = self.0.cast().unwrap();
                motion.SetInitialVelocity(velocity).unwrap();
            }

            /// Sets the property this animation targets when used as an implicit
            /// animation.
            pub fn set_target(&self, target: &str) {
                let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
                animation.SetTarget(target).unwrap();
            }
        }

        impl Sealed for $name {}

        impl Animation for $name {
            fn as_animation(&self) -> CompositionAnimation {
                CompositionAnimation(self.0.cast().unwrap())
            }
        }
    };
}

spring!(
    SpringScalarNaturalMotionAnimation,
    SpringScalarNaturalMotionAnimation,
    f32,
    IScalarNaturalMotionAnimation,
    ISpringScalarNaturalMotionAnimation,
    "A spring over a scalar property — opacity, a single axis, a trim."
);

spring!(
    SpringVector2NaturalMotionAnimation,
    SpringVector2NaturalMotionAnimation,
    Vector2,
    IVector2NaturalMotionAnimation,
    ISpringVector2NaturalMotionAnimation,
    "A spring over a [`Vector2`] property — a two-axis offset or a size."
);

spring!(
    SpringVector3NaturalMotionAnimation,
    SpringVector3NaturalMotionAnimation,
    Vector3,
    IVector3NaturalMotionAnimation,
    ISpringVector3NaturalMotionAnimation,
    "A spring over a [`Vector3`] property — a visual's `Offset` or `Scale`."
);

impl CompositionScopedBatch {
    /// Seals the batch, reporting failure instead of panicking.
    ///
    /// Arming a batch takes two fallible steps — subscribing with
    /// [`on_completed`](Self::on_completed) and sealing — and is correct only if both
    /// succeed: a batch subscribed to but never sealed keeps swallowing later animations,
    /// and one sealed with no subscriber never reports completion. This form lets both be
    /// written in one `?`-chain. [`end`](CompositionScopedBatch::end) covers the case with
    /// no subscriber and so nothing to unwind.
    pub fn try_end(&self) -> Result<()> {
        self.0.End()
    }

    /// Registers `handler` to run once, on the compositor's thread, when every piece of
    /// work this batch tracks has finished.
    ///
    /// This is the **only** signal that a batch's animations are done. Anything held alive
    /// for their duration — a visual retained so an exit transition can play out — is
    /// released from here.
    ///
    /// The returned [`EventRevoker`](windows_core::EventRevoker) unsubscribes when it is
    /// dropped, so it must be kept alive until the handler has run: dropping it early means
    /// the completion never arrives and whatever the handler would have released leaks for
    /// the lifetime of the compositor.
    pub fn on_completed(
        &self,
        handler: impl FnMut() + 'static,
    ) -> Result<windows_core::EventRevoker> {
        // The raw handler is `Fn` and receives a sender and args, neither of which carries
        // anything a caller can act on. A `RefCell` adapts the caller's `FnMut` to that
        // shape without either raw type reaching the public API.
        let handler = core::cell::RefCell::new(handler);
        self.0.Completed(move |_sender, _args| {
            // Raised once per batch, so this borrow is uncontended; yielding rather than
            // panicking keeps an unexpected re-entrant raise from unwinding across the COM
            // boundary.
            if let Ok(mut handler) = handler.try_borrow_mut() {
                handler();
            }
        })
    }
}

impl Compositor {
    /// Creates an expression animation from `expression`, whose text is validated when
    /// the animation is started rather than here.
    pub fn create_expression_animation(&self, expression: &str) -> ExpressionAnimation {
        ExpressionAnimation(
            self.0
                .CreateExpressionAnimationWithExpression(expression)
                .unwrap(),
        )
    }

    /// Creates a `Vector2` key-frame animation.
    pub fn create_vector2_key_frame_animation(&self) -> Vector2KeyFrameAnimation {
        Vector2KeyFrameAnimation(self.0.CreateVector2KeyFrameAnimation().unwrap())
    }

    /// Creates a spring over a scalar property.
    pub fn create_spring_scalar_animation(&self) -> SpringScalarNaturalMotionAnimation {
        let compositor: bindings::ICompositor4 = self.0.cast().unwrap();
        SpringScalarNaturalMotionAnimation(compositor.CreateSpringScalarAnimation().unwrap())
    }

    /// Creates a spring over a [`Vector2`] property.
    pub fn create_spring_vector2_animation(&self) -> SpringVector2NaturalMotionAnimation {
        let compositor: bindings::ICompositor4 = self.0.cast().unwrap();
        SpringVector2NaturalMotionAnimation(compositor.CreateSpringVector2Animation().unwrap())
    }

    /// Creates a spring over a [`Vector3`] property.
    pub fn create_spring_vector3_animation(&self) -> SpringVector3NaturalMotionAnimation {
        let compositor: bindings::ICompositor4 = self.0.cast().unwrap();
        SpringVector3NaturalMotionAnimation(compositor.CreateSpringVector3Animation().unwrap())
    }

    /// Creates a step easing function that advances in `steps` equal jumps instead of
    /// interpolating continuously.
    ///
    /// Applied per key-frame segment, so an animation with `k` segments visits
    /// `k * steps` distinct values. It is the only control a key-frame animation has over
    /// how *often* it writes, which matters where each write invalidates something
    /// expensive — a visual's whole bounds, old and new.
    ///
    /// **It takes the segment's END value immediately.** A key-frame pair intended to
    /// hold a value and then jump instead jumps at the start. To hold a level, insert an
    /// explicit key frame at that value with linear easing.
    ///
    /// `steps` must be positive.
    pub fn create_step_easing_function(&self, steps: i32) -> CompositionEasingFunction {
        debug_assert!(steps > 0, "a step easing function needs at least one step");
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        CompositionEasingFunction(
            compositor
                .CreateStepEasingFunctionWithStepCount(steps.max(1))
                .unwrap()
                .cast()
                .unwrap(),
        )
    }
}
