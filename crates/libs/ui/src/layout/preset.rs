//! Named UI layout declarations. Lengths resolve only when the solver supplies a scope.

use super::{Align, Len, Track};
use crate::role::{Metric, Scope};
use windows_scene::taffy;
use windows_scene::taffy::style_helpers::{TaffyGridLine, TaffyZero};

/// Constructor layout defaults, independent of the layout engine.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Preset {
    #[default]
    Bare,
    Stack,
    Row,
    Wrap,
    Grid,
    Tiles,
    Scroll,
    Text,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// One placement, so grid participation and edge pinning cannot conflict.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Position {
    Flow,
    Grid {
        row: u16,
        column: u16,
        row_span: u16,
        column_span: u16,
    },
    Absolute([Len; 4]),
    Edge(Edge),
    Band {
        at: Len,
        height: Len,
    },
}

/// Authored fields. Absence inherits constructor defaults or the base declaration.
/// An explicitly empty row or column template clears the inherited template.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub flow: Option<Preset>,
    pub width: Option<Len>,
    pub height: Option<Len>,
    pub min_width: Option<Len>,
    pub min_height: Option<Len>,
    pub max_width: Option<Len>,
    pub max_height: Option<Len>,
    pub padding: Option<[Len; 2]>,
    pub gap: Option<Len>,
    pub grow: Option<f32>,
    pub shrink: Option<f32>,
    pub align: Option<Align>,
    pub justify: Option<Align>,
    pub align_self: Option<Align>,
    pub hidden: Option<bool>,
    pub clip: Option<bool>,
    pub position: Option<Position>,
    pub tile_min: Option<Len>,
    pub rows: Option<Vec<Track>>,
    pub columns: Option<Vec<Track>>,
}

impl Layout {
    /// Reuses the declared column buffer. Callers assign its full contents each time.
    pub fn columns(&mut self) -> &mut Vec<Track> {
        self.columns.get_or_insert_default()
    }
    /// Reuses the declared row buffer. Callers assign its full contents each time.
    pub fn rows(&mut self) -> &mut Vec<Track> {
        self.rows.get_or_insert_default()
    }

