//! The InfoBar band: its severity chrome, the wrapped title + message, and the
//! close button.
//!
//! Everything here is **derived geometry**, the same discipline
//! [`nav`](super::nav) follows: the placement, the pointer hit test
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

use windows_canvas::{FontWeight, Rect, TextFormat, TextLayout};

use super::node::{Extras, Node};
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
    /// [`caption`](super::caption) holds its own that way: it is shaped straight
    /// into a cached run, and a `char` would mean a fresh `String` to do it.
    pub(crate) fn glyph(self) -> &'static str {
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
    pub(crate) fn color(self) -> Color {
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
pub(crate) const GLYPH_CLOSE: &str = "\u{E8BB}";

/// Type sizes for the two icon runs, named because the layout pass shapes them
/// and the sprite sync places them — two consumers, one number each.
pub(crate) const ICON_SIZE: f32 = 16.0;
pub(crate) const CLOSE_SIZE: f32 = 10.0;

/// Where the text column starts.
pub(crate) const TEXT_X: f32 = PAD_X + ICON_W + ICON_GAP;

/// The severity icon's cell, in node-local DIPs.
///
/// Aligned with the FIRST line of the paragraph rather than with the band: in a
/// wrapped bar a vertically centred icon drifts away from the text it labels.
pub(crate) fn icon_cell(node_h: f32) -> Rect {
    Rect::from_xywh(PAD_X, 0.0, ICON_W, MIN_H.min(node_h))
}

/// The paragraph's box, in node-local DIPs, for a band `node_w` x `node_h`
/// whose cached run measured `text_h` tall.
///
/// Centred on the band, which is correct for the single-line case and for a
/// wrapped block alike since [`measure`] sized the band to the block plus
/// padding.
pub(crate) fn text_box(node_w: f32, node_h: f32, text_h: f32, closable: bool) -> Rect {
    Rect::from_xywh(
        TEXT_X,
        (node_h - text_h) / 2.0,
        text_w(node_w, closable),
        text_h,
    )
}

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
pub(crate) fn text_w(node_w: f32, closable: bool) -> f32 {
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
    /// The severity glyph's shaped run.
    ///
    /// Rebuilt with the paragraph rather than on its own, which is why
    /// `Prop::Severity` now marks the bar's text dirty: the glyph IS text here,
    /// and a severity change that only marked the node dirty would have left
    /// the previous status icon shaped and on screen.
    pub icon: Option<TextLayout>,
    /// The close button's glyph, shaped once for the same reason.
    pub close: Option<TextLayout>,
}

impl InfoBarText {
    /// The paragraph, re-pinned to the column it is about to be placed in, with
    /// its measured height.
    ///
    /// [`measure`] leaves the run flowed at whatever width Taffy last probed —
    /// it probes several times per pass, and the last probe is not the final
    /// width — so **anything that places this run must re-pin it first**. The
    /// painted path did exactly that on every repaint; a sprite placement that
    /// skipped it would lay glyphs out for a wrap the band no longer has, and
    /// the error would only show at the widths where the two disagree.
    ///
    /// `None` when there is no room to flow into at all, which is the case the
    /// paint path returned early on.
    pub(crate) fn pinned(&self, node_w: f32, closable: bool) -> Option<(&TextLayout, f32)> {
        let run = self.run.as_ref()?;
        let column = text_w(node_w, closable);
        if column <= 0.0 {
            return None;
        }
        let _ = run.set_max_width(column);
        let h = run.measure().map_or(0.0, |(_, h)| h);
        Some((run, h))
    }
}

/// Separator between the title and the message when the bar carries both.
/// Two spaces: the inline gap WinUI gets from a `StackPanel` spacing, expressed
/// where it has to live for the pair to remain one wrappable paragraph.
const TITLE_GAP: &str = "  ";

fn bar_format() -> Option<TextFormat> {
    TextFormat::with_weight("Segoe UI", theme::FONT_SIZE_MD, FontWeight(400)).ok()
}

/// (Re)build the bar's cached text: the paragraph, the severity glyph, and the
/// close glyph.
///
/// Always `Some`. A bar carrying neither a title nor a message used to allocate
/// nothing here, because the icon it still shows was painted from the severity
/// on the spot; now that the icon is a shaped run too, a bar used as a bare
/// coloured strip needs this cache to have anything to show at all. The
/// paragraph itself stays `None` in that case, which is what [`measure`] and
/// [`accessible_name`] already read it as.
pub(crate) fn build_text(x: &Extras) -> Option<Box<InfoBarText>> {
    let (title, message) = (x.title.as_str(), x.message.as_str());
    let plain = if title.is_empty() && message.is_empty() {
        String::new()
    } else if title.is_empty() {
        message.to_string()
    } else if message.is_empty() {
        title.to_string()
    } else {
        format!("{title}{TITLE_GAP}{message}")
    };

    // An empty paragraph shapes to nothing; keep the slot `None` so `measure`
    // and `accessible_name` take their no-text paths unchanged.
    let run = (!plain.is_empty()).then(bar_format).flatten().and_then(|fmt| {
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
    // The two icon runs. Each is a single glyph in a fixed box, so it is shaped
    // at its natural size and placed centred — the alignment the painted
    // `controls::text` call applied through a TextFormat, which a run placed by
    // hand no longer carries.
    let icon = icon_run(severity(x).glyph(), ICON_SIZE);
    let close = x.bar_closable.then(|| icon_run(GLYPH_CLOSE, CLOSE_SIZE)).flatten();
    Some(Box::new(InfoBarText { run, plain, icon, close }))
}

/// Shape one icon-font glyph at `size`.
fn icon_run(glyph: &str, size: f32) -> Option<TextLayout> {
    let fmt = TextFormat::with_weight(theme::FONT_ICON, size, FontWeight(400)).ok()?;
    TextLayout::new(glyph, &fmt, 100_000.0, 100_000.0).ok()
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
/// — Taffy probes several times per pass — which is why anything that PLACES
/// the run re-pins the column width itself rather than trusting what it finds.
/// [`InfoBarText::pinned`] is that step, and the only supported way to read
/// this layout for placement.
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

// ── Retained ────────────────────────────────────────────────────────────────

// The band draws nothing. Its card, severity tint, border and the close
// button's hover wash are retained parts (`parts::bar_plan`); its paragraph and
// its two icon glyphs are glyph sprites (`glyph_text::info_bar_sync`).
//
// The earlier note here argued the opposite — that sprites are for what MOVES,
// and nothing on this control does — which was true of MOTION and beside the
// point for cost. Retained chrome is also what keeps a state change off the
// raster path: hovering the close button now fades one part's opacity on the
// compositor instead of redrawing a card, an icon and a wrapped paragraph, and
// the paragraph is the largest run in the library.

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
