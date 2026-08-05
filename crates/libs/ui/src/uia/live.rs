//! Holds the mutable state that sits beside an immutable published tree.
//!
//! A published tree is replaced only when structure changes. A value moves per pointer
//! sample, a toggle flips per click, a scroll offset arrives from a tracker and the window
//! moves as the user drags it, so those live here instead: allocated with the tree and
//! indexed by the same entry index, which keeps an interaction off the republish path.
//!
//! Every slot is an atomic, because the front thread writes while automation's own worker
//! threads read.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use windows_numerics::Vector2;
use windows_scene::NodeId;

/// Per-entry model state. The bits a click changes and a layout does not.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct State(pub u32);

impl State {
    pub const ENABLED: Self = Self(1 << 0);
    pub const TOGGLED: Self = Self(1 << 1);
    pub const SELECTED: Self = Self(1 << 2);
    pub const EXPANDED: Self = Self(1 << 3);

    /// Returns whether every flag in `other` is set.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns this state with the flags in `other` set or cleared according to `on`.
    #[must_use]
    pub const fn with(self, other: Self, on: bool) -> Self {
        Self(if on {
            self.0 | other.0
        } else {
            self.0 & !other.0
        })
    }
}

impl core::ops::BitOr for State {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// The mutable half of a published tree.
///
/// Every field is written by the front thread and read from anywhere. Nothing here
/// allocates after construction, so a drag writes one store per sample and a tree walk
/// reads without taking a lock.
///
/// Every load and store here is relaxed: each slot stands alone, with no other datum
/// ordered against it, so a reader takes whichever whole value is current. The tree these
/// slots index into is published under its own release-acquire pair.
#[derive(Debug)]
pub struct Live {
    /// `f64` bits per entry, `NaN` where the entry carries no value.
    values: Box<[AtomicU64]>,
    state: Box<[AtomicU32]>,
    /// One slot per scroll container, keyed by node. A handful per screen, so lookup is a
    /// linear scan.
    scrolls: Box<[(NodeId, AtomicU64)]>,
    /// The focused control as a packed generational id. Focus is singular, so it is one
    /// word, and an id rather than an index because an index is only meaningful until the
    /// next republish.
    focused: AtomicU64,
    /// The window's top-left in physical pixels, packed as two `f32`s.
    ///
    /// Automation speaks screen pixels and everything above speaks DIPs; this and
    /// [`scale`](Self::scale) convert between them. Both are written on every window move,
    /// because a stale origin reports every control at the wrong place.
    origin: AtomicU64,
    /// `dpi / 96`, as `f32` bits.
    scale: AtomicU32,
}

/// The `f64::NAN` bit pattern, standing for no value. A written value reads back only when
/// it is finite, so this cannot collide with one.
const NO_VALUE: u64 = 0x7ff8_0000_0000_0000;

impl Live {
    /// Allocates state for `len` entries and one scroll slot per node in `scrolls`.
    ///
    /// Every entry starts enabled and carrying no value.
    pub fn new(len: usize, scrolls: impl Iterator<Item = NodeId>) -> Self {
        Self {
            values: (0..len).map(|_| AtomicU64::new(NO_VALUE)).collect(),
            state: (0..len).map(|_| AtomicU32::new(State::ENABLED.0)).collect(),
            scrolls: scrolls.map(|node| (node, AtomicU64::new(0))).collect(),
            focused: AtomicU64::new(u64::MAX),
            origin: AtomicU64::new(0),
            scale: AtomicU32::new(1.0f32.to_bits()),
        }
    }

    /// Returns the value at entry `at`, or `None` where none was written, the value is not
    /// finite, or the index is past the end.
    #[must_use]
    pub fn value(&self, at: usize) -> Option<f64> {
        let bits = self.values.get(at)?.load(Relaxed);
        let value = f64::from_bits(bits);
        value.is_finite().then_some(value)
    }

    /// Stores `value` at entry `at`. An index past the end is ignored.
    pub fn set_value(&self, at: usize, value: f64) {
        if let Some(slot) = self.values.get(at) {
            slot.store(value.to_bits(), Relaxed);
        }
    }

    /// Returns the flags set at entry `at`, or none where the index is past the end.
    #[must_use]
    pub fn state(&self, at: usize) -> State {
        State(self.state.get(at).map_or(0, |s| s.load(Relaxed)))
    }

