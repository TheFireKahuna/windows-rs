//! The lowering: the build arena, the element, and the walk that turns both into `Model`
//! calls.
//!
//! This module carries the chain from a builder call to a `SinkPatch`, over the alphabets
//! below it — `taffy::Style`, the sink alphabet, `Bind` over `Prop`, `HitDecl`,
//! `GestureDecl` — and the total function `resolve`. It adds no alphabet of its own.
//!
//! It is also the only code that touches `Model`. A widget is a data seed: it writes into
//! the build arena, and it neither calls `Model`, nor resolves a `Radiance`, nor builds a
//! `taffy::Style`.

mod adapt;
mod arena;
mod children;
mod el;
mod host;
mod mount;
mod style;
#[cfg(test)]
pub(crate) mod tests;
/// Holds the thread's shaping engine and the table of laid-out runs behind every label.
///
/// [`text::install`] is the one item an application calls, at start-up, with the ladder its
/// `Backends` already holds. Everything else here is the lowering's.
pub mod text;

pub use adapt::{Each, Switch, When, each, each_into, switch, when};
pub use children::{Children, IntoChildren};
pub use el::{Any, Button, El, Path, View};
pub use host::Host;
pub(crate) use host::Placement;
pub use mount::{Mount, geometry, mount, mount_at, set_geometry};

/// Names where a structural adapter builds: the group its rows or arms become children of,
/// the sibling they sit after, and the scope they resolve against.
///
/// The parent is the enclosing container, not the adapter's own node, so the container the
/// list was passed to lays the rows out: a `stack` stacks them, a grid flows them.
///
/// [`after`](Self::after) carries the adapter's identity in that child list. The adapter's
/// own node stays there as a hidden anchor and every row is placed after it, so rows land
/// between the anchor and whatever sibling follows. Two adjacent lists that are both empty
/// would otherwise share a predecessor, and whichever filled second would land first.
#[derive(Copy, Clone, Debug)]
pub struct Site {
    pub parent: windows_scene::GroupId,
    /// The adapter's anchor: the node a first row or arm is placed after.
    pub after: Option<windows_scene::NodeId>,
    pub scope: crate::role::Scope,
}
