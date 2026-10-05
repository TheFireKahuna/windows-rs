//! Keyboard focus and the scopes that bound it.
//!
//! Focus order is the hit array's order, filtered to `INTERACTIVE`, with `tab_index` as an
//! explicit override. The pointer, keyboard focus order, the window's caption hit
//! test and automation's element-from-point all resolve through that one z-ordered flat array,
//! so no second ordering is maintained beside it.
//!
//! Scopes nest. An open overlay opens one, so `Tab` cycles within it and closing it restores
//! focus to whatever invoked it; `Esc` is delivered to the innermost scope before any control
//! sees it. A scope is named by the entry its subtree begins at rather than by an index range,
//! so it survives the array being rebuilt underneath it.
//!
//! The overlay stack reaches none of this. It emits [`FocusOp`] rows naming what it opened,
//! closed or moved, and [`FocusRing::apply`] is the one place they land, which is what lets
//! the two halves run on separate threads.

use crate::seam::FocusOp;
use rustc_hash::FxHashMap;
use windows_scene::{ControlId, HitFlags, HitTable};

/// Identifies one open focus scope.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScopeId(pub u32);

/// One open scope: where its subtree begins, whether `Tab` may leave it, and what focus
/// returns to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Scope {
    id: ScopeId,
    /// Wraps `Tab` at the ends instead of letting it leave. A modal popup sets this; a flyout
    /// leaves it clear, so tabbing past its last item dismisses it.
    trap: bool,
    /// `None` leaves focus cleared, so the next keystroke reaches the window rather than a
    /// control.
    restore_to: Option<ControlId>,
    /// The scope's first entry in the hit array: its blocker for a light-dismissing overlay,
    /// or its root otherwise. Everything at or after it in the array is inside the scope.
    from: ControlId,
}

/// What a focus step did.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Move {
    /// Focus moved.
    To,
    /// The step ran off the end of a scope that does not trap. The caller dismisses that scope
    /// and steps again outside it.
    Left,
    /// Nothing focusable, or focus was already where the step would put it.
    #[default]
    None,
}

/// Retains control focus and its navigation scopes separately from window keyboard focus.
#[derive(Debug, Default)]
pub struct FocusRing {
    current: Option<ControlId>,
    window_focused: bool,
    scopes: Vec<Scope>,
    /// Explicit tab positions, keyed by control. Empty for every screen that does not override
    /// the hit array's order.
    order: FxHashMap<ControlId, i32>,
    /// Candidate buffer reused across navigations, so moving focus allocates nothing after the
    /// first move.
    scratch: Vec<(i32, ControlId)>,
}

impl FocusRing {
    /// Returns the remembered control, including while the window lacks keyboard focus.
    #[must_use]
    pub const fn current(&self) -> Option<ControlId> {
        self.current
    }

    /// Returns the control receiving keyboard input while the window has focus.
    #[must_use]
    pub const fn keyboard(&self) -> Option<ControlId> {
        if self.window_focused {
            self.current
        } else {
            None
        }
    }

    pub(crate) const fn window_focused(&self) -> bool {
        self.window_focused
    }

    pub(crate) fn window_focus(&mut self, focused: bool, hits: &HitTable) {
        self.window_focused = focused;
        self.validate(hits);
    }

    pub(crate) fn validate(&mut self, hits: &HitTable) {
        let outside = self.scopes.last().is_some_and(|scope| {
            let entries = hits.entries();
            entries.iter().position(|entry| entry.id == scope.from).is_none_or(|start| {
                !entries[start..].iter().any(|entry| Some(entry.id) == self.current)
            })
        });
        if outside || self.current.is_some_and(|id| {
            hits.entry(id).is_none_or(|entry| {
                !entry.flags.contains(HitFlags::INTERACTIVE)
                    || entry.flags.contains(HitFlags::BLOCKER)
            })
        }) {
            self.current = None;
        }
        // An entering overlay can publish its controls after its focus scope.
        if self.current.is_none() && self.scopes.last().is_some() {
            self.enter(hits, false);
        }
    }

    pub(crate) fn report(&self, from: Option<ControlId>, out: &mut Vec<super::Report>) {
        let to = self.keyboard();
        if from != to {
            out.push(super::Report::FocusChanged { from, to });
        }
    }

