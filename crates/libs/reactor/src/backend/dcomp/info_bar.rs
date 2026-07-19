//! The InfoBar band: its severity chrome, the wrapped title + message, and the
//! drawn close button.
//!
//! Everything here is **derived geometry**, the same discipline
//! [`nav`](super::nav) follows: the paint, the pointer hit test
//! ([`input`](super::input)) and the accessibility tree ([`uia`](super::uia))
//! all read one set of rects rather than each re-deriving them from `Extras`.
//! A hit test that disagreed with the paint by a few DIPs would dismiss a bar
//! the user never clicked.
//!
//! # One layout, not two
//!
//! WinUI's InfoBar flows the title and the message **inline** — "Title
//! Message…" on one line while they fit, wrapping to further lines when they
//! do not — with the title in a heavier weight. That is a single paragraph
//! with mixed emphasis, so it is laid out here as a single
//! [`TextLayout`] whose title span is re-weighted through
//! [`TextLayout::set_font_weight`]. Two separate layouts could not reproduce
//! it: each would wrap inside its own box, and flowing the message after the
//! title would mean re-implementing line breaking above DirectWrite.
//!
//! The consequence for the layout pass is that the bar's height is a function
//! of its width, which is why [`measure`] exists and why `InfoBar` gets its own
//! arm in the Taffy measure callback.

use windows_canvas_core::{
    Brush, DrawingSession, FontWeight, ParagraphAlignment, Rect, TextAlignment, TextFormat,
    TextLayout, Vector2,
};

use super::node::{linear, Extras, Node};
use super::theme;
use crate::Color;

// ── Severity ─────────────────────────────────────────────────────────────────

/// The bar's severity, resolved from the WinRT `InfoBarSeverity` discriminant.
///
/// An unrecognised value reads as [`Self::Informational`] rather than being
/// dropped: the seam carries the enum as a plain `i32`, and a bar drawn in the
/// neutral style still says what it has to say.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Severity {
    Informational,
    Success,
    Warning,
    Error,
}

impl Severity {
    pub(crate) fn of(raw: i32) -> Self {
        match raw {
            1 => Self::Success,
            2 => Self::Warning,
            3 => Self::Error,
            _ => Self::Informational,
        }
    }

    /// The status glyph (Segoe Fluent Icons), held as `&str` for the reason
    /// [`caption`](super::caption) holds its own that way: it goes straight to
    /// `draw_text`, and a `char` would mean a fresh `String` per repaint.
    fn glyph(self) -> &'static str {
        match self {
            Self::Informational => "\u{E946}", // Info
            Self::Success => "\u{E930}",       // Completed
            Self::Warning => "\u{E7BA}",       // Warning
            Self::Error => "\u{EA39}",         // ErrorBadge
        }
    }

    /// The role colour the icon takes and the background tint derives from.
    /// Straight off the Fluent status roles, so a host token table restyles
    /// every severity with the rest of the control library.
    fn color(self) -> Color {
        match self {
            Self::Informational => theme::accent(),
            Self::Success => theme::ok(),
            Self::Warning => theme::warn(),
            Self::Error => theme::danger(),
        }
    }

    /// The name a screen reader hears before the bar's own text. WinUI relies
    /// on the icon alone, which is exactly the part a non-visual client cannot
    /// see — so the role is said out loud here instead.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Informational => "Information",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
}

/// This node's severity.
pub(crate) fn severity(x: &Extras) -> Severity {
    Severity::of(x.severity)
}

// ── Geometry ─────────────────────────────────────────────────────────────────

/// `InfoBarMinHeight` — the single-line band.
pub(crate) const MIN_H: f32 = 40.0;
/// Leading inset to the status icon.
const PAD_X: f32 = theme::SPACE_16;
/// Vertical breathing room above/below the text block.
const PAD_Y: f32 = theme::SPACE_12;
/// The status icon's column.
const ICON_W: f32 = 20.0;
/// Gap between the icon column and the text.
const ICON_GAP: f32 = theme::SPACE_12;
/// The drawn close button — a square target, like the pane's chrome buttons.
const CLOSE_D: f32 = theme::ROW_H;
/// Trailing inset from the band edge to that button.
const CLOSE_PAD: f32 = theme::SPACE_8;

