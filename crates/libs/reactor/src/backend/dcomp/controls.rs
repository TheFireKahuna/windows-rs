//! The drawn control library: per-`ControlKind` chrome painters plus the shared
//! ink (hover/press wash), focus-ring, and small text/shape helpers every
//! control reuses. Pure rendering — interaction lives in `input.rs`, state in
//! `node::Ctrl`. All colours/metrics come from [`theme`]; nothing here is a raw
//! literal except geometric ratios and glyph codepoints.

use super::editor;
use super::node::{is_text_editable, linear, Node};
use super::theme;
use crate::backend::ControlKind;
use crate::Color;
use windows_canvas_core::{
    Brush, DrawingSession, Ellipse, FontWeight, ParagraphAlignment, Rect, RoundedRect,
    TextAlignment, TextFormat, Vector2,
};

// Fluent-icon glyph codepoints (rendered in `theme::FONT_ICON`).
const GLYPH_CHEVRON_DOWN: u32 = 0xE70D;
const GLYPH_CHEVRON_RIGHT: u32 = 0xE76C;

/// Draw a stateful/drawn control's chrome into its surface-local `rect`.
/// Returns `true` if it handled the kind (so `paint_chrome` skips the generic
/// fill/border path).
pub(crate) fn paint(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect) -> bool {
    let dim = if node.paint.is_enabled {
        1.0
    } else {
        theme::disabled_opacity()
    };
    // The editable text kinds draw their own box + caret + selection (and their
    // own focus affordance), so they bypass the shared focus-ring tail below.
    if is_text_editable(node.kind) {
        paint_editor(session, brush, node, rect, dim);
        return true;
    }
    match node.kind {
        ControlKind::Button
        | ControlKind::ToggleButton
        | ControlKind::RepeatButton
        | ControlKind::SplitButton => paint_button(session, brush, node, rect, dim),
        ControlKind::HyperlinkButton => paint_hyperlink(session, brush, node, rect, dim),
        // Track, outline, and knob are retained chrome parts (compositor
        // sprites — see `super::parts`); only the focus-ring tail below paints.
        ControlKind::ToggleSwitch => {}
        ControlKind::CheckBox => paint_check_box(session, brush, node, rect, dim),
        ControlKind::SelectorBar => paint_segmented(session, brush, node, rect, dim),
        ControlKind::ComboBox | ControlKind::DropDownButton => {
            paint_select(session, brush, node, rect, dim)
        }
        ControlKind::Slider => paint_slider(session, brush, node, rect, dim),
        // Track, fill, and indeterminate sweep are retained chrome parts
        // (`super::parts::progress_sync`) — the sweep loops on the compositor.
        ControlKind::ProgressBar => {}
        ControlKind::ProgressRing => paint_progress_ring(session, brush, node, rect, dim),
        ControlKind::NavigationView => paint_nav(session, brush, node, rect, dim),
        ControlKind::Expander => paint_expander(session, brush, node, rect, dim),
        // The custom caption band: only the min/max/close cluster is drawn
        // here (the band itself is transparent; slot children are real nodes).
        ControlKind::TitleBar => super::caption::paint(session, brush, node, rect),
        _ => return false,
    }
    if node.focused {
        // The accent segmented tray is a stadium — its focus ring follows suit.
        let radius = if node.kind == ControlKind::SelectorBar && node.paint.style_variant == 1 {
            rect.height() / 2.0
        } else {
            focus_radius(node.kind)
        };
        draw_focus_ring(session, brush, rect, radius);
    }
    true
}

// ── Shared helpers ───────────────────────────────────────────────────────────

/// Set the recolorable brush to a token colour, scaled by `dim` (disabled fade).
fn put(brush: &Brush, c: Color, dim: f32) {
    let mut l = linear(c);
    l.a *= dim;
    brush.set_color(l);
}

