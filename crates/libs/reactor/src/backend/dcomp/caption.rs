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

use windows_canvas_core::{Brush, DrawingSession, ParagraphAlignment, Rect, TextAlignment};

use super::node::{linear, Node};
use super::theme;

/// One caption button's width in DIPs (the Win11 caption metric).
pub(crate) const BTN_W: f32 = 46.0;
/// Total width the TitleBar reserves for the button cluster.
pub(crate) const CLUSTER_W: f32 = BTN_W * 3.0;

/// Segoe Fluent Icons glyphs.
const GLYPH_MINIMIZE: char = '\u{E921}';
const GLYPH_MAXIMIZE: char = '\u{E922}';
const GLYPH_RESTORE: char = '\u{E923}';
const GLYPH_CLOSE: char = '\u{E8BB}';

thread_local! {
    /// Hovered button index (0 = min, 1 = max, 2 = close), -1 = none.
    static HOVER: Cell<i32> = const { Cell::new(-1) };
    /// Pressed button index (armed by `WM_NCLBUTTONDOWN`), -1 = none.
    static PRESSED: Cell<i32> = const { Cell::new(-1) };
    /// Whether the window is maximized (drives the max/restore glyph).
    static MAXIMIZED: Cell<bool> = const { Cell::new(false) };
}

/// Map a non-client hit-test code to a button index (-1 = not a button).
pub(crate) fn index_for_hit(ht: u32) -> i32 {
    match ht {
        crate::system_bindings::HTMINBUTTON => 0,
        crate::system_bindings::HTMAXBUTTON => 1,
        crate::system_bindings::HTCLOSE => 2,
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

/// Paint the three caption buttons onto the TitleBar node's surface. `rect` is
/// the node's local box; the cluster occupies its trailing `CLUSTER_W`.
pub(crate) fn paint(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect) {
    let _ = node;
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
            &glyph.to_string(),
            br,
            "Segoe Fluent Icons",
            10.0,
            400,
            theme::text(),
            TextAlignment::Center,
            ParagraphAlignment::Center,
            1.0,
        );
    }
}
