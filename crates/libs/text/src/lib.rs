#![doc = include_str!("../readme.md")]

// `dead_code` is `allow` and not `expect` for one standing reason: the shaping half of
// this crate — layouts, the glyph-run walker, the coverage rasterizer — is filtered in and
// still unwritten, so the warning would report every entry waiting for it. Narrow this to
// `expect` the moment shaping lands, because after that an unconsumed entry is a filter
// entry with no consumer and that is exactly what the warning is for.
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
mod span;

// Drawing a run needs a target to draw into, which is the one thing this crate cannot
// supply for itself. Everything above it — naming a font, resolving a face, carrying a run
// as plain data — is independent of any drawing stack and is not gated.
#[cfg(feature = "d2d")]
mod glyph;

// Re-exported crate-wide so every module can `use super::*;` rather than naming the
// generated types it touches. None of it is public: a font face leaves this crate as
// `&impl Interface`, and a run leaves it as glyph indices, advances and offsets.
pub(crate) use bindings::*;
pub(crate) use windows_core::Interface;

pub use font::{FamilyId, FontFace, FontLadder, FontSpec, FontStretch, FontStyle, TextEngine};
pub use span::{SegBuffers, Span, Spans};

#[cfg(feature = "d2d")]
pub use glyph::{GlyphDraw, GlyphSeg};

pub use windows_core::Result;
pub use windows_numerics::Vector2;
