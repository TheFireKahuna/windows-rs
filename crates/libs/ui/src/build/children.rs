//! `IntoChildren` — and why there is no `Child` type.
//!
//! One trait covers one child, many, none, an array and a `Vec`, so every container takes
//! the same parameter and there is **no single-child position to get wrong**. That is
//! strictly stronger than distinguishing a `Child` type from a `Children` type: the position
//! that could be given the wrong one does not exist.
//!
//! `Vec` is the only heap form, and it is the caller's own: static structure goes through
//! tuples and arrays and never touches it.

use super::El;

/// Where a container's children are collected.
///
/// Named for what it collects rather than for how: `Slots` is the one store this stack keeps
/// per id family, and a second meaning of that word in the same crate is one a reader has to
/// disambiguate by module every time they meet it.
///
/// Carries no buffer, and that is the point twice over. **It cannot be a borrow**, because a
/// borrow would have to be of something, and the only correct something is the arena's own
/// pending stack — which an implementation must be free to re-enter, since an adapter builds
/// its own slot while it appends. And it hands nothing out: the indices underneath are the
/// arena's, and a trait that exposed them would let an implementation name a node it did not
/// build. Appending an element is the only thing one can do, and the only thing one has ever
/// needed to do.
///
/// Not constructible outside this crate, so [`IntoChildren::append`] cannot be called
/// anywhere a container is not collecting.
pub struct Children(());

impl Children {
    pub(crate) const fn new() -> Self {
        Self(())
    }

    /// Appends one element, in paint order.
    ///
    /// Straight onto the arena's pending stack. One borrow per child, released before the
    /// next — which is what lets an adapter push its own slot from inside an append.
    ///
    /// An element a constant `.when(false)` marked absent is **not appended**, which is what
    /// makes it cost no node rather than a hidden one.
    pub fn push<K>(&mut self, el: El<K>) {
        super::arena::Build::with(|b| {
            if b.nodes[el.at as usize].present {
                b.pending.push(el.at);
            }
        });
    }
}

/// Anything that can be a container's children: one, many, none, an array or a `Vec`.
pub trait IntoChildren {
    /// Appends this into the container's child list, in paint order.
    fn append(self, out: &mut Children);
}

impl<K> IntoChildren for El<K> {
    fn append(self, out: &mut Children) {
        out.push(self);
    }
}

/// Nothing. A container with no children is legal and says so.
impl IntoChildren for () {
    fn append(self, _: &mut Children) {}
}

/// Zero or one. What `.map(..)` on an optional region produces.
impl<T: IntoChildren> IntoChildren for Option<T> {
    fn append(self, out: &mut Children) {
        if let Some(child) = self {
            child.append(out);
        }
    }
}

/// A fixed number of the same thing, at any length, with no macro behind it.
impl<T: IntoChildren, const N: usize> IntoChildren for [T; N] {
    fn append(self, out: &mut Children) {
        for child in self {
            child.append(out);
        }
    }
}

/// A dynamic list — the one heap form, and it is visible at the call site.
impl<T: IntoChildren> IntoChildren for Vec<T> {
    fn append(self, out: &mut Children) {
        for child in self {
            child.append(out);
        }
    }
}

/// Tuples of mixed kinds, which is what a hand-written screen is made of.
///
/// One invocation. Each expansion emits its own arity and then recurses on its tail, so the
/// arities are a consequence of the list rather than twelve lines that have to agree with
/// it — and raising the ceiling is one identifier.
macro_rules! tuple_children {
    () => {};
    ($head:ident $(, $tail:ident)*) => {
        impl<$head: IntoChildren $(, $tail: IntoChildren)*> IntoChildren for ($head, $($tail,)*) {
            #[allow(non_snake_case, reason = "the type parameters name the bindings")]
            fn append(self, out: &mut Children) {
                let ($head, $($tail,)*) = self;
                $head.append(out);
                $($tail.append(out);)*
            }
        }
        tuple_children!($($tail),*);
    };
}

tuple_children!(A, B, C, D, E, F, G, H, I, J, K, L);