    /// Sets or clears `flag` at entry `at` according to `on`, leaving the other flags as
    /// they are. An index past the end is ignored.
    pub fn set_state(&self, at: usize, flag: State, on: bool) {
        if let Some(slot) = self.state.get(at) {
            let _ = slot.fetch_update(Relaxed, Relaxed, |bits| Some(State(bits).with(flag, on).0));
        }
    }

    /// Returns the scroll offset of `node`, or zero where it has no slot.
    #[must_use]
    pub fn scroll(&self, node: NodeId) -> Vector2 {
        match self.scrolls.iter().find(|(id, _)| *id == node) {
            Some((_, packed)) => unpack(packed.load(Relaxed)),
            None => Vector2::zero(),
        }
    }

    /// Stores the scroll offset of `node`. A node with no slot is ignored.
    pub fn set_scroll(&self, node: NodeId, offset: Vector2) {
        if let Some((_, packed)) = self.scrolls.iter().find(|(id, _)| *id == node) {
            packed.store(pack(offset), Relaxed);
        }
    }

    /// Returns the focused control as a packed generational id.
    #[must_use]
    pub fn focused(&self) -> u64 {
        self.focused.load(Relaxed)
    }

    /// Stores the focused control's packed generational id.
    pub fn set_focused(&self, id: u64) {
        self.focused.store(id, Relaxed);
    }

    /// Returns the window's client origin in physical pixels and the DIP scale there.
    #[must_use]
    pub fn window(&self) -> (Vector2, f32) {
        (
            unpack(self.origin.load(Relaxed)),
            f32::from_bits(self.scale.load(Relaxed)),
        )
    }

    /// Stores the window's client origin in physical pixels and the DIP scale there.
    pub fn set_window(&self, origin: Vector2, scale: f32) {
        self.origin.store(pack(origin), Relaxed);
        self.scale.store(scale.to_bits(), Relaxed);
    }

    /// Copies the outgoing tree's live state into this one.
    ///
    /// `mapping` yields `(old, new)` entry-index pairs; an entry with no pair keeps the
    /// state it was constructed with. Scroll offsets, focus and the window are carried
    /// whole. A republish is a layout change, which disables no control and moves no
    /// slider, so without this a resize would announce every toggle as reset.
    pub fn carry(&self, from: &Self, mapping: impl Iterator<Item = (usize, usize)>) {
        for (old, new) in mapping {
            if let (Some(src), Some(dst)) = (from.values.get(old), self.values.get(new)) {
                dst.store(src.load(Relaxed), Relaxed);
            }
            if let (Some(src), Some(dst)) = (from.state.get(old), self.state.get(new)) {
                dst.store(src.load(Relaxed), Relaxed);
            }
        }
        for (node, packed) in &self.scrolls {
            packed.store(pack(from.scroll(*node)), Relaxed);
        }
        self.focused.store(from.focused(), Relaxed);
        let (origin, scale) = from.window();
        self.set_window(origin, scale);
    }
}

const fn pack(v: Vector2) -> u64 {
    (v.x.to_bits() as u64) << 32 | v.y.to_bits() as u64
}

const fn unpack(bits: u64) -> Vector2 {
    Vector2 {
        x: f32::from_bits((bits >> 32) as u32),
        y: f32::from_bits(bits as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unwritten_value_is_absent_rather_than_zero() {
        let live = Live::new(2, core::iter::empty());
        assert_eq!(live.value(0), None, "zero is a value a slider can hold");
        live.set_value(0, -14.5);
        assert_eq!(live.value(0), Some(-14.5));
        assert_eq!(live.value(9), None, "and an index past the end is absent");
    }

    #[test]
    fn a_packed_offset_survives_the_round_trip() {
        let live = Live::new(0, [NodeId::NONE].into_iter());
        live.set_scroll(NodeId::NONE, Vector2 { x: -3.5, y: 128.25 });
        let back = live.scroll(NodeId::NONE);
        assert_eq!((back.x, back.y), (-3.5, 128.25));
    }

    #[test]
    fn carrying_forward_moves_state_to_its_new_index() {
        let old = Live::new(2, core::iter::empty());
        old.set_value(1, 7.0);
        old.set_state(1, State::TOGGLED, true);
        old.set_focused(42);

        let new = Live::new(2, core::iter::empty());
        new.carry(&old, [(1, 0)].into_iter());
        assert_eq!(new.value(0), Some(7.0));
        assert!(new.state(0).has(State::TOGGLED));
        assert_eq!(new.focused(), 42);
        assert_eq!(new.value(1), None, "and leaves what was not mapped alone");
    }
}
