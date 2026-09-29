//! Parent-first declarations write directly into retained records.

pub(crate) mod binding;
pub(crate) mod control;
pub(crate) mod field;
pub(crate) mod geometry;
pub(crate) mod hits;
pub(crate) mod host;
pub(crate) mod mount;
#[cfg(test)]
pub(crate) mod rig;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) mod text;
pub(crate) mod theme;
pub(crate) mod tree;
pub(crate) mod ui;

/// An element with no further vocabulary of its own.
pub struct Any;
/// An element whose shape is a retained path.
pub struct Path;
/// An element whose paint is a presentation region.
pub struct Region;
/// An element whose text the input stack edits.
pub struct Field;

pub use control::{Scalar, Shortcut};
pub use mount::{Stop, root_scope, set_geometry, set_ramp};
pub use ui::{Element, Node, Ui};

pub(crate) use host::{Entrance, Placement};
pub(crate) use mount::Mount;

// The driver's own tests drive the host directly, so it is public under the test-support
// feature and crate-private otherwise.
#[cfg(any(test, feature = "test-support"))]
pub use host::Host;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use host::Host;
