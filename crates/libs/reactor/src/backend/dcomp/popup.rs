//! The popup / overlay layer — the shared top-level surface that backs Select
//! dropdowns, DropDownButton / SplitButton menus and MenuFlyouts. A [`Popup`] is
//! a single FP16 surface z-promoted above the whole reactor tree (a child visual
//! of the compositor root, per the parity spec's sanctioned alternative to a
//! second HWND), anchored under its trigger with monitor/edge flip, light-
//! dismissed on outside-click or Escape, and keyboard-navigable (Up/Down/Enter/
//! Esc). It reveals with a one-shot compositor scale+fade grown out of its
//! anchored edge and dismisses with a compositor fade (the closing visual rides
//! the exit-ghost timer) — both play DWM-side, so a popup costs zero app frames
//! open or closed.

use std::time::Duration;

use super::animate;
use super::bootstrap::{Compositing, NodeSurface};
use super::glyph_atlas::GlyphAtlas;
use super::glyph_text::{Align, Pen, PopupText};
use super::node::{linear, MenuRow};
use super::theme;
use crate::style::{AnimationConfig, Easing};
use crate::backend::ControlId;
use crate::system_bindings::{ContainerVisual, IVisual, Visual, POINT};
use windows_canvas_core::{
    bindings::ID2D1DeviceContext, Brush, ColorF, DrawingSession, FontWeight, ParagraphAlignment,
    Rect, RoundedRect, TextAlignment, TextFormat, Vector2,
};
use windows_core::Interface;
use windows_numerics::{Matrix3x2, Vector3};

/// Shadow bleed margin baked into the surface around the drawn panel (DIPs).
/// Sized to hold the soft Gaussian drop shadow (≈ `SHADOW_BLUR`·3 + drop offset)
/// without clipping at the surface edge.
const MARGIN: f32 = theme::SPACE_32;
/// Gaussian standard deviation of the popup drop shadow (DIPs).
const SHADOW_BLUR: f32 = theme::SPACE_8 - theme::BORDER_W;
/// Vertical drop offset of the popup shadow (DIPs) — depth cue, lit from above.
const SHADOW_DROP: f32 = theme::SPACE_4;
/// One command/selection row height (DIPs).
const ROW: f32 = theme::ROW_H;
/// Separator row height (DIPs).
const SEP: f32 = theme::SPACE_8 + theme::BORDER_W;
const PANEL_PAD: f32 = theme::SPACE_4;
/// Widest a text flyout's paragraph column gets before it wraps (DIPs). A
/// flyout is explanatory prose, and prose set wider than this stops scanning
/// cleanly — the same reason the menu panel caps at 360.
const TEXT_MAX_W: f32 = 320.0;
/// Inset around a text flyout's paragraph (DIPs). Wider than `PANEL_PAD`
/// because nothing here is a row: the padding IS the panel's whole margin.
const TEXT_PAD: f32 = theme::SPACE_12;
/// Inset around a rich flyout's hosted subtree (DIPs). The same margin the
/// paragraph gets, so text and controls sit on the same grid.
const CONTENT_PAD: f32 = TEXT_PAD;

/// What a popup is showing.
///
/// A menu is a list of hit-testable rows; a text flyout is one wrapped
/// paragraph with nothing to hit. They share the panel, the shadow, the reveal
/// and the light dismiss — and differ in every part that treats content as
/// selectable, which is why this is a sum type rather than an empty row list.
pub(crate) enum PopupBody {
    Menu(Vec<MenuRow>),
    /// A wrapped paragraph, carried as the layout it was MEASURED with.
    ///
    /// Not the string: the panel is sized from this run, and `draw_text` would
    /// build a second, identical layout to draw it. Keeping the one that was
    /// measured makes the open path build exactly one.
    Text(windows_canvas_core::TextLayout),
    /// Live reconciled nodes — the root of a mounted flyout-content subtree,
    /// with the size its own layout pass measured.
    ///
    /// Unlike the other two, this body draws NOTHING on the popup's surface:
    /// the subtree owns real composition visuals, and the popup hosts them by
    /// re-parenting the root's container into its own. The panel behind them is
    /// still the popup's, so a rich flyout gets the same field, hairline and
    /// shadow as a menu without having to draw any of it.
    Nodes { root: ControlId, size: (f32, f32) },
}

