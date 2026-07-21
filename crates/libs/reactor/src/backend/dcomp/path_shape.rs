//! Retained vector paths — arbitrary app geometry as compositor sprite shapes.
//!
//! The generalization of the knob's value arc ([`super::knob`]) from one
//! hardcoded arc to any geometry the app transports, and the mechanism the
//! canvas-drawn curves migrate onto. A path node owns **no drawing surface**:
//! its geometry is a [`CompositionPathGeometry`] the compositor holds, so a
//! node that moves, resizes its parent, scrolls or animates its trim costs no
//! repaint at all — DWM moves it.
//!
//! ## Colour stays FP16; the shape carries only alpha
//!
//! The rule the whole backend follows (see [`super::backdrop`] and
//! [`super::knob`]): a sprite shape's only brush is a
//! `CompositionColorBrush`, which is an 8-bit `Windows.UI.Color` and cannot
//! carry this palette's above-paper-white values. So the shape is drawn opaque
//! **white** and used as a MASK, an FP16 surface is the COLOUR, and a
//! [`CompositionMaskBrush`] combines them — solid or gradient, both built by
//! [`super::parts`] as display-mapped rasters.
//!
//! ## Why fill and stroke are separate layers
//!
//! One sprite shape takes a fill brush or a stroke brush, and one mask brush
//! carries one colour source. A curve generally wants BOTH — a gradient area
//! fill under a differently-coloured stroke — and the knob's trick of putting
//! two shapes in one mask shape works only because its arc and thumb are
//! deliberately the same colour. So fill and stroke are built as independent
//! layers, each with its own mask, source and sprite, and each is created only
//! if the app actually asked for it: a stroke-only curve pays for one.
//!
//! ## Glow is not here
//!
//! A compositor-side blur cannot be part of this: `CompositionMaskBrush`
//! [cannot be a `CompositionEffectBrush` source][mask-docs], and effect-graph
//! buffer precision is not reachable through `IGraphicsEffectD2D1Interop`
//! (Win2D applies `D2D1_PROPERTY_PRECISION` on its own effect object at
//! realization, where the compositor never sees it), so an effect glow would
//! be both structurally incompatible and precision-lossy. A glowing path takes
//! a pre-blurred FP16 source surface instead — the halo baked once per geometry
//! change, then composited like any other layer.
//!
//! [mask-docs]: https://learn.microsoft.com/en-us/uwp/api/windows.ui.composition.compositionmaskbrush

use windows_canvas_core::{
    ColorF, DrawingSession, GpuDevice, LayerRenderer, Matrix3x2, Path, PathBuilder, PathFigure,
    Vector2 as CVec2,
};
use windows_core::{implement_decl, Interface, Ref, Result};
use windows_numerics::Vector2;

use super::bootstrap::Compositing;
use super::node::{linear, Node};
use crate::system_bindings::{
    Color as UiColor, CompositionAnimation, CompositionBrush, CompositionDrawingSurface,
    CompositionMaskBrush, CompositionObject, CompositionPath, CompositionPathGeometry,
    CompositionShape, CompositionStrokeCap, CompositionSurfaceBrush, CompositionVisualSurface,
    ICompositionGeometry, ICompositionObject, ICompositionShape, ICompositionSpriteShape,
    ICompositionSurface,
    ICompositor2, ICompositor4, ICompositor5, ICompositorWithVisualSurface, ID2D1Factory,
    ID2D1Geometry, IGeometrySource2D, IGeometrySource2DInterop, IGeometrySource2DInterop_Impl,
    IGeometrySource2D_Impl, IScalarNaturalMotionAnimation, ISpringScalarNaturalMotionAnimation,
    IVisual, ShapeVisual, SpringScalarNaturalMotionAnimation, SpriteVisual, TimeSpan, Visual, POINT,
};
use crate::backend::ControlKind;
use crate::{Color, PathData, PathGeometry, PathVerb};

/// Spring tuning for a trim change. Matches the knob's arc so a curve drawing
/// itself on reads as the same motion system as every other value chrome.
const TRIM_DAMPING: f32 = 1.0;
const TRIM_PERIOD: f64 = 0.12;

fn ts_secs(s: f64) -> TimeSpan {
    TimeSpan {
        duration: (s * 10_000_000.0) as i64,
    }
}

// ── D2D geometry → composition path bridge ───────────────────────────────────

/// A one-geometry [`IGeometrySource2D`] over an arbitrary D2D geometry — the
/// backend's single bridge from a tessellated D2D path to a `CompositionPath`.
///
/// Every retained vector thing goes through here: the app's transported curves,
/// the knob's value arc, ticks and focus ring, and the progress ring's track and
/// arc. There was briefly a second, identical implementation living in
/// [`super::knob`]; the two differed in nothing but name.
struct PathGeometrySource {
    geometry: ID2D1Geometry,
}

implement_decl! {
    impl PathGeometrySource as PathGeometrySource_Impl: [IGeometrySource2D, IGeometrySource2DInterop]
}

impl IGeometrySource2D_Impl for PathGeometrySource_Impl {}

