//! Queries the flat hit array. **Front half.**
//!
//! The array is built on the app thread and queried wherever a contact arrives. A query
//! resolves through live scroll offsets, which a table either records for itself through
//! [`HitTable::set_scroll`] or reads from the trackers' shadow words through
//! [`ShadowOffsets`]; [`ScrollOffsets`] is the seam between the two. Pointer routing, wheel
//! routing, gesture targeting, keyboard focus order, the window's own caption hit test and
//! automation's element-from-point all resolve through this array, and a presentation
//! region's parts extend it rather than forking it.
//!
//! The scan runs back to front and takes the first hit. Paint order is z-order, so that is
//! the last eligible node in a depth-first walk, with no descent and no parent-miss prune.
//! A child drawn past its parent is still hit: a shadow, a focus ring and a popup anchor
//! all extend past theirs.

use crate::hit_build::{HitEntry, HitFlags, NO_ENTRY};
use crate::sink::{NodeId, Point};
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use windows_numerics::Vector2;

/// Names the input device a contact came from. Only touch and pen inflate a target.
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
    /// Returns `true` where a target's touch inflation applies to this contact.
    #[must_use]
    pub const fn inflates(self) -> bool {
        matches!(self, Self::Touch | Self::Pen)
    }
}

/// Identifies the entry a query resolved to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Hit {
    /// Index into the array. Joins to any table a consumer keeps in parallel.
    pub index: u32,
    pub id: crate::hit_build::ControlId,
    pub flags: HitFlags,
    /// The point in the target's own space, with its scroll ancestry applied.
    pub local: Point,
}

/// Returns the entry under `p`, scanning back to front and taking the first hit.
///
/// A free function rather than a method, because it has two callers on two threads:
/// [`HitTable::hit`], and automation's element-from-point, which reads a published copy of
/// the same array from a UI Automation worker. Nothing in the scan is thread-affine.
///
/// `offset` supplies a scroll container's live offset; `contact` decides whether touch
/// inflation applies.
///
/// `floor` bounds the scan from below and does not short-circuit it. A caller may skip
/// everything below a previous answer, because the scan is back-to-front and takes the
/// first hit, so nothing below that index can win. It may not answer from the previous
/// result, because a control drawn *above* can have come under the point meanwhile.
/// Callers holding no previous answer pass 0.
///
/// Returns the winning index into `entries`, and `p` in that entry's own space.
pub fn scan(
    entries: &[HitEntry],
    offset: impl Fn(NodeId) -> Vector2,
    floor: usize,
    p: Point,
    contact: ContactKind,
) -> Option<(usize, Point)> {
    // Layout places content unscrolled and the compositor applies the offset, so a query
    // moves the point rather than the rects.
    let resolve = |p: Point, scroll: NodeId| {
        if scroll.is_none() {
            return p;
        }
        let o = offset(scroll);
        Vector2 {
            x: p.x + o.x,
            y: p.y + o.y,
        }
    };
    // Walks the clip ancestry, rejecting a point any clipping ancestor excludes. The scan
    // has no descent to prune, so overhang survives and only a clip removes an entry.
    let admitted = |mut parent: u32, p: Point| {
        let mut guard = entries.len();
        while parent != NO_ENTRY {
            let Some(entry) = entries.get(parent as usize) else {
                return true;
            };
            if !entry.contains(resolve(p, entry.scroll_src), 0.0) {
                return false;
            }
            parent = entry.clip_parent;
            // A cycle in the clip-parent indices would otherwise spin here and hang the
            // pump. The bound is the array's own length, which no acyclic chain can exceed.
            guard = guard.saturating_sub(1);
            if guard == 0 {
                debug_assert!(false, "the clip chain is cyclic");
                return false;
            }
        }
        true
    };

    let mut best: Option<(usize, f32, Point)> = None;
    for index in (floor..entries.len()).rev() {
        let entry = &entries[index];
        if !entry
            .flags
            .intersects(HitFlags::INTERACTIVE | HitFlags::SCROLL)
        {
            continue;
        }
        let q = resolve(p, entry.scroll_src);
        let inflate = if contact.inflates() && !entry.flags.contains(HitFlags::NO_INFLATE) {
            entry.touch_inflate
        } else {
            0.0
        };
        if !entry.contains(q, inflate) || !admitted(entry.clip_parent, p) {
            continue;
        }
        // An uninflated hit is exact and wins outright. Only inflated ones compete, nearest
        // centre first, so two neighbours whose inflated boxes overlap cannot both claim a
        // point. An exact tie keeps the candidate found first, which is the topmost, so two
        // targets equidistant from a point resolve the same way on every frame.
        if entry.contains(q, 0.0) {
            return Some((index, q));
        }
        let distance = entry.centre_distance_sq(q);
        if best.is_none_or(|(_, existing, _)| distance < existing) {
            best = Some((index, distance, q));
        }
    }
    best.map(|(index, _, q)| (index, q))
}

