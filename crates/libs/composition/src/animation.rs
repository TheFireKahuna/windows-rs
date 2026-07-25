use super::*;
use std::time::Duration;
use windows_time::TimeSpan;

fn to_time_span(duration: Duration) -> TimeSpan {
    // Durations too large for a WinRT `TimeSpan` saturate rather than wrap.
    TimeSpan::try_from(duration).unwrap_or(TimeSpan::MAX)
}

/// The base type shared by every composition animation. An [`Animation`] can be
/// turned into one via [`Animation::as_animation`] to start it on a visual with
/// [`Visual::start_animation`](crate::Visual::start_animation).
#[derive(Clone)]
pub struct CompositionAnimation(pub(crate) bindings::CompositionAnimation);

/// An animation that can be started on a visual property.
///
/// This trait is sealed: only the animation types in this crate implement it.
pub trait Animation: Sealed {
    /// Returns this animation as the shared [`CompositionAnimation`] base type.
    fn as_animation(&self) -> CompositionAnimation;
}

/// An easing function that shapes a key frame's interpolation curve.
///
/// Create one with
/// [`Compositor::create_linear_easing_function`](crate::Compositor::create_linear_easing_function)
/// or
/// [`Compositor::create_cubic_bezier_easing_function`](crate::Compositor::create_cubic_bezier_easing_function)
/// and pass it to a key frame with `insert_key_frame_with_easing`.
#[derive(Clone)]
pub struct CompositionEasingFunction(pub(crate) bindings::CompositionEasingFunction);

/// A key-frame animation that interpolates a scalar (`f32`) property (such as a
/// visual's `Opacity`) through a series of key frames.
#[derive(Clone)]
pub struct ScalarKeyFrameAnimation(pub(crate) bindings::ScalarKeyFrameAnimation);

impl ScalarKeyFrameAnimation {
    /// Inserts a key frame at `progress` (in `0.0..=1.0`) with the given value.
    ///
    /// The segment leading to this frame interpolates smoothly from the previous
    /// frame's value. To hold a value and then jump, insert two frames rather
    /// than reaching for a step easing function: a stepped segment adopts its end
    /// value as soon as the segment begins.
    pub fn insert_key_frame(&self, progress: f32, value: f32) {
        let animation: bindings::IScalarKeyFrameAnimation = self.0.cast().unwrap();
        animation.InsertKeyFrame(progress, value).unwrap();
    }

    /// Inserts a key frame at `progress` (in `0.0..=1.0`) that eases to `value`
    /// along `easing`.
    pub fn insert_key_frame_with_easing(
        &self,
        progress: f32,
        value: f32,
        easing: &CompositionEasingFunction,
    ) {
        let animation: bindings::IScalarKeyFrameAnimation = self.0.cast().unwrap();
        animation
            .InsertKeyFrameWithEasingFunction(progress, value, &easing.0)
            .unwrap();
    }

    /// Inserts a key frame at `progress` whose value is the composition
    /// `expression` (for example `"this.FinalValue"`), eased along `easing`.
    pub fn insert_expression_key_frame_with_easing(
        &self,
        progress: f32,
        expression: &str,
        easing: &CompositionEasingFunction,
    ) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation
            .InsertExpressionKeyFrameWithEasingFunction(progress, expression, &easing.0)
            .unwrap();
    }

    /// Sets how long one iteration of the animation takes.
    pub fn set_duration(&self, duration: Duration) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation.SetDuration(to_time_span(duration)).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit
    /// animation (for example `"Opacity"`).
    pub fn set_target(&self, target: &str) {
        let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
        animation.SetTarget(target).unwrap();
    }

    /// Sets the animation to repeat forever.
    pub fn set_iterate_forever(&self) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation
            .SetIterationBehavior(bindings::AnimationIterationBehavior::Forever)
            .unwrap();
    }
}

impl Sealed for ScalarKeyFrameAnimation {}

