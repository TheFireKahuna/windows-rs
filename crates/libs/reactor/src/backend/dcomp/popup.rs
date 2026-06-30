//! The popup / overlay layer — the shared top-level surface that backs Select
//! dropdowns, DropDownButton / SplitButton menus and MenuFlyouts. A [`Popup`] is
//! a single FP16 surface z-promoted above the whole reactor tree (a child visual
//! of the compositor root, per the parity spec's sanctioned alternative to a
//! second HWND), anchored under its trigger with monitor/edge flip, light-
//! dismissed on outside-click or Escape, and keyboard-navigable (Up/Down/Enter/
//! Esc). It opens with a `SNAPPY` scale+fade on its root visual and idles with no
//! polling — the host message loop drives its open spring via the frame tick.

use super::bootstrap::{Compositing, NodeSurface};
use super::node::{linear, MenuRow, Spring};
use super::theme;
use crate::backend::ControlId;
use crate::system_bindings::{ContainerVisual, IVisual, POINT};
use windows_canvas_core::{
    bindings::ID2D1DeviceContext, Brush, ColorF, DrawingSession, FontWeight, ParagraphAlignment,
    Rect, RoundedRect, TextAlignment, TextFormat, Vector2,
};
use windows_core::Interface;
use windows_numerics::{Matrix3x2, Vector3};

/// Shadow bleed margin baked into the surface around the drawn panel (DIPs).
const MARGIN: f32 = 10.0;
/// One command/selection row height (DIPs).
const ROW: f32 = theme::ROW_H;
/// Separator row height (DIPs).
const SEP: f32 = theme::SPACE_8 + theme::BORDER_W;
const PANEL_PAD: f32 = theme::SPACE_4;

/// A live popup surface. The backend owns at most one (`Option<Popup>`).
pub(crate) struct Popup {
    /// The control that opened this popup (its events fire on selection).
    pub owner: ControlId,
    /// `true` = ComboBox selection list; `false` = command menu / dropdown.
    pub combo: bool,
    container: ContainerVisual,
    vis: IVisual,
    surf: NodeSurface,
    items: Vec<MenuRow>,
    /// Drawn-panel rect in window DIPs (excludes the shadow margin).
    panel: Rect,
    /// Currently highlighted row index (`usize::MAX` = none).
    pub hovered: usize,
    open: Spring,
    px: f32,
}

impl Popup {
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
    ) -> windows_core::Result<Self> {
        // Panel size from row metrics + a generous width tied to the trigger.
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
        let panel = Rect::from_xywh(x, y, w, h);

        let scale = comp.scale();
        let surf_w = w + MARGIN * 2.0;
        let surf_h = h + MARGIN * 2.0;
        let (container, surf) =
            comp.new_overlay((surf_w * scale).ceil() as i32, (surf_h * scale).ceil() as i32)?;
        surf.set_dip_size(surf_w, surf_h);
        let vis: IVisual = container.cast()?;
        // Position the surface so the panel lands at (x, y) (account for margin).
        vis.SetOffset(Vector3::new(x - MARGIN, y - MARGIN, 0.0))?;
        vis.SetSize(Vector2::new(surf_w, surf_h))?;

        let hovered = if combo && selected >= 0 {
            selected as usize
        } else {
            usize::MAX
        };
        let mut popup = Self {
            owner,
            combo,
            container,
            vis,
            surf,
            items,
            panel,
            hovered,
            open: Spring::new(0.0),
            px: scale,
        };
        popup.open.target = 1.0;
        popup.apply_anim();
        popup.draw(comp);
        Ok(popup)
    }

    /// Advance the open animation; returns `true` once settled.
    pub fn tick(&mut self, dt: f32) -> bool {
        let settled = self.open.step(dt);
        self.apply_anim();
        settled
    }

    /// Apply the open spring as a scale (0.96→1.0) + fade on the root visual.
    fn apply_anim(&self) {
        let t = self.open.x.clamp(0.0, 1.0);
        let s = 0.96 + 0.04 * t;
        let _ = self.vis.SetOpacity(t);
        let _ = self.vis.SetScale(Vector3::new(s, s, 1.0));
    }

    /// Detach the overlay surface from the tree (closing the popup).
    pub fn dismiss(self, comp: &Compositing) {
        comp.remove_overlay(&self.container);
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

        // Soft drop shadow: a black rounded rect bled below/right.
        for (i, a) in [(6.0_f32, 0.10_f32), (3.0, 0.16), (1.0, 0.24)] {
            let s = Rect::new(p.left - i + 1.0, p.top - i + 3.0, p.right + i + 1.0, p.bottom + i + 3.0);
            set(brush, theme::b(a));
            session.fill_rounded_rect(&RoundedRect::uniform(s, theme::RADIUS_MD + i), brush);
        }

        // Raised surface + hairline.
        set(brush, theme::SURFACE_RAISED);
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
                draw_text(session, brush, &row.shortcut, lr, "Segoe UI", theme::FONT_SIZE_SM, 400, theme::TEXT_TERTIARY, TextAlignment::Trailing);
            }
            ry += ROW;
        }
    }
}

fn fg(row: &MenuRow) -> crate::Color {
    if row.danger {
        theme::BAD
    } else if row.enabled {
        theme::TEXT
    } else {
        theme::TEXT_DISABLED
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
