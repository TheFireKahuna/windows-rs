//! The NavigationView pane: how wide it is, what sits in it, and the static
//! half of its chrome.
//!
//! The backend used to draw this control as an icon rail and nothing else — a
//! fixed 48-DIP strip of glyphs. The pane state (`IsPaneOpen`, `PaneTitle`,
//! `OpenPaneLength`, `PaneDisplayMode`, the settings row, the back arrow, the
//! hamburger) was stored on [`Extras`] and read by no one. This module is the
//! reader.
//!
//! Everything here is **derived geometry**: one [`Metrics`] value resolved from
//! the pane state plus the node's own width answers every question the paint,
//! the retained sprites ([`parts::nav_sync`](super::parts)), the hit test
//! ([`input`](super::input)) and the accessibility tree ([`uia`](super::uia))
//! ask. They must agree exactly — a hit test that disagrees with the paint by
//! one row selects the wrong page — so they share this one definition rather
//! than each re-deriving from `Extras`.
//!
//! The split between what is painted here and what is a retained compositor
//! sprite follows the rule the rest of the backend uses: anything that MOVES
//! (the pane's width transition, the selection tile and its accent bar, the row
//! hover ink) is a sprite whose motion runs DWM-side; anything that is simply
//! *there* at a given state (glyphs, labels, the divider) paints onto the
//! node's own surface, once per dirty repaint.

use windows_canvas_core::{
    Brush, DrawingSession, ParagraphAlignment, Rect, TextAlignment, TextFormat, TextLayout,
    Trimming, Vector2,
};

use super::node::{linear, Extras, Node};
use super::theme;
use crate::NavigationViewPaneDisplayMode as Mode;

/// Per-item row height, and the compact rail's width — the rail is a column of
/// squares, so one constant is both.
pub(crate) const ITEM_H: f32 = theme::NAV_RAIL_W;

/// The back / hamburger row at the head of the pane. A 40-DIP target (the Win11
/// caption metric, matching the drawn TitleBar back button) rather than a full
/// `ITEM_H` square, so the chrome row reads as buttons and the item list below
/// it reads as a list.
pub(crate) const CHROME_W: f32 = 40.0;
pub(crate) const CHROME_H: f32 = 40.0;

/// Header row height for the pane title (expanded panes only).
const TITLE_H: f32 = theme::ROW_H;

/// Leading inset for a label beside its icon: the icon occupies the rail
/// column, the label starts after it.
const LABEL_X: f32 = theme::NAV_RAIL_W;
/// Trailing inset so a label never runs into the pane's divider.
const LABEL_PAD_R: f32 = theme::SPACE_12;

/// Segoe Fluent Icons glyphs for the pane's own chrome. Held as `&str` for the
/// reason [`caption`](super::caption) holds its own that way: they go straight
/// to `draw_text`, and a `char` would mean a fresh `String` per repaint.
const GLYPH_BACK: &str = "\u{E72B}";
/// `GlobalNavButton` — the hamburger.
const GLYPH_TOGGLE: &str = "\u{E700}";
const GLYPH_SETTINGS: &str = "\u{E713}";

/// The label the settings row carries. WinUI's own `NavigationView` uses the
/// localized system string; this backend ships no resource table, so the
/// English default is stated once here rather than at each use.
const SETTINGS_LABEL: &str = "Settings";

/// The tag a settings selection reports through `SelectionChanged`. The
/// NavigationView seam carries selections BY TAG (`on_selection_changed` takes
/// a `String`), and WinUI's settings item has no app-supplied tag of its own —
/// so this is the name the two sides agree on for "the settings row", stated
/// once here rather than spelled at the fire site.
pub(crate) const SETTINGS_TAG: &str = "settings";

/// WinUI's adaptive thresholds for [`Mode::Auto`]
/// (`ExpandedModeThresholdWidth` / `CompactModeThresholdWidth`).
const EXPANDED_THRESHOLD: f32 = 1008.0;
const COMPACT_THRESHOLD: f32 = 641.0;

/// How the pane is presented, after the display mode, the open flag and the
/// available width have all been taken into account.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PaneKind {
    /// Full pane: labels beside icons, a header, a settings row with its label.
    Expanded,
    /// The icon rail: glyphs only, at `NAV_RAIL_W`.
    Compact,
    /// Collapsed to the hamburger strip alone — no items, no rail.
    MinimalClosed,
    /// A minimal pane opened back up. Presented like [`Self::Expanded`]; it is
    /// a distinct state only because closing it returns to the strip rather
    /// than to the rail.
    MinimalOpen,
}

