use super::*;

/// W5 — `Microsoft.UI.Xaml.Controls.Canvas`. Free-positioning panel
/// where each child is placed via the [`CanvasPosition`] attached
/// property (`canvas_left` / `canvas_top` on [`Element`]).
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Canvas {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub children: Vec<Element>,
}
impl Canvas {
    pub fn new<I>(children: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Element>,
    {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }
}
/// Attached property for children of [`Canvas`]. Set via
/// [`Element::canvas_left`] / [`Element::canvas_top`] / [`Element::canvas_z_index`].
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct CanvasPosition {
    pub left: CanvasOffset,
    pub top: CanvasOffset,
    /// Trailing insets. Set one and the child's size on that axis is SOLVED from
    /// the pair (`extent - leading - trailing`) instead of stated — which is the
    /// difference between a child that costs a repaint when its canvas resizes
    /// and one that costs a style push, a full solve and a full rounding walk.
    /// Unset by default: a child that states its own size wants no such solve.
    pub right: Option<CanvasOffset>,
    pub bottom: Option<CanvasOffset>,
    pub z_index: i32,
}

/// One axis of a [`CanvasPosition`].
///
/// [`Fraction`](CanvasOffset::Fraction) is what lets a child hold a position
/// defined *relative to the canvas* — a grid caption a fifth of the way along a
/// log frequency axis, say — without the app first measuring the canvas to turn
/// that fraction into pixels. Combined with a margin (which layout adds to the
/// resolved inset) it also expresses "a fixed distance in from the far edge":
/// `Fraction(1.0)` with a negative margin.
///
/// Measuring instead is not merely more code: the measurement arrives from a
/// completed layout pass, so consuming it as a layout input makes layout its own
/// input and costs a second pass every time the canvas resizes.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CanvasOffset {
    /// DIPs from the canvas's left/top edge.
    Dip(f64),
    /// A share of the canvas's own width/height, `0.0..=1.0`.
    Fraction(f64),
}

impl Default for CanvasOffset {
    fn default() -> Self {
        Self::Dip(0.0)
    }
}

impl Widget for Canvas {
    widget_header!(ControlKind::Canvas);
    fn bindings(&self) -> PropBindings {
        generated::canvas_bindings(self)
    }
    fn children(&self) -> Children<'_> {
        Children::Keyed(&self.children)
    }
}
