//! Cluster geometry: point to position, position to caret, range to rectangles.
//!
//! All three are `IDWriteTextLayout`'s, and none of them is re-derived from the harvested
//! glyphs. A cluster is not a code unit — a surrogate pair, a combining sequence and a
//! ligature are each one indivisible caret stop spanning several — and across a direction
//! boundary the two edges of a position are different points on opposite sides of a word.
//! Arithmetic over advances gets both wrong in ways that only reproduce in the text that
//! has them.

use super::*;
use core::ops::Range;

/// Where a point landed.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct TextHit {
    /// Code-unit index of the cluster under the point.
    pub position: u32,
    /// Code units in that cluster — what an arrow key steps by.
    pub length: u32,
    /// Whether the caret belongs after `position` rather than before it.
    pub trailing: bool,
    /// Whether the point was inside the text at all, rather than past its end.
    pub inside: bool,
    /// The cluster's box, in layout space.
    pub rect: Rect,
    /// Bidi embedding level; **odd means right-to-left**, and is what says whether the
    /// caret's affinity is observable here at all.
    pub bidi: u32,
}

impl TextHit {
    #[must_use]
    pub const fn is_rtl(&self) -> bool {
        self.bidi % 2 == 1
    }

    /// The position the caret goes to, which is past the cluster on a trailing hit.
    #[must_use]
    pub const fn caret(&self) -> u32 {
        if self.trailing {
            self.position + self.length
        } else {
            self.position
        }
    }
}

/// An axis-aligned box in layout space, DIPs.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl From<DWRITE_HIT_TEST_METRICS> for Rect {
    fn from(m: DWRITE_HIT_TEST_METRICS) -> Self {
        Self {
            x: m.left,
            y: m.top,
            w: m.width,
            h: m.height,
        }
    }
}

impl ShapedRun {
    /// The cluster at `p`, in layout space.
    #[must_use]
    pub fn hit_test(&self, p: Vector2) -> TextHit {
        let mut trailing = windows_core::BOOL(0);
        let mut inside = windows_core::BOOL(0);
        let mut m = DWRITE_HIT_TEST_METRICS::default();
        // SAFETY: every out-parameter is a stack local outliving the call.
        if unsafe {
            self.layout()
                .HitTestPoint(p.x, p.y, &mut trailing, &mut inside, &mut m)
        }
        .is_err()
        {
            return TextHit::default();
        }
        TextHit {
            position: m.textPosition,
            length: m.length,
            trailing: trailing.as_bool(),
            inside: inside.as_bool(),
            rect: m.into(),
            bidi: m.bidiLevel,
        }
    }

    /// Where the caret sits at `position`.
    ///
    /// `after` selects which **edge of the character at `position`** is wanted: `false` its
    /// leading edge, `true` its trailing. In left-to-right text `caret(i, false)` and
    /// `caret(i - 1, true)` name the same point, which is why a caret can get away with
    /// only ever asking for the first. Across a direction boundary they are two points on
    /// opposite sides of a word, and choosing between them is the caret's affinity.
    #[must_use]
    pub fn caret(&self, position: u32, after: bool) -> (Vector2, TextHit) {
        let (mut x, mut y) = (0.0f32, 0.0f32);
        let mut m = DWRITE_HIT_TEST_METRICS::default();
        // SAFETY: every out-parameter is a stack local outliving the call.
        if unsafe {
            self.layout()
                .HitTestTextPosition(position, after, &mut x, &mut y, &mut m)
        }
        .is_err()
        {
            return (Vector2::default(), TextHit::default());
        }
        (
            Vector2 { x, y },
            TextHit {
                position: m.textPosition,
                length: m.length,
                trailing: after,
                // Both are answers to *point* hit-testing, and a text position is by
                // construction a position in the text.
                inside: true,
                rect: m.into(),
                bidi: m.bidiLevel,
            },
        )
    }

    /// Appends the boxes covering the code-unit `range`, one per line it spans and one per
    /// direction run within a line.
    ///
    /// Caller-pooled, and the staging buffer is on the stack for anything a person can
    /// select by dragging: this runs on every pointer move of a selection gesture, and a
    /// heap allocation per move is the whole of its cost.
    pub fn cluster_rects(&self, range: Range<u32>, out: &mut Vec<Rect>) {
        /// Lines a drag-selection covers before this spills to the heap.
        const INLINE: usize = 16;

        let length = range.end.saturating_sub(range.start);
        if length == 0 {
            return;
        }
        let mut count = 0u32;
        // The first call reports the count it needs and returns the expected
        // insufficient-buffer error, so the result is deliberately dropped.
        // SAFETY: both calls take a slice this frame owns and a stack-local counter.
        unsafe {
            let _ = self
                .layout()
                .HitTestTextRange(range.start, length, 0.0, 0.0, None, &mut count);
        }
        if count == 0 {
            return;
        }

        let mut stack = [DWRITE_HIT_TEST_METRICS::default(); INLINE];
        let mut heap;
        let raw: &mut [DWRITE_HIT_TEST_METRICS] = if count as usize <= INLINE {
            &mut stack[..count as usize]
        } else {
            heap = vec![DWRITE_HIT_TEST_METRICS::default(); count as usize];
            &mut heap
        };

        // SAFETY: as above; the slice is sized from the count the probe reported.
        if unsafe {
            self.layout()
                .HitTestTextRange(range.start, length, 0.0, 0.0, Some(raw), &mut count)
        }
        .is_err()
        {
            return;
        }
        out.extend(raw.iter().take(count as usize).map(|m| Rect::from(*m)));
    }
}
