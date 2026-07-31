//! The child list. **Both halves.**
//!
//! Each half keeps its own tree — the app's carries hit declarations and layout ids, the
//! front's carries composition visuals — and they mirror each other. The *mechanics* of
//! that are one thing, so the splice lives here and both halves call it.
//!
//! Children are an intrusive chain and not a `Vec`: five ids give O(1) link and unlink with
//! no allocation, against twenty-four bytes and a heap allocation per branch node. They
//! mirror `VisualCollection`'s vocabulary — insert-at-bottom, insert-above and remove, and
//! no insert-at-index, so neither has this.

use crate::sink::NodeId;

/// A node's membership of its parent's child list.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Links {
    pub(crate) parent: NodeId,
    pub(crate) first: NodeId,
    pub(crate) last: NodeId,
    pub(crate) next: NodeId,
    pub(crate) prev: NodeId,
}

/// A store of nodes that can be spliced.
///
/// The whole of what the splice needs, which is why it is two methods: a generational
/// arena and a dense vector are both this.
///
/// Implementors answer for live ids only. [`NodeId::NONE`] is handled *here*: the splice
/// reads "no neighbour" off a `None`, and a dense vector answers for index zero quite
/// happily — so an implementor that forgot would corrupt the chain silently.
pub(crate) trait Forest {
    fn links(&self, id: NodeId) -> Option<&Links>;
    fn links_mut(&mut self, id: NodeId) -> Option<&mut Links>;
}

fn get(f: &impl Forest, id: NodeId) -> Option<&Links> {
    if id.is_none() { None } else { f.links(id) }
}

fn get_mut(f: &mut impl Forest, id: NodeId) -> Option<&mut Links> {
    if id.is_none() { None } else { f.links_mut(id) }
}

/// Points the forward edge into `to`: `prev.next`, or the parent's head.
fn forward(f: &mut impl Forest, parent: NodeId, prev: NodeId, to: NodeId) {
    match get_mut(f, prev) {
        Some(prev) => prev.next = to,
        None => {
            if let Some(parent) = get_mut(f, parent) {
                parent.first = to;
            }
        }
    }
}

/// Points the back edge into `to`: `next.prev`, or the parent's tail.
fn backward(f: &mut impl Forest, parent: NodeId, next: NodeId, to: NodeId) {
    match get_mut(f, next) {
        Some(next) => next.prev = to,
        None => {
            if let Some(parent) = get_mut(f, parent) {
                parent.last = to;
            }
        }
    }
}

/// Splices `id` into `parent`'s children, directly above `after`.
///
/// `after` names the sibling below; `None` is the bottom of the stack.
///
/// Both directions have the same shape — point the neighbour's edge, or the parent's end
/// pointer where there is no neighbour — and that shape *is* the invariant. Writing it once
/// makes this and [`unlink`] exact mirrors.
pub(crate) fn link(f: &mut impl Forest, id: NodeId, parent: NodeId, after: Option<NodeId>) {
    debug_assert!(
        after.is_none_or(|s| get(f, s).is_some_and(|s| s.parent == parent)),
        "a node was ordered against a sibling of a different parent"
    );
    let prev = after.unwrap_or(NodeId::NONE);
    let next = match get(f, prev) {
        Some(prev) => prev.next,
        None => get(f, parent).map_or(NodeId::NONE, |p| p.first),
    };
    let Some(links) = get_mut(f, id) else {
        return;
    };
    *links = Links {
        parent,
        prev,
        next,
        ..*links
    };
    forward(f, parent, prev, id);
    backward(f, parent, next, id);
}

/// Cuts `id` out of its parent's children, leaving its own subtree hanging off it.
pub(crate) fn unlink(f: &mut impl Forest, id: NodeId) {
    let Some(&Links {
        parent, prev, next, ..
    }) = get(f, id)
    else {
        return;
    };
    forward(f, parent, prev, next);
    backward(f, parent, next, prev);
    if let Some(links) = get_mut(f, id) {
        *links = Links {
            first: links.first,
            last: links.last,
            ..Links::default()
        };
    }
}

