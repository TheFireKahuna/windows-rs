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

    /// Refreshes `out` from the published value, for the same pass [`get`](Self::get) is
    /// called on.
    ///
    /// `out` is the reader's own, so a `T` whose `clone_from` reuses what it already holds
    /// keeps those buffers rather than taking fresh ones.
    pub fn read_into(&self, out: &mut T) {
        match self.value.lock() {
            Ok(held) => out.clone_from(&held),
            Err(poisoned) => out.clone_from(&poisoned.into_inner()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Published;
    use std::sync::Arc;

    /// A reader sees the version move with the value, and both reads answer the same one.
    ///
    /// The version is what every reader gates on, so a value published without it moving
    /// is a frame drawn against the value before it.
    #[test]
    fn a_value_published_from_another_thread_arrives_with_its_version() {
        let published = Arc::new(Published::new(Vec::<u32>::new()));
        let seq = published.seq();
        let producer = Arc::clone(&published);
        std::thread::spawn(move || producer.set(vec![4, 5, 6]))
            .join()
            .expect("the producer finished");
        assert_ne!(published.seq(), seq, "the version stood still");
        assert_eq!(published.get(), [4, 5, 6]);

        let mut out = Vec::with_capacity(8);
        let held = out.as_ptr();
        published.read_into(&mut out);
        assert_eq!(out, [4, 5, 6]);
        assert_eq!(out.as_ptr(), held, "the reader's buffer was replaced");
    }
}
