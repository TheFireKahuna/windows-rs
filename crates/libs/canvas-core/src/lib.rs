//! Reactor-free 2D drawing core for `windows-canvas`.
//!
//! This crate holds every drawing primitive that does **not** depend on
//! `windows-reactor`: the D2D/DWrite/DXGI bindings, [`GpuDevice`],
//! [`DrawingSession`], [`SwapChain`], text layout, geometry, and color. The
//! `windows-canvas` crate re-exports all of it (`pub use windows_canvas_core::*`)
//! and adds the reactor-coupled helpers (`surface_painter`, `animated_canvas`,
//! composition surfaces) on top — so existing `windows_canvas::*` paths are
//! unchanged. Splitting the core out lets `windows-reactor`'s self-hosted
//! DirectComposition backend reuse the drawing code without forming a
//! `canvas -> reactor -> canvas` dependency cycle.

#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::upper_case_acronyms,
    clippy::missing_transmute_annotations,
    clippy::too_many_arguments
)]
// `pub` (doc-hidden) so the `windows-canvas` wrapper crate's reactor modules can
// reach the raw D2D/DWrite vtables they bracket. Not part of the stable surface.
#[doc(hidden)]
pub mod bindings;
mod bitmap;
mod color;
mod device;
mod device_lost;
mod effect;
mod geometry;
mod session;
mod swap_chain;
mod text;
mod types;

use bindings::*;
pub use device_lost::{check_device_lost, is_device_lost};
use std::cell::Cell;
use std::os::windows::ffi::OsStrExt;
use windows_core::*;

pub use bitmap::Bitmap;
pub use color::ColorF;
pub(crate) use device::D2dLock;
pub use device::{GpuDevice, SharedGpuDevice};
pub use effect::Effect;
pub use geometry::*;
/// Set a device context's DPI so drawing happens in DIPs.
///
/// The generated binding methods are crate-private, so the `windows-canvas` surface
/// hosts — which bracket the raw D2D calls themselves — go through this shim rather
/// than the vtable directly. Not part of the stable surface.
#[doc(hidden)]
pub fn set_context_dpi(context: &ID2D1DeviceContext, dpi: f32) {
    unsafe { context.SetDpi(dpi, dpi) };
}

pub use session::DrawingSession;
pub use swap_chain::{SwapChain, WaitObject};
pub use text::*;
pub use types::*;

pub use windows_core::Result;
pub use windows_numerics::{Matrix3x2, Vector2};
