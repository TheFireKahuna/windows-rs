#![doc = include_str!("../readme.md")]

//! `windows-canvas` is a thin wrapper over [`windows-canvas-core`](windows_canvas_core)
//! (the reactor-free drawing primitives — re-exported below, so every
//! `windows_canvas::*` path is unchanged) plus the reactor-coupled helpers
//! (`surface_painter`, `animated_canvas`, `surface_image`, composition surfaces)
//! gated behind the `reactor` feature.

// Re-export the entire reactor-free drawing core. Keeps `windows_canvas::GpuDevice`,
// `windows_canvas::DrawingSession`, `windows_canvas::ColorF`, `windows_canvas::Rect`,
// text, geometry, etc. working exactly as before.
pub use windows_canvas_core::*;

// The reactor-coupled modules below were written against the old single-crate
// layout (`use super::*` reaching the private D2D bindings and a handful of core
// types). Re-establish those names at the crate root so the modules compile
// unchanged: the raw D2D/DWrite vtables via the core's (doc-hidden) `bindings`,
// plus the std/core imports they relied on.
#[cfg(feature = "reactor")]
#[allow(
    unused_imports,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments,
    clippy::missing_transmute_annotations
)]
use windows_canvas_core::bindings::*;
#[cfg(feature = "reactor")]
use std::cell::Cell;
#[cfg(feature = "reactor")]
#[allow(unused_imports)]
use windows_core::*;

#[cfg(feature = "reactor")]
mod composition;
#[cfg(feature = "reactor")]
mod reactor;
#[cfg(feature = "reactor")]
mod surface_image;
#[cfg(feature = "reactor")]
mod virtual_surface_image;

#[cfg(feature = "reactor")]
pub use composition::CompositionDrawTarget;
#[cfg(feature = "reactor")]
pub use reactor::{
    CreateReason, DeviceSource, DpiRounding, DrawContext, FrameTiming, PumpHold, ResourceCx,
    ResourcePainterBuilder, Step, SurfacePainter, SurfacePainterBuilder, animated_canvas,
    invalidate_all_painters,
    surface_image, surface_painter, virtual_surface_image,
};
#[cfg(feature = "reactor")]
pub use surface_image::SurfaceImage;
#[cfg(feature = "reactor")]
pub use virtual_surface_image::VirtualSurfaceImage;
