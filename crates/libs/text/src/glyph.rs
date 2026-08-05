//! Puts a shaped run's coverage on a surface (feature `d2d`).
//!
//! Drawing is one call per segment. What to draw — shaping, fallback, wrapping — is decided
//! before the segments arrive here as [`GlyphSeg`]s over a [`SegBuffers`]; where to draw
//! them belongs to whoever owns the surface.
//!
//! The trait lives in this crate because a segment names a DirectWrite face. `windows-d2d`
//! takes that face as an opaque interface, so neither crate depends on the other's types.

use super::*;
use windows_d2d::{Brush, Draw, GlyphRun};

/// Draws shaped glyph segments.
///
/// Implemented for `windows-d2d`'s [`Draw`], so a caller holding a bound target and a run
/// names one trait to join them.
pub trait GlyphDraw {
    /// Draws one segment, with `at` the top-left of the tile its origin is relative to.
    ///
    /// **The baseline is not snapped here.** `DrawGlyphRun` takes no options parameter, so
    /// the baseline snapping the text-layout APIs perform is unavailable and glyphs land
    /// exactly where they are put: a baseline half a pixel off looks soft and nothing
    /// reports it. Snapping `at` is the caller's.
    ///
    /// A segment whose face does not resolve draws nothing.
    fn segment(
        &self,
        at: Vector2,
        seg: &GlyphSeg,
        buffers: &SegBuffers,
        engine: &TextEngine,
        brush: &impl Brush,
    );

    /// Draws every segment of `segs` against the same tile top-left `at`.
    fn line(
        &self,
        at: Vector2,
        segs: &[GlyphSeg],
        buffers: &SegBuffers,
        engine: &TextEngine,
        brush: &impl Brush,
    ) {
        for seg in segs {
            self.segment(at, seg, buffers, engine, brush);
        }
    }
}

impl GlyphDraw for Draw<'_> {
    fn segment(
        &self,
        at: Vector2,
        seg: &GlyphSeg,
        buffers: &SegBuffers,
        engine: &TextEngine,
        brush: &impl Brush,
    ) {
        // A face that does not resolve draws nothing rather than failing the frame, so the
        // rest of the line still renders.
        let Ok(face) = engine.face(seg.face) else {
            return;
        };
        let Ok(face) = face.as_interface().cast::<windows_core::IUnknown>() else {
            return;
        };

        self.glyphs(
            Vector2 {
                x: at.x + seg.origin.x,
                y: at.y + seg.origin.y,
            },
            &GlyphRun {
                face: &face,
                em: seg.em,
                glyphs: seg.glyphs.of(&buffers.glyphs),
                advances: seg.advances.of(&buffers.advances),
                offsets: seg.offsets.of(&buffers.offsets),
                bidi: seg.bidi,
            },
            brush,
        );
    }
}
