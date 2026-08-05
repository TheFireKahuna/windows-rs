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
use crate::role::{Metric, Scope, WidthClass};
use windows_scene::taffy;
use windows_scene::taffy::style_helpers::{TaffyAuto, TaffyGridLine, TaffyZero};

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
    /// `flex_shrink: 0` — keep the height stated, in a box too small for it.
    ///
    /// What a scroll container's content is: overflow is the whole point, and a flex child
    /// squeezed back to its parent has none. Without it a scroll surface only overflows
    /// where every child happens to pin a minimum, which is a scrollbar that works by
    /// accident.
    NoShrink,
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
    /// Drops the column template accumulated so far.
    ///
    /// What a class-gated column list opens with, so that its tracks are *the* template at
    /// that class rather than an addition to the one stated below it. Without it,
    /// `.cols(..).cols_when(Wide, ..)` silently concatenates and the wide arm gets five
    /// tracks for two declarations — a write that goes somewhere nobody asked for.
    ClearColumns,
    /// The layout class this recipe re-bases on.
    ///
    /// Not applied in sequence like every other override: a preset **is** the base, so
    /// applying one mid-list would wipe the overrides before it. [`lower_with`] resolves the
    /// effective preset first, from the last active one of these, and then applies the rest
    /// in order. That is what lets a width class change a flex direction without a second
    /// storage mechanism beside the override list.
    Class(Preset),
    /// The minimum tile width, for [`Preset::Tiles`].
    TileMin(Len),
    /// Taken out of flow, and positioned against the containing block.
    ///
    /// What chrome uses: a control's fill and its interaction wash are absolute at inset
    /// zero, so they cover the node rather than being laid out beside its content.
    Absolute,
    /// All four insets at once.
    Inset(Len),
    /// A uniform row, placed out of flow at a fixed offset down its container.
    ///
    /// Stated by the container on the child's behalf, as [`Place`](Self::Place) is. What a
    /// virtualized list places its rows with: out of flow, the container's extent is the
    /// whole list's rather than the realized subset's, so the scroll extent does not move
    /// when the window does — and the realized set is free to be several disjoint runs.
    Band {
        at: Len,
        height: Len,
    },
    /// Explicit grid placement, stated by the **container** on the child's behalf.
    Place {
        row: u16,
        column: u16,
        row_span: u16,
        column_span: u16,
    },
}

/// One override, and the width class it applies at.
///
/// A rule names a class; it does not *hold* one. The class a recipe is lowered at is the
/// solve's and arrives in the [`Scope`], so a gated rule is a predicate over that one
/// authority rather than a second copy of it — which is what keeps the recipe class-free
/// while still letting a container change shape when its width class moves.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rule {
    /// The class this applies at, or `None` for every class.
    pub at: Option<WidthClass>,
    pub over: Over,
}

impl Rule {
    /// At every width class.
    #[must_use]
    pub const fn always(over: Over) -> Self {
        Self { at: None, over }
    }

    /// At `class`, and no other.
    ///
    /// Exact rather than a range or a set: the three call sites that exist want one class
    /// each, and a caller who wants two states two. A set type here would be a vocabulary to
    /// keep in step with `WidthClass` for a case nothing has yet asked for.
    #[must_use]
    pub const fn at(class: WidthClass, over: Over) -> Self {
        Self {
            at: Some(class),
            over,
        }
    }

    /// Whether this rule applies in `scope`.
    #[must_use]
    const fn applies(&self, scope: Scope) -> bool {
        match self.at {
            None => true,
            Some(class) => class as u8 == scope.width as u8,
        }
    }
}

impl From<Over> for Rule {
    fn from(over: Over) -> Self {
        Self::always(over)
    }
}

