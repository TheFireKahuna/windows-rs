//! Shapes, paths, and realizations.
//!
//! The value types here are `#[repr(C)]` over Direct2D's own, so a slice of [`Rect`] *is*
//! the array a sprite batch wants and nothing is marshalled per frame.

use super::*;

/// A rectangle, in the target's coordinate space.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    /// Builds a rectangle from its four edges.
    #[must_use]
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Builds a rectangle from a top-left corner and a size.
    #[must_use]
    pub const fn sized(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new(x, y, x + w, y + h)
    }

    /// Returns `right - left`.
    #[must_use]
    pub const fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Returns `bottom - top`.
    #[must_use]
    pub const fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Rounds the corners by the same radius on every corner.
    #[must_use]
    pub const fn rounded(self, radius: f32) -> RoundedRect {
        RoundedRect {
            rect: self,
            x: radius,
            y: radius,
        }
    }

    pub(crate) fn d2d(&self) -> *const D2D_RECT_F {
        const {
            assert!(size_of::<Self>() == size_of::<D2D_RECT_F>());
            assert!(align_of::<Self>() == align_of::<D2D_RECT_F>());
            assert!(core::mem::offset_of!(Self, left) == core::mem::offset_of!(D2D_RECT_F, left));
            assert!(core::mem::offset_of!(Self, top) == core::mem::offset_of!(D2D_RECT_F, top));
            assert!(core::mem::offset_of!(Self, right) == core::mem::offset_of!(D2D_RECT_F, right));
            assert!(
                core::mem::offset_of!(Self, bottom) == core::mem::offset_of!(D2D_RECT_F, bottom)
            );
        }
        (&raw const *self).cast()
    }
}

/// A rectangle with a corner radius per axis.
///
/// Direct2D rasterizes this analytically, from a coverage function over one quad, so a
/// border ring costs by the pixels it touches. [`Gpu::realize`] accepts only a [`Path`].
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct RoundedRect {
    pub rect: Rect,
    pub x: f32,
    pub y: f32,
}

impl RoundedRect {
    pub(crate) fn d2d(&self) -> *const D2D1_ROUNDED_RECT {
        const {
            assert!(size_of::<Self>() == size_of::<D2D1_ROUNDED_RECT>());
            assert!(align_of::<Self>() == align_of::<D2D1_ROUNDED_RECT>());
        }
        (&raw const *self).cast()
    }
}

/// An ellipse, by centre and radii. Rasterized analytically like [`RoundedRect`], and
/// likewise not accepted by [`Gpu::realize`].
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Ellipse {
    pub center: Vector2,
    pub x: f32,
    pub y: f32,
}

impl Ellipse {
    /// Builds an ellipse with equal radii.
    #[must_use]
    pub const fn circle(center: Vector2, radius: f32) -> Self {
        Self {
            center,
            x: radius,
            y: radius,
        }
    }

    pub(crate) fn d2d(&self) -> *const D2D1_ELLIPSE {
        const {
            assert!(size_of::<Self>() == size_of::<D2D1_ELLIPSE>());
            assert!(align_of::<Self>() == align_of::<D2D1_ELLIPSE>());
        }
        (&raw const *self).cast()
    }
}

/// A cubic Bézier segment: two controls and an end point.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Bezier {
    pub c1: Vector2,
    pub c2: Vector2,
    pub to: Vector2,
}

/// What can be filled or stroked.
///
/// A line is not an arm here: it cannot be filled, so it is [`Draw::line`].
#[derive(Copy, Clone)]
pub enum Shape<'a> {
    Rect(Rect),
    Round(RoundedRect),
    Ellipse(Ellipse),
    Path(&'a Path),
}

impl From<Rect> for Shape<'_> {
    fn from(r: Rect) -> Self {
        Self::Rect(r)
    }
}

impl From<RoundedRect> for Shape<'_> {
    fn from(r: RoundedRect) -> Self {
        Self::Round(r)
    }
}

impl From<Ellipse> for Shape<'_> {
    fn from(e: Ellipse) -> Self {
        Self::Ellipse(e)
    }
}

impl<'a> From<&'a Path> for Shape<'a> {
    fn from(p: &'a Path) -> Self {
        Self::Path(p)
    }
}

/// Whether a figure is filled or is an open outline.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Figure {
    #[default]
    Filled,
    Hollow,
}

/// Whether a figure closes back to its start.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum End {
    #[default]
    Open,
    Closed,
}

/// How two paths combine.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Combine {
    Union,
    Intersect,
    Xor,
    Exclude,
}

impl Combine {
    fn d2d(self) -> D2D1_COMBINE_MODE {
        match self {
            Self::Union => D2D1_COMBINE_MODE_UNION,
            Self::Intersect => D2D1_COMBINE_MODE_INTERSECT,
            Self::Xor => D2D1_COMBINE_MODE_XOR,
            Self::Exclude => D2D1_COMBINE_MODE_EXCLUDE,
        }
    }
}

