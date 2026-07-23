use std::sync::Arc;

use super::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
    Line,
    /// Arbitrary geometry carried in [`Shape::geometry`] — splines, polylines,
    /// beziers, closed areas. The other three kinds derive their geometry from
    /// the node's box; this one is the only kind that transports it.
    Path,
}

/// One segment verb in a [`PathData`] figure.
///
/// Stored apart from the points so the geometry is two flat, contiguous arrays
/// rather than a `Vec` of fat enums — a 512-point spline is 4KB of coordinates
/// and 512 bytes of verbs, and comparing two of them for the no-op gate is two
/// `memcmp`s.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PathVerb {
    /// Begin a new figure at the next point.
    Move,
    /// Straight segment to the next point.
    Line,
    /// Cubic bezier through the next THREE points — `c1`, `c2`, `end`.
    Cubic,
    /// Close the current figure back to its start point. Consumes no points.
    Close,
}

impl PathVerb {
    /// How many points this verb consumes from the point stream.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Move | Self::Line => 1,
            Self::Cubic => 3,
            Self::Close => 0,
        }
    }
}

/// Resolution-independent path geometry in the node's **local DIP space** —
/// `(0, 0)` is the shape's top-left corner, not the window's.
///
/// Local coordinates are what let the same geometry survive a move: a node that
/// slides across the window repaints nothing, because nothing in here changed.
/// A node that *resizes* does need new geometry, which is the honest cost — a
/// curve sampled for one width is not the curve for another.
/// The fields are crate-visible rather than public so `points.len() == 2 * Σ
/// verb.arity()` holds **by construction**: [`ShapePath`] is the only thing that
/// can append, and it always appends a verb with its points together. Outside
/// the crate the geometry is read through the accessors.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathData {
    pub(crate) verbs: Vec<PathVerb>,
    /// `x, y` pairs, flat.
    pub(crate) points: Vec<f32>,
}

impl PathData {
    #[must_use]
    pub fn verbs(&self) -> &[PathVerb] {
        &self.verbs
    }
    /// Flat `x, y` pairs, indexed by the running sum of [`PathVerb::arity`].
    #[must_use]
    pub fn points(&self) -> &[f32] {
        &self.points
    }
}

/// A built, immutable [`PathData`] behind a shared pointer.
///
/// Shared because the diff carries it by value through the prop pipeline and
/// across the reconciler→backend command buffer; `Arc` makes each of those hops
/// a refcount bump rather than a copy of every coordinate.
#[derive(Clone, Debug)]
pub struct PathGeometry(Arc<PathData>);

impl PathGeometry {
    #[must_use]
    pub fn data(&self) -> &PathData {
        &self.0
    }
    /// Whether the geometry would draw nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.verbs.is_empty()
    }
}

/// Equality is what gates the repaint, so it takes the cheap answer first: an
/// app that hands back the SAME `Arc` (a cached curve) settles in one pointer
/// compare. An app that rebuilt an identical curve falls through to the content
/// compare and still correctly repaints nothing — geometry is content-addressed
/// here, not identity-addressed.
impl PartialEq for PathGeometry {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

/// Builder for [`PathGeometry`].
///
/// Named for the shape widget rather than `PathBuilder` so it cannot collide
/// with `windows_canvas::PathBuilder` — apps that draw on a canvas *and* mount
/// path shapes import both.
///
/// Non-finite coordinates are dropped at the verb that carries them: a `NaN`
/// reaching the point array would make the geometry unequal to itself and
/// repaint forever, so the gate is here, at the one place points are admitted.
#[derive(Clone, Debug, Default)]
pub struct ShapePath {
    data: PathData,
}

impl ShapePath {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve room for `verbs` verbs and their points — worth calling when
    /// sampling a curve of known length.
    #[must_use]
    pub fn with_capacity(verbs: usize) -> Self {
        Self {
            data: PathData {
                verbs: Vec::with_capacity(verbs),
                points: Vec::with_capacity(verbs * 2),
            },
        }
    }