/// A node's children, bottom to top — paint order, z-order, and the order
/// `VisualCollection` holds them in.
pub(crate) fn children(f: &impl Forest, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let mut at = get(f, id).map_or(NodeId::NONE, |links| links.first);
    core::iter::from_fn(move || {
        let current = at;
        at = get(f, current)?.next;
        Some(current)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Neither half's storage, so the splice is tested against the trait rather than
    /// against whichever store happened to be convenient.
    #[derive(Default)]
    struct Bare(HashMap<usize, Links>);

    impl Forest for Bare {
        fn links(&self, id: NodeId) -> Option<&Links> {
            self.0.get(&id.index())
        }
        fn links_mut(&mut self, id: NodeId) -> Option<&mut Links> {
            self.0.get_mut(&id.index())
        }
    }

    impl Bare {
        fn add(&mut self, index: u32) -> NodeId {
            let id = NodeId::raw(index, 1);
            self.0.insert(index as usize, Links::default());
            id
        }
    }

    /// `n` children linked bottom-to-top under one parent.
    fn stack(n: u32) -> (Bare, NodeId, Vec<NodeId>) {
        let mut f = Bare::default();
        let parent = f.add(0);
        let mut ids = Vec::new();
        let mut top = None;
        for index in 1..=n {
            let id = f.add(index);
            link(&mut f, id, parent, top);
            ids.push(id);
            top = Some(id);
        }
        (f, parent, ids)
    }

    /// Walks forwards and backwards; both must agree, and both must agree with the parent's
    /// end pointers. A splice that corrupts one direction only is invisible to any walk
    /// that goes one way, and that is the failure this catches.
    fn order(f: &Bare, parent: NodeId) -> Vec<NodeId> {
        let forwards: Vec<_> = children(f, parent).collect();
        let mut backwards = Vec::new();
        let mut at = get(f, parent).map_or(NodeId::NONE, |p| p.last);
        while let Some(links) = get(f, at) {
            backwards.push(at);
            at = links.prev;
        }
        backwards.reverse();
        assert_eq!(forwards, backwards, "the chain disagrees with itself");
        let ends = get(f, parent).copied().unwrap_or_default();
        assert_eq!(forwards.first().copied().unwrap_or(NodeId::NONE), ends.first);
        assert_eq!(forwards.last().copied().unwrap_or(NodeId::NONE), ends.last);
        forwards
    }

    #[test]
    fn linking_above_each_sibling_in_turn_stacks_them_in_order() {
        let (f, parent, ids) = stack(4);
        assert_eq!(order(&f, parent), ids);
    }

    #[test]
    fn linking_with_no_sibling_goes_to_the_bottom() {
        let (mut f, parent, ids) = stack(3);
        let bottom = f.add(9);
        link(&mut f, bottom, parent, None);
        assert_eq!(order(&f, parent), [bottom, ids[0], ids[1], ids[2]]);
    }

    #[test]
    fn unlinking_holds_the_chain_together_wherever_it_is_cut() {
        // Every position: head, tail and middle take different arms, and the single-child
        // case is the one where both end pointers move at once.
        for cut in 0..4 {
            let (mut f, parent, ids) = stack(4);
            unlink(&mut f, ids[cut]);
            let expected: Vec<_> = ids
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != cut)
                .map(|(_, id)| *id)
                .collect();
            assert_eq!(order(&f, parent), expected, "cutting at {cut}");
            assert_eq!(
                *get(&f, ids[cut]).unwrap(),
                Links::default(),
                "an unlinked node still points at its old siblings"
            );
        }
    }

    #[test]
    fn unlinking_the_only_child_empties_the_parent() {
        let (mut f, parent, ids) = stack(1);
        unlink(&mut f, ids[0]);
        assert!(order(&f, parent).is_empty());
    }

    #[test]
    fn a_move_is_an_unlink_and_a_link_and_nets_to_a_reorder() {
        let (mut f, parent, ids) = stack(4);
        // Lift the bottom one to the top, which is what a reorder emits.
        unlink(&mut f, ids[0]);
        link(&mut f, ids[0], parent, Some(ids[3]));
        assert_eq!(order(&f, parent), [ids[1], ids[2], ids[3], ids[0]]);
    }

    #[test]
    fn a_subtree_survives_its_root_being_cut_out() {
        let (mut f, parent, ids) = stack(2);
        let grandchild = f.add(7);
        link(&mut f, grandchild, ids[0], None);
        unlink(&mut f, ids[0]);
        assert_eq!(order(&f, parent), [ids[1]]);
        // The destroy walk descends from a node already cut out of its parent, so its own
        // children have to still be reachable from it.
        assert_eq!(children(&f, ids[0]).collect::<Vec<_>>(), [grandchild]);
    }
}
