//! Keyboard focus, and the scopes it nests in.
//!
//! **Focus order is the hit array's order**, filtered to `INTERACTIVE`, with an explicit
//! `tab_index` escape. That is not a convenience: the pointer, the wheel, keyboard focus
//! order, the window's own caption hit test and automation's element-from-point all resolve
//! through the *same* z-ordered flat array, and a second ordering maintained beside it is
//! exactly the failure that arrangement exists to prevent.
//!
//! Scopes nest. An open overlay pushes one, so `Tab` cycles within it and closing it restores
//! focus to whatever invoked it; `Esc` is delivered to the innermost scope before any control
//! sees it. A scope is named by the entry its subtree begins after, rather than by an index
//! range, so that it survives the array being rebuilt underneath it.

use rustc_hash::FxHashMap;
use windows_scene::{ControlId, HitFlags, HitTable};

/// A live focus scope.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScopeId(pub u32);

/// What an overlay declares when it opens.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FocusScope {
    /// `Tab` wraps at the ends rather than leaving. What a modal popup wants; a flyout wants
    /// the opposite, because tabbing past a picker's last item should dismiss it and move on.
    pub trap: bool,
    /// Captured at open and focused at close, unless the control is gone. Without it,
    /// dismissing a menu leaves focus nowhere and the next keystroke goes to the window.
    pub restore_to: Option<ControlId>,
    /// The scope's own first entry in the hit array — its blocker for a light-dismissing
    /// overlay, or its root otherwise. Everything at or after it in the array is inside.
    pub from: ControlId,
}

/// Where focus is, and what bounds it.
#[derive(Debug, Default)]
pub struct FocusRing {
    current: Option<ControlId>,
    scopes: Vec<(ScopeId, FocusScope)>,
    next: u32,
    /// The explicit escape. Empty for every screen that does not need it, which is most.
    order: FxHashMap<ControlId, i32>,
    /// Reused across navigations, so moving focus allocates nothing after the first move.
    scratch: Vec<(i32, ControlId)>,
}

impl FocusRing {
    /// What has focus.
    #[must_use]
    pub const fn current(&self) -> Option<ControlId> {
        self.current
    }

    /// Focuses a control directly — what a press does.
    ///
    /// Answers the control that lost focus, so a caller can move both sets of pixels in one
    /// pass. `None` back means nothing changed.
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

    /// Gives a control an explicit position in the order.
    ///
    /// The escape, not the mechanism: a screen that needs one for every control has a layout
    /// whose reading order is wrong.
    pub fn set_tab_index(&mut self, id: ControlId, index: i32) {
        self.order.insert(id, index);
    }

    /// Forgets a control's explicit position, on unmount.
    pub fn clear_tab_index(&mut self, id: ControlId) {
        self.order.remove(&id);
    }

    /// Opens a scope. Everything at or after `from` in the hit array is inside it.
    pub fn push_scope(&mut self, scope: FocusScope) -> ScopeId {
        self.next += 1;
        let id = ScopeId(self.next);
        self.scopes.push((id, scope));
        id
    }

    /// Closes a scope and restores focus to whatever invoked it.
    ///
    /// Answers what focus should become, which the caller applies — so the pixels move on the
    /// front thread in the same pass that closed the overlay.
    pub fn pop_scope(&mut self, id: ScopeId) -> Option<ControlId> {
        let at = self.scopes.iter().position(|(scope, _)| *scope == id)?;
        // Everything nested inside it goes too: an overlay that closes takes its submenus.
        let restore = self.scopes[at].1.restore_to;
        self.scopes.truncate(at);
        self.current = restore;
        restore
    }

    /// The innermost scope, which is what `Esc` is delivered to first.
    #[must_use]
    pub fn innermost(&self) -> Option<(ScopeId, FocusScope)> {
        self.scopes.last().copied()
    }

