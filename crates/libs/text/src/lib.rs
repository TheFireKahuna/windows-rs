#![doc = include_str!("../readme.md")]

// `dead_code` is `allow` and not `expect` for one standing reason: the filter carries
// entries with no consumer in this crate — the colour-glyph enumerator, which needs a
// sprite of its own before it can mean anything, and the glyph-run analyser, which exists
// so a test can read back what the rasterizer produced. Narrow this the moment either
// grows one.
#[allow(dead_code)]
#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments
)]
mod bindings;

mod font;
mod format;
mod harvest;
mod hit;
mod shape;
mod span;

// Drawing a run needs a target to draw into, which is the one thing this crate cannot
// supply for itself. Everything above it — naming a font, shaping, measuring, carrying a
// run as plain data — is independent of any drawing stack and is not gated.
#[cfg(feature = "d2d")]
mod glyph;

// Re-exported crate-wide so every module can `use super::*;` rather than naming the
// generated types it touches. None of it is public: a font face leaves this crate as
// `&impl Interface`, and a run leaves it as glyph indices, advances and offsets.
pub(crate) use bindings::*;
pub(crate) use core::cell::RefCell;
pub(crate) use font::wide;
pub(crate) use format::FormatKey;
pub(crate) use harvest::{Collector, Harvest};
pub(crate) use windows_core::Interface;

pub use font::{
    FaceId, FaceKey, FamilyId, FontFace, FontFeatures, FontLadder, FontSpec, FontStretch,
    FontStyle, TextEngine,
};
pub use format::Flow;
pub use harvest::{Decoration, DecorationKind};
pub use hit::{Rect, TextHit};
pub use shape::{Ink, LineMetrics, ShapedRun};
pub use span::{GlyphSeg, SegBuffers, Span, Spans};

#[cfg(feature = "d2d")]
pub use glyph::GlyphDraw;

pub use windows_core::Result;
pub use windows_numerics::Vector2;
