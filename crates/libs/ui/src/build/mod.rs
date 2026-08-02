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
#[cfg(test)]
mod tests;
mod text;

pub use adapt::{Each, Switch, When, each, switch, when};
pub use children::{IntoChildren, Slots};
pub use el::{Any, Button, El, Path, View};
pub use host::Host;
pub use mount::{Mount, geometry, mount, mount_at, set_geometry};

/// Where a structural adapter builds: the mounted group its rows or arms parent to, and the
/// scope they resolve against.
///
/// Deliberately narrow: an adapter needs a parent and a scope and nothing else, and a
/// wider handle is how a god-trait starts.
#[derive(Copy, Clone, Debug)]
pub struct Site {
    pub parent: windows_scene::GroupId,
    pub scope: crate::role::Scope,
}
