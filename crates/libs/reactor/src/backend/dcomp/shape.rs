//! Vector chrome parts — resolution-independent geometry in the FP16 pipe.
//!
//! A `CompositionSurfaceBrush` does not paint as a shape fill or stroke, and the
//! only brush a shape *does* take — `CompositionColorBrush` — carries an 8-bit
//! `Windows.UI.Color`, which would drop chrome out of the display-mapped FP16
//! pipeline every other surface stays in. So the shape is always the MASK
//! (opaque white, alpha only) and an FP16 surface is the SOURCE, combined
//! through a [`CompositionMaskBrush`] — the same construction as the knob's
//! value arc, generalized to the rounded rect control chrome is made of.
//!
//! Reach for this over an [`super::parts::Part`] nine-grid when the GEOMETRY
//! ITSELF MOVES. `Size` and `CornerRadius` are compositor properties here, so a
//! pill can morph to a circle and a badge can grow with no repaint and no
//! re-raster. When the geometry is static the atlas part is cheaper — one
//! rasterized source shared by every control of that height, against the ~7 COM
//! objects an instance costs here — so chrome that only ever changes COLOUR
//! belongs there, not here.
//!
//! A rounded rect covers the dot ornament too: at `w == h` and
//! `radius = w / 2` it *is* a circle, so there is no second geometry type.

use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

use super::bootstrap::Compositing;
use super::node::Node;
use super::parts::build_solid_surface;
use crate::system_bindings::{
    Color as UiColor, CompositionBrush, CompositionMaskBrush, CompositionObject,
    CompositionRoundedRectangleGeometry, CompositionShape, CompositionSurfaceBrush,
    CompositionVisualSurface, ICompositionObject, ICompositionSpriteShape, ICompositionSurface,
    ICompositor2, ICompositor5, ICompositorWithVisualSurface, IVisual, ShapeVisual, SpriteVisual,
    Visual,
};

/// How the mask shape paints its geometry.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Stroke {
    /// Filled — the whole rounded rect is opaque in the mask.
    Fill,
    /// Outlined at this DIP thickness (quantized to the same 1/4-DIP grid the
    /// atlas uses, so a hairline border cannot jitter between syncs).
    Width(u32),
}

impl Stroke {
    pub(crate) fn width(w: f32) -> Self {
        Self::Width((w.max(0.0) * 4.0).round() as u32)
    }
    fn dips(self) -> f32 {
        match self {
            Self::Fill => 0.0,
            Self::Width(q) => q as f32 / 4.0,
        }
    }
}

/// One vector chrome sprite: a rounded rect whose geometry animates on the
/// compositor, coloured through the FP16 pipe.
///
/// Every setter is change-gated against the last written value, so a sync that
/// changes nothing issues no COM calls at all.
pub(crate) struct ShapePart {
    sprite: SpriteVisual,
    vis: IVisual,
    /// The mask geometry. Held as both faces: the typed one for `SetSize` /
    /// `SetCornerRadius`, the `CompositionObject` for `StartAnimation` when a
    /// caller springs the geometry.
    geo: CompositionRoundedRectangleGeometry,
    geo_obj: CompositionObject,
    /// Off-tree white shape whose alpha the mask brush reads.
    _mask_visual: ShapeVisual,
    mask_vis: IVisual,
    visual_surface: CompositionVisualSurface,
    mask_brush: CompositionMaskBrush,
    /// The FP16 source currently bound, and the `(colour, scale)` it was built
    /// for — rebuilt only when one of those actually changes.
    _source: Option<CompositionSurfaceBrush>,
    source_for: Option<([u32; 4], u32)>,
    // ── change gates ──
    size: Option<(f32, f32)>,
    radius: Option<f32>,
    offset: Option<(f32, f32)>,
    opacity: Option<f32>,
}

