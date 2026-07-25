use super::*;

/// The base type shared by every composition shape. A [`Shape`] can be turned
/// into one via [`Shape::as_shape`] to append it to a
/// [`CompositionShapeCollection`].
#[derive(Clone)]
pub struct CompositionShape(pub(crate) bindings::CompositionShape);

/// A shape that can be appended to a [`CompositionShapeCollection`].
///
/// This trait is sealed: only the shape types in this crate implement it.
pub trait Shape: Sealed {
    /// Returns this shape as the shared [`CompositionShape`] base type.
    fn as_shape(&self) -> CompositionShape;
}

/// The base type shared by every composition geometry. A [`Geometry`] can be
/// turned into one via [`Geometry::as_geometry`] to give a
/// [`CompositionSpriteShape`] its shape.
#[derive(Clone)]
pub struct CompositionGeometry(pub(crate) bindings::CompositionGeometry);

/// A geometry that a [`CompositionSpriteShape`] can fill and stroke.
///
/// Trimming and animation are provided here rather than on each concrete
/// geometry because they are properties of the shared base: every geometry has
/// an outline, so every geometry can draw a fraction of one and animate that
/// fraction. An implementor supplies only [`as_geometry`](Self::as_geometry).
///
/// This trait is sealed: only the geometry types in this crate implement it.
pub trait Geometry: Sealed {
    /// Returns this geometry as the shared [`CompositionGeometry`] base type.
    fn as_geometry(&self) -> CompositionGeometry;

    /// Sets where along the outline drawing begins, as a fraction in `0.0..=1.0`.
    fn set_trim_start(&self, start: f32) {
        bump_count(Count::PropertyWrite);
        let geometry: bindings::ICompositionGeometry = self.as_geometry().0.cast().unwrap();
        geometry.SetTrimStart(start).unwrap();
    }

    /// Sets where along the outline drawing ends, as a fraction in `0.0..=1.0`.
    ///
    /// This is the property to animate to sweep an arc — or to draw a rounded
    /// rectangle's border on: the geometry stays fixed and only the drawn
    /// fraction of it changes.
    fn set_trim_end(&self, end: f32) {
        bump_count(Count::PropertyWrite);
        let geometry: bindings::ICompositionGeometry = self.as_geometry().0.cast().unwrap();
        geometry.SetTrimEnd(end).unwrap();
    }

    /// Starts an animation on the named property (for example `"TrimEnd"`, or a
    /// rounded rectangle's `"CornerRadius"`).
    fn start_animation(&self, property: &str, animation: &impl Animation) {
        bump_count(Count::AnimationStart);
        let object: bindings::ICompositionObject = self.as_geometry().0.cast().unwrap();
        object
            .StartAnimation(property, &animation.as_animation().0)
            .unwrap();
    }

    /// Stops any animation on the named property, leaving it at its current
    /// value.
    ///
    /// Stopping a property that nothing is animating is the ordinary case — a
    /// caller that unconditionally cancels before re-targeting — so the call is
    /// allowed to fail silently rather than panic.
    fn stop_animation(&self, property: &str) {
        bump_count(Count::AnimationStop);
        let object: bindings::ICompositionObject = self.as_geometry().0.cast().unwrap();
        let _ = object.StopAnimation(property);
    }
}

impl Sealed for CompositionGeometry {}

/// The base type satisfies the trait as the identity case, so a caller holding
/// an erased geometry — one picked at runtime from several concrete kinds — can
/// still trim it, animate it, and hand it to
/// [`Compositor::create_sprite_shape`](crate::Compositor::create_sprite_shape).
impl Geometry for CompositionGeometry {
    fn as_geometry(&self) -> CompositionGeometry {
        self.clone()
    }
}

/// An ellipse (or circle) geometry, defined by its radii.
#[derive(Clone)]
pub struct CompositionEllipseGeometry(pub(crate) bindings::CompositionEllipseGeometry);