fn fill_rr(session: &DrawingSession, brush: &Brush, r: Rect, radius: f32, c: Color, dim: f32) {
    put(brush, c, dim);
    if radius > 0.0 {
        session.fill_rounded_rect(&RoundedRect::uniform(r, radius), brush);
    } else {
        session.fill_rect(&r, brush);
    }
}

fn stroke_rr(
    session: &DrawingSession,
    brush: &Brush,
    r: Rect,
    radius: f32,
    c: Color,
    width: f32,
    dim: f32,
) {
    put(brush, c, dim);
    let inset = Rect::new(
        r.left + width / 2.0,
        r.top + width / 2.0,
        r.right - width / 2.0,
        r.bottom - width / 2.0,
    );
    if radius > 0.0 {
        session.draw_rounded_rect(&RoundedRect::uniform(inset, radius), brush, width);
    } else {
        session.draw_rect(&inset, brush, width);
    }
}

/// One-shot text draw inside `rect` (builds + caches nothing — only runs on a
/// dirty repaint, never per frame). `align`/`valign` position within the box.
#[allow(clippy::too_many_arguments)]
pub(crate) fn text(
    session: &DrawingSession,
    brush: &Brush,
    s: &str,
    rect: Rect,
    family: &str,
    size: f32,
    weight: u16,
    color: Color,
    align: TextAlignment,
    valign: ParagraphAlignment,
    dim: f32,
) {
    if s.is_empty() {
        return;
    }
    let Ok(fmt) = TextFormat::with_weight(family, size, FontWeight(weight as i32)) else {
        return;
    };
    let fmt = fmt.with_alignment(align).with_paragraph_alignment(valign);
    put(brush, color, dim);
    session.draw_text(s, &fmt, &rect, brush);
}

fn glyph_str(cp: u32) -> Option<String> {
    char::from_u32(cp).map(|c| c.to_string())
}

/// A focus ring: a 2px `STROKE_STRONG` rounded outline inset 1px from the edge.
pub(crate) fn draw_focus_ring(session: &DrawingSession, brush: &Brush, rect: Rect, radius: f32) {
    let r = Rect::new(rect.left + 1.0, rect.top + 1.0, rect.right - 1.0, rect.bottom - 1.0);
    stroke_rr(session, brush, r, radius, theme::stroke_strong(), 2.0, 1.0);
}

fn focus_radius(kind: ControlKind) -> f32 {
    match kind {
        ControlKind::ToggleSwitch | ControlKind::Slider => theme::RADIUS_PILL,
        ControlKind::ComboBox | ControlKind::DropDownButton | ControlKind::SelectorBar => {
            theme::RADIUS_SM
        }
        _ => theme::RADIUS_MD,
    }
}

// ── Button family ────────────────────────────────────────────────────────────

fn paint_button(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let radius = node.paint.corner_radius.max(theme::RADIUS_MD);
    let accent = node.paint.style_variant == 1;
    // Subtle (2) and TextLink (3) are chromeless by definition — no resting fill
    // and no border, only a hover/press wash. This is what `pressable` wraps a
    // custom visual in (segmented-pill segments, icon buttons), so the wrapped
    // child frames itself; a stray Button outline would double-box it.
    let chromeless = matches!(node.paint.style_variant, 2 | 3);
    let checked = node.ctrl.is_checked && node.kind == ControlKind::ToggleButton;

    // Base fill.
    let base = if accent {
        theme::accent_fill()
    } else if checked {
        theme::accent_fill()
    } else if chromeless || matches!(node.kind, ControlKind::Button | ControlKind::RepeatButton) {
        // Chromeless / bare button: transparent at rest, wash appears via ink.
        theme::w(0.0)
    } else {
        theme::surface_raised()
    };
    fill_rr(session, brush, rect, radius, base, dim);

    // The hover/press white wash is a retained ink part above this surface
    // (`super::parts::ink_state_changed` fades it compositor-side).
    if !accent && !checked && !chromeless {
        stroke_rr(session, brush, rect, radius, theme::stroke(), theme::BORDER_W, dim);
    }

    // Label (centered). A TextLink reads as an inline accent hyperlink.
    let fg = node.paint.foreground.unwrap_or(
        if accent || checked || node.paint.style_variant == 3 {
            theme::accent()
        } else {
            theme::text()
        },
    );
    text(
        session,
        brush,
        &node.paint.text,
        rect,
        "Segoe UI",
        node.paint.font_size.max(theme::FONT_SIZE_MD),
        if accent { 600 } else { 400 },
        fg,
        TextAlignment::Center,
        ParagraphAlignment::Center,
        dim,
    );
}

