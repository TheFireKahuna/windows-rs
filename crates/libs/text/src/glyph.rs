//! Putting a shaped run's coverage on a surface (feature `d2d`).
//!
//! This is the whole of the drawing side, and it is one call. Everything that decides
//! *what* to draw — shaping, fallback, wrapping — happened on another thread and reached
//! here as [`GlyphSeg`]s over a [`SegBuffers`]; everything that decides *where* to draw it
//! belongs to whoever owns the surface.
//!
//! It lives here rather than in the drawing crate because a run names a face, and a face
//! is DirectWrite's. The drawing crate takes one as an opaque interface and never learns
//! what it is, so neither crate grows a dependency on the other's subject.

use super::*;
use windows_d2d::{Brush, Draw, GlyphRun};

/// One maximal span of a shaped line drawn from a single face.
///
/// A line is a *list* of these and not one, because font fallback splits it: a label
/// mixing Latin and CJK resolves to two faces, and a run that could only name one would
/// simply fail to render the second — which is the failure mode a single-segment seam
/// degrades into rather than an error it reports.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GlyphSeg {
    /// The face and size every glyph in this segment is drawn from.
    pub spec: FontSpec,
    /// Glyph indices, in the buffers this segment was appended to.
    pub glyphs: Span,
    /// Advance per glyph, in DIPs.
    pub advances: Span,
    /// Displacement per glyph, in DIPs.
    pub offsets: Span,
}

impl GlyphSeg {
    /// A segment naming `spec` over the spans `push` returned.
    #[must_use]
    pub const fn new(spec: FontSpec, spans: Spans) -> Self {
        Self {
            spec,
            glyphs: spans.glyphs,
            advances: spans.advances,
            offsets: spans.offsets,
        }
    }
}

/// Draws shaped glyph segments.
///
/// This trait is implemented for `windows-d2d`'s [`Draw`], so a caller that has a bound
/// target and a run has everything it needs and names one trait to join them.
pub trait GlyphDraw {
    /// Draws one segment with its baseline starting at `origin`, and returns the pen
    /// position after it — so a line of several fallback segments is a fold rather than a
    /// second pass over the advances.
    ///
    /// **`origin.y` is not snapped here.** `DrawGlyphRun` takes no options parameter, so
    /// the free baseline snapping the text-layout APIs perform is unavailable, and glyphs
    /// land exactly where they are put: a baseline half a pixel off looks soft and nothing
    /// reports it. Snapping is the caller's, and it is one call.
    fn segment(
        &self,
        origin: Vector2,
        seg: &GlyphSeg,
        buffers: &SegBuffers,
        engine: &TextEngine,
        brush: &impl Brush,
    ) -> Vector2;

    /// Draws a whole line of segments left to right from `origin`.
    fn line(
        &self,
        origin: Vector2,
        segs: &[GlyphSeg],
        buffers: &SegBuffers,
        engine: &TextEngine,
        brush: &impl Brush,
    ) -> Vector2 {
        segs.iter().fold(origin, |pen, seg| {
            self.segment(pen, seg, buffers, engine, brush)
        })
    }
}

impl GlyphDraw for Draw<'_> {
    fn segment(
        &self,
        origin: Vector2,
        seg: &GlyphSeg,
        buffers: &SegBuffers,
        engine: &TextEngine,
        brush: &impl Brush,
    ) -> Vector2 {
        let advances = seg.advances.of(&buffers.advances);
        let width: f32 = advances.iter().sum();
        let pen = Vector2 {
            x: origin.x + width,
            y: origin.y,
        };

        // A face the system does not have is a run that does not draw. It is not worth
        // failing the frame for: the rest of the line still renders, and the caller
        // already learned about it when it resolved the `FontSpec` for measurement.
        let Ok(face) = engine.face(seg.spec) else {
            return pen;
        };
        let Ok(face) = face.as_interface().cast::<windows_core::IUnknown>() else {
            return pen;
        };

        self.glyphs(
            origin,
            &GlyphRun {
                face: &face,
                em: seg.spec.size,
                glyphs: seg.glyphs.of(&buffers.glyphs),
                advances,
                offsets: seg.offsets.of(&buffers.offsets),
            },
            brush,
        );
        pen
    }
}
