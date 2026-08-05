//! Keyboard focus and the scopes that bound it.
//!
//! Focus order is the hit array's order, filtered to `INTERACTIVE`, with `tab_index` as an
//! explicit override. The pointer, the wheel, keyboard focus order, the window's caption hit
//! test and automation's element-from-point all resolve through that one z-ordered flat
//! array, so no second ordering is maintained beside it.
//!
//! Scopes nest. An open overlay pushes one, so `Tab` cycles within it and closing it restores
//! focus to whatever invoked it; `Esc` is delivered to the innermost scope before any control
//! sees it. A scope is named by the entry its subtree begins at rather than by an index range,
//! so it survives the array being rebuilt underneath it.

use rustc_hash::FxHashMap;
use windows_scene::{ControlId, HitFlags, HitTable};

/// Identifies one open focus scope.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScopeId(pub u32);

/// Describes a focus scope at the moment an overlay opens it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FocusScope {
    /// Wraps `Tab` at the ends of the scope instead of letting it leave. A modal popup sets
    /// this; a flyout leaves it clear, so tabbing past its last item dismisses it.
    pub trap: bool,
    /// The control focus returns to when the scope closes. `None` leaves focus cleared, so
    /// the next keystroke reaches the window rather than a control.
    pub restore_to: Option<ControlId>,
    /// The scope's first entry in the hit array: its blocker for a light-dismissing overlay,
    /// or its root otherwise. Everything at or after it in the array is inside the scope.
    pub from: ControlId,
}

/// Holds the focused control and the stack of scopes that bound navigation.
#[derive(Debug, Default)]
pub struct FocusRing {
    current: Option<ControlId>,
    scopes: Vec<(ScopeId, FocusScope)>,
    next: u32,
    /// Explicit tab positions, keyed by control. Empty for every screen that does not
    /// override the hit array's order.
    order: FxHashMap<ControlId, i32>,
    /// Candidate buffer reused across navigations, so moving focus allocates nothing after
    /// the first move.
    scratch: Vec<(i32, ControlId)>,
}

impl FocusRing {
    /// Returns the focused control, or `None` when nothing has focus.
    #[must_use]
    pub const fn current(&self) -> Option<ControlId> {
        self.current
    }

