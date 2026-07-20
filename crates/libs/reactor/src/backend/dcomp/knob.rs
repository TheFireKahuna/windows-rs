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

use windows_canvas_core::{GpuDevice, PathBuilder, Rect, Vector2 as CVec2};
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

/// The background groove, wider than the value arc it sits under.
const RING_WIDTH: f32 = 10.0;
const TICK_MAJOR_LEN: f32 = 8.0;
const TICK_MINOR_LEN: f32 = 5.0;
/// How far inside the arc radius the ticks' outer ends stop.
const TICK_INSET: f32 = 4.0;
const TICK_MAJOR_W: f32 = 1.5;
const TICK_MINOR_W: f32 = 1.0;
/// Centre hub DIAMETER (the painter drew radius 4.0).
const HUB_D: f32 = 8.0;

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
    if !node.ctrl().tick_labels.is_empty() {
        let max_cos = node
            .ctrl()
            .tick_labels
            .iter()
            .map(|(v, _)| {
                value_to_angle(*v, node.ctrl().min, node.ctrl().max, node.ctrl().start_angle, node.ctrl().end_angle)
                    .cos()
                    .abs()
            })
            .fold(0.0f32, f32::max);
        if max_cos >= 1e-3 {
            let pad = tick_em(radius) * 1.1 + 2.0;
            let allowed = ((cx - pad) / max_cos - LABEL_OFFSET).max(10.0);
            radius = radius.min(allowed);
        }
    }
    (cx, cy, radius)
}

// ── The dial's four type sizes, and the boxes its runs sit in ────────────────
//
// Every size here is derived from the RADIUS, which is why the knob is the one
// converted kind that shapes at placement time rather than in the layout pass:
// the radius is not known until the solve has run. They are named rather than
// written out at each use because the dial FITS its radius to the room the tick
// labels need (`dial_geom` above) — so the size the fit assumes and the size the
// run is shaped at have to be one number, and they were previously two.

/// The em an outer tick label is set at.
pub(crate) fn tick_em(radius: f32) -> f32 {
    (radius * 0.1).max(10.0)
}

/// The em the centre readout is set at.
pub(crate) fn readout_em(radius: f32) -> f32 {
    (radius * 0.38).max(20.0)
}

/// The em the unit under the readout is set at — a fraction of the readout's
/// own, not of the radius, so the pair scales together.
pub(crate) fn unit_em(readout_em: f32) -> f32 {
    readout_em * 0.35
}

/// The em the sub-line under the unit is set at.
pub(crate) fn sub_em(radius: f32) -> f32 {
    (radius * 0.1).max(8.0)
}

/// Where the three stacked centre runs hang from.
fn base_y(cy: f32, radius: f32) -> f32 {
    cy + radius * 0.35
}

/// The box a tick label centres in, for a tick at `angle`.
pub(crate) fn tick_label_box(cx: f32, cy: f32, radius: f32, angle: f32) -> Rect {
    let em = tick_em(radius);
    let lr = radius + LABEL_OFFSET;
    let (lx, ly) = (cx + angle.cos() * lr, cy + angle.sin() * lr);
    Rect::from_xywh(lx - lr, ly - em, 2.0 * lr, 2.0 * em)
}

/// The centre readout's box — centred on it both ways.
pub(crate) fn readout_box(cx: f32, cy: f32, radius: f32) -> Rect {
    let em = readout_em(radius);
    Rect::from_xywh(cx - radius, base_y(cy, radius) - em, 2.0 * radius, 2.0 * em)
}

/// The unit's box. The run hangs from its TOP, not its middle — it is stacked
/// under the readout, not centred in a band of its own.
pub(crate) fn unit_box(cx: f32, cy: f32, radius: f32) -> Rect {
    let em = readout_em(radius);
    let y = base_y(cy, radius) + em * 0.45;
    Rect::from_xywh(cx - radius, y, 2.0 * radius, em)
}