fn paint_hyperlink(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    // Hover recolor is event-driven (one repaint per flip, no tick) — links
    // switch colour instantly, as native hyperlinks do.
    let c = if node.hovered { theme::accent_light() } else { theme::accent() };
    put(brush, c, dim);
    let Ok(fmt) = TextFormat::with_weight(
        "Segoe UI",
        node.paint.font_size.max(theme::FONT_SIZE_MD),
        FontWeight(400),
    ) else {
        return;
    };
    let fmt = fmt
        .with_alignment(TextAlignment::Leading)
        .with_paragraph_alignment(ParagraphAlignment::Center);
    session.draw_text(&node.paint.text, &fmt, &rect, brush);
}

// ── CheckBox ─────────────────────────────────────────────────────────────────

fn paint_check_box(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let box_d = 18.0_f32;
    let cy = rect.height() / 2.0;
    let bx = Rect::from_xywh(0.0, cy - box_d / 2.0, box_d, box_d);

    // The accent box fill and the checkmark are retained chrome parts
    // (`super::parts::check_sync`) — a check/uncheck fades on the compositor.
    // Only the outline (hover-brightened by an event repaint) and the label
    // paint here.
    let stroke = theme::w(if node.hovered { 0.36 } else { 0.30 });
    stroke_rr(session, brush, bx, theme::RADIUS_SM, stroke, 1.5, dim);

    // Optional trailing label.
    if !node.paint.text.is_empty() {
        let lr = Rect::new(bx.right + theme::SPACE_8, rect.top, rect.right, rect.bottom);
        text(
            session,
            brush,
            &node.paint.text,
            lr,
            "Segoe UI",
            theme::FONT_SIZE_MD,
            400,
            theme::text(),
            TextAlignment::Leading,
            ParagraphAlignment::Center,
            dim,
        );
    }
}

// ── Segmented / SelectorBar ──────────────────────────────────────────────────

/// Per-variant segmented geometry: segment label padding + tray inset. The
/// accent (mode-toggle) variant is a roomy full pill; the subtle variant is the
/// dense toolbar tray. Shared by paint, hit-testing, and intrinsic measure.
pub(crate) struct SegMetrics {
    /// Horizontal label padding inside a segment.
    pub pad_x: f32,
    /// Vertical label padding inside a segment.
    pub pad_y: f32,
    /// Inset from the tray edge to the segment pills.
    pub tray: f32,
}

/// The subtle variant keys its density off the label size: at the compact
/// (toolbar) font it tightens to the mockup's `--sm` padding.
pub(crate) fn seg_metrics(style_variant: i32, font_size: f32) -> SegMetrics {
    if style_variant == 1 {
        SegMetrics { pad_x: 14.0, pad_y: 5.0, tray: 2.0 }
    } else if font_size <= 10.0 {
        SegMetrics { pad_x: 8.0, pad_y: 3.0, tray: theme::BORDER_W }
    } else {
        SegMetrics { pad_x: 10.0, pad_y: 4.0, tray: theme::BORDER_W }
    }
}

