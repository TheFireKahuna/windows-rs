#![doc = include_str!("../readme.md")]

#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::missing_transmute_annotations,
    clippy::too_many_arguments
)]
mod bindings;
mod bitmap;
mod color;
#[cfg(feature = "reactor")]
mod composition;
mod device;
mod device_lost;
mod effect;
mod geometry;
#[cfg(feature = "reactor")]
mod reactor;
mod session;
#[cfg(feature = "reactor")]
mod surface_image;
mod swap_chain;
mod text;
mod types;
#[cfg(feature = "reactor")]
mod virtual_surface_image;

use bindings::*;
pub use device_lost::{check_device_lost, is_device_lost};
use std::cell::Cell;
use std::os::windows::ffi::OsStrExt;
use windows_core::*;

pub use bitmap::Bitmap;
pub use color::ColorF;
#[cfg(feature = "reactor")]
pub use composition::CompositionDrawTarget;
pub(crate) use device::D2dLock;
pub use device::GpuDevice;
pub use effect::Effect;
pub use geometry::*;
#[cfg(feature = "reactor")]
pub use reactor::{
    CreateReason, DeviceSource, DpiRounding, DrawContext, FrameTiming, PumpHold, ResourceCx,
    ResourcePainterBuilder, Step, SurfacePainter, SurfacePainterBuilder, animated_canvas,
    surface_image, surface_painter, virtual_surface_image,
};
pub use session::DrawingSession;
#[cfg(feature = "reactor")]
pub use surface_image::SurfaceImage;
pub use swap_chain::{SwapChain, WaitObject};
pub use text::*;
pub use types::*;
#[cfg(feature = "reactor")]
pub use virtual_surface_image::VirtualSurfaceImage;

pub use windows_core::Result;
pub use windows_numerics::{Matrix3x2, Vector2};