impl CompositionEllipseGeometry {
    /// Sets the geometry's x and y radii, in DIPs.
    pub fn set_radius(&self, radius: Vector2) {
        bump_count(Count::PropertyWrite);
        self.0.SetRadius(radius).unwrap();
    }
}

impl Sealed for CompositionEllipseGeometry {}

impl Geometry for CompositionEllipseGeometry {
    fn as_geometry(&self) -> CompositionGeometry {
        CompositionGeometry(self.0.cast().unwrap())
    }
}

/// An immutable, opaque snapshot of a Direct2D geometry, ready to be given to a
/// [`CompositionPathGeometry`].
///
/// Create one with [`Compositor::create_path`](crate::Compositor::create_path)
/// and hand it to
/// [`Compositor::create_path_geometry`](crate::Compositor::create_path_geometry).
/// A path has no mutable state of its own: to change what a shape draws, build a
/// new path and re-point the shape with
/// [`CompositionSpriteShape::set_geometry`].
#[derive(Clone)]
pub struct CompositionPath(pub(crate) bindings::CompositionPath);

/// A geometry that draws an arbitrary vector [path](CompositionPath), with an
/// optional [trim](Self::set_trim_end) that renders only part of it.
#[derive(Clone)]
pub struct CompositionPathGeometry(pub(crate) bindings::CompositionPathGeometry);

impl CompositionPathGeometry {
    /// Re-points this geometry at a new [path](CompositionPath).
    ///
    /// The path itself is immutable, but the geometry holding it is not — so a
    /// shape whose vectors change every frame (a curve being dragged) keeps ONE
    /// geometry object and writes one property, instead of minting a geometry
    /// and re-pointing every shape that references it.
    ///
    /// It also keeps whatever is animating this geometry alive. A trim spring in
    /// flight is bound to the OBJECT: replacing the geometry strands the spring
    /// on a retired one and the sweep snaps; re-pathing leaves it running.
    pub fn set_path(&self, path: &CompositionPath) {
        bump_count(Count::PropertyWrite);
        self.0.SetPath(&path.0).unwrap();
    }

}

impl Sealed for CompositionPathGeometry {}

impl Geometry for CompositionPathGeometry {
    fn as_geometry(&self) -> CompositionGeometry {
        CompositionGeometry(self.0.cast().unwrap())
    }
}

impl Object for CompositionPathGeometry {
    fn as_object(&self) -> CompositionObject {
        CompositionObject(self.0.cast().unwrap())
    }
}

/// A rectangle geometry with rounded corners, defined by its
/// [size](Self::set_size), [offset](Self::set_offset), and
/// [corner radius](Self::set_corner_radius).
///
/// Every one of those three is a compositor property, so a rounded box that
/// resizes or changes radius costs a property write rather than a raster — and
/// each can be animated by name (`"Size"`, `"Offset"`, `"CornerRadius"`, all
/// `Vector2`) through [`Geometry::start_animation`]. The
/// [trim](Geometry::set_trim_end) inherited from [`Geometry`] draws a fraction
/// of the outline, which is what a border that traces itself on is made of.
#[derive(Clone)]
pub struct CompositionRoundedRectangleGeometry(
    pub(crate) bindings::CompositionRoundedRectangleGeometry,
);

impl CompositionRoundedRectangleGeometry {
    /// Sets the x and y corner radii, in DIPs.
    pub fn set_corner_radius(&self, radius: Vector2) {
        bump_count(Count::PropertyWrite);
        let geometry: bindings::ICompositionRoundedRectangleGeometry = self.0.cast().unwrap();
        geometry.SetCornerRadius(radius).unwrap();
    }

    /// Sets the rectangle's top-left corner relative to the shape's origin, in
    /// DIPs.
    pub fn set_offset(&self, offset: Vector2) {
        bump_count(Count::PropertyWrite);
        let geometry: bindings::ICompositionRoundedRectangleGeometry = self.0.cast().unwrap();
        geometry.SetOffset(offset).unwrap();
    }

