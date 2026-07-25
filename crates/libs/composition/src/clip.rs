use super::*;

/// The base type shared by every clip. A [`Clip`] can be turned into one via
/// [`Clip::as_clip`] to attach it to a visual.
#[derive(Clone)]
pub struct CompositionClip(pub(crate) bindings::CompositionClip);

/// A clip that restricts a [`Visual`](crate::Visual) and its subtree to a
/// region, hiding whatever falls outside.
///
/// A clip is not a brush: it leaves the visual's single brush slot free. That
/// is what makes it the cheap way to round a sprite already painting through a
/// [`CompositionMaskBrush`](crate::CompositionMaskBrush) — no second visual and
/// no capture, both of which cost a surface and mangle a mask's alpha.
///
/// This trait is sealed: only the clip types in this crate implement it.
pub trait Clip: Sealed {
    /// Returns this clip as the shared [`CompositionClip`] base type.
    fn as_clip(&self) -> CompositionClip;
}

/// Starts `animation` on the named property of a clip.
///
/// Shared by every clip type: a clip is a composition object like any other, and
/// the cast to reach its animation surface is the same for all of them.
fn start_animation(clip: &impl Interface, property: &str, animation: &impl Animation) {
    let object: bindings::ICompositionObject = clip.cast().unwrap();
    object
        .StartAnimation(property, &animation.as_animation().0)
        .unwrap();
}

/// Stops any animation on the named property, leaving it at the value it reached.
///
/// As on [`Visual::stop_animation`](crate::Visual::stop_animation), a failure is
/// discarded rather than panicked on: stopping a property that nothing is
/// animating is the ordinary case, not an exceptional one, so a caller taking a
/// property back under manual control can stop it unconditionally and then set it.
fn stop_animation(clip: &impl Interface, property: &str) {
    let object: bindings::ICompositionObject = clip.cast().unwrap();
    let _ = object.StopAnimation(property);
}

/// A clip that hides a fixed inset from each edge of the visual it is applied
/// to, leaving the rest visible.
///
/// Create one with [`Compositor::create_inset_clip`] and attach it with
/// [`Visual::set_clip`](crate::Visual::set_clip). The insets are in the clipped
/// visual's own coordinate space, in DIPs, measured inward from each edge, so an
/// inset of `0.0` on every edge shows the visual whole.
///
/// The insets are animatable properties in their own right (`"LeftInset"`,
/// `"TopInset"`, `"RightInset"`, `"BottomInset"`), which is how a reveal is
/// expressed: hold the visual still and animate the clip's inset across it,
/// rather than animating the visual's size. Because a clip is not a visual,
/// those animations start through this type's own
/// [`start_animation`](Self::start_animation) rather than the visual's.
#[derive(Clone)]
pub struct InsetClip(pub(crate) bindings::InsetClip);

impl InsetClip {
    /// Sets all four insets, in DIPs, measured inward from the corresponding
    /// edge of the clipped visual.
    pub fn set_insets(&self, left: f32, top: f32, right: f32, bottom: f32) {
        self.0.SetLeftInset(left).unwrap();
        self.0.SetTopInset(top).unwrap();
        self.0.SetRightInset(right).unwrap();
        self.0.SetBottomInset(bottom).unwrap();
    }

    /// Sets the inset from the left edge, in DIPs.
    pub fn set_left_inset(&self, inset: f32) {
        self.0.SetLeftInset(inset).unwrap();
    }

    /// Sets the inset from the top edge, in DIPs.
    pub fn set_top_inset(&self, inset: f32) {
        self.0.SetTopInset(inset).unwrap();
    }

    /// Sets the inset from the right edge, in DIPs.
    pub fn set_right_inset(&self, inset: f32) {
        self.0.SetRightInset(inset).unwrap();
    }

    /// Sets the inset from the bottom edge, in DIPs.
    pub fn set_bottom_inset(&self, inset: f32) {
        self.0.SetBottomInset(inset).unwrap();
    }

    /// Starts an animation on the named property (for example `"RightInset"`).
    pub fn start_animation(&self, property: &str, animation: &impl Animation) {
        start_animation(&self.0, property, animation);
    }

    /// Stops any animation on the named property, leaving the property at the
    /// value it had reached.
    pub fn stop_animation(&self, property: &str) {
        stop_animation(&self.0, property);
    }
}

impl Sealed for InsetClip {}

impl Object for InsetClip {
    fn as_object(&self) -> CompositionObject {
        CompositionObject(self.0.cast().unwrap())
    }
}

impl Clip for InsetClip {
    fn as_clip(&self) -> CompositionClip {
        CompositionClip(self.0.cast().unwrap())
    }
}

/// A clip that keeps a rectangle with optionally rounded corners, hiding
/// everything outside it.
///
/// Create one with [`Compositor::create_rectangle_clip`] and attach it with
/// [`Visual::set_clip`](crate::Visual::set_clip).
///
/// Unlike [`InsetClip`], whose insets are measured inward from the clipped
/// visual's edges, a rectangle clip's sides are **absolute** in that visual's
/// own coordinate space. A clip that must follow the visual's size is therefore
/// rewritten when the size changes — see [`set_sides`](Self::set_sides) — but in
/// exchange it can extend beyond the visual, and negative values are legal.
///
/// Every side and radius animates, through this type's own
/// [`start_animation`](Self::start_animation) — and every one of them is a
/// **scalar** target. The sides are named for the properties (`"Left"`, `"Top"`,
/// `"Right"`, `"Bottom"`), but a radius is a [`Vector2`] here and has no
/// whole-vector target: it animates per channel as `"TopLeftRadiusX"`,
/// `"TopLeftRadiusY"`, `"TopRightRadiusX"`, and so on for all four corners.
/// Neither the property name (`"TopLeftRadius"`) nor a subchannel path
/// (`"TopLeftRadius.X"`) is accepted — both are rejected when the animation
/// starts, so the concatenated name is the only one that binds.
#[derive(Clone)]
pub struct RectangleClip(pub(crate) bindings::RectangleClip);

