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
//! [`super::shape`]): a sprite shape's only brush is a
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

use windows_canvas_core::{GpuDevice, PathBuilder, PathFigure, Vector2 as CVec2};
use windows_core::{implement_decl, Interface, Ref, Result};
use windows_numerics::Vector2;

use super::bootstrap::Compositing;
use super::node::Node;
use crate::system_bindings::{
    Color as UiColor, CompositionAnimation, CompositionBrush, CompositionMaskBrush,
    CompositionObject, CompositionPath, CompositionPathGeometry, CompositionShape,
    CompositionStrokeCap, CompositionSurfaceBrush, CompositionVisualSurface, ICompositionGeometry,
    ICompositionObject, ICompositionSpriteShape, ICompositionSurface, ICompositor2, ICompositor4,
    ICompositor5, ICompositorWithVisualSurface, ID2D1Factory, ID2D1Geometry, IGeometrySource2D,
    IGeometrySource2DInterop, IGeometrySource2DInterop_Impl, IGeometrySource2D_Impl,
    IScalarNaturalMotionAnimation, ISpringScalarNaturalMotionAnimation, IVisual, ShapeVisual,
    SpringScalarNaturalMotionAnimation, SpriteVisual, TimeSpan, Visual,
};
use crate::{PathData, PathGeometry, PathVerb};

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

/// A one-geometry [`IGeometrySource2D`] over an arbitrary D2D geometry.
///
/// The generalization of the knob's `ArcGeometrySource`; the factory-affinity
/// argument in [`super::knob`] applies here **verbatim and for the same
/// reasons** — one `GpuDevice` in the process, and the composition graphics
/// device is created from it, so the factory the compositor passes to
/// `TryGetGeometryUsingFactory` is the one this geometry already belongs to.
/// Read that comment before changing either.
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
    /// Returns the one cached geometry, IGNORING `factory` — sound only under
    /// the single-factory invariant documented on `knob::ArcGeometrySource`.
    fn TryGetGeometryUsingFactory(&self, _factory: Ref<ID2D1Factory>) -> Result<ID2D1Geometry> {
        Ok(self.geometry.clone())
    }
}

/// Replay a transported verb stream into a D2D geometry, then wrap it as a
/// composition path.
///
/// Returns `None` on a malformed stream (a segment verb with no open figure, or
/// points exhausted mid-verb) rather than a partial path — a half-drawn curve
/// reads as a rendering bug, an absent one as a data bug, and the second is the
/// truth. `ShapePath` makes both cases unconstructible; this is the backstop.
///
/// `filled` picks the figure begin mode, so a filled layer's open figures close
/// implicitly and a stroked layer's stay open.
fn build_composition_path(
    gpu: &GpuDevice,
    data: &PathData,
    filled: bool,
) -> Option<CompositionPath> {
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
    let path = builder.take()?.build().ok()?;
    let geometry: ID2D1Geometry = path.raw().cast().ok()?;
    let source: IGeometrySource2D = PathGeometrySource { geometry }.into();
    CompositionPath::Create(&source).ok()
}

// ── One fill-or-stroke layer ─────────────────────────────────────────────────

/// Which brush slot the white mask shape uses, and therefore how the geometry
/// paints. A layer is one or the other for its whole life — the app changing a
/// curve from filled to stroked rebuilds the layer rather than re-roling it.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Role {
    Fill,
    Stroke,
}

/// One retained layer: geometry → white mask shape → visual surface → mask
/// brush over an FP16 source → visible sprite.
struct PathLayer {
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
    size: Option<(f32, f32)>,
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
    fn new(comp: &Compositing, node: &Node, path: &CompositionPath, role: Role) -> Option<Self> {
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
    fn set_path(&mut self, path: &CompositionPath) -> Option<()> {
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

    fn resize(&mut self, w: f32, h: f32) {
        if self.size == Some((w, h)) {
            return;
        }
        let v = Vector2::new(w, h);
        // The three must agree or the mask samples the wrong region and the
        // curve clips against a stale extent.
        let _ = self.mask_vis.SetSize(v);
        let _ = self.visual_surface.SetSourceSize(v);
        let _ = self.display_vis.SetSize(v);
        self.size = Some((w, h));
    }

    fn set_thickness(&mut self, t: f32) {
        if self.role != Role::Stroke || self.thickness == Some(t) {
            return;
        }
        let _ = self.shape.SetStrokeThickness(t);
        self.thickness = Some(t);
    }

    /// Bind the FP16 colour source — a gradient when the app supplied stops,
    /// otherwise the flat colour. Rebuilds only on a real change or a display
    /// epoch bump, so a recolour is one call and a no-op sync is none.
    fn set_source(
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
        let src = if stops.is_empty() {
            super::parts::build_solid_surface(comp, color, scale)
        } else {
            super::parts::build_gradient_surface(comp, stops, scale)
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
    fn set_trim(&mut self, start: f32, end: f32) {
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
}

// ── The node's path parts ────────────────────────────────────────────────────

/// A path node's retained chrome: up to two layers, and the geometry they were
/// built for.
pub(crate) struct PathParts {
    fill: Option<PathLayer>,
    stroke: Option<PathLayer>,
    /// The geometry both layers currently hold. Compared by the transported
    /// value's own equality, which is pointer-first — an unchanged curve
    /// settles the whole sync in one compare.
    geometry_seen: Option<PathGeometry>,
}

impl PathParts {
    fn new() -> Self {
        Self {
            fill: None,
            stroke: None,
            geometry_seen: None,
        }
    }
}

/// Reconcile a path node's retained sprites against its current props.
///
/// Called from the paint walk in place of a surface draw — a path node has no
/// surface to draw. Everything here self-gates, so a sync that finds nothing
/// changed issues no COM calls at all.
pub(crate) fn sync_path(comp: &Compositing, node: &mut Node, atlas_epoch: u32, scale: f32) {
    let Some(geometry) = node.paint.path.clone() else {
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
    let want_stroke = node.paint.stroke.is_some() && node.paint.stroke_thickness > 0.0;

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
    parts.geometry_seen = Some(geometry);

    // Non-allocating: a node that never had control state reads `EMPTY_CTRL`.
    let stops = node.ctrl().stops.as_slice();
    if let Some(l) = &mut parts.fill {
        l.resize(w, h);
        l.set_source(comp, node.paint.fill.unwrap_or_default(), stops, atlas_epoch, scale);
        l.set_trim(node.paint.path_trim.0, node.paint.path_trim.1);
    }
    if let Some(l) = &mut parts.stroke {
        l.resize(w, h);
        l.set_thickness(node.paint.stroke_thickness);
        // A stroke takes the flat stroke colour; the stop list is the FILL's
        // ramp, so handing it here would recolour the outline with the area's
        // gradient and the two would stop reading as separate layers.
        l.set_source(comp, node.paint.stroke.unwrap_or_default(), &[], atlas_epoch, scale);
        l.set_trim(node.paint.path_trim.0, node.paint.path_trim.1);
    }
}