impl PopupBody {
    /// The rows, or nothing at all for a text flyout. Every selection path
    /// (hit-testing, arrow keys, commit) reads through here, so a text flyout
    /// is inert on all of them by construction rather than by a check at each.
    fn rows(&self) -> &[MenuRow] {
        match self {
            Self::Menu(v) => v,
            Self::Text(_) | Self::Nodes { .. } => &[],
        }
    }

    /// The flyout-content root this body hosts, if it hosts one.
    pub(crate) fn hosted_root(&self) -> Option<ControlId> {
        match self {
            Self::Nodes { root, .. } => Some(*root),
            _ => None,
        }
    }
}

/// The four sides a popup can take, once the thirteen `FlyoutPlacementMode`
/// values are reduced to the axis each one actually names.
///
/// The edge-aligned variants differ from their plain counterparts only in how
/// the panel aligns along the free axis, and this backend centres (text) or
/// leading-aligns (menus) rather than honouring each alignment separately — so
/// they collapse onto the side they are edge-aligned to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Placement {
    Above,
    Below,
    Left,
    Right,
}

impl Placement {
    fn of(mode: i32) -> Self {
        use crate::FlyoutPlacementMode as M;
        match M(mode) {
            M::Top | M::TopEdgeAlignedLeft | M::TopEdgeAlignedRight => Self::Above,
            M::Left | M::LeftEdgeAlignedTop | M::LeftEdgeAlignedBottom => Self::Left,
            M::Right | M::RightEdgeAlignedTop | M::RightEdgeAlignedBottom => Self::Right,
            // Bottom, the edge-aligned bottoms, Full and Auto. `Full` has no
            // meaning without a dialog host to fill, and `Auto` means "let the
            // framework decide" — which here is the drop-down direction every
            // menu already uses.
            _ => Self::Below,
        }
    }
}

/// Lay a text flyout's paragraph out, wrapped into the flyout column.
///
/// Returns the layout itself, not just its size — it is what the popup will
/// draw, so measuring and drawing share one run. `None` when DirectWrite cannot
/// lay it out, which the caller treats as "do not open": a flyout panel sized
/// to nothing is a bare shadow with no way to tell what it failed to say.
pub(crate) fn layout_text_body(s: &str) -> Option<windows_canvas_core::TextLayout> {
    let fmt = TextFormat::with_weight("Segoe UI", theme::FONT_SIZE_MD, FontWeight(400))
        .ok()?
        .with_alignment(TextAlignment::Leading)
        .with_paragraph_alignment(ParagraphAlignment::Top)
        .with_word_wrap(true);
    let column = TEXT_MAX_W - TEXT_PAD * 2.0;
    let layout = windows_canvas_core::TextLayout::new(s, &fmt, column, 100_000.0).ok()?;
    Some(layout)
}

/// Reveal one-shot: a snappy Fluent-style grow-out-of-the-trigger (0.96→1
/// scale + fade, decelerating).
const REVEAL_DURATION: Duration = Duration::from_millis(160);
/// Dismiss fade length. The backend parks the closing visual as a ghost for
/// this long (plus grace) before releasing it.
pub(crate) const EXIT_DURATION: Duration = Duration::from_millis(100);

