use super::*;

/// A completed path geometry.
///
/// ```ignore
/// let path = PathBuilder::new(&device)?
///     .begin(Vector2::new(0.0, 0.0))
///     .line_to(Vector2::new(100.0, 0.0))
///     .line_to(Vector2::new(50.0, 80.0))
///     .close()
///     .build()?;
/// ```
#[derive(Clone)]
pub struct Path {
    raw: ID2D1PathGeometry1,
}

/// Direct2D's default flattening tolerance for hit-testing and bounds queries.
const DEFAULT_FLATTENING_TOLERANCE: f32 = 0.25;

/// How [`Path::combine`] resolves the two regions it is given.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CombineMode {
    /// Everything either region covers.
    #[default]
    Union,
    /// Only what both cover.
    Intersect,
    /// What exactly one covers — the union minus the intersection.
    Xor,
    /// What the receiver covers and the argument does not. The one asymmetric
    /// mode: `a.combine(b, Exclude)` and `b.combine(a, Exclude)` differ.
    Exclude,
}

impl CombineMode {
    pub(crate) fn to_abi(self) -> D2D1_COMBINE_MODE {
        match self {
            Self::Union => D2D1_COMBINE_MODE_UNION,
            Self::Intersect => D2D1_COMBINE_MODE_INTERSECT,
            Self::Xor => D2D1_COMBINE_MODE_XOR,
            Self::Exclude => D2D1_COMBINE_MODE_EXCLUDE,
        }
    }
}


impl Path {
    /// Returns the underlying `ID2D1PathGeometry1`.
    pub fn raw(&self) -> &ID2D1PathGeometry1 {
        &self.raw
    }

    /// The OUTLINE of stroking this path at `stroke_width` — the region a
    /// stroke covers, as a fillable geometry.
    ///
    /// A stroke is not a region: it is a line plus a width, and only a renderer
    /// that strokes can draw one. Anything that takes an AREA — a
    /// `CompositionGeometricClip`, a hit test, a fill — needs the stroke
    /// converted into the closed shape it covers, which is what widening does.
    ///
    /// The result self-overlaps wherever the source turns sharply. That is
    /// correct and needs no cleanup: Direct2D fills it under the nonzero winding
    /// rule, so overlapping laps of the outline still describe one region.
    /// `style` decides the caps and joins, and is taken rather than built here
    /// because a caller widening every frame must not mint a COM object per
    /// frame for a value that never varies — see
    /// [`GpuDevice::create_stroke_style`].
    ///
    /// `tolerance` is the furthest the flattened result may stray from the true
    /// outline, in the geometry's OWN units. It is a parameter rather than
    /// Direct2D's fixed default because it decides the cost: a widened spline is
    /// flattened curve by curve and join by join, so halving the tolerance buys
    /// accuracy nobody can see and pays for it in vertices the compositor then
    /// has to carry. A caller drawing in DIPs at a known scale should state what
    /// it wants in DEVICE pixels and divide.
    pub fn widen(
        &self,
        device: &GpuDevice,
        stroke_width: f32,
        style: &StrokeStyle,
        tolerance: f32,
    ) -> Result<Path> {
        let raw = unsafe { device.d2d_factory().CreatePathGeometry()? };
        let sink = unsafe { raw.Open()? };
        unsafe {
            self.raw
                .Widen(stroke_width, &style.0, None, tolerance, &sink)
                .ok()?;
            sink.Close().ok()?;
        }
        Ok(Path { raw })
    }

    /// This path's OUTLINE: the same region with its self-intersections
    /// resolved, so the result never overlaps itself and fills identically under
    /// either winding rule.
    ///
    /// The natural follow-on to [`widen`](Self::widen), whose result laps over
    /// itself wherever the source turned sharply. Both describe the same region,
    /// so this changes no pixel — what it changes is how much geometry a
    /// consumer downstream has to carry and rasterize. Whether that is a saving
    /// depends on the shape: resolving intersections also SPLITS segments at
    /// every crossing, so a path that crosses itself often can come back larger
    /// than it went in. Measure before assuming
    /// (`--example widen_cost`).
    pub fn outline(&self, device: &GpuDevice, tolerance: f32) -> Result<Path> {
        let raw = unsafe { device.d2d_factory().CreatePathGeometry()? };
        let sink = unsafe { raw.Open()? };
        unsafe {
            self.raw.Outline(None, tolerance, &sink).ok()?;
            sink.Close().ok()?;
        }
        Ok(Path { raw })
    }

