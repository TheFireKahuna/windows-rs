//! Lengths and grid tracks, in the units the authoring surface admits.
//!
//! [`Len`] has no raw-DIP constructor. Every length-taking method takes `impl Into<Len>`,
//! and the only conversions into it are from a [`Metric`] — which the palette owns and
//! resolves against the enclosing [`Scope`] — and from the variants declared here. So
//! `.padding(12.0)` does not compile and a spacing is always the theme's.
//!
//! [`Len::Zero`] is the one raw constant: a floor of exactly zero is a layout instruction
//! rather than a spacing, and it is what lets a child shrink below its content size.

use crate::role::{Metric, Scope, metric};
use windows_scene::taffy;
use windows_scene::taffy::style_helpers::{
    FromFr, TaffyAuto, TaffyMaxContent, TaffyMinContent, minmax,
};

/// A length in the authoring surface.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Len {
    /// The palette's, resolved against the enclosing [`Scope`].
    Metric(Metric),
    /// Exactly zero.
    Zero,
    /// A fraction of the containing block, `0.0..=1.0`.
    Pct(f32),
    /// A multiple of a metric: `n` of them, end to end.
    ///
    /// What a virtualized list states its extent in — forty unrealized rows occupy forty
    /// row heights. The metric is still the palette's and `n` is a count, so this states no
    /// arbitrary DIP length.
    Times(Metric, f32),
    /// Sized by content.
    Auto,
}

impl From<Metric> for Len {
    fn from(m: Metric) -> Self {
        Self::Metric(m)
    }
}

impl Len {
    /// Returns this length in DIPs, or `None` where it has no intrinsic value.
    ///
    /// `Pct` and `Auto` both answer `None`: a fraction resolves only against a containing
    /// block, and `Auto` only against content.
    #[must_use]
    pub fn dips(self, scope: Scope) -> Option<f32> {
        match self {
            Self::Metric(m) => Some(metric(m, scope)),
            Self::Times(m, n) => Some(metric(m, scope) * n),
            Self::Zero => Some(0.0),
            Self::Pct(_) | Self::Auto => None,
        }
    }

    /// Returns this length as a size, resolving any metric against `scope`.
    ///
    /// Carries all five cases, so a percentage stays a percentage for the solve to resolve
    /// and `Auto` stays content-sized.
    #[must_use]
    pub fn dimension(self, scope: Scope) -> taffy::Dimension {
        match self {
            Self::Metric(m) => taffy::Dimension::length(metric(m, scope)),
            Self::Times(m, n) => taffy::Dimension::length(metric(m, scope) * n),
            Self::Zero => taffy::Dimension::length(0.0),
            Self::Pct(p) => taffy::Dimension::percent(p),
            Self::Auto => taffy::Dimension::AUTO,
        }
    }

    /// Returns this length as a padding, border or gap, where `Auto` resolves to zero.
    #[must_use]
    pub fn length_percentage(self, scope: Scope) -> taffy::LengthPercentage {
        match self {
            Self::Metric(m) => taffy::LengthPercentage::length(metric(m, scope)),
            Self::Times(m, n) => taffy::LengthPercentage::length(metric(m, scope) * n),
            Self::Zero | Self::Auto => taffy::LengthPercentage::length(0.0),
            Self::Pct(p) => taffy::LengthPercentage::percent(p),
        }
    }

    /// Returns this length as a margin or an inset, where `Auto` centres.
    #[must_use]
    pub fn length_percentage_auto(self, scope: Scope) -> taffy::LengthPercentageAuto {
        match self {
            Self::Metric(m) => taffy::LengthPercentageAuto::length(metric(m, scope)),
            Self::Times(m, n) => taffy::LengthPercentageAuto::length(metric(m, scope) * n),
            Self::Zero => taffy::LengthPercentageAuto::length(0.0),
            Self::Pct(p) => taffy::LengthPercentageAuto::percent(p),
            Self::Auto => taffy::LengthPercentageAuto::AUTO,
        }
    }
}

/// One grid track.
///
/// Separate from [`Len`] because `fr` is a length only inside a track: it has no meaning
/// as the width of a flex child.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Track {
    Fixed(Len),
    /// A share of the leftover space.
    Fr(f32),
    Auto,
    MinContent,
    MaxContent,
    /// The responsive tile track: at least `min`, and a share of what is left.
    MinMax(Len, f32),
}

impl From<Metric> for Track {
    fn from(m: Metric) -> Self {
        Self::Fixed(Len::Metric(m))
    }
}

impl From<Len> for Track {
    fn from(l: Len) -> Self {
        Self::Fixed(l)
    }
}

impl Track {
    /// Returns this track's sizing function, resolving any metric against `scope`.
    ///
    /// The fixed part goes through [`Len::dimension`], which carries all five length cases,
    /// so a percentage track is resolved by the grid against its container and an `Auto`
    /// track is sized by content. [`Len::dips`] answers `None` for both of those, and a
    /// track built from it would be a zero-width column.
    #[must_use]
    pub fn sizing(self, scope: Scope) -> taffy::TrackSizingFunction {
        match self {
            Self::Fixed(l) => taffy::TrackSizingFunction::from(l.dimension(scope)),
            Self::Fr(f) => taffy::TrackSizingFunction::from_fr(f),
            Self::Auto => taffy::TrackSizingFunction::AUTO,
            Self::MinContent => taffy::TrackSizingFunction::MIN_CONTENT,
            Self::MaxContent => taffy::TrackSizingFunction::MAX_CONTENT,
            Self::MinMax(min, fr) => minmax(
                taffy::MinTrackSizingFunction::from(min.dimension(scope)),
                taffy::MaxTrackSizingFunction::from_fr(fr),
            ),
        }
    }
}

/// How a container aligns all of its children.
///
/// Alignment is a container property; the per-child escape is
/// [`El::align_self`](crate::build::El::align_self).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Align {
    Start,
    Center,
    #[default]
    Stretch,
    End,
    /// Only meaningful along the main axis, where it distributes the slack.
    SpaceBetween,
}

impl Align {
    /// Returns the cross-axis alignment for the container's children.
    ///
    /// `SpaceBetween` has no cross-axis meaning and lowers to `STRETCH`.
    #[must_use]
    pub const fn items(self) -> taffy::AlignItems {
        match self {
            Self::Start => taffy::AlignItems::START,
            Self::Center => taffy::AlignItems::CENTER,
            Self::Stretch | Self::SpaceBetween => taffy::AlignItems::STRETCH,
            Self::End => taffy::AlignItems::END,
        }
    }

    /// Returns the main-axis distribution for the container's content.
    #[must_use]
    pub const fn content(self) -> taffy::AlignContent {
        match self {
            Self::Start => taffy::AlignContent::START,
            Self::Center => taffy::AlignContent::CENTER,
            Self::Stretch => taffy::AlignContent::STRETCH,
            Self::End => taffy::AlignContent::END,
            Self::SpaceBetween => taffy::AlignContent::SPACE_BETWEEN,
        }
    }
}
