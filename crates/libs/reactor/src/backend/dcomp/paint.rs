//! Per-node surface painting. Each node with chrome owns a `SpriteVisual` backed
//! by an FP16 `CompositionDrawingSurface` sized to its rect; this module ensures
//! that surface exists, resizes it when the node's size changes, and redraws it
//! **only when the node's own content/size/state changed** (its `dirty` flag).
//! Pure layout containers never get a surface. A small [`PaintCache`] holds a
//! single recolorable solid brush, shared across every node's surface (all
//! surfaces derive from one Direct2D device), so painting allocates nothing per
//! frame.

use super::bootstrap::Compositing;
use super::node::{linear, lerp_color, Arena, Node};
use super::*;
use crate::backend::ControlKind;
use crate::Color;
use windows_canvas_core::{
    bindings::ID2D1DeviceContext, ColorF, DrawingSession, Ellipse, Rect, RoundedRect, Vector2,
};
use windows_numerics::Matrix3x2;

/// Per-device paint resources. One recolorable brush, reused every frame across
/// all node surfaces (Direct2D brushes are shareable across device contexts of
/// the same device).
#[derive(Default)]
pub(crate) struct PaintCache {
    brush: Option<windows_canvas_core::Brush>,
}

impl PaintCache {
    /// Drop cached GPU resources (e.g. after device loss).
    pub fn invalidate(&mut self) {
        self.brush = None;
    }
}

/// Transparent clear color for premultiplied node surfaces.
const CLEAR: ColorF = ColorF::new(0.0, 0.0, 0.0, 0.0);

/// Walk the tree, repainting each dirty node's own surface. Returns `Err` on
/// device loss so the caller can drop and rebuild cached resources.
pub(crate) fn paint(
    comp: &Compositing,
    cache: &mut PaintCache,
    arena: &mut Arena,
    root: ControlId,
    scale: f32,
) -> windows_core::Result<()> {
    paint_node(comp, cache, arena, root, scale)
}

fn paint_node(
    comp: &Compositing,
    cache: &mut PaintCache,
    arena: &mut Arena,
    id: ControlId,
    scale: f32,
) -> windows_core::Result<()> {
    let needs = arena
        .get(id)
        .is_some_and(|n| n.has_chrome() || n.surf.is_some());
    if needs {
        let (w, h) = arena.get(id).map(|n| (n.rect.w, n.rect.h)).unwrap_or((0.0, 0.0));
        let pw = (w * scale).ceil() as i32;
        let ph = (h * scale).ceil() as i32;

        let has_surface = arena.get(id).is_some_and(|n| n.surf.is_some());
        if !has_surface {
            let container = arena.get(id).unwrap().container.clone();
            let surf = comp.new_surface(&container, pw, ph)?;
            if let Some(n) = arena.get_mut(id) {
                n.surf = Some(surf);
            }
        } else if let Some(n) = arena.get_mut(id)
            && let Some(s) = &mut n.surf
        {
            let _ = s.resize(pw, ph);
        }
        if let Some(n) = arena.get(id)
            && let Some(s) = &n.surf
        {
            s.set_dip_size(w, h);
        }

        let dirty = arena.get(id).is_some_and(|n| n.dirty);
        if dirty && w > 0.0 && h > 0.0 {
            draw_surface(comp, cache, arena, id, scale)?;
            if let Some(n) = arena.get_mut(id) {
                n.dirty = false;
            }
        }
    }

    let children = arena.get(id).map(|n| n.children.clone()).unwrap_or_default();
    for c in children {
        paint_node(comp, cache, arena, c, scale)?;
    }
    Ok(())
}

/// Redraw one node's surface: its own chrome at local (0,0)-origin DIP coords.
fn draw_surface(
    comp: &Compositing,
    cache: &mut PaintCache,
    arena: &mut Arena,
    id: ControlId,
    scale: f32,
) -> windows_core::Result<()> {
    let interop = arena.get(id).unwrap().surf.as_ref().unwrap().interop.clone();

    let mut offset = crate::system_bindings::POINT::default();
    comp.device_lost.set(false);
    let ctx: ID2D1DeviceContext = unsafe { interop.BeginDraw(None, &mut offset)? };

    let session = DrawingSession::new_borrowed(&ctx, &comp.device_lost);
    // Grayscale text AA is mandatory on premultiplied/transparent surfaces.
    session.set_grayscale_text_antialiasing();
    // pixel = dip * scale + atlas offset (uniform scale keeps strokes/text crisp).
    session.set_transform(&Matrix3x2 {
        m11: scale,
        m12: 0.0,
        m21: 0.0,
        m22: scale,
        m31: offset.x as f32,
        m32: offset.y as f32,
    });
    session.clear(CLEAR);

    if cache.brush.is_none() {
        cache.brush = session.create_solid_brush(ColorF::BLACK).ok();
    }
    if let Some(brush) = &cache.brush {
        let node = arena.get(id).unwrap();
        // Local rect: the node's own box at the surface origin.
        let local = Rect::from_xywh(0.0, 0.0, node.rect.w, node.rect.h);
        paint_chrome(&session, brush, node, local);
    }

    unsafe { interop.EndDraw()? };
    Ok(())
}