/// Segment boundaries in node-local X (n+1 entries, first = tray inset): each
/// segment sizes to its own measured label + padding, scaled proportionally to
/// fill the tray's inner width. Falls back to equal widths until labels have
/// been measured. Shared by paint, hit-testing, and UIA item rects.
pub(crate) fn segment_edges(node: &Node) -> Vec<f32> {
    let n = node.ctrl.items.len();
    let m = seg_metrics(node.paint.style_variant, node.paint.font_size);
    let inner = (node.rect.w - 2.0 * m.tray).max(0.0);
    let mut widths: Vec<f32> = if node.ctrl.seg_label_w.len() == n {
        node.ctrl.seg_label_w.iter().map(|w| w + 2.0 * m.pad_x).collect()
    } else {
        vec![inner / n.max(1) as f32; n]
    };
    let total: f32 = widths.iter().sum();
    if total > 0.0 {
        let s = inner / total;
        for w in &mut widths {
            *w *= s;
        }
    }
    let mut edges = Vec::with_capacity(n + 1);
    let mut x = m.tray;
    edges.push(x);
    for w in widths {
        x += w;
        edges.push(x);
    }
    edges
}

fn paint_segmented(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let m = seg_metrics(node.paint.style_variant, node.paint.font_size);
    let h = rect.height();

    // The tray (fill + 1px stroke), the sliding indicator pill, and the hover
    // ink are retained chrome parts UNDER this surface (`super::parts`) — the
    // pill glides between segments on the compositor. Only the labels (and the
    // focus-ring tail) are painted here, so they stay crisp above the pill.
    let n = node.ctrl.items.len();
    if n == 0 {
        return;
    }
    let edges = segment_edges(node);
    let pill_h = h - 2.0 * m.tray;
    let seg_rect = |i: usize| {
        Rect::from_xywh(rect.left + edges[i], rect.top + m.tray, edges[i + 1] - edges[i], pill_h)
    };
    let hot = node.ctrl.hot_index;

    // Segment labels.
    for (i, label) in node.ctrl.items.iter().enumerate() {
        let active = i as i32 == node.ctrl.selected_index;
        let hovered = node.paint.is_enabled && node.hovered && i as i32 == hot;
        let color = if active {
            theme::text()
        } else if hovered {
            theme::text_secondary()
        } else {
            theme::text_tertiary()
        };
        text(
            session,
            brush,
            label,
            seg_rect(i),
            "Segoe UI",
            node.paint.font_size.max(theme::FONT_SIZE_MICRO),
            if active { 600 } else { 400 },
            color,
            TextAlignment::Center,
            ParagraphAlignment::Center,
            dim,
        );
    }
}

// ── Select / ComboBox / DropDownButton trigger ───────────────────────────────

fn paint_select(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let radius = theme::RADIUS_SM;
    fill_rr(session, brush, rect, radius, theme::surface_raised(), dim);
    stroke_rr(session, brush, rect, radius, theme::stroke(), theme::BORDER_W, dim);

    // Hover/press wash: a retained ink part above this surface.
    // Label: ComboBox shows the selected item; DropDownButton shows its Content.
    let label = if node.kind == ControlKind::ComboBox {
        node.ctrl
            .items
            .get(node.ctrl.selected_index.max(0) as usize)
            .cloned()
            .filter(|_| node.ctrl.selected_index >= 0)
            .unwrap_or_else(|| node.ctrl.placeholder.clone())
    } else {
        node.paint.text.clone()
    };
    let lr = Rect::new(rect.left + theme::SPACE_8, rect.top, rect.right - theme::SPACE_24, rect.bottom);
    let label_color = if node.ctrl.selected_index < 0 && node.kind == ControlKind::ComboBox {
        theme::text_tertiary()
    } else {
        theme::text()
    };
    text(
        session,
        brush,
        &label,
        lr,
        "Segoe UI",
        theme::FONT_SIZE_SM,
        400,
        label_color,
        TextAlignment::Leading,
        ParagraphAlignment::Center,
        dim,
    );

    // Trailing chevron.
    if let Some(g) = glyph_str(GLYPH_CHEVRON_DOWN) {
        let cr = Rect::new(rect.right - theme::SPACE_24, rect.top, rect.right - theme::SPACE_4, rect.bottom);
        text(
            session,
            brush,
            &g,
            cr,
            theme::FONT_ICON,
            8.0,
            400,
            theme::text_secondary(),
            TextAlignment::Center,
            ParagraphAlignment::Center,
            dim,
        );
    }
}

