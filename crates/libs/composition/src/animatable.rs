//! What every composition object can do: be animated, and be read by an expression
//! (feature `system`).
//!
//! `CompositionObject` is the base class of the whole composition API, and two capabilities
//! live on it rather than on any one derived type: **animation** — `StartAnimation` and
//! `StopAnimation` are `CompositionObject` methods, so *every* composition object is
//! animatable, a drop shadow's blur radius exactly as much as a visual's offset — and
//! **reference binding**, since an expression animation takes a `CompositionObject` as a
//! named parameter and can therefore read from a clip, a property set or an interaction
//! tracker and not only from a visual.
//!
//! So the [`Animatable`] trait carries both, once, as provided methods. A type that wraps a
//! composition object implements `as_object` and gets the rest; nothing re-implements
//! animation per type, and "what can be animated" is exactly "what implements `Animatable`".

use super::*;

/// The base type shared by every composition object — visuals, brushes, clips,
/// geometries, shapes, shadows, property sets, surfaces and interaction trackers.
#[derive(Clone)]
pub struct CompositionObject(pub(crate) bindings::CompositionObject);

/// A composition object: animatable, and readable by an
/// [`ExpressionAnimation`](crate::ExpressionAnimation) that references it.
///
/// The platform's own name for this capability is `IAnimationObject`, which
/// `CompositionObject` implements — animation is not a property of visuals, it is a property
/// of *being* a composition object, and so is the ability to be named in an expression and
/// have those same properties read back out.
///
/// This trait is sealed: only the types in this crate implement it.
pub trait Animatable: Sealed {
    /// Returns this value as the shared [`CompositionObject`] base type.
    fn as_object(&self) -> CompositionObject;

    /// Starts an animation on the named property (for example `"Offset"`,
    /// `"BlurRadius"`, `"TrimEnd"`, `"TopLeftRadiusX"`).
    ///
    /// The property name is the WinRT one, and which names an object accepts is a property
    /// of that object — this crate's setters name the animation target in their
    /// documentation where it is not obvious.
    fn start_animation(&self, property: &str, animation: &impl Animation) {
        let object = self.as_object();
        let object: bindings::ICompositionObject = object.0.cast().unwrap();
        object
            .StartAnimation(property, &animation.as_animation().0)
            .unwrap();
    }

    /// This object's own property set — a bag of named values that live on it.
    ///
    /// Anything inserted here is animatable and readable from an expression that references
    /// the object, so `"card.Progress"` needs no second object to hold `Progress`. Prefer it
    /// when a value belongs to one object; prefer a standalone
    /// [`CompositionPropertySet`](crate::CompositionPropertySet) when one value drives many.
    fn properties(&self) -> CompositionPropertySet {
        let object = self.as_object();
        let object: bindings::ICompositionObject = object.0.cast().unwrap();
        CompositionPropertySet(object.Properties().unwrap())
    }

    /// Stops any animation on the named property, leaving it at the value it had reached.
    ///
    /// Unlike most of this crate, a failure here is discarded rather than panicked on.
    /// Stopping a property that nothing is animating is the ordinary case, not an
    /// exceptional one: a caller taking a property back under manual control stops it
    /// unconditionally and then sets it, without first having to know whether an animation
    /// was ever started. Panicking would make that correct sequence depend on animation
    /// state the caller does not otherwise track, so the inconsistency is deliberate.
    fn stop_animation(&self, property: &str) {
        let object = self.as_object();
        let Ok(object) = object.0.cast::<bindings::ICompositionObject>() else {
            return;
        };
        let _ = object.StopAnimation(property);
    }
}

// There is deliberately no `is_animating`. `TryGetAnimationController` is documented to
// return an `AnimationController` — playback controls for a *key-frame* animation — or null
// when "the animation is not found", and a null class return arrives here as a successful
// call, so a predicate built on it reports `true` whether or not anything is animating. A
// caller that must not restart a live animation knows what it started; that is a fact about
// its own state, not one to be inferred from a compositor round trip that cannot answer it.

/// The `IUnknown` pointer of `value`, which COM guarantees is stable and unique per
/// object — the only sound basis for comparing two interface pointers.
///
/// Used by the wrapper types' `PartialEq` impls, which is how a caller asks "is this the
/// same brush I already bound?" without the pointer itself ever leaving the crate.
pub(crate) fn canonical(value: &impl Interface) -> *mut core::ffi::c_void {
    value
        .cast::<windows_core::IUnknown>()
        .map_or(core::ptr::null_mut(), |unknown| unknown.as_raw())
}

/// Implements `Animatable` for a wrapper whose single field is the composition object, which is
/// every one of them. `sealed` marks the types whose `Sealed` impl belongs here rather than
/// beside the type, because their own module has no other reason to seal them.
macro_rules! composition_object {
    (sealed: $($sealed:ident),* $(,)?; $($name:ident),* $(,)?) => {
        $(
            impl Sealed for $sealed {}
        )*
        $(
            impl Animatable for $name {
                fn as_object(&self) -> CompositionObject {
                    CompositionObject(self.0.cast().unwrap())
                }
            }
        )*
    };
}

// Every composition object this crate wraps, and therefore everything it can animate or
// bind into an expression. A type missing from this list is one whose documented animatable
// properties cannot be reached — which is the failure the list exists to prevent: a
// `DropShadow` whose blur radius is documented as animatable, with nothing to animate it
// with, is worse than one that does not mention animation at all.
composition_object!(
    sealed: Visual, CompositionShape, CompositionGeometry, CompositionClip, DropShadow,
            VisualInteractionSource, InteractionTracker;
    Visual,
    CompositionBrush,
    CompositionColorBrush,
    CompositionMaskBrush,
    CompositionLinearGradientBrush,
    CompositionNineGridBrush,
    CompositionSurfaceBrush,
    CompositionShape,
    CompositionSpriteShape,
    CompositionGeometry,
    CompositionEllipseGeometry,
    CompositionPathGeometry,
    CompositionRoundedRectangleGeometry,
    CompositionClip,
    InsetClip,
    RectangleClip,
    CompositionGeometricClip,
    CompositionVisualSurface,
    DropShadow,
    CompositionPropertySet,
    InteractionTracker,
    VisualInteractionSource,
);

impl PartialEq for Visual {
    fn eq(&self, other: &Self) -> bool {
        canonical(&self.0) == canonical(&other.0)
    }
}

impl Eq for Visual {}
