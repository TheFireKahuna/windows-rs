//! Drawn caption buttons (minimize / maximize-restore / close) + the shared
//! non-client state for the extended-frame window.
//!
//! The dcomp host removes the native caption in `WM_NCCALCSIZE`, so the OS
//! draws no title bar at all: the reactor `TitleBar` control's band is the
//! caption. The three window buttons are painted onto the TitleBar node's own
//! surface at its trailing edge (the node's default style reserves their width
//! as right padding), and hit-tested by the host's `WM_NCHITTEST` as
//! `HTMINBUTTON` / `HTMAXBUTTON` / `HTCLOSE` — which is also what makes Win11
//! snap layouts appear on max-button hover. Hover/pressed state arrives from
//! `WM_NCMOUSEMOVE`/`WM_NCLBUTTON*` (non-client messages), so it lives here as
//! UI-thread state shared between the wndproc and the paint, not in the input
//! pipeline.

use std::cell::Cell;

use windows_canvas_core::{
    Brush, DrawingSession, ParagraphAlignment, Rect, TextAlignment, TextFormat, TextLayout,
    Trimming, Vector2,
};

use super::node::{linear, Extras, Node};
use super::theme;

/// One caption button's width in DIPs (the Win11 caption metric).
pub(crate) const BTN_W: f32 = 46.0;
/// Total width the TitleBar reserves for the button cluster.
pub(crate) const CLUSTER_W: f32 = BTN_W * 3.0;

/// The drawn back button's width in DIPs. Square in a standard band, and the
/// same width in a tall one (it stays a 40-DIP target, vertically centred,
/// rather than growing into a stretched slab).
pub(crate) const BACK_W: f32 = 40.0;

/// Synthetic index of the drawn back button, continuing the window-button
/// numbering (0 = min, 1 = max, 2 = close). Shares [`HOVER`]/[`PRESSED`] with
/// them so one hover edge repaints the whole band, and lines up with the UIA
/// `CAPTION_ITEM_BASE + i` item space so a screen reader sees a fourth element.
pub(crate) const BACK_INDEX: i32 = 3;

/// Standard and tall caption band heights (DIPs). Tall is the Win11 double
/// band an app opts into with `.tall(true)`.
pub(crate) const BAND_H: f32 = theme::ROW_H + theme::SPACE_16;
pub(crate) const BAND_H_TALL: f32 = BAND_H + theme::SPACE_16;

/// Segoe Fluent Icons glyphs. Held as `&str`, not `char`, because the paint
/// path hands them straight to `draw_text` — a `char` would have to be
/// `to_string()`d into a fresh `String` on every caption repaint.
const GLYPH_MINIMIZE: &str = "\u{E921}";
const GLYPH_MAXIMIZE: &str = "\u{E922}";
const GLYPH_RESTORE: &str = "\u{E923}";
const GLYPH_CLOSE: &str = "\u{E8BB}";
const GLYPH_BACK: &str = "\u{E72B}";

thread_local! {
    /// Hovered button index (0 = min, 1 = max, 2 = close, 3 = back), -1 = none.
    static HOVER: Cell<i32> = const { Cell::new(-1) };
    /// Pressed button index (armed by `WM_NCLBUTTONDOWN`), -1 = none.
    static PRESSED: Cell<i32> = const { Cell::new(-1) };
    /// Whether the window is maximized (drives the max/restore glyph).
    static MAXIMIZED: Cell<bool> = const { Cell::new(false) };
}

/// `HTSYSMENU`, the leading-edge non-client hit-test code. Defined here rather
/// than imported because `system_bindings` is generated from `system.txt` and
/// does not carry it; the value is fixed Win32 ABI (`WinUser.h`).
pub(crate) const HTSYSMENU: u32 = 3;

/// `WM_NCLBUTTONDBLCLK`. Same story as [`HTSYSMENU`]: absent from the generated
/// bindings, fixed Win32 ABI. The host swallows it over the back button.
pub(crate) const WM_NCLBUTTONDBLCLK: u32 = 0x00A3;

