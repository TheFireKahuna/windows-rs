//! Carries a shaped run across a thread boundary as plain data.
//!
//! Shaping and rasterizing happen on different threads, so what travels between them is
//! glyph indices, advances and offsets in typed buffers, addressed by `(offset, count)`.
//! Nothing thread-affine can enter, because these types have nowhere to put it.

use crate::FaceId;
use windows_numerics::Vector2;

/// Addresses an `(offset, count)` window into one of a [`SegBuffers`]'s buffers.
///
/// Variable-length payloads travel as spans rather than as owned vectors, so the segment
/// carrying them stays `Copy` and the buffers stay poolable: one allocation per payload
/// *kind* for the life of the process, rather than one per item.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    off: u32,
    len: u32,
}

impl Span {
    /// Covers no items, at offset zero. [`of`](Self::of) yields `&[]` for it against any
    /// buffer, including one nothing has been appended to.
    pub const EMPTY: Self = Self { off: 0, len: 0 };

    /// Returns the span covering `len` items appended at `off`.
    #[must_use]
    pub const fn new(off: u32, len: u32) -> Self {
        Self { off, len }
    }

    /// Returns the number of items it covers.
    #[must_use]
    pub const fn len(self) -> usize {
        self.len as usize
    }

    /// Returns whether it covers no items.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Returns the items of `buffer` this span covers, or `&[]` if it runs past the end.
    ///
    /// One bounds check per read, at the seam. A span read against a buffer it does not
    /// fit yields nothing, so a mismatched pair presents as a missing run rather than a
    /// panic in a draw call.
    #[must_use]
    pub fn of<T>(self, buffer: &[T]) -> &[T] {
        let (off, len) = (self.off as usize, self.len as usize);
        buffer.get(off..off + len).unwrap_or_default()
    }
}

/// Holds the typed side-buffers a shaped run's segments are appended to.
///
/// One buffer per element type: a segment, a glyph index, an advance and an offset are
/// four different types, so nothing here is read through a reinterpretation.
#[derive(Debug, Default)]
pub struct SegBuffers {
    /// The segments themselves, addressed by the [`Span`] a line's append returned.
    pub segs: Vec<GlyphSeg>,
    /// Glyph indices, into the face named by the segment that spans them.
    pub glyphs: Vec<u16>,
    /// Advance per glyph, in DIPs.
    pub advances: Vec<f32>,
    /// Displacement per glyph, in DIPs: `[along the baseline, up from it]`.
    pub offsets: Vec<[f32; 2]>,
}

impl SegBuffers {
    /// Empties every buffer, keeping the allocations.
    ///
    /// A consumer drains and clears; the producer then appends into capacity that already
    /// exists.
    pub fn clear(&mut self) {
        self.segs.clear();
        self.glyphs.clear();
        self.advances.clear();
        self.offsets.clear();
    }

    /// Appends one segment's glyph data and returns the three spans naming it.
    ///
    /// `glyphs`, `advances` and `offsets` must be the same length: they are three views of
    /// one sequence. A mismatch appends the shortest of the three, losing a trailing glyph
    /// rather than reading past a buffer, and asserts in a debug build.
    pub fn push(&mut self, glyphs: &[u16], advances: &[f32], offsets: &[[f32; 2]]) -> Spans {
        debug_assert!(
            glyphs.len() == advances.len() && glyphs.len() == offsets.len(),
            "a segment's indices, advances and offsets are three views of one sequence"
        );
        let n = glyphs.len().min(advances.len()).min(offsets.len());
        fn at<T>(v: &[T]) -> u32 {
            u32::try_from(v.len()).unwrap_or(u32::MAX)
        }
        let n = n as u32;
        let spans = Spans {
            glyphs: Span::new(at(&self.glyphs), n),
            advances: Span::new(at(&self.advances), n),
            offsets: Span::new(at(&self.offsets), n),
        };
        let n = n as usize;
        self.glyphs.extend_from_slice(&glyphs[..n]);
        self.advances.extend_from_slice(&advances[..n]);
        self.offsets.extend_from_slice(&offsets[..n]);
        spans
    }
}

/// Names the three spans one appended segment occupies.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Spans {
    pub glyphs: Span,
    pub advances: Span,
    pub offsets: Span,
}

/// Covers one maximal span of a shaped line drawn from a single face.
///
/// A line is a *list* of these, because font fallback splits it: a label mixing Latin and
/// CJK resolves to two faces, and a segment that could name only one would leave the
/// second unrendered rather than report an error.
///
/// It names a [`FaceId`](crate::FaceId) and not a family, because the face fallback chose
/// is not one the requested family names, and a glyph index is an index into a face.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GlyphSeg {
    /// The face every glyph index here is an index into.
    pub face: FaceId,
    /// Em size in DIPs, as shaped.
    pub em: f32,
    /// Bidi embedding level; odd means the segment advances leftward from `origin`.
    pub bidi: u32,
    /// Baseline origin relative to the tile's top-left, in DIPs. Carried rather than
    /// folded from advances, so a bidi line, where visual order and advance order
    /// disagree, is placed by the same rule as any other.
    pub origin: Vector2,
    /// Glyph indices, in the buffers this segment was appended to.
    pub glyphs: Span,
    /// Advance per glyph, in DIPs.
    pub advances: Span,
    /// Displacement per glyph, in DIPs.
    pub offsets: Span,
}