/// Supplies the live offset of a scroll container to a query.
///
/// A query moves the point rather than the rects, so every entry carrying a `scroll_src`
/// asks this for that viewport's offset. An implementation answers with zero for a viewport
/// it holds nothing for, which is what an unscrolled container reads as.
pub trait ScrollOffsets {
    /// Returns `viewport`'s live offset, or zero where it holds none.
    fn offset(&self, viewport: NodeId) -> Vector2;
}

/// The offsets a table was told directly, through [`HitTable::set_scroll`].
///
/// Searched linearly; a window holds a handful of scrolling surfaces.
#[derive(Debug, Default)]
struct Scrolls(Vec<(NodeId, Vector2)>);

impl ScrollOffsets for Scrolls {
    fn offset(&self, viewport: NodeId) -> Vector2 {
        match self.0.iter().find(|(id, _)| *id == viewport) {
            Some(&(_, offset)) => offset,
            None => Vector2::zero(),
        }
    }
}

/// Packs an offset into the word a tracker shadow holds: `x` in the high half, `y` in the
/// low half.
#[must_use]
pub const fn pack_offset(x: f32, y: f32) -> u64 {
    ((x.to_bits() as u64) << 32) | y.to_bits() as u64
}

/// Unpacks the word [`pack_offset`] produced, into `(x, y)`.
#[must_use]
pub const fn unpack_offset(packed: u64) -> (f32, f32) {
    (
        f32::from_bits((packed >> 32) as u32),
        f32::from_bits(packed as u32),
    )
}

/// Resolves a query's scroll offsets from the trackers' shadow words.
///
/// A shadow is written by the thread the tracker reports on and read here, so a table
/// holding a copy of that thread's array queries against current offsets with no lock and no
/// message. The whole offset is one word, so a reader sees both axes of one reported
/// position and never one axis of two.
///
/// Searched linearly; a window holds a handful of scrolling surfaces.
#[derive(Debug, Default)]
pub struct ShadowOffsets(Vec<(NodeId, Arc<AtomicU64>)>);

impl ShadowOffsets {
    /// Returns a table routing no shadow, having allocated nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Routes `viewport`'s offset through `shadow`, replacing any shadow already held for it.
    pub fn insert(&mut self, viewport: NodeId, shadow: Arc<AtomicU64>) {
        match self.0.iter_mut().find(|(id, _)| *id == viewport) {
            Some((_, existing)) => *existing = shadow,
            None => self.0.push((viewport, shadow)),
        }
    }

    /// Forgets `viewport`'s shadow, after which it resolves as unscrolled.
    /// Forgets every shadow, keeping the table's capacity.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn remove(&mut self, viewport: NodeId) {
        self.0.retain(|(id, _)| *id != viewport);
    }

    /// Returns how many shadows the table routes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` where the table routes no shadow.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl ScrollOffsets for ShadowOffsets {
    fn offset(&self, viewport: NodeId) -> Vector2 {
        match self.0.iter().find(|(id, _)| *id == viewport) {
            Some((_, shadow)) => {
                // acquire: pairs with the release in the tracker's values-changed handler,
                // so the word read here is a position that handler finished writing.
                let (x, y) = unpack_offset(shadow.load(Ordering::Acquire));
                Vector2 { x, y }
            }
            None => Vector2::zero(),
        }
    }
}