    /// This path combined with `other` under `mode` — the boolean algebra of
    /// two regions, as a third region.
    ///
    /// What it buys is a shape that would otherwise have to be *expressed* as
    /// two overlaid layers: a band of a curve is `Intersect` with the band's
    /// box, and a fill that must not double-blend under its own stroke is
    /// `Exclude` with the stroke's outline ([`widen`](Self::widen)). Both come
    /// back as ONE fillable geometry, so the consumer that draws it needs one
    /// sprite, one brush and no compositing between the two inputs.
    ///
    /// Both operands must be in the SAME coordinate space — there is no
    /// transform parameter here, so a caller combining geometries authored
    /// against different origins must translate one before building it.
    ///
    /// `tolerance` is in the geometries' own units, as
    /// [`widen`](Self::widen)'s is. Combining has to find where the two inputs
    /// cross, and it splits segments at every crossing to do so, so the result
    /// is routinely larger than either input — count it with
    /// [`segment_count`](Self::segment_count) before putting it on a per-frame
    /// path.
    pub fn combine(
        &self,
        device: &GpuDevice,
        other: &Path,
        mode: CombineMode,
        tolerance: f32,
    ) -> Result<Path> {
        let raw = unsafe { device.d2d_factory().CreatePathGeometry()? };
        let sink = unsafe { raw.Open()? };
        unsafe {
            self.raw
                .CombineWithGeometry(other.raw(), mode.to_abi(), None, tolerance, &sink)
                .ok()?;
            sink.Close().ok()?;
        }
        Ok(Path { raw })
    }

    /// How many segments this path holds — the size of what a consumer is handed.
    ///
    /// Worth knowing after a [`widen`](Self::widen): the outline feeds a
    /// composition path, and the compositor pays to ingest it in proportion to
    /// this.
    pub fn segment_count(&self) -> u32 {
        unsafe { self.raw.GetSegmentCount().unwrap_or(0) }
    }

    /// How many figures this path holds.
    pub fn figure_count(&self) -> u32 {
        unsafe { self.raw.GetFigureCount().unwrap_or(0) }
    }

    /// Returns whether the point lies within the filled area of the path.
    pub fn fill_contains_point(&self, point: Vector2) -> bool {
        unsafe {
            self.raw
                .FillContainsPoint(point, None, DEFAULT_FLATTENING_TOLERANCE)
                .unwrap()
                .as_bool()
        }
    }

    /// Returns whether the point lies on the path's stroke at the given width.
    pub fn stroke_contains_point(&self, point: Vector2, stroke_width: f32) -> bool {
        unsafe {
            self.raw
                .StrokeContainsPoint(
                    point,
                    stroke_width,
                    None,
                    None,
                    DEFAULT_FLATTENING_TOLERANCE,
                )
                .unwrap()
                .as_bool()
        }
    }

    /// Returns the axis-aligned bounding rectangle of the path.
    pub fn compute_bounds(&self) -> Rect {
        let bounds = unsafe { self.raw.GetBounds(None).unwrap() };
        Rect {
            left: bounds.left,
            top: bounds.top,
            right: bounds.right,
            bottom: bounds.bottom,
        }
    }
}

/// Type-safe path builder.
///
/// ```ignore
/// let path = PathBuilder::new(&device)?
///     .begin(point)
///     .line_to(point2)
///     .bezier_to(c1, c2, end)
///     .close()
///     .build()?;
/// ```
pub struct PathBuilder {
    sink: ID2D1GeometrySink,
    geometry: ID2D1PathGeometry1,
}

