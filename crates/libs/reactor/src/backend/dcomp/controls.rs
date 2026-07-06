//! The drawn control library: per-`ControlKind` chrome painters plus the shared
//! ink (hover/press wash), focus-ring, and small text/shape helpers every
//! control reuses. Pure rendering — interaction lives in `input.rs`, state in
//! `node::Ctrl`. All colours/metrics come from [`theme`]; nothing here is a raw
//! literal except geometric ratios and glyph codepoints.

use super::editor;
use super::node::{is_text_editable, linear, lerp_color, Node};
use super::theme;
use crate::backend::ControlKind;
use crate::Color;
use windows_canvas_core::{
    Brush, ColorF, DrawingSession, Ellipse, FontWeight, ParagraphAlignment, Rect, RoundedRect,
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
        ControlKind::ToggleSwitch => paint_toggle_switch(session, brush, node, rect, dim),
        ControlKind::CheckBox => paint_check_box(session, brush, node, rect, dim),
        ControlKind::SelectorBar => paint_segmented(session, brush, node, rect, dim),
        ControlKind::ComboBox | ControlKind::DropDownButton => {
            paint_select(session, brush, node, rect, dim)
        }
        ControlKind::Slider => paint_slider(session, brush, node, rect, dim),
        ControlKind::ProgressBar => paint_progress_bar(session, brush, node, rect, dim),
        ControlKind::ProgressRing => paint_progress_ring(session, brush, node, rect, dim),
        ControlKind::NavigationView => paint_nav(session, brush, node, rect, dim),
        ControlKind::Expander => paint_expander(session, brush, node, rect, dim),
        // The custom caption band: only the min/max/close cluster is drawn
        // here (the band itself is transparent; slot children are real nodes).
        ControlKind::TitleBar => super::caption::paint(session, brush, node, rect),
        _ => return false,
    }
    if node.focused {
        draw_focus_ring(session, brush, rect, focus_radius(node.kind));
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

fn put_f(brush: &Brush, mut c: ColorF, dim: f32) {
    c.a *= dim;
    brush.set_color(c);
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

fn circle(session: &DrawingSession, brush: &Brush, cx: f32, cy: f32, r: f32, c: Color, dim: f32) {
    put(brush, c, dim);
    session.fill_ellipse(&Ellipse::new(Vector2::new(cx, cy), r, r), brush);
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

/// The hover/press ink overlay alpha (white wash) from the node's springs.
fn ink_wash(node: &Node) -> f32 {
    // hover → w(0.06), pressed adds toward w(0.10).
    0.06 * node.hover.x.clamp(0.0, 1.0) + 0.04 * node.press.x.clamp(0.0, 1.0)
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

    // Hover/press white wash.
    let wash = ink_wash(node);
    if wash > 0.0 {
        fill_rr(session, brush, rect, radius, theme::w(wash), dim);
    }
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
    let c = lerp_color(linear(theme::accent()), linear(theme::accent_light()), node.hover.x);
    put_f(brush, c, dim);
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

// ── ToggleSwitch ─────────────────────────────────────────────────────────────

fn paint_toggle_switch(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let t = node.anim.x.clamp(0.0, 1.0);
    let track_w = 40.0_f32;
    let track_h = 20.0_f32;
    let cy = rect.height() / 2.0;
    let tx = 0.0_f32;
    let track = Rect::from_xywh(tx, cy - track_h / 2.0, track_w, track_h);
    let radius = track_h / 2.0;

    // Track: neutral outline (off) cross-fading to accent fill (on).
    let off = ColorF::new(0.0, 0.0, 0.0, 0.0);
    let on = linear(theme::accent());
    put_f(brush, lerp_color(off, on, t), dim);
    session.fill_rounded_rect(&RoundedRect::uniform(track, radius), brush);
    if t < 0.999 {
        // Off outline brightening slightly on hover.
        let a = 0.20 + 0.08 * node.hover.x;
        stroke_rr(session, brush, track, radius, theme::w(a * (1.0 - t)), 1.5, dim);
    }

    // Knob: white circle sliding L→R.
    let knob_r = 6.0_f32;
    let left = tx + 2.0 + knob_r;
    let right = tx + track_w - 2.0 - knob_r;
    let kx = left + (right - left) * t;
    circle(session, brush, kx, cy, knob_r, theme::w(1.0), dim);
}

// ── CheckBox ─────────────────────────────────────────────────────────────────

fn paint_check_box(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let box_d = 18.0_f32;
    let cy = rect.height() / 2.0;
    let bx = Rect::from_xywh(0.0, cy - box_d / 2.0, box_d, box_d);
    let on = node.anim.x.clamp(0.0, 1.0);

    // Box fill cross-fades transparent → accent.
    put_f(brush, lerp_color(ColorF::new(0.0, 0.0, 0.0, 0.0), linear(theme::accent()), on), dim);
    session.fill_rounded_rect(&RoundedRect::uniform(bx, theme::RADIUS_SM), brush);
    stroke_rr(
        session,
        brush,
        bx,
        theme::RADIUS_SM,
        theme::w(0.30 + 0.06 * node.hover.x),
        1.5,
        dim,
    );

    // Checkmark (two strokes) revealed with the fill.
    if on > 0.05 {
        put(brush, theme::w(on), dim);
        let lx = bx.left;
        let ty = bx.top;
        session.draw_line(
            Vector2::new(lx + 4.0, ty + 9.0),
            Vector2::new(lx + 7.5, ty + 12.5),
            brush,
            2.0,
        );
        session.draw_line(
            Vector2::new(lx + 7.5, ty + 12.5),
            Vector2::new(lx + 14.0, ty + 5.5),
            brush,
            2.0,
        );
    }

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

/// Equal segment width inside the tray (1px tray padding).
pub(crate) fn segment_width(node: &Node) -> f32 {
    let n = node.ctrl.items.len().max(1) as f32;
    (node.rect.w - 2.0 * theme::BORDER_W) / n
}

fn paint_segmented(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let accent = node.paint.style_variant == 1;
    let tray_radius = if accent { theme::RADIUS_LG } else { theme::RADIUS_SM };
    let seg_radius = if accent { theme::RADIUS_LG - theme::SPACE_4 / 2.0 } else { theme::RADIUS_BADGE };
    let tray_bg = if accent { theme::stroke_divider() } else { theme::stroke_subtle() };

    fill_rr(session, brush, rect, tray_radius, tray_bg, dim);
    stroke_rr(session, brush, rect, tray_radius, theme::stroke(), theme::BORDER_W, dim);

    let n = node.ctrl.items.len();
    if n == 0 {
        return;
    }
    let seg_w = segment_width(node);
    let pad = theme::BORDER_W;

    // Active indicator pill, slid via the spring (fractional index).
    let idx = node.anim.x.clamp(0.0, (n.saturating_sub(1)) as f32);
    let px = pad + idx * seg_w;
    let pill = Rect::from_xywh(px + 1.0, rect.top + 1.0, seg_w - 2.0, rect.height() - 2.0);
    let fill = if accent { theme::accent() } else { theme::stroke() };
    fill_rr(session, brush, pill, seg_radius, fill, dim);

    // Segment labels.
    for (i, label) in node.ctrl.items.iter().enumerate() {
        let sx = pad + i as f32 * seg_w;
        let sr = Rect::from_xywh(sx, rect.top, seg_w, rect.height());
        let active = i as i32 == node.ctrl.selected_index;
        let color = if active { theme::text() } else { theme::text_tertiary() };
        text(
            session,
            brush,
            label,
            sr,
            "Segoe UI",
            theme::FONT_SIZE_SM,
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
    let wash = ink_wash(node);
    if wash > 0.0 {
        fill_rr(session, brush, rect, radius, theme::w(wash), dim);
    }
    stroke_rr(session, brush, rect, radius, theme::stroke(), theme::BORDER_W, dim);

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

fn paint_slider(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let cy = rect.height() / 2.0;
    let inset = theme::SLIDER_THUMB / 2.0;
    let x0 = inset;
    let x1 = rect.width() - inset;
    let frac = node.anim.x.clamp(0.0, 1.0);
    let thumb_x = x0 + (x1 - x0) * frac;

    // Groove (neutral) then accent fill up to the thumb.
    let groove = Rect::from_xywh(x0, cy - theme::SLIDER_TRACK / 2.0, x1 - x0, theme::SLIDER_TRACK);
    fill_rr(session, brush, groove, theme::SLIDER_TRACK / 2.0, theme::w(0.06), dim);
    let fill = Rect::from_xywh(x0, cy - theme::SLIDER_TRACK / 2.0, thumb_x - x0, theme::SLIDER_TRACK);
    fill_rr(session, brush, fill, theme::SLIDER_TRACK / 2.0, theme::accent(), dim);

    // Thumb (white) with a faint hover halo.
    if node.hover.x > 0.01 || node.pressed {
        circle(session, brush, thumb_x, cy, theme::SLIDER_THUMB / 2.0 + 3.0, theme::w(0.10), dim);
    }
    circle(session, brush, thumb_x, cy, theme::SLIDER_THUMB / 2.0, theme::w(1.0), dim);
}

// ── Progress ─────────────────────────────────────────────────────────────────

fn paint_progress_bar(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let h = rect.height().min(6.0).max(4.0);
    let cy = rect.height() / 2.0;
    let track = Rect::from_xywh(0.0, cy - h / 2.0, rect.width(), h);
    fill_rr(session, brush, track, h / 2.0, theme::w(0.08), dim);

    if node.ctrl.indeterminate {
        // A travelling lit segment (one-third width), position from the phase.
        let seg_w = rect.width() * 0.33;
        let travel = rect.width() + seg_w;
        let x = (node.phase % 1.0) * travel - seg_w;
        let seg = Rect::from_xywh(x.max(0.0), cy - h / 2.0, seg_w.min(rect.width()), h);
        fill_rr(session, brush, seg, h / 2.0, theme::accent(), dim);
    } else {
        let frac = super::ctrl_value_frac(node) as f32;
        if frac > 0.0 {
            let fill = Rect::from_xywh(0.0, cy - h / 2.0, rect.width() * frac, h);
            fill_rr(session, brush, fill, h / 2.0, theme::accent(), dim);
        }
    }
}

fn paint_progress_ring(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let cx = rect.width() / 2.0;
    let cy = rect.height() / 2.0;
    let r = (cx.min(cy) - 3.0).max(2.0);
    let thick = (r * 0.18).clamp(2.0, 5.0);

    // Track ring.
    put(brush, theme::w(0.08), dim);
    session.draw_ellipse(&Ellipse::new(Vector2::new(cx, cy), r, r), brush, thick);

    let (a0, a1) = if node.ctrl.indeterminate {
        let s = node.phase * std::f32::consts::TAU;
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
    let rail = Rect::from_xywh(0.0, 0.0, theme::NAV_RAIL_W, rect.height());
    fill_rr(session, brush, rail, 0.0, theme::surface_sunken(), dim);
    // Right divider.
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

    // Active tile wash + sliding accent indicator bar.
    if sel >= 0 {
        let iy = node.anim.x * NAV_ITEM_H;
        let tile = Rect::from_xywh(theme::SPACE_4, iy + theme::SPACE_4, theme::NAV_RAIL_W - theme::SPACE_8, NAV_ITEM_H - theme::SPACE_8);
        fill_rr(session, brush, tile, theme::RADIUS_SM, theme::accent_fill(), dim);
        let bar_h = theme::SPACE_16;
        let bar = Rect::from_xywh(0.0, iy + (NAV_ITEM_H - bar_h) / 2.0, theme::BORDER_W * 3.0, bar_h);
        fill_rr(session, brush, bar, theme::BORDER_W, theme::accent(), dim);
    }

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

    // Spin buttons (wide NumberBox only).
    if node.kind == ControlKind::NumberBox && rect.width() >= editor::SPIN_MIN_BOX_W {
        draw_spin(session, brush, rect, node.hover.x, dim);
    }
}

/// Two stacked up/down chevrons on the trailing edge of a wide `NumberBox`.
fn draw_spin(session: &DrawingSession, brush: &Brush, rect: Rect, hover: f32, dim: f32) {
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
    let color = theme::with_alpha(theme::text_secondary(), 0.6 + 0.4 * hover.clamp(0.0, 1.0));
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
    let wash = ink_wash(node);
    if wash > 0.0 {
        fill_rr(session, brush, header, theme::RADIUS_MD, theme::w(wash), dim);
    }
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

    // Chevron: right (collapsed) → down (expanded). Swap the glyph by progress.
    let g = if node.anim.x > 0.5 { GLYPH_CHEVRON_DOWN } else { GLYPH_CHEVRON_RIGHT };
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