    pub(crate) fn lower(
        &self,
        preset: Preset,
        variant: Option<&Self>,
        scope: Scope,
    ) -> taffy::Style {
        let preset = variant.and_then(|v| v.flow).or(self.flow).unwrap_or(preset);
        let grid = matches!(preset, Preset::Grid | Preset::Tiles);
        let row = matches!(preset, Preset::Row | Preset::Wrap);
        let stretch = matches!(preset, Preset::Stack | Preset::Scroll);
        let spaced = matches!(
            preset,
            Preset::Stack | Preset::Row | Preset::Wrap | Preset::Grid | Preset::Tiles
        );
        let gap = Len::Metric(Metric::SpaceMd).length_percentage(scope);
        let mut style = taffy::Style {
            display: if grid {
                taffy::Display::Grid
            } else {
                taffy::Display::Flex
            },
            flex_direction: if row || preset == Preset::Bare {
                taffy::FlexDirection::Row
            } else {
                taffy::FlexDirection::Column
            },
            flex_wrap: if preset == Preset::Wrap {
                taffy::FlexWrap::Wrap
            } else {
                taffy::FlexWrap::NoWrap
            },
            align_items: if stretch {
                Some(Align::Stretch.items())
            } else if row {
                Some(Align::Center.items())
            } else {
                None
            },
            gap: taffy::Size {
                width: if spaced && preset != Preset::Stack {
                    gap
                } else {
                    taffy::LengthPercentage::ZERO
                },
                height: if spaced && preset != Preset::Row {
                    gap
                } else {
                    taffy::LengthPercentage::ZERO
                },
            },
            ..taffy::Style::DEFAULT
        };
        if preset == Preset::Scroll {
            style.overflow = taffy::Point {
                x: taffy::Overflow::Hidden,
                y: taffy::Overflow::Scroll,
            };
        }
        // Pick fields before conversion: neither inherited nor overridden tracks are allocated twice.
        let selected = |f: fn(&Self) -> Option<Len>| variant.and_then(f).or_else(|| f(self));
        for (out, value) in [
            (&mut style.size.width, selected(|l| l.width)),
            (&mut style.size.height, selected(|l| l.height)),
            (&mut style.min_size.width, selected(|l| l.min_width)),
            (&mut style.min_size.height, selected(|l| l.min_height)),
            (&mut style.max_size.width, selected(|l| l.max_width)),
            (&mut style.max_size.height, selected(|l| l.max_height)),
        ] {
            if let Some(value) = value {
                *out = value.dimension(scope);
            }
        }
        let padding = variant.and_then(|v| v.padding).or(self.padding);
        if let Some([x, y]) = padding {
            let (x, y) = (x.length_percentage(scope), y.length_percentage(scope));
            style.padding = taffy::Rect {
                left: x,
                right: x,
                top: y,
                bottom: y,
            };
        }
        if let Some(gap) = selected(|l| l.gap) {
            let gap = gap.length_percentage(scope);
            style.gap = taffy::Size {
                width: gap,
                height: gap,
            };
        }
        style.flex_grow = variant
            .and_then(|v| v.grow)
            .or(self.grow)
            .unwrap_or(style.flex_grow);
        style.flex_shrink = variant
            .and_then(|v| v.shrink)
            .or(self.shrink)
            .unwrap_or(style.flex_shrink);
        if let Some(align) = variant.and_then(|v| v.align).or(self.align) {
            style.align_items = Some(align.items());
        }
        style.align_self = variant
            .and_then(|v| v.align_self)
            .or(self.align_self)
            .map(Align::items);
        style.justify_content = variant
            .and_then(|v| v.justify)
            .or(self.justify)
            .map(Align::content);
        if variant.and_then(|v| v.clip).or(self.clip) == Some(true) {
            style.overflow = taffy::Point {
                x: taffy::Overflow::Hidden,
                y: taffy::Overflow::Hidden,
            };
        }
        if variant.and_then(|v| v.hidden).or(self.hidden) == Some(true) {
            style.display = taffy::Display::None;
        }
        let rows = variant.and_then(|v| v.rows.as_ref()).or(self.rows.as_ref());
        let columns = variant
            .and_then(|v| v.columns.as_ref())
            .or(self.columns.as_ref());
        if let Some(rows) = rows {
            style.grid_template_rows.extend(
                rows.iter()
                    .map(|t| taffy::GridTemplateComponent::Single(t.sizing(scope))),
            );
        }
        if let Some(columns) = columns {
            style.grid_template_columns.extend(
                columns
                    .iter()
                    .map(|t| taffy::GridTemplateComponent::Single(t.sizing(scope))),
            );
        } else if preset == Preset::Tiles || selected(|l| l.tile_min).is_some() {
            let min = selected(|l| l.tile_min).unwrap_or(Len::Metric(Metric::CardMinW));
            style
                .grid_template_columns
                .push(taffy::style_helpers::repeat(
                    taffy::RepetitionCount::AutoFill,
                    vec![Track::MinMax(min, 1.0).sizing(scope)],
                ));
        }
        let insets = match variant.and_then(|v| v.position).or(self.position) {
            Some(Position::Grid {
                row,
                column,
                row_span,
                column_span,
            }) => {
                style.grid_row = placement(row, row_span);
                style.grid_column = placement(column, column_span);
                None
            }
            Some(Position::Absolute(insets)) => Some(insets),
            Some(Position::Edge(edge)) => {
                let mut inset = [Len::Zero; 4];
                inset[match edge {
                    Edge::Left => 1,
                    Edge::Right => 0,
                    Edge::Top => 3,
                    Edge::Bottom => 2,
                }] = Len::Auto;
                Some(inset)
            }
            Some(Position::Band { at, height }) => {
                style.size.height = height.dimension(scope);
                Some([Len::Zero, Len::Zero, at, Len::Auto])
            }
            None | Some(Position::Flow) => None,
        };
        if let Some([left, right, top, bottom]) = insets {
            style.position = taffy::Position::Absolute;
            style.inset = taffy::Rect {
                left: left.length_percentage_auto(scope),
                right: right.length_percentage_auto(scope),
                top: top.length_percentage_auto(scope),
                bottom: bottom.length_percentage_auto(scope),
            };
        }
        style
    }
}

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

/// A clipped popup viewport, sized from window input before any child is measured.
pub(crate) fn viewport_style(
    size: windows_numerics::Vector2,
    anchor: crate::overlay::Anchor,
) -> taffy::Style {
    use crate::overlay::{Align as AnchorAlign, Side};
    let along = match anchor.align {
        AnchorAlign::Start => Align::Start,
        AnchorAlign::Center => Align::Center,
        AnchorAlign::End => Align::End,
    };
    // The viewport fills the inset window; its content still follows the popup's anchor.
    let (x, y) = match anchor.side {
        Side::Center => (Align::Center, Align::Center),
        Side::Left => (Align::Start, along),
        Side::Right => (Align::End, along),
        Side::Top => (along, Align::Start),
        Side::Bottom => (along, Align::End),
    };
    taffy::Style {
        display: taffy::Display::Flex,
        flex_direction: taffy::FlexDirection::Column,
        align_items: Some(x.items()),
        justify_content: Some(y.content()),
        size: taffy::Size {
            width: taffy::Dimension::length(size.x),
            height: taffy::Dimension::length(size.y),
        },
        overflow: taffy::Point {
            x: taffy::Overflow::Hidden,
            y: taffy::Overflow::Hidden,
        },
        ..taffy::Style::DEFAULT
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
