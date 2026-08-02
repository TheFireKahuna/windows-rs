//! Putting a shaped run's coverage on a surface (feature `d2d`).
//!
//! This is the whole of the drawing side, and it is one call per segment. Everything that
//! decides *what* to draw — shaping, fallback, wrapping — happened on another thread and
//! reached here as [`GlyphSeg`]s over a [`SegBuffers`]; everything that decides *where* to
//! draw it belongs to whoever owns the surface.
//!
//! It lives here rather than in the drawing crate because a run names a face, and a face
//! is DirectWrite's. The drawing crate takes one as an opaque interface and never learns
//! what it is, so neither crate grows a dependency on the other's subject.

use super::*;
use windows_d2d::{Brush, Draw, GlyphRun};

/// Draws shaped glyph segments.
///
/// Implemented for `windows-d2d`'s [`Draw`], so a caller that has a bound target and a run
/// has everything it needs and names one trait to join them.
pub trait GlyphDraw {
    /// Draws one segment, with `at` the top-left of the tile its origin is relative to.
    ///
    /// **The baseline is not snapped here.** `DrawGlyphRun` takes no options parameter, so
    /// the free baseline snapping the text-layout APIs perform is unavailable and glyphs
    /// land exactly where they are put: a baseline half a pixel off looks soft and nothing
    /// reports it. Snapping `at` is the caller's, and it is one call.
    fn segment(
        &self,
        at: Vector2,
        seg: &GlyphSeg,
        buffers: &SegBuffers,
        engine: &TextEngine,
        brush: &impl Brush,
    );

    /// Draws a whole line's segments.
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
        // A face the system does not have is a run that does not draw. It is not worth
        // failing the frame for: the rest of the line still renders, and whoever shaped it
        // already learned about it when the id failed to intern.
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
