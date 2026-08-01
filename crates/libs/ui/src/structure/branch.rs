//! Presence: a subtree that exists, or one of several, or none.

use crate::signal::Owner;

/// A subtree keyed by which arm is showing.
///
/// This is the mechanism behind both conditional forms, and they differ only in what they
/// key on:
///
/// - a condition is `Branch<bool>` — `set(Some(true), ..)` builds, `set(None, ..)` tears
///   down. **Absence contributes nothing**: no node, no layout participation, no hidden
///   placeholder. That is what replaces an empty-element ternary at every call site.
/// - navigation is `Branch<Route>` — the scope is dropped and rebuilt on a key change, so
///   a screen's state is genuinely gone when you navigate away. When that is *not* wanted,
///   the state lives in a cell owned by a scope above the branch, which is a decision the
///   call site makes by where it puts the cell rather than by a flag here.
///
/// The arm's scope is **detached** from whatever scope is running the update, for the same
/// reason a keyed list's rows are: a branch driven from an effect would otherwise register
/// every arm it ever built as a child of that effect's scope, and that list would grow for
/// the life of the screen.
pub struct Branch<K: PartialEq> {
    key: Option<K>,
    /// Dropping this disposes the arm. The whole teardown story.
    arm: Option<Owner>,
}

impl<K: PartialEq> Default for Branch<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: PartialEq> Branch<K> {
    /// A branch showing nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            key: None,
            arm: None,
        }
    }

    /// Which arm is showing.
    #[must_use]
    pub fn key(&self) -> Option<&K> {
        self.key.as_ref()
    }

    /// Whether anything is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.arm.is_some()
    }

    /// Shows `key`'s arm, tearing down whatever was showing.
    ///
    /// Nothing happens if `key` is already the one showing — which is what makes this
    /// callable from an effect on every flush rather than only on a change. `teardown` is
    /// called with the outgoing key while its nodes still exist; `build` is called inside
    /// the new arm's own scope.
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