    fn push(mut self, verb: PathVerb, pts: &[f32]) -> Self {
        debug_assert_eq!(verb.arity() * 2, pts.len());
        if pts.iter().any(|v| !v.is_finite()) {
            return self;
        }
        self.data.verbs.push(verb);
        self.data.points.extend_from_slice(pts);
        self
    }

    #[must_use]
    pub fn move_to(self, x: f64, y: f64) -> Self {
        self.push(PathVerb::Move, &[x as f32, y as f32])
    }

    #[must_use]
    pub fn line_to(self, x: f64, y: f64) -> Self {
        self.push(PathVerb::Line, &[x as f32, y as f32])
    }

    #[must_use]
    pub fn cubic_to(self, c1x: f64, c1y: f64, c2x: f64, c2y: f64, x: f64, y: f64) -> Self {
        self.push(
            PathVerb::Cubic,
            &[c1x as f32, c1y as f32, c2x as f32, c2y as f32, x as f32, y as f32],
        )
    }

    /// Close the current figure. A closed figure is what an area fill wants; an
    /// open one is what a stroked curve wants.
    #[must_use]
    pub fn close(mut self) -> Self {
        // A leading or doubled `Close` has no figure to close and would desync
        // nothing but still emit a stray verb — drop it here.
        if matches!(self.data.verbs.last(), None | Some(PathVerb::Close)) {
            return self;
        }
        self.data.verbs.push(PathVerb::Close);
        self
    }

    /// Append a whole polyline: `move_to` the first point, `line_to` the rest.
    /// The wavelet, the spectrum chord run and the waveform envelope are all
    /// this one call.
    #[must_use]
    pub fn polyline(mut self, points: impl IntoIterator<Item = (f64, f64)>) -> Self {
        let mut first = true;
        for (x, y) in points {
            self = if first { self.move_to(x, y) } else { self.line_to(x, y) };
            // Only a point that was actually admitted opens the figure — if the
            // first point was non-finite the next finite one must still `Move`.
            first = self.data.verbs.is_empty();
        }
        self
    }

