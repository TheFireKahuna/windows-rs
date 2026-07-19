//! The InfoBadge: a small status dot, or a pill carrying a count.
//!
//! The simplest drawn control in the library — no state of its own beyond the
//! optional value, no interaction, and no motion, so it is a geometry table and
//! a paint. It lives in its own module rather than inside
//! [`controls`](super::controls) for the reason [`nav`](super::nav) and
//! [`info_bar`](super::info_bar) do: its metrics are read by three consumers
//! (the birth style, the layout measure, and the paint) and must agree exactly.

use windows_canvas_core::{Brush, DrawingSession, Ellipse, Rect, Vector2};

use super::node::{linear, Node};
use super::theme;

/// The dot form's diameter — Fluent's `InfoBadgeDotStyle` size.
pub(crate) const DOT_D: f32 = 4.0;
/// The numeric pill's height (Fluent's value style).
pub(crate) const PILL_H: f32 = 16.0;
/// Horizontal padding either side of the count inside the pill. Shared with the
/// in-button badge (`controls::badge_size`), which is this same control hosted
/// in a button's box rather than beside it.
pub(crate) const PILL_PAD_X: f32 = theme::SPACE_4;

/// The count's type size and weight — the values a badge is BORN with.
///
/// Consumed by [`birth_paint`](super::node::birth_paint) rather than read at
/// paint time, which is what makes them overridable: the layout pass builds the
/// cached run from `node.paint`, so `.font_size(..)` / `.bold()` reach the badge
/// like they reach any other text-bearing control, and an untouched badge still
/// measures and draws at exactly these. Same single-definition discipline as
/// `caption::band_height(&Extras::DEFAULT)` backing a TitleBar's birth style.
pub(crate) const FONT_SIZE: f32 = theme::FONT_SIZE_MICRO;
pub(crate) const FONT_WEIGHT: u16 = 600;

/// The string a numeric badge draws, or `None` for the dot form.
///
/// Deliberately **uncapped**: WinUI shows the value it is given, and a silent
/// "99+" would be this library inventing a number the app never asked for. A
/// host that wants a cap formats one and passes the value it means.
pub(crate) fn label(node: &Node) -> Option<String> {
    node.ctrl().badge_value.map(|v| v.to_string())
}

/// The badge's intrinsic `(width, height)`.
///
/// The dot is a fixed square. The pill takes its height from [`PILL_H`] and its
/// width from the measured count plus padding, floored at a circle so a
/// single-digit badge reads as round rather than as a squashed stadium.
pub(crate) fn measure(node: &Node) -> (f32, f32) {
    if node.ctrl().badge_value.is_none() {
        return (DOT_D, DOT_D);
    }
    let text_w = node
        .text_layout
        .as_ref()
        .and_then(|l| l.measure().ok())
        .map(|(w, _)| w)
        .unwrap_or(0.0);
    ((text_w + 2.0 * PILL_PAD_X).max(PILL_H), PILL_H)
}

/// Paint the badge into its own surface.
///
/// The fill is the accent role unless the app set an explicit `Background`,
/// which is what lets a host colour a badge by meaning (an error count in the
/// danger role) without this control modelling severity the way the InfoBar
/// does — a badge carries a number, not a status.
pub(crate) fn paint(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let fill = node.paint.background.unwrap_or_else(theme::accent);
    let (w, h) = (rect.width(), rect.height());

    // The dot form: a circle centred in whatever box layout gave it, and sized
    // to that box — so a host that asks for a larger badge gets one.
    let Some(layout) = node.text_layout.as_ref().filter(|_| node.ctrl().badge_value.is_some())
    else {
        let d = w.min(h);
        let c = Vector2::new(rect.left + w / 2.0, rect.top + h / 2.0);
        super::controls::put(brush, fill, dim);
        session.fill_ellipse(&Ellipse::new(c, d / 2.0, d / 2.0), brush);
        return;
    };

    // The numeric form: a stadium (radius = half the height, so it stays round
    // at any width) carrying the count.
    super::controls::fill_rr(session, brush, rect, h / 2.0, fill, dim);

    let Ok((text_w, text_h)) = layout.measure() else {
        return;
    };
    // The count sits ON the fill, so its default is the on-accent ink rather
    // than the body-text token — which is near-invisible against a light-theme
    // accent.
    //
    // An explicit `Foreground` wins, and has to: the ink that reads on a badge
    // is a property of the FILL, and the fill is app-supplied (`Background`).
    // A host colouring badges by meaning — a per-band accent, a danger count —
    // picks the fill and therefore owns the contrast decision; deriving the ink
    // from the theme's accent would be answering a question about a colour the
    // theme never saw.
    let mut c = linear(node.paint.foreground.unwrap_or_else(theme::text_on_accent));
    c.a *= dim;
    brush.set_color(c);
    // Centred by hand from the measured run: `draw_text_layout` places by
    // origin, and the cached layout is built unaligned in a max-content box.
    session.draw_text_layout(
        Vector2 {
            x: rect.left + (w - text_w) / 2.0,
            y: rect.top + (h - text_h) / 2.0,
        },
        layout,
        brush,
    );
}
