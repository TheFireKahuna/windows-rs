//! The style recipe: a preset plus its overrides, lowered to a `taffy::Style`.
//!
//! A slot carries the recipe rather than the style, because a scope's `width` axis is
//! decided inside the solve and moves whenever a window crosses a bound. [`lower`] runs at
//! mount, and again for a subtree whose width class moved. `Model::style` compares before it
//! pushes, so a class change that moves no metric emits no op.
//!
//! This is the only producer of a `taffy::Style` in the crate; nothing above it names a
//! taffy type.

use super::len::{Align, Len, Track};
use crate::role::{Metric, Scope, WidthClass};
use windows_scene::taffy;
use windows_scene::taffy::style_helpers::{TaffyAuto, TaffyGridLine, TaffyZero};

/// Which const row a slot's style starts from.
///
/// A layout class is one row of a const table here, not a type of its own: `stack`, `row`
/// and `wrap` differ in four fields.
///
/// **Every variant is a layout class, and nothing else is.** Chrome — a card's padding, a
/// button's row height — is an [`Over`] instead ([`El::surface`](crate::build::El::surface),
/// [`El::control`](crate::build::El::control)), so it composes with any class rather than
/// competing with one for the same slot.
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
/// Every variant that carries a length carries a [`Len`] or a [`Track`], never an `f32`, so
/// a spacing always resolves from the palette.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Over {
    Width(Len),
    Height(Len),
    MinWidth(Len),
    MinHeight(Len),
    MaxWidth(Len),
    MaxHeight(Len),
    Padding(Len),
    /// Padding as horizontal and vertical, in that order.
    ///
    /// A control is wider than it is tall relative to its text, so the two axes take
    /// different values and one uniform padding cannot state both. Both axes ride one
    /// variant rather than two, because a recipe carries four rules inline and spills to the
    /// heap beyond that — and a control already states four.
    PaddingXY(Len, Len),
    Gap(Len),
    /// `flex_grow: 1` — absorb the slack.
    Grow,
    /// `flex_shrink: 0` — keep the height stated, in a box too small for it.
    ///
    /// What a scroll container's content carries: a flex child squeezed back to its parent
    /// never overflows, and a container with no overflow has no travel and no thumb.
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
    /// What a class-gated column list opens with, so its tracks are *the* template at that
    /// class rather than an addition to the one stated below it. Without it,
    /// `.cols(..).cols_when(Wide, ..)` concatenates, and the wide arm gets five tracks for
    /// two declarations.
    ClearColumns,
    /// The layout class this recipe re-bases on.
    ///
    /// Not applied in sequence like every other override: a preset **is** the base, so
    /// applying one mid-list would wipe the overrides before it. [`lower_with`] resolves the
    /// effective preset first, from the last active rule carrying one, and then applies the
    /// rest in order. A width class can therefore change a flex direction through the same
    /// override list every other rule uses.
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
    /// Taken out of flow, pinned to one edge of the containing block and stretched across it
    /// on the other axis.
    ///
    /// The node's own [`Width`](Self::Width) or [`Height`](Self::Height) gives the extent on
    /// the axis it pins along; the perpendicular axis takes both insets at zero. A node with
    /// neither reads as zero-extent, because nothing else states one.
    ///
    /// Clears any [`Place`](Self::Place), and a `Place` applied afterwards is dropped: an
    /// out-of-flow node is not in the track model, so the containing block is the whole
    /// padding box rather than one cell of it. Order in the override list therefore does not
    /// change the result.
    Edge(Edge),
    /// Explicit grid placement, stated by the **container** on the child's behalf.
    Place {
        row: u16,
        column: u16,
        row_span: u16,
        column_span: u16,
    },
}

/// The edge an out-of-flow node pins to, for [`Over::Edge`].
///
/// Four sides and no centre: an edge float stretches on the axis it does not name, so
/// "centred on both axes" is not one of the placements this expresses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// One override, and the width class it applies at.
///
/// A rule names a class rather than holding one. The class a recipe is lowered at arrives
/// from the solve in the [`Scope`], and a gated rule is a predicate over that, so the recipe
/// itself stays class-free while a container can still change shape when its class moves.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rule {
    /// The class this applies at, or `None` for every class.
    pub at: Option<WidthClass>,
    pub over: Over,
}

impl Rule {
    /// Returns a rule that applies at every width class.
    #[must_use]
    pub const fn always(over: Over) -> Self {
        Self { at: None, over }
    }

    /// Returns a rule that applies at `class` and no other.
    ///
    /// Exact rather than a range or a set: a caller wanting two classes states two rules.
    #[must_use]
    pub const fn at(class: WidthClass, over: Over) -> Self {
        Self {
            at: Some(class),
            over,
        }
    }

