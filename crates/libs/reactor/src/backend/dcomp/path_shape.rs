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
//! [`CompositionMaskBrush`] combines them. A flat colour is one display-mapped
//! FP16 cell from [`super::parts`]; a RAMP is a second mask brush nested inside
//! this one ([`super::gradient`]), so the two alphas multiply and no ramp is
//! ever rasterized.
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
//! ## The glow follows the same rule
//!
//! A halo needs the blur to carry ALPHA, not colour, so [`GlowLayer`] lets a
//! `DropShadow` blur a white stroke on the compositor and masks an FP16 source
//! with the result — colour never touches an 8-bit brush, and nothing is
//! rasterized per edit. An effect-graph brush cannot do this: `CompositionMaskBrush`
//! requires a `CompositionSurfaceBrush` for its mask (an effect brush is not one),
//! and its buffers clamp far below paper white — both measured, see [`GlowLayer`].

use windows_canvas::{GpuDevice, Path, PathBuilder, PathFigure, Vector2 as CVec2};
use windows_composition::{
    Color as UiColor, CompositionMaskBrush, CompositionPath, CompositionPathGeometry,
    CompositionSpriteShape, CompositionSurfaceBrush, BorderMode, CompositionVisualSurface, DropShadow,
    ShadowSource, ShapeVisual, SpringScalarNaturalMotionAnimation, SpriteVisual, StrokeCap,
    StrokeJoin,
};
use windows_core::{implement_decl, Interface, Ref, Result};
use windows_numerics::Vector2;

use super::bootstrap::Compositing;
use super::node::Node;
use crate::backend::ControlKind;
use crate::system_bindings::{
    ID2D1Factory, ID2D1Geometry, IGeometrySource2D, IGeometrySource2DInterop,
    IGeometrySource2DInterop_Impl, IGeometrySource2D_Impl,
};
use crate::{Color, GradientAxis, PathGeometry, PathVerb};

/// Spring tuning for a trim change. Matches the knob's arc so a curve drawing
/// itself on reads as the same motion system as every other value chrome.
const TRIM_DAMPING: f32 = 1.0;
const TRIM_PERIOD: f64 = 0.12;

/// Seconds as a [`Duration`](std::time::Duration). Guards the non-finite input
/// `Duration::from_secs_f64` would PANIC on, where the retired `TimeSpan` cast
/// saturated — see `parts::secs`.
fn secs(s: f64) -> std::time::Duration {
    std::time::Duration::from_secs_f64(if s.is_finite() { s.max(0.000_1) } else { 0.000_1 })
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
/// The `Path` is kept, not just its composition wrapper: [`GlowLayer`] builds its
/// own composition geometry from the same stream, so both consumers replay the
/// verbs once.
///
/// Coordinates are replayed in the DIPs they were authored in. Getting them onto
/// the physical pixel grid is [`PathLayer::resize`]'s job, as one transform on
/// the shape.
/// How many consecutive `verb`s start at `from` — the length of one batchable run.
///
/// Always at least 1, since `verbs[from]` is the verb being matched on.
fn run_len(verbs: &[PathVerb], from: usize, verb: PathVerb) -> usize {
    verbs[from..].iter().take_while(|&&v| v == verb).count()
}

fn build_d2d_path(
    gpu: &GpuDevice,
    verbs: &[PathVerb],
    pts: &[f32],
    filled: bool,
) -> Option<Path> {
    let mut builder = Some(PathBuilder::new(gpu).ok()?);
    let mut figure: Option<PathFigure> = None;
    let mut i = 0usize;
    let mut v = 0usize;

    let read = |i: &mut usize, n: usize| -> Option<&[f32]> {
        let s = pts.get(*i..*i + n * 2)?;
        *i += n * 2;
        Some(s)
    };

    // Walked in RUNS of like verbs rather than one verb at a time. Each segment
    // call is a COM crossing, and the sampled curves this exists for are one long
    // run — a 512-point response was 512 crossings to hand over 4KB of
    // coordinates that were already contiguous and already in D2D's own layout.
    // A run now goes over in one call, so the cost tracks the number of runs
    // (normally one per figure) instead of the number of points.
    while v < verbs.len() {
        match verbs[v] {
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
                v += 1;
            }
            PathVerb::Line => {
                let n = run_len(verbs, v, PathVerb::Line);
                let p = read(&mut i, n)?;
                figure = Some(figure.take()?.line_to_flat(p));
                v += n;
            }
            PathVerb::Cubic => {
                let n = run_len(verbs, v, PathVerb::Cubic);
                let p = read(&mut i, n * 3)?;
                figure = Some(figure.take()?.bezier_to_flat(p));
                v += n;
            }
            PathVerb::Close => {
                builder = Some(figure.take()?.close());
                v += 1;
            }
        }
    }
    // D2D requires every figure be ended before the sink closes.
    if let Some(f) = figure.take() {
        builder = Some(f.end_open());
    }
    builder.take()?.build().ok()
}

