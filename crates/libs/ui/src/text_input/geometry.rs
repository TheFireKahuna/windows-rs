//! The immutable, shaped geometry the app publishes. No DirectWrite object crosses.

use super::{Affinity, Selection};
use core::ops::Range;
use std::sync::Arc;
use windows_text::Rect;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Cluster {
    pub start: u32,
    pub end: u32,
    pub rect: Rect,
    pub leading: f32,
    pub trailing: f32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Geometry {
    pub revision: u64,
    pub layout_revision: u64,
    pub clusters: Arc<[Cluster]>,
    pub end: Rect,
    /// Text origin relative to the control, including horizontal reveal.
    pub origin: windows_numerics::Vector2,
    pub viewport: Rect,
}

impl Geometry {
    /// Returns the zero-width caret rect for `selection`, resolved by affinity.
    pub fn caret(&self, selection: Selection) -> Rect {
        let at = selection.caret;
        let upstream = selection.affinity == Affinity::Upstream && at > 0;
        let cluster = self.clusters.iter().find(|c| match upstream {
            true => c.start < at && at <= c.end,
            false => c.start <= at && at < c.end,
        });
        cluster.map_or(self.end, |c| Rect {
            x: if upstream { c.trailing } else { c.leading },
            w: 0.0,
            ..c.rect
        })
    }

    /// Returns the ACP offset and affinity nearest `x`, in text-relative DIPs.
    ///
    /// Every cluster's leading and trailing edge is measured rather than the run scanned in
    /// order: a bidirectional run puts cluster rectangles out of x order, so the nearest edge
    /// and the first edge past `x` are different answers.
    pub fn hit(&self, x: f32) -> (u32, Affinity) {
        let mut nearest = (self.end.x - x).abs();
        let mut at = (
            self.clusters.last().map_or(0, |c| c.end),
            Affinity::Upstream,
        );
        for c in self.clusters.iter() {
            for (edge, offset, affinity) in [
                (c.leading, c.start, Affinity::Downstream),
                (c.trailing, c.end, Affinity::Upstream),
            ] {
                if (edge - x).abs() < nearest {
                    (nearest, at) = ((edge - x).abs(), (offset, affinity));
                }
            }
        }
        at
    }

    /// Returns the visible cluster boundary before `at`.
    ///
    /// Zero-width clusters are skipped, so one arrow key crosses a combining mark or a
    /// joiner and lands where the caret can be seen to move.
    pub fn previous(&self, at: u32) -> u32 {
        self.clusters
            .iter()
            .rev()
            .find(|c| c.start < at && c.rect.w > 0.0)
            .map_or(0, |c| c.start)
    }

    /// Returns the visible cluster boundary after `at`.
    ///
    /// Zero-width clusters are skipped for the same reason [`Self::previous`] skips them.
    pub fn next(&self, at: u32) -> u32 {
        self.clusters
            .iter()
            .find(|c| c.end > at && c.rect.w > 0.0)
            .map_or_else(|| self.clusters.last().map_or(at, |c| c.end), |c| c.end)
    }

    /// Appends the highlight rectangles covering `range`, merging adjacent clusters.
    pub fn rects(&self, range: Range<u32>, out: &mut Vec<Rect>) {
        out.clear();
        let covered = self
            .clusters
            .iter()
            .filter(|c| c.start < range.end && range.start < c.end);
        for c in covered {
            match out.last_mut() {
                Some(r) if r.x + r.w >= c.rect.x && r.x <= c.rect.x + c.rect.w => {
                    let right = (r.x + r.w).max(c.rect.x + c.rect.w);
                    r.x = r.x.min(c.rect.x);
                    r.w = right - r.x;
                }
                _ => out.push(c.rect),
            }
        }
    }
}
