use crate::{ControlId, pack_offset, unpack_offset};
use std::sync::{Arc, atomic::{AtomicU64, Ordering::Relaxed}};
use windows_numerics::Vector2;

/// Shares a subtree's target displacement without publishing animation frames.
#[derive(Clone, Debug)]
pub struct Translation(Arc<State>);

#[derive(Debug)]
struct State {
    target: Vector2,
    value: AtomicU64,
}

impl Translation {
    /// Creates an inactive displacement. Both components must be finite.
    pub fn new(target: Vector2) -> Self {
        assert!(target.x.is_finite() && target.y.is_finite());
        Self(Arc::new(State { target, value: AtomicU64::new(0) }))
    }

    pub fn target(&self) -> Vector2 { self.0.target }

    /// Reads one coherent pair; the word publishes no other storage.
    pub fn get(&self) -> Vector2 {
        let (x, y) = unpack_offset(self.0.value.load(Relaxed));
        Vector2::new(x, y)
    }

    /// Selects the active target or zero and reports whether it changed.
    pub fn set_active(&self, active: bool) -> bool {
        let v = if active { self.0.target } else { Vector2::zero() };
        self.set(v)
    }

    /// Publishes a finite target displacement and reports whether it changed.
    pub fn set(&self, v: Vector2) -> bool {
        assert!(v.x.is_finite() && v.y.is_finite());
        let bits = pack_offset(v.x, v.y);
        // Relaxed: the complete displacement is this word; no other data is published.
        self.0.value.swap(bits, Relaxed) != bits
    }

    pub fn same(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
}

/// Associates a shared displacement with a half-open preorder range.
#[derive(Clone, Debug)]
pub struct TranslationRange {
    pub owner: ControlId,
    pub start: usize,
    pub end: usize,
    pub state: Translation,
}

impl TranslationRange {
    pub fn contains(&self, at: usize) -> bool { (self.start..self.end).contains(&at) }
}