/// The window root's style, and there is exactly one correct value for it.
///
/// The root **is** the client area: the model is told the window's size and solves against
/// it, so anything but a full-extent box either leaves a strip of window nothing lays out in
/// or overflows one. There is no decision here for an application to make, which is why
/// [`Ui::run`](crate::driver::Ui::run) no longer asks for one.
///
/// A **stretching column**, and both halves are load-bearing. A row would give its children
/// their content height, so a shell would size to what it contains instead of to the window
/// — a chain that runs off the bottom edge, and a scroll viewport whose height resolves to
/// zero before its tracker is created. Stretch is what gives a child the full inline extent,
/// without which a scroll container's viewport is zero DIPs wide and its interaction source
/// hit-tests nothing while reporting success.
#[must_use]
pub fn root() -> taffy::Style {
    taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: taffy::FlexDirection::Column,
        align_items: Some(Align::Stretch.items()),
        size: taffy::Size {
            width: taffy::Dimension::percent(1.0),
            height: taffy::Dimension::percent(1.0),
        },
        ..taffy::Style::DEFAULT
    }
}

/// Lowers a recipe. The one place a `taffy::Style` is built.
///
/// A flex class allocates nothing; a grid one allocates its track templates, because that is
/// what `taffy::Style` holds them in. It is one `Vec` per grid node per lower, and a screen
/// has few grids — but it is the one thing on this path that is not free, and it is taffy's
/// to fix rather than this crate's.
#[must_use]
pub fn lower(preset: Preset, rules: &[Rule], scope: Scope) -> taffy::Style {
    lower_with(preset, rules, &[], scope)
}

/// The same, with more overrides on the end.
///
/// What a style that follows a value needs: it re-lowers from the node's **own** recipe, so a
/// width class that moved in between is already in the answer and there is no second copy to
/// fall out of date. Taking the extras here rather than appending to a copy is what keeps that
/// re-lower allocation-free.
///
/// A **slice** and not one override, because a column template is several: `ClearColumns`
/// followed by a track each. One-override-per-effect made two bound styles on one node
/// clobber each other, since each lowered from the recipe plus only its own.
///
/// The extras are unconditional by construction — they are this frame's values for bound
/// properties, not design decisions that could belong to one class.
#[must_use]
pub fn lower_with(preset: Preset, rules: &[Rule], extra: &[Over], scope: Scope) -> taffy::Style {
    let active = || rules.iter().filter(|r| r.applies(scope)).map(|r| r.over);
    // The base first, from the last class that claimed it: a preset replaces the style
    // wholesale, so resolving it in sequence would discard every override written before it.
    let preset = active()
        .filter_map(|over| match over {
            Over::Class(preset) => Some(preset),
            _ => None,
        })
        .next_back()
        .unwrap_or(preset);
    let mut style = base(preset, scope);
    for over in active().chain(extra.iter().copied()) {
        apply(&mut style, over, scope);
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
        // A **column**, and the direction is the whole of what this preset is for.
        //
        // A single-line run is its own sprite and has no children, so nothing here reaches
        // it. A wrapping one owns a sprite per line, and `Style::DEFAULT` is a flex *row* —
        // which laid every line of a paragraph out side by side, so a caption came out one
        // line tall and drew straight off the end of its column. The lines carry a definite
        // size of their own, so stretch never touches them and the group's content height
        // is their sum.
        Preset::Text => taffy::Style {
            display: taffy::Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            ..taffy::Style::DEFAULT
        },
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
        Over::NoShrink => style.flex_shrink = 0.0,
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
        Over::Band { at, height } => {
            style.position = taffy::Position::Absolute;
            style.size.height = height.dimension(scope);
            style.inset = taffy::Rect {
                left: taffy::LengthPercentageAuto::ZERO,
                right: taffy::LengthPercentageAuto::ZERO,
                top: at.length_percentage_auto(scope),
                bottom: taffy::LengthPercentageAuto::AUTO,
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
        Over::ClearColumns => style.grid_template_columns.clear(),
        // Resolved before the base was built, and it is the base.
        Over::Class(_) => {}
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
