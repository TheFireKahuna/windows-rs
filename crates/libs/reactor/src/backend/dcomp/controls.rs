//! The drawn control library: per-`ControlKind` chrome painters plus the shared
//! ink (hover/press wash), focus-ring, and small shape helpers every control
//! reuses. Pure rendering — interaction lives in `input.rs`, state in
//! `node::Ctrl`. All colours/metrics come from [`theme`]; nothing here is a raw
//! literal except geometric ratios and glyph codepoints.
//!
//! **Nothing here draws text.** Every run a control shows is a retained glyph
//! sprite placed by [`glyph_text`](super::glyph_text) above whatever this module
//! paints, so this file imports no DirectWrite type and the geometry it exports
//! — the boxes runs are placed into, the codepoints they are shaped from — is
//! consumed by that module rather than used here. What remains is fills,
//! strokes, arcs and rings.

use super::editor;
use super::node::{is_text_editable, linear, Node};
use super::theme;
use crate::backend::ControlKind;
use crate::Color;
use windows_canvas_core::{Brush, DrawingSession, Ellipse, Rect, RoundedRect, Vector2};

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
        // Fully retained, like the button family: the ring is a part
        // (`parts::hyperlink_plan`) and the words are glyph sprites, so there is
        // nothing to draw and `has_chrome` denies it a surface.
        ControlKind::HyperlinkButton => {}
        // Track, outline, knob and ring are retained chrome parts
        // (`parts::toggle_plan`); the state label beside them is glyph sprites
        // (`glyph_text::toggle_sync`). Nothing left to draw, so `has_chrome`
        // denies it a surface.
        ControlKind::ToggleSwitch => {}
        // Box fill, outline, checkmark and ring are parts
        // (`parts::check_plan`); the trailing label is glyph sprites
        // (`glyph_text::check_sync`).
        ControlKind::CheckBox => {}
        // The same: tray, sliding pill, hover ink and ring are parts
        // (`parts::segmented_plan`), the segment labels are sprites
        // (`glyph_text::segmented_sync`).
        ControlKind::SelectorBar => {}
        // Fully retained, like the button family: box fill, border, hover/press
        // wash and focus ring are parts (`parts::select_plan`), and the current
        // label and trailing chevron are glyph sprites
        // (`glyph_text::select_sync`). The two-statement `paint_select` this
        // replaced was the last thing holding the surface open.
        ControlKind::ComboBox | ControlKind::DropDownButton => {}
        // Track, origin notch, accent fill, hover halo, thumb and focus ring
        // are all retained parts (`parts::slider_sync`) — groove and notch in
        // the below band, the rest above — so `has_chrome` denies it a surface
        // and a scrub stays pure compositor property sets.
        ControlKind::Slider => {}
        // Groove, gradient fill, reference marker and needle are all retained
        // chrome parts (`super::parts::meter_sync`). The groove was the last
        // thing drawn here and it did not depend on the level, so every level
        // change was re-rastering a constant; `has_chrome` denies it a surface.
        ControlKind::Meter => {}
        // The dial is retained end to end: the groove ring and both tick classes
        // are mask layers over the very path the value arc rides, the hub is an
        // FP16 circle sprite, the arc, thumb and needle were compositor chrome
        // already, and all four runs are glyph sprites
        // (`glyph_text::knob_sync`). Only the focus ring is left, and only while
        // the knob is keyboard-focused — see `Node::has_chrome`.
        ControlKind::Knob => {}
        // Track, fill and indeterminate sweep are retained chrome parts
        // (`super::parts::progress_plan`; the sweep is armed by
        // `progress_sweep` and loops on the compositor), so `has_chrome`
        // denies it a surface.
        ControlKind::ProgressBar => {}
        ControlKind::ProgressRing => paint_progress_ring(session, brush, node, rect, dim),
        // Fully retained: background, divider, selection tile, accent bar and
        // both washes are parts (`parts::nav_plan`), every run is sprites
        // (`glyph_text::nav_sync`), and `has_chrome` denies it a surface.
        ControlKind::NavigationView => {}
        // Header fill, border, hover wash and ring are parts
        // (`parts::expander_plan`); the header label and its chevron are glyph
        // sprites (`glyph_text::expander_sync`).
        ControlKind::Expander => {}
        // The custom caption band draws nothing either: it is transparent, its
        // one hover wash is a part (`parts::caption_plan`), and its two titles
        // and four button glyphs are sprites (`glyph_text::caption_sync`).
        ControlKind::TitleBar => {}
        _ => return false,
    }
    // Kinds whose ring is a retained part are deliberately absent from this
    // shared tail — see `ring_is_retained`.
    if node.focus_ring && !ring_is_retained(node.kind) {
        draw_focus_ring(session, brush, rect, focus_radius(node));
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

/// Whether this kind's focus ring is a retained part rather than a draw.
///
/// The two rings are NOT the same visual — `draw_focus_ring` strokes once,
/// INSET into the control; `parts::focus_ring_slots` lays two rings just
/// outside it, which is the shape `parts::focus_rings` argues for (an inset
/// ring eats the control's own fill, so a focused control reads as having grown
/// a border and shrunk). A kind must therefore be on exactly one of them, and
/// this predicate is the single place that is decided.
fn ring_is_retained(kind: ControlKind) -> bool {
    super::node::is_button_family(kind)
        || matches!(
            kind,
            ControlKind::HyperlinkButton
                | ControlKind::ToggleSwitch
                | ControlKind::SelectorBar
                | ControlKind::CheckBox
                | ControlKind::Expander
                | ControlKind::ComboBox
                | ControlKind::DropDownButton
                | ControlKind::Slider
                | ControlKind::Knob
        )
}

/// The corner radius a kind's focus ring follows.
///
/// Read by BOTH ring implementations — `draw_focus_ring` here and
/// `parts::focus_ring_slots` — so a kind keeps its corners across the move from
/// one to the other. The retained ring grows this by how far out each ring sits;
/// this is the authored radius, not the ring's own.
pub(crate) fn focus_radius(node: &Node) -> f32 {
    match node.kind {
        ControlKind::ToggleSwitch | ControlKind::Slider => theme::RADIUS_PILL,
        // The accent segmented tray is a stadium, so its ring is one too. The
        // subtle variant is the dense toolbar tray and keeps square-ish corners.
        ControlKind::SelectorBar if node.paint.style_variant == 1 => node.rect.h / 2.0,
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

/// The word a switch currently shows: `on_content` when on, `off_content` when
/// off, and empty when that side was never given one.
///
/// This is the same choice `glyph_text::toggle_sync` makes when it picks which
/// of the two shaped runs to place, expressed once so a screen reader is told
/// the word that is actually on screen rather than a second guess at it.
pub(crate) fn toggle_state_label(node: &Node) -> &str {
    let x = node.extras();
    if node.ctrl().is_on { &x.on_content } else { &x.off_content }
}

// ── CheckBox ─────────────────────────────────────────────────────────────────

/// The trailing label's box: everything right of the box and its gap.
///
/// Shared by the sprite placement and by nothing else today, but stated here
/// rather than inline because it is derived from the box side the PART is cut
/// to — a label that measured its own gap from a second literal would drift the
/// moment the box changed size.
pub(crate) fn check_label_box(node: &Node) -> Rect {
    let x = super::parts::CHECK_BOX_D + theme::SPACE_8;
    Rect::from_xywh(x, 0.0, (node.rect.w - x).max(0.0), node.rect.h)
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

/// The size a segment label is shaped and drawn at.
///
/// The floor is what makes a bar with no explicit font size legible rather than
/// hairline. It belongs here, beside the geometry, because the measure pass and
/// the sprite placement must shape at the SAME size — when the floor lived only
/// in the draw call, `seg_label_w` was measured at the unfloored size and every
/// segment was sized to a narrower run than the one that landed in it.
pub(crate) fn seg_font_size(node: &Node) -> f32 {
    node.paint.font_size.max(theme::FONT_SIZE_MICRO)
}

/// The rest and emphasis weights a segment label takes. The selected segment
/// sets; the rest do not.
pub(crate) const SEG_WEIGHT: u16 = 400;
pub(crate) const SEG_WEIGHT_ACTIVE: u16 = 600;

// ── Select / ComboBox / DropDownButton trigger ───────────────────────────────

/// The size and weight a select trigger's label is shaped at.
///
/// Deliberately not the node's own text style: the trigger has always drawn at
/// the small ramp regardless of what the app set, and the measure pass shapes
/// its candidates at this size too. One constant, because a measurement taken at
/// a different size from the run that lands is a label that overflows its box.
pub(crate) const SELECT_FONT_SIZE: f32 = theme::FONT_SIZE_SM;
pub(crate) const SELECT_WEIGHT: u16 = 400;

/// The trailing chevron's codepoint and size, shared by the layout pass that
/// shapes it and the sprite sync that places it.
pub(crate) const SELECT_CHEVRON: u32 = GLYPH_CHEVRON_DOWN;
pub(crate) const SELECT_CHEVRON_SIZE: f32 = 8.0;

/// A select trigger's label column: leading inset, stopping at the chevron.
///
/// Node-local, like every other box the sprite syncs place into — the trigger's
/// surface starts at the node's own origin, so the two agree.
pub(crate) fn select_label_box(node: &Node) -> Rect {
    let right = (node.rect.w - theme::SPACE_24).max(theme::SPACE_8);
    Rect::new(theme::SPACE_8, 0.0, right, node.rect.h)
}

/// A select trigger's trailing chevron column.
pub(crate) fn select_chevron_box(node: &Node) -> Rect {
    let left = (node.rect.w - theme::SPACE_24).max(0.0);
    Rect::new(left, 0.0, (node.rect.w - theme::SPACE_4).max(left), node.rect.h)
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

// ── Knob ─────────────────────────────────────────────────────────────────────

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

    // Nothing in the content column is drawn here any more. The text run, the
    // placeholder that stands in for it, the selection wash and the composition
    // rule are all retained sprites (`glyph_text::editor_sync`), clipped by
    // their own host rather than by a `push_clip`. Nor is the caret, whose blink
    // plays DWM-side (`parts::sync_caret`, [`editor_caret_box`]).

    // The spin column's divider (wide NumberBox only). Its two chevrons are
    // sprites, placed by `glyph_text::editor_sync` from the same
    // `editor::spin_boxes` the press reads.
    if node.kind == ControlKind::NumberBox && rect.width() >= editor::SPIN_MIN_BOX_W {
        draw_spin(session, brush, rect, dim);
    }
}

/// The caret's box in node-local DIPs, clamped into the content column the
/// painted text clips to. `None` when the node is not an editor. Consumed by
/// `parts::sync_caret` to place the caret sprite.
///
/// Vertical extent comes from DirectWrite ([`editor::Editor::caret_geom`]) once
/// a layout exists, and from [`editor::TextBand`]'s fallback before that — the
/// same band the painter draws the run at, so the caret cannot drift from the
/// text it sits in either way.
pub(crate) fn editor_caret_box(node: &Node, scale: f32) -> Option<Rect> {
    let ed = node.editor.as_ref()?;
    let band = editor::TextBand::of(node)?;
    // Before the first layout there is nothing to ask, so the band answers.
    // Its fallback line height is what the caret is placed by for that whole
    // window, which is why `TextBand` refuses to let it collapse.
    let geom = ed.caret_geom();
    let caret_x = band.origin_x + geom.map_or_else(|| ed.caret_x(), |g| g.x);
    let (top, height) = match geom {
        Some(g) => (band.origin_y + g.top, g.height),
        None => (band.origin_y, band.text_h),
    };
    Some(caret_box(&band, caret_x, top, height, editor::caret_width(), scale))
}

/// The caret rect from scalars alone — no node, no layout, no device, so the
/// arithmetic every caret inherits is exhaustively testable. Same split
/// [`editor::TextBand::compute`] uses, and for the same reason.
///
/// # Why the bar straddles the insertion point
///
/// It is centred on `caret_x`, not started there. That is what Microsoft's own
/// DirectWrite editor sample does (PadWrite `GetCaretRect`:
/// `rect.left = caretX - caretThickness / 2`), and the reason is only visible
/// once the thickness is not 1: the caret marks a *boundary between* two
/// characters, so a wide one must grow into both sides. Started at the boundary
/// instead, a 10-DIP accessibility caret would sit entirely on top of the
/// character after it and hide it — the thickness setting exists to make the
/// caret easier to see, and that arrangement makes it eat the text.
///
/// # Why every edge is snapped
///
/// A bar at a fractional pixel offset is resolved as two partly-lit columns:
/// dimmer, wider, and smeared toward whichever side got the larger fraction.
/// The caret is the thinnest thing on screen and so the most sensitive to it.
/// Snapping happens AFTER centring, so the half-thickness offset can never
/// reintroduce a fraction.
fn caret_box(
    band: &editor::TextBand,
    caret_x: f32,
    top: f32,
    height: f32,
    width_dip: f32,
    scale: f32,
) -> Rect {
    // A non-finite scale would poison every coordinate through `snap`; a
    // non-finite position or height would reach a visual's Offset/Size, where
    // the failure is not a misplaced caret but one that stops compositing.
    let px = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let snap = |dip: f32| (dip * px).round() / px;
    let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };

    // Whole physical pixels, never zero — the bar is exactly as wide as it is lit.
    let w = (finite(width_dip, 1.0).max(0.0) * px).round().max(1.0) / px;
    let h = snap(finite(height, 0.0).max(1.0 / px));

    let lo = band.content_x;
    // The column the painted run is clipped to. A caret at the very end of a
    // full field would otherwise sit past the clip, drawing over the border or
    // the spin buttons — the one place the sprite has no clip of its own to
    // stop it, since it is a sibling of the text host rather than a child.
    let hi = (lo + band.content_w - w).max(lo);
    let x = snap(finite(caret_x, lo) - w / 2.0).clamp(snap(lo), snap(hi));
    Rect::from_xywh(x, snap(finite(top, 0.0)), w, h)
}

/// Two stacked up/down chevrons on the trailing edge of a wide `NumberBox`.
/// The spin column's two codepoints and the size they are shaped at, shared by
/// the layout pass that shapes them and the sprite sync that places them.
pub(crate) const SPIN_GLYPH_UP: u32 = GLYPH_CHEVRON_UP;
pub(crate) const SPIN_GLYPH_DOWN: u32 = GLYPH_CHEVRON_DOWN;
pub(crate) const SPIN_CHEVRON_SIZE: f32 = 6.0;

/// The ink a spin chevron takes. Hover brightens it; the flip is a `SetSource`
/// on the shared colour brush now, so it re-rasterizes nothing.
pub(crate) fn spin_ink(hover: bool) -> Color {
    theme::with_alpha(theme::text_secondary(), if hover { 1.0 } else { 0.6 })
}

/// The hairline before the spin column. The two chevrons beyond it are retained
/// glyph sprites (`glyph_text::editor_sync`).
fn draw_spin(session: &DrawingSession, brush: &Brush, rect: Rect, dim: f32) {
    let col_x = rect.right - editor::SPIN_W;
    put(brush, theme::stroke(), dim);
    session.draw_line(
        Vector2::new(col_x, rect.top + theme::SPACE_4),
        Vector2::new(col_x, rect.bottom - theme::SPACE_4),
        brush,
        theme::BORDER_W,
    );
}

// ── Expander ─────────────────────────────────────────────────────────────────

/// The header strip's height — the only part of an Expander that is chrome; the
/// content below it is ordinary layout.
///
/// One definition, read by the retained fill, the border, the wash, the label
/// and the chevron. It was previously written twice — here and in
/// `parts::expander_plan` — which is two places to change a strip that has to
/// stay one strip.
pub(crate) fn expander_header_h() -> f32 {
    theme::ROW_H + theme::SPACE_8
}

/// The header label's box: leading inset to the chevron column.
pub(crate) fn expander_label_box(node: &Node) -> Rect {
    let right = (node.rect.w - theme::SPACE_32).max(theme::SPACE_12);
    Rect::new(theme::SPACE_12, 0.0, right, expander_header_h())
}

/// The chevron's box at the header's trailing edge.
pub(crate) fn expander_chevron_box(node: &Node) -> Rect {
    Rect::new(
        (node.rect.w - theme::SPACE_32).max(0.0),
        0.0,
        (node.rect.w - theme::SPACE_8).max(0.0),
        expander_header_h(),
    )
}

/// The chevron's type size, and the two glyphs it alternates between.
pub(crate) const EXPANDER_CHEVRON_SIZE: f32 = 10.0;
/// The header label's weight — heavier than body text, and carried from birth
/// (`default_font_weight`) so `.font_size(..)` / `.bold()` still reach it.
pub(crate) const EXPANDER_HEADER_WEIGHT: u16 = 600;
pub(crate) const EXPANDER_GLYPH_COLLAPSED: u32 = GLYPH_CHEVRON_RIGHT;
pub(crate) const EXPANDER_GLYPH_EXPANDED: u32 = GLYPH_CHEVRON_DOWN;

#[cfg(test)]
mod tests {
    use super::{caret_box, editor::TextBand, resolve_radius};
    use crate::backend::ControlKind;
    use crate::PILL_RADIUS;

    // ── Caret geometry ───────────────────────────────────────────────────────
    //
    // `caret_box` is the whole of the caret's arithmetic, and none of it needs a
    // device. What these pin is not "the numbers are these numbers" but the four
    // properties a caret is wrong without: it lands on the pixel grid, it
    // straddles the insertion point, it stays inside the clip, and no input can
    // make it uncompositable.

    /// A 200×32 left-aligned TextBox band, scrolled to the origin.
    fn band() -> TextBand {
        TextBand::compute(ControlKind::TextBox, 200.0, 32.0, 14.0, Some(19.0), 0.0)
    }

    /// Every edge on a whole physical pixel, at every scale a display reports.
    /// A fractional edge is what makes a 1-DIP bar render as two dim columns.
    #[test]
    fn every_edge_lands_on_the_physical_pixel_grid() {
        let b = band();
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 3.0] {
            // A deliberately awkward sub-pixel caret position.
            let r = caret_box(&b, 51.337, 6.5, 19.0, 1.0, scale);
            for (name, v) in [("x", r.left), ("y", r.top), ("w", r.width()), ("h", r.height())] {
                let px = v * scale;
                assert!(
                    (px - px.round()).abs() < 1e-3,
                    "{name}={v} is {px} physical px at scale {scale} — not a whole pixel"
                );
            }
        }
    }

    /// Centred on the insertion point, as PadWrite's `GetCaretRect` does. At the
    /// default thickness this is half a pixel; at an accessibility thickness it
    /// is the difference between marking the boundary and covering the glyph
    /// after it.
    #[test]
    fn the_bar_straddles_the_insertion_point() {
        let b = band();
        for width in [1.0, 2.0, 5.0, 10.0] {
            let r = caret_box(&b, 80.0, 6.0, 19.0, width, 1.0);
            let centre = r.left + r.width() / 2.0;
            assert!(
                (centre - 80.0).abs() <= 0.5,
                "a {width}-DIP caret centred at {centre}, not on the insertion point 80"
            );
        }
    }

    /// The sprite is a sibling of the text host, not a child, so it inherits no
    /// clip — this clamp is the only thing keeping a caret at the end of a full
    /// field off the border and the spin column.
    #[test]
    fn the_bar_stays_inside_the_content_column() {
        let b = band();
        let lo = b.content_x;
        let hi = b.content_x + b.content_w;
        for x in [-500.0, -1.0, 0.0, 95.0, 1000.0] {
            let r = caret_box(&b, x, 6.0, 19.0, 3.0, 1.5);
            assert!(r.left >= lo - 0.01, "caret at x={x} escaped left ({} < {lo})", r.left);
            assert!(r.right <= hi + 0.01, "caret at x={x} escaped right ({} > {hi})", r.right);
        }
    }

    /// A NaN reaching a visual's Offset or Size does not misplace the caret —
    /// it stops the caret compositing at all, silently. Nothing may propagate.
    #[test]
    fn no_input_can_produce_a_non_finite_or_empty_box() {
        let b = band();
        let bad = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0];
        for v in bad {
            for r in [
                caret_box(&b, v, 6.0, 19.0, 1.0, 1.0),
                caret_box(&b, 50.0, v, 19.0, 1.0, 1.0),
                caret_box(&b, 50.0, 6.0, v, 1.0, 1.0),
                caret_box(&b, 50.0, 6.0, 19.0, v, 1.0),
                caret_box(&b, 50.0, 6.0, 19.0, 1.0, v),
            ] {
                assert!(
                    r.left.is_finite() && r.top.is_finite(),
                    "{v} produced a non-finite origin: {r:?}"
                );
                assert!(
                    r.width() > 0.0 && r.height() > 0.0,
                    "{v} produced an invisible caret: {r:?}"
                );
            }
        }
    }

    /// The thickness setting must actually reach the pixels — this is the whole
    /// point of reading `SPI_GETCARETWIDTH`, and a caret that ignores it is
    /// invisible to the user who turned it up because they could not see it.
    #[test]
    fn the_thickness_setting_widens_the_bar() {
        let b = band();
        let w1 = caret_box(&b, 50.0, 6.0, 19.0, 1.0, 1.0).width();
        let w5 = caret_box(&b, 50.0, 6.0, 19.0, 5.0, 1.0).width();
        assert_eq!(w1, 1.0);
        assert_eq!(w5, 5.0);
        // And it scales with the display, rather than shrinking as density rises.
        assert_eq!(caret_box(&b, 50.0, 6.0, 19.0, 5.0, 2.0).width(), 5.0);
    }

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
