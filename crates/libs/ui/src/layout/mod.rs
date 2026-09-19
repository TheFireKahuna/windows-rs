//! The authored layout vocabulary, and the solver that turns it into boxes.
//!
//! Containers write retained records through `Ui`; their child closures run synchronously.
//! One `Copy` [`Layout`] per node is the whole declaration — a [`Preset`] row with authored
//! fields replacing it, and no second sparse record — and [`solve_root`] is the only reader.

mod anchor;
#[expect(clippy::module_inception, reason = "the vocabulary the module is named for")]
mod layout;
mod probe;
mod responsive;
mod scroll;
pub(crate) mod solve;

pub use anchor::{Anchored, Anchors, Table, anchors};
pub use layout::{
    Align, COLUMN_CAP, Edge, Layout, Len, Position, Preset, Rect, TRACK_CAP, Template, Templates,
    Track, TrackMax,
};
pub use probe::{Placed, Probe, probe};
pub use responsive::{Bounds, HYSTERESIS_DIPS, WidthClass};
pub use scroll::{
    ListSpec, ListState, OVERSCAN, Pos, RAIL, Realized, Reveal, Rows, ScrollDecl, THUMB_MARGIN,
    THUMB_MIN_H, THUMB_W, ThumbGeom, list, list_state, observe as scroll_observe, realize, scroll,
    scroll_for_thumb_y, scroll_list, scroll_with, thumb_geom, thumb_y_for_scroll, window,
};
pub use solve::{shift, snap, solve_root};

pub(crate) use scroll::{ScrollRow, ScrollTable, front as scroll_front};

use crate::build::{Element, Ui};

/// Returns a container stacking `body` along the block axis.
pub fn stack<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.group(Preset::Stack, body)
}

/// Returns a container running `body` along the inline axis.
pub fn row<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.group(Preset::Row, body)
}

/// Returns a row that breaks `body` into lines.
pub fn wrap<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.group(Preset::Wrap, body)
}

/// Returns a container placing `body` on tracks.
pub fn grid<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.group(Preset::Grid, body)
}

/// Returns the container with no opinion, which overlaps `body` in one box.
pub fn layer<'a>(ui: &'a mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    ui.group(Preset::Layer, body)
}

/// Returns a container that classifies its own inline width for `body`.
pub fn responsive<'a>(
    ui: &'a mut Ui<'_>,
    narrow_max: f32,
    medium_max: f32,
    body: impl FnOnce(&mut Ui<'_>),
) -> Element<'a> {
    ui.node(Preset::Layer)
        .responsive([narrow_max, medium_max])
        .children(body)
}
