//! The flat hit array and its query. **Both halves.**
//!
//! One contiguous z-ordered array stands in for every tree walk. Pointer routing, wheel
//! routing, gesture targeting, keyboard focus order, the window's own caption hit test and
//! automation's element-from-point all resolve through it.
//!
//! The scan runs back to front and takes the first hit. Paint order is z-order, so that is
//! the last eligible node in a depth-first walk, with no descent and no parent-miss prune: a
//! child drawn past its parent is still hit, which is what a shadow, a focus ring and a
//! popup anchor all depend on.

use crate::hit_entry::{ContactKind, Hit, HitEntry, HitFlags, NO_ENTRY, unpack_offset};
use crate::sink::{ControlId, NodeId, Point};
use core::cell::Cell;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use windows_numerics::Vector2;

/// Where a viewport's live offset comes from.
///
/// A table the scene owns records offsets directly; a copy queried on another thread routes
/// the trackers' shadow words, so it resolves against current offsets with no lock and no
/// message.
enum Offset {
    Direct(Vector2),
    Shadow(Arc<AtomicU64>),
}

impl Offset {
    fn get(&self) -> Vector2 {
        match self {
            Self::Direct(v) => *v,
            Self::Shadow(word) => {
                // Acquire, pairing with the release in the values-changed handler, so the
                // word read here is a position that handler finished writing.
                let (x, y) = unpack_offset(word.load(Ordering::Acquire));
                Vector2 { x, y }
            }
        }
    }
}

/// The last hit, so motion inside one control is a single rectangle test.
///
/// The rect is the entry's box narrowed by every clip above it, so a point inside it is
/// still admitted by that ancestry; the entry's own box alone would keep answering after the
/// pointer had left a clipped region.
#[derive(Copy, Clone)]
struct Memo {
    index: u32,
    epoch: u64,
    rect: [f32; 4],
}

/// Holds the hit array with the scroll offsets and memo a query resolves through.
#[derive(Default)]
pub struct HitTable {
    entries: Vec<HitEntry>,
    /// Control id to entry index, ordered by id: the app ships it built, so a rebuild copies
    /// and never sorts.
    index: Vec<(ControlId, u32)>,
    /// Searched linearly; a window holds a handful of scrolling surfaces.
    offsets: Vec<(NodeId, Offset)>,
    /// Bumped on every rebuild, which invalidates the memo.
    epoch: Cell<u64>,
    /// Interior mutability keeps the hover path on `&self`, which every consumer of the
    /// array shares.
    memo: Cell<Option<Memo>>,
}

