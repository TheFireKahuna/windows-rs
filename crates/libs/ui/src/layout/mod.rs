//! Engine-independent declarations, solve-time lengths and responsive layout.
//!
//! Containers write retained records through `Ui`; their child closures run synchronously.
//! Named `Layout` fields lower into private Taffy styles once per transaction, however many
//! setters wrote them, and never inside the solve.

mod anchor;
mod len;
mod preset;
mod probe;
mod scroll;

pub use anchor::{Anchored, Anchors, Table, anchors};
pub use len::{Align, Len, Track};
pub use preset::root;
pub use preset::{Edge, Layout, Position, Preset};
pub use probe::{Placed, Probe, probe};
pub use scroll::{
    ListSpec, ListState, Realized, Reveal, Rows, ScrollDecl, THUMB_MARGIN, THUMB_MIN_H, THUMB_W,
    ThumbGeom, list, list_state, observe as scroll_observe, rail_style, realize, scroll,
    scroll_for_thumb_y, scroll_list, scroll_with, thumb_geom, thumb_y_for_scroll, window,
};
pub(crate) use scroll::{ScrollRow, ScrollTable, front as scroll_front, grab_decl, grab_hit};

use crate::build::{Element, Ui};

pub fn stack<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.stack(body)
}
pub fn row<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.row(body)
}
pub fn wrap<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.group(Preset::Wrap, body)
}
pub fn grid<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.grid(body)
}
pub fn tiles<'a>(
    ui: &'a mut Ui<'_>,
    min: impl Into<Len>,
    body: impl FnOnce(&mut Ui<'_>),
) -> Element<'a> {
    ui.node(Preset::Tiles).tiles(min, body)
}
pub fn spacer<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    ui.node(Preset::Bare).grow()
}
pub fn responsive<'a>(
    ui: &'a mut Ui<'_>,
    a: f32,
    b: f32,
    body: impl FnOnce(&mut Ui<'_>),
) -> Element<'a> {
    ui.node(Preset::Bare).responsive([a, b]).children(body)
}

pub(crate) use preset::viewport_style;
