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
//! **The pane owns no surface.** Its chrome — the background, the divider, the
//! selection tile and its accent bar, the row hover ink and the chrome-button
//! wash — is retained compositor parts ([`parts::nav_plan`](super::parts)), and
//! every run it shows — the two chrome glyphs, the header, and a leading glyph
//! plus a label per row — is retained glyph sprites
//! ([`glyph_text::nav_sync`](super::glyph_text::nav_sync)). Nothing here reaches
//! a `BeginDraw`, so a pane that opens, a selection that moves, or a pointer
//! crossing rows costs no raster at all.
//!
//! What still divides the two halves is *motion*: anything that MOVES (the
//! pane's width transition, the tile, the bar, the ink) is a part whose motion
//! runs DWM-side, and every run SNAPS to the state that repaint published,
//! because a shaped text layout cannot be interpolated.
//!
//! The row runs live in a [`RowText`](super::glyph_text::RowText) — the
//! index-addressed leading-glyph-plus-label primitive. That is deliberate and
//! not merely convenient: this control is one of the backend's node-less,
//! index-addressed item containers, and the same primitive is what a virtualized
//! items control draws its window of rows through.

use windows_canvas::{Rect, TextFormat, TextLayout, Trimming};

use super::glyph_text::RowText;
use super::node::Extras;
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

/// The pane's shaped runs and the sprites that place them.
///
/// The runs are built by the layout pass whenever the NavigationView's
/// `text_dirty` is set — i.e. exactly when the pane title, the items or the
/// settings row changed — and read by
/// [`glyph_text::nav_sync`](super::glyph_text::nav_sync), which only runs on a
/// dirty repaint. Nothing here is constructed per frame: a repaint reuses these
/// `IDWriteTextLayout`s and at most re-points a max width, which is a property
/// set, not an allocation.
///
/// This is the same mechanism [`CaptionText`](super::caption::CaptionText) uses
/// for the caption band, and it needs its own home for the same reason: a pane
/// carries two runs per row plus three of its own, and `Node::text_layout` holds
/// exactly one.
///
/// **The sprites live here too, and are never rebuilt with the runs.** A rebuild
/// goes through [`adopt`](Self::adopt), which swaps the layouts and leaves every
/// part alone — the parts own compositor visuals parented into the node, so
/// replacing this struct wholesale would orphan the sprites already on screen.
#[derive(Default)]
pub(crate) struct NavPaneText {
    pub title: Option<TextLayout>,
    pub title_w: f32,
    /// The back arrow and the hamburger, shaped from the Fluent icon face. They
    /// are runs rather than painted characters for the reason the button
    /// family's icon is: a pane that painted even one glyph would need a
    /// surface, and this control no longer gets one.
    pub back: Option<TextLayout>,
    pub toggle: Option<TextLayout>,
    /// Line height of the item runs. Kept for the intrinsic measure; placement
    /// centres on each run's own measured height rather than this shared one.
    pub line_h: f32,
    /// The menu rows, followed by the settings row at index `items.len()`.
    ///
    /// The settings row is a row of this list rather than a case beside it
    /// because it *is* one — the same leading-glyph-plus-label shape, the same
    /// geometry, and it selects like a menu item ([`SETTINGS_INDEX`]). Its index
    /// is the item count and not the visible-row count, so shrinking the window
    /// the pane has room for never re-points its runs at a menu item's.
    pub rows: RowText,
    /// The three chrome runs' sprites, in the order [`ChromeRun`] names them.
    pub chrome: [super::glyph_text::TextPart; 3],
}

/// Which of the pane's own three runs a chrome sprite carries. The pane's rows
/// are addressed by index; these three are not, so they are named.
#[derive(Clone, Copy)]
pub(crate) enum ChromeRun {
    Back = 0,
    Toggle = 1,
    Title = 2,
}

/// The shaped runs alone, as the layout pass builds them.
///
/// A plain value with no sprites in it, so it can be built from an immutable
/// borrow of the node and then moved into the live [`NavPaneText`] under a
/// mutable one — which is what keeps the parts across a rebuild.
#[derive(Default)]
pub(crate) struct NavRuns {
    title: Option<TextLayout>,
    title_w: f32,
    back: Option<TextLayout>,
    toggle: Option<TextLayout>,
    line_h: f32,
    leading: Vec<Option<TextLayout>>,
    labels: Vec<Option<TextLayout>>,
}

