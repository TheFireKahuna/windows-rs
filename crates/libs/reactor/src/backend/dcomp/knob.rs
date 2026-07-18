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
//! Two things move, on two DELIBERATELY separate springs:
//!
//! - the **bar** (arc + value thumb) is what the pointer manipulates, so it
//!   lands 1:1 where you put it. Its two geometries' `TrimEnd` are driven by ONE
//!   spring instance started on both, so the thumb can never trail the arc;
//! - the **needle** is a readout, not a handle, so it keeps its own
//!   `RotationAngle` spring ([`KnobParts::needle_spring`]) and SWEEPS over to a
//!   value the bar jumped straight to. It is not bound to `TrimEnd` by an
//!   expression — that would lock it to the bar and destroy exactly the
//!   click-to-position feel the separation buys. See [`KnobParts`].
//!
//! The track ring, ticks, labels, hub, and readout paint on the node surface
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
/// The value thumb's diameter (DIPs) — the grab handle riding the arc's end.
/// Wider than the arc so it reads as a control rather than a cap.
const THUMB_D: f32 = 15.0;
/// The thumb's trim window as a fraction of the sweep. A round-capped stroke
/// over a near-degenerate span renders as a filled circle (two semicircle caps
/// meeting); a small non-zero span keeps the segment from being culled.
const THUMB_EPS: f32 = 0.004;
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

/// Fraction of the track radius reserved for the centre readout hub. A press
/// inside it starts a relative drag WITHOUT jumping the value, so grabbing the
/// middle of the dial never throws the setting.
const HUB_FRAC: f32 = 0.55;

