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
pub struct Branch<K: PartialEq> {
    key: Option<K>,
    /// Dropping this scope disposes everything the showing arm created.
    arm: Option<Owner>,
}

impl<K: PartialEq> Default for Branch<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: PartialEq> Branch<K> {
    /// Creates a branch showing nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            key: None,
            arm: None,
        }
    }

    /// Returns the key of the arm showing, or `None` where nothing is.
    #[must_use]
    pub fn key(&self) -> Option<&K> {
        self.key.as_ref()
    }

    /// Returns whether an arm is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.arm.is_some()
    }

    /// Shows `key`'s arm, tearing down whatever was showing.
    ///
    /// Nothing happens where `key` is already the one showing, so this may be called from
    /// an effect on every flush rather than only on a change. `teardown` is called with the
    /// outgoing key while its nodes still exist; `build` is called inside the new arm's own
    /// scope.
    pub fn set(&mut self, key: Option<K>, teardown: impl FnOnce(&K), build: impl FnOnce(&K)) {
        if self.key == key {
            return;
        }
        if let Some(previous) = self.key.take() {
            teardown(&previous);
            // Dropped after the caller has removed its nodes, so an exit animation still
            // has the sinks it animates.
            self.arm = None;
        }
        if let Some(key) = key {
            let (owner, ()) = Owner::detached(|| Owner::scope(|| build(&key)));
            self.arm = Some(owner);
            self.key = Some(key);
        }
    }

    /// Tears the current arm down and shows nothing.
    pub fn close(&mut self, teardown: impl FnOnce(&K)) {
        self.set(None, teardown, |_| {
            unreachable!("no arm is built when closing")
        });
    }
}
