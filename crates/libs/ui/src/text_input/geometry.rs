//! The immutable, shaped geometry the app publishes. No DirectWrite object crosses.

use super::{Affinity, Selection};
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
    pub clusters: std::sync::Arc<[Cluster]>,
    pub end: Rect,
    /// Text origin relative to the control, including horizontal reveal.
    pub origin: windows_numerics::Vector2,
    pub viewport: Rect,
}

impl Geometry {
    pub fn caret(&self, selection: Selection) -> Rect {
        let at = selection.caret;
        let cluster = match selection.affinity {
            Affinity::Upstream if at > 0 => {
                self.clusters.iter().find(|c| c.start < at && at <= c.end)
            }
            _ => self.clusters.iter().find(|c| c.start <= at && at < c.end),
        };
        cluster.map_or(self.end, |c| Rect {
            x: if selection.affinity == Affinity::Upstream {
                c.trailing
            } else {
                c.leading
            },
            w: 0.0,
            ..c.rect
        })
    }

    pub fn hit(&self, x: f32) -> (u32, Affinity) {
        let mut nearest = (self.end.x - x).abs();
        let mut result = (
            self.clusters.last().map_or(0, |c| c.end),
            Affinity::Upstream,
        );
        for c in self.clusters.iter() {
            for (edge, at, affinity) in [
                (c.leading, c.start, Affinity::Downstream),
                (c.trailing, c.end, Affinity::Upstream),
            ] {
                let distance = (edge - x).abs();
                if distance < nearest {
                    nearest = distance;
                    result = (at, affinity);
                }
            }
        }
        result
    }

    pub fn previous(&self, at: u32) -> u32 {
        let end = self.clusters.partition_point(|c| c.start < at);
        self.clusters[..end]
            .iter()
            .rev()
            .find(|c| c.rect.w > 0.0)
            .map_or(0, |c| c.start)
    }

    pub fn next(&self, at: u32) -> u32 {
        let start = self.clusters.partition_point(|c| c.end <= at);
        self.clusters[start..]
            .iter()
            .find(|c| c.rect.w > 0.0)
            .map_or_else(|| self.clusters.last().map_or(0, |c| c.end), |c| c.end)
    }

    pub fn rects(&self, range: core::ops::Range<u32>, out: &mut Vec<Rect>) {
        out.clear();
        for c in self.clusters.iter() {
            if c.start < range.end && range.start < c.end {
                if let Some(last) = out.last_mut()
                    && last.y == c.rect.y
                    && last.h == c.rect.h
                    && c.rect.x <= last.x + last.w
                    && last.x <= c.rect.x + c.rect.w
                {
                    let right = (last.x + last.w).max(c.rect.x + c.rect.w);
                    last.x = last.x.min(c.rect.x);
                    last.w = right - last.x;
                    continue;
                }
                out.push(c.rect);
            }
        }
    }
}