/// Wrap a D2D [`Path`] as a composition path via the one-geometry source.
///
/// `Compositor::create_path` is an ACCEPTING seam: it takes any COM object
/// implementing `IGeometrySource2D` and does the widening itself, so the
/// implement-side type above crosses unchanged and no raw composition type is
/// named here. It takes the whole [`Compositing`] rather than a bare
/// `&Compositor` because every caller has one and most also need `comp.gpu` to
/// tessellate the path first.
pub(crate) fn to_composition_path(comp: &Compositing, path: &Path) -> Option<CompositionPath> {
    let geometry: ID2D1Geometry = path.raw().cast().ok()?;
    let source: IGeometrySource2D = PathGeometrySource { geometry }.into();
    comp.compositor().create_path(&source).ok()
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
    comp: &Compositing,
    cx: f32,
    cy: f32,
    radius: f32,
    start: f32,
    end: f32,
) -> Option<CompositionPath> {
    let fig = PathBuilder::new(&comp.gpu)
        .ok()?
        .begin_hollow(CVec2::new(cx + radius * start.cos(), cy + radius * start.sin()));
    // Chorded into a stack buffer and handed over in ONE call, for the reason
    // the transported curves are (see `build_d2d_path`): a per-chord `AddLine`
    // was 96 COM crossings to describe an arc, and every Knob and ProgressRing
    // rebuilds its arc whenever it resizes. The buffer is sized by the same
    // constant that drives the loop, so the two cannot disagree.
    let mut xy = [0.0f32; ARC_SEGMENTS as usize * 2];
    for i in 1..=ARC_SEGMENTS as usize {
        let a = start + (end - start) * (i as f32 / ARC_SEGMENTS as f32);
        xy[(i - 1) * 2] = cx + radius * a.cos();
        xy[(i - 1) * 2 + 1] = cy + radius * a.sin();
    }
    to_composition_path(comp, &fig.line_to_flat(&xy).end_open().build().ok()?)
}