/// Map a non-client hit-test code to a button index (-1 = not a button).
///
/// The back button rides `HTSYSMENU` — the leading-edge non-client code, whose
/// screen position is exactly where it is drawn. That reuses the whole caption
/// pipeline (`WM_NCMOUSEMOVE` hover, `WM_NCLBUTTON*` press/release) for free,
/// at the cost of one hazard the host must neutralise: `DefWindowProc` treats
/// a double-click on `HTSYSMENU` as *close the window*, so the host swallows
/// `WM_NCLBUTTONDBLCLK` there. See `host.rs`.
pub(crate) fn index_for_hit(ht: u32) -> i32 {
    match ht {
        crate::system_bindings::HTMINBUTTON => 0,
        crate::system_bindings::HTMAXBUTTON => 1,
        crate::system_bindings::HTCLOSE => 2,
        HTSYSMENU => BACK_INDEX,
        _ => -1,
    }
}

/// Record the hovered button; returns `true` when it changed (repaint needed).
pub(crate) fn set_hover(i: i32) -> bool {
    HOVER.with(|c| {
        let changed = c.get() != i;
        c.set(i);
        changed
    })
}

pub(crate) fn hover() -> i32 {
    HOVER.with(Cell::get)
}

pub(crate) fn set_pressed(i: i32) {
    PRESSED.with(|c| c.set(i));
}

pub(crate) fn pressed() -> i32 {
    PRESSED.with(Cell::get)
}

/// Record the maximized state; returns `true` when it changed.
pub(crate) fn set_maximized(m: bool) -> bool {
    MAXIMIZED.with(|c| {
        let changed = c.get() != m;
        c.set(m);
        changed
    })
}

pub(crate) fn maximized() -> bool {
    MAXIMIZED.with(Cell::get)
}

/// The caption band's own drawn text, laid out once and reused.
///
/// Built by the layout pass ([`build_text`]) whenever the TitleBar's
/// `text_dirty` is set — i.e. exactly when `Title`/`Subtitle` changed — and
/// read by [`paint`], which only runs on a dirty repaint. Nothing here is
/// constructed per frame: a repaint reuses these two `IDWriteTextLayout`s and
/// at most re-points their max width, which is a property set, not an
/// allocation. This mirrors [`Node::text_layout`](super::node::Node) for
/// TextBlock/Button, but needs its own home because a caption carries *two*
/// independent runs.
pub(crate) struct CaptionText {
    pub title: Option<TextLayout>,
    pub subtitle: Option<TextLayout>,
    /// Natural (untrimmed) widths, measured at build time — the leading inset
    /// the band reserves is derived from these, so layout never has to re-ask
    /// DirectWrite mid-pass.
    pub title_w: f32,
    pub subtitle_w: f32,
    /// Line height of the title run, for vertical centring.
    pub line_h: f32,
}

/// Gap between the title and the subtitle that follows it.
const TITLE_GAP: f32 = theme::SPACE_8;

fn caption_format(weight: u16) -> Option<TextFormat> {
    TextFormat::with_weight(
        "Segoe UI",
        theme::FONT_SIZE_SM,
        windows_canvas_core::FontWeight(weight as i32),
    )
    .ok()
}

/// Lay out one caption run, ellipsized. Built at its natural width; [`paint`]
/// narrows it to the space actually available, at which point the trimming
/// sign takes over.
fn run(text: &str, weight: u16) -> Option<(TextLayout, f32, f32)> {
    if text.is_empty() {
        return None;
    }
    let fmt = caption_format(weight)?;
    let layout = TextLayout::new(text, &fmt, 100_000.0, 100_000.0).ok()?;
    let _ = layout.set_word_wrap(false);
    // Character granularity, not word: a caption is one short run and a word
    // ellipsis on a two-word title drops the whole second word at the first
    // pixel of overflow.
    let _ = layout.set_trimming(Trimming::CharacterEllipsis, &fmt);
    let (w, h) = layout.measure().ok()?;
    Some((layout, w, h))
}

