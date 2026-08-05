//! `IntoChildren`: the one child parameter every container takes.
//!
//! One trait covers one child, many, none, an array and a `Vec`, so a container has a single
//! child parameter and there is **no separate single-child position**.
//!
//! `Vec` is the only heap form, and it is the caller's own: static structure goes through
//! tuples and arrays and never touches it.

use super::El;

/// Where a container's children are collected.
///
/// **Carries no buffer.** A borrow of the arena's pending stack would hold that borrow across
/// an [`IntoChildren::append`] call, and an adapter builds its own slot while it appends. It
/// also hands nothing out: the indices underneath are the arena's, and exposing them would
/// let an implementation name a node it did not build. Appending an element is the only
/// operation.
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
    /// Straight onto the arena's pending stack, one borrow per child released before the
    /// next, so an adapter can push its own slot from inside an append.
    ///
    /// An element a constant `.when(false)` marked absent is **not appended**, so it costs no
    /// node rather than a hidden one.
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

/// Nothing: a container with no children.
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

/// A fixed number of children of one type, at any length, with no macro behind it.
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

/// Implements the trait for tuples of mixed kinds, which is what a hand-written screen is
/// made of.
///
/// One invocation: each expansion emits its own arity and then recurses on its tail, so the
/// set of arities follows from the identifier list and raising the ceiling is one more
/// identifier.
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
