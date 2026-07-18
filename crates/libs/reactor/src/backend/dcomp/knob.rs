//! Knob value arc + needle — retained compositor chrome, fully FP16 (HDR-correct).
//!
//! The value arc grows on the compositor with zero app frames, and stays in the
//! display-mapped FP16 pipeline the way every other chrome colour does. Because a
//! `CompositionSurfaceBrush` does **not** paint as a shape *stroke* (only as a
//! sprite / mask brush), the arc is a [`CompositionMaskBrush`]:
//!
//! - **source** = the FP16 display-mapped gradient surface brush (the same raster
//!   the meter fill uses) — HDR-correct, above-paper-white clip colours intact;
//! - **mask**   = a white `TrimEnd` arc shape (a `CompositionSpriteShape`) captured
//!   live by a [`CompositionVisualSurface`]; its `TrimEnd` is grown by a scalar
//!   spring, so the reveal eases on the compositor.
//!
//! The needle's `RotationAngle` is bound to that same `TrimEnd` by one
//! `ExpressionAnimation`, so a single spring drives both — provably locked. The
//! track ring, ticks, labels, hub, and readout paint on the node surface
//! (dirty-only) — see `controls::paint_knob`.
//!
//! The D2D arc geometry reaches the compositor through a small
//! [`IGeometrySource2DInterop`] implementation (`ArcGeometrySource`).

use windows_canvas_core::{GpuDevice, PathBuilder, Vector2 as CVec2};
use windows_core::{implement_decl, Interface, Ref, Result};
use windows_numerics::{Vector2, Vector3};

use super::bootstrap::Compositing;
use super::node::Node;
use super::theme;
use crate::system_bindings::{
    Color as UiColor, CompositionAnimation, CompositionBrush, CompositionMaskBrush,
    CompositionObject, CompositionPath, CompositionPathGeometry, CompositionShape,
    CompositionStrokeCap, CompositionVisualSurface, ExpressionAnimation, ICompositionAnimation,
    ICompositionGeometry, ICompositionObject, ICompositionSpriteShape, ICompositionSurface,
    ICompositor2, ICompositor4, ICompositor5,
    ICompositorWithVisualSurface, IGeometrySource2D, IGeometrySource2DInterop,
    IGeometrySource2D_Impl, IGeometrySource2DInterop_Impl, IScalarNaturalMotionAnimation,
    ISpringScalarNaturalMotionAnimation, IVisual, ID2D1Factory, ID2D1Geometry, ShapeVisual,
    SpringScalarNaturalMotionAnimation, SpriteVisual, TimeSpan, Visual,
};

/// SOFT chase (the atom default `k = 260, c = 26`): natural period `2π/√k`,
/// damping ratio `c / (2√k)`.
const KNOB_PERIOD: f32 = 0.390;
const KNOB_DAMPING: f32 = 0.806;

/// Arc stroke thickness (DIPs) and needle thickness — matching the atom's
/// `draw_knob` (8-DIP arc, 2-DIP needle).
const ARC_WIDTH: f32 = 8.0;
const NEEDLE_W: f32 = 2.0;
/// Needle length as a fraction of the track radius (atom `radius * 0.7`).
const NEEDLE_FRAC: f32 = 0.7;
/// Arc tessellation chord count over the full sweep.
const ARC_SEGMENTS: u32 = 96;

fn ts_secs(s: f32) -> TimeSpan {
    TimeSpan { duration: (s.max(0.001) * 1.0e7) as i64 }
}

/// Radial gap between the track and the outer numeric labels (DIPs).
pub(crate) const LABEL_OFFSET: f32 = 14.0;

/// The dial geometry (center + label-fitted track radius) derived from the node
/// — the single source both `controls::paint_knob` and the arc geometry use, so
/// the painted ring/labels and the compositor arc/needle share one radius.
pub(crate) fn dial_geom(node: &Node) -> (f32, f32, f32) {
    let (w, h) = (node.rect.w, node.rect.h);
    let (cx, cy) = (w * 0.5, h * 0.56);
    let mut radius = (w * 0.42).min(h * 0.44);
    if !node.ctrl.tick_labels.is_empty() {
        let max_cos = node
            .ctrl
            .tick_labels
            .iter()
            .map(|(v, _)| {
                value_to_angle(*v, node.ctrl.min, node.ctrl.max, node.ctrl.start_angle, node.ctrl.end_angle)
                    .cos()
                    .abs()
            })
            .fold(0.0f32, f32::max);
        if max_cos >= 1e-3 {
            let pad = (radius * 0.1).max(10.0) * 1.1 + 2.0;
            let allowed = ((cx - pad) / max_cos - LABEL_OFFSET).max(10.0);
            radius = radius.min(allowed);
        }
    }
    (cx, cy, radius)
}