    /// Focuses `next` directly, as a press does.
    ///
    /// Returns the control that lost focus alongside the one that took it, so a caller
    /// repaints both in one pass. `None` means `next` already had focus and nothing changed.
    pub fn focus(
        &mut self,
        next: Option<ControlId>,
    ) -> Option<(Option<ControlId>, Option<ControlId>)> {
        if self.current == next {
            return None;
        }
        let previous = self.current;
        self.current = next;
        Some((previous, next))
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
    pub fn clear_tab_index(&mut self, id: ControlId) {
        self.order.remove(&id);
    }

    /// Opens a scope and returns its id. Everything at or after `scope.from` in the hit array
    /// is inside it.
    pub fn push_scope(&mut self, scope: FocusScope) -> ScopeId {
        self.next += 1;
        let id = ScopeId(self.next);
        self.scopes.push((id, scope));
        id
    }

    /// Closes the scope `id` and moves focus to its `restore_to`.
    ///
    /// Returns the control focus was moved to, which the caller repaints in the same pass
    /// that closed the overlay. `None` when `id` names no open scope.
    pub fn pop_scope(&mut self, id: ScopeId) -> Option<ControlId> {
        let at = self.scopes.iter().position(|(scope, _)| *scope == id)?;
        // Truncating takes the nested scopes with it: closing an overlay closes its submenus.
        let restore = self.scopes[at].1.restore_to;
        self.scopes.truncate(at);
        self.current = restore;
        restore
    }

    /// Returns the innermost open scope, which `Esc` is delivered to before any control.
    #[must_use]
    pub fn innermost(&self) -> Option<(ScopeId, FocusScope)> {
        self.scopes.last().copied()
    }

    /// Returns the number of open scopes.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Moves focus one step through the order, forwards when `forward` is set.
    ///
    /// With nothing focused, the step enters the scope at whichever end it came from.
    /// [`Move::Left`] means the step ran off the end of a scope that does not trap, which the
    /// caller answers by dismissing that scope and stepping again outside it.
    pub fn step(&mut self, hits: &HitTable, forward: bool) -> Move {
        self.collect(hits);
        if self.scratch.is_empty() {
            return Move::Nowhere;
        }
        let at = self
            .current
            .and_then(|id| self.scratch.iter().position(|(_, entry)| *entry == id));
        let last = self.scratch.len() - 1;
        let next = match (at, forward) {
            // Nothing focused: enter at whichever end the step came from.
            (None, true) => 0,
            (None, false) => last,
            (Some(at), true) if at == last => {
                if self.innermost().is_some_and(|(_, scope)| !scope.trap) {
                    return Move::Left;
                }
                0
            }
            (Some(0), false) => {
                if self.innermost().is_some_and(|(_, scope)| !scope.trap) {
                    return Move::Left;
                }
                last
            }
            (Some(at), true) => at + 1,
            (Some(at), false) => at - 1,
        };
        self.land(self.scratch[next].1)
    }

    /// Moves focus to the next control after the focused one that `pick` accepts, wrapping.
    ///
    /// Drives type-ahead. The candidates and their order are [`step`](Self::step)'s own, so a
    /// menu's letter navigation cannot disagree with its arrow navigation. The search starts
    /// after the focused control and wraps, so repeated presses of one letter cycle the
    /// controls beginning with it. [`Move::Nowhere`] means `pick` accepted none of them.
    pub fn step_to(&mut self, hits: &HitTable, pick: impl Fn(ControlId) -> bool) -> Move {
        self.collect(hits);
        let count = self.scratch.len();
        if count == 0 {
            return Move::Nowhere;
        }
        let from = self
            .current
            .and_then(|id| self.scratch.iter().position(|(_, entry)| *entry == id));
        let start = from.map_or(0, |at| at + 1);
        let found = (0..count)
            .map(|step| (start + step) % count)
            .find(|&at| pick(self.scratch[at].1));
        let Some(at) = found else {
            return Move::Nowhere;
        };
        self.land(self.scratch[at].1)
    }

    /// Moves focus to the last control of the innermost scope when `last` is set, and to the
    /// first otherwise. Drives `Home` and `End`.
    pub fn step_to_end(&mut self, hits: &HitTable, last: bool) -> Move {
        self.collect(hits);
        let Some(&(_, id)) = (if last {
            self.scratch.last()
        } else {
            self.scratch.first()
        }) else {
            return Move::Nowhere;
        };
        self.land(id)
    }

    /// Focuses the candidate a step chose and reports what that did. Landing where focus
    /// already was is [`Move::Nowhere`], so no caller repaints a ring that did not move.
    fn land(&mut self, id: ControlId) -> Move {
        match self.focus(Some(id)) {
            Some((from, _)) => Move::To { from, to: id },
            None => Move::Nowhere,
        }
    }

    /// Fills `scratch` with the innermost scope's focusable controls, in focus order.
    ///
    /// Explicit indices sort first and among themselves; everything else keeps the hit
    /// array's own order, which is the reading order layout produced. The sort is stable,
    /// which is what preserves that order.
    fn collect(&mut self, hits: &HitTable) {
        self.scratch.clear();
        let entries = hits.entries();
        // A scope begins at its own first entry, so everything before that is outside it.
        //
        // A scope whose entry is absent from the array has no subtree in it either: it has
        // closed, or it has not been flushed yet. Both mean nothing is in scope, so this
        // leaves the candidates empty rather than falling back to the head of the array —
        // that fallback widens a stale scope to the whole window and lets `Tab` walk out of
        // a modal, which nothing downstream would report.
        let start = match self.scopes.last() {
            Some((_, scope)) => match entries.iter().position(|entry| entry.id == scope.from) {
                Some(at) => at,
                None => return,
            },
            None => 0,
        };
        for entry in &entries[start..] {
            // A blocker routes a press rather than holding focus, and an entry that is not
            // interactive — a scroll viewport carrying no control of its own — is not a
            // focus stop either.
            if !entry.flags.contains(HitFlags::INTERACTIVE)
                || entry.flags.contains(HitFlags::BLOCKER)
            {
                continue;
            }
            let index = self.order.get(&entry.id).copied().unwrap_or(0);
            self.scratch.push((index, entry.id));
        }
        // Positive indices first, in their own order; everything unstated after, in the hit
        // array's.
        self.scratch
            .sort_by_key(|(index, _)| if *index > 0 { (0, *index) } else { (1, 0) });
    }
}

/// Reports what a focus step did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// Focus moved from `from` to `to`.
    To {
        from: Option<ControlId>,
        to: ControlId,
    },
    /// The step ran off the end of a scope that does not trap. The caller dismisses that
    /// scope and steps again outside it.
    Left,
    /// Nothing focusable, or focus was already where the step would put it.
    Nowhere,
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_scene::{HitEntry, Ids, NO_ENTRY, NodeId};

    /// Returns the `n`th id a fresh [`Ids`] mints.
    ///
    /// A `ControlId` is a generational index with no public constructor, so it can only come
    /// from an [`Ids`]. Minting densely from a fresh one is deterministic, which makes this
    /// stable across calls and distinct per `n` without any shared state.
    fn cid(n: u32) -> ControlId {
        let mut ids = Ids::<windows_scene::Control>::new();
        let mut id = ids.mint();
        for _ in 1..n {
            id = ids.mint();
        }
        id
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
        let mut table = HitTable::default();
        let entries: Vec<HitEntry> = ids
            .iter()
            .map(|id| entry(*id, HitFlags::INTERACTIVE))
            .collect();
        table.replace(&entries);
        table
    }

