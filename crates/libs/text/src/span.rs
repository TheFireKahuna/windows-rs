//! The buffers a shaped run crosses a thread boundary in.
//!
//! Shaping and rasterizing happen on different threads, so what travels between them is a
//! run in plain data: glyph indices, advances and offsets, in typed buffers, addressed by
//! `(offset, count)`. Nothing thread-affine can enter, because there is nowhere in these
//! types for it to go.

use crate::FaceId;
use windows_numerics::Vector2;

/// A `(offset, count)` window into one of a [`SegBuffers`] buffer.
///
/// Variable-length payloads travel as spans rather than as owned vectors so the thing that
/// carries them stays `Copy` and the buffers stay poolable — one allocation per payload
/// *kind* for the life of the process, rather than one per item.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    off: u32,
    len: u32,
}

impl Span {
    /// An empty span. Distinct from a span at offset zero of length zero only in that
    /// nothing has to have been appended for it to be valid.
    pub const EMPTY: Self = Self { off: 0, len: 0 };

    /// The span covering `len` items appended at `off`.
    #[must_use]
    pub const fn new(off: u32, len: u32) -> Self {
        Self { off, len }
    }

    /// How many items it covers.
    #[must_use]
    pub const fn len(self) -> usize {
        self.len as usize
    }

    /// Whether it covers nothing.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// The items of `buffer` this span covers, or `&[]` if it runs past the end.
    ///
    /// One bounds check, at the seam, once — rather than an unchecked index per read or a
    /// `Result` at every call site. A span that does not fit its buffer is a bug in
    /// whoever built the pair, and reading nothing is how it presents: a missing run, not
    /// a panic in a draw call.
    #[must_use]
    pub fn of<T>(self, buffer: &[T]) -> &[T] {
        let (off, len) = (self.off as usize, self.len as usize);
        buffer.get(off..off + len).unwrap_or_default()
    }
}

/// The typed side-buffers a shaped run's segments are appended to.
///
/// Four buffers and not one: a segment, a glyph index, an advance and an offset are four
/// different types, and packing them into a shared `f32` buffer would trade a real type
/// for a reinterpretation at every read.
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
    /// This is the whole of the pooling: a consumer drains and clears, and the producer
    /// appends into capacity it already has.
    pub fn clear(&mut self) {
        self.segs.clear();
        self.glyphs.clear();
        self.advances.clear();
        self.offsets.clear();
    }

    /// Appends one segment's glyph data and returns the three spans naming it.
    ///
    /// The three slices must be the same length — they are three views of one sequence —
    /// and the shortest wins if they are not, so a mismatch loses a trailing glyph rather
    /// than reading past a buffer.
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

/// The three spans one appended segment occupies.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Spans {
    pub glyphs: Span,
    pub advances: Span,
    pub offsets: Span,
}

/// One maximal span of a shaped line drawn from a single face.
///
/// A line is a *list* of these and not one, because font fallback splits it: a label
/// mixing Latin and CJK resolves to two faces, and a run that could only name one would
/// simply fail to render the second — which is the failure mode a single-segment seam
/// degrades into rather than an error it reports.
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
    /// folded from advances, so a bidi line — where visual order and advance order
    /// disagree — needs no second rule.
    pub origin: Vector2,
    /// Glyph indices, in the buffers this segment was appended to.
    pub glyphs: Span,
    /// Advance per glyph, in DIPs.
    pub advances: Span,
    /// Displacement per glyph, in DIPs.
    pub offsets: Span,
}
