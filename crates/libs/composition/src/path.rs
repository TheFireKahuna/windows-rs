//! Paths, rounded rectangles and stroke style (feature `system`).
//!
//! Everything a retained sprite needs to express a curve without keeping a drawing
//! surface alive for it. A composition **path** is immutable; the **geometry** holding
//! one is not, so a curve that reshapes re-points its geometry and keeps both the
//! object and any trim animation already running on it.

use super::*;

/// An immutable set of vectors — the shape itself, with no position, no brush and no
/// stroke.
///
/// Created from a Direct2D geometry with
/// [`Compositor::create_path`](crate::Compositor::create_path), so a curve tessellated
/// once by the drawing stack is authored once rather than rebuilt as composition
/// geometry.
#[derive(Clone)]
pub struct CompositionPath(pub(crate) bindings::CompositionPath);

/// A geometry that draws a [`CompositionPath`].
///
/// The path it holds can be replaced — that is the point of the split. A curve whose
/// shape changes assigns a new path to the same geometry, so the shape, the sprite and
/// any running trim animation all survive the change.
#[derive(Clone)]
pub struct CompositionPathGeometry(pub(crate) bindings::CompositionPathGeometry);

impl CompositionPathGeometry {
    /// Re-points this geometry at another path.
    pub fn set_path(&self, path: &CompositionPath) {
        self.0.SetPath(&path.0).unwrap();
    }
}

impl Sealed for CompositionPathGeometry {}

impl Geometry for CompositionPathGeometry {
    fn as_geometry(&self) -> CompositionGeometry {
        CompositionGeometry(self.0.cast().unwrap())
    }
}

/// A rectangle geometry with a corner radius, positioned and sized within the shape
/// that draws it.
///
/// The analytic rounded rectangle: D2D rasterizes it by the pixels it touches, so this
/// is what a border ring or a pill should be — never a path realized into a mesh whose
/// cost scales with its perimeter.
#[derive(Clone)]
pub struct CompositionRoundedRectangleGeometry(
    pub(crate) bindings::CompositionRoundedRectangleGeometry,
);

impl CompositionRoundedRectangleGeometry {
    /// Sets the corner radius, in DIPs, capped by the platform at half the box on each
    /// axis.
    pub fn set_corner_radius(&self, radius: Vector2) {
        self.0.SetCornerRadius(radius).unwrap();
    }

    /// Sets the geometry's offset within the shape that draws it, in DIPs.
    pub fn set_offset(&self, offset: Vector2) {
        self.0.SetOffset(offset).unwrap();
    }

    /// Sets the geometry's size, in DIPs.
    pub fn set_size(&self, size: Vector2) {
        self.0.SetSize(size).unwrap();
    }
}

impl Sealed for CompositionRoundedRectangleGeometry {}

impl Geometry for CompositionRoundedRectangleGeometry {
    fn as_geometry(&self) -> CompositionGeometry {
        CompositionGeometry(self.0.cast().unwrap())
    }
}

impl CompositionGeometry {
    /// Exposes only the fraction of the geometry between `start` and `end`, each in
    /// `0.0..=1.0` of its total length.
    ///
    /// Both are animatable — animation targets `"TrimStart"` and `"TrimEnd"` — which is
    /// how an arc sweeps or a curve draws itself in without the geometry being
    /// regenerated: the path is authored once and the trim animates.
    pub fn set_trim(&self, start: f32, end: f32) {
        self.0.SetTrimStart(start).unwrap();
        self.0.SetTrimEnd(end).unwrap();
    }
}

/// How a stroke terminates, at its own ends and at each dash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeCap {
    /// Stop exactly at the end point.
    Flat,
    /// Extend by half the stroke width as a square.
    Square,
    /// Extend by half the stroke width as a semicircle.
    Round,
    /// Extend by half the stroke width as a triangle.
    Triangle,
}

impl From<StrokeCap> for bindings::CompositionStrokeCap {
    fn from(cap: StrokeCap) -> Self {
        match cap {
            StrokeCap::Flat => Self::Flat,
            StrokeCap::Square => Self::Square,
            StrokeCap::Round => Self::Round,
            StrokeCap::Triangle => Self::Triangle,
        }
    }
}

