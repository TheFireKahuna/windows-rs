//! Drawn caption buttons (minimize / maximize-restore / close) + the shared
//! non-client state for the extended-frame window.
//!
//! The dcomp host removes the native caption in `WM_NCCALCSIZE`, so the OS
//! draws no title bar at all: the reactor `TitleBar` control's band is the
//! caption. The three window buttons sit at its trailing edge (the node's
//! default style reserves their width as right padding) as retained sprites,
//! and are hit-tested by the host's `WM_NCHITTEST` as
//! `HTMINBUTTON` / `HTMAXBUTTON` / `HTCLOSE` — which is also what makes Win11
//! snap layouts appear on max-button hover. Hover/pressed state arrives from
//! `WM_NCMOUSEMOVE`/`WM_NCLBUTTON*` (non-client messages), so it lives here as
//! UI-thread state shared between the wndproc and the paint, not in the input
//! pipeline.

use std::cell::Cell;

use windows_canvas::{Rect, TextFormat, TextLayout, Trimming};

use super::node::{Extras, Node};
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

/// Segoe Fluent Icons glyphs. Held as `&str`, not `char`, because they are
/// shaped straight into cached runs — a `char` would have to be `to_string()`d
/// into a fresh `String` to get there.
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
/// read by the sprite sync, which only runs on a dirty reconcile. Nothing here
/// is constructed per frame: a sync reuses these `IDWriteTextLayout`s and at
/// most re-points a max width, which is a property set, not an allocation. This
/// mirrors [`Node::text_layout`](super::node::Node) for TextBlock/Button, but
/// needs its own home because a caption carries *several* independent runs.
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
    /// The band's five icon runs: the leading back chevron, then the window
    /// cluster indexed by [`glyph_slot`].
    ///
    /// Maximize and restore are BOTH shaped, and the state picks between them at
    /// placement. They have to be: the maximized flag is thread-local window
    /// state rather than a prop, so it never sets `text_dirty` and the layout
    /// pass never re-runs on it — a single slot reshaped on demand would put a
    /// DirectWrite build on the path a window snap takes. Same argument, and the
    /// same shape, as a ToggleSwitch keeping both of its state labels.
    pub glyphs: [Option<TextLayout>; GLYPH_N],
}

/// Indices into [`CaptionText::glyphs`].
pub(crate) mod glyph_slot {
    pub const BACK: usize = 0;
    pub const MINIMIZE: usize = 1;
    pub const MAXIMIZE: usize = 2;
    pub const RESTORE: usize = 3;
    pub const CLOSE: usize = 4;
}
pub(crate) const GLYPH_N: usize = 5;

/// Which glyph slot window-button `i` shows, given the maximized state.
pub(crate) fn window_glyph_slot(i: i32, maximized: bool) -> usize {
    match i {
        0 => glyph_slot::MINIMIZE,
        1 if maximized => glyph_slot::RESTORE,
        1 => glyph_slot::MAXIMIZE,
        _ => glyph_slot::CLOSE,
    }
}

/// Where the caption's two text runs go, and how wide each may be before its
/// trimming sign takes over.
///
/// The two are **coupled**: the subtitle begins after whatever width the title
/// was clamped to, and gets only the room left over. That used to be computed as
/// a side effect of drawing the title — `pen` advanced mid-draw — so the
/// subtitle's position could not be established without drawing the title first,
/// and neither could be established at all without a device. Pulling it out
/// makes the coupling a value, which is the only form of it a test can hold.
pub(crate) struct TitlePlacement {
    /// `(x, max_width)` for the title; `None` when the band carries none.
    pub title: Option<(f32, f32)>,
    /// The same for the subtitle, `None` when it is absent or has no room left.
    pub subtitle: Option<(f32, f32)>,
    /// Top of the shared line box — both runs sit on one baseline.
    pub y: f32,
}