impl PaneKind {
    /// Whether labels are drawn beside the icons.
    pub(crate) fn expanded(self) -> bool {
        matches!(self, Self::Expanded | Self::MinimalOpen)
    }
}

/// Resolve the presentation from the pane state and the width the node has to
/// work with.
///
/// `node_w` is the node's laid-out width from the PREVIOUS pass (0 before the
/// first). That is deliberate and mirrors what the caption band does for its
/// title inset: the adaptive thresholds are a function of the width, and the
/// width is a function of the parent, not of this node's padding — so reading
/// last pass's value converges immediately and never feeds back.
///
/// Note on [`Mode::Top`]: a top-mounted pane is a different layout axis (a
/// horizontal band above the content, not a column beside it) and this backend
/// does not build one. It resolves as a left pane rather than as nothing, so an
/// app that asks for `Top` still gets a usable, navigable control instead of an
/// invisible one — see the contract entry for `PaneDisplayMode`.
pub(crate) fn resolve(x: &Extras, node_w: f32) -> PaneKind {
    let mode = Mode(x.pane_display_mode);
    // `Auto` picks the mode a fixed one would have named.
    let effective = if mode == Mode::Auto {
        if node_w >= EXPANDED_THRESHOLD {
            Mode::Left
        } else if node_w >= COMPACT_THRESHOLD {
            Mode::LeftCompact
        } else {
            Mode::LeftMinimal
        }
    } else {
        mode
    };
    match effective {
        Mode::LeftMinimal if x.pane_open => PaneKind::MinimalOpen,
        Mode::LeftMinimal => PaneKind::MinimalClosed,
        // `Left`, `LeftCompact` and the `Top` fallback all collapse to the rail
        // when closed and open to the full pane when not.
        _ if x.pane_open => PaneKind::Expanded,
        _ => PaneKind::Compact,
    }
}

/// The pane's resolved geometry — the single answer every consumer reads.
#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    pub kind: PaneKind,
    /// The pane's width: both what it draws across and what it reserves in
    /// layout. The two are the same number on purpose — see [`pane_width`].
    pub width: f32,
    /// Top of the first item row (below the chrome row and any header).
    pub items_y: f32,
    /// Whether the back arrow / hamburger occupy the chrome row.
    pub back: bool,
    pub toggle: bool,
    /// Whether a settings row is pinned to the foot of the pane.
    pub settings: bool,
}

/// The pane's width for a given state — the SINGLE definition of that geometry,
/// exactly as [`caption::pad_left`](super::caption::pad_left) is for the band's
/// leading inset, and for the same reason: `birth_style` builds a virgin
/// NavigationView with `pane_width(&Extras::DEFAULT, 0.0)` and the layout pass
/// re-derives it from the node's live `Extras`. Two expressions of it would
/// drift, and the drift would break the reset invariant — an `Unset` that
/// re-derived a different width than the node was born with leaves a node that
/// is distinguishable from one that never received the prop.
///
/// The pane always RESERVES what it DRAWS. WinUI's minimal pane overlays the
/// content instead; this backend does not, because the pane paints on the
/// node's own surface and every child visual composites above it — an
/// overlaying pane would be drawn underneath the content it is supposed to
/// cover. Reserving is the honest rendering of the same state, and it is also
/// what keeps the content pane correctly sized at every width.
pub(crate) fn pane_width(x: &Extras, node_w: f32) -> f32 {
    let open = (x.open_pane_length as f32).max(theme::NAV_RAIL_W);
    // Never let the pane eat the whole node: past this the content pane would
    // measure zero and the app's page would vanish rather than merely narrow.
    let open = if node_w > 0.0 {
        open.min((node_w * 0.8).max(theme::NAV_RAIL_W))
    } else {
        open
    };
    match resolve(x, node_w) {
        PaneKind::Expanded | PaneKind::MinimalOpen => open,
        PaneKind::Compact => theme::NAV_RAIL_W,
        PaneKind::MinimalClosed => CHROME_W,
    }
}