impl IGeometrySource2DInterop_Impl for PathGeometrySource_Impl {
    fn GetGeometry(&self) -> Result<ID2D1Geometry> {
        Ok(self.geometry.clone())
    }

    /// Returns the one cached geometry, IGNORING `factory`.
    ///
    /// D2D resources are factory-affine, so handing back a geometry built on a
    /// different factory than the caller asked for would be a real contract
    /// violation — this is sound only because there is exactly ONE D2D factory
    /// in the process, and the caller cannot be holding another one:
    ///
    /// - every geometry reaching this type is tessellated on `comp.gpu`, the
    ///   single [`GpuDevice`] owned by `Compositing`;
    /// - the only caller of this interface is the composition graphics device,
    ///   and `Compositing::new` creates that device *from* `comp.gpu` — see the
    ///   `CreateGraphicsDevice(gpu.d2d_device())` there. A D2D device's factory
    ///   is the factory it was created on, so the factory the compositor passes
    ///   here IS `comp.gpu`'s factory, the one the geometry already belongs to.
    ///
    /// Both facts hold by construction, not by luck, but neither is checkable
    /// from inside this method: `ID2D1Resource::GetFactory` is not scraped into
    /// `system_bindings`, so the two factories cannot be compared here.
    ///
    /// What would break it: a second `GpuDevice`/D2D factory anywhere in the
    /// backend, or a `CompositionGraphicsDevice` created from a device other
    /// than `comp.gpu`. Either change must also make this method honour
    /// `factory` — re-tessellating the geometry on the factory it is handed
    /// (the path parameters are all that is needed) rather than returning a
    /// resource from a foreign one.
    fn TryGetGeometryUsingFactory(&self, _factory: Ref<ID2D1Factory>) -> Result<ID2D1Geometry> {
        Ok(self.geometry.clone())
    }
}

/// Replay a transported verb stream into a D2D [`Path`].
///
/// Returns `None` on a malformed stream (a segment verb with no open figure, or
/// points exhausted mid-verb) rather than a partial path — a half-drawn curve
/// reads as a rendering bug, an absent one as a data bug, and the second is the
/// truth. `ShapePath` makes both cases unconstructible; this is the backstop.
///
/// `filled` picks the figure begin mode, so a filled layer's open figures close
/// implicitly and a stroked layer's stay open.
///
/// The `Path` is kept, not just its composition wrapper: the glow bake strokes
/// it into an off-screen bitmap ([`GlowLayer::bake`]), so both consumers replay
/// the verbs once.
///
/// Coordinates are replayed in the DIPs they were authored in. Getting them onto
/// the physical pixel grid is [`PathLayer::resize`]'s job, as one transform on
/// the shape.
fn build_d2d_path(gpu: &GpuDevice, data: &PathData, filled: bool) -> Option<Path> {
    let pts = data.points();
    let mut builder = Some(PathBuilder::new(gpu).ok()?);
    let mut figure: Option<PathFigure> = None;
    let mut i = 0usize;

    let read = |i: &mut usize, n: usize| -> Option<&[f32]> {
        let s = pts.get(*i..*i + n * 2)?;
        *i += n * 2;
        Some(s)
    };

    for &verb in data.verbs() {
        match verb {
            PathVerb::Move => {
                // A `Move` starts a new subpath; it does not close the previous
                // one, so an unterminated figure ends OPEN here.
                if let Some(f) = figure.take() {
                    builder = Some(f.end_open());
                }
                let p = read(&mut i, 1)?;
                let b = builder.take()?;
                let start = CVec2::new(p[0], p[1]);
                figure = Some(if filled { b.begin(start) } else { b.begin_hollow(start) });
            }
            PathVerb::Line => {
                let p = read(&mut i, 1)?;
                figure = Some(figure.take()?.line_to(CVec2::new(p[0], p[1])));
            }
            PathVerb::Cubic => {
                let p = read(&mut i, 3)?;
                figure = Some(figure.take()?.bezier_to(
                    CVec2::new(p[0], p[1]),
                    CVec2::new(p[2], p[3]),
                    CVec2::new(p[4], p[5]),
                ));
            }
            PathVerb::Close => builder = Some(figure.take()?.close()),
        }
    }
    // D2D requires every figure be ended before the sink closes.
    if let Some(f) = figure.take() {
        builder = Some(f.end_open());
    }
    builder.take()?.build().ok()
}

/// Wrap a D2D [`Path`] as a composition path via the one-geometry source.
pub(crate) fn to_composition_path(path: &Path) -> Option<CompositionPath> {
    let geometry: ID2D1Geometry = path.raw().cast().ok()?;
    let source: IGeometrySource2D = PathGeometrySource { geometry }.into();
    CompositionPath::Create(&source).ok()
}

/// How finely a circular arc is chorded. One value, because the two consumers
/// ([`super::knob`]'s value arc and [`super::ring_shape`]'s track and arc) sit
/// side by side on screen and a difference would read as one of them being
/// lower quality.
const ARC_SEGMENTS: u32 = 96;

