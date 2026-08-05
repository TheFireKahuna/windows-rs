//! The lowering: the build arena, the element, and the walk that turns both into `Model`
//! calls.
//!
//! Below this module sit five closed alphabets — `taffy::Style`, the sink alphabet, `Bind`
//! over `Prop`, `HitDecl`, `GestureDecl` — and one total function, `resolve`. This layer
//! adds none of its own. Its whole job is the chain from a builder call to a `SinkPatch`,
//! under one rule:
//!
//! > **The lowering is the only code that touches `Model`. A widget is a data seed with no
//! > body.**
//!
//! That is what makes a widget one short function, and it is the check: a widget that calls
//! `Model`, resolves a `Radiance` or builds a `taffy::Style` has stopped being a seed.

mod adapt;
mod arena;
mod children;
mod el;
mod host;
mod mount;
mod style;
#[cfg(test)]
pub(crate) mod tests;
/// The thread's shaping engine, and the table of laid-out runs behind every label.
///
/// Public for one function: [`text::install`], which the application calls at
/// start-up with the ladder its `Backends` already holds. Everything else here is
/// the lowering's.
pub mod text;

pub use adapt::{Each, Switch, When, each, each_into, switch, when};
pub use children::{Children, IntoChildren};
pub use el::{Any, Button, El, Path, View};
pub use host::Host;
pub(crate) use host::Placement;
pub use mount::{Mount, geometry, mount, mount_at, set_geometry};

/// Where a structural adapter builds: the group its rows or arms become children of, the
/// sibling they sit after, and the scope they resolve against.
///
/// The parent is the **enclosing container**, not the adapter's own node. A list is laid out
/// by the container it was passed to — a `stack` stacks its rows, a grid flows them — which
/// it cannot be if a box the author never wrote sits in between.
///
/// [`after`](Self::after) is what makes that safe. The adapter's own node stays in the
/// parent's child list as a hidden **anchor**, and every row is placed after it, so rows land
/// between the anchor and whatever sibling follows. Without a per-adapter identity there,
/// two adjacent lists that are both empty share a predecessor and whichever fills second
/// lands first.
///
/// Deliberately narrow: an adapter needs a parent, a position and a scope and nothing else,
/// and a wider handle is how a god-trait starts.
#[derive(Copy, Clone, Debug)]
pub struct Site {
    pub parent: windows_scene::GroupId,
    /// The adapter's anchor: the node a first row or arm is placed after.
    pub after: Option<windows_scene::NodeId>,
    pub scope: crate::role::Scope,
}