/// Path geometry, authored in the coordinate space it is drawn in.
///
/// Authored at final size rather than in a unit box and stretched: a non-uniform stretch
/// distorts stroke width, corner radii and dash phase, so a hairline comes out one DIP on
/// one axis and three on the other.
pub struct Path(ID2D1PathGeometry1);

/// Writes figures into a [`Path`].
///
/// Only the batched forms exist: `AddLines` and `AddBeziers` take N segments in one call
/// where the per-point forms take N calls for the same shape.
pub struct Sink(ID2D1GeometrySink);

impl Sink {
    /// Begins a figure at `start`.
    pub fn figure(&mut self, start: Vector2, kind: Figure) -> &mut Self {
        let kind = match kind {
            Figure::Filled => D2D1_FIGURE_BEGIN_FILLED,
            Figure::Hollow => D2D1_FIGURE_BEGIN_HOLLOW,
        };
        unsafe { self.0.BeginFigure(start, kind) };
        self
    }

    /// Appends a polyline through `points`.
    pub fn lines(&mut self, points: &[Vector2]) -> &mut Self {
        unsafe { self.0.AddLines(points) };
        self
    }

    /// Appends cubic segments.
    pub fn beziers(&mut self, segments: &[Bezier]) -> &mut Self {
        const {
            assert!(size_of::<Bezier>() == size_of::<D2D1_BEZIER_SEGMENT>());
            assert!(align_of::<Bezier>() == align_of::<D2D1_BEZIER_SEGMENT>());
        }
        // SAFETY: the `const` assertions above prove `Bezier` and `D2D1_BEZIER_SEGMENT` have
        // the same size and alignment; both are three consecutive `Vector2` with C layout.
        let segments: &[D2D1_BEZIER_SEGMENT] =
            unsafe { core::slice::from_raw_parts(segments.as_ptr().cast(), segments.len()) };
        unsafe { self.0.AddBeziers(segments) };
        self
    }

    /// Ends the current figure.
    pub fn close(&mut self, end: End) -> &mut Self {
        let end = match end {
            End::Open => D2D1_FIGURE_END_OPEN,
            End::Closed => D2D1_FIGURE_END_CLOSED,
        };
        unsafe { self.0.EndFigure(end) };
        self
    }

    /// Writes a closed box with four independent corner radii, clockwise from the top
    /// left, as one figure.
    ///
    /// [`Rect::rounded`] covers the uniform case, and Direct2D rasterizes *that* from a
    /// coverage function over one quad, so prefer it wherever the four radii agree. Four
    /// independent radii have no analytic form and take this.
    ///
    /// Each radius is clamped to half the shorter side, the platform's own rule for a
    /// rounded rectangle, so two radii on one edge cannot overlap and a fully rounded box
    /// is a stadium.
    pub fn rounded_box(&mut self, rect: Rect, radius: [f32; 4]) -> &mut Self {
        /// The Bézier control-point ratio that approximates a quarter circle, to within
        /// about a part in ten thousand of the radius.
        const KAPPA: f32 = 0.552_284_8;

        let limit = (rect.width().min(rect.height()) * 0.5).max(0.0);
        let [tl, tr, br, bl] = radius.map(|r| r.clamp(0.0, limit));
        let (l, t, r, b) = (rect.left, rect.top, rect.right, rect.bottom);
        let at = |x: f32, y: f32| Vector2 { x, y };
        // Each corner is one cubic arc towards the corner point it rounds.
        let arc = |from: Vector2, to: Vector2, corner: Vector2| Bezier {
            c1: at(
                from.x + (corner.x - from.x) * KAPPA,
                from.y + (corner.y - from.y) * KAPPA,
            ),
            c2: at(
                to.x + (corner.x - to.x) * KAPPA,
                to.y + (corner.y - to.y) * KAPPA,
            ),
            to,
        };

        self.figure(at(l + tl, t), Figure::Filled);
        self.lines(&[at(r - tr, t)]);
        self.beziers(&[arc(at(r - tr, t), at(r, t + tr), at(r, t))]);
        self.lines(&[at(r, b - br)]);
        self.beziers(&[arc(at(r, b - br), at(r - br, b), at(r, b))]);
        self.lines(&[at(l + bl, b)]);
        self.beziers(&[arc(at(l + bl, b), at(l, b - bl), at(l, b))]);
        self.lines(&[at(l, t + tl)]);
        self.beziers(&[arc(at(l, t + tl), at(l + tl, t), at(l, t))]);
        self.close(End::Closed)
    }
}

impl Path {
    /// The path's bounds, before any stroke widens it.
    pub fn bounds(&self) -> Result<Rect> {
        let b = unsafe { self.0.GetBounds(None)? };
        Ok(Rect::new(b.left, b.top, b.right, b.bottom))
    }

    /// Whether the filled interior contains `p`.
    pub fn contains(&self, p: Vector2) -> Result<bool> {
        Ok(unsafe { self.0.FillContainsPoint(p, None, FLATTEN)? }.as_bool())
    }