    /// Sets the rectangle's width and height, in DIPs.
    pub fn set_size(&self, size: Vector2) {
        bump_count(Count::PropertyWrite);
        let geometry: bindings::ICompositionRoundedRectangleGeometry = self.0.cast().unwrap();
        geometry.SetSize(size).unwrap();
    }
}

impl Sealed for CompositionRoundedRectangleGeometry {}

impl Geometry for CompositionRoundedRectangleGeometry {
    fn as_geometry(&self) -> CompositionGeometry {
        CompositionGeometry(self.0.cast().unwrap())
    }
}

impl Object for CompositionRoundedRectangleGeometry {
    fn as_object(&self) -> CompositionObject {
        CompositionObject(self.0.cast().unwrap())
    }
}

/// How the ends of a stroked geometry are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeCap {
    /// End the stroke flush with the geometry's end point.
    Flat,
    /// Extend the stroke past the end point by half its thickness, squared off.
    Square,
    /// Extend the stroke past the end point with a semicircle.
    Round,
    /// Extend the stroke past the end point with a triangular point.
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

/// How a stroke turns a corner between two segments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeJoin {
    /// Extend both edges until they meet in a point.
    Miter,
    /// Cut the corner off with a straight edge.
    Bevel,
    /// Round the corner with an arc of the stroke's half-thickness.
    Round,
    /// Mitre the corner, falling back to a bevel past the mitre limit.
    MiterOrBevel,
}

impl From<StrokeJoin> for bindings::CompositionStrokeLineJoin {
    fn from(join: StrokeJoin) -> Self {
        match join {
            StrokeJoin::Miter => Self::Miter,
            StrokeJoin::Bevel => Self::Bevel,
            StrokeJoin::Round => Self::Round,
            StrokeJoin::MiterOrBevel => Self::MiterOrBevel,
        }
    }
}

/// A shape that fills a geometry with a [`Brush`].
#[derive(Clone)]
pub struct CompositionSpriteShape(pub(crate) bindings::CompositionSpriteShape);

impl CompositionSpriteShape {
    /// Sets the brush used to fill the shape's geometry.
    pub fn set_fill_brush(&self, brush: &impl Brush) {
        bump_count(Count::PropertyWrite);
        self.0.SetFillBrush(&brush.as_brush().0).unwrap();
    }

    /// Sets the shape's offset from its parent, in DIPs.
    pub fn set_offset(&self, offset: Vector2) {
        bump_count(Count::PropertyWrite);
        let shape: bindings::ICompositionShape = self.0.cast().unwrap();
        shape.SetOffset(offset).unwrap();
    }

    /// Sets the geometry the shape fills and strokes.
    ///
    /// A [`CompositionPath`] is immutable, so re-pointing the shape at a
    /// geometry built from a new path is how a path-drawn shape changes what it
    /// draws.
    pub fn set_geometry(&self, geometry: &impl Geometry) {
        self.0.SetGeometry(&geometry.as_geometry().0).unwrap();
    }

    /// Sets the brush used to stroke the outline of the shape's geometry.
    pub fn set_stroke_brush(&self, brush: &impl Brush) {
        bump_count(Count::PropertyWrite);
        self.0.SetStrokeBrush(&brush.as_brush().0).unwrap();
    }

    /// Sets the width of the stroked outline, in DIPs.
    pub fn set_stroke_thickness(&self, thickness: f32) {
        bump_count(Count::PropertyWrite);
        self.0.SetStrokeThickness(thickness).unwrap();
    }

    /// Sets how both ends of the stroke are drawn.
    ///
    /// Start and end caps are set together deliberately. Setting only one leaves
    /// a mitre spike at the other end of an open stroked path; the artifact only
    /// disappears when both caps agree, so exposing them separately would offer
    /// a combination that has no correct use here.
    pub fn set_stroke_caps(&self, cap: StrokeCap) {
        let cap: bindings::CompositionStrokeCap = cap.into();
        self.0.SetStrokeStartCap(cap).unwrap();
        self.0.SetStrokeEndCap(cap).unwrap();
    }

