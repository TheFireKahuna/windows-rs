//! The InfoBadge: a small status dot, or a pill carrying a count.
//!
//! The simplest control in the library — no state of its own beyond the
//! optional value, no interaction, and no motion, so it is a geometry table and
//! nothing else. It lives in its own module rather than inside
//! [`controls`](super::controls) for the reason [`nav`](super::nav) and
//! [`info_bar`](super::info_bar) do: its metrics are read by several consumers
//! (the birth style, the layout measure, the retained plate and the count's
//! sprites) and must agree exactly.
//!
//! It owns no surface. The plate is one retained part
//! ([`parts::badge_plan`](super::parts::badge_plan)) and the count is glyph
//! sprites above it ([`glyph_text::info_badge_sync`](super::glyph_text::info_badge_sync)),
//! so recolouring a badge re-binds a source and a new count re-places sprites —
//! neither reaches a `BeginDraw`.

use windows_canvas::Rect;

use super::node::Node;
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
        .map_or(0.0, |(w, _)| w);
    ((text_w + 2.0 * PILL_PAD_X).max(PILL_H), PILL_H)
}

/// The plate's box within a badge of size `(w, h)`.
///
/// The fourth consumer of this module's metrics, and the reason the dot's
/// `min(w, h)` lives here rather than at the call site: layout can hand a badge
/// a box of any shape, and the dot has to stay round inside whatever it gets.
///
/// Keyed on the VALUE, exactly as [`measure`] is. Keying on the cached run
/// instead — "no run, so draw the dot" — would let a numeric badge whose text
/// failed to shape draw itself as a dot in a box that was measured for a pill,
/// which is the one disagreement between these two functions that layout cannot
/// absorb.
pub(crate) fn plate_box(node: &Node, w: f32, h: f32) -> Rect {
    if node.ctrl().badge_value.is_some() {
        return Rect::from_xywh(0.0, 0.0, w, h);
    }
    let d = w.min(h);
    Rect::from_xywh((w - d) / 2.0, (h - d) / 2.0, d, d)
}
