//! The hit query. **Front half.**
//!
//! The array is built on the app thread and queried here, because the query reads live
//! scroll offsets and those live with the trackers. It is the **single authority**: pointer
//! routing, wheel routing, gesture targeting, keyboard focus order, the window's own
//! caption hit test and automation's element-from-point all resolve through this, and a
//! presentation region's parts extend the array rather than forking it.
//!
//! Back-to-front, first hit wins. Paint order *is* z-order, so that equals "the last
//! eligible node in a depth-first walk" with no descent and no parent-miss prune to get
//! wrong. A child extending past its parent is still hit, which is correct: a shadow, a
//! focus ring and a popup anchor all do.

use crate::hit_build::{HitEntry, HitFlags, NO_ENTRY};
use crate::sink::{NodeId, Point};
use windows_numerics::Vector2;

/// Which device a contact came from. Only touch and pen inflate a target.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ContactKind {
    #[default]
    Mouse,
    Touch,
    Pen,
    /// A precision touchpad, which reports as a cursor and so hits the drawn rect.
    Touchpad,
}

impl ContactKind {
    /// Whether a target's touch inflation applies.
    #[must_use]
    pub const fn inflates(self) -> bool {
        matches!(self, Self::Touch | Self::Pen)
    }
}

/// What a query resolved to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Hit {
    /// Index into the array. The join to anything a consumer keeps in parallel.
    pub index: u32,
    pub id: crate::hit_build::ControlId,
    pub flags: HitFlags,
    /// The point, in the target's own space — with its scroll ancestry applied.
    pub local: Point,
}

/// The array, plus what a query needs that the array does not carry.
#[derive(Debug, Default)]
pub struct HitTable {
    entries: Vec<HitEntry>,
    /// Bumped on every rebuild, which is what invalidates the memo.
    epoch: u64,
    /// The live offset of each scroll container, from its tracker's last reported value.
    /// A handful of scrolling surfaces, so a linear scan beats a map kept in step.
    scrolls: Vec<(NodeId, Vector2)>,
    /// The last hit, so intra-control motion is one rectangle test. Interior mutability
    /// keeps the hover path on `&self`, which every consumer of the array shares.
    memo: core::cell::Cell<Option<Memo>>,
}

#[derive(Copy, Clone, Debug)]
struct Memo {
    index: u32,
    epoch: u64,
    rect: (f32, f32, f32, f32),
}

impl HitTable {
    /// Replaces the whole table.
    pub fn replace(&mut self, entries: &[HitEntry]) {
        self.entries.clear();
        self.entries.extend_from_slice(entries);
        self.epoch = self.epoch.wrapping_add(1);
        self.memo.set(None);
    }

    /// Records a scroll container's live offset.
    ///
    /// Called from the tracker's values-changed handler, the only trustworthy read of a
    /// tracker: it runs in another process and every call and callback is asynchronous.
    pub fn set_scroll(&mut self, node: NodeId, offset: Vector2) {
        match self.scrolls.iter_mut().find(|(id, _)| *id == node) {
            Some((_, existing)) => *existing = offset,
            None => self.scrolls.push((node, offset)),
        }
        // A scroll moves content under the pointer without the array changing, so the memo
        // has to go even though the epoch does not.
        self.memo.set(None);
    }

    /// Forgets a scroll container.
    pub fn clear_scroll(&mut self, node: NodeId) {
        self.scrolls.retain(|(id, _)| *id != node);
        self.memo.set(None);
    }

    /// How many entries the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries, in z-order. Focus order is this, filtered to what routes input.
    #[must_use]
    pub fn entries(&self) -> &[HitEntry] {
        &self.entries
    }