/// A live popup surface. The backend owns at most one (`Option<Popup>`).
pub(crate) struct Popup {
    /// The control that opened this popup (its events fire on selection).
    pub owner: ControlId,
    /// `true` = ComboBox selection list; `false` = command menu / dropdown.
    pub combo: bool,
    /// `true` = AutoSuggestBox suggestion list (commit chooses a suggestion +
    /// keeps the field focused, rather than closing a trigger control).
    pub suggest: bool,
    container: ContainerVisual,
    surf: NodeSurface,
    body: PopupBody,
    /// Drawn-panel rect in window DIPs (excludes the shadow margin).
    panel: Rect,
    /// The trigger/field rect this popup is anchored under (window DIPs).
    anchor: Rect,
    /// The window viewport (w, h) DIPs the panel is clamped/flipped within.
    window: (f32, f32),
    /// Currently highlighted row index (`usize::MAX` = none).
    pub hovered: usize,
    /// The `FlyoutPlacementMode` discriminant this popup was opened with, kept
    /// so a re-layout (a suggestion list that grew) lands on the same side.
    placement: i32,
    px: f32,
    /// Every run this popup shows, as retained glyph sprites above its panel.
    text: PopupText,
}

impl Popup {
    /// Panel + surface geometry for `body` anchored on `anchor`, clamped /
    /// flipped to fit `window`. Returns `(panel_rect, surf_w, surf_h)` in DIPs.
    ///
    /// `placement` is the app's requested side. It is a REQUEST: a side with no
    /// room flips to its opposite, and the result is clamped into the viewport
    /// regardless — an off-screen flyout is worse than a differently-placed one.
    fn layout(
        body: &PopupBody,
        anchor: Rect,
        window: (f32, f32),
        placement: i32,
    ) -> Option<(Rect, f32, f32)> {
        let (w, h) = match body {
            PopupBody::Menu(items) => {
                let h: f32 = items
                    .iter()
                    .map(|r| if r.separator { SEP } else { ROW })
                    .sum::<f32>()
                    + PANEL_PAD * 2.0;
                // A menu is at least as wide as the control it drops from, so
                // it reads as belonging to it.
                (anchor.width().max(200.0).min(360.0), h)
            }
            PopupBody::Text(layout) => {
                let (tw, th) = layout.measure().ok()?;
                (tw + TEXT_PAD * 2.0, th + TEXT_PAD * 2.0)
            }
            // Already measured, by the subtree's own layout pass.
            PopupBody::Nodes { size, .. } => (
                size.0 + CONTENT_PAD * 2.0,
                size.1 + CONTENT_PAD * 2.0,
            ),
        };

        const GAP: f32 = theme::SPACE_4;
        const EDGE: f32 = theme::SPACE_4;
        let fits_below = anchor.bottom + GAP + h <= window.1 - EDGE;
        let fits_above = anchor.top - GAP - h >= EDGE;
        let fits_right = anchor.right + GAP + w <= window.0 - EDGE;
        let fits_left = anchor.left - GAP - w >= EDGE;

        // Centre the panel on the anchor across the free axis, which is what
        // makes a Top/Bottom flyout read as attached to its trigger rather than
        // merely near it. A menu keeps its historical leading-edge alignment.
        let centred_x = anchor.left + (anchor.width() - w) * 0.5;
        let leading_x = anchor.left;
        let x_for_vertical = match body {
            PopupBody::Menu(_) => leading_x,
            // A flyout centres on its trigger, prose or panel alike — that is
            // what reads as attached to it rather than merely near it.
            PopupBody::Text(_) | PopupBody::Nodes { .. } => centred_x,
        };
        let centred_y = anchor.top + (anchor.height() - h) * 0.5;

        // Each side falls back to its opposite, and only to its opposite: a
        // flyout asked to sit left belongs on the horizontal axis even when
        // neither side is roomy, and the clamp below keeps it on screen.
        let (mut x, mut y) = match Placement::of(placement) {
            Placement::Left => {
                let left = fits_left || !fits_right;
                (if left { anchor.left - GAP - w } else { anchor.right + GAP }, centred_y)
            }
            Placement::Right => {
                let right = fits_right || !fits_left;
                (if right { anchor.right + GAP } else { anchor.left - GAP - w }, centred_y)
            }
            Placement::Above => {
                let above = fits_above || !fits_below;
                (x_for_vertical, if above { anchor.top - GAP - h } else { anchor.bottom + GAP })
            }
            Placement::Below => {
                let below = fits_below || !fits_above;
                (x_for_vertical, if below { anchor.bottom + GAP } else { anchor.top - GAP - h })
            }
        };
        x = x.min(window.0 - w - EDGE).max(EDGE);
        y = y.min(window.1 - h - EDGE).max(EDGE);
        Some((
            Rect::from_xywh(x, y, w, h),
            w + MARGIN * 2.0,
            h + MARGIN * 2.0,
        ))
    }