/// The sub-line's box, hanging from its top for the same reason.
pub(crate) fn sub_box(cx: f32, cy: f32, radius: f32) -> Rect {
    let y = base_y(cy, radius) + readout_em(radius) * 0.75;
    Rect::from_xywh(cx - radius, y, 2.0 * radius, 2.0 * sub_em(radius))
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
    let (start, end) = (node.ctrl().start_angle, node.ctrl().end_angle);
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
    Some(node.ctrl().min + f64::from(t.clamp(0.0, 1.0)) * (node.ctrl().max - node.ctrl().min))
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

/// Tessellate one tick CLASS as a single geometry of N two-point figures.
///
/// One geometry rather than one per tick because a tick is two points and a
/// sprite visual is not: N ticks as N sprites would be N visuals to place and
/// rotate, and a rotated axis-aligned sprite resamples where a stroked path
/// rasterizes at the angle it is drawn. Two geometries rather than one because
/// a sprite shape carries one `StrokeThickness` and a mask layer carries one
/// colour, and the two classes differ in both.
///
/// `None` when the class is empty — a dial with no `major_every` builds only
/// the minor layer, and a dial with no ticks builds neither.
fn build_tick_path(
    gpu: &GpuDevice,
    cx: f32,
    cy: f32,
    radius: f32,
    node: &Node,
    major: bool,
) -> Option<CompositionPath> {
    let ctrl = node.ctrl();
    let (min, max) = (ctrl.min, ctrl.max);
    let (start, end) = (ctrl.start_angle, ctrl.end_angle);
    let is_major = |tv: f64| {
        ctrl.major_every
            .filter(|m| *m != 0.0)
            .is_some_and(|m| (tv % m).abs() < 1e-9)
    };

    let len = if major { TICK_MAJOR_LEN } else { TICK_MINOR_LEN };
    let inner = radius - len - TICK_INSET;
    let outer = radius - TICK_INSET;

    let mut b = PathBuilder::new(gpu).ok()?;
    let mut any = false;
    for &tv in ctrl.ticks.iter().filter(|&&tv| is_major(tv) == major) {
        let a = value_to_angle(tv, min, max, start, end);
        let (ca, sa) = (a.cos(), a.sin());
        b = b
            .begin_hollow(CVec2::new(cx + ca * inner, cy + sa * inner))
            .line_to(CVec2::new(cx + ca * outer, cy + sa * outer))
            .end_open();
        any = true;
    }
    if !any {
        return None;
    }
    let path = b.build().ok()?;
    let geometry: ID2D1Geometry = path.raw().cast().ok()?;
    let source: IGeometrySource2D = ArcGeometrySource { geometry }.into();
    CompositionPath::Create(&source).ok()
}

/// A rounded rectangle as a closed path, corners as cubic Béziers.
///
/// The knob's focus ring, and the reason it is a path rather than the nine-grid
/// bar every other control's ring uses: the knob owns no `Parts` band, so it has
/// no `Part::bind` to set up the grid for it. It does have `ChromeLayer`, which
/// strokes an arbitrary path — and a ring is one.
fn build_ring_path(
    gpu: &GpuDevice,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
) -> Option<CompositionPath> {
    // Circular-arc control-point ratio: a quarter turn's Bézier handle length.
    const K: f32 = 0.552_284_75;
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let (x1, y1) = (x + w, y + h);
    let k = K * r;
    let path = PathBuilder::new(gpu)
        .ok()?
        .begin_hollow(CVec2::new(x + r, y))
        .line_to(CVec2::new(x1 - r, y))
        .bezier_to(
            CVec2::new(x1 - r + k, y),
            CVec2::new(x1, y + r - k),
            CVec2::new(x1, y + r),
        )
        .line_to(CVec2::new(x1, y1 - r))
        .bezier_to(
            CVec2::new(x1, y1 - r + k),
            CVec2::new(x1 - r + k, y1),
            CVec2::new(x1 - r, y1),
        )
        .line_to(CVec2::new(x + r, y1))
        .bezier_to(
            CVec2::new(x + r - k, y1),
            CVec2::new(x, y1 - r + k),
            CVec2::new(x, y1 - r),
        )
        .line_to(CVec2::new(x, y + r))
        .bezier_to(
            CVec2::new(x, y + r - k),
            CVec2::new(x + r - k, y),
            CVec2::new(x + r, y),
        )
        .close()
        .build()
        .ok()?;
    let geometry: ID2D1Geometry = path.raw().cast().ok()?;
    let source: IGeometrySource2D = ArcGeometrySource { geometry }.into();
    CompositionPath::Create(&source).ok()
}

/// One retained chrome layer: a path stroked opaque white into an off-tree
/// `ShapeVisual`, snapshotted by a `CompositionVisualSurface`, and used as the
/// MASK of a `CompositionMaskBrush` whose SOURCE is an FP16 colour surface.
///
/// The value arc's chain minus the trim. It is a separate layer per colour
/// rather than more shapes in the arc's own mask shape, for three reasons, any
/// one of which is fatal: that mask feeds one `MaskBrush` with one source, so a
/// ring appended there would render in the ACCENT colour; an untrimmed 10-DIP
/// band composited into the same mask alpha would swallow the arc's trim
/// entirely; and the arc's geometry is the trim spring's target, so sharing the
/// object would trim the ring by the value.
///
/// What IS shared is the tessellated `CompositionPath` — the expensive half —
/// exactly as the thumb already shares it.
struct ChromeLayer {
    shape: ICompositionSpriteShape,
    mask_shape: ShapeVisual,
    visual_surface: CompositionVisualSurface,
    mask_brush: CompositionMaskBrush,
    display_vis: IVisual,
    _display: SpriteVisual,
    /// How far outside the node's rect this layer is allowed to paint.
    ///
    /// A `ShapeVisual` clips to its own size and a `CompositionVisualSurface`
    /// captures only the region it is told to, so a layer whose geometry leaves
    /// the node's bounds is cut at them — which is every focus ring, since a
    /// ring sits OUTSIDE the control it rings. The layer is therefore grown by
    /// `bleed` on all four sides and its sprite offset back by the same, and
    /// callers author such a path already shifted into positive space.
    ///
    /// Zero for the dial's own chrome, all of which is inside the bounds.
    bleed: f32,
}

impl ChromeLayer {
    fn new(
        comp: &Compositing,
        node: &Node,
        path: &CompositionPath,
        width: f32,
        round_caps: bool,
        bleed: f32,
    ) -> Option<Self> {
        let (w, h) = (node.rect.w + 2.0 * bleed, node.rect.h + 2.0 * bleed);
        let display = comp.new_sprite().ok()?;
        let display_vis: IVisual = display.cast().ok()?;
        let compositor = display.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
        let c5 = compositor.cast::<ICompositor5>().ok()?;
        let c2 = compositor.cast::<ICompositor2>().ok()?;
        let cvs = compositor.cast::<ICompositorWithVisualSurface>().ok()?;

        let geo = c5.CreatePathGeometryWithPath(path).ok()?;
        let shape_c = c5.CreateSpriteShapeWithGeometry(&geo).ok()?;
        let shape: ICompositionSpriteShape = shape_c.cast().ok()?;
        shape.SetStrokeThickness(width).ok()?;
        if round_caps {
            shape.SetStrokeStartCap(CompositionStrokeCap::Round).ok()?;
            shape.SetStrokeEndCap(CompositionStrokeCap::Round).ok()?;
        }
        // Opaque white — the mask reads only the alpha channel.
        let white = compositor
            .CreateColorBrushWithColor(UiColor { a: 255, r: 255, g: 255, b: 255 })
            .ok()?;
        shape.SetStrokeBrush(&white.cast::<CompositionBrush>().ok()?).ok()?;

        let mask_shape = c5.CreateShapeVisual().ok()?;
        mask_shape.cast::<IVisual>().ok()?.SetSize(Vector2::new(w, h)).ok()?;
        mask_shape.Shapes().ok()?.Append(&shape_c.cast::<CompositionShape>().ok()?).ok()?;

        let visual_surface = cvs.CreateVisualSurface().ok()?;
        visual_surface.SetSourceVisual(&mask_shape.cast::<Visual>().ok()?).ok()?;
        visual_surface.SetSourceOffset(Vector2::new(0.0, 0.0)).ok()?;
        visual_surface.SetSourceSize(Vector2::new(w, h)).ok()?;
        let mask_surf = compositor
            .CreateSurfaceBrushWithSurface(&visual_surface.cast::<ICompositionSurface>().ok()?)
            .ok()?;

        let mask_brush = c2.CreateMaskBrush().ok()?;
        mask_brush.SetMask(&mask_surf.cast::<CompositionBrush>().ok()?).ok()?;

        display_vis.SetSize(Vector2::new(w, h)).ok()?;
        display_vis.SetOffset(Vector3::new(-bleed, -bleed, 0.0)).ok()?;
        display.SetBrush(&mask_brush.cast::<CompositionBrush>().ok()?).ok()?;
        node.container
            .Children()
            .ok()?
            .InsertAtTop(&display.cast::<Visual>().ok()?)
            .ok()?;

        Some(Self {
            shape,
            mask_shape,
            visual_surface,
            mask_brush,
            display_vis,
            _display: display,
            bleed,
        })
    }

    /// Re-point the layer at a freshly tessellated path (a resize, or a tick
    /// list that changed under it).
    fn set_path(&mut self, c5: &ICompositor5, path: &CompositionPath) {
        if let Ok(geo) = c5.CreatePathGeometryWithPath(path) {
            let _ = self.shape.SetGeometry(&geo);
        }
    }

    fn resize(&self, w: f32, h: f32) {
        let (w, h) = (w + 2.0 * self.bleed, h + 2.0 * self.bleed);
        let _ = self.mask_shape.cast::<IVisual>().map(|v| v.SetSize(Vector2::new(w, h)));
        let _ = self.display_vis.SetSize(Vector2::new(w, h));
        let _ = self.visual_surface.SetSourceSize(Vector2::new(w, h));
    }

    fn set_color(&self, comp: &Compositing, color: crate::Color, scale: f32) {
        if let Some(s) = super::parts::build_solid_surface(comp, color, scale)
            && let Ok(cb) = s.cast::<CompositionBrush>()
        {
            let _ = self.mask_brush.SetSource(&cb);
        }
    }

    fn set_opacity(&self, a: f32) {
        let _ = self.display_vis.SetOpacity(a);
    }
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
    /// The untrimmed background groove — the SAME tessellated path the value arc
    /// rides, stroked wider and in its own mask layer because a mask brush
    /// carries one colour and the arc's is the accent.
    ring: Option<ChromeLayer>,
    ticks_minor: Option<ChromeLayer>,
    ticks_major: Option<ChromeLayer>,
    /// The centre hub: a plain disc, so a sprite over an FP16 circle rather than
    /// a mask layer — three COM objects and a surface capture would be a lot for
    /// one 8-DIP circle.
    hub: SpriteVisual,
    hub_vis: IVisual,
    /// The focus ring, as the same two concentric strokes every other converted
    /// control gets — outside the bounds rather than inset into them. Kept at
    /// zero opacity until focused, so focus is one property write and never a
    /// rebuild.
    focus_rings: [Option<ChromeLayer>; 2],
    /// `(w, h, scale)` the rings were built for. Deliberately NOT `geom`: the
    /// rings follow the node's full rect and the display scale, where the dial
    /// follows its own centre and radius, and the two move independently.
    rings_geom: (f32, f32, f32),
    /// The tick list and `major_every` the two tick layers were tessellated for.
    /// Not covered by `geom`, and both are live props, so a tick change with no
    /// resize would otherwise leave the old tessellation on screen.
    ticks_seen: (Vec<f64>, Option<f64>),
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
        let path = build_arc_path(&comp.gpu, cx, cy, radius, node.ctrl().start_angle, node.ctrl().end_angle)?;

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

        // Bottom-up: ring, ticks, hub, then the arc and the needle. The painted
        // chrome used to sit UNDER everything because `new_surface` inserts at
        // the bottom, so this order reproduces it. Glyph hosts insert at the top
        // afterwards, which keeps the readout above the hub.
        let ring = ChromeLayer::new(comp, node, &path, RING_WIDTH, false, 0.0);
        let ticks_minor = build_tick_path(&comp.gpu, cx, cy, radius, node, false)
            .and_then(|p| ChromeLayer::new(comp, node, &p, TICK_MINOR_W, false, 0.0));
        let ticks_major = build_tick_path(&comp.gpu, cx, cy, radius, node, true)
            .and_then(|p| ChromeLayer::new(comp, node, &p, TICK_MAJOR_W, false, 0.0));

        let hub = comp.new_sprite().ok()?;
        let hub_vis: IVisual = hub.cast().ok()?;
        hub_vis.SetSize(Vector2::new(HUB_D, HUB_D)).ok()?;
        hub_vis.SetOffset(Vector3::new(cx - HUB_D / 2.0, cy - HUB_D / 2.0, 0.0)).ok()?;

        let children = node.container.Children().ok()?;
        children.InsertAtTop(&hub.cast::<Visual>().ok()?).ok()?;
        children.InsertAtTop(&display.cast::<Visual>().ok()?).ok()?;
        children.InsertAtTop(&needle.cast::<Visual>().ok()?).ok()?;

        Some(Self {
            ring,
            ticks_minor,
            ticks_major,
            hub,
            hub_vis,
            // Built on first sync, which is where `scale` reaches us — and the
            // ring's whole geometry is snapped to whole physical pixels, so it
            // cannot be derived without it.
            focus_rings: [None, None],
            rings_geom: (0.0, 0.0, 0.0),
            ticks_seen: (node.ctrl().ticks.clone(), node.ctrl().major_every),
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
        let start = node.ctrl().start_angle;
        let end = node.ctrl().end_angle;

        let resized = self.geom != (cx, cy, radius);
        // `geom` cannot see this: both are live props, and a dial that gains a
        // tick without changing size would keep the old tessellation.
        let ticks_changed = self.ticks_seen.0 != node.ctrl().ticks
            || self.ticks_seen.1 != node.ctrl().major_every;
        if resized || ticks_changed {
            if let Ok(c5) = self
                .needle
                .cast::<ICompositionObject>()
                .and_then(|o| o.Compositor())
                .and_then(|c| c.cast::<ICompositor5>())
            {
                for (layer, major) in [(&mut self.ticks_minor, false), (&mut self.ticks_major, true)]
                {
                    if let Some(p) = build_tick_path(&comp.gpu, cx, cy, radius, node, major)
                        && let Some(l) = layer.as_mut()
                    {
                        l.set_path(&c5, &p);
                    }
                }
            }
            self.ticks_seen.0.clear();
            self.ticks_seen.0.extend_from_slice(&node.ctrl().ticks);
            self.ticks_seen.1 = node.ctrl().major_every;
        }
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
                // The groove rides the very same tessellation.
                if let Ok(c5) = self
                    .needle
                    .cast::<ICompositionObject>()
                    .and_then(|o| o.Compositor())
                    .and_then(|c| c.cast::<ICompositor5>())
                    && let Some(r) = self.ring.as_mut()
                {
                    r.set_path(&c5, &path);
                }
            }
            let _ = self.mask_shape.cast::<IVisual>().map(|v| v.SetSize(Vector2::new(w, h)));
            let _ = self.display_vis.SetSize(Vector2::new(w, h));
            let _ = self.visual_surface.SetSourceSize(Vector2::new(w, h));
            for l in [self.ring.as_ref(), self.ticks_minor.as_ref(), self.ticks_major.as_ref()]
                .into_iter()
                .flatten()
            {
                l.resize(w, h);
            }
            let _ = self
                .hub_vis
                .SetOffset(Vector3::new(cx - HUB_D / 2.0, cy - HUB_D / 2.0, 0.0));
            self.place_needle(cx, cy, radius);
            self.geom = (cx, cy, radius);
        }

        // The focus ring follows the node's full rect and the display scale, not
        // the dial's centre and radius, so it gets its own gate. Built once and
        // then held at zero opacity — focus must be a property write, because it
        // arrives on a Tab and a rebuild there would be visible.
        if self.rings_geom != (w, h, scale) {
            let radius = super::controls::focus_radius(node);
            let rings = super::parts::focus_rings(scale);
            // One bleed for both, taken from the OUTER ring, so the two layers
            // share a coordinate origin and cannot drift apart by a half pixel.
            let bleed = rings[1].0.max(rings[0].0);
            for (i, (out, sw, _)) in rings.into_iter().enumerate() {
                // The bounds grown by `out`, then inset by half the stroke —
                // `ShapeKey::HBar` strokes inset the same way, and this ring has
                // to agree with every other control's. Shifted by `bleed` so the
                // whole path sits in positive space: a `ShapeVisual` clips to its
                // own size, so a ring authored at negative coordinates is cut
                // away before the surface ever captures it.
                let (rx, ry) = (bleed - out + sw / 2.0, bleed - out + sw / 2.0);
                let (rw, rh) = (w + 2.0 * out - sw, h + 2.0 * out - sw);
                let Some(path) = build_ring_path(&comp.gpu, rx, ry, rw, rh, radius + out) else {
                    continue;
                };
                match self.focus_rings[i].as_mut() {
                    Some(l) => {
                        if let Ok(c5) = self
                            .needle
                            .cast::<ICompositionObject>()
                            .and_then(|o| o.Compositor())
                            .and_then(|c| c.cast::<ICompositor5>())
                        {
                            l.set_path(&c5, &path);
                        }
                        l.resize(w, h);
                    }
                    None => {
                        if let Some(l) = ChromeLayer::new(comp, node, &path, sw, false, bleed) {
                            l.set_opacity(0.0);
                            self.focus_rings[i] = Some(l);
                        }
                    }
                }
            }
            self.rings_geom = (w, h, scale);
        }

        // FP16 gradient SOURCE (display-mapped) + needle colour: rebind on a
        // display epoch or a stops-list change.
        if self.grad_epoch != atlas_epoch
            || !stops_eq(&self.stops_seen, &node.ctrl().stops)
            || resized
        {
            let src = if node.ctrl().stops.is_empty() {
                super::parts::build_solid_surface(comp, node.ctrl().accent.unwrap_or_else(theme::accent), scale)
            } else {
                super::parts::build_gradient_surface(comp, &node.ctrl().stops, scale)
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
            // The static dial, in the painter's own tokens.
            if let Some(r) = self.ring.as_ref() {
                r.set_color(comp, theme::w(0.06), scale);
            }
            if let Some(t) = self.ticks_minor.as_ref() {
                t.set_color(comp, theme::w(0.14), scale);
            }
            if let Some(t) = self.ticks_major.as_ref() {
                t.set_color(comp, theme::w(0.28), scale);
            }
            if let Some(hb) = super::parts::build_circle_surface(comp, HUB_D, theme::w(1.0), scale)
                && let Ok(cb) = hb.cast::<CompositionBrush>()
            {
                let _ = self.hub.SetBrush(&cb);
            }
            for (i, (_, _, c)) in super::parts::focus_rings(scale).into_iter().enumerate() {
                if let Some(l) = self.focus_rings[i].as_ref() {
                    l.set_color(comp, c, scale);
                }
            }
            self.grad_epoch = atlas_epoch;
            self.stops_seen.clear();
            self.stops_seen.extend_from_slice(&node.ctrl().stops);
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
        let angle = value_to_angle(node.ctrl().value, node.ctrl().min, node.ctrl().max, start, end);
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
        for l in [self.ring.as_ref(), self.ticks_minor.as_ref(), self.ticks_major.as_ref()]
            .into_iter()
            .flatten()
        {
            l.set_opacity(dim);
        }
        let _ = self.hub_vis.SetOpacity(dim);
        // Focus is one opacity write on a ring that already exists. Unlike the
        // dial, it does NOT take `dim`: a disabled control cannot be focused, so
        // a dimmed ring would only ever be a half-drawn one.
        let ring_a = if node.focus_ring { 1.0 } else { 0.0 };
        for l in self.focus_rings.iter().flatten() {
            l.set_opacity(ring_a);
        }
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
    let span = node.ctrl().max - node.ctrl().min;
    if span.abs() < f64::EPSILON {
        0.0
    } else {
        ((node.ctrl().value - node.ctrl().min) / span).clamp(0.0, 1.0)
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
