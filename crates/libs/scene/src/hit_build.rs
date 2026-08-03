//! Building the flat hit array. **App half.**
//!
//! One contiguous, z-ordered array replaces every tree walk. It is the **single
//! authority** for pointer routing, wheel routing, gesture targeting, keyboard focus
//! order, the window's own caption hit test and UI Automation's element-from-point — and a
//! presentation region's parts *extend* it rather than forking it. No parallel path may
//! exist.
//!
//! It is built here, in one linear pass over the solved layout in paint order, and queried
//! on the other side of the seam, where live scroll offsets are.

use crate::id::Id;
use crate::layout::Solved;
use crate::sink::{Control, NodeId, Point};

/// A widget's identity, as the layer above mints it.
///
/// The same generational index every other family uses, so the staleness rule is one rule:
/// a queued intent naming a control that has since unmounted finds **nothing**, rather than
/// whatever now occupies the slot. It was a bare `u64` and therefore forgeable, and
/// interchangeable with any other id that happened to be one.
pub type ControlId = Id<Control>;

/// The index meaning "no entry".
pub const NO_ENTRY: u32 = u32::MAX;

/// What a node participates in. A bitmask rather than a set of flags on the node, because
/// the query reads all of them in one test per entry.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct HitFlags(u32);

impl HitFlags {
    /// Routes pointer events at all.
    pub const INTERACTIVE: Self = Self(1 << 0);
    /// Is a scroll container: its descendants' rects resolve through its offset.
    pub const SCROLL: Self = Self(1 << 1);
    /// Has a gesture declaration.
    pub const GESTURE: Self = Self(1 << 2);
    /// Accepts wheel input that a tracker did not already take.
    pub const WHEEL: Self = Self(1 << 3);
    /// Has an automation peer.
    pub const UIA: Self = Self(1 << 4);
    /// Is text-services editable.
    pub const TEXT: Self = Self(1 << 5);
    /// Opts out of touch inflation — a dense meter, a curve node field, anywhere
    /// inflation would make adjacent targets indistinguishable.
    pub const NO_INFLATE: Self = Self(1 << 6);
    /// Confines its descendants. Set by the builder from the node's own clip, not
    /// declared.
    pub const CLIP: Self = Self(1 << 7);
    /// Dismisses an overlay and consumes the press. The full-window entry an overlay
    /// contributes ahead of its own subtree.
    pub const BLOCKER: Self = Self(1 << 8);
    /// Chrome pinned to a scroll container's viewport: its rect does **not** resolve
    /// through that container's offset.
    ///
    /// A scrollbar's rail is the case, and it is not a special case — it is inside the
    /// thing it reports on and does not move with it, so inheriting the offset would slide
    /// the target off the surface exactly as far as the content has scrolled.
    pub const UNSCROLLED: Self = Self(1 << 9);

    pub const NONE: Self = Self(0);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for HitFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        self.union(other)
    }
}

/// One entry. `#[repr(C)]` and `Copy`: the array is scanned linearly and rides the patch.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HitEntry {
    /// Absolute layout DIPs, unscrolled — the position layout placed the node at.
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    /// DIPs added on each side for touch and pen contacts **only**.
    pub touch_inflate: f32,
    /// Index of the nearest clipping ancestor's entry, or [`NO_ENTRY`].
    pub clip_parent: u32,
    /// Index of the nearest *enclosing entry*, or [`NO_ENTRY`] for a top-level one.
    ///
    /// Structural ancestry rather than clipping ancestry, and the two are unrelated: a
    /// group that clips nothing is still a parent. Filled during the same walk, so the
    /// array carries its own tree and a consumer that needs one — automation's fragment
    /// navigation — reads it here instead of keeping a second one in step.
    pub parent: u32,
    pub flags: HitFlags,
    /// The nearest scrolling ancestor, or [`NodeId::NONE`]. A node id and not an index,
    /// because the offset it resolves through lives on the front thread with the tracker
    /// and outlives any one rebuild of this array.
    pub scroll_src: NodeId,
    pub id: ControlId,
}