impl HitTable {
    /// The rebuild count. Two tables copied from the same source at the same epoch hold the
    /// same entries, so a consumer keeping a copy compares this before copying.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch.get()
    }

    /// The entries in z-order. Focus order is this sequence filtered to what routes input.
    #[must_use]
    pub fn entries(&self) -> &[HitEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Replaces every entry and its id index, bumps the epoch and drops the memo.
    ///
    /// `index` must be ordered by control id: [`entry`](Self::entry) binary-searches it, and
    /// the app builds it during the walk that fills `entries`.
    pub fn replace(&mut self, entries: &[HitEntry], index: &[(ControlId, u32)]) {
        debug_assert!(
            index.windows(2).all(|pair| pair[0].0 <= pair[1].0),
            "the id index reached the scene unsorted"
        );
        self.entries.clear();
        self.entries.extend_from_slice(entries);
        self.index.clear();
        self.index.extend_from_slice(index);
        self.bump();
    }

    /// Copies another table's array and id index.
    ///
    /// The offsets are not copied by value: a snapshot queried on another thread reads the
    /// shadow words instead, because an offset copied here is stale by the time it would be
    /// read. Allocates nothing once both tables have reached the same working size.
    pub fn copy_from(&mut self, other: &Self) {
        self.entries.clear();
        self.entries.extend_from_slice(&other.entries);
        self.index.clear();
        self.index.extend_from_slice(&other.index);
        self.epoch.set(other.epoch.get());
        self.memo.set(None);
    }

    /// The entry `id` declared, or `None` where it declared none.
    ///
    /// This answers with that control's own rect, not with whatever entry lies under a point.
    #[must_use]
    pub fn entry(&self, id: ControlId) -> Option<&HitEntry> {
        let at = self.index.binary_search_by_key(&id, |&(key, _)| key).ok()?;
        self.entries.get(self.index[at].1 as usize)
    }

    /// Reports whether any interior survives the entry's scroll and clip ancestry.
    #[must_use]
    pub fn visible(&self, id: ControlId) -> bool {
        let Some(entry) = self.entry(id) else { return false; };
        let resolved = |entry: &HitEntry| {
            let offset = if entry.flags.contains(HitFlags::UNSCROLLED) {
                Vector2::zero()
            } else { self.offset(entry.scroll_src) };
            [entry.x0-offset.x, entry.y0-offset.y, entry.x1-offset.x, entry.y1-offset.y]
        };
        let mut rect = resolved(entry);
        let mut at = entry.clip_parent;
        let mut remaining = self.entries.len();
        while at != NO_ENTRY {
            if remaining == 0 { return false; }
            let Some(parent) = self.entries.get(at as usize) else { return false; };
            let clip = resolved(parent);
            rect = [rect[0].max(clip[0]), rect[1].max(clip[1]), rect[2].min(clip[2]), rect[3].min(clip[3])];
            at = parent.clip_parent;
            remaining -= 1;
        }
        rect[2] > rect[0] && rect[3] > rect[1]
    }

    /// Records a viewport's live offset and drops the memo.
    ///
    /// Called from the values-changed handler: a tracker runs in another process and every
    /// callback out of it is asynchronous, so the value the handler carries is the only
    /// current one. A scroll moves content under the pointer without the array changing, so
    /// the memo goes even though the epoch does not move.
    pub fn set_scroll(&mut self, viewport: NodeId, offset: Vector2) {
        self.put(viewport, Offset::Direct(offset));
        self.memo.set(None);
    }

    /// Routes viewports' offsets through the trackers' shadow words, replacing every offset
    /// held.
    ///
    /// The set changes only when a tracker is created or dropped, so a caller lists it on
    /// those edges and reads the atomics between them.
    pub fn set_shadows(&mut self, shadows: &[(NodeId, Arc<AtomicU64>)]) {
        self.offsets.clear();
        for (viewport, word) in shadows {
            self.offsets
                .push((*viewport, Offset::Shadow(Arc::clone(word))));
        }
        self.memo.set(None);
    }

    /// Forgets a viewport's offset, after which it resolves as unscrolled.
    pub fn clear_scroll(&mut self, viewport: NodeId) {
        self.offsets.retain(|(held, _)| *held != viewport);
        self.memo.set(None);
    }

    fn put(&mut self, viewport: NodeId, offset: Offset) {
        match self.offsets.iter_mut().find(|(held, _)| *held == viewport) {
            Some(slot) => slot.1 = offset,
            None => self.offsets.push((viewport, offset)),
        }
    }

    fn bump(&mut self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.memo.set(None);
    }

    /// Returns `viewport`'s live scroll offset, or zero where the table holds none for it.
    ///
    /// An entry's box is in unscrolled layout DIPs, so a caller placing something against a
    /// scrolled entry subtracts the offset of that entry's `scroll_src`, as [`hit`](Self::hit) does.
    #[must_use]
    pub fn offset(&self, viewport: NodeId) -> Vector2 {
        match self.offsets.iter().find(|(held, _)| *held == viewport) {
            Some((_, offset)) => offset.get(),
            None => Vector2::zero(),
        }
    }

    /// What is under `p`, recorded as the next memo.
    ///
    /// The memo bounds the scan and does not short-circuit it: a pointer still inside the
    /// rect the last answer was admitted through can have a control drawn *above* that
    /// answer under it by now. What it does establish is that nothing *below* that entry can
    /// win, since the scan is back-to-front and takes the first hit, so it supplies a floor
    /// and the skipped tail holds most of the entries.
    pub fn hit(&self, p: Point, contact: ContactKind) -> Option<Hit> {
        let floor = match self.memo.get() {
            Some(memo)
                if memo.epoch == self.epoch.get()
                    && p.x >= memo.rect[0]
                    && p.y >= memo.rect[1]
                    && p.x <= memo.rect[2]
                    && p.y <= memo.rect[3] =>
            {
                memo.index
            }
            _ => 0,
        };
        let found = scan(&self.entries, p, contact, floor, &|viewport| {
            self.offset(viewport)
        })?;
        self.memo.set(Some(Memo {
            index: found.index,
            epoch: self.epoch.get(),
            rect: self.clipped_box(found.index),
        }));
        Some(found)
    }

    /// The entry's box, narrowed by every clip above it.
    fn clipped_box(&self, index: u32) -> [f32; 4] {
        let Some(entry) = self.entries.get(index as usize) else {
            return [0.0; 4];
        };
        let mut box_ = [entry.x0, entry.y0, entry.x1, entry.y1];
        let mut at = entry.clip_parent;
        let mut guard = self.entries.len();
        while at != NO_ENTRY && guard > 0 {
            let Some(clip) = self.entries.get(at as usize) else {
                break;
            };
            box_ = [
                box_[0].max(clip.x0),
                box_[1].max(clip.y0),
                box_[2].min(clip.x1),
                box_[3].min(clip.y1),
            ];
            at = clip.clip_parent;
            guard -= 1;
        }
        box_
    }
}