    /// What is under `p`.
    ///
    /// The memo **bounds the scan; it does not short-circuit it**, and that distinction is
    /// its correctness. "Still inside what it was inside" does not imply the same answer —
    /// a control drawn *above* can have come under the pointer meanwhile, and answering
    /// from the memo would report the thing underneath. What it does imply is that nothing
    /// *below* can win, since the scan is back-to-front and takes the first hit. So it is a
    /// floor, and the skipped tail is where the entries mostly are.
    pub fn hit(&self, p: Point, contact: ContactKind) -> Option<Hit> {
        let floor = match self.memo.get() {
            Some(memo)
                if memo.epoch == self.epoch
                    && p.x >= memo.rect.0
                    && p.y >= memo.rect.1
                    && p.x <= memo.rect.2
                    && p.y <= memo.rect.3 =>
            {
                memo.index as usize
            }
            _ => 0,
        };

        let mut best: Option<(usize, f32, Point)> = None;
        for index in (floor..self.entries.len()).rev() {
            let entry = &self.entries[index];
            if !entry
                .flags
                .intersects(HitFlags::INTERACTIVE | HitFlags::SCROLL)
            {
                continue;
            }
            let q = self.resolve(p, entry.scroll_src);
            let inflate = if contact.inflates() && !entry.flags.contains(HitFlags::NO_INFLATE) {
                entry.touch_inflate
            } else {
                0.0
            };
            if !entry.contains(q, inflate) {
                continue;
            }
            if !self.clip_chain_contains(entry.clip_parent, p) {
                continue;
            }
            // An uninflated hit is exact and wins outright. Only inflated ones compete,
            // nearest centre first, so two neighbours cannot both claim a point once a
            // finger's slack is added to each.
            if entry.contains(q, 0.0) {
                return Some(self.record(index, q));
            }
            // Only inflated candidates compete, and the nearest centre wins. An exact tie
            // goes to the one drawn later, which is what z-order means everywhere else
            // here — and it has to be decided rather than left to scan order, or two
            // targets equidistant from a point answer differently on different frames.
            let distance = entry.centre_distance_sq(q);
            // Strict, and the scan is back-to-front, so an exact tie keeps the candidate
            // found first — the topmost.
            if best.is_none_or(|(_, existing, _)| distance < existing) {
                best = Some((index, distance, q));
            }
        }
        best.map(|(index, _, q)| self.record(index, q))
    }

    /// Walks the clip ancestry, rejecting a point any ancestor excludes.
    ///
    /// A parent-index test and not a control-flow prune: there is no descent to prune, so
    /// overhang survives by construction and only a genuine clip removes anything.
    fn clip_chain_contains(&self, mut parent: u32, p: Point) -> bool {
        let mut guard = self.entries.len();
        while parent != NO_ENTRY {
            let Some(entry) = self.entries.get(parent as usize) else {
                return true;
            };
            let q = self.resolve(p, entry.scroll_src);
            if !entry.contains(q, 0.0) {
                return false;
            }
            parent = entry.clip_parent;
            // A malformed table — a cycle in the parent indices — would otherwise hang the
            // pump. The bound is the array's own length, which no acyclic chain can exceed.
            guard = guard.saturating_sub(1);
            if guard == 0 {
                debug_assert!(false, "the clip chain is cyclic");
                return false;
            }
        }
        true
    }

    /// Applies a scroll ancestry to a window-space point.
    ///
    /// Layout places content **unscrolled** and the compositor applies the offset, so the
    /// query moves the *point* rather than the rects. That is the simplification: the prior
    /// shape had layout write a scroll translation and the walk carry a compensating offset
    /// in a different coordinate space.
    fn resolve(&self, p: Point, scroll: NodeId) -> Point {
        if scroll.is_none() {
            return p;
        }
        match self.scrolls.iter().find(|(id, _)| *id == scroll) {
            Some((_, offset)) => Vector2 {
                x: p.x + offset.x,
                y: p.y + offset.y,
            },
            None => p,
        }
    }