impl NavPaneText {
    /// Take a freshly shaped run set, keeping every sprite.
    pub(crate) fn adopt(&mut self, r: NavRuns) {
        self.title = r.title;
        self.title_w = r.title_w;
        self.back = r.back;
        self.toggle = r.toggle;
        self.line_h = r.line_h;
        self.rows.adopt(r.leading, r.labels);
    }

    /// One chrome run and the part that places it, borrowed together.
    ///
    /// One call rather than two accessors for the reason [`RowText::row`] is
    /// one: the part is taken mutably and the run shared, out of fields the
    /// borrow checker will only let a caller split from inside the type.
    pub(crate) fn chrome_slot(
        &mut self,
        which: ChromeRun,
    ) -> (&mut super::glyph_text::TextPart, Option<&TextLayout>) {
        let run = match which {
            ChromeRun::Back => self.back.as_ref(),
            ChromeRun::Toggle => self.toggle.as_ref(),
            ChromeRun::Title => self.title.as_ref(),
        };
        (&mut self.chrome[which as usize], run)
    }
}

/// Lay out one pane run at its natural width.
///
/// `trim` marks the runs that must degrade rather than spill: a label is built
/// unconstrained here and narrowed to the room actually available at sync time,
/// at which point the trimming sign takes over — so a long item label reads
/// "Equalizer…" rather than running across the divider. A glyph run is never
/// trimmed; there is nothing to elide in one character, and an ellipsis sign on
/// the icon face is not a character anyone wants to see.
fn run(text: &str, family: &str, size: f32, weight: u16, trim: bool) -> Option<(TextLayout, f32, f32)> {
    if text.is_empty() {
        return None;
    }
    let fmt = TextFormat::with_weight(family, size, windows_canvas::FontWeight(weight as i32))
        .ok()?;
    let layout = TextLayout::new(text, &fmt, 100_000.0, 100_000.0).ok()?;
    let _ = layout.set_word_wrap(false);
    if trim {
        let _ = layout.set_trimming(Trimming::CharacterEllipsis, &fmt);
    }
    let (w, h) = layout.measure().ok()?;
    Some((layout, w, h))
}

/// A pane label: Segoe UI, ellipsized.
fn label_run(text: &str, size: f32, weight: u16) -> Option<(TextLayout, f32, f32)> {
    run(text, "Segoe UI", size, weight, true)
}

/// One row's leading glyph, shaped from the Fluent icon face.
///
/// Falls back to the label's first character in the text face when the item
/// carries no icon — which is what makes a compact rail of icon-less items still
/// readable. The fallback borrows from the label, so it allocates nothing.
fn leading_run(icon: u32, label: &str, size: f32) -> Option<TextLayout> {
    let mut buf = [0u8; 4];
    let (glyph, family) = match super::controls::glyph_into(icon, &mut buf) {
        Some(g) if icon != 0 => (g, theme::FONT_ICON),
        _ => (
            label
                .char_indices()
                .next()
                .map_or("", |(_, c)| &label[..c.len_utf8()]),
            "Segoe UI",
        ),
    };
    run(glyph, family, size, 400, false).map(|(l, _, _)| l)
}

