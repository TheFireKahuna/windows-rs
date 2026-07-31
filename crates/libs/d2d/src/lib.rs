#![doc = include_str!("../readme.md")]

// Every `unsafe` block in this crate is a call into a generated Direct2D binding, which
// COM cannot express as safe in a signature. What discharges it is uniform enough to
// state once instead of at eighty call sites: the interface pointer is owned by the
// wrapper and can be neither null nor dangling, every out-parameter is a stack local
// that outlives its call, and no Direct2D method here retains a borrow past its return.
// The two places that genuinely ask something of the *caller* are `Gpu::adopt` and the
// composition bridge, and both are marked and say what they need.
// `dead_code` is `expect` and not `allow`, for one standing reason that has to stay true:
// naming an enum *type* in the filter generates every one of its constants, and
// `D2D1_RENDERING_CONTROLS` has a `D2D1_BUFFER_PRECISION` field, so the five precisions
// this crate never sets come along with the one it does. Every other unused binding is a
// filter entry the wrapper does not consume — which is exactly what this warning is for,
// so if it ever reports something that is not a generated enum sibling, trim the filter
// rather than widening this.
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
// boundary, and the two objects that must — the Direct2D and Direct3D devices — leave as
// `&impl Interface` for a sibling crate to cast to its own projection.
pub(crate) use bindings::*;
pub(crate) use brush::d2d_color;
pub(crate) use sealed::Sealed;
#[cfg(feature = "composition")]
pub(crate) use target::check_dpi;
pub(crate) use windows_core::Interface;

/// The one pixel format. Named here and never a parameter: an 8-bit surface is treated
/// as sRGB and colour-managed by the compositor on terms we do not control, and a UNORM
/// one cannot hold a value above white or outside Rec.709 at all.
pub(crate) const FORMAT: DXGI_FORMAT = DXGI_FORMAT_R16G16B16A16_FLOAT;

/// Direct2D's default flattening tolerance, in *target* space — which is why a
/// realization divides it by the scale it is built for rather than using it raw.
pub(crate) const FLATTEN: f32 = 0.25;

pub use batch::{Interp, SpriteBatch};
pub use brush::{
    Brush, BrushRef, Cap, Extend, Join, Ramp, Solid, Stop, Stroke, StrokeSpec, StrokeStyle, Tile,
};
pub use device::{Gpu, Loss, classify};
pub use geometry::{
    Bezier, Combine, Ellipse, End, Figure, Path, Realization, Rect, RoundedRect, Shape, Sink,
};
pub use pass::{
    Additive, Aliased, Clipped, Draw, GlyphRun, Layer, Layered, Pass, PassError,
    TextParams, Transformed,
};
pub use target::{Opacity, Readback, Target};

#[cfg(feature = "composition")]
pub use comp::{Fp16Surface, SurfaceDraw};

pub use windows_color::Scrgb;
pub use windows_core::Result;
pub use windows_numerics::{Matrix3x2, Vector2};