/// Map a value to its needle/sweep angle (radians), clamped to `[min, max]`.
pub(crate) fn value_to_angle(value: f64, min: f64, max: f64, start: f32, end: f32) -> f32 {
    let span = max - min;
    let t = if span == 0.0 { 0.0 } else { ((value - min) / span).clamp(0.0, 1.0) };
    start + (end - start) * t as f32
}

// ── D2D arc geometry → composition path bridge ───────────────────────────────

/// A one-geometry [`IGeometrySource2D`]: hands the compositor the D2D arc
/// geometry the mask arc was tessellated into.
struct ArcGeometrySource {
    geometry: ID2D1Geometry,
}

implement_decl! {
    impl ArcGeometrySource as ArcGeometrySource_Impl: [IGeometrySource2D, IGeometrySource2DInterop]
}

impl IGeometrySource2D_Impl for ArcGeometrySource_Impl {}

impl IGeometrySource2DInterop_Impl for ArcGeometrySource_Impl {
    fn GetGeometry(&self) -> Result<ID2D1Geometry> {
        Ok(self.geometry.clone())
    }
    fn TryGetGeometryUsingFactory(&self, _factory: Ref<ID2D1Factory>) -> Result<ID2D1Geometry> {
        Ok(self.geometry.clone())
    }
}

/// Tessellate the full value-arc centerline (`start → end`) and wrap it as a
/// composition path.
fn build_arc_path(
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
    let path = fig.end_open().build().ok()?;
    let geometry: ID2D1Geometry = path.raw().cast().ok()?;
    let source: IGeometrySource2D = ArcGeometrySource { geometry }.into();
    CompositionPath::Create(&source).ok()
}

// ── Retained knob parts ──────────────────────────────────────────────────────

/// The knob's retained compositor pieces. The arc is a `MaskBrush` (FP16 gradient
/// source × white-`TrimEnd`-shape mask); the needle rides the same `TrimEnd`.
pub(crate) struct KnobParts {
    /// The white `TrimEnd` arc shape (off-tree) whose alpha the mask reads.
    mask_shape: ShapeVisual,
    geo: CompositionPathGeometry,
    geo_obj: CompositionObject,
    sprite_shape: ICompositionSpriteShape,
    trim_spring: Option<SpringScalarNaturalMotionAnimation>,
    /// The visible arc sprite (its brush is the `MaskBrush`).
    display: SpriteVisual,
    display_vis: IVisual,
    mask_brush: CompositionMaskBrush,
    /// Live snapshot of `mask_shape` feeding the mask alpha.
    visual_surface: CompositionVisualSurface,
    needle: SpriteVisual,
    needle_vis: IVisual,
    grad_epoch: u32,
    stops_sig: u64,
    geom: (f32, f32, f32),
    init: bool,
    frac: f32,
    needle_bound: bool,
}

