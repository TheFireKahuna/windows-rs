//! Structure: the two places a tree changes shape, and what owns each piece while it does.
//!
//! # Views are built once
//!
//! A view function runs once, at mount, and installs
//! [`Effect`](crate::signal::Effect)s that update sinks thereafter. There is no re-render
//! and so no hook order: a cell is created once per scope, at whatever point in the code
//! creates it, including inside a conditional.
//!
//! Structure therefore changes only where the shape genuinely changes, and there are two
//! such places. This module is both of them.
//!
//! | | shape | mechanism |
//! |---|---|---|
//! | [`Keyed`] | a list whose items are inserted, removed and reordered | key delta + a longest increasing subsequence |
//! | [`Branch`] | a subtree that is present or absent, or one of several | an [`Owner`] that exists or does not |
//!
//! Both own [`Owner`](crate::signal::Owner) scopes and nothing else, so disposing structure
//! is disposing scopes, exactly as it is for values. Neither knows what a widget is: the
//! widget layer supplies the callbacks that turn a step into nodes, and it is the only
//! layer that does.
//!
//! # What is not here
//!
//! `each`, `when` and `switch` — the authoring-facing functions — are these two mechanisms
//! bound to a view type, and that type belongs to the widget layer.

mod branch;
mod keyed;
#[cfg(test)]
mod tests;

pub use branch::Branch;
pub use keyed::{Keyed, Step, compute_lis};