impl Animation for ScalarKeyFrameAnimation {
    fn as_animation(&self) -> CompositionAnimation {
        CompositionAnimation(self.0.cast().unwrap())
    }
}

/// A key-frame animation that interpolates a `Vector3` property (such as a
/// visual's `Scale`) through a series of key frames.
#[derive(Clone)]
pub struct Vector3KeyFrameAnimation(pub(crate) bindings::Vector3KeyFrameAnimation);

impl Vector3KeyFrameAnimation {
    /// Inserts a key frame at `progress` (in `0.0..=1.0`) with the given value.
    pub fn insert_key_frame(&self, progress: f32, value: Vector3) {
        self.0.InsertKeyFrame(progress, value).unwrap();
    }

    /// Inserts a key frame at `progress` that eases to `value` along `easing`.
    pub fn insert_key_frame_with_easing(
        &self,
        progress: f32,
        value: Vector3,
        easing: &CompositionEasingFunction,
    ) {
        let animation: bindings::IVector3KeyFrameAnimation = self.0.cast().unwrap();
        animation
            .InsertKeyFrameWithEasingFunction(progress, value, &easing.0)
            .unwrap();
    }

    /// Inserts a key frame at `progress` whose value is the composition
    /// `expression` (for example `"this.FinalValue"`), eased along `easing`.
    pub fn insert_expression_key_frame_with_easing(
        &self,
        progress: f32,
        expression: &str,
        easing: &CompositionEasingFunction,
    ) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation
            .InsertExpressionKeyFrameWithEasingFunction(progress, expression, &easing.0)
            .unwrap();
    }

    /// Sets how long one iteration of the animation takes.
    pub fn set_duration(&self, duration: Duration) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation.SetDuration(to_time_span(duration)).unwrap();
    }

    /// Sets how long to wait before the animation starts.
    pub fn set_delay(&self, delay: Duration) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation.SetDelayTime(to_time_span(delay)).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit
    /// animation (for example `"Scale"`).
    pub fn set_target(&self, target: &str) {
        let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
        animation.SetTarget(target).unwrap();
    }

    /// Sets the animation to run for a fixed number of iterations.
    pub fn set_iteration_count(&self, count: i32) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation
            .SetIterationBehavior(bindings::AnimationIterationBehavior::Count)
            .unwrap();
        animation.SetIterationCount(count).unwrap();
    }

    /// Sets the animation to repeat forever.
    pub fn set_iterate_forever(&self) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation
            .SetIterationBehavior(bindings::AnimationIterationBehavior::Forever)
            .unwrap();
    }
}

impl Sealed for Vector3KeyFrameAnimation {}

impl Animation for Vector3KeyFrameAnimation {
    fn as_animation(&self) -> CompositionAnimation {
        CompositionAnimation(self.0.cast().unwrap())
    }
}

/// A key-frame animation that interpolates a `Vector2` property (such as a
/// visual's `Size`) through a series of key frames.
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
        let animation: bindings::IVector2KeyFrameAnimation = self.0.cast().unwrap();
        animation
            .InsertKeyFrameWithEasingFunction(progress, value, &easing.0)
            .unwrap();
    }

    /// Inserts a key frame at `progress` whose value is the composition
    /// `expression` (for example `"this.FinalValue"`), eased along `easing`.
    pub fn insert_expression_key_frame_with_easing(
        &self,
        progress: f32,
        expression: &str,
        easing: &CompositionEasingFunction,
    ) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation
            .InsertExpressionKeyFrameWithEasingFunction(progress, expression, &easing.0)
            .unwrap();
    }

    /// Sets how long one iteration of the animation takes.
    pub fn set_duration(&self, duration: Duration) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation.SetDuration(to_time_span(duration)).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit
    /// animation (for example `"Offset"`).
    pub fn set_target(&self, target: &str) {
        let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
        animation.SetTarget(target).unwrap();
    }

    /// Sets the animation to repeat forever.
    pub fn set_iterate_forever(&self) {
        let animation: bindings::IKeyFrameAnimation = self.0.cast().unwrap();
        animation
            .SetIterationBehavior(bindings::AnimationIterationBehavior::Forever)
            .unwrap();
    }
}