/// `ChromeClose` — the same glyph the caption cluster's close button draws, so
/// the two dismiss affordances in one window read identically.
const GLYPH_CLOSE: &str = "\u{E8BB}";

/// Where the text column starts.
const TEXT_X: f32 = PAD_X + ICON_W + ICON_GAP;

/// Everything the band spends on chrome rather than text: the icon column and
/// its gap, plus whatever the trailing edge reserves.
///
/// The **single definition** of that budget — [`measure`], [`paint`] and the
/// close button's rect all derive from it, so the width the text is measured
/// at and the width it is drawn at cannot drift.
fn chrome_w(closable: bool) -> f32 {
    TEXT_X + if closable { CLOSE_D + CLOSE_PAD } else { PAD_X }
}

/// The text column's width inside a band `node_w` wide.
fn text_w(node_w: f32, closable: bool) -> f32 {
    (node_w - chrome_w(closable)).max(0.0)
}

/// The drawn close button's box in node-local DIPs, or `None` when the bar is
/// not closable (or is too narrow to hold the button at all).
pub(crate) fn close_rect(node_w: f32, node_h: f32, closable: bool) -> Option<Rect> {
    if !closable {
        return None;
    }
    let left = node_w - CLOSE_PAD - CLOSE_D;
    if left <= 0.0 {
        return None;
    }
    // Vertically centred rather than band-filling: in a tall (wrapped) bar a
    // full-height button reads as a column, not a button — the same call the
    // caption band's back button makes.
    let h = CLOSE_D.min(node_h);
    let top = (node_h - h) / 2.0;
    Some(Rect::from_xywh(left, top, CLOSE_D, h))
}

/// `hot_index` value the close button occupies while the pointer rests on it.
/// Negative, mirroring [`nav::HOT_BACK`](super::nav::HOT_BACK), so it can never
/// be mistaken for an item index and -1 keeps its "nothing hot" meaning.
pub(crate) const HOT_CLOSE: i32 = -2;

/// Whether a node-local point lands on the close button.
pub(crate) fn hit_close(node: &Node, lx: f32, ly: f32) -> bool {
    let Some(r) = close_rect(node.rect.w, node.rect.h, node.extras().bar_closable) else {
        return false;
    };
    // Half-open, matching `LaidRect::contains`.
    lx >= r.left && lx < r.right && ly >= r.top && ly < r.bottom
}

// ── Cached text ──────────────────────────────────────────────────────────────

/// The bar's own drawn text, laid out once and reused.
///
/// Built by the layout pass whenever the InfoBar's `text_dirty` is set — i.e.
/// exactly when the title or the message changed — and read by [`measure`] and
/// [`paint`]. Nothing here is constructed per frame: both reuse this one
/// `IDWriteTextLayout` and at most re-point its max width, which is a property
/// set, not an allocation.
///
/// The same mechanism [`CaptionText`](super::caption::CaptionText) and
/// [`NavPaneText`](super::nav::NavPaneText) use, and it needs its own home for
/// the same reason: `Node::text_layout` is the generic single-run slot, and its
/// wrap pin (`layout::solve_walk`) would re-flow this run to the node's FULL
/// width — past the icon and close columns the text must stay clear of.
pub(crate) struct InfoBarText {
    /// The combined "title  message" paragraph, or `None` if DirectWrite
    /// refused the run.
    ///
    /// Its box is shared mutable COM state that both [`measure`] and [`paint`]
    /// re-pin, so neither may assume the width it finds — see `measure`.
    pub run: Option<TextLayout>,
    /// The plain string the layout carries, reused as the accessible name so
    /// what a screen reader announces and what the eye reads cannot diverge.
    pub plain: String,
}

