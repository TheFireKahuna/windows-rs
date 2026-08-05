#![doc = include_str!("../readme.md")]

// `system` and `reactor` select compatible generated bindings for the same
// handwritten wrappers.
#[cfg(all(not(feature = "system"), not(feature = "reactor")))]
compile_error!(
    "enable exactly one composition stack: the `system` feature (default) or the `reactor` feature"
);
#[cfg(all(feature = "system", feature = "reactor"))]
compile_error!(
    "the `system` and `reactor` composition stacks are mutually exclusive; enable only one"
);

// An interface named in the filter's `--implement` list also keeps its caller-side
// projection, and this crate receives the tracker owner's six callbacks rather than calling
// them, so those projections go unused. `allow` rather than `expect`, because the count of
// unused generated items is not something this file asserts. It covers the generated module
// alone: an unused filter entry anywhere else still reports through `--dead-code`.
#[cfg(feature = "system")]
#[allow(dead_code)]
#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::missing_transmute_annotations
)]
#[path = "bindings.rs"]
mod bindings;
#[cfg(feature = "reactor")]
#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms
)]
#[path = "bindings_lifted.rs"]
mod bindings;

mod animation;
mod batch;
mod brush;
mod color;
mod compositor;
mod geometry;
mod shape;
mod visual;

// Only system composition hosts an HWND directly. Lifted composition is hosted
// in a WinUI element.
#[cfg(feature = "system")]
mod stack;
#[cfg(feature = "system")]
mod surface;
#[cfg(feature = "system")]
mod target;

// The retained-composition surface: clips, paths, masks, springs, expressions, property
// sets, interaction trackers, and the constructions built out of them. Carried for the
// system stack only, matching the filter's `// region: system-only` block. Each capability
// is a module of its own with its own `impl Compositor`, so adding one touches two lines
// here and nothing else.
#[cfg(feature = "system")]
mod animatable;
#[cfg(feature = "system")]
mod clip;
#[cfg(feature = "system")]
mod idiom;
#[cfg(feature = "system")]
mod interactions;
#[cfg(feature = "system")]
mod mask;
#[cfg(feature = "system")]
mod motion;
#[cfg(feature = "system")]
mod path;
#[cfg(feature = "system")]
mod property_set;
#[cfg(feature = "system")]
mod retained;
#[cfg(feature = "system")]
mod shadow;

mod sealed {
    /// Prevents downstream crates from implementing this crate's marker traits
    /// ([`Brush`](crate::Brush), [`Shape`](crate::Shape),
    /// [`Geometry`](crate::Geometry), [`Animation`](crate::Animation), and — on the system
    /// stack — [`Clip`](crate::Clip), [`Surface`](crate::Surface) and
    /// [`Animatable`](crate::Animatable)).
    pub trait Sealed {}
}

// Wrapper modules import these through `super::*`.
pub(crate) use sealed::Sealed;
pub(crate) use windows_core::Interface;

// Object identity for the wrapper types' `PartialEq` impls, which compare two objects
// without an interface pointer leaving the crate.
#[cfg(feature = "system")]
pub(crate) use animatable::canonical;

pub use animation::{
    Animation, CompositionAnimation, CompositionAnimationGroup, CompositionEasingFunction,
    ImplicitAnimationCollection, ScalarKeyFrameAnimation, Vector3KeyFrameAnimation,
};
pub use batch::{BatchKind, CompositionScopedBatch};
pub use brush::{Brush, CompositionBrush, CompositionColorBrush, CompositionNineGridBrush};
pub use color::Color;
pub use compositor::Compositor;
pub use geometry::Geometry;
pub use shape::{
    CompositionContainerShape, CompositionEllipseGeometry, CompositionGeometry, CompositionShape,
    CompositionShapeCollection, CompositionSpriteShape, Shape, ShapeVisual,
};
pub use visual::{BorderMode, ContainerVisual, SpriteVisual, Visual, VisualCollection};

#[cfg(feature = "system")]
pub use stack::DispatcherQueueController;
#[cfg(feature = "system")]
pub use surface::{
    AlphaMode, CompositionDrawHandle, CompositionDrawingSurface, CompositionGraphicsDevice,
    CompositionSurface, CompositionSurfaceBrush, CompositionVirtualDrawingSurface,
    CompositionVisualSurface, PixelFormat, Stretch, Surface,
};
#[cfg(feature = "system")]
pub use target::DesktopWindowTarget;

#[cfg(feature = "system")]
pub use animatable::{Animatable, CompositionObject};
#[cfg(feature = "system")]
pub use clip::{Clip, CompositionClip, CompositionGeometricClip, InsetClip, RectangleClip};
#[cfg(feature = "system")]
pub use idiom::Captured;
#[cfg(feature = "system")]
pub use interactions::{
    BindingAxes, ChainingMode, Clamping, InertiaModifier, InteractionTracker, RedirectionMode,
    RequestId, ScaleAnimationPolicy, SourceMode, TrackerEvent, VisualInteractionSource, WheelMode,
};
#[cfg(feature = "system")]
pub use mask::{CompositionLinearGradientBrush, CompositionMaskBrush, MappingMode};
#[cfg(feature = "system")]
pub use motion::{
    ExpressionAnimation, SpringScalarNaturalMotionAnimation, SpringVector2NaturalMotionAnimation,
    SpringVector3NaturalMotionAnimation, Vector2KeyFrameAnimation,
};
#[cfg(feature = "system")]
pub use path::{
    CompositionPath, CompositionPathGeometry, CompositionRoundedRectangleGeometry, StrokeCap,
    StrokeJoin,
};
#[cfg(feature = "system")]
pub use property_set::CompositionPropertySet;
#[cfg(feature = "system")]
pub use shadow::{DropShadow, ShadowSource};

pub use windows_core::Result;
pub use windows_numerics::{Vector2, Vector3};
