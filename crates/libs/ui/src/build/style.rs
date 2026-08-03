//! The style recipe: what a node's style can be lowered from again, and nothing more.
//!
//! A style is a function of a preset, a rule list and a [`Scope`]. The width axis reaches it
//! two ways and no third: through `metric` and `typography`, which is how a class re-spaces a
//! subtree it has no declaration in; and through a rule that names a class, which is how a
//! container states the handful of arrangements that are genuinely a layout decision. Both
//! read the same axis of the same scope, so a container that changes class changes the styles
//! of its subtree and nothing else about them — no structure, and no colour.
//!
//! **The scope kept here is class-free.** The width axis is a layout *output* and
//! `windows-scene` is its one authority — it resolves the class inside the solve and hands it
//! back through [`Restyle`](windows_scene::Restyle). Storing a class-resolved scope is what
//! made the class two facts on two sides of the crate boundary, one of them a frame stale.
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
    /// Taken straight off the arena's chain, and not through a `Vec` on the way.
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

/// One node's style, as the thing it can be lowered from again.
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
    /// The store itself, with nothing wrapped around it: a type whose every method forwarded
    /// one of `place`, `take`, `get` and `len` under a second name would be four chances for
    /// this table to mean something different from the others, for no behaviour of its own.
    ///
    /// **No authority beside it**, and that is the statement: this layer owns no node counter,
    /// so it can place a recipe against a node and never invent one. The staleness rule stays
    /// the model's — a node index is dense and reused, and the stored id catches the reuse.
    static STYLES: RefCell<Slots<Node, Recipe>> = RefCell::new(Slots::new());
}

/// Runs `f` against the thread's recipe table.
pub(crate) fn with<R>(f: impl FnOnce(&mut Slots<Node, Recipe>) -> R) -> R {
    STYLES.with(|table| f(&mut table.borrow_mut()))
}

/// The same, for a caller that may be running while the thread's locals are being destroyed.
pub(crate) fn try_with<R>(f: impl FnOnce(&mut Slots<Node, Recipe>) -> R) -> Option<R> {
    STYLES.try_with(|table| f(&mut table.borrow_mut())).ok()
}

/// What the solve asks when a container's class moved: this node's style at that class.
///
/// `None` where the node has no recipe — a sprite the widget layer minted for chrome rather
/// than from a slot — which leaves its style alone, since nothing about it reads the class.
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

/// This node's style with a definite inline size written over it, at the class the last solve
/// resolved.
///
/// What a single-line run's own node needs, and it does not go through the [`Over`] vocabulary
/// for the reason a wrapped line's box does not either: the number is the **text engine's**
/// rather than an author's, so this is the lowering resolving a measurement and `Len`
/// deliberately cannot say it.
///
/// The reason it has to be said at all is that a measured cross size does not survive a
/// container that stretches its children — CSS stretch applies wherever the cross size is
/// `auto`, and a measurement leaves it `auto`. A run stretched that way keeps drawing the same
/// coverage tile into a box several times its width, and the tile's brush fills, so the glyphs
/// come out smeared across the container. A definite size is the only thing stretch yields to.
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

/// This node's style with one override appended, at the class the last solve resolved.
///
/// What a style that follows a value re-lowers through: taking the class from the solve
/// rather than from a captured scope is what keeps a bound style and a classified container
/// from disagreeing.
pub(crate) fn lower_with(
    node: NodeId,
    class: WidthClass,
    extra: Option<Over>,
) -> Option<taffy::Style> {
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
