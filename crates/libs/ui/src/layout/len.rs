//! Lengths and tracks — and the reason a widget cannot express a spacing.
//!
//! A call site writing `.font_size(14.0)` or `.spacing(8.0)` is doing the theme's job. A
//! lint catches that after it is written; a type stops it being written:
//!
//! > **[`Len`] has no raw-DIP constructor.**
//!
//! Every length-taking method takes `impl Into<Len>`, and the only things that convert are a
//! [`Metric`] — which the palette owns and resolves against the enclosing scope — and the
//! shapes below, which carry no design decision. `.padding(12.0)` does not compile.
//!
//! [`Len::Zero`] is the one honest constant: a floor of exactly zero is a layout instruction
//! rather than a spacing, and it is what lets a child shrink below its content instead of
//! inflating its container.

use crate::role::{Metric, Scope, metric};
use windows_scene::taffy;
use windows_scene::taffy::style_helpers::{
    FromFr, FromLength, TaffyAuto, TaffyMaxContent, TaffyMinContent, minmax,
};

/// A length in the authoring surface.
///
/// Adding a variant here, or a variant to [`Metric`], is a lower-crate API addition and
/// therefore an escalation rather than a merge.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Len {
    /// The palette's, resolved against the enclosing [`Scope`].
    Metric(Metric),
    /// Exactly zero.
    Zero,
    /// A fraction of the containing block, `0.0..=1.0`.
    Pct(f32),
    /// A **multiple** of a metric: `n` of them, end to end.
    ///
    /// The one thing virtualization cannot do without — the space forty unrealized rows
    /// occupy is forty row heights — and it does not reopen the door this type exists to
    /// close. It can say "forty rows"; it cannot say "twelve DIPs", because the metric is
    /// still the palette's and the number is a **count**, not a length. A caller wanting an
    /// arbitrary size still has nothing to write.
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
    /// This length in DIPs, or `None` where it has no intrinsic value.
    ///
    /// `Pct` answers `None` because a fraction is not a length until something says of
    /// what, and `Auto` because it is a question rather than an answer.
    #[must_use]
    pub fn dips(self, scope: Scope) -> Option<f32> {
        match self {
            Self::Metric(m) => Some(metric(m, scope)),
            Self::Times(m, n) => Some(metric(m, scope) * n),
            Self::Zero => Some(0.0),
            Self::Pct(_) | Self::Auto => None,
        }
    }

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

    /// As a padding, border or gap, where `Auto` has no meaning and reads as zero.
    #[must_use]
    pub fn length_percentage(self, scope: Scope) -> taffy::LengthPercentage {
        match self {
            Self::Metric(m) => taffy::LengthPercentage::length(metric(m, scope)),
            Self::Times(m, n) => taffy::LengthPercentage::length(metric(m, scope) * n),
            Self::Zero | Self::Auto => taffy::LengthPercentage::length(0.0),
            Self::Pct(p) => taffy::LengthPercentage::percent(p),
        }
    }

    /// As a margin or an inset, where `Auto` centres.
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
/// Separate from [`Len`] because `fr` is only a length inside a track, and a type that
/// carried it everywhere would have to answer what `width: 1fr` means on a flex child —
/// which is a question with two plausible answers and therefore a footgun.
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
    #[must_use]
    pub fn sizing(self, scope: Scope) -> taffy::TrackSizingFunction {
        match self {
            Self::Fixed(l) => taffy::TrackSizingFunction::from_length(l.dips(scope).unwrap_or(0.0)),
            Self::Fr(f) => taffy::TrackSizingFunction::from_fr(f),
            Self::Auto => taffy::TrackSizingFunction::AUTO,
            Self::MinContent => taffy::TrackSizingFunction::MIN_CONTENT,
            Self::MaxContent => taffy::TrackSizingFunction::MAX_CONTENT,
            Self::MinMax(min, fr) => minmax(
                taffy::MinTrackSizingFunction::from_length(min.dips(scope).unwrap_or(0.0)),
                taffy::MaxTrackSizingFunction::from_fr(fr),
            ),
        }
    }
}

/// How a container aligns **all** of its children.
///
/// Alignment is a container property. The per-child escape is `align_self`, and it should
/// be rare: stating it once per container is what stops every child restating it.
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
    #[must_use]
    pub const fn items(self) -> taffy::AlignItems {
        match self {
            Self::Start => taffy::AlignItems::START,
            Self::Center => taffy::AlignItems::CENTER,
            Self::Stretch | Self::SpaceBetween => taffy::AlignItems::STRETCH,
            Self::End => taffy::AlignItems::END,
        }
    }

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
