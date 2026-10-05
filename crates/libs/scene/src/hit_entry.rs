//! The hit vocabulary a widget declares and the array carries. **Both halves.**
//!
//! Declared on the app side, scanned on whichever thread a contact arrives on, and carried
//! between them inside the patch, so the declaration and the entry are one vocabulary.

use crate::sink::{ControlId, NodeId, Point};

/// Marks the absence of an entry in a `clip_parent` or `parent` field.
pub const NO_ENTRY: u32 = u32::MAX;

/// Names the input device a contact came from. Only touch and pen inflate a target.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ContactKind {
    #[default]
    Mouse,
    Touch,
    Pen,
    /// A precision touchpad, which reports as a cursor and so hits the drawn rect.
    Touchpad,
}

impl ContactKind {
    /// Whether a target's touch inflation applies to this contact.
    #[must_use]
    pub const fn inflates(self) -> bool {
        matches!(self, Self::Touch | Self::Pen)
    }
}

/// Records what a node participates in. One bitmask per entry, so a query tests every kind
/// of participation with a single read.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct HitFlags(u32);

impl HitFlags {
    /// The empty set.
    pub const NONE: Self = Self(0);
    /// Routes pointer events at all.
    pub const INTERACTIVE: Self = Self(1 << 0);
    /// Is a scroll container: its descendants' rects resolve through its offset.
    pub const SCROLL: Self = Self(1 << 1);
    /// Has a gesture declaration.
    pub const GESTURE: Self = Self(1 << 2);
    /// Takes the wheel through an interaction source of its own.
    pub const WHEEL: Self = Self(1 << 3);
    /// Has an automation peer.
    pub const UIA: Self = Self(1 << 4);
    /// Is text-services editable.
    pub const TEXT: Self = Self(1 << 5);
    /// Opts out of touch inflation, where inflation would make adjacent targets
    /// indistinguishable.
    pub const NO_INFLATE: Self = Self(1 << 6);
    /// Confines its descendants. Set by the builder from the node's own clip, not declared.
    pub const CLIP: Self = Self(1 << 7);
    /// Dismisses an overlay and consumes the press.
    pub const BLOCKER: Self = Self(1 << 8);
    /// Chrome pinned to a scroll container's viewport: its rect does not resolve through
    /// that container's offset, so a rail does not slide off its own track.
    pub const UNSCROLLED: Self = Self(1 << 9);
    /// Receives hover without accepting presses or keyboard focus. Set by the builder.
    pub const HOVER: Self = Self(1 << 10);

    /// Every flag this vocabulary defines. The bound [`from_bits`](Self::from_bits) admits.
    const ALL: u32 = (1 << 11) - 1;

    /// Whether every flag in `other` is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any flag in `other` is set.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// The flags set in either.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// The raw bitmask.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// The set `bits` names, dropping every bit this vocabulary does not define.
    ///
    /// The inverse of [`bits`](Self::bits) over everything constructible, and fail-closed
    /// past it: a caller unpacking a set out of a wider word cannot introduce a flag whose
    /// meaning nothing here states.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits & Self::ALL)
    }
}

impl core::ops::BitOr for HitFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        self.union(other)
    }
}

/// Describes one node's participation in the hit array.
///
/// `#[repr(C)]` and `Copy`: the array is scanned linearly and rides the patch.
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
    /// Structural ancestry rather than clipping ancestry, and the two are unrelated: a group
    /// that clips nothing is still a parent. Automation's fragment navigation reads the
    /// array's own tree here rather than keeping a second one in step.
    pub parent: u32,
    pub flags: HitFlags,
    /// The nearest scrolling ancestor, or [`NodeId::NONE`]. A node id and not an index,
    /// because the offset it resolves through lives with the tracker and outlives any one
    /// rebuild of this array.
    pub scroll_src: NodeId,
    pub id: ControlId,
}

impl HitEntry {
    /// Whether `p` is inside the box, with `inflate` DIPs added on each side.
    #[must_use]
    pub fn contains(&self, p: Point, inflate: f32) -> bool {
        p.x >= self.x0 - inflate
            && p.x <= self.x1 + inflate
            && p.y >= self.y0 - inflate
            && p.y <= self.y1 + inflate
    }

    /// The squared distance from `p` to the box's centre. The tie-break when two inflated
    /// targets both claim a point.
    #[must_use]
    pub fn centre_distance_sq(&self, p: Point) -> f32 {
        let (cx, cy) = ((self.x0 + self.x1) * 0.5, (self.y0 + self.y1) * 0.5);
        (p.x - cx) * (p.x - cx) + (p.y - cy) * (p.y - cy)
    }
}

/// What a widget declares. Every other field of an entry is derived during the build.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct HitDecl {
    pub flags: HitFlags,
    pub id: ControlId,
    /// DIPs added for touch and pen, or `None` for the size-derived default.
    pub touch_inflate: Option<f32>,
}

/// The platform's ~9 mm touch-target guidance, in DIPs: `9 / 25.4 * 96`.
pub const TOUCH_TARGET_DIPS: f32 = 34.015_75;

/// The DIPs to add on each side of a `w` × `h` box so a finger can hit it.
///
/// Zero where the shorter side already reaches [`TOUCH_TARGET_DIPS`]; otherwise half the
/// shortfall on each side, so the inflated box reaches the guidance and no further. Past it,
/// neighbouring targets start claiming the same point.
#[must_use]
pub fn default_inflation(w: f32, h: f32) -> f32 {
    ((TOUCH_TARGET_DIPS - w.min(h)) * 0.5).max(0.0)
}

/// Identifies the entry a query resolved to.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Hit {
    /// Index into the array. Joins to any table a consumer keeps in parallel.
    pub index: u32,
    pub id: ControlId,
    pub flags: HitFlags,
    /// The point in the target's own space, with its scroll ancestry applied.
    pub local: Point,
}

/// Packs an offset into the word a tracker shadow holds: `x` in the high half, `y` in the
/// low half.
///
/// The whole offset is one word, so a reader sees both axes of one reported position and
/// never one axis of two.
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

    #[test]
    fn packed_flags_round_trip_and_an_undefined_bit_cannot_enter_the_set() {
        let every = [
            HitFlags::NONE,
            HitFlags::INTERACTIVE,
            HitFlags::SCROLL,
            HitFlags::GESTURE,
            HitFlags::WHEEL,
            HitFlags::UIA,
            HitFlags::TEXT,
            HitFlags::NO_INFLATE,
            HitFlags::CLIP,
            HitFlags::BLOCKER,
            HitFlags::UNSCROLLED,
            HitFlags::HOVER,
        ];
        let all = every.into_iter().fold(HitFlags::NONE, HitFlags::union);
        assert_eq!(HitFlags::from_bits(all.bits()), all);
        for flag in every {
            assert_eq!(HitFlags::from_bits(flag.bits()), flag);
        }
        assert_eq!(HitFlags::from_bits(u32::MAX), all);
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
}