/// Draw a node's own chrome into its surface (no recursion — children own their
/// own surfaces).
fn paint_chrome(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
) {
    match node.kind {
        ControlKind::Button => paint_button(session, brush, node, rect),
        ControlKind::TextBlock => paint_text(session, brush, node, rect),
        ControlKind::Rectangle => {
            fill_and_stroke(session, brush, node, rect, node.paint.corner_radius)
        }
        ControlKind::Ellipse => paint_ellipse(session, brush, node, rect),
        ControlKind::Line => paint_line(session, brush, node, rect),
        _ => {
            // Border / StackPanel / Grid / Canvas / ScrollViewer: bg + border box.
            fill_and_stroke(session, brush, node, rect, node.paint.corner_radius)
        }
    }
}

/// Fill a node's background and stroke its border, honouring corner radius.
fn fill_and_stroke(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
    radius: f32,
) {
    if let Some(bg) = node.paint.background {
        brush.set_color(linear(bg));
        if radius > 0.0 {
            session.fill_rounded_rect(&RoundedRect::uniform(rect, radius), brush);
        } else {
            session.fill_rect(&rect, brush);
        }
    }
    if let (Some(bc), t) = (node.paint.border_brush, node.paint.border_thickness)
        && t > 0.0
    {
        brush.set_color(linear(bc));
        let inset = inset_rect(rect, t / 2.0);
        if radius > 0.0 {
            session.draw_rounded_rect(&RoundedRect::uniform(inset, radius), brush, t);
        } else {
            session.draw_rect(&inset, brush, t);
        }
    }
}

fn paint_button(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
) {
    let radius = node.paint.corner_radius.max(6.0);
    // Base palette by variant; hover lightens, press darkens (spring-driven).
    let (base, hover, press) = match node.paint.style_variant {
        1 => (
            Color::rgb(0x0E, 0xA5, 0xE9),
            Color::rgb(0x38, 0xB6, 0xF0),
            Color::rgb(0x0B, 0x86, 0xBD),
        ),
        _ => (
            Color::rgb(0x3A, 0x3A, 0x3D),
            Color::rgb(0x4A, 0x4A, 0x4E),
            Color::rgb(0x2C, 0x2C, 0x2F),
        ),
    };
    let mut c = lerp_color(linear(base), linear(hover), node.hover.x.clamp(0.0, 1.0));
    c = lerp_color(c, linear(press), node.press.x.clamp(0.0, 1.0));
    brush.set_color(c);
    session.fill_rounded_rect(&RoundedRect::uniform(rect, radius), brush);

    // Centered label.
    if let Some(layout) = &node.text_layout {
        let fg = node.paint.foreground.unwrap_or(Color::rgb(0xF2, 0xF2, 0xF4));
        brush.set_color(linear(fg));
        let (tw, th) = layout.measure().unwrap_or((0.0, 0.0));
        let ox = rect.left + (rect.width() - tw) / 2.0;
        let oy = rect.top + (rect.height() - th) / 2.0;
        session.draw_text_layout(Vector2::new(ox, oy), layout, brush);
    }
}

fn paint_text(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
) {
    let Some(layout) = &node.text_layout else { return };
    let fg = node.paint.foreground.unwrap_or(Color::rgb(0xEA, 0xEA, 0xEC));
    brush.set_color(linear(fg));
    session.draw_text_layout(Vector2::new(rect.left, rect.top), layout, brush);
}

fn paint_ellipse(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
) {
    let e = Ellipse::new(
        Vector2::new((rect.left + rect.right) / 2.0, (rect.top + rect.bottom) / 2.0),
        rect.width() / 2.0,
        rect.height() / 2.0,
    );
    if let Some(fill) = node.paint.fill {
        brush.set_color(linear(fill));
        session.fill_ellipse(&e, brush);
    }
    if let (Some(stroke), t) = (node.paint.stroke, node.paint.stroke_thickness)
        && t > 0.0
    {
        brush.set_color(linear(stroke));
        session.draw_ellipse(&e, brush, t);
    }
}

fn paint_line(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
) {
    let l = node.paint.line;
    let t = if node.paint.stroke_thickness > 0.0 {
        node.paint.stroke_thickness
    } else {
        1.0
    };
    let color = node.paint.stroke.unwrap_or(Color::rgb(0x80, 0x80, 0x80));
    brush.set_color(linear(color));
    session.draw_line(
        Vector2::new(rect.left + l.x1 as f32, rect.top + l.y1 as f32),
        Vector2::new(rect.left + l.x2 as f32, rect.top + l.y2 as f32),
        brush,
        t,
    );
}

fn inset_rect(r: Rect, by: f32) -> Rect {
    Rect::new(r.left + by, r.top + by, r.right - by, r.bottom - by)
}