impl HitEntry {
    /// Whether `p` is inside, with `inflate` DIPs added on each side.
    #[must_use]
    pub fn contains(&self, p: Point, inflate: f32) -> bool {
        p.x >= self.x0 - inflate
            && p.x <= self.x1 + inflate
            && p.y >= self.y0 - inflate
            && p.y <= self.y1 + inflate
    }

    /// Distance from `p` to the centre, squared. The tie-break when two inflated targets
    /// both claim a point.
    #[must_use]
    pub fn centre_distance_sq(&self, p: Point) -> f32 {
        let (cx, cy) = ((self.x0 + self.x1) * 0.5, (self.y0 + self.y1) * 0.5);
        (p.x - cx) * (p.x - cx) + (p.y - cy) * (p.y - cy)
    }
}

/// What a widget declares about a node. Everything else on an entry is derived.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct HitDecl {
    pub flags: HitFlags,
    pub id: ControlId,
    /// DIPs added for touch and pen, or `None` for the DPI-derived default.
    pub touch_inflate: Option<f32>,
}

/// The platform's ~9 mm touch-target guidance, in DIPs: `9 / 25.4 * 96`.
pub const TOUCH_TARGET_DIPS: f32 = 34.015_75;

/// How much to inflate a box of `w` × `h` DIPs so a finger can hit it.
///
/// Zero for a target already big enough. Half the shortfall on each side, so the inflated
/// box reaches the guidance and no further — inflating past it is what makes neighbouring
/// targets both claim a point.
#[must_use]
pub fn default_inflation(w: f32, h: f32) -> f32 {
    ((TOUCH_TARGET_DIPS - w.min(h)) * 0.5).max(0.0)
}

/// Builds the array from solved layout in paint order.
///
/// Three small stacks carry the ancestry — every entry by index, clipping entries by
/// index, scrolling nodes by id — so `parent`, `clip_parent` and `scroll_src` are filled
/// during the walk rather than by a second pass. Slot roots are appended by the caller
/// *after* the window subtree, in the order they opened, each light-dismissing overlay
/// preceded by its blocker: because the array is the z-order and the scan takes the first
/// hit from the back, that places every overlay above the content it covers and gives
/// "press outside dismisses" for free.
#[derive(Debug, Default)]
pub struct HitBuilder {
    /// (depth the entry was emitted at, its index). Every entry, not only the clipping
    /// ones, because this is the array's own tree.
    entries: Vec<(usize, u32)>,
    /// (depth the clip entered at, its entry index).
    clips: Vec<(usize, u32)>,
    /// (depth the scroll container entered at, its node).
    scrolls: Vec<(usize, NodeId)>,
}

impl HitBuilder {
    /// Starts a fresh table. The output buffer is the patch's own, so nothing intermediate
    /// is allocated.
    pub fn begin(&mut self, out: &mut Vec<HitEntry>) {
        self.entries.clear();
        self.clips.clear();
        self.scrolls.clear();
        out.clear();
    }

    /// Enters a node's subtree, after emitting its own entry if it declared one.
    ///
    /// `depth` is the node's depth in the walk; the builder pops its stacks to match, so a
    /// caller emits in paint order and states depth rather than pairing enter with exit.
    pub fn push(
        &mut self,
        out: &mut Vec<HitEntry>,
        depth: usize,
        node: NodeId,
        solved: &Solved,
        decl: Option<HitDecl>,
    ) {
        self.unwind(depth);

        if let Some(decl) = decl {
            let (w, h) = (solved.size.x, solved.size.y);
            let flags = if solved.bounded {
                decl.flags | HitFlags::CLIP
            } else {
                decl.flags
            };
            out.push(HitEntry {
                x0: solved.rect.x0,
                y0: solved.rect.y0,
                x1: solved.rect.x1,
                y1: solved.rect.y1,
                touch_inflate: decl
                    .touch_inflate
                    .unwrap_or_else(|| default_inflation(w, h)),
                clip_parent: self.clips.last().map_or(NO_ENTRY, |&(_, entry)| entry),
                parent: self.entries.last().map_or(NO_ENTRY, |&(_, entry)| entry),
                flags,
                scroll_src: if flags.contains(HitFlags::UNSCROLLED) {
                    NodeId::NONE
                } else {
                    self.scrolls.last().map_or(NodeId::NONE, |&(_, node)| node)
                },
                id: decl.id,
            });
            self.entries.push((depth, (out.len() - 1) as u32));
            if flags.contains(HitFlags::CLIP) {
                self.clips.push((depth, (out.len() - 1) as u32));
            }
            if flags.contains(HitFlags::SCROLL) {
                self.scrolls.push((depth, node));
            }
        }
    }

