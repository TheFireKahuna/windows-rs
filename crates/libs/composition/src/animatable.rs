//! Animation and expression binding, the two capabilities every composition object carries
//! (feature `system`).
//!
//! `StartAnimation` and `StopAnimation` are `CompositionObject` methods, so a drop shadow's
//! blur radius is animatable exactly as a visual's offset is. An expression animation takes
//! a `CompositionObject` as a named parameter, so a clip, a property set or an interaction
//! tracker can be read by one as readily as a visual.
//!
//! The [`Animatable`] trait carries both as provided methods. A type that wraps a
//! composition object implements `as_object` and gets the rest, so what can be animated is
//! exactly what implements [`Animatable`].

use super::*;

/// The base type shared by every composition object — visuals, brushes, clips,
/// geometries, shapes, shadows, property sets, surfaces and interaction trackers.
#[derive(Clone)]
pub struct CompositionObject(pub(crate) bindings::CompositionObject);

/// A composition object: animatable, and readable by an
/// [`ExpressionAnimation`](crate::ExpressionAnimation) that references it.
///
/// The platform names this capability `IAnimationObject`, which `CompositionObject`
/// implements. Both animation and the ability to be named in an expression that reads these
/// properties back out therefore belong to every composition object, not to visuals alone.
///
/// This trait is sealed: only the types in this crate implement it.
pub trait Animatable: Sealed {
    /// Returns this value as the shared [`CompositionObject`] base type.
    fn as_object(&self) -> CompositionObject;

    /// Starts an animation on the named property (for example `"Offset"`,
    /// `"BlurRadius"`, `"TrimEnd"`, `"TopLeftRadiusX"`).
    ///
    /// The property name is the WinRT one. Which names an object accepts depends on that
    /// object; this crate's setters name the animation target where it is not obvious.
    fn start_animation(&self, property: &str, animation: &impl Animation) {
        let object = self.as_object();
        let object: bindings::ICompositionObject = object.0.cast().unwrap();
        object
            .StartAnimation(property, &animation.as_animation().0)
            .unwrap();
    }

    /// Returns this object's own property set, the named values stored on it.
    ///
    /// A value inserted here is animatable and readable from an expression that references
    /// the object, so `"card.Progress"` needs no second object to hold `Progress`. Use it
    /// when a value belongs to one object, and a standalone
    /// [`CompositionPropertySet`](crate::CompositionPropertySet) when one value drives many.
    fn properties(&self) -> CompositionPropertySet {
        let object = self.as_object();
        let object: bindings::ICompositionObject = object.0.cast().unwrap();
        CompositionPropertySet(object.Properties().unwrap())
    }

    /// Stops any animation on the named property, leaving it at the value it had reached.
    ///
    /// A failure is discarded rather than panicked on, unlike most of this crate. Stopping
    /// a property that nothing is animating is the ordinary case, so a caller taking a
    /// property back under manual control stops it unconditionally and then sets it,
    /// without tracking whether an animation was ever started.
    fn stop_animation(&self, property: &str) {
        let object = self.as_object();
        let Ok(object) = object.0.cast::<bindings::ICompositionObject>() else {
            return;
        };
        let _ = object.StopAnimation(property);
    }
}

// The trait exposes no "is this property animating" predicate.
// `TryGetAnimationController` returns playback controls for a key-frame animation, or null
// when the animation is not found, and a null class return arrives here as a successful
// call — so it cannot distinguish the two. Whether an animation is running is state the
// caller that started it holds.

/// Returns the `IUnknown` pointer of `value`, which COM guarantees is stable and unique per
/// object, or null if the cast fails.
///
/// The wrapper types' `PartialEq` impls compare objects through it, so the pointer never
/// leaves the crate.
pub(crate) fn canonical(value: &impl Interface) -> *mut core::ffi::c_void {
    value
        .cast::<windows_core::IUnknown>()
        .map_or(core::ptr::null_mut(), |unknown| unknown.as_raw())
}

/// Implements `Animatable` for wrappers whose single field is the composition object. The
/// `sealed:` list names the types whose `Sealed` impl is emitted here too, their own module
/// having no other reason to seal them.
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

// Every composition object this crate wraps, and so everything it can animate or bind into
// an expression. A wrapper missing from this list has no reachable animatable properties,
// so a new wrapper type is added here as well.
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