    /// How many scopes are open.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Moves focus one step through the order.
    ///
    /// [`Move::Left`] means the step ran off the end of a non-trapping scope, which is what
    /// dismisses a flyout and moves on rather than wrapping inside it.
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
        let id = self.scratch[next].1;
        match self.focus(Some(id)) {
            Some((from, _)) => Move::To { from, to: id },
            None => Move::Nowhere,
        }
    }

    /// The focusable controls, in order, restricted to the innermost scope.
    ///
    /// Explicit indices sort first and among themselves; everything else keeps the array's
    /// own order, which is the reading order layout produced. A stable sort is what makes
    /// the second half true.
    fn collect(&mut self, hits: &HitTable) {
        self.scratch.clear();
        let entries = hits.entries();
        // A scope begins at its own first entry, so everything before that is outside it.
        let start = match self.scopes.last() {
            Some((_, scope)) => entries
                .iter()
                .position(|entry| entry.id == scope.from)
                .unwrap_or(0),
            None => 0,
        };
        for entry in &entries[start..] {
            // A blocker routes a press and is not a place focus can rest, and neither is a
            // scroll viewport that carries no control of its own.
            if !entry.flags.contains(HitFlags::INTERACTIVE)
                || entry.flags.contains(HitFlags::BLOCKER)
            {
                continue;
            }
            let index = self.order.get(&entry.id).copied().unwrap_or(0);
            self.scratch.push((index, entry.id));
        }
        // Positive indices first, in their own order; zero — everything unstated — after, in
        // the array's.
        self.scratch
            .sort_by_key(|(index, _)| if *index > 0 { (0, *index) } else { (1, 0) });
    }
}

/// What a focus step did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// Focus moved.
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
    use windows_scene::{HitEntry, NO_ENTRY, NodeId};

    fn entry(id: u64, flags: HitFlags) -> HitEntry {
        HitEntry {
            x0: 0.0,
            y0: 0.0,
            x1: 10.0,
            y1: 10.0,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            flags,
            scroll_src: NodeId::NONE,
            id: ControlId(id),
        }
    }

    fn table(ids: &[u64]) -> HitTable {
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
                to: ControlId(1)
            }
        );
        assert_eq!(
            ring.step(&hits, true),
            Move::To {
                from: Some(ControlId(1)),
                to: ControlId(2)
            }
        );
        assert_eq!(
            ring.step(&hits, false),
            Move::To {
                from: Some(ControlId(2)),
                to: ControlId(1)
            }
        );
    }

    #[test]
    fn an_explicit_index_sorts_ahead_of_everything_unstated() {
        let hits = table(&[1, 2, 3]);
        let mut ring = FocusRing::default();
        ring.set_tab_index(ControlId(3), 1);
        assert_eq!(
            ring.step(&hits, true),
            Move::To {
                from: None,
                to: ControlId(3)
            }
        );
        // …and the rest keep the array's own order behind it.
        assert_eq!(
            ring.step(&hits, true),
            Move::To {
                from: Some(ControlId(3)),
                to: ControlId(1)
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
            restore_to: Some(ControlId(1)),
            from: ControlId(9),
        });
        // The blocker is not a focus stop, so the scope is exactly its two items.
        assert!(matches!(popup.step(&hits, true), Move::To { to, .. } if to == ControlId(10)));
        assert!(matches!(popup.step(&hits, true), Move::To { to, .. } if to == ControlId(11)));
        assert!(
            matches!(popup.step(&hits, true), Move::To { to, .. } if to == ControlId(10)),
            "a trapping scope let focus out"
        );

        let mut flyout = FocusRing::default();
        flyout.push_scope(FocusScope {
            trap: false,
            restore_to: Some(ControlId(1)),
            from: ControlId(9),
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
        _ = ring.focus(Some(ControlId(1)));
        let scope = ring.push_scope(FocusScope {
            trap: true,
            restore_to: Some(ControlId(1)),
            from: ControlId(9),
        });
        _ = ring.focus(Some(ControlId(10)));
        assert_eq!(ring.pop_scope(scope), Some(ControlId(1)));
        assert_eq!(ring.current(), Some(ControlId(1)));
        assert_eq!(ring.depth(), 0);
    }

    #[test]
    fn closing_a_scope_takes_everything_nested_inside_it() {
        let mut ring = FocusRing::default();
        let outer = ring.push_scope(FocusScope {
            trap: false,
            restore_to: Some(ControlId(1)),
            from: ControlId(9),
        });
        ring.push_scope(FocusScope {
            trap: false,
            restore_to: Some(ControlId(10)),
            from: ControlId(20),
        });
        assert_eq!(ring.depth(), 2);
        assert_eq!(ring.pop_scope(outer), Some(ControlId(1)));
        assert_eq!(ring.depth(), 0, "a submenu outlived the menu that owned it");
    }
}