/// (Re)build the pane's shaped runs.
///
/// Everything the pane could show is shaped, not merely what the current state
/// displays: the header of a collapsed pane, the labels of a rail, and the
/// settings row of a pane that has hidden it. All three are per-repaint
/// decisions that do not set `text_dirty`, so a build that skipped them would
/// leave the pane with no run to place on the repaint that reveals them — and
/// would put a DirectWrite layout build on the click path that opens the pane.
pub(crate) fn build_runs(x: &Extras, items: &[String], icons: &[u32]) -> NavRuns {
    let title = label_run(&x.pane_title, theme::FONT_SIZE_SM, 600);
    let settings = label_run(SETTINGS_LABEL, theme::FONT_SIZE_MD, 400);

    // One entry per row: the menu items, then the settings row.
    let mut leading = Vec::with_capacity(items.len() + 1);
    let mut labels = Vec::with_capacity(items.len() + 1);
    let mut line_h = 0.0f32;
    for (i, label) in items.iter().enumerate() {
        let r = label_run(label, theme::FONT_SIZE_MD, 400);
        if let Some((_, _, h)) = &r {
            line_h = line_h.max(*h);
        }
        leading.push(leading_run(
            icons.get(i).copied().unwrap_or(0),
            label,
            theme::FONT_SIZE_LG,
        ));
        labels.push(r.map(|(l, _, _)| l));
    }
    if line_h <= 0.0 {
        line_h = settings
            .as_ref()
            .map(|s| s.2)
            .or(title.as_ref().map(|t| t.2))
            .unwrap_or(theme::FONT_SIZE_MD * 1.4);
    }
    // The settings glyph sets a size smaller than an item's: it is a fixed
    // Fluent cog rather than an app-chosen icon, and at rail size it reads as
    // heavier than the icons above it.
    leading.push(leading_run(
        GLYPH_SETTINGS.chars().next().map_or(0, u32::from),
        SETTINGS_LABEL,
        theme::FONT_SIZE_MD,
    ));
    labels.push(settings.map(|s| s.0));

    NavRuns {
        title_w: title.as_ref().map_or(0.0, |t| t.1),
        title: title.map(|t| t.0),
        back: run(GLYPH_BACK, theme::FONT_ICON, 12.0, 400, false).map(|r| r.0),
        toggle: run(GLYPH_TOGGLE, theme::FONT_ICON, 14.0, 400, false).map(|r| r.0),
        line_h,
        leading,
        labels,
    }
}

// ── Row geometry ────────────────────────────────────────────

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

/// The rail column of a row — where its leading glyph is centred.
///
/// A square at the head of the row whatever the pane's width, so a glyph does
/// not shift sideways when the pane opens and the row grows a label beside it.
pub(crate) fn icon_cell(row: Rect) -> Rect {
    Rect::from_xywh(row.left, row.top, theme::NAV_RAIL_W, row.height())
}

/// The column a row's label occupies: after the rail, before the divider.
///
/// The one definition of that box, and it is read twice per row for two
/// different purposes — as the origin the run is placed from, and as the clip
/// the run is cut to. A sprite is clipped by nothing it is not given, so a label
/// too long to elide (or one whose trimming sign DirectWrite declined to apply)
/// would otherwise cross the divider and land on the content beside it.
///
/// Empty when the pane is too narrow to hold a label at all, which is what makes
/// a rail show glyphs alone without a second code path deciding so.
pub(crate) fn label_box(m: &Metrics, row: Rect) -> Rect {
    let w = (m.width - LABEL_X - LABEL_PAD_R).max(0.0);
    Rect::from_xywh(row.left + LABEL_X, row.top, w, row.height())
}

/// The pane header's box, or `None` when no header is shown.
///
/// Both the height and the top are **decomposed from what [`metrics`] already
/// reserved**, never re-tested against the pane state. `items_y` is the chrome
/// row plus the header row, so subtracting the one leaves the other — and a
/// header therefore exists here exactly when `metrics` made room for it.
///
/// Deriving it instead of re-deriving it is load-bearing twice over. A pane with
/// no back arrow and no hamburger has NO chrome row, so a header pinned to a
/// constant `CHROME_H` would float 40 DIPs below its own reserved row; and a
/// header gated on a constant would vanish entirely in that same state, because
/// the row it was reserved is shorter than the chrome row it was compared to.
pub(crate) fn title_box(m: &Metrics) -> Option<Rect> {
    let chrome_h = if m.back || m.toggle { CHROME_H } else { 0.0 };
    let title_h = m.items_y - chrome_h;
    let w = (m.width - theme::SPACE_16 - LABEL_PAD_R).max(0.0);
    (m.kind.expanded() && title_h > 0.0 && w > 0.0)
        .then(|| Rect::from_xywh(theme::SPACE_16, chrome_h, w, title_h))
}

/// The chrome button a `hot_index` names, if it names one.
///
/// The wash under a hovered back arrow or hamburger is one part that snaps
/// between the two — only one can be hot, because `hot_index` holds a single
/// value — so this is the single place that resolves which box it takes.
pub(crate) fn chrome_rect(m: &Metrics, hot: i32) -> Option<Rect> {
    match hot {
        HOT_BACK => back_rect(m),
        HOT_TOGGLE => toggle_rect(m),
        _ => None,
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