    /// Mint the overlay surface for `panel`, position it (accounting for the shadow
    /// margin), and size its backing bitmap to the DIP/scale.
    fn build_surface(
        comp: &Compositing,
        panel: Rect,
        surf_w: f32,
        surf_h: f32,
    ) -> windows_core::Result<(ContainerVisual, NodeSurface)> {
        let scale = comp.scale();
        let (container, mut surf) =
            comp.new_overlay((surf_w * scale).ceil() as i32, (surf_h * scale).ceil() as i32)?;
        surf.set_dip_size(surf_w, surf_h);
        let vis: IVisual = container.cast()?;
        vis.SetOffset(Vector3::new(panel.left - MARGIN, panel.top - MARGIN, 0.0))?;
        vis.SetSize(Vector2::new(surf_w, surf_h))?;
        Ok((container, surf))
    }

    /// Open a popup of `items` anchored under `anchor` (window DIPs), clamped /
    /// flipped to fit the `window` (w, h) DIP viewport.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        comp: &Compositing,
        glyphs: &mut GlyphAtlas,
        owner: ControlId,
        body: PopupBody,
        anchor: Rect,
        window: (f32, f32),
        combo: bool,
        selected: i32,
        suggest: bool,
        placement: i32,
    ) -> windows_core::Result<Self> {
        let Some((panel, surf_w, surf_h)) = Self::layout(&body, anchor, window, placement) else {
            return Err(windows_core::Error::from_hresult(windows_core::HRESULT(
                -2147024809, // E_INVALIDARG: nothing measurable to show.
            )));
        };
        let (container, surf) = Self::build_surface(comp, panel, surf_w, surf_h)?;
        let hovered = if combo && selected >= 0 {
            selected as usize
        } else {
            usize::MAX
        };
        let mut popup = Self {
            owner,
            combo,
            suggest,
            container,
            surf,
            body,
            panel,
            anchor,
            window,
            hovered,
            placement,
            px: comp.scale(),
            text: PopupText::default(),
        };
        popup.reveal(comp);
        popup.draw(comp, glyphs);
        Ok(popup)
    }

    /// Play the open reveal: a one-shot compositor scale (0.96→1) + fade,
    /// pivoted on the panel's anchored edge (top-center when it drops below its
    /// trigger, bottom-center when flipped above), so the menu grows out of the
    /// control that opened it. DWM plays it; the app takes zero frames.
    fn reveal(&self, comp: &Compositing) {
        let Ok(v) = self.container.cast::<Visual>() else { return };
        let cfg = AnimationConfig {
            opacity: Some(1.0),
            from_opacity: Some(0.0),
            scale: Some(1.0),
            from_scale: Some(0.96),
            duration: REVEAL_DURATION,
            easing: Easing::EaseOut,
        };
        // Pivot in surface DIPs: the panel sits at (MARGIN, MARGIN) inside the
        // shadow-bleed surface; flipped-above panels anchor at their bottom.
        let pivot_y = if self.panel.top < self.anchor.top {
            MARGIN + self.panel.height()
        } else {
            MARGIN
        };
        animate::start(
            comp.compositor(),
            &v,
            &cfg,
            Some((MARGIN + self.panel.width() / 2.0, pivot_y)),
        );
    }

    /// Replace a suggestion popup's rows in place (the filtered list changed as the
    /// user typed). No reveal replays, so the panel does not re-pop on each
    /// keystroke; the backing surface is rebuilt only when the row count (and
    /// thus the panel height) changes — the fresh visual mounts at rest
    /// (opacity 1, scale 1).
    pub fn update_items(&mut self, comp: &Compositing, glyphs: &mut GlyphAtlas, items: Vec<MenuRow>) {
        let resized = items.len() != self.body.rows().len();
        self.body = PopupBody::Menu(items);
        if self.hovered != usize::MAX && self.hovered >= self.body.rows().len() {
            self.hovered = usize::MAX;
        }
        if resized
            && let Some((panel, surf_w, surf_h)) =
                Self::layout(&self.body, self.anchor, self.window, self.placement)
            && let Ok((container, surf)) = Self::build_surface(comp, panel, surf_w, surf_h)
        {
            // The sprites are parented into the container being removed, so
            // they go with it — hiding them would leave the next sync placing
            // into a visual no longer in the tree.
            self.text.orphan();
            comp.remove_overlay(&self.container);
            self.container = container;
            self.surf = surf;
            self.panel = panel;
        }
        self.draw(comp, glyphs);
    }

    /// Start the compositor-side dismiss fade and hand the overlay visual back
    /// to the caller, who parks it as a ghost until the fade lands (the visual
    /// COM-references the whole surface chain, so nothing else need be held).
    /// `None` means the visual was removed immediately (cast failure).
    pub fn into_exit(self, comp: &Compositing) -> Option<Visual> {
        match self.container.cast::<Visual>() {
            Ok(v) => {
                let cfg = AnimationConfig {
                    opacity: Some(0.0),
                    from_opacity: None,
                    scale: None,
                    from_scale: None,
                    duration: EXIT_DURATION,
                    easing: Easing::EaseIn,
                };
                animate::start(comp.compositor(), &v, &cfg, None);
                Some(v)
            }
            Err(_) => {
                comp.remove_overlay(&self.container);
                None
            }
        }
    }

    /// The flyout-content root this popup shows, if it shows one.
    pub fn body_root(&self) -> Option<ControlId> {
        self.body.hosted_root()
    }

    /// Adopt a flyout-content subtree's container into this popup's overlay,
    /// placed at the content inset inside the panel.
    ///
    /// Re-parenting rather than redrawing is the whole point: the subtree keeps
    /// the visuals, surfaces and springs it was built with, so the controls in a
    /// flyout are the same controls they would be anywhere else — not a
    /// snapshot the popup has to re-interpret.
    ///
    /// The offset is NOT set here: the subtree's own layout pass writes it, so
    /// that one place decides where the content sits and a later re-layout
    /// cannot silently disagree with an offset stamped at adoption time. See
    /// [`Self::content_inset`].
    pub fn adopt(&self, content: &Visual) -> windows_core::Result<()> {
        self.container.Children()?.InsertAtTop(content)?;
        Ok(())
    }

    /// Where hosted content sits inside this popup's own container: the panel
    /// is inset by `MARGIN` in the shadow-bleed surface, and the content by
    /// `CONTENT_PAD` inside the panel.
    pub fn content_inset() -> (f32, f32) {
        (MARGIN + CONTENT_PAD, MARGIN + CONTENT_PAD)
    }

    /// Hand a hosted subtree's container back out of the overlay.
    ///
    /// Called before the popup's own visual is parked as an exit ghost: the
    /// subtree outlives the popup — its nodes stay mounted, and the same flyout
    /// may open again — so it must not ride the ghost into release.
    pub fn release_content(&self, content: &Visual) {
        if let Ok(children) = self.container.Children() {
            let _ = children.Remove(content);
        }
    }

    /// The content rect a hosted subtree is laid out against: the panel's box
    /// less the content inset, in window DIPs.
    pub fn content_origin(&self) -> (f32, f32) {
        (self.panel.left + CONTENT_PAD, self.panel.top + CONTENT_PAD)
    }

    /// `true` if `(x, y)` window-DIP lies inside the drawn panel.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.panel.left && x < self.panel.right && y >= self.panel.top && y < self.panel.bottom
    }

    /// The selectable row index at `(x, y)` window-DIP, if any (skips separators
    /// and disabled rows).
    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        let mut ry = self.panel.top + PANEL_PAD;
        for (i, r) in self.body.rows().iter().enumerate() {
            let rh = if r.separator { SEP } else { ROW };
            if y >= ry && y < ry + rh && !r.separator && r.enabled {
                return Some(i);
            }
            ry += rh;
        }
        None
    }

    /// The tag/text the row at `index` reports on selection.
    pub fn row_tag(&self, index: usize) -> Option<String> {
        self.body.rows().get(index).map(|r| {
            if r.tag.is_empty() {
                r.text.clone()
            } else {
                r.tag.clone()
            }
        })
    }

    /// Move the highlight by `delta`, skipping separators/disabled rows.
    pub fn move_highlight(&mut self, delta: i32, comp: &Compositing, glyphs: &mut GlyphAtlas) {
        let n = self.body.rows().len() as i32;
        if n == 0 {
            return;
        }
        let mut i = if self.hovered == usize::MAX {
            if delta > 0 {
                -1
            } else {
                n
            }
        } else {
            self.hovered as i32
        };
        for _ in 0..n {
            i = (i + delta).rem_euclid(n);
            if let Some(r) = self.body.rows().get(i as usize)
                && !r.separator
                && r.enabled
            {
                self.hovered = i as usize;
                break;
            }
        }
        self.draw(comp, glyphs);
    }

    /// Set the highlighted row (pointer hover) and repaint if it changed.
    pub fn set_hovered(&mut self, idx: Option<usize>, comp: &Compositing, glyphs: &mut GlyphAtlas) {
        let new = idx.unwrap_or(usize::MAX);
        if new != self.hovered {
            self.hovered = new;
            self.draw(comp, glyphs);
        }
    }

    /// Redraw the panel surface (shadow + chrome + separators + highlight), then
    /// reconcile the text sprites above it.
    ///
    /// The atlas is threaded in rather than reached for because a popup owns no
    /// backend handle — it is held BY the backend, beside the atlas, so the two
    /// arrive together from every call site.
    fn draw(&mut self, comp: &Compositing, glyphs: &mut GlyphAtlas) {
        self.paint_surface(comp);
        self.sync_text(comp, glyphs);
    }

    fn paint_surface(&self, comp: &Compositing) {
        let mut offset = POINT::default();
        comp.device_lost.set(false);
        let ctx: ID2D1DeviceContext = match unsafe { self.surf.interop.BeginDraw(None, &mut offset) }
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let session = DrawingSession::from_borrowed_context(
            &ctx,
            Matrix3x2::translation(offset.x as f32, offset.y as f32),
        );
        session.set_grayscale_text_antialiasing();
        session.set_transform(&Matrix3x2 {
            m11: self.px,
            m12: 0.0,
            m21: 0.0,
            m22: self.px,
            m31: 0.0,
            m32: 0.0,
        });
        session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));

        if let Ok(brush) = session.create_solid_brush(ColorF::BLACK) {
            self.paint_panel(&session, &brush);
        }
        let _ = unsafe { self.surf.interop.EndDraw() };
    }

    fn paint_panel(&self, session: &DrawingSession, brush: &Brush) {
        // Panel-local origin: the panel sits at (MARGIN, MARGIN) in the surface.
        let p = Rect::from_xywh(MARGIN, MARGIN, self.panel.width(), self.panel.height());

        // Real soft Gaussian drop shadow: blur the panel silhouette's alpha and
        // composite it tinted black, dropped down. Runs only on discrete open/hover
        // repaints (the per-frame tick animates the visual's opacity/scale, never
        // redraws the surface), so the blur is never a per-frame cost. Falls back to
        // a layered approximation if the off-screen path fails (device loss).
        let panel_rr = RoundedRect::uniform(p, theme::RADIUS_MD);
        let real = session.drop_shadow(SHADOW_BLUR, linear(theme::b(0.6)), (0.0, SHADOW_DROP), || {
            set(brush, theme::b(1.0));
            session.fill_rounded_rect(&panel_rr, brush);
        });
        if !real {
            for (i, a) in [(6.0_f32, 0.10_f32), (3.0, 0.16), (1.0, 0.24)] {
                let s = Rect::new(p.left - i + 1.0, p.top - i + 3.0, p.right + i + 1.0, p.bottom + i + 3.0);
                set(brush, theme::b(a));
                session.fill_rounded_rect(&RoundedRect::uniform(s, theme::RADIUS_MD + i), brush);
            }
        }

        // Raised surface + hairline.
        set(brush, theme::surface_raised());
        session.fill_rounded_rect(&RoundedRect::uniform(p, theme::RADIUS_MD), brush);
        set(brush, theme::stroke());
        let inset = Rect::new(p.left + 0.5, p.top + 0.5, p.right - 0.5, p.bottom - 0.5);
        session.draw_rounded_rect(&RoundedRect::uniform(inset, theme::RADIUS_MD), brush, theme::BORDER_W);

        // Every run a popup shows — a flyout's paragraph, a row's icon, label
        // and shortcut — is a retained glyph sprite above this surface
        // (`sync_text`). What is left here is the separators and the highlight,
        // which is also what makes a hover a fill-only repaint: moving the
        // highlight no longer redraws a single word.
        let rows = match &self.body {
            PopupBody::Text(_) => return,
            // Live nodes draw themselves, onto their own visuals. The panel
            // above is all this surface contributes.
            PopupBody::Nodes { .. } => return,
            PopupBody::Menu(rows) => rows,
        };

        for (i, row, y, _) in rows_laid(rows, p.top) {
            if row.separator {
                set(brush, theme::stroke_divider());
                let y = y + SEP / 2.0;
                session.draw_line(
                    Vector2::new(p.left + theme::SPACE_8, y),
                    Vector2::new(p.right - theme::SPACE_8, y),
                    brush,
                    theme::BORDER_W,
                );
                continue;
            }
            if i == self.hovered {
                set(brush, if self.combo { theme::accent_fill() } else { theme::w(0.06) });
                let rr = row_rect(p, y);
                session.fill_rounded_rect(&RoundedRect::uniform(rr, theme::RADIUS_SM), brush);
            }
        }
    }

    /// Reconcile the popup's text as retained glyph sprites above its panel.
    ///
    /// A popup is the one surface in the backend that is not a node, so this
    /// does not run from the paint walk and takes its host from the popup's own
    /// container rather than from a `Node`. Everything past that is the ordinary
    /// placement path — [`Pen::over`] exists precisely so it can be.
    ///
    /// `dim` is `1.0`: a popup is never disabled as a whole, and a disabled ROW
    /// carries its own greyed ink through [`fg`].
    fn sync_text(&mut self, comp: &Compositing, glyphs: &mut GlyphAtlas) {
        // Split the borrow by field: the body is read while the text beside it
        // is written, and both live on `self`.
        let Self { body, text, container, panel, px, .. } = self;
        let p = Rect::from_xywh(MARGIN, MARGIN, panel.width(), panel.height());
        let mut pen = Pen::over(comp, glyphs, container.clone(), 1.0, *px);

        let rows = match body {
            // Placed from the very layout the panel was sized by — no second
            // run, and no chance of the two disagreeing. Leading-aligned and
            // top-anchored, because prose that grows downward from a fixed
            // corner is what a reader expects; a centred paragraph shifts every
            // line when the text changes.
            PopupBody::Text(layout) => {
                let b = Rect::new(
                    p.left + TEXT_PAD,
                    p.top + TEXT_PAD,
                    p.right - TEXT_PAD,
                    p.bottom - TEXT_PAD,
                );
                pen.place(&mut text.para, Some(layout), b, Align::Leading, theme::text());
                text.hide_rows_from(0);
                return;
            }
            // A hosted subtree's nodes carry their own runs, placed by the paint
            // walk into their own containers. This surface contributes no words.
            PopupBody::Nodes { .. } => {
                text.para.hide_all();
                text.hide_rows_from(0);
                return;
            }
            PopupBody::Menu(rows) => rows,
        };
        text.para.hide_all();

        let mut count = 0usize;
        for (i, row, y, _) in rows_laid(rows, p.top) {
            count = i + 1;
            let slot = text.row(i);
            if row.separator {
                for s in slot.iter_mut() {
                    s.hide();
                }
                continue;
            }
            let rr = row_rect(p, y);
            let has_icon = row.icon != 0;
            let ink = fg(row);

            // Alloc-free: the codepoint is encoded into a caller-owned buffer
            // rather than a fresh `String` per row per repaint. An icon-less row
            // pins the empty string, which shapes nothing and hides the slot.
            let mut buf = [0u8; 4];
            let glyph = if has_icon {
                super::controls::glyph_into(row.icon, &mut buf).unwrap_or("")
            } else {
                ""
            };
            let (part, run) = slot[0].pin(glyph, ICON_EM, 400, theme::FONT_ICON);
            pen.place(part, run, icon_cell(rr), Align::Centered, ink);

            // The label and the hint share one column and are told apart by
            // alignment alone — which is what `TrailingCentered` is for. Placing
            // them by hand would otherwise mean computing two origins where the
            // painted row computed none.
            let col = label_col(rr, has_icon);
            let (part, run) = slot[1].pin(&row.text, theme::FONT_SIZE_MD, 400, MENU_FACE);
            pen.place(part, run, col, Align::LeadingCentered, ink);

            let (part, run) = slot[2].pin(&row.shortcut, theme::FONT_SIZE_SM, 400, MENU_FACE);
            pen.place(part, run, col, Align::TrailingCentered, theme::text_tertiary());
        }
        // Rows the body no longer has.
        text.hide_rows_from(count);
    }
}