// ── Slider (native) ──────────────────────────────────────────────────────────

fn paint_slider(session: &DrawingSession, brush: &Brush, _node: &Node, rect: Rect, dim: f32) {
    let cy = rect.height() / 2.0;
    let inset = theme::SLIDER_THUMB / 2.0;
    let x0 = inset;
    let x1 = rect.width() - inset;

    // Only the static groove paints here; the accent fill, hover halo, and
    // thumb are retained chrome parts above this surface (`super::parts`) —
    // a drag is pure compositor property sets, no repaint.
    let groove = Rect::from_xywh(x0, cy - theme::SLIDER_TRACK / 2.0, x1 - x0, theme::SLIDER_TRACK);
    fill_rr(session, brush, groove, theme::SLIDER_TRACK / 2.0, theme::w(0.06), dim);
}

// ── Progress ─────────────────────────────────────────────────────────────────

fn paint_progress_ring(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let cx = rect.width() / 2.0;
    let cy = rect.height() / 2.0;
    let r = (cx.min(cy) - 3.0).max(2.0);
    let thick = (r * 0.18).clamp(2.0, 5.0);

    // Track ring.
    put(brush, theme::w(0.08), dim);
    session.draw_ellipse(&Ellipse::new(Vector2::new(cx, cy), r, r), brush, thick);

    // Indeterminate: paint the arc ONCE at a fixed angle — the revolve is a
    // forever-looping RotationAngle animation on this surface's sprite
    // (`super::parts::ring_sync`; the track circle is rotation-invariant).
    let (a0, a1) = if node.ctrl.indeterminate {
        let s = -std::f32::consts::FRAC_PI_2;
        (s, s + std::f32::consts::FRAC_PI_2 * 2.4)
    } else {
        let frac = super::ctrl_value_frac(node) as f32;
        (-std::f32::consts::FRAC_PI_2, -std::f32::consts::FRAC_PI_2 + frac * std::f32::consts::TAU)
    };
    arc(session, brush, cx, cy, r, thick, a0, a1, theme::accent(), dim);
}

/// Tessellate an arc as short chords (PathBuilder has no arc; the surface only
/// repaints on change, so a per-paint loop is fine).
#[allow(clippy::too_many_arguments)]
fn arc(
    session: &DrawingSession,
    brush: &Brush,
    cx: f32,
    cy: f32,
    r: f32,
    width: f32,
    a0: f32,
    a1: f32,
    c: Color,
    dim: f32,
) {
    put(brush, c, dim);
    let steps = (((a1 - a0).abs() / 0.20).ceil() as i32).max(1);
    let mut prev = Vector2::new(cx + r * a0.cos(), cy + r * a0.sin());
    for i in 1..=steps {
        let a = a0 + (a1 - a0) * (i as f32 / steps as f32);
        let p = Vector2::new(cx + r * a.cos(), cy + r * a.sin());
        session.draw_line(prev, p, brush, width);
        prev = p;
    }
}

// ── NavigationView (icon rail) ───────────────────────────────────────────────

/// Per-item square side in the rail (height of one nav row).
pub(crate) const NAV_ITEM_H: f32 = theme::NAV_RAIL_W;