impl Sealed for Vector2KeyFrameAnimation {}

impl Animation for Vector2KeyFrameAnimation {
    fn as_animation(&self) -> CompositionAnimation {
        CompositionAnimation(self.0.cast().unwrap())
    }
}

/// An animation whose value is a composition expression evaluated every frame
/// by the compositor, rather than a fixed set of key frames.
///
/// The expression may read the properties of other composition objects bound
/// into it by name with
/// [`set_reference_parameter`](Self::set_reference_parameter), which is how one
/// visual is driven from another without either one being touched per frame.
///
/// The expression's text is not parsed when the animation is created or when it
/// is assigned — the compositor validates it only when the animation is started
/// on a property. A malformed expression, or a name the expression uses that was
/// never bound, therefore surfaces from
/// [`Visual::start_animation`](crate::Visual::start_animation) and not from any
/// method on this type.
#[derive(Clone)]
pub struct ExpressionAnimation(pub(crate) bindings::ExpressionAnimation);

impl ExpressionAnimation {
    /// Replaces the expression this animation evaluates.
    ///
    /// Reference parameters already bound are kept, so an animation can be
    /// retargeted by rewriting its expression alone.
    pub fn set_expression(&self, expression: &str) {
        let animation: bindings::IExpressionAnimation = self.0.cast().unwrap();
        animation.SetExpression(expression).unwrap();
    }

    /// Binds `target` into the expression under `name`, making that object's
    /// properties readable from the expression (for example, binding a sibling
    /// visual as `"source"` lets the expression read `source.Offset`).
    ///
    /// The binding holds a reference to `target`, keeping it alive for as long as
    /// this animation is.
    pub fn set_reference_parameter(&self, name: &str, target: &impl Object) {
        let animation: bindings::ICompositionAnimation = self.0.cast().unwrap();
        animation
            .SetReferenceParameter(name, &target.as_object().0)
            .unwrap();
    }

    /// Binds a constant scalar into the expression under `name`.
    pub fn set_scalar_parameter(&self, name: &str, value: f32) {
        let animation: bindings::ICompositionAnimation = self.0.cast().unwrap();
        animation.SetScalarParameter(name, value).unwrap();
    }

    /// Binds a constant `Vector2` into the expression under `name`.
    pub fn set_vector2_parameter(&self, name: &str, value: Vector2) {
        let animation: bindings::ICompositionAnimation = self.0.cast().unwrap();
        animation.SetVector2Parameter(name, value).unwrap();
    }

