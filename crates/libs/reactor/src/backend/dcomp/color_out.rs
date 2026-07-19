//! App-registered **output colour transform** — the seam through which an app
//! maps its authored colours to the current display.
//!
//! The dcomp backend composites to a linear-scRGB FP16 surface, and every chrome
//! colour is written through [`node::linear`](super::node::linear). An app that
//! authors in an absolute HDR luminance domain needs those colours *tonemapped* to
//! whatever display it is on (track the SDR-white level, roll highlights that
//! exceed the panel). This module lets the app register that map once; the backend
//! applies it at the draw choke, in linear light, on every painted colour.
//!
//! Why here and not a Composition effect: the transform runs inside the D2D draw
//! session, which is genuine linear-light scRGB, so a multiply/roll is hue-safe. A
//! post-composite `Exposure` effect multiplies in a non-linear space and rotates
//! hue at large factors. This seam is also gap-free by construction — one function,
//! every colour — where a per-surface effect has coverage to get right.
//!
//! The backend stays policy-free: it knows nothing of luminance, gamut, or display
//! modes. It receives a linear `[r, g, b, a]` and returns whatever the app's
//! transform yields. Register **before** the window is created (like the other
//! display hooks); only the first registration is kept. The transform must be cheap
//! and non-blocking — it runs on the UI thread per painted colour.

use std::sync::OnceLock;

/// A linear-scRGB colour transform: `[r, g, b, a] -> [r, g, b, a]`.
type ColorTransform = Box<dyn Fn([f32; 4]) -> [f32; 4] + Send + Sync + 'static>;

/// The app-registered transform. Unset = identity (every colour passes through),
/// so the backend is correct with no app opt-in.
static TRANSFORM: OnceLock<ColorTransform> = OnceLock::new();

/// Register the process-global output colour transform, applied to every chrome
/// colour the dcomp backend paints. Call **before** the window is created; only the
/// first registration is kept.
///
/// The transform receives and returns **linear scRGB** `[r, g, b, a]`. It runs on
/// the UI thread once per painted colour — keep it allocation-free and cheap. It
/// may read live per-display state the app updates elsewhere (so a display change
/// need not re-register anything); this seam only decides *where* the map is
/// applied, never *what* it is.
pub fn set_output_color_transform(
    transform: impl Fn([f32; 4]) -> [f32; 4] + Send + Sync + 'static,
) {
    let _ = TRANSFORM.set(Box::new(transform));
}

/// Apply the registered transform to a linear colour, or pass it through unchanged
/// when no app opted in.
#[inline]
pub(crate) fn apply(rgba: [f32; 4]) -> [f32; 4] {
    TRANSFORM.get().map_or(rgba, |f| f(rgba))
}