/// Tessellate a circular arc centreline (`start → end`, radians) and wrap it as
/// a composition path.
///
/// Chords rather than Béziers: the result is stroked, never filled, and at 96
/// segments over a full turn the chord error is far below a physical pixel at
/// any dial or ring size the layout produces. It is tessellated ONCE per
/// geometry change — the value is expressed by trimming this path, not by
/// rebuilding it (see [`PathLayer::set_trim`]).
pub(crate) fn arc_path(
    gpu: &GpuDevice,
    cx: f32,
    cy: f32,
    radius: f32,
    start: f32,
    end: f32,
) -> Option<CompositionPath> {
    let mut fig = PathBuilder::new(gpu)
        .ok()?
        .begin_hollow(CVec2::new(cx + radius * start.cos(), cy + radius * start.sin()));
    for i in 1..=ARC_SEGMENTS {
        let a = start + (end - start) * (i as f32 / ARC_SEGMENTS as f32);
        fig = fig.line_to(CVec2::new(cx + radius * a.cos(), cy + radius * a.sin()));
    }
    to_composition_path(&fig.end_open().build().ok()?)
}

/// Replay the verbs and wrap the result as a composition path — the mask
/// layers' input. See [`build_d2d_path`].
fn build_composition_path(gpu: &GpuDevice, data: &PathData, filled: bool) -> Option<CompositionPath> {
    to_composition_path(&build_d2d_path(gpu, data, filled)?)
}

// ── One fill-or-stroke layer ─────────────────────────────────────────────────

/// Which brush slot the white mask shape uses, and therefore how the geometry
/// paints. A layer is one or the other for its whole life — the app changing a
/// curve from filled to stroked rebuilds the layer rather than re-roling it.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Role {
    Fill,
    Stroke,
}

/// One retained layer: geometry → white mask shape → visual surface → mask
/// brush over an FP16 source → visible sprite.
///
/// Named for the app-authored curve it was written to draw, but nothing in it
/// is curve-specific — it takes a `CompositionPath` and asks no questions. The
/// `ProgressRing` builds its track and value arc on it too ([`super::ring_shape`]),
/// which is why the type and the methods below are `pub(crate)` rather than
/// private to this module.
pub(crate) struct PathLayer {
    role: Role,
    geo: CompositionPathGeometry,
    /// The geometry as a `CompositionObject`, for springing `TrimEnd`.
    geo_obj: CompositionObject,
    shape: ICompositionSpriteShape,
    mask_shape: ShapeVisual,
    mask_vis: IVisual,
    visual_surface: CompositionVisualSurface,
    mask_brush: CompositionMaskBrush,
    display: SpriteVisual,
    display_vis: IVisual,
    /// The bound FP16 source and what it was built for — a solid colour's raw
    /// bits, or the exact stop list. Rebuilt only when one really changes.
    _source: Option<CompositionSurfaceBrush>,
    solid_for: Option<([u32; 4], u32)>,
    stops_seen: Vec<(f64, crate::Color)>,
    grad_epoch: u32,
    // ── change gates ──
    // Carries the DIP→px scale too, so a display move re-rasterizes the mask.
    size: Option<(f32, f32, f32)>,
    thickness: Option<f32>,
    trim: (f32, f32),
    trim_spring: Option<SpringScalarNaturalMotionAnimation>,
}