    /// Binds a constant `Vector3` into the expression under `name`.
    pub fn set_vector3_parameter(&self, name: &str, value: Vector3) {
        let animation: bindings::ICompositionAnimation = self.0.cast().unwrap();
        animation.SetVector3Parameter(name, value).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit
    /// animation (for example `"Offset"`).
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

impl Object for ExpressionAnimation {
    fn as_object(&self) -> CompositionObject {
        CompositionObject(self.0.cast().unwrap())
    }
}

/// A spring animation that carries a scalar (`f32`) property to a final value
/// along a damped-spring curve, continuing from whatever velocity the property
/// already has.
///
/// Because the spring picks up the property's current position and velocity when
/// it starts, one cached animation can be redirected mid-flight: give it a new
/// [final value](Self::set_final_value) (and, if the gesture calls for it, a new
/// [period](Self::set_period)) and start it again. The motion bends toward the
/// new target without the jump a fresh animation would produce.
#[derive(Clone)]
pub struct SpringScalarNaturalMotionAnimation(
    pub(crate) bindings::SpringScalarNaturalMotionAnimation,
);

impl SpringScalarNaturalMotionAnimation {
    /// Sets how quickly the spring loses energy: below `1.0` overshoots and
    /// oscillates, `1.0` settles as fast as it can without overshooting, and
    /// above `1.0` eases in slowly.
    pub fn set_damping_ratio(&self, ratio: f32) {
        let animation: bindings::ISpringScalarNaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetDampingRatio(ratio).unwrap();
    }

    /// Sets the spring's undamped period — the time one oscillation would take
    /// with no damping, which sets the overall pace of the motion.
    pub fn set_period(&self, period: Duration) {
        let animation: bindings::ISpringScalarNaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetPeriod(to_time_span(period)).unwrap();
    }

    /// Sets the value the spring settles at.
    pub fn set_final_value(&self, value: f32) {
        let animation: bindings::IScalarNaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetFinalValue(Some(value)).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit
    /// animation (for example `"Opacity"`).
    pub fn set_target(&self, target: &str) {
        let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
        animation.SetTarget(target).unwrap();
    }
}

impl Sealed for SpringScalarNaturalMotionAnimation {}

impl Animation for SpringScalarNaturalMotionAnimation {
    fn as_animation(&self) -> CompositionAnimation {
        CompositionAnimation(self.0.cast().unwrap())
    }
}

/// A spring animation that carries a `Vector2` property to a final value along a
/// damped-spring curve.
///
/// See [`SpringScalarNaturalMotionAnimation`] for how a running spring is
/// redirected mid-flight.
#[derive(Clone)]
pub struct SpringVector2NaturalMotionAnimation(
    pub(crate) bindings::SpringVector2NaturalMotionAnimation,
);

impl SpringVector2NaturalMotionAnimation {
    /// Sets how quickly the spring loses energy: below `1.0` overshoots and
    /// oscillates, `1.0` settles as fast as it can without overshooting, and
    /// above `1.0` eases in slowly.
    pub fn set_damping_ratio(&self, ratio: f32) {
        let animation: bindings::ISpringVector2NaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetDampingRatio(ratio).unwrap();
    }

    /// Sets the spring's undamped period — the time one oscillation would take
    /// with no damping, which sets the overall pace of the motion.
    pub fn set_period(&self, period: Duration) {
        let animation: bindings::ISpringVector2NaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetPeriod(to_time_span(period)).unwrap();
    }

    /// Sets the value the spring settles at.
    pub fn set_final_value(&self, value: Vector2) {
        let animation: bindings::IVector2NaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetFinalValue(Some(value)).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit
    /// animation (for example `"Size"`).
    pub fn set_target(&self, target: &str) {
        let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
        animation.SetTarget(target).unwrap();
    }
}

impl Sealed for SpringVector2NaturalMotionAnimation {}

impl Animation for SpringVector2NaturalMotionAnimation {
    fn as_animation(&self) -> CompositionAnimation {
        CompositionAnimation(self.0.cast().unwrap())
    }
}

/// A spring animation that carries a `Vector3` property to a final value along a
/// damped-spring curve.
///
/// See [`SpringScalarNaturalMotionAnimation`] for how a running spring is
/// redirected mid-flight.
#[derive(Clone)]
pub struct SpringVector3NaturalMotionAnimation(
    pub(crate) bindings::SpringVector3NaturalMotionAnimation,
);

impl SpringVector3NaturalMotionAnimation {
    /// Sets how quickly the spring loses energy: below `1.0` overshoots and
    /// oscillates, `1.0` settles as fast as it can without overshooting, and
    /// above `1.0` eases in slowly.
    pub fn set_damping_ratio(&self, ratio: f32) {
        let animation: bindings::ISpringVector3NaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetDampingRatio(ratio).unwrap();
    }

    /// Sets the spring's undamped period — the time one oscillation would take
    /// with no damping, which sets the overall pace of the motion.
    pub fn set_period(&self, period: Duration) {
        let animation: bindings::ISpringVector3NaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetPeriod(to_time_span(period)).unwrap();
    }

    /// Sets the value the spring settles at.
    pub fn set_final_value(&self, value: Vector3) {
        let animation: bindings::IVector3NaturalMotionAnimation = self.0.cast().unwrap();
        animation.SetFinalValue(Some(value)).unwrap();
    }

    /// Sets the property this animation targets when used as an implicit
    /// animation (for example `"Scale"`).
    pub fn set_target(&self, target: &str) {
        let animation: bindings::ICompositionAnimation2 = self.0.cast().unwrap();
        animation.SetTarget(target).unwrap();
    }
}

impl Sealed for SpringVector3NaturalMotionAnimation {}

impl Animation for SpringVector3NaturalMotionAnimation {
    fn as_animation(&self) -> CompositionAnimation {
        CompositionAnimation(self.0.cast().unwrap())
    }
}

/// A map of property-name → animation applied to a visual so that changes to
/// those properties animate automatically.
///
/// Create one with
/// [`Compositor::create_implicit_animation_collection`](crate::Compositor::create_implicit_animation_collection),
/// populate it with [`insert`](Self::insert), then attach it via
/// [`Visual::set_implicit_animations`](crate::Visual::set_implicit_animations).
#[derive(Clone)]
pub struct ImplicitAnimationCollection(pub(crate) bindings::ImplicitAnimationCollection);

impl ImplicitAnimationCollection {
    /// Associates `animation` with the property named `target` (for example
    /// `"Opacity"`). The animation should
    /// [target](ScalarKeyFrameAnimation::set_target) the same property.
    pub fn insert(&self, target: &str, animation: &impl Animation) {
        let map: windows_collections::IMap<
            windows_core::HSTRING,
            bindings::ICompositionAnimationBase,
        > = self.0.cast().unwrap();
        let base: bindings::ICompositionAnimationBase = animation.as_animation().0.cast().unwrap();
        map.Insert(&windows_core::HSTRING::from(target), &base)
            .unwrap();
    }
}

impl Compositor {
    /// Creates a `Vector2` key-frame animation.
    pub fn create_vector2_key_frame_animation(&self) -> Vector2KeyFrameAnimation {
        bump_count(Count::Animation);
        Vector2KeyFrameAnimation(self.0.CreateVector2KeyFrameAnimation().unwrap())
    }

    /// Creates an animation that evaluates `expression` every frame.
    ///
    /// The expression is not parsed here — see [`ExpressionAnimation`] for where
    /// a malformed one surfaces.
    pub fn create_expression_animation(&self, expression: &str) -> ExpressionAnimation {
        bump_count(Count::Animation);
        ExpressionAnimation(
            self.0
                .CreateExpressionAnimationWithExpression(expression)
                .unwrap(),
        )
    }

    /// Creates a spring animation for a scalar (`f32`) property.
    pub fn create_spring_scalar_animation(&self) -> SpringScalarNaturalMotionAnimation {
        bump_count(Count::Animation);
        let compositor: bindings::ICompositor4 = self.0.cast().unwrap();
        SpringScalarNaturalMotionAnimation(compositor.CreateSpringScalarAnimation().unwrap())
    }

    /// Creates a spring animation for a `Vector2` property.
    pub fn create_spring_vector2_animation(&self) -> SpringVector2NaturalMotionAnimation {
        bump_count(Count::Animation);
        let compositor: bindings::ICompositor4 = self.0.cast().unwrap();
        SpringVector2NaturalMotionAnimation(compositor.CreateSpringVector2Animation().unwrap())
    }

    /// Creates a spring animation for a `Vector3` property.
    pub fn create_spring_vector3_animation(&self) -> SpringVector3NaturalMotionAnimation {
        bump_count(Count::Animation);
        let compositor: bindings::ICompositor4 = self.0.cast().unwrap();
        SpringVector3NaturalMotionAnimation(compositor.CreateSpringVector3Animation().unwrap())
    }

}
