use super::*;

/// Whether a shadow tints with its own colour or derives from what the visual
/// draws.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowSource {
    /// Tint the blurred silhouette with [`DropShadow::set_color`]. The colour is
    /// an 8-bit `Windows.UI.Color`, so it cannot exceed paper white.
    Color,
    /// Derive the shadow from the visual's own content instead of a flat tint.
    VisualContent,
}

/// A compositor-rendered Gaussian cast by a [`SpriteVisual`](crate::SpriteVisual).
///
/// The blur runs entirely in the compositor, so a shadow costs the app no
/// rasterization and nothing per frame — and at [`set_offset`](Self::set_offset)
/// zero it is a *glow* rather than a shadow. Unlike an effect-graph brush this
/// needs no D2D interop at all.
///
/// [`set_mask`](Self::set_mask) supplies the alpha the blur is taken from, so an
/// arbitrary shape — a curve's coverage captured through a visual surface — can
/// drive the halo.
#[derive(Clone)]
pub struct DropShadow(pub(crate) bindings::DropShadow);

impl DropShadow {
    /// Sets the Gaussian blur radius, in DIPs.
    pub fn set_blur_radius(&self, radius: f32) {
        self.0.SetBlurRadius(radius).unwrap();
    }

    /// Sets the tint applied to the blurred silhouette. 8-bit — see
    /// [`ShadowSource::Color`].
    pub fn set_color(&self, color: Color) {
        self.0.SetColor(color.0).unwrap();
    }

    /// Sets the brush whose alpha the blur is taken from.
    pub fn set_mask(&self, brush: &impl Brush) {
        self.0.SetMask(&brush.as_brush().0).unwrap();
    }

    /// Offsets the shadow from the visual. Zero makes it a centred glow.
    pub fn set_offset(&self, x: f32, y: f32, z: f32) {
        self.0.SetOffset(Vector3 { x, y, z }).unwrap();
    }

    /// Scales the shadow's alpha.
    pub fn set_opacity(&self, opacity: f32) {
        self.0.SetOpacity(opacity).unwrap();
    }

    /// Selects where the shadow's colour comes from.
    pub fn set_source(&self, source: ShadowSource) {
        let policy = match source {
            ShadowSource::Color => bindings::CompositionDropShadowSourcePolicy::Default,
            ShadowSource::VisualContent => {
                bindings::CompositionDropShadowSourcePolicy::InheritFromVisualContent
            }
        };
        let shadow2: bindings::IDropShadow2 = self.0.cast().unwrap();
        shadow2.SetSourcePolicy(policy).unwrap();
    }
}

impl Compositor {
    /// Creates a drop shadow. Attach it with
    /// [`SpriteVisual::set_shadow`](crate::SpriteVisual::set_shadow).
    pub fn create_drop_shadow(&self) -> DropShadow {
        bump_count(Count::Brush);
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        DropShadow(compositor.CreateDropShadow().unwrap())
    }
}