    fn record(&self, index: usize, local: Point) -> Hit {
        let entry = self.entries[index];
        let hit = Hit {
            index: index as u32,
            id: entry.id,
            flags: entry.flags,
            local,
        };
        // Intersected with the clip ancestry, so "inside the memo rect" really does mean
        // "still admitted by every clip above it". Without that, leaving a clipped region
        // while staying inside the entry's own box would keep answering with it.
        self.memo.set(Some(Memo {
            index: index as u32,
            epoch: self.epoch,
            rect: self.clipped_box(index),
        }));
        hit
    }

    /// An entry's box, narrowed by every clip above it.
    fn clipped_box(&self, index: usize) -> (f32, f32, f32, f32) {
        let entry = &self.entries[index];
        let mut box_ = (entry.x0, entry.y0, entry.x1, entry.y1);
        let mut parent = entry.clip_parent;
        let mut guard = self.entries.len();
        while parent != NO_ENTRY && guard > 0 {
            let Some(clip) = self.entries.get(parent as usize) else {
                break;
            };
            box_ = (
                box_.0.max(clip.x0),
                box_.1.max(clip.y0),
                box_.2.min(clip.x1),
                box_.3.min(clip.y1),
            );
            parent = clip.clip_parent;
            guard -= 1;
        }
        box_
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hit_build::ControlId;

    fn entry(id: u64, rect: (f32, f32, f32, f32), flags: HitFlags) -> HitEntry {
        HitEntry {
            x0: rect.0,
            y0: rect.1,
            x1: rect.2,
            y1: rect.3,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            flags,
            scroll_src: NodeId::NONE,
            id: ControlId(id),
        }
    }

    fn at(x: f32, y: f32) -> Point {
        Vector2 { x, y }
    }

    #[test]
    fn the_last_entry_wins_because_paint_order_is_z_order() {
        let mut table = HitTable::default();
        table.replace(&[
            entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE),
            entry(2, (20.0, 20.0, 60.0, 60.0), HitFlags::INTERACTIVE),
        ]);
        assert_eq!(
            table.hit(at(30.0, 30.0), ContactKind::Mouse).unwrap().id.0,
            2
        );
        assert_eq!(
            table.hit(at(90.0, 90.0), ContactKind::Mouse).unwrap().id.0,
            1
        );
    }

    #[test]
    fn overhang_survives_because_there_is_no_descent_to_prune() {
        let mut table = HitTable::default();
        // A focus ring drawn outside its parent's box, with no clip anywhere.
        table.replace(&[
            entry(1, (0.0, 0.0, 50.0, 50.0), HitFlags::INTERACTIVE),
            entry(2, (40.0, 40.0, 90.0, 90.0), HitFlags::INTERACTIVE),
        ]);
        assert_eq!(
            table.hit(at(80.0, 80.0), ContactKind::Mouse).unwrap().id.0,
            2
        );
    }

    #[test]
    fn a_clip_ancestor_removes_what_it_excludes() {
        let mut table = HitTable::default();
        let mut child = entry(2, (40.0, 40.0, 200.0, 200.0), HitFlags::INTERACTIVE);
        child.clip_parent = 0;
        table.replace(&[
            entry(
                1,
                (0.0, 0.0, 100.0, 100.0),
                HitFlags::INTERACTIVE | HitFlags::CLIP,
            ),
            child,
        ]);
        assert_eq!(
            table.hit(at(60.0, 60.0), ContactKind::Mouse).unwrap().id.0,
            2
        );
        // Outside the clip: the child is gone, and so is the clip itself.
        assert!(table.hit(at(150.0, 150.0), ContactKind::Mouse).is_none());
    }

    #[test]
    fn inflation_applies_to_touch_and_pen_and_to_nothing_else() {
        let mut table = HitTable::default();
        let mut small = entry(1, (50.0, 50.0, 60.0, 60.0), HitFlags::INTERACTIVE);
        small.touch_inflate = 10.0;
        table.replace(&[small]);
        assert!(table.hit(at(45.0, 55.0), ContactKind::Mouse).is_none());
        assert!(table.hit(at(45.0, 55.0), ContactKind::Touchpad).is_none());
        assert!(table.hit(at(45.0, 55.0), ContactKind::Touch).is_some());
        assert!(table.hit(at(45.0, 55.0), ContactKind::Pen).is_some());
    }