    /// Returns the innermost open scope, which `Esc` is delivered to before any control.
    #[must_use]
    pub fn innermost(&self) -> Option<ScopeId> {
        self.scopes.last().map(|scope| scope.id)
    }

    /// Returns how many scopes are open.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Gives `id` an explicit position in the focus order.
    ///
    /// A position above zero sorts ahead of every control without one; the rest keep the hit
    /// array's order.
    pub fn set_tab_index(&mut self, id: ControlId, index: i32) {
        self.order.insert(id, index);
    }

    /// Drops `id`'s explicit position, returning it to the hit array's order. Called on
    /// unmount.
    pub fn forget(&mut self, id: ControlId) {
        self.order.remove(&id);
    }

    /// Focuses `next` directly, as a press does.
    ///
    /// Returns whether focus moved, so a caller repaints the ring only when it did.
    pub fn focus(&mut self, next: Option<ControlId>) -> bool {
        let moved = self.current != next;
        self.current = next;
        moved
    }

    /// Applies the focus edits the app half emitted, in the order it emitted them, and returns
    /// whether any of them moved focus.
    ///
    /// Every op changes exactly one datum, so one read before the loop and one comparison
    /// after it reports whether the batch moved focus.
    pub(crate) fn apply(&mut self, ops: &[FocusOp], hits: &HitTable) -> bool {
        let before = self.current;
        for op in ops {
            match *op {
                FocusOp::TabIndex(id, index) => self.set_tab_index(id, index),
                FocusOp::Push {
                    id,
                    trap,
                    from,
                    restore_to,
                } => {
                    self.scopes.push(Scope {
                        id,
                        trap,
                        // An overlay anchored to a control names that control. One anchored to a
                        // point or to the window names none, and what it interrupted is known
                        // only here.
                        restore_to: restore_to.or(self.current),
                        // A scope naming no entry collects nothing, which is the same fail-closed
                        // answer as one whose entry has left the array: `ControlId::NONE` is never
                        // an entry's id.
                        from: from.unwrap_or(ControlId::NONE),
                    });
                    self.current = None;
                }
                FocusOp::Pop(scope) => self.pop(scope),
                FocusOp::Focus(next) => {
                    self.focus(next);
                }
                FocusOp::Step { forward } => {
                    self.step(hits, forward);
                }
                FocusOp::End { last } => self.enter(hits, last),
            }
        }
        before != self.current
    }

    /// Closes the scope `id` and moves focus to its `restore_to`. Does nothing where `id`
    /// names no open scope.
    pub fn pop(&mut self, id: ScopeId) {
        let Some(at) = self.scopes.iter().position(|scope| scope.id == id) else {
            return;
        };
        // Truncating takes the nested scopes with it: closing an overlay closes its submenus.
        self.current = self.scopes[at].restore_to;
        self.scopes.truncate(at);
    }

    /// Moves focus one step through the order, forwards when `forward` is set.
    ///
    /// With nothing focused, the step enters the scope at whichever end it came from.
    /// [`Move::Left`] means the step ran off the end of a scope that does not trap, which the
    /// caller answers by dismissing that scope and stepping again outside it.
    pub fn step(&mut self, hits: &HitTable, forward: bool) -> Move {
        self.collect(hits);
        let Some(last) = self.scratch.len().checked_sub(1) else {
            return Move::None;
        };
        let at = self
            .current
            .and_then(|id| self.scratch.iter().position(|(_, e)| *e == id));
        let next = match (at, forward) {
            (None, true) => 0,
            (None, false) => last,
            (Some(at), true) if at < last => at + 1,
            (Some(at), false) if at > 0 => at - 1,
            // Off the end: a trapping scope wraps, a flyout lets go.
            _ if !self.scopes.last().is_some_and(|scope| scope.trap) => return Move::Left,
            (_, true) => 0,
            (_, false) => last,
        };
        let id = self.scratch[next].1;
        if self.focus(Some(id)) {
            Move::To
        } else {
            Move::None
        }
    }