/// Replay the verbs and wrap the result as a composition path — the mask
/// layers' input. See [`build_d2d_path`].
///
/// Takes the two arrays rather than a `PathData` so the transported geometry
/// and [`live_trace`](super::live_trace)'s reusable buffers reach the same replay
/// without either of them having to mint the other's type.
pub(crate) fn build_composition_path(
    comp: &Compositing,
    verbs: &[PathVerb],
    points: &[f32],
    filled: bool,
) -> Option<CompositionPath> {
    to_composition_path(comp, &build_d2d_path(&comp.gpu, verbs, points, filled)?)
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

/// Size a mask subtree and its capture for `scale`, and put that scale on the
/// shape so the geometry inside it lands on the physical pixel grid.
///
/// Shared by every mask layer in the backend: [`PathLayer`] and [`GlowLayer`]
/// here, and the knob's [`ChromeLayer`](super::knob) and its arc-plus-thumb
/// layer. Those keep their own layer types only because each carries different
/// extra state (a trim, a bleed, a halo); the SIZING rule is one rule, and lives
/// here so the four cannot drift.
///
/// # The rule
///
/// The captured extent and the geometry inside it must be in the SAME space. A
/// `CompositionVisualSurface` stretches what it captured onto the display
/// sprite, so a shape covering only part of the captured extent renders
/// proportionally small — which is exactly how every app-transported curve came
/// to be cut off at two thirds of its host box at 150%.
///
/// The scale goes on the SHAPE, never on the mask visual. A visual surface
/// captures its source visual's CONTENT, and that visual's own transform is not
/// part of what it captures — a scale on the mask VISUAL looks like it does this
/// job and does nothing at all. A shape's transform IS content, so it survives.
/// (The wrapper puts `set_scale` on `CompositionSpriteShape` and deliberately
/// documents that reason there too.)
///
/// # Why physical rather than DIPs
///
/// Not for sharpness. DIP sourcing renders identically today — this exact
/// change was measured against the previous DIP-sized layers at 150% and moved
/// ZERO pixels, because DWM rasterizes captured vector content at the device
/// resolution it composites at, whatever `SourceSize` says.
///
/// It is physical because [`PathLayer`] is, and one rule
/// across every mask layer in the backend is worth more than the property sets
/// it costs: the alternative is a reader having to know which of two
/// conventions a given layer follows, and the failure mode for guessing wrong
/// is silent mis-sizing rather than a compile error. Sourcing at the physical
/// extent also means the capture can never be the thing that caps the raster,
/// if that ever stops being DWM's choice to make.
pub(crate) fn size_mask(
    mask_shape: &ShapeVisual,
    visual_surface: &CompositionVisualSurface,
    shapes: &[&CompositionSpriteShape],
    w: f32,
    h: f32,
    scale: f32,
) {
    let phys = Vector2::new(w * scale, h * scale);
    mask_shape.set_size(phys.x, phys.y);
    visual_surface.set_source_size(phys);
    for shape in shapes {
        shape.set_scale(Vector2::new(scale, scale));
    }
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
    /// The geometry. It carries its own `set_trim_*` AND its own
    /// `start_animation` / `stop_animation`, so the separate `CompositionObject`
    /// face the raw path had to keep alongside it is gone.
    geo: CompositionPathGeometry,
    shape: CompositionSpriteShape,
    /// The off-tree mask subtree. Held as the `ShapeVisual` itself now rather
    /// than as a widened `IVisual`; it derefs to `Visual` for the sizing write.
    mask_shape: ShapeVisual,
    visual_surface: CompositionVisualSurface,
    mask_brush: CompositionMaskBrush,
    display: SpriteVisual,
    /// The bound FP16 source and what it was built for — a solid colour's raw
    /// bits, or the exact stop list. Rebuilt only when one really changes.
    _source: Option<CompositionSurfaceBrush>,
    /// The compositor-side ramp, when the layer is on a gradient. Held because
    /// it needs the layer's extent (see [`super::gradient::RampSource::resize`])
    /// and because nothing else keeps its brushes alive.
    ramp: Option<super::gradient::RampSource>,
    /// The capture + gradient mask a ramp is applied through. Built the first
    /// time this layer shows one and kept thereafter, so a curve that toggles
    /// between a flat colour and a ramp rebinds one brush and mints nothing.
    ramp_stage: Option<super::gradient::RampStage>,
    solid_for: Option<([u32; 4], u32)>,
    stops_seen: Vec<(f64, Color)>,
    /// The axis that stop list was rasterized on. `None` while the layer is on a
    /// flat colour, so a first gradient always builds.
    axis_seen: Option<GradientAxis>,
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
fn color_bits(c: Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

/// Exact stop-list comparison on raw bits, for the same reason
/// [`super::knob`]'s does it this way: a digest can collide silently, and the
/// failure mode is a curve that keeps rendering the previous ramp.
fn stops_eq(a: &[(f64, Color)], b: &[(f64, Color)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|((pa, ca), (pb, cb))| {
            pa.to_bits() == pb.to_bits() && color_bits(*ca) == color_bits(*cb)
        })
}

impl PathLayer {
    /// Builds the layer under `container` — the host node's container visual.
    /// Taken directly rather than as a `&Node` so a caller holding the node
    /// mutably (the retained-trace service, whose field lives ON the node) can
    /// still hand over the one thing this needs.
    pub(crate) fn new(
        comp: &Compositing,
        container: &windows_composition::ContainerVisual,
        path: &CompositionPath,
        role: Role,
    ) -> Self {
        let display = comp.new_sprite();
        let compositor = comp.compositor();

        // ── The mask: an opaque-white shape over the app's geometry ──
        let geo = compositor.create_path_geometry(path);
        // Full extent by default; a caller that animates the draw-on retargets
        // `TrimEnd` from here.
        geo.set_trim_start(0.0);
        geo.set_trim_end(1.0);
        let shape = compositor.create_sprite_shape(&geo);
        let white = compositor.create_color_brush(UiColor::rgb(255, 255, 255));
        match role {
            Role::Fill => shape.set_fill_brush(&white),
            Role::Stroke => {
                shape.set_stroke_brush(&white);
                // Round caps and joins: these are sampled curves, and a mitre
                // spike on a steep spline segment is the classic artefact.
                // Both caps in one call — the wrapper sets start and end
                // together, which is the only combination that has a use here.
                shape.set_stroke_caps(StrokeCap::Round);
                // The join has to be stated as well: the compositor mitres by
                // default, where the painted trace this replaces joins round
                // (`DrawKit`'s round style), so a retained curve and a painted
                // one of the same points disagreed at every sharp turn.
                shape.set_stroke_join(StrokeJoin::Round);
            }
        }

        let mask_shape = compositor.create_shape_visual();
        mask_shape.shapes().append(&shape);

        // ── Live snapshot of the mask → a surface brush ──
        // The mask ShapeVisual is OFF-TREE, so `BorderMode::Inherit` (the default)
        // has no parent to inherit from. Ask for antialiased edges explicitly —
        // this visual is rasterized by DWM through the visual-surface capture, and
        // it is the only place that rasterization's quality can be stated.
        mask_shape.set_border_mode(BorderMode::Soft);

        let visual_surface = compositor.create_visual_surface();
        visual_surface.set_source_visual(&mask_shape);
        visual_surface.set_source_offset(Vector2::new(0.0, 0.0));

        let mask_surf = compositor.create_surface_brush(&visual_surface);
        let mask_brush = compositor.create_mask_brush();
        mask_brush.set_mask(&mask_surf);
        display.set_brush(&mask_brush);

        container.children().insert_at_top(&display);

        Self {
            role,
            geo,
            shape,
            mask_shape,
            visual_surface,
            mask_brush,
            display,
            _source: None,
            ramp: None,
            ramp_stage: None,
            solid_for: None,
            stops_seen: Vec::new(),
            axis_seen: None,
            grad_epoch: u32::MAX,
            size: None,
            thickness: None,
            // Deliberately not (0.0, 1.0): the constructor already wrote that,
            // and recording it here would let a caller's first explicit
            // full-extent trim be skipped as "unchanged".
            trim: (f32::NAN, f32::NAN),
            trim_spring: None,
        }
    }

    /// Re-point the geometry at a new path — the app's curve changed shape.
    ///
    /// ONE property write. The `CompositionPath` is immutable, but the GEOMETRY
    /// holding it is not, so nothing here is minted or rebound: no new geometry,
    /// no `set_geometry` on the shape, and no re-writing of a trim the surviving
    /// object still holds. A reshape drag is the hot case — it lands here on
    /// every tick — and this is the whole of it.
    ///
    /// Keeping the object also keeps the trim spring VALID. This used to replace
    /// the geometry and therefore had to drop the spring, which is why a curve
    /// mid draw-on that changed shape snapped to its target instead of
    /// continuing; the spring is bound to the object, and the object survives.
    pub(crate) fn set_path(&mut self, path: &CompositionPath) {
        self.geo.set_path(path);
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
        // captures — so a scale on the mask VISUAL cannot do this, and for a long time
        // the geometry silently rendered `scale`× too small: at 150% a 48-DIP
        // ellipse came out 46 px where it should be 70, and every app curve was
        // cut off at two thirds of its host box. A shape's transform IS content,
        // so it survives the capture.
        //
        // Putting it here rather than in the geometry is what keeps DIPs the
        // only space anyone authors in: a display change is one property set,
        // not a re-tessellation of every curve in the tree.
        size_mask(&self.mask_shape, &self.visual_surface, &[&self.shape], w, h, scale);
        // The display sprite stays in DIPs — it IS under the root scale.
        self.display.set_size(w, h);
        // A single-hue ramp is `MappingMode::Relative` and follows the sprite by
        // itself; a multi-hue staircase is a real visual tree and takes the
        // extent. Either way nothing is re-rasterized here.
        if let Some(r) = self.ramp.as_ref() {
            r.resize(w, h, scale);
        }
        if let Some(st) = self.ramp_stage.as_ref() {
            st.resize(w, h, scale);
        }
        self.size = Some((w, h, scale));
    }

    pub(crate) fn set_thickness(&mut self, t: f32) {
        if self.role != Role::Stroke || self.thickness == Some(t) {
            return;
        }
        self.shape.set_stroke_thickness(t);
        self.thickness = Some(t);
    }

    /// Bind the FP16 colour source — a gradient when the app supplied stops,
    /// otherwise the flat colour. Rebuilds only on a real change or a display
    /// epoch bump, so a recolour is one call and a no-op sync is none.
    pub(crate) fn set_source(
        &mut self,
        comp: &Compositing,
        color: Color,
        stops: &[(f64, Color)],
        axis: GradientAxis,
        atlas_epoch: u32,
        scale: f32,
    ) {
        let want_solid = (color_bits(color), scale.to_bits());
        // The axis is in the gate, not just the stops: the same ramp turned on
        // its side is a different raster, and a layer that compared only stops
        // would keep serving the old orientation for the life of the shape.
        let unchanged = self.grad_epoch == atlas_epoch
            && if stops.is_empty() {
                self.solid_for == Some(want_solid) && self.stops_seen.is_empty()
            } else {
                stops_eq(&self.stops_seen, stops) && self.axis_seen == Some(axis)
            };
        if unchanged {
            return;
        }
        // A ramp is a compositor brush, not a raster: its shape lives in a
        // gradient brush's alpha and its colour in flat FP16 sources, so the
        // axis picks the gradient's direction rather than which bitmap to
        // stretch. Nesting it under this layer's shape mask multiplies the two
        // alphas — coverage from the shape, ramp from the gradient — and leaves
        // the trim, the geometry and the capture above it untouched.
        // The flat branch serves a layer the app gave no ramp at all.
        if stops.is_empty() {
            let Some(s) = super::parts::build_solid_surface(comp, color, scale) else {
                return;
            };
            self.mask_brush.set_source(&s);
            // Straight back onto the shape mask: no capture, no ramp.
            self.display.set_brush(&self.mask_brush);
            self._source = Some(s);
            self.ramp = None;
            self.solid_for = Some(want_solid);
            self.stops_seen.clear();
            self.axis_seen = None;
        } else {
            let Some(r) = super::gradient::RampSource::build(comp, stops, axis, scale) else {
                return;
            };
            if let Some((w, h, s)) = self.size {
                r.resize(w, h, s);
            }
            // The colour rides the mask brush exactly as a flat one does. The
            // RAMP goes on top of a capture of that, never inside one — see
            // `gradient::RampStage` for the two routes that measured wrong.
            self.mask_brush.set_source(r.colour());
            if self.ramp_stage.is_none() {
                let st = super::gradient::RampStage::new(comp, &self.mask_brush);
                if let Some((w, h, s)) = self.size {
                    st.resize(w, h, s);
                }
                self.ramp_stage = Some(st);
            }
            let Some(stage) = self.ramp_stage.as_ref() else { return };
            stage.set_ramp(r.ramp());
            self.display.set_brush(stage.brush());
            self.ramp = Some(r);
            self._source = None;
            self.solid_for = None;
            self.stops_seen = stops.to_vec();
            self.axis_seen = Some(axis);
        }
        self.grad_epoch = atlas_epoch;
    }

    /// Retarget the draw-on trim. `TrimStart` snaps (it is a static crop);
    /// `TrimEnd` springs, so a curve revealing itself does so DWM-side with no
    /// app frame — the payoff the retained path exists for.
    pub(crate) fn set_trim(&mut self, start: f32, end: f32) {
        if self.trim == (start, end) {
            return;
        }
        if self.trim.0 != start {
            self.geo.set_trim_start(start);
        }
        if self.trim.1 != end {
            self.spring_trim_end(end);
        }
        self.trim = (start, end);
    }

    /// Retarget the cached trim spring. Built once and retargeted thereafter —
    /// a fresh animation per retarget would reset the spring's velocity and
    /// turn a redirection into a restart.
    fn spring_trim_end(&mut self, target: f32) {
        if self.trim_spring.is_none() {
            let a = self.display.compositor().create_spring_scalar_animation();
            a.set_damping_ratio(TRIM_DAMPING);
            a.set_period(secs(TRIM_PERIOD));
            self.trim_spring = Some(a);
        }
        let Some(a) = self.trim_spring.as_ref() else { return };
        a.set_final_value(target);
        // Started on the GEOMETRY, which carries its own animation seam —
        // `TrimEnd` is the geometry's property, not the visual's.
        self.geo.start_animation("TrimEnd", a);
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
        self.geo.stop_animation("TrimEnd");
        self.geo.set_trim_start(start);
        self.geo.set_trim_end(end);
        self.trim = (start, end);
    }

    pub(crate) fn set_opacity(&self, a: f32) {
        self.display.set_opacity(a);
    }

    /// The in-tree sprite, for a caller that animates the layer as a whole.
    ///
    /// Handed out rather than wrapping each transform property, because the one
    /// caller ([`super::ring_shape`]) drives `RotationAngle` on the composited
    /// layer — one cheap transform on an already-rasterized mask, as against
    /// rotating the off-tree mask visual and re-rasterizing its alpha every
    /// frame.
    ///
    /// One handle now, not two: `SpriteVisual` derefs to `Visual`, so the caller
    /// reaches `set_center_point` / `set_rotation_angle` and the animation seam
    /// through the same value.
    pub(crate) fn display(&self) -> &SpriteVisual {
        &self.display
    }
}

// ── The glow layer: a compositor-blurred halo over an FP16 colour ────────────

/// A soft glow behind the stroke.
///
/// ## The blur runs on the compositor, the colour never does
///
/// A halo needs the blur to carry **alpha**, not colour — and alpha is `0..1`, a
/// range an 8-bit tint represents exactly. So a `DropShadow` (at zero offset: a
/// glow, not a shadow) blurs a white stroke purely as an *alpha generator*, that
/// halo is captured through a visual surface, and it masks the same kind of FP16
/// source surface the [`PathLayer`]s use. Colour therefore stays in an
/// app-allocated `Rgba16Float` surface end to end.
///
/// This matters because every route that lets the COMPOSITOR carry the colour
/// clamps it. Measured on a 240-nit HDR desktop, where paper white is scRGB 3.0
/// (`examples/dcomp_effect_precision.rs` probes the raw FP16 frame):
///
/// | route | ceiling |
/// |---|---|
/// | effect-graph brush (`GaussianBlur` → `Composite`) | 1.0 — a third of paper white |
/// | a `DropShadow`'s own 8-bit tint | 2.9941 — exactly paper white |
/// | **this: shadow as alpha, FP16 source** | **3.9922 — unclamped** |
///
/// `DropShadow.Opacity` above 1.0 is clamped, and `InheritFromVisualContent` is a
/// *masking* policy that ignores the content's colour, so neither escapes the
/// tint's ceiling. The mask indirection is what does.
///
/// ## Why it replaced a baked surface
///
/// This used to rasterize the halo itself — stroke into an off-screen bitmap,
/// `D2D1Shadow`, composite — re-baked on every geometry, size, colour or
/// thickness change. The compositor now blurs it, so an edit costs a geometry
/// write and nothing else: no bitmap, no blur pass, no per-edit raster.
///
/// It sits at the BOTTOM of the node's container, so both mask layers draw over
/// it regardless of creation order.
struct GlowLayer {
    /// The white stroke the shadow blurs. Off-tree — only its visual surface
    /// reads it — and stroked WHITE so its alpha is at full strength whatever
    /// the eventual tint.
    geo: CompositionPathGeometry,
    shape: CompositionSpriteShape,
    stroke_shape: ShapeVisual,
    stroke_surface: CompositionVisualSurface,
    /// Off-tree sprite that draws nothing and exists only to cast the shadow.
    /// Its content is transparent so the capture below is pure halo: a sharp
    /// stroke here would double the alpha the real stroke layer already draws.
    halo_sprite: SpriteVisual,
    shadow: DropShadow,
    halo_surface: CompositionVisualSurface,
    mask_brush: CompositionMaskBrush,
    display: SpriteVisual,
    _source: Option<CompositionSurfaceBrush>,
    // ── change gates ──
    solid_for: Option<([u32; 4], u32)>,
    size: Option<(f32, f32, f32)>,
    thickness: Option<f32>,
    blur: Option<u32>,
}

impl GlowLayer {
    /// `path` is the stroke layer's hollow geometry, handed over rather than
    /// re-tessellated: the halo blurs exactly the line the stroke draws, so the
    /// two share one walk of the verb stream and one `CompositionPath`.
    fn new(comp: &Compositing, node: &Node, path: &CompositionPath) -> Option<Self> {
        let compositor = comp.compositor();

        let geo = compositor.create_path_geometry(path);
        let shape = compositor.create_sprite_shape(&geo);
        shape.set_stroke_brush(&compositor.create_color_brush(UiColor::rgb(255, 255, 255)));
        shape.set_stroke_caps(StrokeCap::Round);
        // The halo blurs exactly the line the stroke draws, joins included.
        shape.set_stroke_join(StrokeJoin::Round);

        let stroke_shape = compositor.create_shape_visual();
        stroke_shape.shapes().append(&shape);
        // Off-tree, as in `PathLayer::new` — state the edge quality explicitly.
        stroke_shape.set_border_mode(BorderMode::Soft);

        let stroke_surface = compositor.create_visual_surface();
        stroke_surface.set_source_visual(&stroke_shape);
        stroke_surface.set_source_offset(Vector2::new(0.0, 0.0));

        // The shadow blurs THAT alpha. Zero offset makes it a centred glow.
        let shadow = compositor.create_drop_shadow();
        shadow.set_offset(0.0, 0.0, 0.0);
        shadow.set_opacity(1.0);
        shadow.set_color(UiColor::rgb(255, 255, 255));
        shadow.set_source(ShadowSource::Color);
        shadow.set_mask(&compositor.create_surface_brush(&stroke_surface));

        let halo_sprite = comp.new_sprite();
        halo_sprite.set_shadow(&shadow);
        // Off-tree and captured, like every mask source here — state the edge
        // quality rather than inheriting from a parent that does not exist.
        halo_sprite.set_border_mode(BorderMode::Soft);

        let halo_surface = compositor.create_visual_surface();
        halo_surface.set_source_visual(&halo_sprite);
        halo_surface.set_source_offset(Vector2::new(0.0, 0.0));

        let mask_brush = compositor.create_mask_brush();
        mask_brush.set_mask(&compositor.create_surface_brush(&halo_surface));

        let display = comp.new_sprite();
        display.set_brush(&mask_brush);
        node.container.children().insert_at_bottom(&display);

        Some(Self {
            geo,
            shape,
            stroke_shape,
            stroke_surface,
            halo_sprite,
            shadow,
            halo_surface,
            mask_brush,
            display,
            _source: None,
            solid_for: None,
            size: None,
            thickness: None,
            blur: None,
        })
    }

    /// Re-point the blurred stroke at new geometry. One property write — the
    /// compositor re-blurs; nothing is minted or rasterized here.
    fn set_path(&mut self, path: &CompositionPath) {
        self.geo.set_path(path);
    }

    /// Size every stage. The two off-tree visuals never inherit the root's
    /// DIP→px scale (they are only visual-surface sources), so — exactly as in
    /// [`PathLayer::resize`] — the scale rides the SHAPE, whose transform is
    /// content and therefore survives capture, while the extents are physical.
    fn resize(&mut self, w: f32, h: f32, scale: f32) {
        if self.size == Some((w, h, scale)) {
            return;
        }
        let phys = Vector2::new(w * scale, h * scale);
        size_mask(&self.stroke_shape, &self.stroke_surface, &[&self.shape], w, h, scale);
        // The halo stage already works in physical pixels (it captures the
        // stroke surface), so it takes the extent but no scale of its own.
        self.halo_sprite.set_size(phys.x, phys.y);
        self.halo_surface.set_source_size(phys);
        // The display sprite stays in DIPs — it IS under the root scale.
        self.display.set_size(w, h);
        self.size = Some((w, h, scale));
    }

    /// Sync the halo's inputs. Each is gated separately, so a drag that only
    /// moves geometry writes one property and a recolour rebuilds one surface.
    #[allow(clippy::too_many_arguments)]
    /// `path` is the stroke layer's hollow composition path, and is `None`
    /// whenever the caller had no reason to build one — see the call site. Only a
    /// geometry change consumes it, so the two travel together: a `None` here
    /// means the halo keeps the geometry it already holds.
    fn sync(
        &mut self,
        comp: &Compositing,
        path: Option<&CompositionPath>,
        geometry_changed: bool,
        color: Color,
        blur: f32,
        thickness: f32,
        w: f32,
        h: f32,
        scale: f32,
    ) {
        self.resize(w, h, scale);
        if geometry_changed
            && let Some(p) = path
        {
            self.set_path(p);
        }
        if self.thickness != Some(thickness) {
            self.shape.set_stroke_thickness(thickness.max(0.5));
            self.thickness = Some(thickness);
        }
        // `DropShadow.BlurRadius` is **not** a standard deviation — like CSS
        // `box-shadow`, it is 2σ, the same convention canvas `shadowBlur` uses.
        // So the authored value passes straight through and a mockup constant
        // means exactly what it did on a canvas.
        //
        // Measured, not assumed. Halving it here (on the theory that the property
        // was σ) rendered every halo at half its spread: against the D2D
        // `D2D1Shadow` glow it replaces, the analyzer response curve's halo fell
        // to half strength 2.4 px off the stroke where the painted one took 4.7 —
        // a ratio of 2.0 across the whole falloff, which is what a σ off by two
        // looks like. Passing the value through puts the two on top of each other.
        //
        // Physical, because the alpha it blurs was captured at physical size.
        let radius = blur * scale;
        if self.blur != Some(radius.to_bits()) {
            self.shadow.set_blur_radius(radius);
            self.blur = Some(radius.to_bits());
        }
        // The FP16 colour the halo is masked against — the same solid-source
        // builder the stroke and fill layers bind, so a glow tint and a stroke
        // tint travel one colour path.
        let want_solid = (glow_bits(color), scale.to_bits());
        if self.solid_for != Some(want_solid)
            && let Some(src) = super::parts::build_solid_surface(comp, color, scale)
        {
            self.mask_brush.set_source(&src);
            self._source = Some(src);
            self.solid_for = Some(want_solid);
        }
    }
}

/// Quantize a glow colour to raw bits so its gate is `Eq` without float caveats.
fn glow_bits(c: Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
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
/// Whether a layer was given a colour source of EITHER kind — a flat colour or a
/// ramp.
///
/// A ramp is a colour source in its own right. Asking only whether the flat
/// colour was set made a gradient-only layer silently draw nothing: the app
/// authored stops, the layer was never created, and there was no error to see —
/// the shape simply had no fill.
fn has_colour(flat: Option<Color>, stops: &[(f64, Color)]) -> bool {
    flat.is_some() || !stops.is_empty()
}

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
    let fill_stops = node.ctrl().stops.as_slice();
    let want_fill = has_colour(node.paint.fill, fill_stops);
    let (stroke_color, stroke_w) = stroke_of(node);
    let stroke_stops = node.paint.path_stroke_stops.as_slice();
    // Thickness still gates the stroke either way: a zero-width outline is not a
    // line however it is coloured.
    let want_stroke = has_colour(stroke_color, stroke_stops) && stroke_w > 0.0;

    // A layer whose role the app stopped asking for goes away entirely, rather
    // than lingering at zero alpha: an unasked-for layer should cost nothing.
    if !want_fill {
        parts.fill = None;
    }
    if !want_stroke {
        parts.stroke = None;
    }

    let geometry_changed = parts.geometry_seen.as_ref() != Some(&geometry);
    // The glow follows the stroke — a halo with no line to sit under reads as a
    // smudge — so it is decided here, before either is tessellated.
    let want_glow = want_stroke && node.paint.path_glow.is_some();

    // ── Tessellate: at most twice, however many layers consume the result ──
    //
    // The two FIGURE MODES are what differ, not the layer count: a filled
    // layer's open figures close implicitly and a stroked layer's stay open, so
    // filled and hollow are genuinely two different D2D geometries. But the
    // STROKE and the GLOW both want the hollow one — the stroke as its mask, the
    // glow as the alpha a `DropShadow` blurs — and they used to build it
    // separately, walking the same verb stream twice per tick to produce two
    // identical geometries. One walk now serves both.
    //
    // Each is built ONLY when something has a use for the result: a layer's first
    // build, or a curve that actually moved. Anything that merely marks the node
    // dirty — a recolour, a hover, a theme flip — re-tessellates nothing.
    let need = |want: bool, slot_empty: bool| want && (slot_empty || geometry_changed);
    let fill_needs = need(want_fill, parts.fill.is_none());
    let stroke_needs = need(want_stroke, parts.stroke.is_none());
    let filled = fill_needs
        .then(|| build_composition_path(comp, geometry.data().verbs(), geometry.data().points(), true))
        .flatten();
    let hollow = (stroke_needs || need(want_glow, parts.glow.is_none()))
        .then(|| build_composition_path(comp, geometry.data().verbs(), geometry.data().points(), false))
        .flatten();

    if let Some(path) = &filled {
        match &mut parts.fill {
            Some(l) => l.set_path(path),
            slot => *slot = Some(PathLayer::new(comp, &node.container, path, Role::Fill)),
        }
    }
    // Gated on the STROKE's own need, not merely on a path existing: the glow's
    // first build tessellates a hollow path the stroke may already be holding,
    // and re-pointing it at an identical geometry is a wasted write.
    if stroke_needs
        && let Some(path) = &hollow
    {
        match &mut parts.stroke {
            Some(l) => l.set_path(path),
            slot => *slot = Some(PathLayer::new(comp, &node.container, path, Role::Stroke)),
        }
    }

    // ── Glow: a compositor-blurred halo BELOW the stroke ──
    if !want_glow {
        parts.glow = None;
    } else if let Some((color, blur)) = node.paint.path_glow {
        if parts.glow.is_none()
            && let Some(p) = &hollow
        {
            parts.glow = GlowLayer::new(comp, node, p);
        }
        if let Some(g) = parts.glow.as_mut() {
            g.sync(
                comp,
                hollow.as_ref(),
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

    if let Some(l) = &mut parts.fill {
        l.resize(w, h, scale);
        l.set_source(
            comp,
            node.paint.fill.unwrap_or_default(),
            fill_stops,
            node.paint.path_fill_grad_axis,
            atlas_epoch,
            scale,
        );
        l.set_trim(node.paint.path_trim.0, node.paint.path_trim.1);
    }
    if let Some(l) = &mut parts.stroke {
        l.resize(w, h, scale);
        // DIPs, like the geometry it strokes — the shape's own scale takes both
        // to physical pixels together.
        l.set_thickness(stroke_w);
        // The stroke takes its OWN ramp, never the fill's: the fill's describes
        // an area fading away from the line and reusing it here would paint the
        // outline in the area's colours, so the two would stop reading as two
        // layers. An empty list leaves the outline on its flat colour.
        l.set_source(
            comp,
            stroke_color.unwrap_or_default(),
            stroke_stops,
            node.paint.path_stroke_grad_axis,
            atlas_epoch,
            scale,
        );
        l.set_trim(node.paint.path_trim.0, node.paint.path_trim.1);
    }
}

#[cfg(test)]
mod tests {
    use super::{has_colour, run_len};
    use crate::{Color, PathVerb};

    // ── Batched replay runs ──────────────────────────────────────────────────
    //
    // `run_len` decides how many segments go over in one COM call, and it also
    // decides how far the point cursor advances. A run that is too LONG reads
    // points belonging to a later verb; too SHORT and the walk desyncs from the
    // verb stream. Both corrupt the geometry rather than merely slowing it.

    /// The whole point: one long polyline is ONE run, not N.
    #[test]
    fn a_polyline_is_a_single_run() {
        let verbs = [PathVerb::Line; 512];
        assert_eq!(run_len(&verbs, 0, PathVerb::Line), 512);
    }

    /// A run stops at the first verb of a different kind, so the cursor never
    /// advances past points that belong to the next verb.
    #[test]
    fn a_run_stops_at_the_first_different_verb() {
        let verbs = [
            PathVerb::Line,
            PathVerb::Line,
            PathVerb::Cubic,
            PathVerb::Line,
            PathVerb::Close,
        ];
        assert_eq!(run_len(&verbs, 0, PathVerb::Line), 2);
        assert_eq!(run_len(&verbs, 2, PathVerb::Cubic), 1);
        assert_eq!(run_len(&verbs, 3, PathVerb::Line), 1);
    }

    /// Never zero — a zero-length run would leave the walk on the same verb and
    /// spin forever, which is worse than any wrong drawing.
    #[test]
    fn a_run_is_never_empty() {
        for (i, v) in [PathVerb::Move, PathVerb::Line, PathVerb::Cubic, PathVerb::Close]
            .into_iter()
            .enumerate()
        {
            let verbs = [PathVerb::Move, PathVerb::Line, PathVerb::Cubic, PathVerb::Close];
            assert!(run_len(&verbs, i, v) >= 1);
        }
    }

    /// Runs must partition the verb stream exactly — every verb consumed once —
    /// or the points cursor and the verbs disagree by the end.
    #[test]
    fn runs_partition_the_whole_verb_stream() {
        let verbs = [
            PathVerb::Move,
            PathVerb::Line,
            PathVerb::Line,
            PathVerb::Line,
            PathVerb::Cubic,
            PathVerb::Cubic,
            PathVerb::Close,
            PathVerb::Move,
            PathVerb::Line,
        ];
        let (mut v, mut runs, mut points) = (0usize, 0usize, 0usize);
        while v < verbs.len() {
            let n = run_len(&verbs, v, verbs[v]);
            points += n * verbs[v].arity() * 2;
            v += n;
            runs += 1;
        }
        assert_eq!(v, verbs.len(), "the walk must land exactly on the end");
        // Move, Line×3, Cubic×2, Close, Move, Line — seven runs, not nine verbs.
        assert_eq!(runs, 6);
        // And the same point total the per-verb walk would have consumed.
        let claimed: usize = verbs.iter().map(|v| v.arity() * 2).sum();
        assert_eq!(points, claimed);
    }

    const RED: Color = Color::rgb(0xFF, 0x00, 0x00);

    fn ramp() -> Vec<(f64, Color)> {
        vec![(0.0, RED), (1.0, Color::rgb(0x00, 0x00, 0xFF))]
    }

    // ── Which layers a shape asks for ────────────────────────────────────────
    //
    // A path's fill and stroke layers are built only for a role the app actually
    // coloured, so this predicate decides whether a layer exists at all. Getting
    // it wrong is silent in the worst way: no error, no warning, just a shape
    // that draws nothing where the app authored a gradient.

    /// A ramp alone is enough. This is the case that was broken — `fill_gradient`
    /// with no flat `fill` beneath it created no layer and drew nothing.
    #[test]
    fn a_ramp_alone_asks_for_a_layer() {
        assert!(has_colour(None, &ramp()));
    }

    /// A flat colour alone is still enough — the case that always worked.
    #[test]
    fn a_flat_colour_alone_asks_for_a_layer() {
        assert!(has_colour(Some(RED), &[]));
    }

    /// Both together, the ordinary authored shape.
    #[test]
    fn both_together_ask_for_a_layer() {
        assert!(has_colour(Some(RED), &ramp()));
    }

    /// Neither asks for nothing: an unasked-for layer must cost nothing rather
    /// than linger as a transparent sprite.
    #[test]
    fn no_colour_at_all_asks_for_no_layer() {
        assert!(!has_colour(None, &[]));
    }
}