/// Resolve the full pane geometry. `has_title` comes from the cached text (a
/// header row is reserved only when there is something to put in it), so the
/// caller passes what it measured rather than this module re-reading the string.
pub(crate) fn metrics(x: &Extras, node_w: f32, has_title: bool) -> Metrics {
    let kind = resolve(x, node_w);
    // A minimal, closed pane is the hamburger and nothing else: no items, no
    // settings, and no back arrow (there is no room beside the toggle).
    let closed = kind == PaneKind::MinimalClosed;
    let toggle = x.pane_toggle_visible;
    let back = x.back_button_visible && !closed;
    let chrome_h = if back || toggle { CHROME_H } else { 0.0 };
    let title_h = if kind.expanded() && has_title && !closed {
        TITLE_H
    } else {
        0.0
    };
    Metrics {
        kind,
        width: pane_width(x, node_w),
        items_y: chrome_h + title_h,
        back,
        toggle,
        settings: x.settings_visible && !closed,
    }
}

/// The back arrow's box in node-local DIPs, or `None` when it is hidden.
pub(crate) fn back_rect(m: &Metrics) -> Option<Rect> {
    m.back
        .then(|| Rect::from_xywh(0.0, 0.0, CHROME_W, CHROME_H))
}

/// The hamburger's box, which follows the back arrow when both are present.
pub(crate) fn toggle_rect(m: &Metrics) -> Option<Rect> {
    let x = if m.back { CHROME_W } else { 0.0 };
    m.toggle
        .then(|| Rect::from_xywh(x, 0.0, CHROME_W, CHROME_H))
}

/// One menu item's row. Rows are full-width so the selection wash and the hover
/// ink span the label as well as the icon.
pub(crate) fn item_rect(m: &Metrics, i: i32) -> Rect {
    Rect::from_xywh(0.0, m.items_y + i as f32 * ITEM_H, m.width, ITEM_H)
}

/// The settings row, pinned to the foot of the pane. `None` when hidden, or
/// when the pane is too short to hold the row below its header at all.
///
/// Deliberately NOT gated on the item count: the settings row keeps its place
/// at the foot and the item list gives way, which is what WinUI does and the
/// only stable arrangement — gating it on the items would make a menu one entry
/// too long for the window silently drop the settings row instead of the
/// last page, and would make the two rows' geometry mutually recursive.
pub(crate) fn settings_rect(m: &Metrics, node_h: f32) -> Option<Rect> {
    if !m.settings {
        return None;
    }
    let top = node_h - ITEM_H;
    (top >= m.items_y).then(|| Rect::from_xywh(0.0, top, m.width, ITEM_H))
}

/// How many item rows actually fit between the header and the settings row.
/// Beyond this the pane is simply too short; the surplus items are not drawn,
/// and — because this is the shared definition — not hit-tested or exposed
/// either, so nothing is reachable that is not visible.
pub(crate) fn visible_items(m: &Metrics, node_h: f32, count: usize) -> usize {
    let floor = match settings_rect(m, node_h) {
        Some(r) => r.top,
        None => node_h,
    };
    let room = ((floor - m.items_y) / ITEM_H).floor().max(0.0) as usize;
    count.min(room)
}

/// What a point in the pane lands on. The one hit-test definition shared by the
/// pointer path and the accessibility tree, so the two can never disagree about
/// which row is where.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Hit {
    Back,
    Toggle,
    Item(i32),
    Settings,
}