/// Holds the hit array with the scroll offsets and memo a query resolves through.
#[derive(Debug, Default)]
pub struct HitTable {
    entries: Vec<HitEntry>,
    /// Bumped on every rebuild, which invalidates the memo.
    epoch: u64,
    /// The live offset of each scroll container, as its tracker last reported it.
    scrolls: Scrolls,
    /// Control id to entry index, sorted for binary search. A consumer holds an id rather
    /// than a position, and a value control asks for its own rect on every pointer move.
    /// Rebuilt whole with the array and never edited in place.
    by_id: Vec<(crate::hit_build::ControlId, u32)>,
    /// The last hit, so motion inside one control is a single rectangle test. Interior
    /// mutability keeps the hover path on `&self`, which every consumer of the array shares.
    memo: core::cell::Cell<Option<Memo>>,
}

#[derive(Copy, Clone, Debug)]
struct Memo {
    index: u32,
    epoch: u64,
    rect: (f32, f32, f32, f32),
}

impl HitTable {
    /// Returns the rebuild count. Two tables copied from the same source at the same epoch
    /// hold the same entries, so a consumer keeping a copy compares this before copying.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Replaces every entry, rebuilds the id index, bumps the epoch and drops the memo.
    pub fn replace(&mut self, entries: &[HitEntry]) {
        self.entries.clear();
        self.entries.extend_from_slice(entries);
        self.by_id.clear();
        self.by_id
            .extend(entries.iter().enumerate().map(|(at, e)| (e.id, at as u32)));
        self.by_id.sort_unstable_by_key(|&(id, _)| id);
        self.epoch = self.epoch.wrapping_add(1);
        self.memo.set(None);
    }

    /// Replaces every entry with `other`'s, as [`replace`](HitTable::replace) does.
    ///
    /// The scroll offsets are not copied: a table filled this way is a snapshot queried on
    /// another thread, where an offset copied from the source table is stale by the time it
    /// would be read, and [`hit_with`](HitTable::hit_with) supplies live ones instead.
    ///
    /// Allocates nothing once both tables have reached the same working size.
    pub fn copy_from(&mut self, other: &Self) {
        self.replace(&other.entries);
    }

    /// Returns the entry `id` declared, or `None` where it declared none.
    ///
    /// Binary searches the id index. This answers with that control's own rect, not with
    /// whatever entry lies under a point.
    #[must_use]
    pub fn entry(&self, id: crate::hit_build::ControlId) -> Option<&HitEntry> {
        let at = self.by_id.binary_search_by_key(&id, |&(key, _)| key).ok()?;
        self.entries.get(self.by_id[at].1 as usize)
    }

    /// Records a scroll container's live offset and drops the memo.
    ///
    /// Called from the tracker's values-changed handler. A tracker runs in another process
    /// and every call into it and callback out of it is asynchronous, so the value the
    /// handler carries is the only current one.
    pub fn set_scroll(&mut self, node: NodeId, offset: Vector2) {
        match self.scrolls.0.iter_mut().find(|(id, _)| *id == node) {
            Some((_, existing)) => *existing = offset,
            None => self.scrolls.0.push((node, offset)),
        }
        // A scroll moves content under the pointer without the array changing, so the memo
        // is dropped even though the epoch does not move.
        self.memo.set(None);
    }

    /// Forgets a scroll container's offset and drops the memo.
    pub fn clear_scroll(&mut self, node: NodeId) {
        self.scrolls.0.retain(|(id, _)| *id != node);
        self.memo.set(None);
    }

    /// Returns how many entries the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` where the table holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the entries in z-order. Focus order is this sequence filtered to what routes
    /// input.
    #[must_use]
    pub fn entries(&self) -> &[HitEntry] {
        &self.entries
    }

    /// Returns what is under `p`, and records it as the next memo.
    ///
    /// The memo bounds the scan and does not short-circuit it. A pointer still inside the
    /// rect the last answer was admitted through can have a control drawn *above* that
    /// answer under it by now, so answering from the memo would report the entry
    /// underneath. What the memo does establish is that nothing *below* that entry can win,
    /// since the scan is back-to-front and takes the first hit, so it supplies a floor and
    /// the skipped tail holds most of the entries.
    pub fn hit(&self, p: Point, contact: ContactKind) -> Option<Hit> {
        self.hit_with(p, contact, &self.scrolls)
    }

