//! Handing a tree across threads without either side waiting on the other.
//!
//! The same shape a presentation region publishes its parts with: a version anybody can
//! read, and the value behind a lock only the publisher and a stale reader ever take. A
//! client walking the tree does one acquire load per query and touches no lock at all;
//! the front thread takes the lock once per republish, which is once per layout change.
//!
//! The reader keeps its own reference rather than copying out of a slot the writer cycles.
//! That buys the same freedom from blocking and costs no cutover protocol to get wrong.

use super::tree::{Part, Tree};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use windows_scene::ControlId;

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
    /// Replaces the tree, and hands back what it replaced so the caller can carry the live
    /// half forward.
    pub fn publish(&self, tree: Arc<Tree>) -> Arc<Tree> {
        let previous = {
            let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
            core::mem::replace(&mut *current, tree)
        };
        // Released *after* the value, so a reader that sees the new version cannot then
        // read the old tree.
        self.version.fetch_add(1, Ordering::Release);
        previous
    }

    /// The current tree, refreshing this thread's own reference only if it has moved.
    ///
    /// A poisoned lock is stepped over rather than propagated: the tree is plain data with
    /// no invariant a panicking publisher could have broken halfway, and an accessibility
    /// client should not inherit a panic from the thread it is reading.
    pub fn read(&self, cached: &RefCell<Option<(u64, Arc<Tree>)>>) -> Arc<Tree> {
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

/// What a presentation region declares about itself, beside the tree rather than in it.
///
/// **Not in the tree, and that is the whole point.** A region's parts move whenever its
/// renderer's mapping does — a range change, a band added, a resize — and a band being
/// dragged moves them per frame. Baking them into the snapshot would make each of those a
/// republish of every element on the screen. They are also the one thing a *producer*
/// owns: the cell a region's value is read from is written by the thread that drew the
/// pixels it describes, so it outlives any one tree by construction.
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
    /// Replaces what `id` declares. `None` leaves that half alone.
    ///
    /// The parts are **copied into the row's own buffer** rather than handed over behind a
    /// pointer. A band being dragged republishes its geometry every frame, so this is the
    /// one write here that is not rare — and a row at its high-water mark makes it a
    /// memcpy under a lock nobody contends, rather than an allocation per frame.
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
        self.version.fetch_add(1, Ordering::Release);
    }

    pub fn forget(&self, id: ControlId) {
        self.rows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|row| row.id != id);
        self.version.fetch_add(1, Ordering::Release);
    }

    /// This thread's own copy, refreshed only when a declaration moved.
    ///
    /// The same bargain the tree's slot makes: a hover reads one atomic and takes no lock,
    /// and only the rare pass where the mapping actually changed pays for a refresh.
    fn refresh(&self, cached: &RefCell<(u64, Vec<Row>)>) {
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

    /// The parts `id` declared, handed to `f`. Borrowed rather than cloned, because the
    /// caller scans them and drops them.
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

    /// The value a producer writes for `id`, if it declared one.
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
                .load(Ordering::Relaxed);
            let value = f64::from_bits(bits);
            value.is_finite().then_some(value)
        })
    }
}

thread_local! {
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