/// (Re)build the caption's cached text. `None` when the band carries neither a
/// title nor a subtitle, so a TitleBar used purely as a drag strip allocates
/// nothing at all.
pub(crate) fn build_text(x: &Extras) -> Option<Box<CaptionText>> {
    let title = run(&x.title, 600);
    let subtitle = run(&x.subtitle, 400);
    if title.is_none() && subtitle.is_none() {
        return None;
    }
    let line_h = title
        .as_ref()
        .map(|t| t.2)
        .or(subtitle.as_ref().map(|s| s.2))
        .unwrap_or(0.0);
    Some(Box::new(CaptionText {
        title_w: title.as_ref().map(|t| t.1).unwrap_or(0.0),
        subtitle_w: subtitle.as_ref().map(|s| s.1).unwrap_or(0.0),
        line_h,
        title: title.map(|t| t.0),
        subtitle: subtitle.map(|s| s.0),
    }))
}

/// The band height this TitleBar's `tall` state asks for.
pub(crate) fn band_height(x: &Extras) -> f32 {
    if x.tall {
        BAND_H_TALL
    } else {
        BAND_H
    }
}

/// Width the drawn back button occupies at the leading edge (0 when absent).
///
/// `back_button_visible` alone decides whether space is reserved: a *disabled*
/// back button is still drawn (greyed), exactly like WinUI's, so hiding it on
/// disable would make the whole band reflow every time navigation depth hits
/// zero.
pub(crate) fn back_width(x: &Extras) -> f32 {
    if x.back_button_visible {
        BACK_W
    } else {
        0.0
    }
}

/// The band's left padding for a given caption state: the token side padding
/// plus whatever the drawn back button occupies.
///
/// This is the **single definition** of that geometry — `birth_style` builds a
/// virgin TitleBar with `pad_left(&Extras::DEFAULT)` and the layout pass
/// re-derives with the node's live `Extras`. Two expressions of it would drift
/// the moment one changed, and the drift is not cosmetic: an `Unset` that
/// re-derived a *different* padding than the node was born with would fail the
/// reset invariant (a reset node must be indistinguishable from one that never
/// received the prop).
pub(crate) fn pad_left(x: &Extras) -> f32 {
    theme::SPACE_16 + back_width(x)
}

/// Extra leading space the drawn title/subtitle block needs, on top of
/// [`pad_left`]. Zero when the band carries no titles, which is what keeps a
/// virgin TitleBar's derived padding equal to its birth padding.
///
/// Clamped to half the band so a very long title cannot push the app's own
/// content off the strip — past the clamp the title ellipsizes instead.
pub(crate) fn title_block(text: Option<&CaptionText>, band_w: f32) -> f32 {
    let Some(t) = text else { return 0.0 };
    let mut block = t.title_w;
    if t.subtitle_w > 0.0 {
        block += t.subtitle_w + if t.title_w > 0.0 { TITLE_GAP } else { 0.0 };
    }
    if block <= 0.0 {
        return 0.0;
    }
    block += theme::SPACE_12;
    if band_w > 0.0 {
        block = block.min(band_w * 0.5);
    }
    block
}

/// The drawn back button's box within the band, or `None` when it is hidden.
pub(crate) fn back_rect(x: &Extras, rect: Rect) -> Option<Rect> {
    if !x.back_button_visible {
        return None;
    }
    // Vertically centred rather than band-filling: in a tall band a full-height
    // arrow reads as a column, not a button.
    let h = BACK_W.min(rect.height());
    let top = rect.top + (rect.height() - h) / 2.0;
    Some(Rect::from_xywh(rect.left, top, BACK_W, h))
}