/// Quantize a colour to raw bits so the source gate is `Eq` without float
/// caveats (a `NaN` channel compares equal to itself and cannot loop).
fn color_bits(c: crate::Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

/// Exact stop-list comparison on raw bits, for the same reason
/// [`super::knob`]'s does it this way: a digest can collide silently, and the
/// failure mode is a curve that keeps rendering the previous ramp.
fn stops_eq(a: &[(f64, crate::Color)], b: &[(f64, crate::Color)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|((pa, ca), (pb, cb))| {
            pa.to_bits() == pb.to_bits() && color_bits(*ca) == color_bits(*cb)
        })
}

impl PathLayer {
    pub(crate) fn new(comp: &Compositing, node: &Node, path: &CompositionPath, role: Role) -> Option<Self> {
        let display = comp.new_sprite().ok()?;
        let display_vis: IVisual = display.cast().ok()?;
        let compositor = display.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
        let c5 = compositor.cast::<ICompositor5>().ok()?;
        let c2 = compositor.cast::<ICompositor2>().ok()?;
        let cvs = compositor.cast::<ICompositorWithVisualSurface>().ok()?;

        // ── The mask: an opaque-white shape over the app's geometry ──
        let geo = c5.CreatePathGeometryWithPath(path).ok()?;
        let geo_obj: CompositionObject = geo.cast().ok()?;
        let ig = geo.cast::<ICompositionGeometry>().ok()?;
        // Full extent by default; a caller that animates the draw-on retargets
        // `TrimEnd` from here.
        ig.SetTrimStart(0.0).ok()?;
        ig.SetTrimEnd(1.0).ok()?;
        let shape_c = c5.CreateSpriteShapeWithGeometry(&geo).ok()?;
        let shape: ICompositionSpriteShape = shape_c.cast().ok()?;
        let white = compositor
            .CreateColorBrushWithColor(UiColor { a: 255, r: 255, g: 255, b: 255 })
            .ok()?;
        let white_cb: CompositionBrush = white.cast().ok()?;
        match role {
            Role::Fill => shape.SetFillBrush(&white_cb).ok()?,
            Role::Stroke => {
                shape.SetStrokeBrush(&white_cb).ok()?;
                // Round caps and joins: these are sampled curves, and a mitre
                // spike on a steep spline segment is the classic artefact.
                shape.SetStrokeStartCap(CompositionStrokeCap::Round).ok()?;
                shape.SetStrokeEndCap(CompositionStrokeCap::Round).ok()?;
            }
        }

        let mask_shape = c5.CreateShapeVisual().ok()?;
        let mask_vis: IVisual = mask_shape.cast().ok()?;
        mask_shape
            .Shapes()
            .ok()?
            .Append(&shape_c.cast::<CompositionShape>().ok()?)
            .ok()?;

        // ── Live snapshot of the mask → a surface brush ──
        let visual_surface = cvs.CreateVisualSurface().ok()?;
        visual_surface.SetSourceVisual(&mask_shape.cast::<Visual>().ok()?).ok()?;
        visual_surface.SetSourceOffset(Vector2::new(0.0, 0.0)).ok()?;

        let mask_surf = compositor
            .CreateSurfaceBrushWithSurface(&visual_surface.cast::<ICompositionSurface>().ok()?)
            .ok()?;
        let mask_brush = c2.CreateMaskBrush().ok()?;
        mask_brush.SetMask(&mask_surf.cast::<CompositionBrush>().ok()?).ok()?;
        display.SetBrush(&mask_brush.cast::<CompositionBrush>().ok()?).ok()?;

        node.container
            .Children()
            .ok()?
            .InsertAtTop(&display.cast::<Visual>().ok()?)
            .ok()?;

        Some(Self {
            role,
            geo,
            geo_obj,
            shape,
            mask_shape,
            mask_vis,
            visual_surface,
            mask_brush,
            display,
            display_vis,
            _source: None,
            solid_for: None,
            stops_seen: Vec::new(),
            grad_epoch: u32::MAX,
            size: None,
            thickness: None,
            // Deliberately not (0.0, 1.0): the constructor already wrote that,
            // and recording it here would let a caller's first explicit
            // full-extent trim be skipped as "unchanged".
            trim: (f32::NAN, f32::NAN),
            trim_spring: None,
        })
    }

    /// Point every visual at a new geometry — the app's curve changed shape.
    ///
    /// The old `CompositionPathGeometry` is replaced rather than mutated: a
    /// composition path is immutable once created, so this is the only way, and
    /// it drops the trim spring because a spring bound to the retired object
    /// would keep animating something no longer on screen.
    pub(crate) fn set_path(&mut self, path: &CompositionPath) -> Option<()> {
        let c5 = self
            .display
            .cast::<ICompositionObject>()
            .and_then(|o| o.Compositor())
            .and_then(|c| c.cast::<ICompositor5>())
            .ok()?;
        let geo = c5.CreatePathGeometryWithPath(path).ok()?;
        self.shape.SetGeometry(&geo).ok()?;
        if let Ok(ig) = geo.cast::<ICompositionGeometry>() {
            let _ = ig.SetTrimStart(self.trim.0.max(0.0));
            let _ = ig.SetTrimEnd(if self.trim.1.is_nan() { 1.0 } else { self.trim.1 });
        }
        if let Ok(obj) = geo.cast::<CompositionObject>() {
            self.geo_obj = obj;
        }
        self.geo = geo;
        self.trim_spring = None;
        Some(())
    }

    pub(crate) fn resize(&mut self, w: f32, h: f32, scale: f32) {
        if self.size == Some((w, h, scale)) {
            return;
        }
        // The mask ShapeVisual is OFF-TREE (only this layer's VisualSurface
        // source), so it never inherits the root DIP→px scale the rest of the
        // tree rasterizes under (see bootstrap's one root `SetScale`). Left at
        // 1×, the stroke rasterizes at 1 px per DIP and the in-tree display
        // sprite — composited at `scale`× — upsamples that bitmap, softening
        // every line on a HiDPI display.
        //
        // The scale therefore goes on the SHAPE, not on the visual.
        //
        // A `CompositionVisualSurface` captures a region of its source visual's
        // CONTENT, and the source visual's own transform is not part of what it
        // captures — so `mask_vis.SetScale` cannot do this, and for a long time
        // the geometry silently rendered `scale`× too small: at 150% a 48-DIP
        // ellipse came out 46 px where it should be 70, and every app curve was
        // cut off at two thirds of its host box. A shape's transform IS content,
        // so it survives the capture.
        //
        // Putting it here rather than in the geometry is what keeps DIPs the
        // only space anyone authors in: a display change is one property set,
        // not a re-tessellation of every curve in the tree.
        let phys = Vector2::new(w * scale, h * scale);
        // The mask extent and the captured region must agree, or the mask
        // samples the wrong area and the curve clips against a stale extent.
        let _ = self.mask_vis.SetSize(phys);
        let _ = self.visual_surface.SetSourceSize(phys);
        if let Ok(shape) = self.shape.cast::<ICompositionShape>() {
            let _ = shape.SetScale(Vector2::new(scale, scale));
        }
        // The display sprite stays in DIPs — it IS under the root scale.
        let _ = self.display_vis.SetSize(Vector2::new(w, h));
        self.size = Some((w, h, scale));
    }

    pub(crate) fn set_thickness(&mut self, t: f32) {
        if self.role != Role::Stroke || self.thickness == Some(t) {
            return;
        }
        let _ = self.shape.SetStrokeThickness(t);
        self.thickness = Some(t);
    }

    /// Bind the FP16 colour source — a gradient when the app supplied stops,
    /// otherwise the flat colour. Rebuilds only on a real change or a display
    /// epoch bump, so a recolour is one call and a no-op sync is none.
    pub(crate) fn set_source(
        &mut self,
        comp: &Compositing,
        color: crate::Color,
        stops: &[(f64, crate::Color)],
        atlas_epoch: u32,
        scale: f32,
    ) {
        let want_solid = (color_bits(color), scale.to_bits());
        let unchanged = self.grad_epoch == atlas_epoch
            && if stops.is_empty() {
                self.solid_for == Some(want_solid) && self.stops_seen.is_empty()
            } else {
                stops_eq(&self.stops_seen, stops)
            };
        if unchanged {
            return;
        }
        // A stop list only ever reaches the FILL layer (the stroke passes `&[]`),
        // and a curve underfill fades DOWN the plot — so gradients here are
        // vertical. The flat branch serves the stroke's solid colour.
        let src = if stops.is_empty() {
            super::parts::build_solid_surface(comp, color, scale)
        } else {
            super::parts::build_vgradient_surface(comp, stops, scale)
        };
        let Some(s) = src else { return };
        let Ok(cb) = s.cast::<CompositionBrush>() else { return };
        if self.mask_brush.SetSource(&cb).is_ok() {
            self._source = Some(s);
            self.grad_epoch = atlas_epoch;
            if stops.is_empty() {
                self.solid_for = Some(want_solid);
                self.stops_seen.clear();
            } else {
                self.solid_for = None;
                self.stops_seen = stops.to_vec();
            }
        }
    }

    /// Retarget the draw-on trim. `TrimStart` snaps (it is a static crop);
    /// `TrimEnd` springs, so a curve revealing itself does so DWM-side with no
    /// app frame — the payoff the retained path exists for.
    pub(crate) fn set_trim(&mut self, start: f32, end: f32) {
        if self.trim == (start, end) {
            return;
        }
        if self.trim.0 != start
            && let Ok(ig) = self.geo.cast::<ICompositionGeometry>()
        {
            let _ = ig.SetTrimStart(start);
        }
        if self.trim.1 != end {
            let _ = self.spring_trim_end(end);
        }
        self.trim = (start, end);
    }

    fn spring_trim_end(&mut self, target: f32) -> Option<()> {
        let compositor = self
            .display
            .cast::<ICompositionObject>()
            .and_then(|o| o.Compositor())
            .ok()?;
        if self.trim_spring.is_none() {
            let a = compositor.cast::<ICompositor4>().ok()?.CreateSpringScalarAnimation().ok()?;
            let sa: ISpringScalarNaturalMotionAnimation = a.cast().ok()?;
            sa.SetDampingRatio(TRIM_DAMPING).ok()?;
            sa.SetPeriod(ts_secs(TRIM_PERIOD)).ok()?;
            self.trim_spring = Some(a);
        }
        let a = self.trim_spring.as_ref()?;
        a.cast::<IScalarNaturalMotionAnimation>().ok()?.SetFinalValue(Some(target)).ok()?;
        let anim: CompositionAnimation = a.cast().ok()?;
        self.geo_obj.StartAnimation("TrimEnd", &anim).ok()
    }

    /// Jump the trim to `end` with no motion, cancelling any spring in flight.
    ///
    /// [`Self::set_trim`] springs, which is right for a value that CHANGED and
    /// wrong for one that was REDEFINED — a progress ring flipping between its
    /// indeterminate sweep and its value arc is a mode change, and springing it
    /// reads as the ring unwinding. The `StopAnimation` is the load-bearing
    /// half: without it an in-flight spring keeps driving `TrimEnd` past the
    /// value just written, and the `self.trim` gate then suppresses every later
    /// correction, so the arc is stranded for good.
    pub(crate) fn snap_trim(&mut self, start: f32, end: f32) {
        let _ = self.geo_obj.StopAnimation("TrimEnd");
        if let Ok(ig) = self.geo.cast::<ICompositionGeometry>() {
            let _ = ig.SetTrimStart(start);
            let _ = ig.SetTrimEnd(end);
        }
        self.trim = (start, end);
    }

    pub(crate) fn set_opacity(&self, a: f32) {
        let _ = self.display_vis.SetOpacity(a);
    }

    /// The in-tree sprite, for a caller that animates the layer as a whole.
    ///
    /// Handed out rather than wrapping each transform property, because the one
    /// caller ([`super::ring_shape`]) drives `RotationAngle` on the composited
    /// layer — one cheap transform on an already-rasterized mask, as against
    /// rotating the off-tree mask visual and re-rasterizing its alpha every
    /// frame.
    pub(crate) fn display(&self) -> (&SpriteVisual, &IVisual) {
        (&self.display, &self.display_vis)
    }
}

// ── The glow layer: a pre-blurred FP16 halo ──────────────────────────────────

/// What a baked glow surface currently reflects — its whole rebuild gate.
#[derive(Clone, Copy, PartialEq, Eq)]
struct GlowBake {
    px: (i32, i32),
    color: [u32; 4],
    blur: u32,
    thickness: u32,
}

fn glow_bits(c: Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

/// A soft glow behind the stroke, as a single **pre-blurred FP16 surface**.
///
/// Unlike the mask layers this is not a mask-over-source: the surface already
/// carries the blurred, tinted halo — colour and alpha both — so it composites
/// as a plain surface brush. The blur cannot run on the compositor (see the
/// module header), so it is baked once per geometry / size / colour / thickness
/// change with the same `D2D1Shadow` primitive the painted glow used: an alpha
/// blur of the stroke, tinted with the glow colour.
///
/// It sits at the BOTTOM of the node's container, so both mask layers draw over
/// it regardless of creation order.
struct GlowLayer {
    display: SpriteVisual,
    display_vis: IVisual,
    _surface: Option<CompositionDrawingSurface>,
    _brush: Option<CompositionSurfaceBrush>,
    baked: Option<GlowBake>,
    size: Option<(f32, f32)>,
}

impl GlowLayer {
    fn new(comp: &Compositing, node: &Node) -> Option<Self> {
        let display = comp.new_sprite().ok()?;
        let display_vis: IVisual = display.cast().ok()?;
        node.container
            .Children()
            .ok()?
            .InsertAtBottom(&display.cast::<Visual>().ok()?)
            .ok()?;
        Some(Self {
            display,
            display_vis,
            _surface: None,
            _brush: None,
            baked: None,
            size: None,
        })
    }

    fn resize(&mut self, w: f32, h: f32) {
        if self.size != Some((w, h)) {
            let _ = self.display_vis.SetSize(Vector2::new(w, h));
            self.size = Some((w, h));
        }
    }

    /// Bake (or rebake) the halo. Self-gates on the geometry epoch plus every
    /// input the raster depends on, so a static curve pays once.
    #[allow(clippy::too_many_arguments)]
    fn sync(
        &mut self,
        comp: &Compositing,
        path: &Path,
        geometry_changed: bool,
        color: Color,
        blur: f32,
        thickness: f32,
        w: f32,
        h: f32,
        scale: f32,
    ) {
        self.resize(w, h);
        let px = (((w * scale).round() as i32).max(1), ((h * scale).round() as i32).max(1));
        let want = GlowBake {
            px,
            color: glow_bits(color),
            blur: blur.to_bits(),
            thickness: thickness.to_bits(),
        };
        if !geometry_changed && self.baked == Some(want) {
            return;
        }
        let Some((surface, brush)) = bake_glow(comp, path, px, color, blur, thickness, scale) else {
            return;
        };
        let Ok(cb) = brush.cast::<CompositionBrush>() else { return };
        if self.display.SetBrush(&cb).is_ok() {
            self._surface = Some(surface);
            self._brush = Some(brush);
            self.baked = Some(want);
        }
    }
}

/// Draw the stroke into an off-screen FP16 bitmap, blur + tint its alpha, and
/// composite that halo into a source surface — the retained equivalent of the
/// painted `effects::glow`, baked once instead of redrawn each frame.
#[allow(clippy::too_many_arguments)]
fn bake_glow(
    comp: &Compositing,
    path: &Path,
    px: (i32, i32),
    color: Color,
    blur: f32,
    thickness: f32,
    scale: f32,
) -> Option<(CompositionDrawingSurface, CompositionSurfaceBrush)> {
    let (surface, interop, brush) = comp.new_source_surface(px.0, px.1).ok()?;
    let mut origin = POINT::default();
    let ctx = match unsafe { interop.BeginDraw(None, &mut origin) } {
        Ok(c) => c,
        Err(e) => {
            comp.note_error(&e);
            return None;
        }
    };
    let session = DrawingSession::from_borrowed_context(
        &ctx,
        Matrix3x2::translation(origin.x as f32, origin.y as f32),
    );
    let ok = (|| -> Option<()> {
        session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
        // Render the sharp stroke on a SCRATCH context rather than by retargeting
        // this surface's own. Two bugs die with the retarget:
        //
        //  1. Direct2D must resolve the batched work against the current target
        //     before it can switch, so a `with_target` inside this `BeginDraw`
        //     published a half-drawn surface that the compositor could sample — a
        //     flickering shadow.
        //  2. This session carries the surface's ATLAS OFFSET, so the shape was
        //     drawn at `origin` INSIDE the bitmap and the composite below then
        //     applied `origin` a second time — a double offset that slid a baked
        //     shadow off position whenever the surface landed on a non-zero atlas
        //     slot. A scratch context has no atlas offset at all, so that is now
        //     unrepresentable rather than merely corrected.
        //
        // Only the stroke's alpha feeds the shadow, so WHITE keeps that alpha at
        // full strength whatever the eventual tint. The bitmap is the SURFACE's
        // size, not the shared atlas's (which is what `create_bitmap_target` would
        // have handed back).
        let renderer = LayerRenderer::new(&comp.gpu).ok()?;
        let shape = renderer
            .render((px.0.max(1) as u32, px.1.max(1) as u32), scale, true, |s| {
                if let Ok(white) = s.create_solid_brush(ColorF::new(1.0, 1.0, 1.0, 1.0)) {
                    s.draw_path(path, &white, thickness.max(0.5));
                }
            })
            .ok()?;
        // Canvas `shadowBlur` is 2σ (the value the mockups author), so σ = blur/2
        // — the same halving the painted glow applied. The tint rides the output
        // colour map like every solid.
        let halo = session.create_shadow(&shape, blur * 0.5, linear(color)).ok()?;
        // The shape bitmap already holds scaled pixels: composite it 1:1.
        session.set_transform(&Matrix3x2 {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: 0.0,
            m32: 0.0,
        });
        session.draw_effect(&halo);
        Some(())
    })();
    if let Err(e) = unsafe { interop.EndDraw() }.ok() {
        comp.note_error(&e);
        return None;
    }
    ok.map(|()| (surface, brush))
}

// ── The node's path parts ────────────────────────────────────────────────────

/// A path node's retained chrome: up to two mask layers, an optional glow, and
/// the geometry they were built for.
pub(crate) struct PathParts {
    fill: Option<PathLayer>,
    stroke: Option<PathLayer>,
    glow: Option<GlowLayer>,
    /// The geometry all layers currently hold. Compared by the transported
    /// value's own equality, which is pointer-first — an unchanged curve
    /// settles the whole sync in one compare.
    geometry_seen: Option<PathGeometry>,
}

impl PathParts {
    fn new() -> Self {
        Self {
            fill: None,
            stroke: None,
            glow: None,
            geometry_seen: None,
        }
    }
}

/// The magic constant for approximating a quarter circle with a cubic Bézier.
///
/// `4/3 · (√2 − 1)`. The classic value: it places the control points so the
/// curve's midpoint lands exactly on the arc, leaving a maximum radial error of
/// about 0.027% of the radius — far under a physical pixel at any size a layout
/// produces, and the reason an ellipse needs four segments rather than the 96
/// chords [`arc_path`] spends on a stroked dial.
const KAPPA: f64 = 0.552_284_749_83;

/// The geometry a shape node draws, derived from its box when it does not
/// transport one.
///
/// [`crate::ShapeKind`] already states this split — *"the other three kinds
/// derive their geometry from the node's box; this one is the only kind that
/// transports it"* — and this is where the deriving happens. It is what lets an
/// `Ellipse` and a `Line` be the `Path` they always were: one retained
/// implementation instead of three immediate-mode painters, and they gain trim,
/// gradient fill and glow by arriving through the same door.
///
/// A `Rectangle` is deliberately absent. It derives a box, and a box is the
/// nine-grid atlas's job (`parts::box_plan`), not a tessellation's.
fn derived_geometry(node: &Node) -> Option<PathGeometry> {
    let (w, h) = (node.rect.w as f64, node.rect.h as f64);
    match node.kind {
        ControlKind::Path => node.paint.path.clone(),
        // Four Béziers from the box's inscribed ellipse. Closed, so a filled
        // layer has an area and a stroked one has no seam at the start point.
        ControlKind::Ellipse => {
            let (rx, ry) = (w / 2.0, h / 2.0);
            if rx <= 0.0 || ry <= 0.0 {
                return None;
            }
            let (cx, cy) = (rx, ry);
            let (ox, oy) = (rx * KAPPA, ry * KAPPA);
            Some(
                crate::ShapePath::with_capacity(6)
                    .move_to(cx, cy - ry)
                    .cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy)
                    .cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry)
                    .cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy)
                    .cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry)
                    .close()
                    .build(),
            )
        }
        // Two points in the node's own space, exactly as the painted line read
        // them: `LineEndpoints` are node-local offsets, and the rect's origin is
        // the sprite's origin, so no translation is needed here.
        ControlKind::Line => {
            let l = node.paint.line;
            Some(
                crate::ShapePath::with_capacity(2)
                    .move_to(l.x1, l.y1)
                    .line_to(l.x2, l.y2)
                    .build(),
            )
        }
        _ => None,
    }
}