    /// Moves focus to the last control of the innermost scope when `last` is set, and to the
    /// first otherwise. Drives `Home` and `End`.
    pub fn enter(&mut self, hits: &HitTable, last: bool) {
        self.collect(hits);
        let end = if last {
            self.scratch.last()
        } else {
            self.scratch.first()
        };
        if let Some(&(_, id)) = end {
            self.focus(Some(id));
        }
    }

    /// Fills `scratch` with the innermost scope's focusable controls, in focus order.
    ///
    /// Explicit indices above zero sort first and among themselves; everything else keeps the
    /// hit array's own order, which is the reading order layout produced. The sort is stable,
    /// which is what preserves that order.
    fn collect(&mut self, hits: &HitTable) {
        self.scratch.clear();
        let entries = hits.entries();
        // A scope begins at its own first entry, so everything before that is outside it.
        //
        // A scope whose entry is absent from the array has no subtree in it either: it has
        // closed, or it has not been flushed yet. Both mean nothing is in scope, so this
        // leaves the candidates empty rather than falling back to the head of the array —
        // that fallback widens a stale scope to the whole window and lets `Tab` walk out of a
        // modal.
        let start = match self.scopes.last() {
            Some(scope) => match entries.iter().position(|entry| entry.id == scope.from) {
                Some(at) => at,
                None => return,
            },
            None => 0,
        };
        for entry in &entries[start..] {
            // A blocker routes a press rather than holding focus, and an entry that is not
            // interactive — a scroll viewport carrying no control of its own — is not a focus
            // stop.
            if entry.flags.contains(HitFlags::INTERACTIVE)
                && !entry.flags.contains(HitFlags::BLOCKER)
            {
                let index = self.order.get(&entry.id).copied().unwrap_or(0);
                if index < 0 && self.current != Some(entry.id) { continue; }
                self.scratch.push((index.max(0), entry.id));
            }
        }
        self.scratch
            .sort_by_key(|(index, _)| if *index > 0 { (0, *index) } else { (1, 0) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_scene::{HitEntry, NO_ENTRY, NodeId};

    /// Returns the id at row `n` of the first generation, which is what an authority mints
    /// `n`th.
    fn cid(n: u32) -> ControlId {
        ControlId::raw(n, 1)
    }

    fn entry(id: u32, flags: HitFlags) -> HitEntry {
        HitEntry {
            x0: 0.0,
            y0: 0.0,
            x1: 10.0,
            y1: 10.0,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: NO_ENTRY,
            flags,
            scroll_src: NodeId::NONE,
            id: cid(id),
        }
    }

    fn table(ids: &[u32]) -> HitTable {
        let entries: Vec<HitEntry> = ids
            .iter()
            .map(|id| entry(*id, HitFlags::INTERACTIVE))
            .collect();
        table_of(&entries)
    }

    /// Returns a table over `entries`, with the id index the array ships beside them.
    fn table_of(entries: &[HitEntry]) -> HitTable {
        let mut table = HitTable::default();
        let mut index: Vec<(ControlId, u32)> = entries
            .iter()
            .enumerate()
            .map(|(at, entry)| (entry.id, at as u32))
            .collect();
        index.sort_unstable_by_key(|&(id, _)| id);
        table.replace(entries, &index);
        table
    }

    /// Opens a scope beginning at `from`, as an overlay's blocker does.
    fn push(ring: &mut FocusRing, scope: u32, trap: bool, restore_to: Option<ControlId>) {
        ring.scopes.push(Scope {
            id: ScopeId(scope),
            trap,
            restore_to,
            from: cid(9),
        });
    }

    #[test]
    fn window_focus_preserves_the_control_and_reports_only_keyboard_transitions() {
        let hits = table(&[1, 2]);
        let mut ring = FocusRing::default();
        let mut reports = Vec::new();
        ring.focus(Some(cid(1)));
        assert_eq!(ring.keyboard(), None);
        for (active, from, to) in [
            (true, None, Some(cid(1))),
            (false, Some(cid(1)), None),
            (true, None, Some(cid(1))),
        ] {
            assert_eq!(ring.keyboard(), from);
            ring.window_focus(active, &hits);
            ring.report(from, &mut reports);
            assert_eq!(reports.pop(), Some(super::super::Report::FocusChanged { from, to }));
            assert_eq!(ring.current(), Some(cid(1)));
            ring.window_focus(active, &hits);
            ring.report(to, &mut reports);
            assert!(reports.is_empty());
        }
    }

    #[test]
    fn background_scope_changes_do_not_claim_keyboard_focus() {
        let hits = table(&[1, 10]);
        let mut ring = FocusRing::default();
        let mut reports = Vec::new();
        ring.focus(Some(cid(1)));
        push(&mut ring, 1, true, Some(cid(1)));
        ring.focus(Some(cid(10)));
        ring.window_focus(true, &hits);
        ring.window_focus(false, &hits);
        ring.apply(&[FocusOp::Pop(ScopeId(1))], &hits);
        ring.report(None, &mut reports);
        assert!(reports.is_empty());
        assert_eq!(ring.current(), Some(cid(1)));
        ring.window_focus(true, &hits);
        assert_eq!(ring.keyboard(), Some(cid(1)));
    }

    #[test]
    fn scope_entry_waits_for_hits_and_rejects_background_focus() {
        let background = table(&[1, 2]);
        let visible = table_of(&[
            entry(1, HitFlags::INTERACTIVE),
            entry(9, HitFlags::INTERACTIVE | HitFlags::BLOCKER),
            entry(10, HitFlags::INTERACTIVE),
            entry(11, HitFlags::INTERACTIVE),
        ]);
        let mut ring = FocusRing::default();
        ring.focus(Some(cid(1)));
        ring.apply(&[FocusOp::Push {
            id: ScopeId(1), trap: true, from: Some(cid(9)), restore_to: None,
        }], &background);
        ring.validate(&background);
        assert_eq!(ring.current(), None);
        ring.validate(&visible);
        assert_eq!(ring.current(), Some(cid(10)));
        ring.focus(Some(cid(1)));
        ring.validate(&visible);
        assert_eq!(ring.current(), Some(cid(10)));
        ring.focus(Some(cid(11)));
        ring.validate(&visible);
        assert_eq!(ring.current(), Some(cid(11)));
        ring.validate(&background);
        assert_eq!(ring.current(), None);
        ring.apply(&[FocusOp::Pop(ScopeId(1))], &background);
        ring.validate(&background);
        assert_eq!(ring.current(), Some(cid(1)));
        ring.focus(None);
        ring.validate(&background);
        assert_eq!(ring.current(), None);
    }

    #[test]
    fn restoration_rejects_removed_disabled_and_reused_controls() {
        for hits in [table(&[2]), table_of(&[entry(1, HitFlags::from_bits(0))])] {
            let mut ring = FocusRing::default();
            ring.focus(Some(cid(1)));
            ring.window_focus(true, &hits);
            assert_eq!(ring.current(), None);
            assert_eq!(ring.keyboard(), None);
        }
        let mut ring = FocusRing::default();
        ring.focus(Some(ControlId::raw(1, 2)));
        ring.window_focus(true, &table(&[1]));
        assert_eq!(ring.keyboard(), None);
    }

    #[test]
    fn focus_order_is_the_hit_arrays_order() {
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        assert_eq!(ring.step(&hits, true), Move::To);
        assert_eq!(ring.current(), Some(cid(1)));
        assert_eq!(ring.step(&hits, true), Move::To);
        assert_eq!(ring.current(), Some(cid(2)));
        assert_eq!(ring.step(&hits, false), Move::To);
        assert_eq!(ring.current(), Some(cid(1)));
    }

    #[test]
    fn a_scope_whose_entry_has_gone_bounds_navigation_to_nothing() {
        // A scope is named by its own first entry. If that entry is absent from the array, its
        // subtree is absent too — the scope closed, or it has not been flushed yet — so
        // navigation is bounded to nothing. Resolving to the head of the array instead would
        // widen a stale scope to the whole window and let `Tab` walk out of a modal.
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        ring.scopes.push(Scope {
            id: ScopeId(1),
            trap: true,
            restore_to: None,
            from: cid(99),
        });
        assert_eq!(ring.step(&hits, true), Move::None);
        ring.enter(&hits, false);
        assert_eq!(ring.current(), None, "and nothing was focused on the way");
    }

    #[test]
    fn an_explicit_index_sorts_ahead_of_everything_unstated() {
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        ring.set_tab_index(cid(3), 1);
        assert_eq!(ring.step(&hits, true), Move::To);
        assert_eq!(ring.current(), Some(cid(3)));
        // …and the rest keep the array's own order behind it.
        assert_eq!(ring.step(&hits, true), Move::To);
        assert_eq!(ring.current(), Some(cid(1)));
        // Unmounting the control returns it to the array's order.
        ring.forget(cid(3));
        ring.focus(None);
        assert_eq!(ring.step(&hits, true), Move::To);
        assert_eq!(ring.current(), Some(cid(1)));
    }

    #[test]
    fn a_negative_index_skips_tab_entry_but_can_leave_direct_focus() {
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        ring.set_tab_index(cid(2), -1);
        ring.focus(Some(cid(1)));
        assert_eq!(ring.step(&hits, true), Move::To);
        assert_eq!(ring.current(), Some(cid(3)));
        ring.focus(Some(cid(2)));
        assert_eq!(ring.step(&hits, true), Move::To);
        assert_eq!(ring.current(), Some(cid(3)));
        ring.focus(Some(cid(2)));
        assert_eq!(ring.step(&hits, false), Move::To);
        assert_eq!(ring.current(), Some(cid(1)));
    }

    #[test]
    fn a_trapping_scope_wraps_and_a_flyout_lets_go() {
        let hits = table_of(&[
            entry(1, HitFlags::INTERACTIVE),
            entry(9, HitFlags::INTERACTIVE | HitFlags::BLOCKER),
            entry(10, HitFlags::INTERACTIVE),
            entry(11, HitFlags::INTERACTIVE),
        ]);

        let mut popup = FocusRing::default();
        push(&mut popup, 1, true, Some(cid(1)));
        // The blocker is not a focus stop, so the scope is exactly its two items.
        assert_eq!(popup.step(&hits, true), Move::To);
        assert_eq!(popup.current(), Some(cid(10)));
        assert_eq!(popup.step(&hits, true), Move::To);
        assert_eq!(popup.current(), Some(cid(11)));
        assert_eq!(popup.step(&hits, true), Move::To);
        assert_eq!(
            popup.current(),
            Some(cid(10)),
            "a trapping scope let focus out"
        );

        let mut flyout = FocusRing::default();
        push(&mut flyout, 1, false, Some(cid(1)));
        _ = flyout.step(&hits, true);
        _ = flyout.step(&hits, true);
        assert_eq!(
            flyout.step(&hits, true),
            Move::Left,
            "tabbing past the last item should dismiss a flyout rather than wrap"
        );
        // Backwards off the front is the same answer, from the one guard.
        flyout.focus(Some(cid(10)));
        assert_eq!(flyout.step(&hits, false), Move::Left);
    }

    #[test]
    fn closing_a_scope_restores_focus_to_the_invoker() {
        let mut ring = FocusRing::default();
        ring.focus(Some(cid(1)));
        push(&mut ring, 1, true, Some(cid(1)));
        ring.focus(Some(cid(10)));
        ring.pop(ScopeId(1));
        assert_eq!(ring.current(), Some(cid(1)));
        assert_eq!(ring.innermost(), None);
    }

    #[test]
    fn closing_a_scope_takes_everything_nested_inside_it() {
        let mut ring = FocusRing::default();
        push(&mut ring, 1, false, Some(cid(1)));
        push(&mut ring, 2, false, Some(cid(10)));
        assert_eq!(ring.innermost(), Some(ScopeId(2)));
        ring.pop(ScopeId(1));
        assert_eq!(ring.current(), Some(cid(1)));
        assert_eq!(
            ring.innermost(),
            None,
            "a submenu outlived the menu that owned it"
        );
    }

    #[test]
    fn apply_reports_a_move_once_for_the_whole_batch() {
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        assert!(ring.apply(&[FocusOp::Step { forward: true }], &hits));
        assert_eq!(ring.current(), Some(cid(1)));
        // Two ops that cancel out leave focus where it was, so nothing is repainted.
        assert!(!ring.apply(
            &[FocusOp::Focus(Some(cid(2))), FocusOp::Focus(Some(cid(1)))],
            &hits
        ));
        assert!(ring.apply(&[FocusOp::End { last: true }], &hits));
        assert_eq!(ring.current(), Some(cid(3)));
    }
}