    /// Returns what is under `p` resolved through `offsets`, and records it as the next memo.
    ///
    /// The form [`hit`](HitTable::hit) is written in: it supplies the offsets the table was
    /// told through [`set_scroll`](HitTable::set_scroll), and a table holding a copy of
    /// another thread's array supplies a [`ShadowOffsets`] instead.
    ///
    /// The memo bounds the scan and does not short-circuit it, for the reason
    /// [`hit`](HitTable::hit) states. It is keyed on the array's epoch alone, so a caller
    /// alternating two offset sources over one table would answer the second from a memo the
    /// first recorded.
    pub fn hit_with(
        &self,
        p: Point,
        contact: ContactKind,
        offsets: &dyn ScrollOffsets,
    ) -> Option<Hit> {
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
        let (index, local) = scan(
            &self.entries,
            |node| offsets.offset(node),
            floor,
            p,
            contact,
        )?;
        Some(self.record(index, local))
    }

    fn record(&self, index: usize, local: Point) -> Hit {
        let entry = self.entries[index];
        let hit = Hit {
            index: index as u32,
            id: entry.id,
            flags: entry.flags,
            local,
        };
        // The memo rect is intersected with the clip ancestry, so a point inside it is
        // still admitted by every clip above the entry. The entry's own box alone would
        // keep answering after the pointer had left a clipped region.
        self.memo.set(Some(Memo {
            index: index as u32,
            epoch: self.epoch,
            rect: self.clipped_box(index),
        }));
        hit
    }

    /// Returns the entry's box, narrowed by every clip above it.
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

