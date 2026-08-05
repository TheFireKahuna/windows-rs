//! Multi-object constructions a retained tree repeats: a masked sprite, a clipped group,
//! and a captured subtree (feature `system`).
//!
//! Each creates several composition objects and wires their properties together, so a call
//! site names the construction instead of respelling the sequence. The individual factories
//! stay available for anything these three do not cover.

use super::*;

impl Compositor {
    /// Creates a single sprite visual that paints `paint`'s colour through `mask`'s alpha.
    ///
    /// Coverage and colour stay separate brushes: `mask` supplies 8-bit coverage, and
    /// `paint` supplies the colour, which for wide-gamut or above-paper-white content is a
    /// float surface a colour brush cannot express.
    ///
    /// The sprite is returned unsized: a `SpriteVisual` paints its own bounds, so it draws
    /// nothing until [`set_size`](Visual::set_size) or
    /// [`fill_parent`](Visual::fill_parent) gives it some.
    pub fn masked_sprite(&self, mask: &impl Brush, paint: &impl Brush) -> SpriteVisual {
        let brush = self.create_mask_brush();
        brush.set_mask(mask);
        brush.set_source(paint);
        let sprite = self.create_sprite_visual();
        sprite.set_brush(&brush);
        sprite
    }

    /// Creates a container visual whose subtree is clipped by `clip`.
    ///
    /// The clip is a property of the group, so clipping adds no visual to the tree.
    pub fn clipped_group(&self, clip: &impl Clip) -> ContainerVisual {
        let group = self.create_container_visual();
        group.set_clip(Some(clip));
        group
    }

    /// Creates a brush that paints with `source`'s already-composed subtree rather than
    /// with pixels something drew.
    ///
    /// A subtree captured this way becomes reusable content — a mask, a layer held alive
    /// for an exit transition, a cached layer — without the app rasterizing it again.
    ///
    /// `size` is the DIP box to capture and `scale` the DIP→pixel factor it is rasterized
    /// at, so the captured region is `size * scale` in the source's own coordinate space.
    /// **A visual surface captures content, not the source visual's own transform.**
    /// Scaling the source visual does not scale what lands in the surface, so the scale
    /// lives on the geometry inside it — on the shape — and `scale` must match that.
    ///
    /// The brush is set to stretch [`Fill`](crate::Stretch::Fill) from a `(0, 0)`
    /// alignment, because the caller has just declared the surface's size: composition's
    /// own defaults would letterbox it and centre it inside the sprite instead.
    pub fn capture(&self, source: &Visual, size: Vector2, scale: f32) -> Captured {
        let surface = self.create_visual_surface();
        surface.set_source_visual(source);
        let brush = self.create_surface_brush(&surface);
        brush.set_stretch(Stretch::Fill);
        brush.set_alignment_ratio(0.0, 0.0);
        let captured = Captured { surface, brush };
        captured.resize(size, scale);
        captured
    }
}

/// A captured subtree: the surface that reads it, and the brush that paints with it.
///
/// A capture's extent must be restated whenever the box it stands for moves, and only the
/// surface accepts that. The two travel together so a holder can both paint with the
/// capture and [`resize`](Self::resize) it; a brush on its own can only paint.
pub struct Captured {
    /// The surface reading the source subtree, and the only place its extent is set.
    pub surface: CompositionVisualSurface,
    /// The brush painting a visual with the captured content.
    pub brush: CompositionSurfaceBrush,
}

impl Captured {
    /// Sets the captured region to `size * scale`, in the source's own coordinate space.
    ///
    /// A capture's extent is written only here, by [`Compositor::capture`] and by every
    /// resize, so the origin and the `size * scale` convention have one definition. A stale
    /// extent mis-sizes the captured content without reporting an error.
    ///
    /// The scale belongs here and not on the source visual because a visual surface
    /// captures *content*: scaling the source changes nothing about what lands in the
    /// surface, so geometry inside it is scaled by the same factor separately.
    pub fn resize(&self, size: Vector2, scale: f32) {
        self.surface.set_source_offset(Vector2 { x: 0.0, y: 0.0 });
        self.surface.set_source_size(Vector2 {
            x: size.x * scale,
            y: size.y * scale,
        });
    }
}