/// Separator between the title and the message when the bar carries both.
/// Two spaces: the inline gap WinUI gets from a `StackPanel` spacing, expressed
/// where it has to live for the pair to remain one wrappable paragraph.
const TITLE_GAP: &str = "  ";

fn bar_format() -> Option<TextFormat> {
    TextFormat::with_weight("Segoe UI", theme::FONT_SIZE_MD, FontWeight(400)).ok()
}

/// (Re)build the bar's cached text. `None` when it carries neither a title nor
/// a message, so a bar used purely as a coloured strip allocates nothing.
pub(crate) fn build_text(x: &Extras) -> Option<Box<InfoBarText>> {
    let (title, message) = (x.title.as_str(), x.message.as_str());
    if title.is_empty() && message.is_empty() {
        return None;
    }
    let plain = if title.is_empty() {
        message.to_string()
    } else if message.is_empty() {
        title.to_string()
    } else {
        format!("{title}{TITLE_GAP}{message}")
    };

    let run = bar_format().and_then(|fmt| {
        // A generous construction box, i.e. the paragraph's max-content state,
        // exactly as `layout::build_text_layout` builds the generic runs;
        // `measure` re-flows it against the real column width.
        let layout = TextLayout::new(&plain, &fmt, 100_000.0, 100_000.0).ok()?;
        let _ = layout.set_word_wrap(true);
        // The title span carries the heavier weight. Measured in UTF-16 code
        // units because that is what a DirectWrite text range indexes — a
        // `char` count would mis-span the moment a title held an emoji or any
        // other astral-plane character.
        if !title.is_empty() {
            let units = title.encode_utf16().count() as u32;
            let _ = layout.set_font_weight(FontWeight(600), 0, units);
        }
        Some(layout)
    });
    Some(Box::new(InfoBarText { run, plain }))
}

// ── Measure ──────────────────────────────────────────────────────────────────

/// The bar's intrinsic `(width, height)` for a given available width — the
/// Taffy measure callback's answer for an `InfoBar`.
///
/// `avail_w` is `None` for the max-content probe (the paragraph on one line),
/// `Some(0.0)` for min-content, and `Some(w)` for a real constraint, where the
/// paragraph re-flows into the text column and the band grows to hold however
/// many lines that took.
///
/// **Both** returned axes come from the re-flowed measure, never from the
/// paragraph's natural width. Reporting the natural width for the min-content
/// probe is what a first cut did, and it makes the bar unshrinkable: a flex
/// item's automatic minimum size IS its min-content size, so a long message
/// forced every ancestor as wide as its one unwrapped line and the band ran off
/// the window instead of wrapping. Re-flowing into a zero-width column gives
/// the longest single word, which is min-content's actual definition — the same
/// mapping the generic wrapping-run path makes.
///
/// This LEAVES the layout re-flowed at whatever width it was last asked about
/// — Taffy probes several times per pass — which is why [`paint`] re-pins the
/// column width itself rather than trusting what it finds.
pub(crate) fn measure(node: &Node, avail_w: Option<f32>) -> (f32, f32) {
    let closable = node.extras().bar_closable;
    let chrome = chrome_w(closable);
    let Some(t) = node.bar_text.as_deref() else {
        return (chrome, MIN_H);
    };
    let Some(run) = &t.run else {
        return (chrome, MIN_H);
    };
    let column = match avail_w {
        Some(w) => text_w(w, closable),
        None => f32::INFINITY,
    };
    let _ = run.set_max_width(column);
    let (tw, th) = run.measure().unwrap_or((0.0, 0.0));
    (chrome + tw, (th + 2.0 * PAD_Y).max(MIN_H))
}

// ── Paint ────────────────────────────────────────────────────────────────────