/// Resolve [`TitlePlacement`] for a band of `rect` whose back button occupies
/// `back_w`. `None` when the band has no horizontal room for text at all.
///
/// `content_left` is where the Content slot actually landed, in the same space
/// as `rect` — i.e. the far edge of the title's own grid track, read back from
/// layout rather than re-derived. It is what makes the drawn title agree with
/// the track that was sized for it: when the band compresses that track, the
/// title has to ellipsize into the compressed width, exactly as an XAML
/// TextBlock clipped to its column does. Without it a squeezed title would
/// simply draw over the app's content.
///
/// `None` when the band hosts no Content, and then the title may use everything
/// up to the window-button cluster — which is the one case where it should
/// *grow* into the room nothing else is claiming.
pub(crate) fn title_placement(
    t: &CaptionText,
    back_w: f32,
    rect: Rect,
    content_left: Option<f32>,
) -> Option<TitlePlacement> {
    let x0 = rect.left + back_w;
    let edge = (rect.right - CLUSTER_W - theme::SPACE_12)
        .min(content_left.unwrap_or(f32::INFINITY));
    let avail = (edge - x0).max(0.0);
    if avail <= 0.0 {
        return None;
    }
    let y = rect.top + (rect.height() - t.line_h) / 2.0;
    let mut pen = x0;
    let title = t.title.as_ref().map(|_| {
        let w = t.title_w.min(avail);
        let at = (pen, w);
        pen += w + TITLE_GAP;
        at
    });
    let subtitle = t.subtitle.as_ref().and_then(|_| {
        let left = (x0 + avail - pen).max(0.0);
        (left > 0.0).then(|| (pen, t.subtitle_w.min(left)))
    });
    Some(TitlePlacement { title, subtitle, y })
}

/// Window-button `i`'s box within the band (0 = min, 1 = max, 2 = close).
pub(crate) fn button_rect(i: i32, rect: Rect) -> Rect {
    let bx = rect.right - (3 - i) as f32 * BTN_W;
    Rect::from_xywh(bx, rect.top, BTN_W, rect.height())
}

/// The one hover wash the band shows, as `(box, corner radius, colour)`.
///
/// One wash, not four: [`HOVER`] holds a single index, so at most one of the
/// band's four buttons is lit at a time and a second sprite could only ever be
/// invisible. It moves and re-binds between them instead — which is also what
/// keeps the four reading identically, the property the painted version got by
/// drawing them in one loop.
pub(crate) fn hot_wash(node: &Node, rect: Rect) -> Option<(Rect, f32, crate::Color)> {
    let hot = hover();
    if hot == BACK_INDEX {
        let x = node.extras();
        if !x.back_button_enabled {
            return None;
        }
        let br = back_rect(x, rect)?;
        let a = if pressed() == BACK_INDEX { 0.10 } else { 0.06 };
        return Some((br, theme::RADIUS_SM, theme::w(a)));
    }
    if !(0..3).contains(&hot) {
        return None;
    }
    // Close hovers the system alarm red; the others take a subtle wash. Square,
    // not rounded: the window cluster runs to the band's edges.
    let c = if hot == 2 {
        crate::Color::rgb(0xC4, 0x2B, 0x1C)
    } else {
        theme::w(0.06)
    };
    Some((button_rect(hot, rect), 0.0, c))
}

/// Gap between the title and the subtitle that follows it.
const TITLE_GAP: f32 = theme::SPACE_8;