    /// Appends a full-window blocker ahead of an overlay that dismisses on a press
    /// outside. The press is **consumed**: dismiss-and-act would make an accidental menu
    /// open cost an unintended edit.
    pub fn blocker(&mut self, out: &mut Vec<HitEntry>, id: ControlId, window: (f32, f32)) {
        // A slot root is not inside the window subtree, so it inherits neither its clips,
        // nor its scroll offsets, nor its ancestry — and a blocker covers the window
        // whatever is under it.
        self.entries.clear();
        self.clips.clear();
        self.scrolls.clear();
        out.push(HitEntry {
            x0: 0.0,
            y0: 0.0,
            x1: window.0,
            y1: window.1,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: NO_ENTRY,
            flags: HitFlags::INTERACTIVE | HitFlags::BLOCKER,
            scroll_src: NodeId::NONE,
            id,
        });
    }

    /// Pops whatever entered at or below `depth`: an ancestor pushed at depth `d` is live
    /// exactly while something strictly deeper is being walked.
    fn unwind(&mut self, depth: usize) {
        while self.entries.last().is_some_and(|&(d, _)| d >= depth) {
            self.entries.pop();
        }
        while self.clips.last().is_some_and(|&(d, _)| d >= depth) {
            self.clips.pop();
        }
        while self.scrolls.last().is_some_and(|&(d, _)| d >= depth) {
            self.scrolls.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_target_inflates_to_the_guidance_and_no_further() {
        assert_eq!(default_inflation(40.0, 40.0), 0.0);
        let inflate = default_inflation(20.0, 20.0);
        assert!((20.0 + 2.0 * inflate - TOUCH_TARGET_DIPS).abs() < 1.0e-3);
    }

    #[test]
    fn flags_compose_and_test_as_a_set() {
        let f = HitFlags::INTERACTIVE | HitFlags::SCROLL;
        assert!(f.contains(HitFlags::INTERACTIVE));
        assert!(f.contains(HitFlags::SCROLL));
        assert!(!f.contains(HitFlags::WHEEL));
        assert!(f.intersects(HitFlags::WHEEL | HitFlags::SCROLL));
    }

    /// The walk emits at mixed depths and skips nodes that declared nothing, so ancestry
    /// is "the nearest *emitted* one" rather than "the one a level up".
    #[test]
    fn ancestry_names_the_nearest_emitted_entry_and_a_slot_root_has_none() {
        fn at(depth: usize, decl: bool) -> (usize, Option<HitDecl>) {
            (
                depth,
                decl.then_some(HitDecl {
                    flags: HitFlags::INTERACTIVE,
                    id: ControlId::default(),
                    touch_inflate: Some(0.0),
                }),
            )
        }
        let mut builder = HitBuilder::default();
        let mut out = Vec::new();
        builder.begin(&mut out);
        let solved = Solved::default();
        // root · a bare wrapper that declares nothing · its child · a sibling of the root
        for (depth, decl) in [at(0, true), at(1, false), at(2, true), at(1, true)] {
            builder.push(&mut out, depth, NodeId::NONE, &solved, decl);
        }
        builder.blocker(&mut out, ControlId::default(), (100.0, 100.0));
        builder.push(&mut out, 0, NodeId::NONE, &solved, at(0, true).1);

        assert_eq!(out[0].parent, NO_ENTRY, "the first entry has no ancestor");
        assert_eq!(out[1].parent, 0, "the wrapper is skipped, not counted");
        assert_eq!(out[2].parent, 0, "and a sibling pops back to the root");
        assert_eq!(out[3].parent, NO_ENTRY, "a blocker is not in any subtree");
        assert_eq!(out[4].parent, NO_ENTRY, "nor is the slot root after it");
    }
}