fn paint_nav(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    // The rail background, active tile wash, and accent indicator bar are
    // retained chrome parts UNDER this surface (`super::parts`) — a selection
    // change glides them on the compositor. The divider and per-item glyphs
    // paint here, above them.
    put(brush, theme::stroke_divider(), dim);
    session.draw_line(
        Vector2::new(theme::NAV_RAIL_W, 0.0),
        Vector2::new(theme::NAV_RAIL_W, rect.height()),
        brush,
        theme::BORDER_W,
    );

    let n = node.ctrl.items.len();
    if n == 0 {
        return;
    }
    let sel = node.ctrl.selected_index;

    for (i, label) in node.ctrl.items.iter().enumerate() {
        let iy = i as f32 * NAV_ITEM_H;
        let active = i as i32 == sel;
        let color = if active { theme::accent() } else { theme::text_tertiary() };
        let cell = Rect::from_xywh(0.0, iy, theme::NAV_RAIL_W, NAV_ITEM_H);
        let glyph = node
            .ctrl
            .icons
            .get(i)
            .copied()
            .filter(|g| *g != 0)
            .and_then(glyph_str)
            .unwrap_or_else(|| label.chars().next().map(|c| c.to_string()).unwrap_or_default());
        let family = if node.ctrl.icons.get(i).copied().unwrap_or(0) != 0 {
            theme::FONT_ICON
        } else {
            "Segoe UI"
        };
        text(
            session,
            brush,
            &glyph,
            cell,
            family,
            theme::FONT_SIZE_LG,
            400,
            color,
            TextAlignment::Center,
            ParagraphAlignment::Center,
            dim,
        );
    }
}

// ── Text editor (NumberBox / TextBox / PasswordBox / AutoSuggestBox) ─────────

const GLYPH_CHEVRON_UP: u32 = 0xE70E;

fn editor_text_alignment(align: i32) -> TextAlignment {
    match align {
        1 => TextAlignment::Center,
        2 => TextAlignment::Trailing,
        _ => TextAlignment::Leading,
    }
}

fn paint_editor(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let radius = theme::RADIUS_SM;
    // Box: raised surface fill + a border that turns accent on focus.
    fill_rr(session, brush, rect, radius, theme::surface_raised(), dim);
    let border_c = if node.focused {
        theme::accent()
    } else {
        theme::stroke()
    };
    let border_w = if node.focused { 1.5 } else { theme::BORDER_W };
    stroke_rr(session, brush, rect, radius, border_c, border_w, dim);

    let Some(ed) = &node.editor else { return };
    let align = node.ctrl.content_align;
    let (pad_left, content_w) = editor::editor_content(node.kind, rect.width());
    let cx0 = rect.left + pad_left;
    let font_size = node.paint.font_size;

    // Vertical centering from the measured line height.
    let text_h = ed
        .layout
        .as_ref()
        .and_then(|l| l.measure().ok())
        .map(|(_, h)| h)
        .filter(|h| *h > 0.0)
        .unwrap_or(font_size * 1.4);
    let origin_y = rect.top + (rect.height() - text_h) / 2.0;
    // Left-aligned fields scroll; centered/right fields keep `scroll_x == 0` and
    // let DWrite position the run within `content_w`.
    let origin_x = cx0 - ed.scroll_x;

    // Confine drawing to the content column (clip overflow / spin area).
    let clip = Rect::from_xywh(cx0, rect.top, content_w, rect.height());
    session.push_clip(&clip);

    if ed.buf.is_empty() {
        // Placeholder.
        text(
            session,
            brush,
            &node.ctrl.placeholder,
            Rect::new(cx0, rect.top, cx0 + content_w, rect.bottom),
            "Segoe UI",
            font_size,
            400,
            theme::text_tertiary(),
            editor_text_alignment(align),
            ParagraphAlignment::Center,
            dim,
        );
    } else {
        // Selection highlight (accent wash) behind the text.
        if node.focused
            && ed.has_selection()
            && let Some(layout) = &ed.layout
        {
            let (a, b) = ed.sel();
            if let Ok(rects) =
                layout.hit_test_range(a as u32, (b - a) as u32, origin_x, origin_y)
            {
                put(brush, theme::with_alpha(theme::accent(), 0.32), dim);
                for (x, y, w, h) in rects {
                    session.fill_rect(&Rect::from_xywh(x, y, w, h), brush);
                }
            }
        }
        // The text run.
        if let Some(layout) = &ed.layout {
            put(brush, node.paint.foreground.unwrap_or(theme::text()), dim);
            session.draw_text_layout(Vector2::new(origin_x, origin_y), layout, brush);
        }
    }

    // Caret (blink-gated).
    if node.focused && ed.blink_on {
        let caret_x = origin_x + ed.caret_x();
        put(brush, theme::text(), dim);
        session.draw_line(
            Vector2::new(caret_x, origin_y + 1.0),
            Vector2::new(caret_x, origin_y + text_h - 1.0),
            brush,
            1.0,
        );
    }

    session.pop_clip();

    // Spin buttons (wide NumberBox only). The chevron brighten keys off the
    // hover flag — the flip repaints once, event-driven (no tick).
    if node.kind == ControlKind::NumberBox && rect.width() >= editor::SPIN_MIN_BOX_W {
        draw_spin(session, brush, rect, node.hovered, dim);
    }
}