fn caption_format(weight: u16) -> Option<TextFormat> {
    TextFormat::with_weight(
        "Segoe UI",
        theme::FONT_SIZE_SM,
        windows_canvas::FontWeight(weight as i32),
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

/// (Re)build the caption's cached text: the two title runs, and the band's five
/// icon glyphs.
///
/// Always `Some`. A band carrying neither a title nor a subtitle used to
/// allocate nothing, because its four buttons were painted from glyph literals
/// on the spot; now that those are shaped runs too, a bare drag strip still
/// needs this cache to show its window cluster at all. The title slots simply
/// stay `None`, which is what [`title_placement`] and [`title_block`] already
/// read them as.
pub(crate) fn build_text(x: &Extras) -> Option<Box<CaptionText>> {
    let title = run(&x.title, 600);
    let subtitle = run(&x.subtitle, 400);
    let line_h = title
        .as_ref()
        .map(|t| t.2)
        .or(subtitle.as_ref().map(|s| s.2))
        .unwrap_or(0.0);
    let mut glyphs: [Option<TextLayout>; GLYPH_N] = Default::default();
    glyphs[glyph_slot::BACK] = icon_run(GLYPH_BACK, BACK_GLYPH_SIZE);
    glyphs[glyph_slot::MINIMIZE] = icon_run(GLYPH_MINIMIZE, BTN_GLYPH_SIZE);
    glyphs[glyph_slot::MAXIMIZE] = icon_run(GLYPH_MAXIMIZE, BTN_GLYPH_SIZE);
    glyphs[glyph_slot::RESTORE] = icon_run(GLYPH_RESTORE, BTN_GLYPH_SIZE);
    glyphs[glyph_slot::CLOSE] = icon_run(GLYPH_CLOSE, BTN_GLYPH_SIZE);
    Some(Box::new(CaptionText {
        title_w: title.as_ref().map_or(0.0, |t| t.1),
        subtitle_w: subtitle.as_ref().map_or(0.0, |s| s.1),
        line_h,
        title: title.map(|t| t.0),
        subtitle: subtitle.map(|s| s.0),
        glyphs,
    }))
}

/// Type sizes for the band's icon runs — the values the painted calls passed.
pub(crate) const BTN_GLYPH_SIZE: f32 = 10.0;
pub(crate) const BACK_GLYPH_SIZE: f32 = 12.0;

/// Shape one icon-font glyph at `size`.
fn icon_run(glyph: &str, size: f32) -> Option<TextLayout> {
    let fmt = TextFormat::with_weight(
        theme::FONT_ICON,
        size,
        windows_canvas::FontWeight(400),
    )
    .ok()?;
    TextLayout::new(glyph, &fmt, 100_000.0, 100_000.0).ok()
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
/// This is the title track's DESIRED width — the caller makes it the maximum of
/// a `minmax(0, block)` grid track, never a padding — so it is deliberately
/// unclamped. The track yields it back when the band cannot afford it, and the
/// title ellipsizes into whatever is left.
///
/// It used to be clamped to half the band, and applied as `padding.left`. That
/// could not work and the clamp was not what was wrong with it: a node's border
/// box can never be narrower than its own padding, so a measured title reserved
/// that way was a hard floor on the band's width no matter what it was clamped
/// to. Measured against a 480 DIP window with a long title, the band sat at 543
/// — `pad_left` + block + the cluster reservation — and since the min/max/close
/// cluster is positioned from the band's own right edge, it went off screen.
/// (`min_size.width = 0` does not rescue that: it lets the CONTENT box
/// collapse, not the padding.)
pub(crate) fn title_block(text: Option<&CaptionText>) -> f32 {
    let Some(t) = text else { return 0.0 };
    let mut block = t.title_w;
    if t.subtitle_w > 0.0 {
        block += t.subtitle_w + if t.title_w > 0.0 { TITLE_GAP } else { 0.0 };
    }
    if block <= 0.0 {
        return 0.0;
    }
    block + theme::SPACE_12
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

// ── Retained ────────────────────────────────────────────────────────────────

// The band draws nothing. Its one hover wash is a retained part
// (`parts::caption_plan`) and its six runs — two titles, four button glyphs —
// are glyph sprites (`glyph_text::caption_sync`).
//
// The earlier note here argued the back button must NOT be a retained sprite,
// on the grounds that splitting one band's four buttons across two paint
// mechanisms would make them drift apart. That argument is kept, not discarded:
// all four moved together, and they share a SINGLE wash part precisely so there
// is only one place their appearance can be decided.

#[cfg(test)]
mod tests {
    use super::*;

    /// A band carrying `title` and `subtitle`, laid out `w` wide.
    ///
    /// Uses the real `build_text`, so the widths are DirectWrite's own — the
    /// coupling under test is about how two measured runs share a budget, and
    /// inventing the measurements would test only the arithmetic.
    fn placed(title: &str, subtitle: &str, w: f32) -> (Box<CaptionText>, Option<TitlePlacement>) {
        let x = Extras {
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            ..Extras::DEFAULT
        };
        let t = build_text(&x).expect("build_text always yields a cache");
        let rect = Rect::from_xywh(0.0, 0.0, w, BAND_H);
        let p = title_placement(&t, 0.0, rect, None);
        (t, p)
    }

    /// The subtitle starts after the width the title was CLAMPED to, not after
    /// its natural width.
    ///
    /// This is the whole coupling. While the band is wide enough the two are
    /// indistinguishable — the clamp does nothing — so the case that matters is
    /// the narrow one, where a subtitle placed from the natural width would sit
    /// somewhere past the window buttons with nothing visible under it.
    #[test]
    fn the_subtitle_starts_after_the_title_was_clamped_not_after_its_natural_width() {
        // Wide: nothing clamps, and the subtitle follows the natural width.
        let (t, wide) = placed("A Fairly Long Window Title", "subtitle", 1200.0);
        let wide = wide.expect("a 1200 DIP band has room for text");
        let (tx, tw) = wide.title.expect("a title was set");
        assert_eq!(tw, t.title_w, "nothing should clamp at this width");
        assert_eq!(
            wide.subtitle.expect("a subtitle was set").0,
            tx + tw + TITLE_GAP,
            "the subtitle follows the title by exactly one gap",
        );

        // Narrow: the title clamps, and the subtitle must follow the CLAMP.
        let (t, tight) = placed("A Fairly Long Window Title", "subtitle", 260.0);
        let tight = tight.expect("260 DIP still leaves a text column");
        let (tx, tw) = tight.title.expect("a title was set");
        assert!(tw < t.title_w, "the title must clamp in a 260 DIP band");
        // A `None` is legitimate: the clamped title can consume the whole budget.
        if let Some((sx, _)) = tight.subtitle {
            assert_eq!(
                sx,
                tx + tw + TITLE_GAP,
                "the subtitle must follow the CLAMPED title, not the natural one",
            );
        }
    }

    /// A title that eats the whole budget leaves the subtitle nothing, and a
    /// run with no room is dropped rather than placed at zero width.
    #[test]
    fn a_title_that_fills_the_band_leaves_no_subtitle() {
        let (_, p) = placed(
            "An Extremely Long Window Title That Cannot Possibly Fit In The Band",
            "subtitle",
            230.0,
        );
        let p = p.expect("the band still has a text column");
        assert!(p.title.is_some(), "the title is placed, ellipsized");
        assert!(
            p.subtitle.is_none(),
            "a subtitle with no room left must be dropped, not placed at zero width",
        );
    }

    /// A band too narrow to hold anything past its window cluster places
    /// nothing at all — the early return the paint path had.
    #[test]
    fn a_band_with_no_text_column_places_nothing() {
        let (_, p) = placed("Title", "subtitle", CLUSTER_W);
        assert!(p.is_none(), "no room past the cluster means no placement");
    }

    /// A bare drag strip still gets its window glyphs.
    ///
    /// `build_text` used to return `None` for a band with no title, because the
    /// four buttons were painted from literals on the spot. Now that they are
    /// shaped runs, that early return would have left the window cluster blank.
    #[test]
    fn a_band_with_no_titles_still_shapes_its_buttons() {
        let t = build_text(&Extras::DEFAULT).expect("a bare band still needs its glyphs");
        assert!(t.title.is_none() && t.subtitle.is_none());
        for slot in [
            glyph_slot::BACK,
            glyph_slot::MINIMIZE,
            glyph_slot::MAXIMIZE,
            glyph_slot::RESTORE,
            glyph_slot::CLOSE,
        ] {
            assert!(t.glyphs[slot].is_some(), "glyph slot {slot} must be shaped");
        }
    }

    /// Maximize and restore are both shaped, and the state picks between them.
    ///
    /// The maximized flag never sets `text_dirty` — it is thread-local window
    /// state, not a prop — so a design that shaped only the current one would
    /// leave the wrong glyph on screen after a snap.
    #[test]
    fn the_middle_button_swaps_glyphs_without_reshaping() {
        assert_eq!(window_glyph_slot(1, false), glyph_slot::MAXIMIZE);
        assert_eq!(window_glyph_slot(1, true), glyph_slot::RESTORE);
        // The other two are indifferent to it.
        assert_eq!(window_glyph_slot(0, true), glyph_slot::MINIMIZE);
        assert_eq!(window_glyph_slot(2, true), glyph_slot::CLOSE);
    }

    /// A mounted Content slot is the title's far edge: the title ellipsizes into
    /// the track that was sized for it rather than drawing over the app.
    ///
    /// This is the half of the fix that lives outside layout. The title block is
    /// a `minmax(0, block)` grid TRACK now, so a band too narrow for it hands
    /// some of it back — and the drawn title has to agree with the width the
    /// track actually got, which is exactly where Content begins.
    #[test]
    fn content_takes_precedence_over_the_cluster_as_the_titles_far_edge() {
        let x = Extras {
            title: "A Deliberately Long Window Title".to_string(),
            ..Extras::DEFAULT
        };
        let t = build_text(&x).expect("build_text always yields a cache");
        let rect = Rect::from_xywh(0.0, 0.0, 900.0, BAND_H);

        // No Content: the title may run up to the window-button cluster.
        let free = title_placement(&t, 0.0, rect, None).expect("a 900 DIP band has room");
        let (_, free_w) = free.title.expect("a title was set");

        // Content mounted closer in than the title's own natural width: the
        // title must stop at Content, not at the cluster.
        let edge = free_w / 2.0;
        let bounded = title_placement(&t, 0.0, rect, Some(edge)).expect("still has room");
        let (_, bounded_w) = bounded.title.expect("a title was set");

        assert!(
            bounded_w < free_w,
            "Content must narrow the title ({bounded_w} vs {free_w})",
        );
        assert!(
            bounded_w <= edge,
            "the title must not cross into Content (got {bounded_w}, edge {edge})",
        );
    }
}