    /// Sets how the stroke turns each corner of its geometry.
    ///
    /// The compositor's default is a mitre, which spikes wherever a sampled
    /// spline turns sharply — an artifact a painted trace of the same curve does
    /// not have, because the D2D stroke style behind it joins round.
    pub fn set_stroke_join(&self, join: StrokeJoin) {
        self.0.SetStrokeLineJoin(join.into()).unwrap();
    }

    /// Sets how far a mitred corner may extend, as a multiple of the stroke's
    /// thickness, before it is drawn as a bevel instead.
    ///
    /// This is the only thing that separates [`StrokeJoin::MiterOrBevel`] from a
    /// plain [`StrokeJoin::Miter`]: without a limit there is no angle at which
    /// the fallback engages.
    pub fn set_stroke_miter_limit(&self, limit: f32) {
        self.0.SetStrokeMiterLimit(limit).unwrap();
    }

    /// Replaces the dash pattern with alternating on/off run lengths, each a
    /// MULTIPLE OF THE STROKE THICKNESS rather than a length in DIPs. An empty
    /// slice draws the stroke solid.
    ///
    /// The dash array is a live collection owned by the shape, not a property to
    /// assign, so re-dashing rewrites it in place and the shape keeps its object
    /// — the same reason a geometry is re-pathed rather than replaced.
    pub fn set_stroke_dashes(&self, dashes: &[f32]) {
        let array = self.0.StrokeDashArray().unwrap();
        array.Clear().unwrap();
        for &run in dashes {
            array.Append(run).unwrap();
        }
    }

    /// Sets how the two ends of each dash are drawn.
    ///
    /// Distinct from [`Self::set_stroke_caps`], which caps the whole path: a
    /// dashed rule reads as a row of pills or a row of bars depending on this
    /// alone, and the path's own caps never show once it is dashed.
    pub fn set_stroke_dash_cap(&self, cap: StrokeCap) {
        let cap: bindings::CompositionStrokeCap = cap.into();
        self.0.SetStrokeDashCap(cap).unwrap();
    }

    /// Sets how far into the dash pattern the stroke starts, in the same units
    /// as [`Self::set_stroke_dashes`].
    ///
    /// Phase is what centres a pattern on a rule of arbitrary length, and it is
    /// animatable, so a marching-ants dash costs no app frame at all.
    pub fn set_stroke_dash_offset(&self, offset: f32) {
        self.0.SetStrokeDashOffset(offset).unwrap();
    }

    /// Sets whether the stroke keeps its thickness when the shape is scaled.
    ///
    /// [`Self::set_scale`] scales the stroke along with the geometry, so a
    /// hairline rule on a scaled shape thickens with it. A non-scaling stroke is
    /// measured after the transform instead, which is the only way a one-DIP
    /// rule stays one DIP.
    pub fn set_stroke_non_scaling(&self, non_scaling: bool) {
        self.0.SetIsStrokeNonScaling(non_scaling).unwrap();
    }

    /// Sets the shape's scale factor about its origin.
    ///
    /// Scaling belongs on the shape, not on the visual that hosts it. A
    /// `CompositionVisualSurface` captures its source visual's *content*, and a
    /// visual's own transform is not part of its content — scaling the visual
    /// instead leaves the captured output at its original size, silently, with
    /// nothing to indicate the transform was dropped.
    pub fn set_scale(&self, scale: Vector2) {
        bump_count(Count::PropertyWrite);
        let shape: bindings::ICompositionShape = self.0.cast().unwrap();
        shape.SetScale(scale).unwrap();
    }
}

impl Sealed for CompositionSpriteShape {}

impl Shape for CompositionSpriteShape {
    fn as_shape(&self) -> CompositionShape {
        CompositionShape(self.0.cast().unwrap())
    }
}