impl PathBuilder {
    /// Creates a new path builder for the given device.
    pub fn new(device: &GpuDevice) -> Result<Self> {
        let geometry = unsafe { device.d2d_factory().CreatePathGeometry()? };
        let sink = unsafe { geometry.Open()? };
        Ok(Self { sink, geometry })
    }

    /// Begin a filled figure.
    pub fn begin(self, start: Vector2) -> PathFigure {
        unsafe {
            self.sink.BeginFigure(start, D2D1_FIGURE_BEGIN_FILLED);
        }
        PathFigure {
            sink: self.sink,
            geometry: self.geometry,
        }
    }

    /// Begin a hollow (stroke-only) figure.
    pub fn begin_hollow(self, start: Vector2) -> PathFigure {
        unsafe {
            self.sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
        }
        PathFigure {
            sink: self.sink,
            geometry: self.geometry,
        }
    }

    /// Finalize the path geometry.
    pub fn build(self) -> Result<Path> {
        unsafe { self.sink.Close().ok()? };
        Ok(Path { raw: self.geometry })
    }

    /// Builds a closed, filled polygon from a sequence of points.
    ///
    /// Convenience for `begin(first).line_to(rest)…close().build()`. Returns an
    /// error if `points` yields no points.
    pub fn polygon(self, points: impl IntoIterator<Item = Vector2>) -> Result<Path> {
        let mut points = points.into_iter();
        let Some(first) = points.next() else {
            return Err(Error::empty());
        };
        // Collected and submitted as ONE run rather than a call per vertex: the
        // iterator has to be drained either way, and draining it into a buffer
        // costs one allocation against a COM crossing per point.
        let rest: Vec<Vector2> = points.collect();
        self.begin(first).line_to_many(&rest).close().build()
    }
}

/// A figure within a path being built.
///
/// Returned by [`PathBuilder::begin`]. Add segments with [`line_to`](Self::line_to)
/// and [`bezier_to`](Self::bezier_to), then call [`close`](Self::close) or
/// [`end_open`](Self::end_open) to return to `PathBuilder`.
pub struct PathFigure {
    sink: ID2D1GeometrySink,
    geometry: ID2D1PathGeometry1,
}

impl PathFigure {
    /// Adds a straight line segment to the given point.
    pub fn line_to(self, point: Vector2) -> Self {
        unsafe { self.sink.AddLine(point) };
        self
    }

    /// Adds a whole run of straight line segments in ONE call.
    ///
    /// The reason to reach for this over [`line_to`](Self::line_to) is that the
    /// per-segment form is one COM call per point: a 512-point polyline costs 512
    /// crossings to describe 4KB of coordinates. A sampled curve — a frequency
    /// response, a waveform envelope, a spectrum trace — is exactly that shape,
    /// and it is rebuilt whenever the data moves, so the crossings land on the
    /// interactive path where they are least affordable.
    ///
    /// `points` is passed straight through to `AddLines`, so a caller holding its
    /// coordinates as a flat contiguous array can submit a run without copying.
    pub fn line_to_many(self, points: &[Vector2]) -> Self {
        if !points.is_empty() {
            unsafe { self.sink.AddLines(points) };
        }
        self
    }

    /// Adds a cubic Bézier segment with the given control points and end point.
    pub fn bezier_to(self, control1: Vector2, control2: Vector2, end: Vector2) -> Self {
        let segment = D2D1_BEZIER_SEGMENT {
            point1: control1,
            point2: control2,
            point3: end,
        };
        unsafe { self.sink.AddBezier(&segment) };
        self
    }