/// Quantize a colour to raw bits so the source gate is `Eq` without float
/// caveats (a `NaN` channel compares equal to itself and cannot loop).
fn color_bits(c: crate::Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

impl ShapePart {
    /// Build a vector part parented into `container`, at the top of its
    /// children. Returns `None` if any compositor object could not be made —
    /// the caller falls back to its painted chrome rather than rendering
    /// nothing.
    pub(crate) fn new(comp: &Compositing, node: &Node, stroke: Stroke) -> Option<Self> {
        let sprite = comp.new_sprite().ok()?;
        let vis: IVisual = sprite.cast().ok()?;
        let compositor = sprite.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
        let c5 = compositor.cast::<ICompositor5>().ok()?;
        let c2 = compositor.cast::<ICompositor2>().ok()?;
        let cvs = compositor.cast::<ICompositorWithVisualSurface>().ok()?;

        // ── Mask geometry + shape (opaque white; only its alpha is read) ──
        let geo = c5.CreateRoundedRectangleGeometry().ok()?;
        let geo_obj: CompositionObject = geo.cast().ok()?;
        let shape_c = c5.CreateSpriteShapeWithGeometry(&geo).ok()?;
        let shape: ICompositionSpriteShape = shape_c.cast().ok()?;
        let white = compositor
            .CreateColorBrushWithColor(UiColor { a: 255, r: 255, g: 255, b: 255 })
            .ok()?;
        let white_cb: CompositionBrush = white.cast().ok()?;
        match stroke {
            Stroke::Fill => shape.SetFillBrush(&white_cb).ok()?,
            Stroke::Width(_) => {
                shape.SetStrokeBrush(&white_cb).ok()?;
                shape.SetStrokeThickness(stroke.dips()).ok()?;
            }
        }

        let mask_visual = c5.CreateShapeVisual().ok()?;
        let mask_vis: IVisual = mask_visual.cast().ok()?;
        mask_visual
            .Shapes()
            .ok()?
            .Append(&shape_c.cast::<CompositionShape>().ok()?)
            .ok()?;

        // ── Live snapshot of the mask → a surface brush ──
        let visual_surface = cvs.CreateVisualSurface().ok()?;
        visual_surface.SetSourceVisual(&mask_visual.cast::<Visual>().ok()?).ok()?;
        visual_surface.SetSourceOffset(Vector2::new(0.0, 0.0)).ok()?;

        let mask_surf = compositor
            .CreateSurfaceBrushWithSurface(&visual_surface.cast::<ICompositionSurface>().ok()?)
            .ok()?;
        let mask_brush = c2.CreateMaskBrush().ok()?;
        mask_brush.SetMask(&mask_surf.cast::<CompositionBrush>().ok()?).ok()?;
        sprite.SetBrush(&mask_brush.cast::<CompositionBrush>().ok()?).ok()?;

        node.container
            .Children()
            .ok()?
            .InsertAtTop(&sprite.cast::<Visual>().ok()?)
            .ok()?;

        Some(Self {
            sprite,
            vis,
            geo,
            geo_obj,
            _mask_visual: mask_visual,
            mask_vis,
            visual_surface,
            mask_brush,
            _source: None,
            source_for: None,
            size: None,
            radius: None,
            offset: None,
            opacity: None,
        })
    }

    /// The geometry as a `CompositionObject`, for a caller that wants to spring
    /// `Size` or `CornerRadius` rather than snap it.
    pub(crate) fn geometry(&self) -> &CompositionObject {
        &self.geo_obj
    }

    /// Resize the part. Writes through to the sprite, the off-tree mask visual
    /// and the visual surface's source size together — they must agree or the
    /// mask samples the wrong region and the chrome clips.
    pub(crate) fn resize(&mut self, w: f32, h: f32) {
        if self.size == Some((w, h)) {
            return;
        }
        let v = Vector2::new(w, h);
        let _ = self.vis.SetSize(v);
        let _ = self.mask_vis.SetSize(v);
        let _ = self.visual_surface.SetSourceSize(v);
        let _ = self.geo.SetSize(v);
        self.size = Some((w, h));
    }

    /// Set the corner radius (DIPs, uniform). `radius >= min(w, h) / 2` on a
    /// square part is a circle — the dot ornament asks for exactly that.
    pub(crate) fn set_radius(&mut self, r: f32) {
        if self.radius == Some(r) {
            return;
        }
        let _ = self.geo.SetCornerRadius(Vector2::new(r, r));
        self.radius = Some(r);
    }

    /// Position within the parent (DIPs).
    pub(crate) fn place(&mut self, x: f32, y: f32) {
        if self.offset == Some((x, y)) {
            return;
        }
        let _ = self.vis.SetOffset(Vector3::new(x, y, 0.0));
        self.offset = Some((x, y));
    }

    /// Bind the FP16 colour source. Rebuilds the source surface only when the
    /// authored colour or the DIP→px scale actually changes; a recolour is the
    /// one call, with no geometry work and no repaint of anything.
    pub(crate) fn set_color(&mut self, comp: &Compositing, c: crate::Color, scale: f32) {
        let want = (color_bits(c), scale.to_bits());
        if self.source_for == Some(want) {
            return;
        }
        let Some(src) = build_solid_surface(comp, c, scale) else { return };
        let Ok(cb) = src.cast::<CompositionBrush>() else { return };
        if self.mask_brush.SetSource(&cb).is_ok() {
            self._source = Some(src);
            self.source_for = Some(want);
        }
    }

    /// Set opacity directly (no animation). Callers that want a fade drive the
    /// sprite through the shared animate helpers instead.
    pub(crate) fn set_opacity(&mut self, o: f32) {
        if self.opacity == Some(o) {
            return;
        }
        let _ = self.vis.SetOpacity(o);
        self.opacity = Some(o);
    }

    /// The sprite, for a caller attaching a compositor animation to it.
    pub(crate) fn sprite(&self) -> &SpriteVisual {
        &self.sprite
    }
}
