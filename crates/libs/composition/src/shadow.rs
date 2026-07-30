//! Drop shadows: a compositor-side Gaussian (feature `system`).
//!
//! The blur happens in the compositor process from an alpha mask, so a shadow costs the
//! app no rasterization and no drawing surface. At zero offset it is a glow, and unlike
//! an effect graph it needs no Direct2D interop at all.

use super::*;

/// Where a shadow's silhouette comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowSource {
    /// The [mask](DropShadow::set_mask) if one is assigned, and **a rectangle the size of
    /// the visual if none is**. This is the platform default, and the "or a rectangle" half
    /// is the part that surprises: a shadow configured this way with no mask is a box
    /// shadow, not a missing one.
    MaskOrRectangle,
    /// A mask derived from the alpha of the visual's own brush, so the shadow matches
    /// whatever the visual actually paints.
    VisualAlpha,
}

impl From<ShadowSource> for bindings::CompositionDropShadowSourcePolicy {
    fn from(source: ShadowSource) -> Self {
        match source {
            ShadowSource::MaskOrRectangle => Self::Default,
            ShadowSource::VisualAlpha => Self::InheritFromVisualContent,
        }
    }
}

/// A blurred silhouette cast behind a [`SpriteVisual`].
///
/// Its colour is a `Windows.UI.Color` and therefore 8-bit — which is the right precision
/// for a shadow, since a shadow is darkness rather than light and never needs to exceed
/// paper white.
///
/// Every property here is animatable through [`Animatable`], and the shape of the shadow is
/// what it costs: a rectangular one is cheap, an assigned mask or
/// [`ShadowSource::VisualAlpha`] is expensive, and **animating the blur radius is
/// expensive** — a static blur is not.
///
/// A shadow is not clipped by the implicit clip a visual's size implies, but it *is* clipped
/// by an explicit [clip](Visual::set_clip) — so a shadow inside a clipped group is cut off
/// at the group, and one on an unclipped visual draws outside its bounds.
#[derive(Clone)]
pub struct DropShadow(pub(crate) bindings::DropShadow);

impl DropShadow {
    /// Sets the Gaussian's radius, in DIPs. Animatable as `"BlurRadius"`.
    pub fn set_blur_radius(&self, radius: f32) {
        self.0.SetBlurRadius(radius).unwrap();
    }

    /// Sets the shadow's colour.
    pub fn set_color(&self, color: Color) {
        self.0.SetColor(color.0).unwrap();
    }

    /// Sets the brush whose alpha is the shadow's silhouette. Pair it with
    /// [`ShadowSource::MaskOrRectangle`], which is the default.
    pub fn set_mask(&self, brush: &impl Brush) {
        self.0.SetMask(&brush.as_brush().0).unwrap();
    }

    /// Displaces the shadow from the visual it belongs to, in DIPs. Animatable as
    /// `"Offset"`; at zero the shadow is a glow.
    pub fn set_offset(&self, x: f32, y: f32, z: f32) {
        self.0.SetOffset(Vector3 { x, y, z }).unwrap();
    }

    /// Sets the shadow's opacity, in `0.0..=1.0`. Animatable as `"Opacity"`.
    pub fn set_opacity(&self, opacity: f32) {
        self.0.SetOpacity(opacity).unwrap();
    }

    /// Selects where the silhouette comes from.
    pub fn set_source(&self, source: ShadowSource) {
        let shadow: bindings::IDropShadow2 = self.0.cast().unwrap();
        shadow.SetSourcePolicy(source.into()).unwrap();
    }
}

impl SpriteVisual {
    /// Casts `shadow` behind this visual.
    pub fn set_shadow(&self, shadow: &DropShadow) {
        let visual: &Visual = self;
        let sprite: bindings::ISpriteVisual2 = visual.0.cast().unwrap();
        sprite.SetShadow(&shadow.0).unwrap();
    }
}

impl Compositor {
    /// Creates a drop shadow, black and unblurred until configured.
    pub fn create_drop_shadow(&self) -> DropShadow {
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        DropShadow(compositor.CreateDropShadow().unwrap())
    }
}
