//! Publishes the accessibility tree and the region declarations across threads.
//!
//! Each holds a version any thread can read and a value behind a lock. A reader compares
//! the version against the copy it already holds and takes the lock only when the version
//! moved, so a query against an unchanged tree is one acquire load. The front thread takes
//! the lock once per publish, which is once per layout change.
//!
//! A reader keeps its own `Arc` rather than copying out of a slot the writer cycles, so
//! there is no cutover window for a reader to land in.

use super::tree::{Part, Tree};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use windows_scene::ControlId;

/// The published tree, with the version a reader checks before taking the lock.
#[derive(Debug)]
pub struct Slot {
    version: AtomicU64,
    current: Mutex<Arc<Tree>>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            version: AtomicU64::new(0),
            current: Mutex::new(Arc::new(Tree::empty())),
        }
    }
}

impl Slot {
    /// Replaces the published tree and returns the one it replaced, so the caller can carry
    /// its live half forward.
    pub fn publish(&self, tree: Arc<Tree>) -> Arc<Tree> {
        let previous = {
            let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
            core::mem::replace(&mut *current, tree)
        };
        // release: pairs with the acquire in `read`. Bumped after the value is installed,
        // so a reader that sees this version finds the new tree under the lock.
        self.version.fetch_add(1, Ordering::Release);
        previous
    }

    /// Returns the current tree, refreshing `cached` only when the version has moved.
    ///
    /// `cached` is this thread's own reference and the version it was taken at. A poisoned
    /// lock is stepped over rather than propagated: the tree is plain data, and a publisher
    /// that panicked left either the outgoing `Arc` or the incoming one, never a half-built
    /// tree.
    pub fn read(&self, cached: &RefCell<Option<(u64, Arc<Tree>)>>) -> Arc<Tree> {
        // acquire: pairs with the release in `publish`, so a version this observes as new
        // means the tree behind the lock is the one it names.
        let version = self.version.load(Ordering::Acquire);
        if let Some((seen, tree)) = cached.borrow().as_ref()
            && *seen == version
        {
            return Arc::clone(tree);
        }
        let tree = Arc::clone(&*self.current.lock().unwrap_or_else(|e| e.into_inner()));
        *cached.borrow_mut() = Some((version, Arc::clone(&tree)));
        tree
    }
}

/// What each presentation region declares about itself, held beside the tree.
///
/// A region's parts move whenever its renderer's mapping does — a range change, a band
/// added, a resize, every frame of a drag — so they are versioned separately here and a
/// change to them republishes no tree. The value cell is the producer's: it is written by
/// the thread that drew the pixels it describes and outlives any one tree.
#[derive(Debug, Default)]
pub struct Regions {
    version: AtomicU64,
    rows: Mutex<Vec<Row>>,
}

#[derive(Clone, Debug, Default)]
struct Row {
    id: ControlId,
    parts: Vec<Part>,
    value: Option<Arc<AtomicU64>>,
}

impl Regions {
    /// Replaces what `id` declares, adding the row on first use.
    ///
    /// A `None` argument leaves that half of the declaration as it was. The parts are
    /// copied into the row's own buffer rather than handed over behind a pointer, so a row
    /// at its high-water mark takes a memcpy under an uncontended lock rather than an
    /// allocation per frame of a drag.
    pub fn declare(&self, id: ControlId, parts: Option<&[Part]>, value: Option<Arc<AtomicU64>>) {
        let mut rows = self.rows.lock().unwrap_or_else(|e| e.into_inner());
        if !rows.iter().any(|row| row.id == id) {
            rows.push(Row {
                id,
                ..Row::default()
            });
        }
        let row = rows
            .iter_mut()
            .find(|row| row.id == id)
            .expect("present or just pushed");
        if let Some(parts) = parts {
            row.parts.clear();
            row.parts.extend_from_slice(parts);
        }
        if value.is_some() {
            row.value = value;
        }
        // release: pairs with the acquire in `refresh`, so a reader that sees this version
        // finds the declaration it names under the lock.
        self.version.fetch_add(1, Ordering::Release);
    }