    #[test]
    fn two_inflated_targets_never_both_claim_a_point() {
        let mut table = HitTable::default();
        // Two 10-DIP targets 6 DIPs apart, each inflated by 8 — so their inflated boxes
        // overlap and the gap between them is claimed by both.
        let mut left = entry(1, (0.0, 0.0, 10.0, 10.0), HitFlags::INTERACTIVE);
        let mut right = entry(2, (16.0, 0.0, 26.0, 10.0), HitFlags::INTERACTIVE);
        left.touch_inflate = 8.0;
        right.touch_inflate = 8.0;
        table.replace(&[left, right]);

        for x in 0..=26 {
            let p = at(x as f32, 5.0);
            let hit = table
                .hit(p, ContactKind::Touch)
                .expect("inside one of them");
            // Nearest centre, and exactly one answer — the property the tie-break exists
            // for. Centres are at 5 and 21, so the boundary is 13.
            // An exact (uninflated) hit outranks any inflated one, whichever is nearer.
            let exact = if p.x <= 10.0 {
                Some(1)
            } else if p.x >= 16.0 {
                Some(2)
            } else {
                None
            };
            // Equidistant goes to the one drawn later — decided, not left to scan order.
            let expected = if (p.x - 5.0).abs() < (p.x - 21.0).abs() {
                1
            } else {
                2
            };
            assert_eq!(hit.id.0, exact.unwrap_or(expected), "at x={x}");
        }
    }

    #[test]
    fn a_scroll_offset_moves_the_point_and_not_the_rects() {
        let mut table = HitTable::default();
        let scroller = NodeId::raw(4, 1);
        let mut viewport = entry(
            1,
            (0.0, 0.0, 100.0, 100.0),
            HitFlags::SCROLL | HitFlags::CLIP,
        );
        viewport.scroll_src = NodeId::NONE;
        let mut row = entry(2, (0.0, 200.0, 100.0, 240.0), HitFlags::INTERACTIVE);
        row.scroll_src = scroller;
        row.clip_parent = 0;
        table.replace(&[viewport, row]);

        // Unscrolled, the row is far below the viewport and is clipped away.
        assert_eq!(
            table.hit(at(50.0, 50.0), ContactKind::Mouse).unwrap().id.0,
            1
        );
        // Scrolled down by 200, the row is under the pointer — and the entry's own rect
        // never moved.
        table.set_scroll(scroller, Vector2 { x: 0.0, y: 200.0 });
        assert_eq!(
            table.hit(at(50.0, 20.0), ContactKind::Mouse).unwrap().id.0,
            2
        );
    }

    #[test]
    fn the_memo_is_dropped_when_the_table_or_a_scroll_moves() {
        let mut table = HitTable::default();
        table.replace(&[entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE)]);
        assert_eq!(
            table.hit(at(50.0, 50.0), ContactKind::Mouse).unwrap().id.0,
            1
        );
        assert!(table.memo.get().is_some());

        table.replace(&[entry(9, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE)]);
        assert!(table.memo.get().is_none(), "a rebuild left a stale memo");
        assert_eq!(
            table.hit(at(50.0, 50.0), ContactKind::Mouse).unwrap().id.0,
            9
        );

        table.set_scroll(NodeId::raw(4, 1), Vector2 { x: 0.0, y: 10.0 });
        assert!(table.memo.get().is_none(), "a scroll left a stale memo");
    }

    #[test]
    fn a_cyclic_clip_chain_terminates_rather_than_hanging_the_pump() {
        let mut table = HitTable::default();
        let mut a = entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE);
        let mut b = entry(2, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE);
        a.clip_parent = 1;
        b.clip_parent = 0;
        table.replace(&[a, b]);
        // Debug builds assert; either way it returns, which is the property under test.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            table.hit(at(50.0, 50.0), ContactKind::Mouse)
        }));
    }
}