/// Paint the band onto the InfoBar node's surface: the tinted card, the
/// severity icon, the paragraph, and the close button.
///
/// Nothing here is a retained compositor sprite. That is the deliberate
/// application of the rule the rest of the backend follows — sprites are for
/// what MOVES — and on this control nothing does: the card, the icon and the
/// text are simply *there* at a given state, and the close button's hover wash
/// is a flat state fill with no animation, so it repaints on the hover edge
/// exactly as the caption band's and the nav pane's chrome buttons do.
pub(crate) fn paint(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let x = node.extras();
    let sev = severity(x);
    let radius = theme::RADIUS_SM;

    // The card: a raised surface carrying a wash of the severity role, so the
    // bar reads as its status at a glance without the text having to say so.
    super::controls::fill_rr(session, brush, rect, radius, theme::surface_raised(), dim);
    super::controls::fill_rr(
        session,
        brush,
        rect,
        radius,
        theme::with_alpha(sev.color(), 0.10),
        dim,
    );
    super::controls::stroke_rr(
        session,
        brush,
        rect,
        radius,
        theme::stroke(),
        theme::BORDER_W,
        dim,
    );

    paint_icon(session, brush, sev, rect, dim);
    paint_text(session, brush, node, rect, dim);
    paint_close(session, brush, node, rect, dim);
}

/// The severity glyph, centred in its column and aligned with the FIRST line of
/// the paragraph rather than with the band — in a wrapped bar a vertically
/// centred icon drifts away from the text it is labelling.
fn paint_icon(session: &DrawingSession, brush: &Brush, sev: Severity, rect: Rect, dim: f32) {
    let h = MIN_H.min(rect.height());
    let cell = Rect::from_xywh(rect.left + PAD_X, rect.top, ICON_W, h);
    super::controls::text(
        session,
        brush,
        sev.glyph(),
        cell,
        theme::FONT_ICON,
        16.0,
        400,
        sev.color(),
        TextAlignment::Center,
        ParagraphAlignment::Center,
        dim,
    );
}

/// The title + message paragraph, from the cached layout. The only per-repaint
/// work is re-pinning it to the current text column, which is what makes it
/// re-wrap as the window resizes.
fn paint_text(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let Some(t) = node.bar_text.as_deref() else {
        return;
    };
    let Some(run) = &t.run else { return };
    let column = text_w(rect.width(), node.extras().bar_closable);
    if column <= 0.0 {
        return;
    }
    let _ = run.set_max_width(column);
    let h = run.measure().map(|(_, h)| h).unwrap_or(0.0);
    // Centred on the band: correct for the single-line case and for a wrapped
    // block alike, since `measure` sized the band to the block plus padding.
    let y = rect.top + (rect.height() - h) / 2.0;
    let mut c = linear(node.paint.foreground.unwrap_or_else(theme::text));
    c.a *= dim;
    brush.set_color(c);
    session.draw_text_layout(
        Vector2 {
            x: rect.left + TEXT_X,
            y,
        },
        run,
        brush,
    );
}

/// The close button: a hover/press wash plus the glyph.
fn paint_close(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let Some(r) = close_rect(rect.width(), rect.height(), node.extras().bar_closable) else {
        return;
    };
    let r = Rect::from_xywh(rect.left + r.left, rect.top + r.top, r.width(), r.height());
    if node.paint.is_enabled && node.ctrl().hot_index == HOT_CLOSE {
        let wash = if node.pressed { 0.10 } else { 0.06 };
        super::controls::fill_rr(session, brush, r, theme::RADIUS_SM, theme::w(wash), dim);
    }
    super::controls::text(
        session,
        brush,
        GLYPH_CLOSE,
        r,
        theme::FONT_ICON,
        10.0,
        400,
        theme::text_secondary(),
        TextAlignment::Center,
        ParagraphAlignment::Center,
        dim,
    );
}

/// The accessible name for the whole band: the severity role followed by the
/// text as drawn, so a screen reader conveys what the icon shows visually.
pub(crate) fn accessible_name(node: &Node) -> String {
    let role = severity(node.extras()).label();
    match node.bar_text.as_deref() {
        Some(t) if !t.plain.is_empty() => format!("{role}. {}", t.plain),
        _ => role.to_string(),
    }
}

/// The name the drawn close button announces — the same string the caption
/// cluster's close button uses, for the same action.
pub(crate) const CLOSE_LABEL: &str = "Close";
