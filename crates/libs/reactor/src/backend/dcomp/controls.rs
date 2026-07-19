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
        // The button family draws nothing. Its fill, border, hover/press ink,
        // badge plate and focus ring are all retained compositor parts
        // (`parts::button_sync`), and its label, icon and badge count are
        // retained glyph sprites (`glyph_text::button_sync`) — so `has_chrome`
        // denies the family a surface entirely and this arm is what makes the
        // generic fill/border path skip it on the way there.
        ControlKind::Button
        | ControlKind::ToggleButton
        | ControlKind::RepeatButton
        | ControlKind::SplitButton => {}
        ControlKind::HyperlinkButton => paint_hyperlink(session, brush, node, rect, dim),
        // Track, outline, and knob are retained chrome parts (compositor
        // sprites — see `super::parts`); the state label beside them is static
        // text, so it paints here on the node's own surface.
        ControlKind::ToggleSwitch => paint_toggle_label(session, brush, node, rect, dim),
        ControlKind::CheckBox => paint_check_box(session, brush, node, rect, dim),
        ControlKind::SelectorBar => paint_segmented(session, brush, node, rect, dim),
        ControlKind::ComboBox | ControlKind::DropDownButton => {
            paint_select(session, brush, node, rect, dim)
        }
        ControlKind::Slider => paint_slider(session, brush, node, rect, dim),
        ControlKind::Meter => paint_meter(session, brush, node, rect, dim),
        ControlKind::Knob => paint_knob(session, brush, node, rect, dim),
        // Track, fill, and indeterminate sweep are retained chrome parts
        // (`super::parts::progress_sync`) — the sweep loops on the compositor.
        ControlKind::ProgressBar => {}
        ControlKind::ProgressRing => paint_progress_ring(session, brush, node, rect, dim),
        ControlKind::NavigationView => super::nav::paint(session, brush, node, rect, dim),
        // Both draw everything they have on their own surface: nothing on
        // either control moves, so neither owns a retained chrome part.
        ControlKind::InfoBar => super::info_bar::paint(session, brush, node, rect, dim),
        ControlKind::InfoBadge => super::info_badge::paint(session, brush, node, rect, dim),
        ControlKind::Expander => paint_expander(session, brush, node, rect, dim),
        // The custom caption band: only the min/max/close cluster is drawn
        // here (the band itself is transparent; slot children are real nodes).
        ControlKind::TitleBar => super::caption::paint(session, brush, node, rect),
        _ => return false,
    }
    // The button family's ring is a retained part, so it is deliberately absent
    // from this shared tail — see `parts::focus_ring_key`.
    if node.focused && !super::node::is_button_family(node.kind) {
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
pub(crate) fn put(brush: &Brush, c: Color, dim: f32) {
    let mut l = linear(c);
    l.a *= dim;
    brush.set_color(l);
}

pub(crate) fn fill_rr(
    session: &DrawingSession,
    brush: &Brush,
    r: Rect,
    radius: f32,
    c: Color,
    dim: f32,
) {
    put(brush, c, dim);
    if radius > 0.0 {
        session.fill_rounded_rect(&RoundedRect::uniform(r, radius), brush);
    } else {
        session.fill_rect(&r, brush);
    }
}

pub(crate) fn stroke_rr(
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

pub(crate) fn glyph_str(cp: u32) -> Option<String> {
    char::from_u32(cp).map(|c| c.to_string())
}

/// Encode a glyph codepoint into a caller-owned stack buffer — the alloc-free
/// counterpart to [`glyph_str`], for paths that hand the glyph straight to
/// `draw_text` and never need to own it.
pub(crate) fn glyph_into(cp: u32, buf: &mut [u8; 4]) -> Option<&str> {
    char::from_u32(cp).map(|c| &*c.encode_utf8(buf))
}

/// A button's leading icon box.
pub(crate) const ICON_SIZE: f32 = 16.0;
/// The gap between any two adjacent things in a button's content row — icon to
/// label, label to badge, or one ornament to the next on a button with no
/// words. One constant, because the row is a single sequence and a gap that
/// varied by which pair it separated would read as a misalignment.
pub(crate) const ORNAMENT_GAP: f32 = theme::SPACE_8;

/// The hairline of window base between the focus ring and the control it rings
/// (`parts::focus_rings`).
pub(crate) const FOCUS_RING_INNER_W: f32 = 1.0;

/// A focus ring's stroke width. Shared with the button family's retained ring
/// (`parts::button_sync`), which reproduces this geometry as a part because the
/// family owns no surface to draw on.
///
/// Fluent's nominal focus thickness. It only reads as that thickness once it is
/// snapped to whole physical pixels — see `parts::focus_rings`, where an
/// unsnapped ring picked up a partial-coverage edge on BOTH sides and looked
/// both softer and fatter than the number it was given.
pub(crate) const FOCUS_RING_W: f32 = 2.0;

/// A focus ring: a [`FOCUS_RING_W`] `STROKE_STRONG` rounded outline inset 1px
/// from the edge.
pub(crate) fn draw_focus_ring(session: &DrawingSession, brush: &Brush, rect: Rect, radius: f32) {
    let r = Rect::new(rect.left + 1.0, rect.top + 1.0, rect.right - 1.0, rect.bottom - 1.0);
    stroke_rr(session, brush, r, radius, theme::stroke_strong(), FOCUS_RING_W, 1.0);
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

/// Everything about a button-family node's appearance that is a pure function
/// of its state.
///
/// One definition, two consumers that must not disagree: `parts::button_sync`
/// builds the retained fill / border sprites from it, and [`paint_button`]
/// draws the label with it. Splitting the chrome across a compositor part and a
/// painted surface only works while both read the same answer.
pub(crate) struct ButtonPalette {
    pub fill: Color,
    /// `None` when the variant draws no outline.
    pub border: Option<Color>,
    pub fg: Color,
    pub weight: u16,
    pub radius: f32,
    /// The variant fills itself with the accent role (Accent, or a checked
    /// ToggleButton). Read by [`badge_paint`], which has to invert against it.
    pub lit: bool,
}

/// A badge's plate fill and its count's ink, or `None` when the button carries
/// no badge.
///
/// One definition, two consumers that must not disagree: `parts::button_sync`
/// binds the plate and `glyph_text::button_sync` colours the numeral on it.
///
/// The default plate is the accent role, as a standalone `InfoBadge`'s is — but
/// on a LIT button the accent is already the surface, and an accent plate on an
/// accent fill is invisible. There the badge inverts instead, taking the
/// button's own ink for its plate and the button's fill for its count, which
/// preserves exactly the contrast the neutral case has.
///
/// An authored tint always wins, and takes the on-accent ink with it: the host
/// picked a colour the theme never saw, so it owns that contrast decision —
/// the same reasoning `info_badge::paint` documents for a standalone one.
pub(crate) fn badge_paint(node: &Node, pal: &ButtonPalette) -> Option<(Color, Color)> {
    let badge = node.extras().badge?;
    Some(match badge.tint {
        Some(t) => (t, theme::text_on_accent()),
        None if pal.lit => (pal.fg, pal.fill),
        None => (theme::accent(), theme::text_on_accent()),
    })
}

pub(crate) fn button_palette(node: &Node) -> ButtonPalette {
    let accent = node.paint.style_variant == 1;
    // Subtle (2) and TextLink (3) are chromeless by definition — no resting fill
    // and no border, only a hover/press wash. This is what `pressable` wraps a
    // custom visual in (segmented-pill segments, icon buttons), so the wrapped
    // child frames itself; a stray Button outline would double-box it.
    let chromeless = matches!(node.paint.style_variant, 2 | 3);
    let checked = node.ctrl().is_checked && node.kind == ControlKind::ToggleButton;
    let lit = accent || checked;

    // An accent button is a SOLID accent field, not an accent wash: it is the
    // one call-to-action on a surface, and a wash reads as a selected state
    // rather than a primary action. Its label is therefore the on-accent token
    // — accent text on an accent fill is the same hue twice and fails contrast
    // outright.
    // An authored fill wins outright. A pill or chip is a button whose whole
    // identity is its tint — a filter chip in the accent wash, a layer chip in
    // its layer's colour — and deriving that from a style variant would mean
    // enumerating the app's palette here. `Background` on a button used to be
    // silently inert, which is what made every such control hand-roll a
    // `Border` around a chromeless button instead of just being one.
    let fill = node.paint.background.unwrap_or(if lit {
        theme::accent()
    } else if chromeless || matches!(node.kind, ControlKind::Button | ControlKind::RepeatButton) {
        // Chromeless / bare button: transparent at rest, wash appears via ink.
        theme::w(0.0)
    } else {
        theme::surface_raised()
    });

    ButtonPalette {
        fill,
        // Same rule for the outline, and it has to be the same rule: a chip
        // authoring a fill almost always authors the stroke that frames it, and
        // honouring one but not the other would draw the theme's border around
        // the app's fill.
        border: node
            .paint
            .border_brush
            .or_else(|| (!lit && !chromeless).then(theme::stroke)),
        fg: node.paint.foreground.unwrap_or(if lit {
            theme::text_on_accent()
        } else if node.paint.style_variant == 3 {
            // A TextLink reads as an inline accent hyperlink: accent type, and
            // no field behind it to need an on-accent colour.
            theme::accent()
        } else {
            theme::text()
        }),
        weight: if accent { 600 } else { 400 },
        lit,
        // No floor: the family is BORN at `RADIUS_MD` (`node::birth_paint`), so
        // an authored radius here is one the app asked for — including a
        // smaller one, which a `.max()` used to swallow.
        radius: resolve_radius(node.paint.corner_radius, node.rect.h),
    }
}

/// Clamp an authored corner radius to what the box can actually curve.
///
/// A rounded rectangle's corners overlap past half the shorter side, so this is
/// the geometric bound rather than a style choice. It is also what makes
/// [`crate::PILL_RADIUS`] work: an unbounded authored radius lands here and
/// resolves to the fully-rounded end for whatever height the button measured.
pub(crate) fn resolve_radius(authored: f32, h: f32) -> f32 {
    if h <= 0.0 {
        // Pre-layout, before a height exists. Returning the authored value
        // unchanged would hand `f32::INFINITY` to the geometry; 0 draws a
        // square that the first real sync immediately replaces.
        return if authored.is_finite() { authored } else { 0.0 };
    }
    authored.clamp(0.0, h * 0.5)
}

/// Whether a button-family node has a leading icon glyph *and* a label to sit
/// beside it.
///
/// `0` is the "no icon" convention (`Ctrl::icons`), and it must be tested rather
/// than left to `glyph_into`: `char::from_u32(0)` is `Some('\0')`, so every
/// un-iconed button would otherwise lay out and draw a NUL.
fn has_leading_icon(node: &Node) -> bool {
    node.extras().icon != 0
}

/// A button's badge as laid out: its `(width, height)`, or `None` when the
/// button carries none.
///
/// The dot is a fixed square. The count's plate takes its height from the
/// badge's pill metric and its width from the measured numeral plus padding,
/// floored at a circle so a single digit reads as round rather than squashed —
/// the same geometry a standalone `InfoBadge` measures, because it is the same
/// control hosted in a different box.
pub(crate) fn badge_size(node: &Node) -> Option<(f32, f32)> {
    let badge = node.extras().badge?;
    if badge.count.is_none() {
        return Some((super::info_badge::DOT_D, super::info_badge::DOT_D));
    }
    let text_w = node
        .button_text
        .as_ref()
        .and_then(|t| t.badge_layout.as_ref())
        .and_then(|l| l.measure().ok())
        .map_or(0.0, |(w, _)| w);
    let h = super::info_badge::PILL_H;
    Some((
        (text_w + 2.0 * super::info_badge::PILL_PAD_X).max(h),
        h,
    ))
}

/// Where a button's content sits: the leading icon, the badge, and the label
/// between them. All node-local, all derived from `rect`.
///
/// One definition, four consumers that must not disagree — the same discipline
/// [`ButtonPalette`] documents. `glyph_text::button_sync` places all three runs
/// from this, `parts::button_sync` sizes the badge plate from it, and the
/// layout measure widens the control by exactly the ornaments it reserves here.
pub(crate) struct ButtonBoxes {
    pub icon: Option<Rect>,
    pub badge: Option<Rect>,
    pub label: Rect,
}

pub(crate) fn button_boxes(node: &Node, rect: Rect) -> ButtonBoxes {
    let leads = node.extras().badge.is_some_and(|b| b.leading);
    let badge_sz = badge_size(node);
    let label_w = label_width(node);

    // The row, in visual order, with the width each present item occupies. A
    // leading badge heads it: it is a status lamp for the whole control —
    // "● Live" — so it reads at the start rather than wedged between the icon
    // and the words it qualifies.
    let row = [
        (Slot::Badge, badge_sz.filter(|_| leads).map(|s| s.0)),
        (Slot::Icon, has_leading_icon(node).then_some(ICON_SIZE)),
        (Slot::Label, (label_w > 0.0).then_some(label_w)),
        (Slot::Badge, badge_sz.filter(|_| !leads).map(|s| s.0)),
    ];
    let n = row.iter().filter(|(_, w)| w.is_some()).count();

    // The row is CENTRED AS A GROUP, and that is the whole placement rule.
    //
    // Pinning the ornaments to the edges and centring the label in whatever
    // survives is the same arithmetic only when the row is symmetric: give a
    // button a leading icon and the label centres inside a box that still
    // contains the TRAILING padding, so it sits half that padding right of
    // where it belongs — visible on every iconed button in the family.
    //
    // Centring the group is also what makes the row honour whatever padding
    // the host authored, instead of an inset constant that is only ever right
    // for the family's own.
    let content: f32 = row.iter().filter_map(|(_, w)| *w).sum::<f32>()
        + ORNAMENT_GAP * n.saturating_sub(1) as f32;
    let mut cursor = rect.left + ((rect.width() - content) / 2.0).max(0.0);

    // Vertically centred rather than full-height: unlike the icon, the badge
    // has a plate, and a plate as tall as the button is not a badge.
    let plate = |x: f32, (bw, bh): (f32, f32)| {
        let y = rect.top + ((rect.height() - bh) / 2.0).max(0.0);
        Rect::new(x, y, x + bw, y + bh)
    };

    let (mut icon, mut badge, mut label) = (None, None, rect);
    let mut first = true;
    for (slot, w) in row {
        let Some(w) = w else { continue };
        if !first {
            cursor += ORNAMENT_GAP;
        }
        first = false;
        let at = cursor;
        cursor += w;
        match slot {
            Slot::Icon => icon = Some(Rect::new(at, rect.top, at + w, rect.bottom)),
            Slot::Badge => badge = badge_sz.map(|sz| plate(at, sz)),
            // Full height so the run still centres VERTICALLY on the control.
            // Horizontally this box is exactly the run's own width, so the
            // centring `place_centered` does becomes a no-op on x.
            Slot::Label => label = Rect::new(at, rect.top, at + w, rect.bottom),
        }
    }
    ButtonBoxes { icon, badge, label }
}

/// What a button's own outline takes out of its box, on each axis.
///
/// The stroke is drawn INSIDE the border box — a nine-grid part cut to the full
/// rect — so a button that draws one has to measure room for it, exactly as CSS
/// `border-box` reserves a border. Without this an outlined button comes out two
/// strokes smaller than the same control composed from a `Border` with the same
/// padding, which is the difference every pill lost on being migrated onto the
/// family.
///
/// Zero for the variants that draw no outline, so a chromeless chip is not
/// charged for a stroke it never renders.
pub(crate) fn chrome_inset(node: &Node) -> f32 {
    if button_palette(node).border.is_some() {
        2.0 * theme::BORDER_W
    } else {
        0.0
    }
}

/// Which piece of the content row a slot holds.
#[derive(Copy, Clone)]
enum Slot {
    Badge,
    Icon,
    Label,
}

/// The measured width of the label's cached run, or `0.0` when there is none.
///
/// Read here rather than passed in, so every consumer of [`button_boxes`]
/// answers from the same shaped run the sprites are actually placed from.
fn label_width(node: &Node) -> f32 {
    node.text_layout
        .as_ref()
        .filter(|_| label_is_retained(node))
        .and_then(|l| l.measure().ok())
        .map_or(0.0, |(w, _)| w)
}

/// The width a button's ornaments add to its measured label.
///
/// The row is a sequence — leading badge, icon, label, trailing badge — so the
/// reservation is the items' own widths plus one gap between each adjacent
/// pair. Counting the gaps from the items is what keeps an icon-and-badge
/// button with no words from measuring the two flush against each other.
pub(crate) fn ornament_width(node: &Node) -> f32 {
    let mut total = 0.0;
    let mut items = i32::from(!node.paint.text.is_empty());
    if node.extras().icon != 0 {
        total += ICON_SIZE;
        items += 1;
    }
    if let Some((bw, _)) = badge_size(node) {
        total += bw;
        items += 1;
    }
    total + ORNAMENT_GAP * (items - 1).max(0) as f32
}

/// Whether this node draws a label at all.
///
/// The layout is required, not merely preferred: it is the shaped run the
/// sprites are placed from, and without one there is nothing to place. In
/// practice a button-family node with a label always has one — `layout::is_text`
/// covers the whole family — so the `None` arm is a genuine failure (a
/// `TextFormat` that would not build), and a button with no words is a better
/// answer for it than a fallback path nothing else in the family has.
pub(crate) fn label_is_retained(node: &Node) -> bool {
    super::node::is_button_family(node.kind)
        && !node.paint.text.is_empty()
        && node.text_layout.is_some()
}

// ── ToggleSwitch ─────────────────────────────────────────────────────────────

/// Gap between the switch track and its state label (the WinUI metric).
pub(crate) const TOGGLE_LABEL_GAP: f32 = theme::SPACE_12;

/// The label for the state the switch is currently in. Empty when the app set
/// neither `OnContent` nor `OffContent`, which is the bare-switch default.
pub(crate) fn toggle_label(node: &Node) -> &str {
    let x = node.extras();
    if node.ctrl().is_on {
        &x.on_content
    } else {
        &x.off_content
    }
}

/// The state label beside the track — left-aligned after it, vertically
/// centred on the track, in the body text style.
///
/// Intentionally NOT a retained part: it does not move or fade, it only
/// changes at the moment the switch flips, which already forces exactly one
/// repaint of this surface.
fn paint_toggle_label(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let label = toggle_label(node);
    if label.is_empty() {
        return;
    }
    let x0 = rect.left + super::parts::TRACK_W + TOGGLE_LABEL_GAP;
    let box_ = Rect::new(x0, rect.top, rect.right, rect.bottom);
    text(
        session,
        brush,
        label,
        box_,
        "Segoe UI",
        node.paint.font_size.max(theme::FONT_SIZE_MD),
        400,
        node.paint.foreground.unwrap_or(theme::text()),
        TextAlignment::Leading,
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
    let n = node.ctrl().items.len();
    let m = seg_metrics(node.paint.style_variant, node.paint.font_size);
    let inner = (node.rect.w - 2.0 * m.tray).max(0.0);
    let mut widths: Vec<f32> = if node.ctrl().seg_label_w.len() == n {
        node.ctrl().seg_label_w.iter().map(|w| w + 2.0 * m.pad_x).collect()
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
    let n = node.ctrl().items.len();
    if n == 0 {
        return;
    }
    let edges = segment_edges(node);
    let pill_h = h - 2.0 * m.tray;
    let seg_rect = |i: usize| {
        Rect::from_xywh(rect.left + edges[i], rect.top + m.tray, edges[i + 1] - edges[i], pill_h)
    };
    let hot = node.ctrl().hot_index;

    // Segment labels.
    for (i, label) in node.ctrl().items.iter().enumerate() {
        let active = i as i32 == node.ctrl().selected_index;
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
        node.ctrl()
            .items
            .get(node.ctrl().selected_index.max(0) as usize)
            .cloned()
            .filter(|_| node.ctrl().selected_index >= 0)
            .unwrap_or_else(|| node.ctrl().placeholder.clone())
    } else {
        node.paint.text.clone()
    };
    let lr = Rect::new(rect.left + theme::SPACE_8, rect.top, rect.right - theme::SPACE_24, rect.bottom);
    let label_color = if node.ctrl().selected_index < 0 && node.kind == ControlKind::ComboBox {
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

    // Only the static groove paints here; the accent fill, hover halo, and
    // thumb are retained chrome parts above this surface (`super::parts`) —
    // a drag is pure compositor property sets, no repaint.
    let groove = Rect::from_xywh(x0, cy - theme::SLIDER_TRACK / 2.0, x1 - x0, theme::SLIDER_TRACK);
    fill_rr(session, brush, groove, theme::SLIDER_TRACK / 2.0, theme::w(0.06), dim);

    // A fill origin strictly inside the range (a bidirectional gain-style
    // slider) gets a notch standing proud of the track — brighter than the
    // groove, dimmer than the thumb — so "where is neutral?" is answerable at
    // rest. Origins at (or clamped to) an endpoint are just the track end.
    if let Some(o) = node.ctrl().fill_origin
        && o > node.ctrl().min.min(node.ctrl().max)
        && o < node.ctrl().min.max(node.ctrl().max)
    {
        let ofrac = super::parts::slider_origin_frac(node);
        let ox = x0 + (x1 - x0).max(0.0) * ofrac;
        let tick_h = theme::SLIDER_TRACK + 4.0;
        let tick = Rect::from_xywh(ox - 0.5, cy - tick_h / 2.0, 1.0, tick_h);
        fill_rr(session, brush, tick, 0.5, theme::w(0.15), dim);
    }
}

// ── Meter ────────────────────────────────────────────────────────────────────

/// The meter's track fraction for its reference marker (`ctrl.marker` clamped
/// into `[min, max]`; `None` = no marker).
pub(crate) fn meter_marker_frac(node: &Node) -> Option<f32> {
    let m = node.ctrl().marker?;
    let span = node.ctrl().max - node.ctrl().min;
    if span.abs() < f64::EPSILON {
        None
    } else {
        Some(((m - node.ctrl().min) / span).clamp(0.0, 1.0) as f32)
    }
}

/// Only the static groove paints here; the gradient fill, reference marker,
/// and position needle are retained chrome parts above this surface
/// (`super::parts::meter_sync` — the marker rides above the fill so it stays
/// visible when the level passes it) — a level change is a compositor spring
/// retarget, no repaint.
fn paint_meter(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let _ = node;
    let top = theme::METER_INSET;
    let bot = (rect.height() - theme::METER_INSET).max(top + 1.0);
    let groove = Rect::new(0.0, top, rect.width(), bot);
    fill_rr(session, brush, groove, theme::METER_RADIUS, theme::w(0.06), dim);
}

// ── Knob ─────────────────────────────────────────────────────────────────────

/// Static dial chrome: background track ring, ticks, numeric labels, center hub,
/// and the center readout. The gradient value arc + needle are retained
/// compositor vector chrome above this surface (`super::knob`) — the arc grows
/// on a `TrimEnd` spring, so a value change repaints only the readout here.
fn paint_knob(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    use super::knob::{dial_geom, value_to_angle, LABEL_OFFSET};
    let (cx, cy, radius) = dial_geom(node);
    let (min, max) = (node.ctrl().min, node.ctrl().max);
    let (start, end) = (node.ctrl().start_angle, node.ctrl().end_angle);
    let _ = rect;

    // Background track (full sweep), a wide soft groove under the value arc.
    arc(session, brush, cx, cy, radius, 10.0, start, end, theme::w(0.06), dim);

    // Ticks: minor + major (longer/brighter on an exact `major_every` multiple).
    for &tv in &node.ctrl().ticks {
        let a = value_to_angle(tv, min, max, start, end);
        let major = node
            .ctrl()
            .major_every
            .filter(|m| *m != 0.0)
            .is_some_and(|m| (tv % m).abs() < 1e-9);
        let tick_len = if major { 8.0 } else { 5.0 };
        let inner = radius - tick_len - 4.0;
        let outer = radius - 4.0;
        let (ca, sa) = (a.cos(), a.sin());
        put(brush, theme::w(if major { 0.28 } else { 0.14 }), dim);
        session.draw_line(
            Vector2::new(cx + ca * inner, cy + sa * inner),
            Vector2::new(cx + ca * outer, cy + sa * outer),
            brush,
            if major { 1.5 } else { 1.0 },
        );
    }

    // Numeric labels outside the track (app-formatted strings).
    let label_font = (radius * 0.1).max(10.0);
    let lr = radius + LABEL_OFFSET;
    for (v, label) in &node.ctrl().tick_labels {
        let a = value_to_angle(*v, min, max, start, end);
        let lx = cx + a.cos() * lr;
        let ly = cy + a.sin() * lr;
        let box_ = Rect::from_xywh(lx - lr, ly - label_font, 2.0 * lr, 2.0 * label_font);
        text(
            session,
            brush,
            label,
            box_,
            "Segoe UI",
            label_font,
            400,
            theme::text_tertiary(),
            TextAlignment::Center,
            ParagraphAlignment::Center,
            dim,
        );
    }

    // Center hub.
    put(brush, theme::w(1.0), dim);
    session.fill_ellipse(&Ellipse::new(Vector2::new(cx, cy), 4.0, 4.0), brush);

    // Center readout: value (large, thin) + unit + sub-line, app-formatted.
    let readout_size = (radius * 0.38).max(20.0);
    let base_y = cy + radius * 0.35;
    if !node.paint.text.is_empty() {
        let box_ = Rect::from_xywh(cx - radius, base_y - readout_size, 2.0 * radius, 2.0 * readout_size);
        text(
            session, brush, &node.paint.text, box_, "Segoe UI", readout_size, 200,
            theme::w(0.9), TextAlignment::Center, ParagraphAlignment::Center, dim,
        );
    }
    if !node.ctrl().unit.is_empty() {
        let uy = base_y + readout_size * 0.45;
        let box_ = Rect::from_xywh(cx - radius, uy, 2.0 * radius, readout_size);
        text(
            session, brush, &node.ctrl().unit, box_, "Segoe UI", readout_size * 0.35, 400,
            theme::w(0.35), TextAlignment::Center, ParagraphAlignment::Top, dim,
        );
    }
    if !node.ctrl().sub_text.is_empty() {
        let sy = base_y + readout_size * 0.75;
        let sub_size = (radius * 0.1).max(8.0);
        let box_ = Rect::from_xywh(cx - radius, sy, 2.0 * radius, 2.0 * sub_size);
        text(
            session, brush, &node.ctrl().sub_text, box_, "Segoe UI", sub_size, 400,
            theme::w(0.25), TextAlignment::Center, ParagraphAlignment::Top, dim,
        );
    }
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
    let (a0, a1) = if node.ctrl().indeterminate {
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
    let Some(band) = editor::TextBand::of(node) else { return };
    let align = node.ctrl().content_align;
    let content_w = band.content_w;
    let cx0 = rect.left + band.content_x;
    let font_size = node.paint.font_size;

    // Only the placeholder is still drawn in this column; the run itself is
    // sprites, clipped by their own host.
    let clip = Rect::from_xywh(cx0, rect.top, content_w, rect.height());
    session.push_clip(&clip);

    if ed.buf.is_empty() {
        // The placeholder is still painted: it is not the editor's text, it has
        // no layout of its own, and it is drawn by whole-string alignment rather
        // than from a shaped run.
        text(
            session,
            brush,
            &node.ctrl().placeholder,
            Rect::new(cx0, rect.top, cx0 + content_w, rect.bottom),
            "Segoe UI",
            font_size,
            400,
            theme::text_tertiary(),
            editor_text_alignment(align),
            ParagraphAlignment::Center,
            dim,
        );
    }

    // The text run, its selection wash and its composition rule are NOT drawn
    // here — they are retained sprites (`glyph_text::editor_sync`), clipped by
    // their host rather than by this `push_clip`. Nor is the caret, whose blink
    // plays DWM-side (`parts::sync_caret`, [`editor_caret_box`]).

    session.pop_clip();

    // Spin buttons (wide NumberBox only). The chevron brighten keys off the
    // hover flag — the flip repaints once, event-driven (no tick).
    if node.kind == ControlKind::NumberBox && rect.width() >= editor::SPIN_MIN_BOX_W {
        draw_spin(session, brush, rect, node.hovered, dim);
    }
}

/// The caret's box in node-local DIPs, clamped into the content column the
/// painted text clips to. `None` when the node is not an editor. Consumed by
/// `parts::sync_caret` to place the caret sprite.
///
/// Takes its origin and line height from [`editor::TextBand`], which is the
/// same answer the painter draws the run at — so the caret cannot drift from
/// the text it sits in.
pub(crate) fn editor_caret_box(node: &Node) -> Option<Rect> {
    let ed = node.editor.as_ref()?;
    let band = editor::TextBand::of(node)?;
    let caret_x = band.origin_x + ed.caret_x();
    // A 1-DIP bar centred on the caret position, kept inside the clip column.
    let lo = band.content_x;
    let x = (caret_x - 0.5).clamp(lo, (lo + band.content_w - 1.0).max(lo));
    Some(Rect::from_xywh(
        x,
        band.origin_y + 1.0,
        1.0,
        (band.text_h - 2.0).max(1.0),
    ))
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
    let g = if node.ctrl().expanded { GLYPH_CHEVRON_DOWN } else { GLYPH_CHEVRON_RIGHT };
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

#[cfg(test)]
mod tests {
    use super::resolve_radius;
    use crate::PILL_RADIUS;

    #[test]
    fn pill_radius_resolves_to_half_the_height() {
        assert_eq!(resolve_radius(PILL_RADIUS as f32, 32.0), 16.0);
        assert_eq!(resolve_radius(PILL_RADIUS as f32, 48.0), 24.0);
    }

    #[test]
    fn an_authored_radius_passes_through_untouched() {
        assert_eq!(resolve_radius(0.0, 32.0), 0.0);
        assert_eq!(resolve_radius(2.0, 32.0), 2.0);
        assert_eq!(resolve_radius(8.0, 32.0), 8.0);
    }

    #[test]
    fn an_over_large_radius_clamps_rather_than_overlapping() {
        assert_eq!(resolve_radius(100.0, 24.0), 12.0);
    }

    /// Before layout there is no height to resolve against. An unbounded
    /// radius must not reach the geometry as an infinity.
    #[test]
    fn an_unmeasured_box_never_yields_a_non_finite_radius() {
        assert_eq!(resolve_radius(PILL_RADIUS as f32, 0.0), 0.0);
        assert_eq!(resolve_radius(8.0, 0.0), 8.0);
    }
}