/// How a stroke turns a corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeJoin {
    /// Extend both edges until they meet at a point.
    Miter,
    /// Extend to a point, but cut the corner off past the
    /// [mitre limit](CompositionSpriteShape::set_stroke_miter_limit).
    MiterOrBevel,
    /// Round the corner.
    Round,
    /// Cut the corner off with a straight edge.
    Bevel,
}

impl From<StrokeJoin> for bindings::CompositionStrokeLineJoin {
    fn from(join: StrokeJoin) -> Self {
        match join {
            StrokeJoin::Miter => Self::Miter,
            StrokeJoin::MiterOrBevel => Self::MiterOrBevel,
            StrokeJoin::Round => Self::Round,
            StrokeJoin::Bevel => Self::Bevel,
        }
    }
}

impl CompositionSpriteShape {
    /// Re-points the shape at another geometry.
    pub fn set_geometry(&self, geometry: &impl Geometry) {
        self.0.SetGeometry(&geometry.as_geometry().0).unwrap();
    }

    /// Sets the brush the shape's outline is stroked with. A shape with no stroke brush
    /// is filled only.
    pub fn set_stroke_brush(&self, brush: &impl Brush) {
        self.0.SetStrokeBrush(&brush.as_brush().0).unwrap();
    }

    /// Sets the stroke width, in DIPs, centred on the geometry's outline.
    pub fn set_stroke_thickness(&self, thickness: f32) {
        self.0.SetStrokeThickness(thickness).unwrap();
    }

    /// Sets both ends of the stroke to the same cap.
    pub fn set_stroke_caps(&self, cap: StrokeCap) {
        self.0.SetStrokeStartCap(cap.into()).unwrap();
        self.0.SetStrokeEndCap(cap.into()).unwrap();
    }

    /// Sets how the stroke turns corners.
    pub fn set_stroke_join(&self, join: StrokeJoin) {
        self.0.SetStrokeLineJoin(join.into()).unwrap();
    }

    /// Sets how far a mitre may extend before
    /// [`StrokeJoin::MiterOrBevel`] bevels it, as a multiple of the stroke thickness.
    pub fn set_stroke_miter_limit(&self, limit: f32) {
        self.0.SetStrokeMiterLimit(limit).unwrap();
    }

    /// Sets the dash pattern as alternating dash and gap lengths.
    ///
    /// The runs are **multiples of the stroke thickness**, as Direct2D's are, not DIPs
    /// — so the same array on a 1-DIP rule and a 4-DIP rule draws dashes four times as
    /// long on the second. An empty slice clears the pattern back to a solid stroke.
    pub fn set_stroke_dashes(&self, dashes: &[f32]) {
        // The dash array is a live collection owned by the shape rather than a property
        // to assign, so re-dashing rewrites it in place and the shape keeps its object —
        // the same reason a geometry is re-pathed rather than replaced.
        let array: windows_collections::IVector<f32> =
            self.0.StrokeDashArray().unwrap().cast().unwrap();
        array.Clear().unwrap();
        for &run in dashes {
            array.Append(run).unwrap();
        }
    }

    /// Sets the cap drawn at each end of each dash.
    pub fn set_stroke_dash_cap(&self, cap: StrokeCap) {
        self.0.SetStrokeDashCap(cap.into()).unwrap();
    }

    /// Shifts the dash pattern along the outline, in multiples of the stroke thickness.
    /// Animate it — the target is `"StrokeDashOffset"` — for a marching-ants effect
    /// with no geometry churn.
    pub fn set_stroke_dash_offset(&self, offset: f32) {
        self.0.SetStrokeDashOffset(offset).unwrap();
    }

