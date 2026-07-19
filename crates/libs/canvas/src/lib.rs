#![doc = include_str!("../readme.md")]

//! `windows-canvas` is a thin wrapper over [`windows-canvas-core`](windows_canvas_core)
//! (the reactor-free drawing primitives — re-exported below, so every
//! `windows_canvas::*` path is unchanged) plus the reactor-coupled helpers
//! gated behind the `reactor` feature.

// Re-export the entire reactor-free drawing core. Keeps `windows_canvas::GpuDevice`,
// `windows_canvas::DrawingSession`, `windows_canvas::ColorF`, `windows_canvas::Rect`,
// text, geometry, etc. working exactly as before.
pub use windows_canvas_core::*;

// The reactor-coupled module below was written against the old single-crate
// layout, reaching a handful of names through `use super::*`. Re-establish them
// at the crate root so it compiles unchanged.
#[cfg(feature = "reactor")]
use std::cell::Cell;
#[cfg(feature = "reactor")]
#[allow(unused_imports)]
use windows_core::*;

#[cfg(feature = "reactor")]
mod reactor;

#[cfg(feature = "reactor")]
pub use reactor::{CanvasImageSource, DrawContext, animated_canvas};
