use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

/// A value one thread publishes and a renderer reads by version.
///
/// The reader compares [`seq`](Self::seq) first and takes the lock only on the pass where it
/// moved, so a frame that draws the same value as the last one does one acquire load and
/// touches no lock. The pattern is the hero feed's.
pub struct Published<T> {
    seq: AtomicU64,
    value: Mutex<T>,
}

impl<T: Clone> Published<T> {
    pub fn new(value: T) -> Self {
        Self {
            seq: AtomicU64::new(0),
            value: Mutex::new(value),
        }
    }

    /// Publishes `value`. Callable from any thread.
    pub fn set(&self, value: T) {
        *self.value.lock().unwrap_or_else(|p| p.into_inner()) = value;
        // release: pairs with the acquire in `seq`, so the value is in place before the
        // version advertising it becomes visible.
        self.seq.fetch_add(1, Ordering::Release);
    }

    /// Returns the published version. No kernel call and no lock.
    #[must_use]
    pub fn seq(&self) -> u64 {
        // acquire: pairs with the release in `set`.
        self.seq.load(Ordering::Acquire)
    }

    /// Returns the published value. Called only on a pass where [`seq`](Self::seq) moved.
    #[must_use]
    pub fn get(&self) -> T {
        self.value
            .lock()
            .map(|held| held.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }
}
