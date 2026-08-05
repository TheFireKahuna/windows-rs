#![doc = include_str!("../readme.md")]

// Every `unsafe` block in this crate calls a generated Direct2D binding, and one invariant
// discharges all of them: the interface pointer is owned by the wrapper, so it is neither
// null nor dangling; every out-parameter is a stack local outliving its call; and no
// Direct2D method used here retains a borrow past its return. `Gpu::adopt` and the
// composition bridge require something of the caller as well, and each states it.
//
// `dead_code` is expected because naming an enum *type* in the binding filter generates
// every one of its constants: `D2D1_RENDERING_CONTROLS` has a `D2D1_BUFFER_PRECISION`
// field, so the five precisions this crate never sets come along with the one it does. A
// report for anything other than a generated enum sibling names a binding the filter lists
// and the wrapper does not consume.
#[expect(
    dead_code,
    non_snake_case,
    non_camel_case_types,
    clippy::missing_transmute_annotations,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments
)]
mod bindings;

mod batch;
mod brush;
mod device;
mod geometry;
mod pass;
mod target;

#[cfg(feature = "composition")]
mod comp;

mod sealed {
    /// Prevents downstream crates from implementing [`Brush`](crate::Brush).
    pub trait Sealed {}
}

// Re-exported crate-wide so every module can `use super::*;` rather than naming the
// generated types it touches. None of it is public: no generated struct crosses a crate
// boundary, and the Direct2D and Direct3D devices leave as `&impl Interface` for a sibling
// crate to cast to its own projection.
pub(crate) use bindings::*;
pub(crate) use brush::d2d_color;
pub(crate) use sealed::Sealed;
#[cfg(feature = "composition")]
pub(crate) use target::check_dpi;
pub(crate) use windows_core::Interface;

/// The pixel format every render target in this crate carries.
///
/// Never a parameter: an 8-bit surface is treated as sRGB and colour-managed by the
/// compositor, and a UNORM format holds no value above white or outside Rec.709.
pub(crate) const FORMAT: DXGI_FORMAT = DXGI_FORMAT_R16G16B16A16_FLOAT;

/// Direct2D's default flattening tolerance, in *target* space. [`Gpu::realize`] divides it
/// by the scale a realization is tessellated at.
pub(crate) const FLATTEN: f32 = 0.25;

pub use batch::{Interp, SpriteBatch};
pub use brush::{
    Brush, BrushRef, Cap, Extend, Join, Radial, Ramp, Solid, Stop, Stroke, StrokeSpec, StrokeStyle,
    Tile,
};
pub use device::{Gpu, Loss, classify};
pub use geometry::{
    Bezier, Combine, Ellipse, End, Figure, Path, Realization, Rect, RoundedRect, Shape, Sink,
};
pub use pass::{
    Additive, Aliased, Clipped, Draw, GlyphRun, Layer, Layered, Pass, PassError, TextParams,
    Transformed,
};
pub use target::{Opacity, Readback, Target};

#[cfg(feature = "composition")]
pub use comp::{SceneSurface, SurfaceDraw};

pub use windows_color::Scrgb;
pub use windows_core::Result;
pub use windows_numerics::{Matrix3x2, Vector2};