/// A shape node's stroke colour and width, with the `Line` defaults applied.
///
/// A `Line` is the one shape that draws with no styling at all: the painter it
/// replaces defaulted an unstyled line to the themable strong stroke at 1 DIP,
/// and the `Shape` widget emits neither prop unless the app sets it. Without
/// this the commonest divider in the library — `Shape::line()` with nothing on
/// it — would silently stop rendering.
///
/// Every other kind answers its props verbatim, so a curve with no stroke still
/// builds no stroke layer.
fn stroke_of(node: &Node) -> (Option<Color>, f32) {
    if node.kind == ControlKind::Line {
        let w = if node.paint.stroke_thickness > 0.0 { node.paint.stroke_thickness } else { 1.0 };
        return (Some(node.paint.stroke.unwrap_or_else(super::theme::stroke_strong)), w);
    }
    (node.paint.stroke, node.paint.stroke_thickness)
}

/// Reconcile a shape node's retained sprites against its current props.
///
/// Called from the sync walk in place of a surface draw — a shape node has no
/// surface to draw. Everything here self-gates, so a sync that finds nothing
/// changed issues no COM calls at all.
///
/// The derived kinds re-derive their geometry on every visit rather than
/// caching it, and that is deliberate: this runs only for a node the walk found
/// DIRTY, the two allocations are a handful of verbs and points, and
/// `geometry_seen` still compares the result by value — so a dirty ellipse whose
/// box did not change rebuilds no composition path and issues no COM call. A
/// cache here would gate the same work behind a second copy of the inputs.
pub(crate) fn sync_path(comp: &Compositing, node: &mut Node, atlas_epoch: u32, scale: f32) {
    let Some(geometry) = derived_geometry(node) else {
        // Geometry withdrawn: drop the layers so the sprites leave the tree.
        node.path = None;
        return;
    };
    if geometry.is_empty() {
        node.path = None;
        return;
    }

    // Detached for the duration so the layers can be mutated while `node` is
    // read for colour, thickness, trim and the gradient stops — disjoint
    // fields the borrow checker cannot see through accessors.
    let mut parts = node.path.take().unwrap_or_else(|| Box::new(PathParts::new()));
    sync_parts(comp, node, &mut parts, geometry, atlas_epoch, scale);
    node.path = Some(parts);
}