    /// Returns whether this rule applies in `scope`.
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

/// Returns the window root's style: a full-extent stretching column.
///
/// The root **is** the client area. The model is told the window's size and solves against
/// it, so anything but a full-extent box either leaves a strip of window nothing lays out in
/// or overflows one. [`Ui::run`](crate::driver::Ui::run) applies this style itself; an
/// application states no root style.
///
/// Both halves are load-bearing. A row would give its children their content height, so a
/// shell would size to what it contains instead of to the window — a chain that runs off the
/// bottom edge, and a scroll viewport whose height resolves to zero before its tracker is
/// created. Stretch gives a child the full inline extent, without which a scroll container's
/// viewport is zero DIPs wide and its interaction source hit-tests nothing while reporting
/// success.
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

/// Lowers a recipe to a style. The one place a `taffy::Style` is built.
///
/// A flex class allocates nothing. A grid class allocates its track templates — one `Vec`
/// per grid node per lower — because that is how `taffy::Style` holds them.
#[must_use]
pub fn lower(preset: Preset, rules: &[Rule], scope: Scope) -> taffy::Style {
    lower_with(preset, rules, &[], scope)
}

/// Lowers a recipe with `extra` applied after the recipe's own overrides.
///
/// What a style bound to a value re-lowers through: it starts from the node's **own** recipe,
/// so a width class that moved in between is already in the answer. Taking the extras as a
/// borrowed slice keeps the re-lower allocation-free.
///
/// `extra` is a slice rather than one override for two reasons: a single bound property can
/// need several — a column template is `ClearColumns` followed by a track each — and each
/// call produces the whole style, so every extra a node needs must arrive in one call or the
/// last style pushed is the only one that survives.
///
/// The extras apply at every width class: they carry this frame's values for bound
/// properties rather than class-gated design decisions.
#[must_use]
pub fn lower_with(preset: Preset, rules: &[Rule], extra: &[Over], scope: Scope) -> taffy::Style {
    let active = || rules.iter().filter(|r| r.applies(scope)).map(|r| r.over);
    // The base first, from the last active rule naming one: a preset replaces the style
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
        // A column, and the direction is what this preset is for. A single-line run is its
        // own sprite with no children, so nothing here reaches it; a wrapping run owns a
        // sprite per line, and a flex row would lay those lines out side by side. The lines
        // carry a definite size of their own, so stretch never touches them and the group's
        // content height is their sum.
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
        Over::PaddingXY(x, y) => {
            let (h, v) = (x.length_percentage(scope), y.length_percentage(scope));
            style.padding = taffy::Rect {
                left: h,
                right: h,
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
        Over::Edge(edge) => {
            style.position = taffy::Position::Absolute;
            let (zero, auto) = (
                taffy::LengthPercentageAuto::ZERO,
                taffy::LengthPercentageAuto::AUTO,
            );
            // The pinned edge is zero and its opposite is auto, which is what makes the
            // node's own size the extent on that axis; both are zero on the other axis, so
            // it stretches.
            style.inset = match edge {
                Edge::Left => taffy::Rect {
                    left: zero,
                    right: auto,
                    top: zero,
                    bottom: zero,
                },
                Edge::Right => taffy::Rect {
                    left: auto,
                    right: zero,
                    top: zero,
                    bottom: zero,
                },
                Edge::Top => taffy::Rect {
                    left: zero,
                    right: zero,
                    top: zero,
                    bottom: auto,
                },
                Edge::Bottom => taffy::Rect {
                    left: zero,
                    right: zero,
                    top: auto,
                    bottom: zero,
                },
            };
            style.grid_row = auto_placement();
            style.grid_column = auto_placement();
        }
        // An out-of-flow node is not in the track model, so the placement its container
        // states for the in-flow case does not confine it.
        Over::Place { .. } if style.position == taffy::Position::Absolute => {}
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

/// Returns the placement that leaves a node to flow rather than seating it on a named line.
fn auto_placement<S: taffy::CheapCloneStr>() -> taffy::Line<taffy::GridPlacement<S>> {
    taffy::Line {
        start: taffy::GridPlacement::Auto,
        end: taffy::GridPlacement::Auto,
    }
}

fn placement<S: taffy::CheapCloneStr>(at: u16, span: u16) -> taffy::Line<taffy::GridPlacement<S>> {
    // Taffy's grid lines are 1-based; a placement in this crate's vocabulary is 0-based.
    let start = i16::try_from(at).unwrap_or(i16::MAX).saturating_add(1);
    taffy::Line {
        start: taffy::GridPlacement::from_line_index(start),
        end: taffy::GridPlacement::Span(span.max(1)),
    }
}
