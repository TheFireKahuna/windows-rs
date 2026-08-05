//! The style recipe: what a node's style can be lowered from again, and nothing more.
//!
//! A style is a function of a preset, a rule list and a [`Scope`]. The width axis reaches it
//! two ways: through `metric` and `typography`, which is how a class re-spaces a subtree that
//! has no declaration in it, and through a rule naming a class, which is how a container
//! states an arrangement. Both read the same axis of the same scope, so a container that
//! changes class changes the styles of its subtree and nothing else — no structure, and no
//! colour.
//!
//! **The scope kept here is class-free.** The width axis is a layout *output* and
//! `windows-scene` is its one authority: it resolves the class inside the solve and hands it
//! back through [`Restyle`](windows_scene::Restyle). A class-resolved scope stored here would
//! be a second copy of that fact, on the other side of the crate boundary and a frame stale.
//!
//! A thread-local rather than a field of the host, for the reason the text table is one:
//! re-lowering runs *inside* the solve, where the host is already borrowed.

use crate::layout::{Over, Preset, Rule};
use crate::role::Scope;
use std::cell::RefCell;
use windows_scene::{Node, NodeId, Slots, WidthClass, taffy};

/// Up to four rules inline, and a boxed slice beyond.
///
/// Four covers every widget in the set and almost every call site, so a realized list row
/// allocates nothing for its styles.
#[derive(Clone)]
pub(crate) enum OverStore {
    Inline { count: u8, items: [Rule; 4] },
    Spill(Box<[Rule]>),
}

impl OverStore {
    /// Collects `items` into a store, taking them straight off the arena's chain rather than
    /// through a `Vec` on the way.
    pub(crate) fn collect(items: impl Iterator<Item = Rule>) -> Self {
        let mut inline = [Rule::always(Over::Grow); 4];
        let mut count = 0usize;
        let mut spill: Option<Vec<Rule>> = None;
        for rule in items {
            match &mut spill {
                Some(spilled) => spilled.push(rule),
                None if count < inline.len() => {
                    inline[count] = rule;
                    count += 1;
                }
                None => {
                    let mut spilled = inline.to_vec();
                    spilled.push(rule);
                    spill = Some(spilled);
                }
            }
        }
        match spill {
            Some(spilled) => Self::Spill(spilled.into_boxed_slice()),
            None => Self::Inline {
                count: count as u8,
                items: inline,
            },
        }
    }

    pub(crate) fn as_slice(&self) -> &[Rule] {
        match self {
            Self::Inline { count, items } => &items[..*count as usize],
            Self::Spill(items) => items,
        }
    }
}

/// Everything one node's style is lowered from, kept so it can be lowered again.
#[derive(Clone)]
pub(crate) struct Recipe {
    pub preset: Preset,
    pub over: OverStore,
    /// Class-free: the width axis comes from the solve, never from here.
    pub scope: Scope,
}

thread_local! {
    /// A recipe per node, keyed by ids the **model** mints.
    ///
    /// The store itself, with no wrapper around it, so `place`, `take`, `get` and `len` mean
    /// here what they mean in every other table over [`Slots`].
    ///
    /// This layer owns no node counter, so it can place a recipe against a node and never
    /// invent one. Staleness stays the model's rule: a node index is dense and reused, and
    /// the stored id catches the reuse.
    static STYLES: RefCell<Slots<Node, Recipe>> = RefCell::new(Slots::new());
}

/// Runs `f` against the thread's recipe table.
pub(crate) fn with<R>(f: impl FnOnce(&mut Slots<Node, Recipe>) -> R) -> R {
    STYLES.with(|table| f(&mut table.borrow_mut()))
}

/// Runs `f` against the thread's recipe table, answering `None` where the thread's locals are
/// being destroyed and the table cannot be reached.
pub(crate) fn try_with<R>(f: impl FnOnce(&mut Slots<Node, Recipe>) -> R) -> Option<R> {
    STYLES.try_with(|table| f(&mut table.borrow_mut())).ok()
}

/// Lowers this node's style at `class`. What the solve asks when a container's class moved.
///
/// `None` where the node has no recipe — a sprite the widget layer minted for chrome rather
/// than from a slot — which leaves its style alone, since nothing in it reads the class.
pub(crate) fn restyle(node: NodeId, class: WidthClass) -> Option<taffy::Style> {
    with(|table| {
        let recipe = table.get(node)?;
        Some(crate::layout::lower(
            recipe.preset,
            recipe.over.as_slice(),
            recipe.scope.at_width(class),
        ))
    })
}

/// Lowers this node's style with a definite inline size written over it, at the class the
/// last solve resolved. `None` where the node has no recipe.
///
/// What a single-line run's own node needs. It does not go through the [`Over`] vocabulary,
/// for the reason a wrapped line's box does not either: the number is the **text engine's**
/// rather than an author's, so this is the lowering resolving a measurement and `Len` cannot
/// say it.
///
/// A measured cross size does not survive a container that stretches its children: CSS
/// stretch applies wherever the cross size is `auto`, and a measurement leaves it `auto`. A
/// run stretched that way draws the same coverage tile into a box several times its width,
/// and the tile's brush fills, so the glyphs smear across the container. A definite size is
/// what stretch yields to.
pub(crate) fn pin_width(node: NodeId, class: WidthClass, width: f32) -> Option<taffy::Style> {
    with(|table| {
        let recipe = table.get(node)?;
        let mut style = crate::layout::lower(
            recipe.preset,
            recipe.over.as_slice(),
            recipe.scope.at_width(class),
        );
        style.size.width = taffy::Dimension::length(width);
        Some(style)
    })
}

/// Lowers this node's style with `extra` appended, at the class the last solve resolved.
/// `None` where the node has no recipe.
///
/// What a style that follows a value re-lowers through. Taking the class from the solve
/// rather than from a captured scope is what keeps a bound style and a classified container
/// from disagreeing.
///
/// `extra` must carry **every** bound override for the node, which is why the node has one
/// effect rather than one per act: lowering from the recipe plus a single override discards
/// the others, and two bound styles on one node would take turns winning.
pub(crate) fn lower_with(node: NodeId, class: WidthClass, extra: &[Over]) -> Option<taffy::Style> {
    with(|table| {
        let recipe = table.get(node)?;
        Some(crate::layout::lower_with(
            recipe.preset,
            recipe.over.as_slice(),
            extra,
            recipe.scope.at_width(class),
        ))
    })
}