    #[must_use]
    pub fn build(self) -> PathGeometry {
        PathGeometry(Arc::new(self.data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Total points the verb stream claims, which is what the backend's replay
    /// walks. If this ever disagrees with `points.len()`, the replay reads off
    /// the end of the array — so it is the invariant worth asserting directly.
    fn claimed(d: &PathData) -> usize {
        d.verbs().iter().map(|v| v.arity() * 2).sum()
    }

    #[test]
    fn verb_stream_and_points_stay_in_lockstep() {
        let g = ShapePath::new()
            .move_to(0.0, 0.0)
            .line_to(1.0, 2.0)
            .cubic_to(3.0, 4.0, 5.0, 6.0, 7.0, 8.0)
            .close()
            .build();
        let d = g.data();
        assert_eq!(d.verbs(), &[PathVerb::Move, PathVerb::Line, PathVerb::Cubic, PathVerb::Close]);
        assert_eq!(claimed(d), d.points().len());
        assert_eq!(d.points(), &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn non_finite_points_are_dropped_whole_verb_at_a_time() {
        let g = ShapePath::new()
            .move_to(0.0, 0.0)
            .line_to(f64::NAN, 1.0)
            .cubic_to(0.0, 0.0, f64::INFINITY, 0.0, 1.0, 1.0)
            .line_to(2.0, 2.0)
            .build();
        let d = g.data();
        // The NaN line and the infinite cubic are gone; nothing partial remains.
        assert_eq!(d.verbs(), &[PathVerb::Move, PathVerb::Line]);
        assert_eq!(claimed(d), d.points().len());
        assert_eq!(d.points(), &[0.0, 0.0, 2.0, 2.0]);
    }

    /// A `NaN` in the point array would make the geometry unequal to itself,
    /// and an always-unequal prop repaints on every single reconcile forever.
    #[test]
    fn geometry_is_equal_to_itself_even_when_fed_garbage() {
        let g = ShapePath::new().move_to(f64::NAN, 0.0).line_to(1.0, 1.0).build();
        assert_eq!(g, g.clone());
    }

    #[test]
    fn identical_content_compares_equal_so_a_rebuilt_curve_does_not_repaint() {
        let build = || ShapePath::new().move_to(0.0, 0.0).line_to(10.0, 5.0).build();
        let (a, b) = (build(), build());
        // Distinct allocations — this is the content path, not the pointer one.
        assert!(!Arc::ptr_eq(&a.0, &b.0));
        assert_eq!(a, b);
    }

    #[test]
    fn differing_content_compares_unequal_so_a_changed_curve_does_repaint() {
        let a = ShapePath::new().move_to(0.0, 0.0).line_to(10.0, 5.0).build();
        let b = ShapePath::new().move_to(0.0, 0.0).line_to(10.0, 6.0).build();
        assert_ne!(a, b);
        // Same points, different verbs, must still differ.
        let c = ShapePath::new().move_to(0.0, 0.0).move_to(10.0, 5.0).build();
        assert_ne!(a, c);
    }

    #[test]
    fn polyline_opens_one_figure_and_lines_the_rest() {
        let g = ShapePath::new().polyline([(0.0, 0.0), (1.0, 1.0), (2.0, 4.0)]).build();
        assert_eq!(g.data().verbs(), &[PathVerb::Move, PathVerb::Line, PathVerb::Line]);
    }

    /// If the first point is unusable the figure is not open yet, so the next
    /// usable point must still `Move` — a `Line` here would be a segment with
    /// no figure, which the backend refuses to draw at all.
    #[test]
    fn polyline_still_opens_a_figure_when_its_first_point_is_dropped() {
        let g = ShapePath::new().polyline([(f64::NAN, 0.0), (1.0, 1.0), (2.0, 4.0)]).build();
        assert_eq!(g.data().verbs(), &[PathVerb::Move, PathVerb::Line]);
        assert_eq!(g.data().points(), &[1.0, 1.0, 2.0, 4.0]);
    }

    #[test]
    fn a_close_with_no_open_figure_is_dropped() {
        assert!(ShapePath::new().close().build().is_empty());
        let g = ShapePath::new().move_to(0.0, 0.0).line_to(1.0, 1.0).close().close().build();
        assert_eq!(g.data().verbs().iter().filter(|v| **v == PathVerb::Close).count(), 1);
    }

    #[test]
    fn path_shape_transports_its_geometry_as_a_prop() {
        let g = ShapePath::new().move_to(0.0, 0.0).line_to(1.0, 1.0).build();
        let shape = Shape::path(g.clone()).stroke(Color::rgb(255, 0, 0)).stroke_thickness(2.0);
        assert_eq!(shape.kind(), ControlKind::Path);
        assert!(shape
            .bindings()
            .iter()
            .any(|b| matches!(b, Binding::Prop(Prop::PathGeometry, PropValue::Path(p)) if *p == g)));
    }

    #[test]
    fn a_gradient_fill_rides_the_shared_stops_prop() {
        let stops = vec![(0.0, Color::rgb(255, 0, 0)), (1.0, Color::rgb(0, 0, 255))];
        let shape = Shape::path(ShapePath::new().move_to(0.0, 0.0).line_to(1.0, 1.0).build())
            .fill_gradient(stops.clone());
        assert!(shape.bindings().iter().any(|b| matches!(
            b,
            Binding::Prop(Prop::GradientStops, PropValue::GradientStops(s)) if *s == stops
        )));
    }

    /// Both ends must be emitted together: the backend springs `TrimEnd` and
    /// snaps `TrimStart`, and a half-specified window would animate against a
    /// stale crop.
    #[test]
    fn trim_emits_both_ends() {
        let shape = Shape::path(ShapePath::new().move_to(0.0, 0.0).line_to(1.0, 1.0).build())
            .trim(0.25, 0.75);
        let b = shape.bindings();
        assert!(b.iter().any(
            |b| matches!(b, Binding::Prop(Prop::TrimStart, PropValue::F64(v)) if *v == 0.25)
        ));
        assert!(b.iter().any(
            |b| matches!(b, Binding::Prop(Prop::TrimEnd, PropValue::F64(v)) if *v == 0.75)
        ));
    }

    /// A glow emits its colour and blur together — the backend needs both to
    /// bake the halo, and a colour with no blur (or the reverse) is not a glow.
    #[test]
    fn glow_emits_colour_and_blur() {
        let shape = Shape::path(ShapePath::new().move_to(0.0, 0.0).line_to(1.0, 1.0).build())
            .glow(Color::rgb(0, 200, 255), 6.0);
        let b = shape.bindings();
        assert!(b.iter().any(
            |b| matches!(b, Binding::Prop(Prop::GlowColor, PropValue::Color(c)) if *c == Color::rgb(0, 200, 255))
        ));
        assert!(b.iter().any(
            |b| matches!(b, Binding::Prop(Prop::GlowBlur, PropValue::F64(v)) if *v == 6.0)
        ));
    }

    /// A path with no glow must emit no glow props — an unasked-for halo costs
    /// a baked FP16 surface, so the default is genuinely nothing.
    #[test]
    fn an_unglowed_path_emits_no_glow() {
        let shape = Shape::path(ShapePath::new().move_to(0.0, 0.0).line_to(1.0, 1.0).build());
        assert!(!shape
            .bindings()
            .iter()
            .any(|b| matches!(b, Binding::Prop(Prop::GlowColor | Prop::GlowBlur, _))));
    }

    /// An untrimmed path must emit NO trim props, so it takes the backend's
    /// born-at-full-extent default rather than being pinned to one here.
    #[test]
    fn an_untrimmed_path_emits_no_trim() {
        let shape = Shape::path(ShapePath::new().move_to(0.0, 0.0).line_to(1.0, 1.0).build());
        assert!(!shape
            .bindings()
            .iter()
            .any(|b| matches!(b, Binding::Prop(Prop::TrimStart | Prop::TrimEnd, _))));
    }

    /// The box-derived kinds must not start transporting an empty geometry —
    /// that would put a prop on every rectangle in a tree.
    #[test]
    fn box_kinds_transport_no_geometry() {
        for shape in [Shape::rectangle(), Shape::ellipse(), Shape::line(0.0, 0.0, 1.0, 1.0)] {
            assert!(!shape
                .bindings()
                .iter()
                .any(|b| matches!(b, Binding::Prop(Prop::PathGeometry, _))));
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub kind: ShapeKind,
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_thickness: Option<f64>,
    pub corner_radius: Option<f64>,
    pub line: LineEndpoints,
    /// Set only by [`ShapeKind::Path`]; `None` for the box-derived kinds.
    pub geometry: Option<PathGeometry>,
    /// Gradient ramp for a path's FILL, `(position 0..1, linear-scRGB colour)`.
    /// Overrides [`Self::fill`]'s flat colour when present.
    pub fill_gradient: Option<Vec<(f64, Color)>>,
    /// Which way [`Self::fill_gradient`] runs across the shape's box.
    pub fill_gradient_axis: GradientAxis,
    /// Gradient ramp for a path's STROKE, `(position 0..1, linear-scRGB colour)`.
    /// Overrides [`Self::stroke`]'s flat colour when present.
    ///
    /// Separate from [`Self::fill_gradient`], and with its own axis, because the
    /// two ramps describe different things: an area fill under a curve fades
    /// DOWN the plot, while a line's own ramp runs ALONG the line. A single
    /// shared ramp would force one reading on both and the layers would stop
    /// reading as two.
    pub stroke_gradient: Option<Vec<(f64, Color)>>,
    /// Which way [`Self::stroke_gradient`] runs across the shape's box.
    pub stroke_gradient_axis: GradientAxis,
    /// `(start, end)` fraction of the geometry's length to draw, `0..1`.
    /// `end` animates on the compositor, so a curve can draw itself on with no
    /// app frame. `None` leaves the path at full extent.
    pub trim: Option<(f64, f64)>,
    /// A pre-blurred FP16 glow behind the stroke: `(colour, blur σ in DIPs)`.
    /// The backend bakes a soft halo of `colour` once per geometry change and
    /// composites it under the stroke — the FabFilter bloom, retained rather
    /// than repainted. `None` draws no glow. Only a stroked path glows.
    pub glow: Option<(Color, f64)>,
    /// A multi-hue ramp for the glow, running ALONG the box like the stroke's — the
    /// bloom coloured by frequency instead of a single tint. `None` (the default)
    /// leaves the glow on its flat [`glow`](Self::glow) colour; a non-empty ramp
    /// makes the halo carry each stop's hue, its magnitude free to run above paper
    /// white (the FP16 source is unclamped). The blur σ still comes from
    /// [`glow`](Self::glow), so a gradient glow states both.
    pub glow_gradient: Option<Vec<(f64, Color)>>,
    /// Which way the glow ramp runs (defaults to horizontal, matching the stroke).
    pub glow_gradient_axis: GradientAxis,
    /// Post-mount callback, for taking an [`ElementHandle`] to this shape — its one
    /// use is [`ElementHandle::live_opacity`], for a shape whose geometry reconciles
    /// normally but whose opacity a render pump eases (the Simple preview's lit spans).
    pub mounted: Option<Callback<MountInfo>>,
}
/// The axis a gradient ramp runs along, in the shape's own local box: stop `0.0`
/// sits at the leading edge of that axis and stop `1.0` at the trailing one.
///
/// The ramp is measured across the whole box and masked by the shape, so the
/// axis is a property of the BOX, not of the path's direction of travel — a
/// horizontal ramp under a curve that doubles back still reads left-to-right.
/// For a plot whose x is frequency, that is exactly what "colour by frequency"
/// means.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum GradientAxis {
    /// Left to right across the box.
    Horizontal = 0,
    /// Top to bottom down the box. The default: the ramp that existed before
    /// this axis did was the curve underfill, which fades down the plot.
    #[default]
    Vertical = 1,
}

impl GradientAxis {
    /// Reconstruct from the transported discriminant, defaulting to the variant
    /// a shape is born with rather than failing — an unknown axis is a newer
    /// widget talking to an older backend, and a gradient drawn the wrong way is
    /// a better answer than no gradient at all.
    #[must_use]
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Horizontal,
            _ => Self::Vertical,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct LineEndpoints {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}
impl Default for Shape {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            kind: ShapeKind::Rectangle,
            fill: None,
            stroke: None,
            stroke_thickness: None,
            corner_radius: None,
            line: LineEndpoints::default(),
            geometry: None,
            fill_gradient: None,
            fill_gradient_axis: GradientAxis::Vertical,
            stroke_gradient: None,
            stroke_gradient_axis: GradientAxis::Horizontal,
            trim: None,
            glow: None,
            glow_gradient: None,
            glow_gradient_axis: GradientAxis::Horizontal,
            mounted: None,
        }
    }
}
impl Shape {
    pub fn rectangle() -> Self {
        Self {
            kind: ShapeKind::Rectangle,
            ..Default::default()
        }
    }
    pub fn ellipse() -> Self {
        Self {
            kind: ShapeKind::Ellipse,
            ..Default::default()
        }
    }
    pub fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            kind: ShapeKind::Line,
            line: LineEndpoints { x1, y1, x2, y2 },
            ..Default::default()
        }
    }
    /// A shape drawing `geometry`, in the node's own local DIP space.
    ///
    /// The node still gets its box from layout as any other shape does — the
    /// geometry is drawn into that box, unscaled and unclipped, so a caller
    /// that sampled a curve for a different width will see it overflow. Sample
    /// against the size you laid out.
    pub fn path(geometry: PathGeometry) -> Self {
        Self {
            kind: ShapeKind::Path,
            geometry: Some(geometry),
            ..Default::default()
        }
    }
    pub fn fill(mut self, v: Color) -> Self {
        self.fill = Some(v);
        self
    }
    pub fn fill_rgb(mut self, r: u8, g: u8, b: u8) -> Self {
        self.fill = Some(Color::rgb(r, g, b));
        self
    }
    pub fn stroke(mut self, v: Color) -> Self {
        self.stroke = Some(v);
        self
    }
    pub fn stroke_thickness(mut self, v: f64) -> Self {
        self.stroke_thickness = Some(v);
        self
    }
    pub fn corner_radius(mut self, v: f64) -> Self {
        self.corner_radius = Some(v);
        self
    }
    /// Fill a path with a gradient ramp instead of a flat colour, running DOWN
    /// the shape's box — the curve underfill, which fades away from the line.
    /// Use [`Self::fill_gradient_along`] for any other axis.
    pub fn fill_gradient(mut self, stops: Vec<(f64, Color)>) -> Self {
        self.fill_gradient = Some(stops);
        self.fill_gradient_axis = GradientAxis::Vertical;
        self
    }
    /// [`Self::fill_gradient`] on a stated axis.
    pub fn fill_gradient_along(mut self, axis: GradientAxis, stops: Vec<(f64, Color)>) -> Self {
        self.fill_gradient = Some(stops);
        self.fill_gradient_axis = axis;
        self
    }
    /// Stroke a path with a gradient ramp instead of a flat colour, running
    /// ACROSS the shape's box — a response curve coloured by frequency, a trace
    /// coloured by time. Use [`Self::stroke_gradient_along`] for any other axis.
    ///
    /// The fill keeps its own ramp and its own axis: the two describe different
    /// things and a shared one would force one reading on both.
    pub fn stroke_gradient(mut self, stops: Vec<(f64, Color)>) -> Self {
        self.stroke_gradient = Some(stops);
        self.stroke_gradient_axis = GradientAxis::Horizontal;
        self
    }
    /// [`Self::stroke_gradient`] on a stated axis.
    pub fn stroke_gradient_along(mut self, axis: GradientAxis, stops: Vec<(f64, Color)>) -> Self {
        self.stroke_gradient = Some(stops);
        self.stroke_gradient_axis = axis;
        self
    }
    /// Draw only `start..end` of the path's length (fractions of `0..1`).
    pub fn trim(mut self, start: f64, end: f64) -> Self {
        self.trim = Some((start, end));
        self
    }
    /// Bake a soft glow of `color` (blur σ `blur` DIPs) behind the stroke.
    pub fn glow(mut self, color: Color, blur: f64) -> Self {
        self.glow = Some((color, blur));
        self
    }
    /// Give the glow a multi-hue ramp instead of the flat [`glow`](Self::glow)
    /// colour — the halo coloured ALONG the box, running with the stroke. Still
    /// needs [`glow`](Self::glow) for the blur σ (its colour then serves only as the
    /// fallback if the ramp cannot be built). Use [`glow_gradient_along`](Self::glow_gradient_along)
    /// for any other axis.
    pub fn glow_gradient(mut self, stops: Vec<(f64, Color)>) -> Self {
        self.glow_gradient = Some(stops);
        self.glow_gradient_axis = GradientAxis::Horizontal;
        self
    }
    /// [`glow_gradient`](Self::glow_gradient) on a stated axis.
    pub fn glow_gradient_along(mut self, axis: GradientAxis, stops: Vec<(f64, Color)>) -> Self {
        self.glow_gradient = Some(stops);
        self.glow_gradient_axis = axis;
        self
    }
    /// Callback invoked once after the shape is created, handed an [`ElementHandle`]
    /// for it. Take [`ElementHandle::live_opacity`] here to ease this shape's opacity
    /// from a producer thread while its geometry keeps reconciling normally.
    pub fn on_mounted(mut self, f: impl Fn(ElementHandle) + 'static) -> Self {
        self.mounted = Some(Callback::new(move |info: MountInfo| {
            f(ElementHandle::from(info));
        }));
        self
    }
}

impl Widget for Shape {
    fn kind(&self) -> ControlKind {
        match self.kind {
            ShapeKind::Rectangle => ControlKind::Rectangle,
            ShapeKind::Ellipse => ControlKind::Ellipse,
            ShapeKind::Line => ControlKind::Line,
            ShapeKind::Path => ControlKind::Path,
        }
    }
    fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }
    fn modifiers(&self) -> &Modifiers {
        &self.modifiers
    }
    fn on_mounted_callback(&self) -> Option<&Callback<MountInfo>> {
        self.mounted.as_ref()
    }
    fn bindings(&self) -> PropBindings {
        let mut out = Vec::with_capacity(5);
        if let Some(fill) = &self.fill {
            out.push(Binding::Prop(Prop::Fill, PropValue::Color(*fill)));
        }
        if let Some(stroke) = &self.stroke {
            out.push(Binding::Prop(
                Prop::Stroke,
                PropValue::Color(*stroke),
            ));
        }
        if let Some(th) = self.stroke_thickness {
            out.push(Binding::Prop(Prop::StrokeThickness, PropValue::F64(th)));
        }
        if let Some(cr) = self.corner_radius
            && matches!(self.kind, ShapeKind::Rectangle) {
                out.push(Binding::Prop(Prop::CornerRadius, PropValue::F64(cr)));
            }
        if matches!(self.kind, ShapeKind::Line) {
            out.push(Binding::Prop(
                Prop::LineEndpoints,
                PropValue::LineEndpoints(self.line),
            ));
        }
        if let Some(g) = &self.geometry {
            out.push(Binding::Prop(
                Prop::PathGeometry,
                PropValue::Path(g.clone()),
            ));
        }
        // Each ramp carries its axis beside it rather than the two sharing one:
        // the motivating shape is a curve whose underfill fades DOWN while its
        // line colours ALONG the plot, so the axes genuinely differ per layer.
        if let Some(stops) = &self.fill_gradient {
            out.push(Binding::Prop(
                Prop::GradientStops,
                PropValue::GradientStops(stops.clone()),
            ));
            out.push(Binding::Prop(
                Prop::GradientAxis,
                PropValue::I32(self.fill_gradient_axis as i32),
            ));
        }
        if let Some(stops) = &self.stroke_gradient {
            out.push(Binding::Prop(
                Prop::StrokeGradientStops,
                PropValue::GradientStops(stops.clone()),
            ));
            out.push(Binding::Prop(
                Prop::StrokeGradientAxis,
                PropValue::I32(self.stroke_gradient_axis as i32),
            ));
        }
        if let Some((start, end)) = self.trim {
            out.push(Binding::Prop(Prop::TrimStart, PropValue::F64(start)));
            out.push(Binding::Prop(Prop::TrimEnd, PropValue::F64(end)));
        }
        if let Some((color, blur)) = self.glow {
            out.push(Binding::Prop(Prop::GlowColor, PropValue::Color(color)));
            out.push(Binding::Prop(Prop::GlowBlur, PropValue::F64(blur)));
        }
        if let Some(stops) = &self.glow_gradient {
            out.push(Binding::Prop(
                Prop::GlowStops,
                PropValue::GradientStops(stops.clone()),
            ));
            out.push(Binding::Prop(
                Prop::GlowGradientAxis,
                PropValue::I32(self.glow_gradient_axis as i32),
            ));
        }
        out
    }
}

#[cfg(test)]
mod gradient_tests {
    use super::*;
    use crate::backend::{Prop, PropValue};
    use crate::widget::Binding;

    fn ramp() -> Vec<(f64, Color)> {
        vec![
            (0.0, Color::rgb(0x00, 0xFF, 0x00)),
            (1.0, Color::rgb(0xFF, 0x00, 0x00)),
        ]
    }

    fn geometry() -> PathGeometry {
        ShapePath::new().move_to(0.0, 0.0).line_to(10.0, 10.0).build()
    }

    /// The axis a transported discriminant resolves to.
    fn axis_of(bindings: &[Binding], prop: Prop) -> Option<GradientAxis> {
        bindings.iter().find_map(|b| match b {
            Binding::Prop(p, PropValue::I32(v)) if *p == prop => Some(GradientAxis::from_i32(*v)),
            _ => None,
        })
    }

    fn has_stops(bindings: &[Binding], prop: Prop) -> bool {
        bindings
            .iter()
            .any(|b| matches!(b, Binding::Prop(p, PropValue::GradientStops(_)) if *p == prop))
    }

    // ── Each ramp carries its own axis ───────────────────────────────────────
    //
    // The motivating shape is a response curve: the area under it fades DOWN the
    // plot while the line itself colours ACROSS it. The two axes must therefore
    // be able to differ on one shape, which is the whole reason they are two
    // props rather than one.

    /// The defaults encode the physical meaning of each layer, so the common
    /// case needs no axis stated at all.
    #[test]
    fn each_layer_defaults_to_the_axis_its_job_implies() {
        let b = Shape::path(geometry())
            .fill_gradient(ramp())
            .stroke_gradient(ramp())
            .bindings();
        assert_eq!(axis_of(&b, Prop::GradientAxis), Some(GradientAxis::Vertical));
        assert_eq!(
            axis_of(&b, Prop::StrokeGradientAxis),
            Some(GradientAxis::Horizontal)
        );
    }

    /// And they are genuinely independent — a shape can state both, differently.
    #[test]
    fn the_two_axes_are_independent() {
        let b = Shape::path(geometry())
            .fill_gradient_along(GradientAxis::Horizontal, ramp())
            .stroke_gradient_along(GradientAxis::Vertical, ramp())
            .bindings();
        assert_eq!(
            axis_of(&b, Prop::GradientAxis),
            Some(GradientAxis::Horizontal)
        );
        assert_eq!(
            axis_of(&b, Prop::StrokeGradientAxis),
            Some(GradientAxis::Vertical)
        );
    }

    /// The ramps travel on separate props: handing the fill's stops to the
    /// stroke would paint the outline in the area's colours and the two layers
    /// would stop reading as two.
    #[test]
    fn a_fill_ramp_never_reaches_the_stroke() {
        let b = Shape::path(geometry()).fill_gradient(ramp()).bindings();
        assert!(has_stops(&b, Prop::GradientStops));
        assert!(!has_stops(&b, Prop::StrokeGradientStops));
        assert!(axis_of(&b, Prop::StrokeGradientAxis).is_none());
    }

    /// A shape with no ramp transports neither stops nor an axis — an unset
    /// gradient must not pin a layer to an axis it never asked for.
    #[test]
    fn no_ramp_transports_no_gradient_props() {
        let b = Shape::path(geometry()).stroke(Color::rgb(1, 2, 3)).bindings();
        assert!(!has_stops(&b, Prop::GradientStops));
        assert!(!has_stops(&b, Prop::StrokeGradientStops));
        assert!(axis_of(&b, Prop::GradientAxis).is_none());
        assert!(axis_of(&b, Prop::StrokeGradientAxis).is_none());
    }

    /// An axis the backend does not know resolves to the born-with variant
    /// rather than panicking: a newer widget against an older backend should
    /// draw the gradient the wrong way round, not fail to draw it.
    #[test]
    fn an_unknown_axis_falls_back_rather_than_failing() {
        assert_eq!(GradientAxis::from_i32(0), GradientAxis::Horizontal);
        assert_eq!(GradientAxis::from_i32(1), GradientAxis::Vertical);
        for junk in [-1, 2, 99, i32::MIN, i32::MAX] {
            assert_eq!(GradientAxis::from_i32(junk), GradientAxis::Vertical);
        }
    }
}