fn sync_parts(
    comp: &Compositing,
    node: &Node,
    parts: &mut PathParts,
    geometry: PathGeometry,
    atlas_epoch: u32,
    scale: f32,
) {
    let (w, h) = (node.rect.w, node.rect.h);
    let want_fill = node.paint.fill.is_some();
    let (stroke_color, stroke_w) = stroke_of(node);
    let want_stroke = stroke_color.is_some() && stroke_w > 0.0;

    // A layer whose role the app stopped asking for goes away entirely, rather
    // than lingering at zero alpha: an unasked-for layer should cost nothing.
    if !want_fill {
        parts.fill = None;
    }
    if !want_stroke {
        parts.stroke = None;
    }

    let geometry_changed = parts.geometry_seen.as_ref() != Some(&geometry);

    // The two roles need DIFFERENT D2D figure-begin modes, so each builds its
    // own composition path from the same transported verbs.
    for role in [Role::Fill, Role::Stroke] {
        let wanted = match role {
            Role::Fill => want_fill,
            Role::Stroke => want_stroke,
        };
        if !wanted {
            continue;
        }
        let slot_empty = match role {
            Role::Fill => parts.fill.is_none(),
            Role::Stroke => parts.stroke.is_none(),
        };
        if !slot_empty && !geometry_changed {
            continue;
        }
        let Some(path) =
            build_composition_path(&comp.gpu, geometry.data(), role == Role::Fill)
        else {
            continue;
        };
        match role {
            Role::Fill => match &mut parts.fill {
                Some(l) => {
                    let _ = l.set_path(&path);
                }
                slot => *slot = PathLayer::new(comp, node, &path, role),
            },
            Role::Stroke => match &mut parts.stroke {
                Some(l) => {
                    let _ = l.set_path(&path);
                }
                slot => *slot = PathLayer::new(comp, node, &path, role),
            },
        }
    }
    // ── Glow: a baked FP16 halo BELOW the stroke ──
    // Follows the stroke — a glow with no line to sit under reads as a smudge —
    // and needs the stroke's own hollow `Path` to raster, built once here.
    let want_glow = want_stroke && node.paint.path_glow.is_some();
    if !want_glow {
        parts.glow = None;
    } else {
        if parts.glow.is_none() {
            parts.glow = GlowLayer::new(comp, node);
        }
        if let (Some(g), Some((color, blur))) = (parts.glow.as_mut(), node.paint.path_glow)
            && let Some(path) = build_d2d_path(&comp.gpu, geometry.data(), false)
        {
            g.sync(
                comp,
                &path,
                geometry_changed,
                color,
                blur,
                stroke_w,
                w,
                h,
                scale,
            );
        }
    }

    parts.geometry_seen = Some(geometry);

    // Non-allocating: a node that never had control state reads `EMPTY_CTRL`.
    let stops = node.ctrl().stops.as_slice();
    if let Some(l) = &mut parts.fill {
        l.resize(w, h, scale);
        l.set_source(comp, node.paint.fill.unwrap_or_default(), stops, atlas_epoch, scale);
        l.set_trim(node.paint.path_trim.0, node.paint.path_trim.1);
    }
    if let Some(l) = &mut parts.stroke {
        l.resize(w, h, scale);
        // DIPs, like the geometry it strokes — the shape's own scale takes both
        // to physical pixels together.
        l.set_thickness(stroke_w);
        // A stroke takes the flat stroke colour; the stop list is the FILL's
        // ramp, so handing it here would recolour the outline with the area's
        // gradient and the two would stop reading as separate layers.
        l.set_source(comp, stroke_color.unwrap_or_default(), &[], atlas_epoch, scale);
        l.set_trim(node.paint.path_trim.0, node.paint.path_trim.1);
    }
}