/// A shape that groups a child [`shapes`](Self::shapes) collection.
#[derive(Clone)]
pub struct CompositionContainerShape(pub(crate) bindings::CompositionContainerShape);

impl CompositionContainerShape {
    /// Returns the collection of child shapes.
    pub fn shapes(&self) -> CompositionShapeCollection {
        CompositionShapeCollection(self.0.Shapes().unwrap())
    }
}

impl Sealed for CompositionContainerShape {}

impl Shape for CompositionContainerShape {
    fn as_shape(&self) -> CompositionShape {
        CompositionShape(self.0.cast().unwrap())
    }
}

/// An ordered collection of shapes owned by a [`ShapeVisual`] or a
/// [`CompositionContainerShape`].
#[derive(Clone)]
pub struct CompositionShapeCollection(pub(crate) bindings::CompositionShapeCollection);

impl CompositionShapeCollection {
    /// Appends a shape to the end of the collection.
    pub fn append(&self, shape: &impl Shape) {
        let vector: windows_collections::IVector<bindings::CompositionShape> =
            self.0.cast().unwrap();
        vector.Append(&shape.as_shape().0).unwrap();
    }
}

/// A visual that renders a collection of composition [`shapes`](Self::shapes).
/// Derefs to [`Visual`], so a shape visual can be positioned and sized directly.
#[derive(Clone)]
pub struct ShapeVisual {
    visual: Visual,
    shape_visual: bindings::ShapeVisual,
}

impl ShapeVisual {
    pub(crate) fn new(shape_visual: bindings::ShapeVisual) -> Self {
        Self {
            visual: Visual(shape_visual.cast().unwrap()),
            shape_visual,
        }
    }

    /// Returns the collection of shapes rendered by the visual.
    pub fn shapes(&self) -> CompositionShapeCollection {
        CompositionShapeCollection(self.shape_visual.Shapes().unwrap())
    }
}

impl core::ops::Deref for ShapeVisual {
    type Target = Visual;
    fn deref(&self) -> &Visual {
        &self.visual
    }
}

impl Compositor {
    /// Adopts a Direct2D geometry as an immutable composition
    /// [`CompositionPath`].
    ///
    /// Pass any COM object implementing `IGeometrySource2D` — for example
    /// canvas's geometry wrapper over an `ID2D1Geometry`.
    ///
    /// # Factory affinity
    ///
    /// The geometry **must** come from the same Direct2D factory as the device
    /// passed to `create_graphics_device`. When the
    /// compositor realises the path it calls back into the geometry source with
    /// a factory of its own choosing, and the source is required to return
    /// geometry belonging to that factory. Nothing on either side of that
    /// callback can verify the match: a mismatch surfaces later as a failed
    /// realisation or as content that never appears, not as an error from this
    /// call. Keeping one Direct2D factory per compositor is the only way to hold
    /// this invariant.
    pub fn create_path(&self, geometry: &impl Interface) -> Result<CompositionPath> {
        let source: bindings::IGeometrySource2D = geometry.cast()?;
        Ok(CompositionPath(bindings::CompositionPath::Create(&source)?))
    }

    /// Creates a geometry that draws the given path.
    pub fn create_path_geometry(&self, path: &CompositionPath) -> CompositionPathGeometry {
        bump_count(Count::Geometry);
        let compositor: bindings::ICompositor5 = self.0.cast().unwrap();
        CompositionPathGeometry(compositor.CreatePathGeometryWithPath(&path.0).unwrap())
    }

    /// Creates a rounded-rectangle geometry.
    pub fn create_rounded_rectangle_geometry(&self) -> CompositionRoundedRectangleGeometry {
        bump_count(Count::Geometry);
        let compositor: bindings::ICompositor5 = self.0.cast().unwrap();
        CompositionRoundedRectangleGeometry(compositor.CreateRoundedRectangleGeometry().unwrap())
    }
}