/// Scans back to front and takes the first hit.
///
/// A free function because it has two callers on two threads — [`HitTable::hit`], and
/// automation's element-from-point over a published copy of the same array — and nothing in
/// it is thread-affine.
///
/// `floor` bounds the scan from below and does not short-circuit it. A caller may skip
/// everything below a previous answer, because the scan takes the first hit from the back,
/// so nothing below that index can win. Callers holding no previous answer pass zero.
pub fn scan(
    entries: &[HitEntry],
    p: Point,
    contact: ContactKind,
    floor: u32,
    offset: &dyn Fn(NodeId) -> Vector2,
) -> Option<Hit> {
    // Layout places content unscrolled and the compositor applies the offset, so a query
    // moves the point rather than the rects.
    let resolve = |entry: &HitEntry| -> Point {
        if entry.scroll_src.is_none() || entry.flags.contains(HitFlags::UNSCROLLED) {
            p
        } else {
            let o = offset(entry.scroll_src);
            Point {
                x: p.x + o.x,
                y: p.y + o.y,
            }
        }
    };
    // Walks the clip ancestry, rejecting a point any clipping ancestor excludes. The scan
    // has no descent to prune, so overhang survives and only a clip removes an entry.
    let clipped_out = |mut at: u32| -> bool {
        // A cycle in the clip-parent indices would otherwise spin here and hang the pump.
        // The bound is the array's own length, which no acyclic chain can exceed.
        let mut guard = entries.len();
        while at != NO_ENTRY {
            let Some(clip) = entries.get(at as usize) else {
                return false;
            };
            if !clip.contains(resolve(clip), 0.0) {
                return true;
            }
            at = clip.clip_parent;
            guard = guard.saturating_sub(1);
            if guard == 0 {
                debug_assert!(false, "the clip chain is cyclic");
                return true;
            }
        }
        false
    };

    let mut best: Option<(f32, Hit)> = None;
    for index in (floor as usize..entries.len()).rev() {
        let entry = &entries[index];
        if !entry
            .flags
            .intersects(HitFlags::INTERACTIVE.union(HitFlags::SCROLL))
        {
            continue;
        }
        let q = resolve(entry);
        let inflate = if contact.inflates() && !entry.flags.contains(HitFlags::NO_INFLATE) {
            entry.touch_inflate
        } else {
            0.0
        };
        if !entry.contains(q, inflate) || clipped_out(entry.clip_parent) {
            continue;
        }
        let hit = Hit {
            index: index as u32,
            id: entry.id,
            flags: entry.flags,
            local: Point {
                x: q.x - entry.x0,
                y: q.y - entry.y0,
            },
        };
        // An uninflated hit is exact and wins outright; only inflated ones compete, nearest
        // centre first, so two neighbours whose inflated boxes overlap cannot both claim a
        // point. An exact tie keeps the candidate found first, which is the topmost, so the
        // answer is stable frame to frame.
        if entry.contains(q, 0.0) {
            return Some(hit);
        }
        let distance = entry.centre_distance_sq(q);
        if best.as_ref().is_none_or(|(held, _)| distance < *held) {
            best = Some((distance, hit));
        }
    }
    best.map(|(_, hit)| hit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hit_entry::pack_offset;

    fn entry(id: u32, rect: (f32, f32, f32, f32), flags: HitFlags) -> HitEntry {
        HitEntry {
            x0: rect.0,
            y0: rect.1,
            x1: rect.2,
            y1: rect.3,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: NO_ENTRY,
            flags,
            scroll_src: NodeId::NONE,
            id: ControlId::raw(id, 1),
        }
    }

    fn at(x: f32, y: f32) -> Point {
        Vector2 { x, y }
    }

    /// The id index the app ships alongside the entries, ordered by id.
    fn index(entries: &[HitEntry]) -> Vec<(ControlId, u32)> {
        let mut rows: Vec<_> = entries
            .iter()
            .enumerate()
            .map(|(at, entry)| (entry.id, at as u32))
            .collect();
        rows.sort_unstable_by_key(|&(id, _)| id);
        rows
    }

    fn table(entries: &[HitEntry]) -> HitTable {
        let mut table = HitTable::default();
        table.replace(entries, &index(entries));
        table
    }

    #[test]
    fn visibility_follows_clip_scroll_and_control_generation() {
        let mut child = entry(2,(10.0,80.0,90.0,140.0),HitFlags::INTERACTIVE);
        child.clip_parent = 0;
        child.scroll_src = NodeId::raw(4,1);
        let mut hits = table(&[entry(1,(0.0,0.0,100.0,100.0),HitFlags::CLIP),child]);
        let id = ControlId::raw(2,1);
        assert!(hits.visible(id));
        hits.set_scroll(child.scroll_src,Vector2::new(0.0,140.0));
        assert!(!hits.visible(id));
        hits.set_scroll(child.scroll_src,Vector2::new(0.0,40.0));
        assert!(hits.visible(id));
        assert!(!hits.visible(ControlId::raw(2,2)));
        child.x1 = child.x0;
        hits.replace(&[child],&index(&[child]));
        assert!(!hits.visible(id));
    }

    #[test]
    fn the_last_entry_wins_because_paint_order_is_z_order() {
        let table = table(&[
            entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE),
            entry(2, (20.0, 20.0, 60.0, 60.0), HitFlags::INTERACTIVE),
        ]);
        let hit = |p| table.hit(p, ContactKind::Mouse).unwrap().id.index();
        assert_eq!(hit(at(30.0, 30.0)), 2);
        assert_eq!(hit(at(90.0, 90.0)), 1);
    }

    #[test]
    fn overhang_survives_because_there_is_no_descent_to_prune() {
        // A focus ring drawn outside its parent's box, with no clip anywhere.
        let table = table(&[
            entry(1, (0.0, 0.0, 50.0, 50.0), HitFlags::INTERACTIVE),
            entry(2, (40.0, 40.0, 90.0, 90.0), HitFlags::INTERACTIVE),
        ]);
        assert_eq!(
            table
                .hit(at(80.0, 80.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            2
        );
    }

    #[test]
    fn a_clip_ancestor_removes_what_it_excludes() {
        let mut child = entry(2, (40.0, 40.0, 200.0, 200.0), HitFlags::INTERACTIVE);
        child.clip_parent = 0;
        let table = table(&[
            entry(
                1,
                (0.0, 0.0, 100.0, 100.0),
                HitFlags::INTERACTIVE | HitFlags::CLIP,
            ),
            child,
        ]);
        assert_eq!(
            table
                .hit(at(60.0, 60.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            2
        );
        // Outside the clip: the child is gone, and so is the clip itself.
        assert!(table.hit(at(150.0, 150.0), ContactKind::Mouse).is_none());
    }

    #[test]
    fn inflation_applies_to_touch_and_pen_and_to_nothing_else() {
        let mut small = entry(1, (50.0, 50.0, 60.0, 60.0), HitFlags::INTERACTIVE);
        small.touch_inflate = 10.0;
        let table = table(&[small]);
        assert!(table.hit(at(45.0, 55.0), ContactKind::Mouse).is_none());
        assert!(table.hit(at(45.0, 55.0), ContactKind::Touchpad).is_none());
        assert!(table.hit(at(45.0, 55.0), ContactKind::Touch).is_some());
        assert!(table.hit(at(45.0, 55.0), ContactKind::Pen).is_some());
    }

    #[test]
    fn an_opted_out_target_is_not_inflated_for_touch() {
        let mut small = entry(
            1,
            (50.0, 50.0, 60.0, 60.0),
            HitFlags::INTERACTIVE | HitFlags::NO_INFLATE,
        );
        small.touch_inflate = 10.0;
        let table = table(&[small]);
        assert!(table.hit(at(45.0, 55.0), ContactKind::Touch).is_none());
        assert!(table.hit(at(55.0, 55.0), ContactKind::Touch).is_some());
    }

    #[test]
    fn two_inflated_targets_never_both_claim_a_point() {
        // Two 10-DIP targets 6 DIPs apart, each inflated by 8 — so their inflated boxes
        // overlap and the gap between them is claimed by both.
        let mut left = entry(1, (0.0, 0.0, 10.0, 10.0), HitFlags::INTERACTIVE);
        let mut right = entry(2, (16.0, 0.0, 26.0, 10.0), HitFlags::INTERACTIVE);
        left.touch_inflate = 8.0;
        right.touch_inflate = 8.0;
        let table = table(&[left, right]);

        for x in 0..=26 {
            let p = at(x as f32, 5.0);
            let hit = table
                .hit(p, ContactKind::Touch)
                .expect("inside one of them");
            // An exact (uninflated) hit outranks any inflated one, whichever is nearer.
            let exact = if p.x <= 10.0 {
                Some(1)
            } else if p.x >= 16.0 {
                Some(2)
            } else {
                None
            };
            // Centres are at 5 and 21, so the boundary is 13; equidistant goes to the one
            // drawn later.
            let nearest = if (p.x - 5.0).abs() < (p.x - 21.0).abs() {
                1
            } else {
                2
            };
            assert_eq!(hit.id.index(), exact.unwrap_or(nearest), "at x={x}");
        }
    }

    #[test]
    fn a_scroll_offset_moves_the_point_and_not_the_rects() {
        let (mut table, scroller) = scrolled();
        // Unscrolled, the row is far below the viewport and is clipped away.
        assert_eq!(
            table
                .hit(at(50.0, 50.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            1
        );
        // Scrolled down by 200, the row is under the pointer — and the entry's own rect
        // never moved.
        table.set_scroll(scroller, Vector2 { x: 0.0, y: 200.0 });
        assert_eq!(
            table
                .hit(at(50.0, 20.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            2
        );
    }

    #[test]
    fn chrome_pinned_to_a_viewport_does_not_travel_with_its_content() {
        let (mut table, scroller) = scrolled();
        let mut rail = entry(
            3,
            (90.0, 0.0, 100.0, 100.0),
            HitFlags::INTERACTIVE | HitFlags::UNSCROLLED,
        );
        rail.scroll_src = scroller;
        let mut entries = table.entries().to_vec();
        entries.push(rail);
        table.replace(&entries, &index(&entries));
        table.set_scroll(scroller, Vector2 { x: 0.0, y: 200.0 });
        assert_eq!(
            table
                .hit(at(95.0, 50.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            3,
            "the rail slid off its own track"
        );
    }

    #[test]
    fn the_local_point_is_in_the_targets_own_space() {
        let (mut table, scroller) = scrolled();
        table.set_scroll(scroller, Vector2 { x: 0.0, y: 200.0 });
        let hit = table.hit(at(50.0, 20.0), ContactKind::Mouse).unwrap();
        // The row is at y 200 unscrolled, and the pointer resolves to 220 inside it.
        assert_eq!((hit.local.x, hit.local.y), (50.0, 20.0));
    }

    #[test]
    fn the_memo_is_dropped_when_the_table_or_a_scroll_moves() {
        let mut table = table(&[entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE)]);
        assert_eq!(
            table
                .hit(at(50.0, 50.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            1
        );
        assert!(table.memo.get().is_some());

        let next = [entry(9, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE)];
        table.replace(&next, &index(&next));
        assert!(table.memo.get().is_none(), "a rebuild left a stale memo");
        assert_eq!(
            table
                .hit(at(50.0, 50.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            9
        );

        table.set_scroll(NodeId::raw(4, 1), Vector2 { x: 0.0, y: 10.0 });
        assert!(table.memo.get().is_none(), "a scroll left a stale memo");
    }

    /// The memo's rect is the clipped box, so a pointer leaving a clipped region stops
    /// answering from it.
    #[test]
    fn the_memo_rect_is_narrowed_by_the_clip_ancestry() {
        let mut child = entry(2, (40.0, 40.0, 200.0, 200.0), HitFlags::INTERACTIVE);
        child.clip_parent = 0;
        let table = table(&[
            entry(
                1,
                (0.0, 0.0, 100.0, 100.0),
                HitFlags::INTERACTIVE | HitFlags::CLIP,
            ),
            child,
        ]);
        assert!(table.hit(at(60.0, 60.0), ContactKind::Mouse).is_some());
        let memo = table.memo.get().expect("a recorded memo");
        assert_eq!(memo.rect, [40.0, 40.0, 100.0, 100.0]);
    }

    #[test]
    fn a_copy_answers_every_query_the_source_does() {
        let mut child = entry(2, (40.0, 40.0, 200.0, 200.0), HitFlags::INTERACTIVE);
        child.clip_parent = 0;
        let mut inflated = entry(3, (300.0, 0.0, 310.0, 10.0), HitFlags::INTERACTIVE);
        inflated.touch_inflate = 8.0;
        let source = table(&[
            entry(
                1,
                (0.0, 0.0, 100.0, 100.0),
                HitFlags::INTERACTIVE | HitFlags::CLIP,
            ),
            child,
            inflated,
        ]);

        let mut copy = HitTable::default();
        copy.copy_from(&source);
        assert_eq!(copy.entries(), source.entries());
        assert_eq!(
            copy.entry(ControlId::raw(2, 1)),
            source.entry(ControlId::raw(2, 1))
        );
        assert_eq!(copy.epoch(), source.epoch());

        for x in (0..320).step_by(7) {
            for y in (0..220).step_by(11) {
                let p = at(x as f32, y as f32);
                for contact in [ContactKind::Mouse, ContactKind::Touch] {
                    assert_eq!(
                        copy.hit(p, contact).map(|hit| hit.id),
                        source.hit(p, contact).map(|hit| hit.id),
                        "at ({x}, {y}) for {contact:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_copy_into_a_filled_table_leaves_nothing_of_the_old_one() {
        let source = table(&[entry(1, (0.0, 0.0, 10.0, 10.0), HitFlags::INTERACTIVE)]);
        let mut copy = table(&[
            entry(7, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE),
            entry(8, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE),
        ]);
        // A query first, so the copy has a memo to invalidate.
        assert!(copy.hit(at(50.0, 50.0), ContactKind::Mouse).is_some());

        copy.copy_from(&source);
        assert_eq!(copy.len(), 1);
        assert!(copy.entry(ControlId::raw(7, 1)).is_none());
        assert!(copy.hit(at(50.0, 50.0), ContactKind::Mouse).is_none());
        assert_eq!(
            copy.hit(at(5.0, 5.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            1
        );
    }

    /// The scrolled-viewport scene the offset tests share: a clipping viewport with one row
    /// placed 200 DIPs below it, so the row is reachable only once the offset moves it.
    fn scrolled() -> (HitTable, NodeId) {
        let scroller = NodeId::raw(4, 1);
        let viewport = entry(
            1,
            (0.0, 0.0, 100.0, 100.0),
            HitFlags::SCROLL | HitFlags::CLIP,
        );
        let mut row = entry(2, (0.0, 200.0, 100.0, 240.0), HitFlags::INTERACTIVE);
        row.scroll_src = scroller;
        row.clip_parent = 0;
        (table(&[viewport, row]), scroller)
    }

    #[test]
    fn a_shadow_resolves_a_scrolled_viewport_as_a_recorded_offset_does() {
        let (mut told, scroller) = scrolled();
        let (mut shadowed, _) = scrolled();

        let shadow = Arc::new(AtomicU64::new(pack_offset(0.0, 0.0)));
        shadowed.set_shadows(&[(scroller, Arc::clone(&shadow))]);

        for y in [0.0f32, 40.0, 120.0, 200.0, 239.0] {
            told.set_scroll(scroller, Vector2 { x: 0.0, y });
            shadow.store(pack_offset(0.0, y), Ordering::Release);
            for probe in (0..100).step_by(9) {
                let p = at(50.0, probe as f32);
                assert_eq!(
                    shadowed.hit(p, ContactKind::Mouse).map(|hit| hit.id),
                    told.hit(p, ContactKind::Mouse).map(|hit| hit.id),
                    "scrolled to {y}, probed at {probe}"
                );
            }
        }
    }

    #[test]
    fn a_cleared_shadow_resolves_as_unscrolled() {
        let (mut table, scroller) = scrolled();
        let shadow = Arc::new(AtomicU64::new(pack_offset(0.0, 200.0)));
        table.set_shadows(&[(scroller, shadow)]);
        assert_eq!(
            table
                .hit(at(50.0, 20.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            2
        );

        table.clear_scroll(scroller);
        assert_eq!(
            table
                .hit(at(50.0, 20.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            1
        );
    }

    #[test]
    fn installing_shadows_again_replaces_every_offset_held() {
        let (mut table, scroller) = scrolled();
        table.set_shadows(&[(scroller, Arc::new(AtomicU64::new(pack_offset(0.0, 0.0))))]);
        table.set_shadows(&[(scroller, Arc::new(AtomicU64::new(pack_offset(0.0, 200.0))))]);
        assert_eq!(
            table
                .hit(at(50.0, 20.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            2
        );
    }

    #[test]
    fn a_cyclic_clip_chain_terminates_rather_than_hanging_the_pump() {
        let mut a = entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE);
        let mut b = entry(2, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE);
        a.clip_parent = 1;
        b.clip_parent = 0;
        let table = table(&[a, b]);
        // Debug builds assert; either way the call returns rather than spinning.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            table.hit(at(50.0, 50.0), ContactKind::Mouse)
        }));
    }

    #[test]
    fn the_floor_skips_the_tail_without_changing_the_answer() {
        let entries = [
            entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE),
            entry(2, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE),
            entry(3, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE),
        ];
        let nothing = |_: NodeId| Vector2::zero();
        for floor in 0..3 {
            let hit = scan(&entries, at(5.0, 5.0), ContactKind::Mouse, floor, &nothing);
            assert_eq!(hit.unwrap().id.index(), 3, "floor {floor}");
        }
        assert!(scan(&entries, at(5.0, 5.0), ContactKind::Mouse, 3, &nothing).is_none());
    }
}
