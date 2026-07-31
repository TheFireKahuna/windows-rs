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
    #[must_use]
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[must_use]
    pub const fn sized(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new(x, y, x + w, y + h)
    }

    #[must_use]
    pub const fn width(&self) -> f32 {
        self.right - self.left
    }

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
/// border ring costs by the pixels it touches — which is why [`Gpu::realize`] does not
/// accept one.
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

/// An ellipse, by centre and radii. Analytic too, and likewise not realizable.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Ellipse {
    pub center: Vector2,
    pub x: f32,
    pub y: f32,
}

impl Ellipse {
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
/// One enum rather than a trait, so no generated type appears in a public bound, and four
/// arms rather than a `draw_*`/`fill_*` pair per primitive. A line is not here: it cannot
/// be filled, so it is [`Draw::line`] instead of an arm that would be meaningless in half
/// its uses.
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

/// Path geometry, authored in the coordinate space it will be drawn in.
///
/// Not authored in a unit box and stretched: a non-uniform stretch distorts stroke width,
/// corner radii and dash phase, so a hairline comes out one DIP on one axis and three on
/// the other.
pub struct Path(ID2D1PathGeometry1);

/// Writes figures into a [`Path`].
///
/// Only the batched calls exist. `AddLines` and `AddBeziers` are one call for N segments
/// where the per-point forms are N calls for the same shape, and every curve here is built
/// from a slice already.
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

    pub fn lines(&mut self, points: &[Vector2]) -> &mut Self {
        unsafe { self.0.AddLines(points) };
        self
    }

    pub fn beziers(&mut self, segments: &[Bezier]) -> &mut Self {
        const {
            assert!(size_of::<Bezier>() == size_of::<D2D1_BEZIER_SEGMENT>());
            assert!(align_of::<Bezier>() == align_of::<D2D1_BEZIER_SEGMENT>());
        }
        // SAFETY: the assertion above establishes the layouts agree; both are three
        // consecutive `Vector2` with C layout.
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
    /// [`Rect::rounded`] covers the uniform case and Direct2D rasterizes *that* from a
    /// coverage function over one quad, so prefer it whenever the four radii agree. Four
    /// independent radii have no analytic form and need this.
    ///
    /// Radii are clamped so that two on the same edge cannot overlap, which is the
    /// platform's own rule for a rounded rectangle — so "fully rounded" is a stadium and
    /// never a football.
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
    /// Worth reaching for before a layer: a shape with a notch cut out of it is one fill of
    /// a combined path, computed once here, where a layer with a mask is an intermediate
    /// per frame.
    pub fn combine(&self, gpu: &Gpu, other: &Self, mode: Combine) -> Result<Self> {
        gpu.path(|sink| unsafe {
            self.0
                .CombineWithGeometry(&other.0, mode.d2d(), None, FLATTEN, sink.raw())
                .ok()
        })
    }

    /// The geometry, for a compositor to wrap as a composition path.
    ///
    /// Must be a path from the same [`Gpu`] whose Direct2D device built that compositor's
    /// graphics device — see [`Gpu::d2d`].
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
/// The scale is part of it and not incidental: the triangles are fixed, so magnifying them
/// shows the flattening as facets. Rotation and translation are free, so a scrolling plot
/// re-realizes when its data changes and never when it scrolls.
pub struct Realization {
    inner: ID2D1GeometryRealization,
    scale: f32,
}

impl Realization {
    /// The scale this was tessellated at — half of the cache key its owner needs, the
    /// other half being the geometry.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub(crate) fn raw(&self) -> &ID2D1GeometryRealization {
        &self.inner
    }
}

impl Gpu {
    /// Builds a path. The sink is closed for you, and a failure inside `f` aborts the
    /// path rather than leaving a half-built one.
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
    /// the pixels they touch — and realizing one widens the stroke into an outline and
    /// tessellates *that*, so a one-DIP ring becomes a mesh scaling with its perimeter.
    /// Measured on a card's border ring and gate dot: 51 µs of the panel's ~206.
    ///
    /// So the discriminator is not "does it survive frames" alone. It is whether the shape
    /// has enough tessellation in it to pay for the extra draw path — a spline with a
    /// hundred segments does; a primitive with a closed-form coverage function never does.
    /// Accepting only a path is that rule made structural.
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