    /// Drops what `id` declares, so nothing is published for it.
    pub fn forget(&self, id: ControlId) {
        self.rows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|row| row.id != id);
        // release: pairs with the acquire in `refresh`, so a reader that sees this version
        // finds the row gone.
        self.version.fetch_add(1, Ordering::Release);
    }

    /// Refreshes this thread's copy of the declarations, and only when the version moved.
    ///
    /// A pass with nothing declared since the last one reads one atomic and takes no lock.
    /// A refresh reuses each row's buffers rather than reallocating them.
    fn refresh(&self, cached: &RefCell<(u64, Vec<Row>)>) {
        // acquire: pairs with the release in `declare` and `forget`, so a version this
        // observes as new means the rows behind the lock are the ones it names.
        let version = self.version.load(Ordering::Acquire);
        if cached.borrow().0 == version {
            return;
        }
        let held = self.rows.lock().unwrap_or_else(|e| e.into_inner());
        let mut cached = cached.borrow_mut();
        cached.1.resize_with(held.len(), Row::default);
        for (into, from) in cached.1.iter_mut().zip(held.iter()) {
            into.id = from.id;
            into.value = from.value.clone();
            into.parts.clear();
            into.parts.extend_from_slice(&from.parts);
        }
        cached.0 = version;
    }

    /// Calls `f` with the parts `id` declared, or with an empty slice where it declared
    /// none. The parts are borrowed from this thread's copy rather than cloned.
    pub fn with_parts<R>(&self, id: ControlId, f: impl FnOnce(&[Part]) -> R) -> R {
        SEEN_REGIONS.with(|cached| {
            self.refresh(cached);
            let rows = cached.borrow();
            let parts = rows
                .1
                .iter()
                .find(|row| row.id == id)
                .map_or(&[] as &[Part], |row| &row.parts);
            f(parts)
        })
    }

    /// Returns the value a producer wrote for `id`, or `None` where it declared no cell or
    /// the cell holds no finite number.
    pub fn value(&self, id: ControlId) -> Option<f64> {
        SEEN_REGIONS.with(|cached| {
            self.refresh(cached);
            let rows = cached.borrow();
            let bits = rows
                .1
                .iter()
                .find(|row| row.id == id)?
                .value
                .as_ref()?
                // relaxed: the cell stands alone, with no other datum ordered against it,
                // so a reader takes whichever whole value is current.
                .load(Ordering::Relaxed);
            let value = f64::from_bits(bits);
            value.is_finite().then_some(value)
        })
    }
}

thread_local! {
    /// This thread's copy of the declarations, with the version it was taken at.
    static SEEN_REGIONS: RefCell<(u64, Vec<Row>)> = const { RefCell::new((0, Vec::new())) };
}

#[cfg(test)]
mod tests {
    use super::*;

    thread_local! {
        static CACHE: RefCell<Option<(u64, Arc<Tree>)>> = const { RefCell::new(None) };
    }

    #[test]
    fn a_reader_sees_a_publish_and_reuses_its_own_reference_until_then() {
        let slot = Slot::default();
        let first = CACHE.with(|c| slot.read(c));
        assert!(Arc::ptr_eq(&first, &CACHE.with(|c| slot.read(c))));

        slot.publish(Arc::new(Tree::empty()));
        let second = CACHE.with(|c| slot.read(c));
        assert!(!Arc::ptr_eq(&first, &second), "a publish is observed");
    }

    #[test]
    fn publishing_hands_back_what_it_replaced() {
        let slot = Slot::default();
        let first = CACHE.with(|c| slot.read(c));
        let replaced = slot.publish(Arc::new(Tree::empty()));
        assert!(Arc::ptr_eq(&first, &replaced));
    }
}
