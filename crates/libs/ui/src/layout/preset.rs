//! The style recipe: a preset plus its overrides, lowered to a `taffy::Style`.
//!
//! A slot carries the recipe rather than the style, because a scope's `width` axis is
//! decided inside the solve and moves whenever a window crosses a threshold. [`lower`] runs
//! at mount and again for a subtree whose class moved — already the scope of that work,
//! since taffy's cache key excludes the class. `Model::style` compares before it pushes, so
//! a class change that moves no metric emits no op.
//!
//! This is the only producer of a `taffy::Style` in the crate, and nothing above it names a
//! taffy type.

use super::len::{Align, Len, Track};
use crate::role::{Metric, Scope};
use windows_scene::taffy;
use windows_scene::taffy::style_helpers::{TaffyGridLine, TaffyZero};

/// Which const row a slot's style starts from.
///
/// A layout class is a row here, not a struct and not a builder. `stack`, `row` and `wrap`
/// differ in four fields; making each a type would make them four types that have to agree.
///
/// **Every variant is a layout class, and nothing else is.** Chrome — a card's padding, a
/// button's row height — is an override ([`El::surface`](crate::build::El::surface),
/// [`El::control`](crate::build::El::control)), so it composes with any class and a layout
/// modifier can always set one. A node that carried both would have to choose between them,
/// and the losing half goes missing without a diagnostic.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Preset {
    /// No layout opinion at all. A leaf, or a container that states everything itself.
    #[default]
    Bare,
    /// A column. Children stretch.
    Stack,
    /// A row. Children centre.
    Row,
    /// A row that wraps.
    Wrap,
    /// An explicit grid.
    Grid,
    /// `repeat(auto-fill, minmax(min, 1fr))` — the responsive tile track.
    Tiles,
    /// A scroll container: overflow clipped, a tracker on the inside.
    Scroll,
    /// A text run. Content-sized, and it measures.
    Text,
}

/// One departure from a preset.
///
/// Every variant that carries a length carries a [`Len`] or a [`Track`], never an `f32`.
/// That is the whole of "a widget may not accept a spacing", expressed as a type rather
/// than as a lint.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Over {
    Width(Len),
    Height(Len),
    MinWidth(Len),
    MinHeight(Len),
    MaxWidth(Len),
    MaxHeight(Len),
    Padding(Len),
    Gap(Len),
    /// `flex_grow: 1` — absorb the slack.
    Grow,
    /// A container aligning **all** of its children.
    Align(Align),
    /// Along the main axis.
    Justify(Align),
    /// The rare per-child escape.
    AlignSelf(Align),
    /// Not laid out, and not drawn. What a width variant uses, so that nothing unmounts
    /// while a window is being dragged across a threshold.
    Hidden,
    /// A track appended to the row template.
    Row(Track),
    /// A track appended to the column template.
    Column(Track),
    /// The minimum tile width, for [`Preset::Tiles`].
    TileMin(Len),
    /// Taken out of flow, and positioned against the containing block.
    ///
    /// What chrome uses: a control's fill and its interaction wash are absolute at inset
    /// zero, so they cover the node rather than being laid out beside its content.
    Absolute,
    /// All four insets at once.
    Inset(Len),
    /// Explicit grid placement, stated by the **container** on the child's behalf.
    Place {
        row: u16,
        column: u16,
        row_span: u16,
        column_span: u16,
    },
}

/// Lowers a recipe. The one place a `taffy::Style` is built.
///
/// A flex class allocates nothing; a grid one allocates its track templates, because that is
/// what `taffy::Style` holds them in. It is one `Vec` per grid node per lower, and a screen
/// has few grids — but it is the one thing on this path that is not free, and it is taffy's
/// to fix rather than this crate's.
#[must_use]
pub fn lower(preset: Preset, over: &[Over], scope: Scope) -> taffy::Style {
    lower_with(preset, over, None, scope)
}

/// The same, with one more override on the end.
///
/// What a style that follows a value needs: it re-lowers from the node's **own** recipe, so a
/// width class that moved in between is already in the answer and there is no second copy to
/// fall out of date. Taking the extra here rather than appending to a copy is what keeps that
/// re-lower allocation-free.
#[must_use]
pub fn lower_with(
    preset: Preset,
    over: &[Over],
    extra: Option<Over>,
    scope: Scope,
) -> taffy::Style {
    let mut style = base(preset, scope);
    for o in over.iter().copied().chain(extra) {
        apply(&mut style, o, scope);
    }
    style
}