/// Two stacked up/down chevrons on the trailing edge of a wide `NumberBox`.
fn draw_spin(session: &DrawingSession, brush: &Brush, rect: Rect, hover: bool, dim: f32) {
    let col_x = rect.right - editor::SPIN_W;
    let mid = rect.top + rect.height() / 2.0;
    // Hairline divider before the spin column.
    put(brush, theme::stroke(), dim);
    session.draw_line(
        Vector2::new(col_x, rect.top + theme::SPACE_4),
        Vector2::new(col_x, rect.bottom - theme::SPACE_4),
        brush,
        theme::BORDER_W,
    );
    let color = theme::with_alpha(theme::text_secondary(), if hover { 1.0 } else { 0.6 });
    let cr_up = Rect::new(col_x, rect.top, rect.right, mid);
    let cr_down = Rect::new(col_x, mid, rect.right, rect.bottom);
    if let Some(g) = glyph_str(GLYPH_CHEVRON_UP) {
        text(session, brush, &g, cr_up, theme::FONT_ICON, 6.0, 400, color,
            TextAlignment::Center, ParagraphAlignment::Center, dim);
    }
    if let Some(g) = glyph_str(GLYPH_CHEVRON_DOWN) {
        text(session, brush, &g, cr_down, theme::FONT_ICON, 6.0, 400, color,
            TextAlignment::Center, ParagraphAlignment::Center, dim);
    }
}

// ── Expander ─────────────────────────────────────────────────────────────────

fn paint_expander(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let header = Rect::from_xywh(0.0, 0.0, rect.width(), theme::ROW_H + theme::SPACE_8);
    fill_rr(session, brush, header, theme::RADIUS_MD, theme::surface_raised(), dim);
    // The hover/press wash is a retained ink part over the header strip
    // (`super::parts::expander_sync`) — a compositor fade, no tick.
    stroke_rr(session, brush, header, theme::RADIUS_MD, theme::stroke(), theme::BORDER_W, dim);

    let lr = Rect::new(theme::SPACE_12, header.top, header.right - theme::SPACE_32, header.bottom);
    text(
        session,
        brush,
        &node.paint.text,
        lr,
        "Segoe UI",
        theme::FONT_SIZE_MD,
        600,
        theme::text(),
        TextAlignment::Leading,
        ParagraphAlignment::Center,
        dim,
    );

    // Chevron: right (collapsed) → down (expanded); the toggle repaints once.
    let g = if node.ctrl.expanded { GLYPH_CHEVRON_DOWN } else { GLYPH_CHEVRON_RIGHT };
    if let Some(gs) = glyph_str(g) {
        let cr = Rect::new(header.right - theme::SPACE_32, header.top, header.right - theme::SPACE_8, header.bottom);
        text(
            session,
            brush,
            &gs,
            cr,
            theme::FONT_ICON,
            10.0,
            400,
            theme::text_secondary(),
            TextAlignment::Center,
            ParagraphAlignment::Center,
            dim,
        );
    }
}