    /// [`line_to_many`](Self::line_to_many) for a caller holding its coordinates
    /// as flat `x, y` pairs rather than as [`Vector2`]s.
    ///
    /// The cast is the point: `Vector2` is `#[repr(C)] { x: f32, y: f32 }`, so a
    /// flat pair array already IS the array `AddLines` takes, and the run reaches
    /// D2D without being rebuilt into a second buffer first. Sampled-curve data
    /// is normally stored exactly this way — two flat arrays are cheaper to
    /// compare and to keep — so the alternative is a per-frame repack.
    ///
    /// A trailing odd coordinate is dropped rather than read past: the caller
    /// owns pairing its own data, and reading a half point off the end is the one
    /// failure here that would be memory-unsafe.
    pub fn line_to_flat(self, xy: &[f32]) -> Self {
        debug_assert!(xy.len().is_multiple_of(2), "flat point data must be x, y pairs");
        let n = xy.len() / 2;
        if n == 0 {
            return self;
        }
        // SAFETY: `Vector2` is `#[repr(C)]` over two `f32`, so `n` of them span
        // exactly `2 * n` contiguous `f32` with the same alignment (both 4). The
        // length is floored to whole pairs above, so the range is in bounds.
        let points = unsafe { core::slice::from_raw_parts(xy.as_ptr().cast::<Vector2>(), n) };
        unsafe { self.sink.AddLines(points) };
        self
    }

    /// The cubic counterpart to [`line_to_flat`](Self::line_to_flat): flat `x, y`
    /// pairs read as `c1, c2, end` triples, one Bézier per six floats.
    ///
    /// Trailing floats that do not complete a segment are dropped, for the reason
    /// given there.
    pub fn bezier_to_flat(self, xy: &[f32]) -> Self {
        debug_assert!(xy.len().is_multiple_of(6), "flat bezier data must be c1, c2, end triples");
        let n = xy.len() / 6;
        if n == 0 {
            return self;
        }
        // SAFETY: `D2D1_BEZIER_SEGMENT` is `#[repr(C)]` over three `Vector2`, so
        // `n` of them span exactly `6 * n` contiguous `f32` at the same alignment
        // (both 4). The length is floored to whole segments above.
        let segments =
            unsafe { core::slice::from_raw_parts(xy.as_ptr().cast::<D2D1_BEZIER_SEGMENT>(), n) };
        unsafe { self.sink.AddBeziers(segments) };
        self
    }

    /// Close the current figure and connect back to the start point.
    pub fn close(self) -> PathBuilder {
        unsafe { self.sink.EndFigure(D2D1_FIGURE_END_CLOSED) };
        PathBuilder {
            sink: self.sink,
            geometry: self.geometry,
        }
    }

