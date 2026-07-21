#![doc = include_str!("../readme.md")]

#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::upper_case_acronyms,
    clippy::missing_transmute_annotations,
    clippy::too_many_arguments
)]
// `pub` (doc-hidden) so consumers that bracket the raw D2D/DWrite calls
// themselves — the reactor's self-hosted DirectComposition backend — can reach
// the vtables. Not part of the stable surface.
#[doc(hidden)]
pub mod bindings;
mod bitmap;
mod color;
#[cfg(feature = "composition")]
mod composition;
mod device;
mod device_lost;
mod effect;
mod geometry;
mod glyph_coverage;
mod glyphs;
mod layer;
mod render_target;
mod session;
mod swap_chain;
mod text;
mod types;

pub use bindings::ID2D1DeviceContext;
use bindings::*;
pub use device_lost::{check_device_lost, device_lost_error, is_device_lost};
use std::cell::Cell;
use std::os::windows::ffi::OsStrExt;
use windows_core::*;

pub use bitmap::Bitmap;
pub use color::ColorF;
#[cfg(feature = "composition")]
pub use composition::CanvasCompositionExt;
pub(crate) use device::D2dLock;
pub use device::{GpuDevice, SharedGpuDevice};
pub use effect::Effect;
pub use geometry::*;
pub use glyph_coverage::{GlyphCoverage, condition, glyph_run_coverage};
pub use glyphs::{
    DecorationKind, FontFace, FontMetrics, GlyphMetrics, GlyphOffset, GlyphRun, ShapedText,
    TextDecoration,
};
pub use layer::LayerRenderer;
pub use render_target::RenderTarget;
pub use session::DrawingSession;
pub use swap_chain::{SwapChain, WaitObject};
pub use text::*;
pub use types::*;

pub use windows_core::Result;
pub use windows_numerics::{Matrix3x2, Vector2};