/// Map a pointer position (window DIPs) to a knob value — the inverse of
/// [`value_to_angle`]. The angle is unwrapped into the sweep; a point in the
/// gap below the dial clamps to whichever end is angularly nearer, so a press
/// near the bottom can never wrap min↔max.
///
/// Returns `None` inside the readout hub, where a press must not jump.
pub(crate) fn value_at_point(node: &Node, x: f32, y: f32) -> Option<f64> {
    let (cx, cy, radius) = dial_geom(node);
    let dx = x - node.rect.x - cx;
    let dy = y - node.rect.y - cy;
    if (dx * dx + dy * dy).sqrt() < radius * HUB_FRAC {
        return None;
    }
    let (start, end) = (node.ctrl.start_angle, node.ctrl.end_angle);
    // Canvas convention: 0 = east, clockwise — matching `value_to_angle`.
    let tau = std::f32::consts::TAU;
    let mut a = dy.atan2(dx);
    while a < start {
        a += tau;
    }
    while a >= start + tau {
        a -= tau;
    }
    let sweep = end - start;
    let t = if a <= end {
        (a - start) / sweep
    } else if (a - end) <= (start + tau - a) {
        1.0
    } else {
        0.0
    };
    Some(node.ctrl.min + f64::from(t.clamp(0.0, 1.0)) * (node.ctrl.max - node.ctrl.min))
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

    /// Returns the one cached geometry, IGNORING `factory`.
    ///
    /// D2D resources are factory-affine, so handing back a geometry built on a
    /// different factory than the caller asked for would be a real contract
    /// violation — this is sound only because there is exactly ONE D2D factory
    /// in the process, and the caller cannot be holding another one:
    ///
    /// - the geometry is tessellated by [`build_arc_path`] on `comp.gpu`, the
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
    /// `factory` — re-tessellating the arc on the factory it is handed (the
    /// path parameters are all that is needed) rather than returning a resource
    /// from a foreign one.
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

/// Build the thumb's geometry + thick round-capped stroke over `path`. The
/// trims are driven by expressions bound to the arc's `TrimEnd`
/// ([`KnobParts::bind_thumb`]); this only establishes the stroke.
fn build_thumb(
    c5: &ICompositor5,
    path: &CompositionPath,
    white: &CompositionBrush,
) -> Option<(
    CompositionPathGeometry,
    CompositionObject,
    ICompositionSpriteShape,
    CompositionShape,
)> {
    let geo = c5.CreatePathGeometryWithPath(path).ok()?;
    let obj: CompositionObject = geo.cast().ok()?;
    let ig = geo.cast::<ICompositionGeometry>().ok()?;
    ig.SetTrimStart(0.0).ok()?;
    ig.SetTrimEnd(0.0).ok()?;
    let shape_c = c5.CreateSpriteShapeWithGeometry(&geo).ok()?;
    let shape: ICompositionSpriteShape = shape_c.cast().ok()?;
    shape.SetStrokeThickness(THUMB_D).ok()?;
    shape.SetStrokeStartCap(CompositionStrokeCap::Round).ok()?;
    shape.SetStrokeEndCap(CompositionStrokeCap::Round).ok()?;
    shape.SetStrokeBrush(white).ok()?;
    Some((geo, obj, shape, shape_c.cast().ok()?))
}

// ── Retained knob parts ──────────────────────────────────────────────────────

/// The knob's retained compositor pieces. The arc is a `MaskBrush` (FP16 gradient
/// source × white-`TrimEnd`-shape mask).
///
/// The arc and the thumb are the BAR — the thing the pointer manipulates — so
/// they move as one: both geometries' `TrimEnd` are driven by the SAME spring
/// instance, with identical parameters and identical current values, so they
/// are in lockstep by construction and the circle can never trail the line.
///
/// The needle is a readout, not a handle, and runs on its OWN spring: a click
/// puts the bar exactly where you clicked while the needle sweeps over to it.
pub(crate) struct KnobParts {
    /// The white `TrimEnd` arc shape (off-tree) whose alpha the mask reads.
    mask_shape: ShapeVisual,
    geo: CompositionPathGeometry,
    geo_obj: CompositionObject,
    sprite_shape: ICompositionSpriteShape,
    /// The value thumb: a SECOND stroke over the same arc path, trimmed to a
    /// tiny window at the arc's end and stroked thick with round caps, so it
    /// renders as a filled circle there. It lives in the same mask shape as the
    /// arc, so the one gradient `MaskBrush` colours it with the arc's own colour
    /// at that point — the thumb continues the bar's gradient by construction.
    thumb_geo: CompositionPathGeometry,
    thumb_geo_obj: CompositionObject,
    thumb_shape: ICompositionSpriteShape,
    thumb_bound: bool,
    /// Drives BOTH `geo.TrimEnd` and `thumb_geo.TrimEnd` — one object started on
    /// two targets, so the arc and thumb settle identically.
    trim_spring: Option<SpringScalarNaturalMotionAnimation>,
    /// The needle's own retargetable spring on `RotationAngle`.
    needle_spring: Option<SpringScalarNaturalMotionAnimation>,
    /// Whether a spring may currently hold the bar's / the needle's property.
    /// `frac` and `angle` record the last target REQUESTED, so while one of
    /// these is set they say nothing about where the chrome actually is, and a
    /// snap must run even when the target is unchanged (see `Part::place`).
    trim_gliding: bool,
    needle_gliding: bool,
    /// The visible arc sprite (its brush is the `MaskBrush`), held as its
    /// `IVisual` — the same COM object, and the only face of it anything needs.
    display_vis: IVisual,
    mask_brush: CompositionMaskBrush,
    /// Live snapshot of `mask_shape` feeding the mask alpha.
    visual_surface: CompositionVisualSurface,
    needle: SpriteVisual,
    needle_vis: IVisual,
    grad_epoch: u32,
    /// The ramp the gradient source was last built for, stored EXACTLY.
    ///
    /// This was a truncated `FxHash` digest. A digest is smaller, but `FxHash`
    /// is a fast non-cryptographic mixer, not a collision-resistant one, and the
    /// failure it admits here is the silent kind: two different ramps hashing
    /// alike reads as "nothing changed", so the arc keeps rendering the previous
    /// colour ramp with nothing to signal it. Comparing the (handful of) stops
    /// themselves cannot be wrong, and only reallocates when the ramp really
    /// changes — a sync that finds them equal allocates nothing.
    stops_seen: Vec<(f64, crate::Color)>,
    geom: (f32, f32, f32),
    init: bool,
    frac: f32,
    /// Last needle angle written (radians), so an unchanged value costs nothing.
    angle: f32,
}

/// Exact stop-list comparison, on raw bits so a `NaN` position or channel
/// compares equal to itself (a `NaN` under `PartialEq` would report "changed"
/// forever and rebuild the gradient surface on every sync).
fn stops_eq(a: &[(f64, crate::Color)], b: &[(f64, crate::Color)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|((pa, ca), (pb, cb))| {
            pa.to_bits() == pb.to_bits()
                && ca.r.to_bits() == cb.r.to_bits()
                && ca.g.to_bits() == cb.g.to_bits()
                && ca.b.to_bits() == cb.b.to_bits()
                && ca.a.to_bits() == cb.a.to_bits()
        })
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
        let white_cb: CompositionBrush = white.cast().ok()?;
        sprite_shape.SetStrokeBrush(&white_cb).ok()?;

        // The thumb, in the SAME mask shape so the gradient brush colours it
        // with the arc's colour where it sits.
        let (thumb_geo, thumb_geo_obj, thumb_shape, thumb_shape_c) =
            build_thumb(&c5, &path, &white_cb)?;

        let mask_shape = c5.CreateShapeVisual().ok()?;
        mask_shape.cast::<IVisual>().ok()?.SetSize(Vector2::new(w, h)).ok()?;
        let shapes = mask_shape.Shapes().ok()?;
        shapes.Append(&sprite_shape_c.cast::<CompositionShape>().ok()?).ok()?;
        shapes.Append(&thumb_shape_c).ok()?;

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
            thumb_geo,
            thumb_geo_obj,
            thumb_shape,
            thumb_bound: false,
            trim_spring: None,
            needle_spring: None,
            trim_gliding: false,
            needle_gliding: false,
            display_vis,
            mask_brush,
            visual_surface,
            needle,
            needle_vis,
            grad_epoch: u32::MAX,
            stops_seen: Vec::new(),
            geom: (0.0, 0.0, 0.0),
            init: false,
            frac: -1.0,
            angle: f32::NAN,
        })
    }

    pub(crate) fn sync(
        &mut self,
        comp: &Compositing,
        node: &Node,
        atlas_epoch: u32,
        scale: f32,
        scrubbing: bool,
    ) {
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
                    // The thumb rides the same path — rebuild it too, then let
                    // both re-bind against the new geometry below.
                    if let Ok(tgeo) = c5.CreatePathGeometryWithPath(&path) {
                        let _ = self.thumb_shape.SetGeometry(&tgeo);
                        if let Ok(obj) = tgeo.cast::<CompositionObject>() {
                            self.thumb_geo_obj = obj;
                        }
                        self.thumb_geo = tgeo;
                    }
                    self.thumb_bound = false;
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
        if self.grad_epoch != atlas_epoch
            || !stops_eq(&self.stops_seen, &node.ctrl.stops)
            || resized
        {
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
            self.stops_seen.clear();
            self.stops_seen.extend_from_slice(&node.ctrl.stops);
        }

        if !self.thumb_bound {
            self.bind_thumb();
        }

        // ── The BAR (arc + thumb) ────────────────────────────────────────────
        // Direct manipulation lands 1:1: while this knob is the one under the
        // pointer, snap with plain property sets (stopping any in-flight
        // spring). That covers both a click-to-position jump and the drag that
        // may follow it — the bar goes exactly where you put it.
        //
        // It also tracks 1:1 while ANY drag is streaming updates: a
        // natural-motion spring restarted on every update never leaves rest, so
        // springing here left the arc pinned until the pointer stopped and only
        // then sprang to the final value.
        //
        // A discrete change — a preset, the wheel, an external set — eases in.
        let frac = ctrl_frac(node) as f32;
        let spring_bar = self.init && !resized && !node.pressed && !scrubbing;
        // Run when the target moved, and ALSO whenever a snap is wanted while a
        // spring may still hold the property — otherwise an interrupted glide
        // strands the bar and the unchanged target suppresses every later fix.
        if (self.frac - frac).abs() > f32::EPSILON || resized || (!spring_bar && self.trim_gliding) {
            if spring_bar {
                self.spring_trim(frac);
            } else {
                self.snap_trim(frac);
            }
            self.frac = frac;
        }

        // ── The NEEDLE ───────────────────────────────────────────────────────
        // A readout, not a handle, so it keeps its own motion: it SWEEPS to a
        // discrete change (including a click-to-position jump the bar took
        // instantly) on its own spring. It still tracks 1:1 during a live drag,
        // for the same restart-pinning reason as the bar.
        let angle = value_to_angle(node.ctrl.value, node.ctrl.min, node.ctrl.max, start, end);
        let spring_needle = self.angle.is_finite() && !resized && !scrubbing;
        if !self.angle.is_finite()
            || (self.angle - angle).abs() > f32::EPSILON
            || resized
            || (!spring_needle && self.needle_gliding)
        {
            if spring_needle {
                self.spring_needle(angle);
            } else {
                self.snap_needle(angle);
            }
            self.angle = angle;
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

    /// Bind the thumb's trim WINDOW: `TrimStart = Max(0, TrimEnd − ε)` against
    /// the thumb geometry's OWN `TrimEnd`. Referencing itself (rather than the
    /// arc) keeps the window a same-object read, so the circle's two edges can
    /// never disagree about where the end is; `TrimEnd` itself is driven by the
    /// arc's spring in [`spring_trim`] / [`snap_trim`].
    fn bind_thumb(&mut self) {
        let run = || -> Option<()> {
            let compositor = self.needle.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
            let expr: ExpressionAnimation = compositor
                .CreateExpressionAnimationWithExpression(&format!("Max(0.0, tg.TrimEnd - {THUMB_EPS})"))
                .ok()?;
            let ianim: ICompositionAnimation = expr.cast().ok()?;
            ianim.SetReferenceParameter("tg", &self.thumb_geo_obj).ok()?;
            self.thumb_geo_obj
                .StartAnimation("TrimStart", &expr.cast::<CompositionAnimation>().ok()?)
                .ok()
        };
        self.thumb_bound = run().is_some();
    }

    /// Ease the bar to `target`. ONE spring object is started on both the arc's
    /// and the thumb's `TrimEnd`: same parameters, same current value, so the
    /// two instances trace the same curve and the circle rides the line's end
    /// exactly rather than trailing it.
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
            let anim: CompositionAnimation = a.cast().ok()?;
            self.geo_obj.StartAnimation("TrimEnd", &anim).ok()?;
            self.thumb_geo_obj.StartAnimation("TrimEnd", &anim).ok()
        })()
        .is_some();
        if started {
            self.trim_gliding = true;
        } else {
            self.snap_trim(target);
        }
    }

    /// Put the bar at `target` NOW — stop any in-flight spring on both the arc
    /// and the thumb and write the value directly.
    fn snap_trim(&mut self, target: f32) {
        if self.trim_gliding {
            let _ = self.geo_obj.StopAnimation("TrimEnd");
            let _ = self.thumb_geo_obj.StopAnimation("TrimEnd");
            self.trim_gliding = false;
        }
        for g in [&self.geo, &self.thumb_geo] {
            if let Ok(ig) = g.cast::<ICompositionGeometry>() {
                let _ = ig.SetTrimEnd(target);
            }
        }
        self.init = true;
    }

    /// Sweep the needle to `angle` (radians) on its own spring.
    fn spring_needle(&mut self, angle: f32) {
        let started = (|| -> Option<()> {
            let needle_obj: ICompositionObject = self.needle.cast().ok()?;
            if self.needle_spring.is_none() {
                let compositor = needle_obj.Compositor().ok()?;
                let a = compositor.cast::<ICompositor4>().ok()?.CreateSpringScalarAnimation().ok()?;
                let sa: ISpringScalarNaturalMotionAnimation = a.cast().ok()?;
                sa.SetDampingRatio(KNOB_DAMPING).ok()?;
                sa.SetPeriod(ts_secs(KNOB_PERIOD)).ok()?;
                self.needle_spring = Some(a);
            }
            let a = self.needle_spring.as_ref()?;
            a.cast::<IScalarNaturalMotionAnimation>().ok()?.SetFinalValue(Some(angle)).ok()?;
            needle_obj
                .StartAnimation("RotationAngle", &a.cast::<CompositionAnimation>().ok()?)
                .ok()
        })()
        .is_some();
        if started {
            self.needle_gliding = true;
        } else {
            self.snap_needle(angle);
        }
    }

    /// Put the needle at `angle` NOW (stopping any in-flight sweep).
    fn snap_needle(&mut self, angle: f32) {
        if self.needle_gliding
            && let Ok(o) = self.needle.cast::<ICompositionObject>()
        {
            let _ = o.StopAnimation("RotationAngle");
            self.needle_gliding = false;
        }
        let _ = self.needle_vis.SetRotationAngle(angle);
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
pub(crate) fn sync_knob(
    comp: &Compositing,
    node: &mut Node,
    atlas_epoch: u32,
    scale: f32,
    scrubbing: bool,
) {
    if node.knob.is_none() {
        node.knob = KnobParts::new(comp, node).map(Box::new);
    }
    if let Some(mut kp) = node.knob.take() {
        kp.sync(comp, node, atlas_epoch, scale, scrubbing);
        node.knob = Some(kp);
    }
}