    /// Keeps the stroke's thickness fixed when the shape is scaled.
    ///
    /// The only way a hairline rule survives a
    /// [`set_scale`](CompositionSpriteShape::set_scale): scaled normally, a 1-DIP
    /// stroke at 1.5× is 1.5 DIPs and stops being a hairline.
    pub fn set_stroke_non_scaling(&self, non_scaling: bool) {
        self.0.SetIsStrokeNonScaling(non_scaling).unwrap();
    }

    /// Scales the shape about its own origin.
    pub fn set_scale(&self, scale: Vector2) {
        let shape: bindings::ICompositionShape = self.0.cast().unwrap();
        shape.SetScale(scale).unwrap();
    }
}

/// Presents a Direct2D geometry to the compositor as the `IGeometrySource2D` a
/// [`CompositionPath`] is built from.
///
/// A geometry from `ID2D1Factory` does not implement that interface — only a WinRT
/// geometry wrapper does — so something has to bridge the two, and doing it here means a
/// caller hands over the `ID2D1Geometry` it already has rather than an object it had to
/// build for this call.
#[windows_core::implement(bindings::IGeometrySource2D, bindings::IGeometrySource2DInterop)]
struct GeometrySource(bindings::ID2D1Geometry);

impl bindings::IGeometrySource2D_Impl for GeometrySource_Impl {}

impl bindings::IGeometrySource2DInterop_Impl for GeometrySource_Impl {
    fn GetGeometry(&self) -> Result<bindings::ID2D1Geometry> {
        Ok(self.0.clone())
    }

    fn TryGetGeometryUsingFactory(
        &self,
        _factory: windows_core::Ref<bindings::ID2D1Factory>,
    ) -> Result<bindings::ID2D1Geometry> {
        // The contract is "return geometry belonging to the factory I am handing you",
        // and a geometry cannot be moved between factories. Returning the one we hold
        // is correct exactly when it already belongs to that factory, which is the
        // affinity rule `create_path` documents; there is nothing truthful to return
        // otherwise, and failing here would turn a satisfiable case into a hard error.
        Ok(self.0.clone())
    }
}

impl Compositor {
    /// Wraps a Direct2D geometry as an immutable [`CompositionPath`].
    ///
    /// `geometry` is the drawing stack's own `ID2D1Geometry`; it arrives as
    /// `&impl Interface` and is cast on the way in, so no generated type from another
    /// crate crosses this boundary. The `Err` is an object that is not a D2D geometry at
    /// all — which is how a caller learns it handed over the wrong thing, rather than by a
    /// failure deep inside the compositor.
    ///
    /// # Factory affinity
    ///
    /// The geometry **must** come from the same Direct2D factory as the device given to
    /// [`create_graphics_device`](Compositor::create_graphics_device). When the compositor
    /// realizes the path it asks the source for geometry belonging to a factory of its own
    /// choosing, and nothing on either side of that callback can verify the match: a
    /// mismatch surfaces later as a failed realization or as content that never appears,
    /// not as an error from this call. One Direct2D factory per compositor is the only way
    /// to hold the invariant.
    pub fn create_path(&self, geometry: &impl Interface) -> Result<CompositionPath> {
        let geometry: bindings::ID2D1Geometry = geometry.cast()?;
        let source: bindings::IGeometrySource2D =
            windows_core::ComObject::new(GeometrySource(geometry)).into_interface();
        Ok(CompositionPath(bindings::CompositionPath::Create(&source)?))
    }

    /// Creates a geometry that draws `path`.
    pub fn create_path_geometry(&self, path: &CompositionPath) -> CompositionPathGeometry {
        let compositor: bindings::ICompositor5 = self.0.cast().unwrap();
        CompositionPathGeometry(compositor.CreatePathGeometryWithPath(&path.0).unwrap())
    }

    /// Creates a rounded-rectangle geometry, zero-sized until
    /// [`set_size`](CompositionRoundedRectangleGeometry::set_size) is called.
    pub fn create_rounded_rectangle_geometry(&self) -> CompositionRoundedRectangleGeometry {
        let compositor: bindings::ICompositor5 = self.0.cast().unwrap();
        CompositionRoundedRectangleGeometry(compositor.CreateRoundedRectangleGeometry().unwrap())
    }
}