    #[test]
    fn the_last_entry_wins_because_paint_order_is_z_order() {
        let mut table = HitTable::default();
        table.replace(&[
            entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE),
            entry(2, (20.0, 20.0, 60.0, 60.0), HitFlags::INTERACTIVE),
        ]);
        assert_eq!(
            table
                .hit(at(30.0, 30.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            2
        );
        assert_eq!(
            table
                .hit(at(90.0, 90.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
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
            // Nearest centre, and exactly one answer. Centres are at 5 and 21, so the
            // boundary is 13.
            // An exact (uninflated) hit outranks any inflated one, whichever is nearer.
            let exact = if p.x <= 10.0 {
                Some(1)
            } else if p.x >= 16.0 {
                Some(2)
            } else {
                None
            };
            // Equidistant goes to the one drawn later.
            let expected = if (p.x - 5.0).abs() < (p.x - 21.0).abs() {
                1
            } else {
                2
            };
            assert_eq!(hit.id.index(), exact.unwrap_or(expected), "at x={x}");
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
    fn the_memo_is_dropped_when_the_table_or_a_scroll_moves() {
        let mut table = HitTable::default();
        table.replace(&[entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE)]);
        assert_eq!(
            table
                .hit(at(50.0, 50.0), ContactKind::Mouse)
                .unwrap()
                .id
                .index(),
            1
        );
        assert!(table.memo.get().is_some());

        table.replace(&[entry(9, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE)]);
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

    #[test]
    fn a_copy_answers_every_query_the_source_does() {
        let mut source = HitTable::default();
        let mut child = entry(2, (40.0, 40.0, 200.0, 200.0), HitFlags::INTERACTIVE);
        child.clip_parent = 0;
        let mut inflated = entry(3, (300.0, 0.0, 310.0, 10.0), HitFlags::INTERACTIVE);
        inflated.touch_inflate = 8.0;
        source.replace(&[
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
        let mut source = HitTable::default();
        source.replace(&[entry(1, (0.0, 0.0, 10.0, 10.0), HitFlags::INTERACTIVE)]);
        let mut copy = HitTable::default();
        copy.replace(&[
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

    #[test]
    fn a_packed_offset_round_trips_through_its_word() {
        for (x, y) in [
            (0.0, 0.0),
            (0.0, -200.0),
            (12.5, 240.75),
            (-1.0, f32::MAX),
            (f32::MIN, 1.0e-30),
        ] {
            assert_eq!(unpack_offset(pack_offset(x, y)), (x, y));
        }
        // The halves do not bleed into each other: x is the high word, y the low.
        assert_eq!(pack_offset(0.0, 0.0), 0);
        assert_eq!(pack_offset(1.0, 0.0), (1.0f32.to_bits() as u64) << 32);
        assert_eq!(pack_offset(0.0, 1.0), 1.0f32.to_bits() as u64);
    }

    /// The scrolled-viewport scene the offset tests share: a clipping viewport with one row
    /// placed 200 DIPs below it, so the row is reachable only once the offset moves it.
    fn scrolled() -> (HitTable, NodeId) {
        let mut table = HitTable::default();
        let scroller = NodeId::raw(4, 1);
        let viewport = entry(
            1,
            (0.0, 0.0, 100.0, 100.0),
            HitFlags::SCROLL | HitFlags::CLIP,
        );
        let mut row = entry(2, (0.0, 200.0, 100.0, 240.0), HitFlags::INTERACTIVE);
        row.scroll_src = scroller;
        row.clip_parent = 0;
        table.replace(&[viewport, row]);
        (table, scroller)
    }

    #[test]
    fn a_shadow_resolves_a_scrolled_viewport_as_a_recorded_offset_does() {
        let (mut told, scroller) = scrolled();
        let (shadowed, _) = scrolled();

        let shadow = Arc::new(AtomicU64::new(pack_offset(0.0, 0.0)));
        let mut offsets = ShadowOffsets::new();
        offsets.insert(scroller, Arc::clone(&shadow));

        for y in [0.0f32, 40.0, 120.0, 200.0, 239.0] {
            told.set_scroll(scroller, Vector2 { x: 0.0, y });
            shadow.store(pack_offset(0.0, y), Ordering::Release);
            for probe in (0..100).step_by(9) {
                let p = at(50.0, probe as f32);
                assert_eq!(
                    shadowed
                        .hit_with(p, ContactKind::Mouse, &offsets)
                        .map(|h| h.id),
                    told.hit(p, ContactKind::Mouse).map(|h| h.id),
                    "scrolled to {y}, probed at {probe}"
                );
            }
        }
    }

    #[test]
    fn a_removed_shadow_resolves_as_unscrolled() {
        let (table, scroller) = scrolled();
        let shadow = Arc::new(AtomicU64::new(pack_offset(0.0, 200.0)));
        let mut offsets = ShadowOffsets::new();
        offsets.insert(scroller, Arc::clone(&shadow));
        assert_eq!(
            table
                .hit_with(at(50.0, 20.0), ContactKind::Mouse, &offsets)
                .unwrap()
                .id
                .index(),
            2
        );

        offsets.remove(scroller);
        assert!(offsets.is_empty());
        assert_eq!(offsets.offset(scroller), Vector2::zero());
        assert_eq!(
            table
                .hit_with(at(50.0, 20.0), ContactKind::Mouse, &offsets)
                .unwrap()
                .id
                .index(),
            1
        );
    }

    #[test]
    fn inserting_a_second_shadow_for_one_viewport_replaces_the_first() {
        let (table, scroller) = scrolled();
        let mut offsets = ShadowOffsets::new();
        offsets.insert(scroller, Arc::new(AtomicU64::new(pack_offset(0.0, 0.0))));
        offsets.insert(scroller, Arc::new(AtomicU64::new(pack_offset(0.0, 200.0))));
        assert_eq!(offsets.len(), 1);
        assert_eq!(
            table
                .hit_with(at(50.0, 20.0), ContactKind::Mouse, &offsets)
                .unwrap()
                .id
                .index(),
            2
        );
    }

    #[test]
    fn a_cyclic_clip_chain_terminates_rather_than_hanging_the_pump() {
        let mut table = HitTable::default();
        let mut a = entry(1, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE);
        let mut b = entry(2, (0.0, 0.0, 100.0, 100.0), HitFlags::INTERACTIVE);
        a.clip_parent = 1;
        b.clip_parent = 0;
        table.replace(&[a, b]);
        // Debug builds assert; either way the call returns rather than spinning.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            table.hit(at(50.0, 50.0), ContactKind::Mouse)
        }));
    }
}
