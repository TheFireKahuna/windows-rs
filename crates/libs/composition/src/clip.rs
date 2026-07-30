//! Clips: what a visual's subtree is allowed to draw inside (feature `system`).
//!
//! A clip is not a brush, so it costs no brush slot, no second visual and no capture —
//! which is what makes it the only way to round the corners of a sprite whose one brush
//! is already spent on a mask. Its values are animatable, so a reveal is a clip whose
//! inset animates rather than a visual whose size does.

use super::*;

/// The base type shared by every composition clip.
///
/// A [`Clip`] can be turned into one via [`Clip::as_clip`] to apply it to a visual.
#[derive(Clone)]
pub struct CompositionClip(pub(crate) bindings::CompositionClip);

/// A clip that can be applied to a [`Visual`] and its subtree.
///
/// This trait is sealed: only the clip types in this crate implement it.
pub trait Clip: Sealed {
    /// Returns this clip as the shared [`CompositionClip`] base type.
    fn as_clip(&self) -> CompositionClip;
}

impl Visual {
    /// Sets (or, with `None`, clears) the clip applied to this visual and its subtree.
    ///
    /// A bare `None` has no clip type to infer, so use
    /// [`clear_clip`](Self::clear_clip) to remove a clip unconditionally; `None` here
    /// is for the conditional case, where the `Some` arm names the type.
    pub fn set_clip(&self, clip: Option<&impl Clip>) {
        let clip = clip.map(|clip| clip.as_clip().0);
        self.0.SetClip(clip.as_ref()).unwrap();
    }

    /// Removes any clip from this visual, leaving its subtree unclipped.
    pub fn clear_clip(&self) {
        self.0.SetClip(None).unwrap();
    }
}

/// A clip that insets each edge of the visual's own box.
///
/// Insets are measured inward from the visual's edges, so a clip written once keeps
/// following the visual as it resizes — the opposite of [`RectangleClip`], whose sides
/// are absolute. Every inset is animatable, and animating one is how a subtree is
/// revealed or hidden without touching its layout.
#[derive(Clone)]
pub struct InsetClip(pub(crate) bindings::InsetClip);

impl InsetClip {
    /// Sets all four insets, in DIPs inward from the visual's edges.
    pub fn set_insets(&self, left: f32, top: f32, right: f32, bottom: f32) {
        self.0.SetLeftInset(left).unwrap();
        self.0.SetTopInset(top).unwrap();
        self.0.SetRightInset(right).unwrap();
        self.0.SetBottomInset(bottom).unwrap();
    }
}

impl Sealed for InsetClip {}

impl Clip for InsetClip {
    fn as_clip(&self) -> CompositionClip {
        CompositionClip(self.0.cast().unwrap())
    }
}

/// A clip with absolute sides and per-corner radii — the rounding primitive.
///
/// Its sides are coordinates in the clipped visual's own space, **not** insets from its
/// edges, so a clip that must track the visual's size is rewritten on resize. In
/// exchange it carries corner radii, and rounding a sprite this way costs no brush slot.
///
/// **Corner radii animate only as scalars**, named `"TopLeftRadiusX"`,
/// `"TopLeftRadiusY"`, `"TopRightRadiusX"`, and so on. Animating a `Vector2` radius
/// property silently does nothing — it does not fail, it does not warn, the corner
/// simply never moves.
#[derive(Clone)]
pub struct RectangleClip(pub(crate) bindings::RectangleClip);

impl RectangleClip {
    /// Sets all four sides, in the clipped visual's own coordinate space.
    pub fn set_sides(&self, left: f32, top: f32, right: f32, bottom: f32) {
        self.0.SetLeft(left).unwrap();
        self.0.SetTop(top).unwrap();
        self.0.SetRight(right).unwrap();
        self.0.SetBottom(bottom).unwrap();
    }

    /// Sets every corner to the same radius.
    ///
    /// A radius is capped at half the box on each axis, so "fully rounded" is a
    /// stadium and never a circle-ended football: passing a large number gives a pill,
    /// not an ellipse.
    pub fn set_corner_radius(&self, radius: Vector2) {
        self.0.SetTopLeftRadius(radius).unwrap();
        self.0.SetTopRightRadius(radius).unwrap();
        self.0.SetBottomRightRadius(radius).unwrap();
        self.0.SetBottomLeftRadius(radius).unwrap();
    }

    /// Sets each corner's radius independently, clockwise from the top left.
    pub fn set_corner_radii(
        &self,
        top_left: Vector2,
        top_right: Vector2,
        bottom_right: Vector2,
        bottom_left: Vector2,
    ) {
        self.0.SetTopLeftRadius(top_left).unwrap();
        self.0.SetTopRightRadius(top_right).unwrap();
        self.0.SetBottomRightRadius(bottom_right).unwrap();
        self.0.SetBottomLeftRadius(bottom_left).unwrap();
    }
}

impl Sealed for RectangleClip {}

impl Clip for RectangleClip {
    fn as_clip(&self) -> CompositionClip {
        CompositionClip(self.0.cast().unwrap())
    }
}

/// A clip whose shape is any composition [`Geometry`].
///
/// This is what lets a path already tessellated for a shape clip a subtree instead of
/// being rebuilt as one, and it is the only clip that is not a rectangle.
#[derive(Clone)]
pub struct CompositionGeometricClip(pub(crate) bindings::CompositionGeometricClip);

impl CompositionGeometricClip {
    /// Re-points the clip at another geometry.
    pub fn set_geometry(&self, geometry: &impl Geometry) {
        self.0.SetGeometry(&geometry.as_geometry().0).unwrap();
    }
}

impl Sealed for CompositionGeometricClip {}

impl Clip for CompositionGeometricClip {
    fn as_clip(&self) -> CompositionClip {
        CompositionClip(self.0.cast().unwrap())
    }
}

impl Compositor {
    /// Creates an inset clip, with every inset at zero (clipping nothing).
    pub fn create_inset_clip(&self) -> InsetClip {
        InsetClip(self.0.CreateInsetClip().unwrap())
    }

    /// Creates a rectangle clip, with every side at zero — which clips *everything*
    /// until the sides are set, unlike an inset clip's harmless default.
    pub fn create_rectangle_clip(&self) -> RectangleClip {
        let compositor: bindings::ICompositor7 = self.0.cast().unwrap();
        RectangleClip(compositor.CreateRectangleClip().unwrap())
    }

    /// Creates a clip in the shape of `geometry`.
    pub fn create_geometric_clip(&self, geometry: &impl Geometry) -> CompositionGeometricClip {
        let compositor: bindings::ICompositor6 = self.0.cast().unwrap();
        CompositionGeometricClip(
            compositor
                .CreateGeometricClipWithGeometry(&geometry.as_geometry().0)
                .unwrap(),
        )
    }
}
