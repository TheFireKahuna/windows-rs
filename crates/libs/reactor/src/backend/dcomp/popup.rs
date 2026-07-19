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
    items: Vec<MenuRow>,
    /// Drawn-panel rect in window DIPs (excludes the shadow margin).
    panel: Rect,
    /// The trigger/field rect this popup is anchored under (window DIPs).
    anchor: Rect,
    /// The window viewport (w, h) DIPs the panel is clamped/flipped within.
    window: (f32, f32),
    /// Currently highlighted row index (`usize::MAX` = none).
    pub hovered: usize,
    px: f32,
}

impl Popup {
    /// Panel + surface geometry for `items` anchored under `anchor`, clamped /
    /// flipped to fit `window`. Returns `(panel_rect, surf_w, surf_h)` in DIPs.
    fn layout(items: &[MenuRow], anchor: Rect, window: (f32, f32)) -> (Rect, f32, f32) {
        let h: f32 = items
            .iter()
            .map(|r| if r.separator { SEP } else { ROW })
            .sum::<f32>()
            + PANEL_PAD * 2.0;
        let w = anchor.width().max(200.0).min(360.0);
        // Anchor below the trigger; flip above on the bottom monitor edge.
        let mut y = anchor.bottom + theme::SPACE_4;
        if y + h > window.1 - theme::SPACE_4 {
            y = (anchor.top - h - theme::SPACE_4).max(theme::SPACE_4);
        }
        let x = anchor.left.min(window.0 - w - theme::SPACE_4).max(theme::SPACE_4);
        (Rect::from_xywh(x, y, w, h), w + MARGIN * 2.0, h + MARGIN * 2.0)
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
        owner: ControlId,
        items: Vec<MenuRow>,
        anchor: Rect,
        window: (f32, f32),
        combo: bool,
        selected: i32,
        suggest: bool,
    ) -> windows_core::Result<Self> {
        let (panel, surf_w, surf_h) = Self::layout(&items, anchor, window);
        let (container, surf) = Self::build_surface(comp, panel, surf_w, surf_h)?;
        let hovered = if combo && selected >= 0 {
            selected as usize
        } else {
            usize::MAX
        };
        let popup = Self {
            owner,
            combo,
            suggest,
            container,
            surf,
            items,
            panel,
            anchor,
            window,
            hovered,
            px: comp.scale(),
        };
        popup.reveal(comp);
        popup.draw(comp);
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
    pub fn update_items(&mut self, comp: &Compositing, items: Vec<MenuRow>) {
        let resized = items.len() != self.items.len();
        self.items = items;
        if self.hovered != usize::MAX && self.hovered >= self.items.len() {
            self.hovered = usize::MAX;
        }
        if resized {
            let (panel, surf_w, surf_h) = Self::layout(&self.items, self.anchor, self.window);
            if let Ok((container, surf)) = Self::build_surface(comp, panel, surf_w, surf_h) {
                comp.remove_overlay(&self.container);
                self.container = container;
                self.surf = surf;
                self.panel = panel;
            }
        }
        self.draw(comp);
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
        for (i, r) in self.items.iter().enumerate() {
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
        self.items.get(index).map(|r| {
            if r.tag.is_empty() {
                r.text.clone()
            } else {
                r.tag.clone()
            }
        })
    }

    /// Move the highlight by `delta`, skipping separators/disabled rows.
    pub fn move_highlight(&mut self, delta: i32, comp: &Compositing) {
        let n = self.items.len() as i32;
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
            if let Some(r) = self.items.get(i as usize)
                && !r.separator
                && r.enabled
            {
                self.hovered = i as usize;
                break;
            }
        }
        self.draw(comp);
    }

    /// Set the highlighted row (pointer hover) and repaint if it changed.
    pub fn set_hovered(&mut self, idx: Option<usize>, comp: &Compositing) {
        let new = idx.unwrap_or(usize::MAX);
        if new != self.hovered {
            self.hovered = new;
            self.draw(comp);
        }
    }

    /// Redraw the panel surface (shadow + chrome + rows + highlight).
    fn draw(&self, comp: &Compositing) {
        let mut offset = POINT::default();
        comp.device_lost.set(false);
        let ctx: ID2D1DeviceContext = match unsafe { self.surf.interop.BeginDraw(None, &mut offset) }
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let session = DrawingSession::new_borrowed(&ctx, &comp.device_lost);
        session.set_grayscale_text_antialiasing();
        session.set_transform(&Matrix3x2 {
            m11: self.px,
            m12: 0.0,
            m21: 0.0,
            m22: self.px,
            m31: offset.x as f32,
            m32: offset.y as f32,
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

        let mut ry = p.top + PANEL_PAD;
        for (i, row) in self.items.iter().enumerate() {
            if row.separator {
                set(brush, theme::stroke_divider());
                let y = ry + SEP / 2.0;
                session.draw_line(
                    Vector2::new(p.left + theme::SPACE_8, y),
                    Vector2::new(p.right - theme::SPACE_8, y),
                    brush,
                    theme::BORDER_W,
                );
                ry += SEP;
                continue;
            }
            let rr = Rect::from_xywh(p.left + PANEL_PAD, ry, p.width() - PANEL_PAD * 2.0, ROW);
            if i == self.hovered {
                set(brush, if self.combo { theme::accent_fill() } else { theme::w(0.06) });
                session.fill_rounded_rect(&RoundedRect::uniform(rr, theme::RADIUS_SM), brush);
            }
            // Leading icon.
            let mut text_left = rr.left + theme::SPACE_12;
            if row.icon != 0
                && let Some(g) = char::from_u32(row.icon).map(|c| c.to_string())
            {
                let ir = Rect::from_xywh(rr.left + theme::SPACE_8, rr.top, theme::SPACE_16, ROW);
                draw_text(session, brush, &g, ir, theme::FONT_ICON, 12.0, 400, fg(row), TextAlignment::Center);
                text_left = rr.left + theme::SPACE_32;
            }
            // Label.
            let lr = Rect::new(text_left, rr.top, rr.right - theme::SPACE_12, rr.bottom);
            draw_text(session, brush, &row.text, lr, "Segoe UI", theme::FONT_SIZE_MD, 400, fg(row), TextAlignment::Leading);
            // Trailing shortcut hint.
            if !row.shortcut.is_empty() {
                draw_text(session, brush, &row.shortcut, lr, "Segoe UI", theme::FONT_SIZE_SM, 400, theme::text_tertiary(), TextAlignment::Trailing);
            }
            ry += ROW;
        }
    }
}

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

#[allow(clippy::too_many_arguments)]
fn draw_text(
    session: &DrawingSession,
    brush: &Brush,
    s: &str,
    rect: Rect,
    family: &str,
    size: f32,
    weight: u16,
    color: crate::Color,
    align: TextAlignment,
) {
    if s.is_empty() {
        return;
    }
    let Ok(fmt) = TextFormat::with_weight(family, size, FontWeight(weight as i32)) else {
        return;
    };
    let fmt = fmt
        .with_alignment(align)
        .with_paragraph_alignment(ParagraphAlignment::Center);
    set(brush, color);
    session.draw_text(s, &fmt, &rect, brush);
}
