//! Presence: a subtree that exists, or one of several, or none.

use crate::signal::Owner;

/// A subtree keyed by which arm is showing.
///
/// Both conditional forms are this one mechanism, differing only in what they key on:
///
/// - a condition is `Branch<bool>`: [`set`](Self::set) with `Some(true)` builds, with
///   `None` tears down. Absence contributes nothing — no node, no layout participation, no
///   placeholder.
/// - navigation is `Branch<Route>`: the scope is dropped and rebuilt on a key change, so a
///   screen's state is gone once its arm is torn down. State that must outlive the arm
///   lives in a cell owned by a scope above the branch, which the call site decides by
///   where it creates the cell.
///
/// The arm's scope is detached from whatever scope is running the update, as a keyed list's
/// rows are: a branch driven from an effect would otherwise register every arm it ever
/// built as a child of that effect's scope, and that list would grow for the life of the
/// screen.
pub struct Branch<K: PartialEq, V = ()> {
    // Tuple order drops the result before the scope it was built in.
    arm: Option<(K, V, Owner)>,
}

impl<K: PartialEq, V> Default for Branch<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: PartialEq, V> Branch<K, V> {
    /// Creates a branch showing nothing.
    #[must_use]
    pub fn new() -> Self {
        Self { arm: None }
    }

    /// Returns the key of the showing arm.
    #[must_use]
    pub fn key(&self) -> Option<&K> {
        self.arm.as_ref().map(|(key, _, _)| key)
    }

    /// Returns whether an arm is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.arm.is_some()
    }

    /// Retains the result of building inside a detached scope. A repeated key keeps
    /// both the result and its scope. A replacement drops the result, then its scope,
    /// before constructing the incoming arm.
    pub fn set(&mut self, key: Option<K>, build: impl FnOnce(&K) -> V) {
        if self.key() == key.as_ref() {
            return;
        }
        self.close();
        if let Some(key) = key {
            let (owner, value) = Owner::detached(|| Owner::scope(|| build(&key)));
            self.arm = Some((key, value, owner));
        }
    }

    /// Drops the retained result and its scope, showing nothing.
    pub fn close(&mut self) {
        self.arm = None;
    }
}
