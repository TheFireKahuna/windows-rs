//! Constructions rather than factories (feature `system`).
//!
//! A wrapper that is 1:1 with WinRT classes makes every call site spell out the same
//! sequence — create, set eight properties, parent, clip. The three constructions here
//! are the ones a retained tree repeats until they are worth naming, and naming them is
//! what stops a subtle one (a masked sprite, a capture) from being written five slightly
//! different ways.
//!
//! The raw factories stay available; nothing that has one of these to reach for should be
//! using them.

use super::*;

impl Compositor {
    /// A sprite that paints `paint`'s colour through `mask`'s alpha — the universal
    /// retained construction, as **one** visual.
    ///
    /// Coverage and colour are separate brushes because they have to be: coverage comes
    /// from an 8-bit alpha source, and colour comes from whatever can carry the value,
    /// which for wide-gamut or above-paper-white content is a float surface no colour
    /// brush could express. Stacking source-over is associative, so a sprite tree built
    /// this way is exactly equivalent to one surface drawing the same layers in
    /// sequence — at lower cost, and with no effect graph anywhere.
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

    /// A container visual whose subtree is clipped.
    ///
    /// The clip is a property of the group and costs no extra visual, which is why
    /// clipping is not something a caller composes out of nested sprites.
    pub fn clipped_group(&self, clip: &impl Clip) -> ContainerVisual {
        let group = self.create_container_visual();
        group.set_clip(Some(clip));
        group
    }

    /// A brush that paints with an already-composed subtree, rather than with pixels
    /// something drew.
    ///
    /// This is how a subtree becomes reusable content — a mask, a ghost held alive for an
    /// exit transition, a cached layer — without being rasterized again by the app.
    ///
    /// `size` is the DIP box to capture and `scale` the DIP→pixel factor it is rasterized
    /// at, so the captured region is `size * scale` in the source's own coordinate space.
    /// That factor is not a display property this crate can infer, and the reason is worth
    /// stating: **a visual surface captures content, not the source visual's own
    /// transform.** Scaling the source visual therefore does not scale what lands in the
    /// surface; the scale has to live on the geometry inside it — on the shape — which is
    /// what `scale` here has to agree with.
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
/// The two are kept together because a capture's extent has to be **restated whenever the
/// box it stands for moves**, and only the surface can be told. A caller holding the brush
/// alone can paint with a capture but can never correct one, which is the shape that makes
/// a resized path draw at its old size.
pub struct Captured {
    pub surface: CompositionVisualSurface,
    pub brush: CompositionSurfaceBrush,
}

impl Captured {
    /// States the region captured, in the source's own coordinate space.
    ///
    /// **The one place a capture's extent is written**, called by the construction above
    /// and by every resize. Two places would be two conventions, and the failure mode for
    /// getting the second one wrong is silent mis-sizing rather than an error — a curve
    /// that draws at the wrong scale, on the frame after a window edge moved.
    ///
    /// The scale is here and not on the source visual because a visual surface captures
    /// *content*: scaling the source changes nothing about what lands in the surface, so
    /// whatever geometry is inside it must be scaled by the same factor separately.
    pub fn resize(&self, size: Vector2, scale: f32) {
        self.surface.set_source_offset(Vector2 { x: 0.0, y: 0.0 });
        self.surface.set_source_size(Vector2 {
            x: size.x * scale,
            y: size.y * scale,
        });
    }
}