/// Every row's top edge and height, in order, given the panel's top.
///
/// One walk, read by the painter's separators and highlight, by the sprite
/// placement, and by the hit test — the three consumers of a menu's vertical
/// rhythm, which each used to step it themselves. They differ only in the origin
/// they start from: the hit test works in window DIPs, the other two in
/// surface-local ones.
fn rows_laid(rows: &[MenuRow], top: f32) -> impl Iterator<Item = (usize, &MenuRow, f32, f32)> {
    let mut y = top + PANEL_PAD;
    rows.iter().enumerate().map(move |(i, r)| {
        let h = if r.separator { SEP } else { ROW };
        let at = y;
        y += h;
        (i, r, at, h)
    })
}

/// A row's highlight/content rect inside the panel `p`, at row top `y`.
fn row_rect(p: Rect, y: f32) -> Rect {
    Rect::from_xywh(p.left + PANEL_PAD, y, p.width() - PANEL_PAD * 2.0, ROW)
}

/// The leading icon's cell inside a row.
fn icon_cell(rr: Rect) -> Rect {
    Rect::from_xywh(rr.left + theme::SPACE_8, rr.top, theme::SPACE_16, ROW)
}

/// The column the label and the shortcut hint SHARE — the two are kept apart by
/// alignment, not by separate boxes, so there is one column and not two.
///
/// It starts after the icon on a row that has one, which is what keeps a label
/// from running underneath its own glyph.
fn label_col(rr: Rect, has_icon: bool) -> Rect {
    let left = rr.left + if has_icon { theme::SPACE_32 } else { theme::SPACE_12 };
    Rect::new(left, rr.top, rr.right - theme::SPACE_12, rr.bottom)
}

/// The em a menu row's leading icon is set at. The label and the hint take the
/// text ramp's own sizes.
const ICON_EM: f32 = 12.0;
/// The face a menu row's label and shortcut hint are set in.
const MENU_FACE: &str = "Segoe UI";

fn fg(row: &MenuRow) -> crate::Color {
    if row.danger {
        theme::bad()
    } else if row.enabled {
        theme::text()
    } else {
        theme::text_disabled()
    }
}

fn set(brush: &Brush, c: crate::Color) {
    brush.set_color(linear(c));
}

