//! Structure: how a tree changes shape, and what owns what while it does.
//!
//! # Views are built once
//!
//! There is no re-render. A view function runs **once**, at mount, and installs
//! [`Effect`](crate::signal::Effect)s that update sinks thereafter. Because there is no
//! re-render there is no hook order, and therefore the entire class of conditional-hook
//! bug does not exist: a cell is created once per scope, at whatever point in the code
//! creates it, inside whatever `if` you like.
//!
//! What follows from that is that structure changes only where the shape genuinely
//! changes, and there are exactly two such places. This module is both of them.
//!
//! | | shape | mechanism |
//! |---|---|---|
//! | [`Keyed`] | a list whose items are inserted, removed and reordered | key delta + a longest increasing subsequence |
//! | [`Branch`] | a subtree that is present or absent, or one of several | an [`Owner`] that exists or does not |
//!
//! Both own [`Owner`](crate::signal::Owner) scopes and nothing else, so they are the
//! disposal story for structure in the same way `Owner` is for values. Neither knows what
//! a widget is: the widget layer supplies the callbacks that turn a step into nodes, and
//! keeps the alphabet closed by being the only thing that does.
//!
//! # What is not here
//!
//! `each`, `when` and `switch` — the authoring-facing functions — are these two
//! mechanisms bound to a view type, and the view type is the widget layer's. Binding them
//! here would mean inventing that type before the layer that owns it exists, which is the
//! one thing the build order is shaped to avoid.

mod branch;
mod keyed;
#[cfg(test)]
mod tests;

pub use branch::Branch;
pub use keyed::{Keyed, Step, compute_lis};