/// Half-open containment, matching [`LaidRect::contains`](super::node::LaidRect)
/// — a point on the right/bottom edge belongs to the next row, so adjacent rows
/// tile without a one-pixel seam that hits neither.
fn in_rect(r: Rect, x: f32, y: f32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

/// Resolve a node-local point against the pane. `None` when the point is
/// outside the pane entirely (it belongs to the content) or lands on padding.
pub(crate) fn hit(m: &Metrics, node_h: f32, count: usize, lx: f32, ly: f32) -> Option<Hit> {
    if lx < 0.0 || lx >= m.width || ly < 0.0 || ly >= node_h {
        return None;
    }
    if let Some(r) = back_rect(m)
        && in_rect(r, lx, ly)
    {
        return Some(Hit::Back);
    }
    if let Some(r) = toggle_rect(m)
        && in_rect(r, lx, ly)
    {
        return Some(Hit::Toggle);
    }
    if let Some(r) = settings_rect(m, node_h)
        && in_rect(r, lx, ly)
    {
        return Some(Hit::Settings);
    }
    let n = visible_items(m, node_h, count);
    if n == 0 || ly < m.items_y {
        return None;
    }
    let i = ((ly - m.items_y) / ITEM_H).floor() as i32;
    (i >= 0 && (i as usize) < n).then_some(Hit::Item(i))
}

// ── Cached text ──────────────────────────────────────────────────────────────

/// The pane's own drawn text, laid out once and reused.
///
/// Built by the layout pass whenever the NavigationView's `text_dirty` is set —
/// i.e. exactly when the pane title, the items or the settings row changed —
/// and read by [`paint`], which only runs on a dirty repaint. Nothing here is
/// constructed per frame: a repaint reuses these `IDWriteTextLayout`s and at
/// most re-points a max width, which is a property set, not an allocation.
///
/// This is the same mechanism [`CaptionText`](super::caption::CaptionText) uses
/// for the caption band, and it needs its own home for the same reason: a pane
/// carries one run per item plus two of its own, and `Node::text_layout` holds
/// exactly one.
pub(crate) struct NavPaneText {
    pub title: Option<TextLayout>,
    pub title_w: f32,
    /// One laid-out label per menu item, parallel to `Ctrl::items`. An entry is
    /// `None` only if DirectWrite refused the run.
    pub items: Vec<Option<TextLayout>>,
    pub settings: Option<TextLayout>,
    /// Line height of the item runs, for vertical centring. One value for all
    /// of them: they share a format, so they share a line height.
    pub line_h: f32,
}

fn pane_format(size: f32, weight: u16) -> Option<TextFormat> {
    TextFormat::with_weight(
        "Segoe UI",
        size,
        windows_canvas_core::FontWeight(weight as i32),
    )
    .ok()
}

/// Lay out one pane run, ellipsized. Built at its natural width; [`paint`]
/// narrows it to the room actually available, at which point the trimming sign
/// takes over — so a long item label degrades to "Equalizer…" rather than
/// spilling across the divider.
fn run(text: &str, size: f32, weight: u16) -> Option<(TextLayout, f32, f32)> {
    if text.is_empty() {
        return None;
    }
    let fmt = pane_format(size, weight)?;
    let layout = TextLayout::new(text, &fmt, 100_000.0, 100_000.0).ok()?;
    let _ = layout.set_word_wrap(false);
    let _ = layout.set_trimming(Trimming::CharacterEllipsis, &fmt);
    let (w, h) = layout.measure().ok()?;
    Some((layout, w, h))
}

/// (Re)build the pane's cached text. `None` when the pane carries no title, no
/// items and no settings row, so a NavigationView used as a bare rail with
/// glyph-only items allocates nothing at all.
pub(crate) fn build_text(x: &Extras, items: &[String]) -> Option<Box<NavPaneText>> {
    let title = run(&x.pane_title, theme::FONT_SIZE_SM, 600);
    let settings = run(SETTINGS_LABEL, theme::FONT_SIZE_MD, 400);
    if title.is_none() && items.is_empty() && settings.is_none() {
        return None;
    }
    let mut runs = Vec::with_capacity(items.len());
    let mut line_h = 0.0f32;
    for label in items {
        let r = run(label, theme::FONT_SIZE_MD, 400);
        if let Some((_, _, h)) = &r {
            line_h = line_h.max(*h);
        }
        runs.push(r.map(|(l, _, _)| l));
    }
    if line_h <= 0.0 {
        line_h = settings
            .as_ref()
            .map(|s| s.2)
            .or(title.as_ref().map(|t| t.2))
            .unwrap_or(theme::FONT_SIZE_MD * 1.4);
    }
    Some(Box::new(NavPaneText {
        title_w: title.as_ref().map(|t| t.1).unwrap_or(0.0),
        title: title.map(|t| t.0),
        items: runs,
        settings: settings.map(|s| s.0),
        line_h,
    }))
}

// ── Paint ────────────────────────────────────────────────────────────────────

/// Paint the pane's static chrome onto the NavigationView node's surface.
///
/// The pane background, the selection tile and its accent bar, and the row
/// hover ink are retained sprites UNDER this surface
/// ([`parts::nav_sync`](super::parts)) — a selection change or a pane-width
/// change glides them on the compositor. What paints here is everything that
/// does not move: the divider, the chrome glyphs, the header, the per-item
/// icons and labels, and the settings row.
pub(crate) fn paint(session: &DrawingSession, brush: &Brush, node: &Node, rect: Rect, dim: f32) {
    let x = node.extras();
    let text = node.nav_text.as_deref();
    let m = metrics(x, rect.width(), text.is_some_and(|t| t.title.is_some()));

    // The divider between pane and content.
    super::controls::put(brush, theme::stroke_divider(), dim);
    session.draw_line(
        Vector2::new(m.width, 0.0),
        Vector2::new(m.width, rect.height()),
        brush,
        theme::BORDER_W,
    );

    paint_chrome(session, brush, node, &m, dim);
    paint_title(session, brush, &m, text, dim);
    paint_items(session, brush, node, &m, rect, text, dim);
    paint_settings(session, brush, node, &m, rect, text, dim);
}

/// The back arrow and the hamburger. Their hover/press wash is a flat state
/// fill painted here rather than a retained sprite — the same call the drawn
/// caption back button makes, and for the same reason: the wash does not
/// animate, and splitting one row's two buttons across two paint mechanisms
/// would let them drift apart.
fn paint_chrome(
    session: &DrawingSession,
    brush: &Brush,
    node: &Node,
    m: &Metrics,
    dim: f32,
) {
    let hot = node.ctrl().hot_index;
    if let Some(r) = back_rect(m) {
        // A disabled back arrow is still DRAWN (greyed), exactly like the
        // caption band's — hiding it on disable would reflow the pane every
        // time the navigation stack hit depth zero.
        let enabled = node.extras().back_enabled;
        if enabled && hot == HOT_BACK {
            wash(session, brush, r, dim);
        }
        super::controls::text(
            session,
            brush,
            GLYPH_BACK,
            r,
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
            dim,
        );
    }
    if let Some(r) = toggle_rect(m) {
        if hot == HOT_TOGGLE {
            wash(session, brush, r, dim);
        }
        super::controls::text(
            session,
            brush,
            GLYPH_TOGGLE,
            r,
            theme::FONT_ICON,
            14.0,
            400,
            theme::text(),
            TextAlignment::Center,
            ParagraphAlignment::Center,
            dim,
        );
    }
}

/// The hover wash under a pane chrome button.
fn wash(session: &DrawingSession, brush: &Brush, r: Rect, dim: f32) {
    super::controls::fill_rr(session, brush, r, theme::RADIUS_SM, theme::w(0.06), dim);
}

/// `hot_index` values the pane's chrome buttons occupy. Negative, so they can
/// never collide with an item index — `Ctrl::hot_index` is the item-hover slot
/// the SelectorBar already uses, and -1 keeps its "nothing hovered" meaning.
pub(crate) const HOT_BACK: i32 = -2;
pub(crate) const HOT_TOGGLE: i32 = -3;
/// The settings row's slot in the shared item-index space — far above any real
/// item index, so it can never collide with one. It serves BOTH per-row index
/// fields: `Ctrl::hot_index` when the pointer rests on the row, and
/// `Ctrl::selected_index` when the settings page is the selection (the row is a
/// selectable page like any menu item; `sync_selected_tag` resolves the
/// [`SETTINGS_TAG`] echo to this value, and the tile/bar sprites place
/// themselves on [`settings_rect`] from it).
pub(crate) const SETTINGS_INDEX: i32 = 1 << 16;

/// The pane header. Drawn only in an expanded pane — a rail has no room for it,
/// and [`metrics`] has already reserved no header row in that case.
fn paint_title(
    session: &DrawingSession,
    brush: &Brush,
    m: &Metrics,
    text: Option<&NavPaneText>,
    dim: f32,
) {
    let Some(t) = text else { return };
    let Some(title) = &t.title else { return };
    if !m.kind.expanded() {
        return;
    }
    let avail = (m.width - theme::SPACE_16 - LABEL_PAD_R).max(0.0);
    if avail <= 0.0 {
        return;
    }
    let _ = title.set_max_width(t.title_w.min(avail));
    let y = CHROME_H + (TITLE_H - t.line_h) / 2.0;
    let mut c = linear(theme::text_secondary());
    c.a *= dim;
    brush.set_color(c);
    session.draw_text_layout(Vector2 { x: theme::SPACE_16, y }, title, brush);
}

/// Per-item icon glyph plus, in an expanded pane, its label.
fn paint_items(
    session: &DrawingSession,
    brush: &Brush,
    node: &Node,
    m: &Metrics,
    rect: Rect,
    text: Option<&NavPaneText>,
    dim: f32,
) {
    let ctrl = node.ctrl();
    let count = ctrl.items.len();
    let n = visible_items(m, rect.height(), count);
    if n == 0 {
        return;
    }
    let sel = ctrl.selected_index;
    for i in 0..n {
        let row = item_rect(m, i as i32);
        let active = i as i32 == sel;
        let color = if active {
            theme::accent()
        } else {
            theme::text_tertiary()
        };
        paint_row_icon(
            session,
            brush,
            ctrl.icons.get(i).copied().unwrap_or(0),
            &ctrl.items[i],
            row,
            color,
            dim,
        );
        if m.kind.expanded() {
            paint_row_label(
                session,
                brush,
                text.and_then(|t| t.items.get(i)).and_then(|l| l.as_ref()),
                text.map(|t| t.line_h).unwrap_or(0.0),
                row,
                m.width,
                if active { theme::text() } else { theme::text_secondary() },
                dim,
            );
        }
    }
}

/// One row's leading glyph, centred in the rail column. Falls back to the
/// label's first character when the item carries no icon — which is what makes
/// a compact rail of icon-less items still readable.
fn paint_row_icon(
    session: &DrawingSession,
    brush: &Brush,
    icon: u32,
    label: &str,
    row: Rect,
    color: crate::Color,
    dim: f32,
) {
    let cell = Rect::from_xywh(row.left, row.top, theme::NAV_RAIL_W, row.height());
    let mut buf = [0u8; 4];
    let (glyph, family) = match super::controls::glyph_into(icon, &mut buf) {
        Some(g) if icon != 0 => (g, theme::FONT_ICON),
        // The fallback borrows from the label itself, so it allocates nothing.
        _ => (
            label.char_indices().next().map(|(_, c)| &label[..c.len_utf8()]).unwrap_or(""),
            "Segoe UI",
        ),
    };
    super::controls::text(
        session,
        brush,
        glyph,
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

/// One row's label, from the cached layout. The only per-repaint work is
/// narrowing it to the room the current pane width leaves, which is what makes
/// labels ellipsize as the pane narrows.
fn paint_row_label(
    session: &DrawingSession,
    brush: &Brush,
    layout: Option<&TextLayout>,
    line_h: f32,
    row: Rect,
    pane_w: f32,
    color: crate::Color,
    dim: f32,
) {
    let Some(l) = layout else { return };
    let avail = (pane_w - LABEL_X - LABEL_PAD_R).max(0.0);
    if avail <= 0.0 {
        return;
    }
    let _ = l.set_max_width(avail);
    let mut c = linear(color);
    c.a *= dim;
    brush.set_color(c);
    session.draw_text_layout(
        Vector2 {
            x: row.left + LABEL_X,
            y: row.top + (row.height() - line_h) / 2.0,
        },
        l,
        brush,
    );
}

/// The settings row at the foot of the pane — the same icon-plus-label shape as
/// a menu item, and it selects like one: when the settings page is current
/// (`selected_index == `[`SETTINGS_INDEX`]) the row takes the same active
/// colors a selected menu item does, and the selection tile/bar sprites sit
/// under it.
fn paint_settings(
    session: &DrawingSession,
    brush: &Brush,
    node: &Node,
    m: &Metrics,
    rect: Rect,
    text: Option<&NavPaneText>,
    dim: f32,
) {
    let Some(row) = settings_rect(m, rect.height()) else {
        return;
    };
    if node.ctrl().hot_index == SETTINGS_INDEX {
        wash(session, brush, row, dim);
    }
    let active = node.ctrl().selected_index == SETTINGS_INDEX;
    let cell = Rect::from_xywh(row.left, row.top, theme::NAV_RAIL_W, row.height());
    super::controls::text(
        session,
        brush,
        GLYPH_SETTINGS,
        cell,
        theme::FONT_ICON,
        theme::FONT_SIZE_MD,
        400,
        if active {
            theme::accent()
        } else {
            theme::text_tertiary()
        },
        TextAlignment::Center,
        ParagraphAlignment::Center,
        dim,
    );
    if m.kind.expanded() {
        paint_row_label(
            session,
            brush,
            text.and_then(|t| t.settings.as_ref()),
            text.map(|t| t.line_h).unwrap_or(0.0),
            row,
            m.width,
            if active { theme::text() } else { theme::text_secondary() },
            dim,
        );
    }
}

/// Accessible names for the pane's own chrome. The settings row's is the same
/// string it is drawn with, so what a screen reader announces and what the eye
/// reads cannot diverge; the two buttons carry no visible label, so theirs name
/// the action the glyph stands for.
pub(crate) fn chrome_label(hit: Hit) -> &'static str {
    match hit {
        Hit::Back => "Back",
        Hit::Toggle => "Navigation",
        Hit::Settings => SETTINGS_LABEL,
        Hit::Item(_) => "",
    }
}