impl RectangleClip {
    /// Sets all four sides, in DIPs, in the clipped visual's coordinate space.
    ///
    /// A clip covering the whole of a visual sized `w` by `h` is
    /// `set_sides(0.0, 0.0, w, h)`.
    pub fn set_sides(&self, left: f32, top: f32, right: f32, bottom: f32) {
        self.0.SetLeft(left).unwrap();
        self.0.SetTop(top).unwrap();
        self.0.SetRight(right).unwrap();
        self.0.SetBottom(bottom).unwrap();
    }

    /// Sets the left side, in DIPs.
    pub fn set_left(&self, left: f32) {
        self.0.SetLeft(left).unwrap();
    }

    /// Sets the top side, in DIPs.
    pub fn set_top(&self, top: f32) {
        self.0.SetTop(top).unwrap();
    }

    /// Sets the right side, in DIPs.
    pub fn set_right(&self, right: f32) {
        self.0.SetRight(right).unwrap();
    }

    /// Sets the bottom side, in DIPs.
    pub fn set_bottom(&self, bottom: f32) {
        self.0.SetBottom(bottom).unwrap();
    }

    /// Rounds all four corners by the same radius, in DIPs.
    ///
    /// Each corner is an ellipse quadrant, so `x` and `y` are its two radii; a
    /// circular corner passes the same value for both.
    pub fn set_corner_radius(&self, radius: Vector2) {
        self.0.SetTopLeftRadius(radius).unwrap();
        self.0.SetTopRightRadius(radius).unwrap();
        self.0.SetBottomLeftRadius(radius).unwrap();
        self.0.SetBottomRightRadius(radius).unwrap();
    }

    /// Sets the four corner radii independently, in DIPs, clockwise from the
    /// top-left corner.
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

    /// Starts an animation on the named property (for example `"TopLeftRadius"`).
    pub fn start_animation(&self, property: &str, animation: &impl Animation) {
        start_animation(&self.0, property, animation);
    }

    /// Stops any animation on the named property, leaving the property at the
    /// value it had reached.
    pub fn stop_animation(&self, property: &str) {
        stop_animation(&self.0, property);
    }
}

impl Sealed for RectangleClip {}

impl Object for RectangleClip {
    fn as_object(&self) -> CompositionObject {
        CompositionObject(self.0.cast().unwrap())
    }
}

impl Clip for RectangleClip {
    fn as_clip(&self) -> CompositionClip {
        CompositionClip(self.0.cast().unwrap())
    }
}

/// A clip shaped by an arbitrary [`Geometry`], hiding everything outside it.
///
/// Create one with [`Compositor::create_geometric_clip`] and attach it with
/// [`Visual::set_clip`](crate::Visual::set_clip). The geometry is the same kind
/// a sprite shape draws, so a path already built for a stroke can clip a subtree
/// without being tessellated again — and re-pointing that geometry (for example
/// [`CompositionPathGeometry::set_path`](crate::CompositionPathGeometry::set_path))
/// reshapes the clip in place, keeping any animation running on it.
///
/// Prefer [`RectangleClip`] for a rounded rectangle: it needs no geometry object
/// and its sides and radii animate directly.
#[derive(Clone)]
pub struct CompositionGeometricClip(pub(crate) bindings::CompositionGeometricClip);

impl CompositionGeometricClip {
    /// Re-points the clip at a different geometry.
    pub fn set_geometry(&self, geometry: &impl Geometry) {
        self.0.SetGeometry(&geometry.as_geometry().0).unwrap();
    }

    /// Starts an animation on the named property.
    pub fn start_animation(&self, property: &str, animation: &impl Animation) {
        start_animation(&self.0, property, animation);
    }

    /// Stops any animation on the named property, leaving the property at the
    /// value it had reached.
    pub fn stop_animation(&self, property: &str) {
        stop_animation(&self.0, property);
    }
}

impl Sealed for CompositionGeometricClip {}

impl Object for CompositionGeometricClip {
    fn as_object(&self) -> CompositionObject {
        CompositionObject(self.0.cast().unwrap())
    }
}

impl Clip for CompositionGeometricClip {
    fn as_clip(&self) -> CompositionClip {
        CompositionClip(self.0.cast().unwrap())
    }
}

impl Compositor {
    /// Creates an inset clip, initially clipping nothing (every inset `0.0`).
    pub fn create_inset_clip(&self) -> InsetClip {
        bump_count(Count::Clip);
        InsetClip(self.0.CreateInsetClip().unwrap())
    }

    /// Creates a rectangle clip with square corners and, until
    /// [`set_sides`](RectangleClip::set_sides) is called, every side `0.0` —
    /// which clips the visual away entirely rather than leaving it whole.
    pub fn create_rectangle_clip(&self) -> RectangleClip {
        bump_count(Count::Clip);
        let compositor: bindings::ICompositor7 = self.0.cast().unwrap();
        RectangleClip(compositor.CreateRectangleClip().unwrap())
    }

    /// Creates a clip shaped by `geometry`.
    pub fn create_geometric_clip(&self, geometry: &impl Geometry) -> CompositionGeometricClip {
        bump_count(Count::Clip);
        let compositor: bindings::ICompositor6 = self.0.cast().unwrap();
        CompositionGeometricClip(
            compositor
                .CreateGeometricClipWithGeometry(&geometry.as_geometry().0)
                .unwrap(),
        )
    }
}
