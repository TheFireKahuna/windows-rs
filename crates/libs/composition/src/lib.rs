#![doc = include_str!("../readme.md")]

// A single wrapper surface over two composition stacks, forked at compile time.
// Exactly one of `system` (Windows.UI.Composition, default) or `lifted`
// (the lifted Microsoft.UI.Composition stack) selects the generated bindings.
// Both are generated from one filter by `tool_composition` and expose the same
// type and method names, so every wrapper module below compiles unchanged
// against either.
#[cfg(all(not(feature = "system"), not(feature = "lifted")))]
compile_error!(
    "enable exactly one composition stack: the `system` feature (default) or the `lifted` feature"
);
#[cfg(all(feature = "system", feature = "lifted"))]
compile_error!(
    "the `system` and `lifted` composition stacks are mutually exclusive; enable only one"
);

#[cfg(feature = "system")]
#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms
)]
#[path = "bindings.rs"]
mod bindings;
// `not(system)` as well as `lifted`, so enabling both stacks by mistake leaves
// exactly one `bindings` module and the reader sees the `compile_error!` above
// rather than a redefinition error stacked on top of it.
#[cfg(all(feature = "lifted", not(feature = "system")))]
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
mod clip;
mod color;
mod compositor;
mod object;
mod property_set;
mod shadow;
mod shape;
mod visual;

// Standalone HWND hosting is a system-stack capability (it also owns the
// `windows-window` dependency); lifted composition is hosted inside a WinUI
// element instead.
#[cfg(feature = "system")]
mod stack;
#[cfg(feature = "system")]
mod surface;
#[cfg(feature = "system")]
mod target;

mod sealed {
    /// Prevents downstream crates from implementing this crate's marker traits
    /// ([`Brush`](crate::Brush), [`Shape`](crate::Shape),
    /// [`Geometry`](crate::Geometry), [`Animation`](crate::Animation),
    /// [`Object`](crate::Object)).
    pub trait Sealed {}
}

// Re-exported crate-wide so every wrapper module can `use super::*;` instead of
// naming these explicitly. `Sealed` and `Interface` stay crate-internal; only
// the wrapper types and `Result` form the public surface.
pub(crate) use object::canonical;
pub(crate) use sealed::Sealed;
pub(crate) use windows_core::Interface;

pub use animation::{
    Animation, CompositionAnimation, CompositionEasingFunction, ExpressionAnimation,
    ImplicitAnimationCollection, ScalarKeyFrameAnimation, SpringScalarNaturalMotionAnimation,
    SpringVector2NaturalMotionAnimation, SpringVector3NaturalMotionAnimation,
    Vector2KeyFrameAnimation, Vector3KeyFrameAnimation,
};
pub use batch::{BatchKind, CompositionScopedBatch};
pub use brush::{
    Brush, CompositionBrush, CompositionColorBrush, CompositionMaskBrush,
    CompositionNineGridBrush,
};
pub use clip::InsetClip;
pub use color::Color;
pub use compositor::Compositor;
pub use object::{CompositionObject, Object};
pub use property_set::CompositionPropertySet;
pub use shadow::{DropShadow, ShadowSource};
pub use shape::{
    CompositionContainerShape, CompositionEllipseGeometry, CompositionGeometry, CompositionPath,
    CompositionPathGeometry, CompositionRoundedRectangleGeometry, CompositionShape,
    CompositionShapeCollection, CompositionSpriteShape, Geometry, Shape, ShapeVisual, StrokeCap,
};
pub use visual::{BorderMode, ContainerVisual, SpriteVisual, Visual, VisualCollection};

#[cfg(feature = "system")]
pub use stack::DispatcherQueueController;
#[cfg(feature = "system")]
pub use surface::{
    AlphaMode, CompositionDrawHandle, CompositionDrawingSurface, CompositionGraphicsDevice,
    CompositionSurface, CompositionSurfaceBrush, CompositionVisualSurface, PixelFormat, Stretch,
    Surface,
};
#[cfg(feature = "system")]
pub use target::DesktopWindowTarget;

pub use windows_core::Result;
pub use windows_numerics::{Vector2, Vector3};