    /// End the current figure without closing.
    pub fn end_open(self) -> PathBuilder {
        unsafe { self.sink.EndFigure(D2D1_FIGURE_END_OPEN) };
        PathBuilder {
            sink: self.sink,
            geometry: self.geometry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An axis-aligned rectangle as a closed path.
    fn rect(device: &GpuDevice, l: f32, t: f32, r: f32, b: f32) -> Path {
        PathBuilder::new(device)
            .unwrap()
            .polygon([
                Vector2::new(l, t),
                Vector2::new(r, t),
                Vector2::new(r, b),
                Vector2::new(l, b),
            ])
            .unwrap()
    }

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    /// `Intersect` keeps only the overlap — the case a lit span is: a band of a
    /// curve, expressed as one region instead of two overlaid layers.
    ///
    /// Asserted on BOUNDS rather than a segment count, because how many segments
    /// Direct2D emits for a rectangle is its business; where the region lies is
    /// this call's contract.
    #[test]
    fn combine_intersect_keeps_only_the_overlap() {
        let Ok(gpu) = GpuDevice::new_or_warp() else {
            eprintln!("no D3D device available; skipping");
            return;
        };
        let a = rect(&gpu, 0.0, 0.0, 100.0, 100.0);
        let b = rect(&gpu, 60.0, 40.0, 200.0, 80.0);

        let hit = a.combine(&gpu, &b, CombineMode::Intersect, 0.25).unwrap();
        let bounds = hit.compute_bounds();

        assert!(
            approx(bounds.left, 60.0, 0.5)
                && approx(bounds.top, 40.0, 0.5)
                && approx(bounds.right, 100.0, 0.5)
                && approx(bounds.bottom, 80.0, 0.5),
            "intersection bounds were {bounds:?}, wanted (60, 40)-(100, 80)"
        );
        // A point inside both inputs survives; one inside only `a` does not.
        assert!(hit.fill_contains_point(Vector2::new(80.0, 60.0)));
        assert!(!hit.fill_contains_point(Vector2::new(20.0, 60.0)));
    }

    /// `Exclude` is the asymmetric mode — the receiver minus the argument. This
    /// is the direction a fill takes to stop double-blending under its own
    /// stroke, so getting the operand order backwards is a real hazard.
    #[test]
    fn combine_exclude_removes_only_the_argument() {
        let Ok(gpu) = GpuDevice::new_or_warp() else {
            eprintln!("no D3D device available; skipping");
            return;
        };
        let outer = rect(&gpu, 0.0, 0.0, 100.0, 100.0);
        let bite = rect(&gpu, 40.0, 40.0, 60.0, 60.0);

        let carved = outer.combine(&gpu, &bite, CombineMode::Exclude, 0.25).unwrap();
        assert!(
            carved.fill_contains_point(Vector2::new(10.0, 10.0)),
            "the receiver's own area must survive"
        );
        assert!(
            !carved.fill_contains_point(Vector2::new(50.0, 50.0)),
            "the argument's area must be gone"
        );

        // Reversed, the same pair yields the other difference — nothing of the
        // small rect is left once the large one is removed from it.
        let inverse = bite.combine(&gpu, &outer, CombineMode::Exclude, 0.25).unwrap();
        assert!(!inverse.fill_contains_point(Vector2::new(50.0, 50.0)));
    }

    /// `Union` covers both, including a point in neither operand alone would not
    /// reach.
    #[test]
    fn combine_union_covers_both() {
        let Ok(gpu) = GpuDevice::new_or_warp() else {
            eprintln!("no D3D device available; skipping");
            return;
        };
        let a = rect(&gpu, 0.0, 0.0, 50.0, 50.0);
        let b = rect(&gpu, 50.0, 0.0, 100.0, 50.0);
        let both = a.combine(&gpu, &b, CombineMode::Union, 0.25).unwrap();

        assert!(both.fill_contains_point(Vector2::new(25.0, 25.0)));
        assert!(both.fill_contains_point(Vector2::new(75.0, 25.0)));
        let bounds = both.compute_bounds();
        assert!(approx(bounds.right, 100.0, 0.5), "union bounds were {bounds:?}");
    }

    /// The contract hit-testing a curve rests on: a stroke's WIDENED outline
    /// discriminates the line from its bounding box.
    ///
    /// A diagonal is the case that matters — its box is almost entirely empty,
    /// which is exactly why box hit-testing hands a curve every pointer in its
    /// card. The far corner is inside the box and nowhere near the stroke; the
    /// midpoint is on it.
    #[test]
    fn widened_stroke_contains_the_line_not_its_box() {
        let Ok(gpu) = GpuDevice::new_or_warp() else {
            eprintln!("no D3D device available; skipping");
            return;
        };
        let line = PathBuilder::new(&gpu)
            .unwrap()
            .begin_hollow(Vector2::new(0.0, 0.0))
            .line_to(Vector2::new(100.0, 100.0))
            .end_open()
            .build()
            .unwrap();
        let style = gpu
            .create_stroke_style(&StrokeStyleBuilder::new().caps(CapStyle::Round).line_join(LineJoin::Round))
            .unwrap();
        let outline = line.widen(&gpu, 4.0, &style, 0.25).unwrap();

        assert!(
            outline.fill_contains_point(Vector2::new(50.0, 50.0)),
            "a point ON the stroke must hit"
        );
        assert!(
            !outline.fill_contains_point(Vector2::new(95.0, 5.0)),
            "a point inside the BOX but off the stroke must miss"
        );
        // The box itself is the whole diagonal's extent, which is what makes the
        // distinction worth paying for.
        let bounds = outline.compute_bounds();
        assert!(bounds.right > 90.0 && bounds.bottom > 90.0, "bounds were {bounds:?}");
    }
}
