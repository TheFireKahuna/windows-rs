#![doc = include_str!("../readme.md")]

// The bindings filter carries interfaces no module here calls — the colour-glyph
// enumerator and the glyph-run analyser — so the whole generated module allows
// `dead_code`.
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

// Drawing a run needs a target, which comes from `windows-d2d`. Naming a font, shaping,
// measuring and carrying a run as plain data need no drawing stack and stay ungated.
#[cfg(feature = "d2d")]
mod glyph;

// Re-exported crate-wide so each module reaches the generated types through
// `use super::*;`. None of it is public: a font face leaves this crate as
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
