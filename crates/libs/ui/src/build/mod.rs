//! Parent-first declarations write directly into retained records.
mod binding;
mod control;
pub use control::Scalar;
mod ui;
pub use ui::{Element, Node, Ui};
pub struct Any;
pub struct Path;
pub struct Region;
pub struct Field;
pub(crate) mod field;
mod geometry;
mod host;
mod mount;
mod style;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) mod text;
mod theme;
#[cfg(any(test, feature = "test-support"))]
pub use host::Host;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use host::Host;
pub(crate) use host::{Placement, ScrollId};
pub(crate) use mount::Mount;
pub use mount::{Stop, root_scope, set_geometry, set_ramp};