    #[test]
    fn focus_order_is_the_hit_arrays_order() {
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        assert_eq!(
            ring.step(&hits, true),
            Move::To {
                from: None,
                to: cid(1)
            }
        );
        assert_eq!(
            ring.step(&hits, true),
            Move::To {
                from: Some(cid(1)),
                to: cid(2)
            }
        );
        assert_eq!(
            ring.step(&hits, false),
            Move::To {
                from: Some(cid(2)),
                to: cid(1)
            }
        );
    }

    #[test]
    fn a_scope_whose_entry_has_gone_bounds_navigation_to_nothing() {
        // A scope is named by its own first entry. If that entry is absent from the array,
        // its subtree is absent too — the scope closed, or it has not been flushed yet — so
        // navigation is bounded to nothing. Resolving to the head of the array instead would
        // widen a stale scope to the whole window and let `Tab` walk out of a modal.
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        ring.push_scope(FocusScope {
            trap: true,
            restore_to: None,
            from: cid(99),
        });
        assert_eq!(ring.step(&hits, true), Move::Nowhere);
        assert_eq!(ring.step_to_end(&hits, false), Move::Nowhere);
        assert_eq!(ring.step_to(&hits, |_| true), Move::Nowhere);
        assert_eq!(ring.current(), None, "and nothing was focused on the way");
    }

    #[test]
    fn an_explicit_index_sorts_ahead_of_everything_unstated() {
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        ring.set_tab_index(cid(3), 1);
        assert_eq!(
            ring.step(&hits, true),
            Move::To {
                from: None,
                to: cid(3)
            }
        );
        // …and the rest keep the array's own order behind it.
        assert_eq!(
            ring.step(&hits, true),
            Move::To {
                from: Some(cid(3)),
                to: cid(1)
            }
        );
    }

    #[test]
    fn a_trapping_scope_wraps_and_a_flyout_lets_go() {
        let mut hits = HitTable::default();
        hits.replace(&[
            entry(1, HitFlags::INTERACTIVE),
            entry(9, HitFlags::INTERACTIVE | HitFlags::BLOCKER),
            entry(10, HitFlags::INTERACTIVE),
            entry(11, HitFlags::INTERACTIVE),
        ]);

        let mut popup = FocusRing::default();
        popup.push_scope(FocusScope {
            trap: true,
            restore_to: Some(cid(1)),
            from: cid(9),
        });
        // The blocker is not a focus stop, so the scope is exactly its two items.
        assert!(matches!(popup.step(&hits, true), Move::To { to, .. } if to == cid(10)));
        assert!(matches!(popup.step(&hits, true), Move::To { to, .. } if to == cid(11)));
        assert!(
            matches!(popup.step(&hits, true), Move::To { to, .. } if to == cid(10)),
            "a trapping scope let focus out"
        );

        let mut flyout = FocusRing::default();
        flyout.push_scope(FocusScope {
            trap: false,
            restore_to: Some(cid(1)),
            from: cid(9),
        });
        _ = flyout.step(&hits, true);
        _ = flyout.step(&hits, true);
        assert_eq!(
            flyout.step(&hits, true),
            Move::Left,
            "tabbing past the last item should dismiss a flyout rather than wrap"
        );
    }

    #[test]
    fn closing_a_scope_restores_focus_to_the_invoker() {
        let mut ring = FocusRing::default();
        _ = ring.focus(Some(cid(1)));
        let scope = ring.push_scope(FocusScope {
            trap: true,
            restore_to: Some(cid(1)),
            from: cid(9),
        });
        _ = ring.focus(Some(cid(10)));
        assert_eq!(ring.pop_scope(scope), Some(cid(1)));
        assert_eq!(ring.current(), Some(cid(1)));
        assert_eq!(ring.depth(), 0);
    }

    #[test]
    fn closing_a_scope_takes_everything_nested_inside_it() {
        let mut ring = FocusRing::default();
        let outer = ring.push_scope(FocusScope {
            trap: false,
            restore_to: Some(cid(1)),
            from: cid(9),
        });
        ring.push_scope(FocusScope {
            trap: false,
            restore_to: Some(cid(10)),
            from: cid(20),
        });
        assert_eq!(ring.depth(), 2);
        assert_eq!(ring.pop_scope(outer), Some(cid(1)));
        assert_eq!(ring.depth(), 0, "a submenu outlived the menu that owned it");
    }
}