    /// Whether a stroke of this path would cover `p` — the hit test for a hairline, which
    /// has no interior to be inside of.
    pub fn stroke_contains(&self, p: Vector2, k: Stroke<'_>) -> Result<bool> {
        let (w, style) = k.parts();
        Ok(unsafe { self.0.StrokeContainsPoint(p, w, style, None, FLATTEN)? }.as_bool())
    }

    /// The outline a stroke of this path would fill: the path's stroke as a *shape*.
    pub fn widen(&self, gpu: &Gpu, k: Stroke<'_>) -> Result<Self> {
        let (w, style) = k.parts();
        gpu.path(|sink| unsafe { self.0.Widen(w, style, None, FLATTEN, sink.raw()).ok() })
    }

    /// This path's own outline, with self-intersections resolved.
    pub fn outline(&self, gpu: &Gpu) -> Result<Self> {
        gpu.path(|sink| unsafe { self.0.Outline(None, FLATTEN, sink.raw()).ok() })
    }

    /// Combines with another path.
    ///
    /// A shape with a notch cut out of it is one fill of a combined path, computed once
    /// here, where the same result through a layer mask is an intermediate per frame.
    pub fn combine(&self, gpu: &Gpu, other: &Self, mode: Combine) -> Result<Self> {
        gpu.path(|sink| unsafe {
            self.0
                .CombineWithGeometry(&other.0, mode.d2d(), None, FLATTEN, sink.raw())
                .ok()
        })
    }

    /// Returns the geometry, for a compositor to wrap as a composition path.
    ///
    /// The path must come from the same [`Gpu`] whose Direct2D device built that
    /// compositor's graphics device, as [`Gpu::d2d`] states.
    pub fn geometry(&self) -> &impl Interface {
        &self.0
    }

    pub(crate) fn raw(&self) -> &ID2D1Geometry {
        (&self.0).into()
    }

    pub(crate) fn owned(&self) -> ID2D1Geometry {
        self.raw().clone()
    }
}

impl Sink {
    pub(crate) fn raw(&self) -> &ID2D1SimplifiedGeometrySink {
        (&self.0).into()
    }
}

/// A tessellated path, fixed at the scale it was built for.
///
/// Direct2D tessellates a path every time it is filled or stroked, even when the path has
/// not changed. A realization does it once and rasterizes per frame, drawing identical
/// pixels because it is the same tessellator.
///
/// The triangles are fixed at that scale, so magnifying them shows the flattening as
/// facets. Rotation and translation cost nothing, so a scrolling plot re-realizes when its
/// data changes and never when it scrolls.
pub struct Realization {
    inner: ID2D1GeometryRealization,
    scale: f32,
}

impl Realization {
    /// Returns the scale this was tessellated at, which with the geometry is what an owner
    /// keys a cache on.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub(crate) fn raw(&self) -> &ID2D1GeometryRealization {
        &self.inner
    }
}

impl Gpu {
    /// Builds a path from the figures `f` writes.
    ///
    /// The sink is closed on the way out, and a failure inside `f` aborts the path rather
    /// than returning a half-built one.
    pub fn path(&self, f: impl FnOnce(&mut Sink) -> Result<()>) -> Result<Path> {
        let geometry = unsafe { self.factory().CreatePathGeometry()? };
        let mut sink = Sink(unsafe { geometry.Open()? });
        f(&mut sink)?;
        unsafe { sink.0.Close().ok()? };
        Ok(Path(geometry))
    }

    /// Tessellates `path` once, at `scale`, filled when `stroke` is `None` and stroked
    /// otherwise.
    ///
    /// **Only a path.** A rounded rectangle's stroke and a circle's fill are among the
    /// primitives Direct2D renders from a coverage function over one quad, so they cost by
    /// the pixels they touch, and realizing one widens the stroke into an outline and
    /// tessellates *that*, turning a one-DIP ring into a mesh that scales with its
    /// perimeter.
    ///
    /// A realization pays where the shape carries enough tessellation to cover the extra
    /// draw path: a spline with a hundred segments does, and a primitive with a closed-form
    /// coverage function does not.
    pub fn realize(
        &self,
        path: &Path,
        scale: f32,
        stroke: Option<Stroke<'_>>,
    ) -> Result<Realization> {
        // The tolerance is in target space, so a realization built for a larger scale needs
        // a proportionally tighter one to hold the same on-screen error.
        let flatten = FLATTEN / scale;
        let inner = unsafe {
            match stroke {
                None => self
                    .ctx()
                    .CreateFilledGeometryRealization(path.raw(), flatten)?,
                Some(k) => {
                    let (w, style) = k.parts();
                    self.ctx()
                        .CreateStrokedGeometryRealization(path.raw(), flatten, w, style)?
                }
            }
        };
        Ok(Realization { inner, scale })
    }
}