fn base(preset: Preset, scope: Scope) -> taffy::Style {
    let gap = |m: Metric| taffy::LengthPercentage::length(crate::role::metric(m, scope));
    match preset {
        Preset::Bare => taffy::Style::DEFAULT,
        Preset::Stack => taffy::Style {
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            align_items: Some(Align::Stretch.items()),
            gap: taffy::Size {
                width: taffy::LengthPercentage::ZERO,
                height: gap(Metric::SpaceMd),
            },
            ..taffy::Style::DEFAULT
        },
        Preset::Row => taffy::Style {
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Row,
            align_items: Some(Align::Center.items()),
            gap: taffy::Size {
                width: gap(Metric::SpaceMd),
                height: taffy::LengthPercentage::ZERO,
            },
            ..taffy::Style::DEFAULT
        },
        Preset::Wrap => taffy::Style {
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Row,
            flex_wrap: taffy::FlexWrap::Wrap,
            align_items: Some(Align::Center.items()),
            gap: taffy::Size {
                width: gap(Metric::SpaceMd),
                height: gap(Metric::SpaceMd),
            },
            ..taffy::Style::DEFAULT
        },
        Preset::Grid => taffy::Style {
            display: taffy::Display::Grid,
            gap: taffy::Size {
                width: gap(Metric::SpaceMd),
                height: gap(Metric::SpaceMd),
            },
            ..taffy::Style::DEFAULT
        },
        // `TileMin` replaces the minimum; no column count is computed anywhere.
        Preset::Tiles => taffy::Style {
            display: taffy::Display::Grid,
            grid_template_columns: vec![tile_track(Len::Metric(Metric::CardMinW), scope)],
            gap: taffy::Size {
                width: gap(Metric::SpaceMd),
                height: gap(Metric::SpaceMd),
            },
            ..taffy::Style::DEFAULT
        },
        Preset::Scroll => taffy::Style {
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            align_items: Some(Align::Stretch.items()),
            overflow: taffy::Point {
                x: taffy::Overflow::Hidden,
                y: taffy::Overflow::Scroll,
            },
            ..taffy::Style::DEFAULT
        },
        Preset::Text => taffy::Style::DEFAULT,
    }
}

fn tile_track<S: taffy::CheapCloneStr>(min: Len, scope: Scope) -> taffy::GridTemplateComponent<S> {
    taffy::style_helpers::repeat(
        taffy::RepetitionCount::AutoFill,
        vec![Track::MinMax(min, 1.0).sizing(scope)],
    )
}

fn apply(style: &mut taffy::Style, over: Over, scope: Scope) {
    match over {
        Over::Width(l) => style.size.width = l.dimension(scope),
        Over::Height(l) => style.size.height = l.dimension(scope),
        Over::MinWidth(l) => style.min_size.width = l.dimension(scope),
        Over::MinHeight(l) => style.min_size.height = l.dimension(scope),
        Over::MaxWidth(l) => style.max_size.width = l.dimension(scope),
        Over::MaxHeight(l) => style.max_size.height = l.dimension(scope),
        Over::Padding(l) => {
            let v = l.length_percentage(scope);
            style.padding = taffy::Rect {
                left: v,
                right: v,
                top: v,
                bottom: v,
            };
        }
        Over::Gap(l) => {
            let v = l.length_percentage(scope);
            style.gap = taffy::Size {
                width: v,
                height: v,
            };
        }
        Over::Grow => style.flex_grow = 1.0,
        Over::Absolute => style.position = taffy::Position::Absolute,
        Over::Inset(l) => {
            let v = l.length_percentage_auto(scope);
            style.inset = taffy::Rect {
                left: v,
                right: v,
                top: v,
                bottom: v,
            };
        }
        Over::Align(a) => style.align_items = Some(a.items()),
        Over::Justify(a) => style.justify_content = Some(a.content()),
        Over::AlignSelf(a) => style.align_self = Some(a.items()),
        // Display::None, never an unmount: a half-typed field must survive a window edge
        // being dragged across a breakpoint.
        Over::Hidden => style.display = taffy::Display::None,
        Over::Row(t) => style
            .grid_template_rows
            .push(taffy::GridTemplateComponent::Single(t.sizing(scope))),
        Over::Column(t) => style
            .grid_template_columns
            .push(taffy::GridTemplateComponent::Single(t.sizing(scope))),
        // In place, because `Preset::Tiles` already put one track there and every caller
        // overrides it — assigning a fresh `vec!` would allocate one and drop the other on
        // every lower.
        Over::TileMin(l) => {
            let track = tile_track(l, scope);
            match style.grid_template_columns.first_mut() {
                Some(slot) => *slot = track,
                None => style.grid_template_columns.push(track),
            }
        }
        Over::Place {
            row,
            column,
            row_span,
            column_span,
        } => {
            style.grid_row = placement(row, row_span);
            style.grid_column = placement(column, column_span);
        }
    }
}

fn placement<S: taffy::CheapCloneStr>(at: u16, span: u16) -> taffy::Line<taffy::GridPlacement<S>> {
    // Taffy's grid lines are 1-based and ours are 0-based, because a caller counting cells
    // counts from zero and a caller who has to remember which is which eventually gets it
    // wrong in one of the two places.
    let start = i16::try_from(at).unwrap_or(i16::MAX).saturating_add(1);
    taffy::Line {
        start: taffy::GridPlacement::from_line_index(start),
        end: taffy::GridPlacement::Span(span.max(1)),
    }
}
