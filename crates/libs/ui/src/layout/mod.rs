//! Layout: the length vocabulary, the style presets, and the layout classes over them.
//!
//! Taffy runs the solve; a layout class here is one row of a const preset table lowered to
//! a `taffy::Style`. Every class exists in two forms over that one table: a free function
//! in this module, and an [`El`] method. `stack(c)` is `El::seed(Preset::Bare).stack(c)`.

mod len;
mod preset;
mod probe;
mod scroll;

pub use len::{Align, Len, Track};
pub use preset::{Edge, Over, Preset, Rule, lower, lower_with, root};
pub(crate) use probe::ProbeRow;
pub use probe::{Placed, Probe, probe};
pub use scroll::{
    ListSpec, Realized, Reveal, ScrollDecl, ScrollState, THUMB_MARGIN, THUMB_MIN_H, THUMB_W,
    ThumbGeom, front as scroll_front, list, observe as scroll_observe, rail_style, realize, scroll,
    scroll_for_thumb_y, scroll_with, thumb_geom, thumb_style, thumb_y_for_scroll, window,
};
pub(crate) use scroll::{ScrollRow, grab_decl, grab_hit};

use crate::build::{El, IntoChildren, View};

/// Returns a column of `children`. Children stretch, and the gap comes from the scope.
#[must_use]
pub fn stack(children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).stack(children)
}

/// Returns a row of `children`. Children centre, and the gap comes from the scope.
#[must_use]
pub fn row(children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).row(children)
}

/// Returns a row of `children` that wraps onto further lines.
#[must_use]
pub fn wrap(children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).wrap(children)
}

/// Returns an explicit grid. Children are auto-placed unless the container places them.
#[must_use]
pub fn grid(children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).grid(children)
}

/// Returns a tile grid whose single track is `repeat(auto-fill, minmax(min, 1fr))`.
///
/// The track expression decides how many columns fit; no column count is computed here.
#[must_use]
pub fn tiles(min: impl Into<Len>, children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).tiles(min, children)
}

/// Returns an empty node with `flex_grow: 1`, absorbing its container's slack.
#[must_use]
pub fn spacer() -> View {
    El::seed(Preset::Bare).grow()
}

/// Returns a column that classifies its own inline size for its subtree.
///
/// `bounds` is `[narrow_max, medium_max]` in DIPs: the two widths that separate the three
/// width classes. Descendants resolve their metrics against the class this container
/// publishes, so no caller passes a width down. A width class changes styles, never
/// structure — nothing unmounts while a window edge is dragged across a bound.
#[must_use]
pub fn responsive(bounds: [f32; 2], children: impl IntoChildren) -> View {
    El::seed(Preset::Bare)
        .stack(children)
        .responsive(bounds[0], bounds[1])
}
