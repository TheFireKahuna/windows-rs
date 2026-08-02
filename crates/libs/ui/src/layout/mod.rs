//! Layout: the length vocabulary, the style presets, and the classes over them.
//!
//! Taffy implements all of this, so these are presets over one `Style` and they are cheap
//! *because* the engine does the work. Every class exists in two forms over one const table:
//! a free function here, and an `El` method. `stack(c)` is `El::seed(Bare).stack(c)`, which
//! is what keeps surfaces × classes from being a grid of signatures.

mod len;
mod preset;
mod scroll;

pub use len::{Align, Len, Track};
pub use preset::{Over, Preset, lower, lower_with};
pub use scroll::{
    ListSpec, Reveal, ScrollState, THUMB_MARGIN, THUMB_MIN_H, THUMB_W, ThumbGeom, list, scroll,
    scroll_with, thumb_geom, thumb_style, window,
};

use crate::build::{El, IntoChildren, View};

/// A column. Children stretch; the gap comes from the scope.
#[must_use]
pub fn stack(children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).stack(children)
}

/// A row. Children centre; the gap comes from the scope.
#[must_use]
pub fn row(children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).row(children)
}

/// A row that wraps.
#[must_use]
pub fn wrap(children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).wrap(children)
}

/// An explicit grid. Children are auto-placed unless the container places them.
#[must_use]
pub fn grid(children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).grid(children)
}

/// Responsive tiles: `repeat(auto-fill, minmax(min, 1fr))`.
///
/// No column count is computed anywhere; the track expression is the whole of it.
#[must_use]
pub fn tiles(min: impl Into<Len>, children: impl IntoChildren) -> View {
    El::seed(Preset::Bare).tiles(min, children)
}

/// Absorbs the slack.
///
/// This replaces the prior pattern for bottom-aligning a card's call to action: a
/// three-track grid, two placement calls, an explicit track list and a row spacing.
#[must_use]
pub fn spacer() -> View {
    El::seed(Preset::Bare).grow()
}

/// A container that classifies its own inline size for its subtree.
///
/// A caller never passes a width down, so a body is correct in a full-width row, a narrow
/// column and a detail pane at once. A width variant changes **styles, never structure**.
#[must_use]
pub fn responsive(bounds: [f32; 2], children: impl IntoChildren) -> View {
    El::seed(Preset::Bare)
        .stack(children)
        .responsive(bounds[0], bounds[1])
}