/// Paint the caption band's own chrome onto the TitleBar node's surface:
/// the leading back button, the title/subtitle, and the trailing window-button
/// cluster. `rect` is the node's local box.
pub(crate) fn paint(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect) {
    paint_back(session, brush, node, rect);
    paint_titles(session, brush, node, rect);
    let hover = hover();
    let maximized = maximized();
    for i in 0..3 {
        let bx = rect.right - (3 - i) as f32 * BTN_W;
        let br = Rect::from_xywh(bx, rect.top, BTN_W, rect.height());
        if hover == i {
            // Close hovers the system alarm red; the others a subtle wash.
            let fill = if i == 2 {
                // #C42B1C, decoded to linear by the shared Color helper.
                linear(crate::Color::rgb(0xC4, 0x2B, 0x1C))
            } else {
                linear(theme::w(0.06))
            };
            brush.set_color(fill);
            session.fill_rect(&br, brush);
        }
        let glyph = match i {
            0 => GLYPH_MINIMIZE,
            1 if maximized => GLYPH_RESTORE,
            1 => GLYPH_MAXIMIZE,
            _ => GLYPH_CLOSE,
        };
        super::controls::text(
            session,
            brush,
            glyph,
            br,
            theme::FONT_ICON,
            10.0,
            400,
            theme::text(),
            TextAlignment::Center,
            ParagraphAlignment::Center,
            1.0,
        );
    }
}

/// The leading back button: a hover/press wash plus the chevron glyph, drawn
/// with the same mechanism as its three siblings in this band (a fill on the
/// node's own surface, repainted on the hover edge). It is deliberately NOT a
/// retained `parts` sprite — the wash is a flat state fill with no animation,
/// and splitting one band's four buttons across two paint mechanisms would
/// make them drift apart.
fn paint_back(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect) {
    let x = node.extras();
    let Some(br) = back_rect(x, rect) else { return };
    let enabled = x.back_button_enabled;
    if enabled && hover() == BACK_INDEX {
        let wash = if pressed() == BACK_INDEX { 0.10 } else { 0.06 };
        brush.set_color(linear(theme::w(wash)));
        session.fill_rounded_rect(
            &windows_canvas_core::RoundedRect::uniform(br, theme::RADIUS_SM),
            brush,
        );
    }
    super::controls::text(
        session,
        brush,
        GLYPH_BACK,
        br,
        theme::FONT_ICON,
        12.0,
        400,
        if enabled {
            theme::text()
        } else {
            theme::text_disabled()
        },
        TextAlignment::Center,
        ParagraphAlignment::Center,
        1.0,
    );
}

/// Title and subtitle, left-aligned after any back button and vertically
/// centred in the band. Both runs are cached layouts; the only per-repaint work
/// is narrowing them to the width actually left over, which is what makes them
/// ellipsize as the window shrinks.
fn paint_titles(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect) {
    let Some(t) = node.caption_text.as_deref() else {
        return;
    };
    let x0 = rect.left + back_width(node.extras());
    // Everything up to the window-button cluster is available; the block was
    // already clamped when the inset was reserved, so this only ever *grows*
    // the room a title has (e.g. when no Content child was mounted).
    let avail = (rect.right - CLUSTER_W - theme::SPACE_12 - x0).max(0.0);
    if avail <= 0.0 {
        return;
    }
    let y = rect.top + (rect.height() - t.line_h) / 2.0;
    let mut pen = x0;
    if let Some(title) = &t.title {
        let w = t.title_w.min(avail);
        let _ = title.set_max_width(w);
        brush.set_color(linear(theme::text()));
        session.draw_text_layout(Vector2 { x: pen, y }, title, brush);
        pen += w + TITLE_GAP;
    }
    if let Some(sub) = &t.subtitle {
        let left = (x0 + avail - pen).max(0.0);
        if left > 0.0 {
            let _ = sub.set_max_width(t.subtitle_w.min(left));
            brush.set_color(linear(theme::text_secondary()));
            session.draw_text_layout(Vector2 { x: pen, y }, sub, brush);
        }
    }
}