fn stops_signature(stops: &[(f64, crate::Color)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    for (p, c) in stops {
        p.to_bits().hash(&mut h);
        [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()].hash(&mut h);
    }
    h.finish()
}

impl KnobParts {
    fn new(comp: &Compositing, node: &Node) -> Option<Self> {
        let (w, h) = (node.rect.w, node.rect.h);
        let (cx, cy, radius) = dial_geom(node);
        let path = build_arc_path(&comp.gpu, cx, cy, radius, node.ctrl.start_angle, node.ctrl.end_angle)?;

        let needle = comp.new_sprite().ok()?;
        let needle_vis: IVisual = needle.cast().ok()?;
        let compositor = needle.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
        let c5 = compositor.cast::<ICompositor5>().ok()?;
        let c2 = compositor.cast::<ICompositor2>().ok()?;
        let cvs = compositor.cast::<ICompositorWithVisualSurface>().ok()?;

        // ── Mask: a white TrimEnd arc shape in an off-tree ShapeVisual ──
        let geo = c5.CreatePathGeometryWithPath(&path).ok()?;
        let geo_obj: CompositionObject = geo.cast().ok()?;
        let ig = geo.cast::<ICompositionGeometry>().ok()?;
        ig.SetTrimStart(0.0).ok()?;
        ig.SetTrimEnd(0.0).ok()?;
        let sprite_shape_c = c5.CreateSpriteShapeWithGeometry(&geo).ok()?;
        let sprite_shape: ICompositionSpriteShape = sprite_shape_c.cast().ok()?;
        sprite_shape.SetStrokeThickness(ARC_WIDTH).ok()?;
        sprite_shape.SetStrokeStartCap(CompositionStrokeCap::Round).ok()?;
        sprite_shape.SetStrokeEndCap(CompositionStrokeCap::Round).ok()?;
        // Opaque white — the mask reads only the alpha channel.
        let white = compositor
            .CreateColorBrushWithColor(UiColor { a: 255, r: 255, g: 255, b: 255 })
            .ok()?;
        sprite_shape.SetStrokeBrush(&white.cast::<CompositionBrush>().ok()?).ok()?;
        let mask_shape = c5.CreateShapeVisual().ok()?;
        mask_shape.cast::<IVisual>().ok()?.SetSize(Vector2::new(w, h)).ok()?;
        mask_shape.Shapes().ok()?.Append(&sprite_shape_c.cast::<CompositionShape>().ok()?).ok()?;

        // ── Live snapshot of the mask shape → a surface brush ──
        let visual_surface = cvs.CreateVisualSurface().ok()?;
        visual_surface.SetSourceVisual(&mask_shape.cast::<Visual>().ok()?).ok()?;
        visual_surface.SetSourceOffset(Vector2::new(0.0, 0.0)).ok()?;
        visual_surface.SetSourceSize(Vector2::new(w, h)).ok()?;
        let mask_surf = compositor
            .CreateSurfaceBrushWithSurface(&visual_surface.cast::<ICompositionSurface>().ok()?)
            .ok()?;

        // ── Mask brush: FP16 gradient source × mask alpha (source set in sync) ──
        let mask_brush = c2.CreateMaskBrush().ok()?;
        mask_brush.SetMask(&mask_surf.cast::<CompositionBrush>().ok()?).ok()?;

        // ── The visible arc sprite ──
        let display = comp.new_sprite().ok()?;
        let display_vis: IVisual = display.cast().ok()?;
        display_vis.SetSize(Vector2::new(w, h)).ok()?;
        display.SetBrush(&mask_brush.cast::<CompositionBrush>().ok()?).ok()?;

        let children = node.container.Children().ok()?;
        children.InsertAtTop(&display.cast::<Visual>().ok()?).ok()?;
        children.InsertAtTop(&needle.cast::<Visual>().ok()?).ok()?;

        Some(Self {
            mask_shape,
            geo,
            geo_obj,
            sprite_shape,
            trim_spring: None,
            display,
            display_vis,
            mask_brush,
            visual_surface,
            needle,
            needle_vis,
            grad_epoch: u32::MAX,
            stops_sig: 0,
            geom: (0.0, 0.0, 0.0),
            init: false,
            frac: -1.0,
            needle_bound: false,
        })
    }

    pub(crate) fn sync(&mut self, comp: &Compositing, node: &Node, atlas_epoch: u32, scale: f32) {
        let (w, h) = (node.rect.w, node.rect.h);
        let (cx, cy, radius) = dial_geom(node);
        let start = node.ctrl.start_angle;
        let end = node.ctrl.end_angle;

        let resized = self.geom != (cx, cy, radius);
        if resized {
            if let Some(path) = build_arc_path(&comp.gpu, cx, cy, radius, start, end) {
                if let Ok(c5) = self
                    .needle
                    .cast::<ICompositionObject>()
                    .and_then(|o| o.Compositor())
                    .and_then(|c| c.cast::<ICompositor5>())
                    && let Ok(geo) = c5.CreatePathGeometryWithPath(&path)
                {
                    let _ = self.sprite_shape.SetGeometry(&geo);
                    if let Ok(ig) = geo.cast::<ICompositionGeometry>() {
                        let _ = ig.SetTrimStart(0.0);
                    }
                    if let Ok(obj) = geo.cast::<CompositionObject>() {
                        self.geo_obj = obj;
                    }
                    self.geo = geo;
                    self.trim_spring = None;
                    self.needle_bound = false;
                }
            }
            let _ = self.mask_shape.cast::<IVisual>().map(|v| v.SetSize(Vector2::new(w, h)));
            let _ = self.display_vis.SetSize(Vector2::new(w, h));
            let _ = self.visual_surface.SetSourceSize(Vector2::new(w, h));
            self.place_needle(cx, cy, radius);
            self.geom = (cx, cy, radius);
        }

        // FP16 gradient SOURCE (display-mapped) + needle colour: rebind on a
        // display epoch or a stops-list change.
        let sig = stops_signature(&node.ctrl.stops);
        if self.grad_epoch != atlas_epoch || self.stops_sig != sig || resized {
            let src = if node.ctrl.stops.is_empty() {
                super::parts::build_solid_surface(comp, node.ctrl.accent.unwrap_or_else(theme::accent), scale)
            } else {
                super::parts::build_gradient_surface(comp, &node.ctrl.stops, scale)
            };
            if let Some(s) = src
                && let Ok(cb) = s.cast::<CompositionBrush>()
            {
                let _ = self.mask_brush.SetSource(&cb);
            }
            if let Some(nb) = super::parts::build_solid_surface(comp, theme::w(1.0), scale)
                && let Ok(cb) = nb.cast::<CompositionBrush>()
            {
                let _ = self.needle.SetBrush(&cb);
            }
            self.grad_epoch = atlas_epoch;
            self.stops_sig = sig;
        }

        if !self.needle_bound {
            self.bind_needle(start, end);
        }

        let frac = ctrl_frac(node) as f32;
        if (self.frac - frac).abs() > f32::EPSILON || resized {
            if self.init && !resized {
                self.spring_trim(frac);
            } else {
                if let Ok(ig) = self.geo.cast::<ICompositionGeometry>() {
                    let _ = ig.SetTrimEnd(frac);
                }
                self.init = true;
            }
            self.frac = frac;
        }

        let dim = if node.paint.is_enabled { 1.0 } else { theme::disabled_opacity() };
        let _ = self.display_vis.SetOpacity(dim);
        let _ = self.needle_vis.SetOpacity(dim);
    }

    fn place_needle(&self, cx: f32, cy: f32, radius: f32) {
        let len = (radius * NEEDLE_FRAC).max(1.0);
        let _ = self.needle_vis.SetSize(Vector2::new(len, NEEDLE_W));
        let _ = self.needle_vis.SetOffset(Vector3::new(cx, cy - NEEDLE_W / 2.0, 0.0));
        let _ = self.needle_vis.SetCenterPoint(Vector3::new(0.0, NEEDLE_W / 2.0, 0.0));
    }

    /// Bind the needle's `RotationAngle` to the arc's animated `TrimEnd`:
    /// `angle = start + TrimEnd·(end − start)` — one spring drives both.
    fn bind_needle(&mut self, start: f32, end: f32) {
        let run = || -> Option<()> {
            let compositor = self.needle.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
            let expr: ExpressionAnimation = compositor
                .CreateExpressionAnimationWithExpression(&format!(
                    "{start} + geo.TrimEnd * {sweep}",
                    sweep = end - start
                ))
                .ok()?;
            let ianim: ICompositionAnimation = expr.cast().ok()?;
            ianim.SetReferenceParameter("geo", &self.geo_obj).ok()?;
            let needle_obj: ICompositionObject = self.needle.cast().ok()?;
            needle_obj
                .StartAnimation("RotationAngle", &expr.cast::<CompositionAnimation>().ok()?)
                .ok()
        };
        self.needle_bound = run().is_some();
    }

    fn spring_trim(&mut self, target: f32) {
        let started = (|| -> Option<()> {
            if self.trim_spring.is_none() {
                let compositor = self.needle.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
                let a = compositor.cast::<ICompositor4>().ok()?.CreateSpringScalarAnimation().ok()?;
                let sa: ISpringScalarNaturalMotionAnimation = a.cast().ok()?;
                sa.SetDampingRatio(KNOB_DAMPING).ok()?;
                sa.SetPeriod(ts_secs(KNOB_PERIOD)).ok()?;
                self.trim_spring = Some(a);
            }
            let a = self.trim_spring.as_ref()?;
            a.cast::<IScalarNaturalMotionAnimation>().ok()?.SetFinalValue(Some(target)).ok()?;
            self.geo_obj
                .StartAnimation("TrimEnd", &a.cast::<CompositionAnimation>().ok()?)
                .ok()
        })()
        .is_some();
        if !started
            && let Ok(ig) = self.geo.cast::<ICompositionGeometry>()
        {
            let _ = ig.SetTrimEnd(target);
        }
    }

    /// Detach the visible arc + needle from the node container (node teardown).
    /// The mask shape is off-tree already (only referenced by the visual surface).
    pub(crate) fn detach(&self, node: &Node) {
        if let Ok(children) = node.container.Children() {
            if let Ok(v) = self.display.cast::<Visual>() {
                let _ = children.Remove(&v);
            }
            if let Ok(v) = self.needle.cast::<Visual>() {
                let _ = children.Remove(&v);
            }
        }
    }
}

/// The node's value fraction (0..1) over `[min, max]`.
fn ctrl_frac(node: &Node) -> f64 {
    let span = node.ctrl.max - node.ctrl.min;
    if span.abs() < f64::EPSILON {
        0.0
    } else {
        ((node.ctrl.value - node.ctrl.min) / span).clamp(0.0, 1.0)
    }
}

/// Ensure the node has its knob parts and reconcile them (the paint-pass entry).
pub(crate) fn sync_knob(comp: &Compositing, node: &mut Node, atlas_epoch: u32, scale: f32) {
    if node.knob.is_none() {
        node.knob = KnobParts::new(comp, node).map(Box::new);
    }
    if let Some(mut kp) = node.knob.take() {
        kp.sync(comp, node, atlas_epoch, scale);
        node.knob = Some(kp);
    }
}
