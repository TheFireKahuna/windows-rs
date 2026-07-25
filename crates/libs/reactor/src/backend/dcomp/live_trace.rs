//! One retained polyline a producer thread reshapes without a reconcile — a
//! measured analyzer trace, as a compositor sprite instead of a repainted surface.
//!
//! The sibling of [`bar_field`](super::bar_field), for the facet a bar field
//! cannot express: a continuous line whose *shape* changes every publish. The bars
//! move by one animated scalar each, so DWM owns their motion entirely; a line has
//! no such scalar — its geometry genuinely differs frame to frame. What it can
//! still avoid is the rasterization: a [`CompositionPathGeometry`] re-pointed at a
//! new path is a property write the compositor tessellates, not a surface the UI
//! thread redraws inside DirectComposition's commit.
//!
//! A controlled A/B of exactly this content — a polyline rebuilt every compositor
//! tick, retained shape against repainted surface — measured 4.14% of a core
//! against 17.03% at 512 points, with `CBitmapInfoFront::CommitUpdate`,
//! `CAtlasSurfacePool::D2DEndDraw` and the per-present composition token *absent*
//! from the retained profile rather than merely smaller.
//!
//! ## The seam
//!
//! [`bar_field`](super::bar_field)'s, verbatim: the visual tree is thread-affine,
//! so the producer is handed nothing but a control id. Geometry is queued from
//! whatever thread computed it, coalesced per control (a producer that outruns the
//! front thread overwrites its own pending frame), and applied on the front thread.
//!
//! ## What one publish costs
//!
//! One walk of the verb stream into a Direct2D path, and one `set_path` on a
//! geometry object that already exists. The sprite, its mask shape, the visual
//! surface, the mask brush and the FP16 colour source are all built once per
//! *layout* — a resize, a DPI move, a recolour — and a publish touches none of
//! them. The one thing a publish must mint is the `CompositionPath` itself: a
//! composition path is immutable by contract, so new geometry IS a new path. That
//! is the same hot case [`PathLayer::set_path`](super::path_shape::PathLayer::set_path)
//! documents for an app dragging a curve's shape.
//!
//! There is no per-publish *allocation* on either side of the queue: the verb and
//! point buffers are swapped with the front thread's, so both keep their capacity.
//!
//! ## Multiple runs are ONE shape
//!
//! A path geometry holds any number of disjoint figures, and a trace broken by a
//! coherence gate is exactly that — N runs sharing one ink, one width and one
//! style. So the whole trace is one sprite whose figure count changes freely from
//! publish to publish, never a sprite per run: nothing is created or destroyed when
//! the gate carves the line differently.
//!
//! ## The optional underfill
//!
//! A trace may also carry a FILLED companion — the closed region under the line
//! that an area plot washes in. It is a second [`PathLayer`] at
//! [`Role::Fill`](super::path_shape::Role::Fill), stated and fed by
//! [`LiveTrace::set_fill_path`], and it exists
//! for the same reason the stroke does: an area whose shape changes every publish
//! is a geometry write, not a surface repaint. It is created before the stroke so
//! the line always composites over its own wash, and a trace that never calls
//! [`LiveTrace::set_fill_path`] never builds one.
//!
//! Its ink rides with its geometry rather than sitting in [`TraceLayout`]: an
//! underfill is optional, so putting it there would make every caller that has
//! none say so, and the two are pushed together anyway.
//!
//! ## The fill as a METER
//!
//! A second use of the same underfill: a bar meter whose shape never changes and
//! whose *extent* does. [`LiveTrace::set_fill_extent`] states the fill geometry
//! once — the whole track — and then moves only a scalar, so the level travels
//! under [`bar_field`](super::bar_field)'s ballistics
//! ([`LiveTrace::set_fill_motion`]) instead of arriving as a new path per publish.
//! It scales the composited sprite about the anchored edge, which carries the
//! colour source with it: a ramp under a meter is therefore FILL-relative, the
//! same thing a CSS gradient on the fill element means.
//!
//! That level is the ONE thing here that is interpolated over time, and it is
//! interpolated by whichever mechanism [`live_anim`](super::bar_field::live_anim)
//! names — a retargeted key-frame animation the compositor evaluates, or a
//! one-pole this thread steps. The geometry is not interpolated by either: a
//! published path IS the new shape, and there is no motion between two of them
//! to schedule.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows_composition::{CompositionEasingFunction, ScalarKeyFrameAnimation};
use windows_numerics::Vector3;

use super::bar_field::{
    closing, live_anim, note_anim_start, note_property_writes, retarget_easing, LiveAnim,
    MAX_STEP_SECS,
};
use super::bootstrap::Compositing;
use super::path_shape::{ClipLayer, PathLayer, Role};
use crate::backend::ControlId;
use crate::{Color, GradientAxis, PathVerb};

/// Which construction one of a trace's two layers is built from.
///
/// A trace restates its geometry every publish, and under
/// [`PathLayer`] each restatement dirties a `CompositionVisualSurface` that DWM
/// must then re-render into an intermediate — a per-frame offscreen pass per
/// layer, which measured as the compositor's single largest cost on a live
/// analyzer. [`ClipLayer`] draws the same picture with the colour bound
/// straight to the sprite and the geometry supplied by a clip, so there is
/// nothing to capture.
///
/// The clip route cannot carry a RAMP: a multi-hue source is a staircase of
/// masked layers composited through a capture of its own, which puts back the
/// thing being removed. So a flat layer is clipped and a ramped one is masked,
/// and a layer that changes which it is gets rebuilt.
enum TraceLayer {
    Clipped(ClipLayer),
    Masked(PathLayer),
}

impl TraceLayer {
    fn set_path(&mut self, path: &windows_composition::CompositionPath) {
        match self {
            Self::Clipped(l) => l.set_path(path),
            Self::Masked(l) => l.set_path(path),
        }
    }

    fn resize(&mut self, w: f32, h: f32, scale: f32) {
        match self {
            // No scale: the sprite is IN the tree, under the root's DIP→px
            // scale, so its clip rasterizes at device resolution already. The
            // masked route needs it because its geometry lives off-tree.
            Self::Clipped(l) => l.resize(w, h),
            Self::Masked(l) => l.resize(w, h, scale),
        }
    }

    fn display(&self) -> &windows_composition::SpriteVisual {
        match self {
            Self::Clipped(l) => l.display(),
            Self::Masked(l) => l.display(),
        }
    }

    /// Whether this layer is the flat, capture-free construction — so a caller
    /// can notice that the ramp state no longer matches and rebuild.
    fn is_clipped(&self) -> bool {
        matches!(self, Self::Clipped(_))
    }
}

/// A reusable geometry buffer the producer refills each publish.
///
/// The transported counterpart of [`ShapePath`](crate::ShapePath), and it exists
/// for the one thing that type cannot do: a `ShapePath` builds an immutable
/// `PathGeometry` and therefore allocates its two vectors per build. This one is
/// cleared and refilled, so a producer running at publish rate allocates nothing
/// after warmup.
///
/// The fields are private for `ShapePath`'s reason — `points.len() == 2 · Σ
/// verb.arity()` holds **by construction**, because appending a verb and appending
/// its points is one operation and there is no other way in.
#[derive(Clone, Debug, Default)]
pub struct TracePath {
    verbs: Vec<PathVerb>,
    points: Vec<f32>,
}

impl TracePath {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop the geometry, keeping the capacity — the first call of every publish.
    pub fn clear(&mut self) {
        self.verbs.clear();
        self.points.clear();
    }

    /// Whether this would draw nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    /// Begin a new figure. A trace broken into runs is one `move_to` per run.
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.push(PathVerb::Move, &[x, y]);
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.push(PathVerb::Line, &[x, y]);
    }

    pub fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.push(PathVerb::Cubic, &[c1x, c1y, c2x, c2y, x, y]);
    }

    /// Drops the whole verb when any of its points is non-finite, exactly as
    /// [`ShapePath`](crate::ShapePath) does: a `NaN` coordinate reaching the point
    /// array would make the geometry unequal to itself and reshape forever.
    fn push(&mut self, verb: PathVerb, pts: &[f32]) {
        debug_assert_eq!(verb.arity() * 2, pts.len());
        if pts.iter().any(|v| !v.is_finite()) {
            return;
        }
        self.verbs.push(verb);
        self.points.extend_from_slice(pts);
    }
}

/// Everything about a trace except its geometry: the box it is drawn in, its ink
/// and its width. Pushed only when one of them actually changes — a resize, a DPI
/// move, a theme flip.
///
/// Compared by value before anything is rebound, so a producer that re-pushes an
/// identical layout costs one comparison.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceLayout {
    /// The host element's width in DIPs — the box the mask is captured over.
    pub width: f32,
    /// The host element's height in DIPs.
    pub height: f32,
    /// The stroke's colour. Linear scRGB like every other reactor colour, and
    /// rasterized into the same FP16 display-mapped source every other sprite
    /// takes — a trace and a bar authored from one token land on one colour.
    pub color: Color,
    /// Stroke width in DIPs, in the same space the geometry is authored in.
    pub thickness: f32,
}

/// Which edge of its own box a metered fill grows from.
///
/// A level meter is anchored: a true-peak bar grows rightward off its floor, and
/// a gain-reduction bar grows leftward off zero (the mastering convention). The
/// anchor IS the scale pivot, so this is the whole of the difference between the
/// two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillAnchor {
    /// Grows rightward; the left edge is pinned.
    Left,
    /// Grows leftward; the right edge is pinned.
    Right,
}

/// The ballistics a metered fill travels under — [`BarFieldLayout`](super::bar_field::BarFieldLayout)'s
/// asymmetric pair, for the same reason: a meter that rises and falls at one rate
/// reads as jitter. Each retarget picks one from its own direction of travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FillMotion {
    pub anchor: FillAnchor,
    /// Time to reach a level ABOVE the one shown — the attack.
    pub rise: Duration,
    /// Time to reach a level BELOW it — the release.
    pub fall: Duration,
}

// ── The cross-thread queue ───────────────────────────────────────────────────

/// One trace's pending update. The geometry buffers are retained (and swapped,
/// never cloned, on drain) so neither side allocates after warmup.
struct Pending {
    layout: Option<TraceLayout>,
    verbs: Vec<PathVerb>,
    points: Vec<f32>,
    geometry_dirty: bool,
    fill_ink: Option<Color>,
    fill_verbs: Vec<PathVerb>,
    fill_points: Vec<f32>,
    fill_dirty: bool,
    /// The underfill's colour ramp, and whether it moved. Retained across pushes
    /// so a producer restating one allocates nothing after the first.
    fill_stops: Vec<(f64, Color)>,
    fill_axis: GradientAxis,
    fill_ramp_dirty: bool,
    fill_motion: Option<FillMotion>,
    /// The metered fill's extent, `0.0..=1.0` of its own box.
    fill_extent: Option<f32>,
}

impl Pending {
    const fn new() -> Self {
        Self {
            layout: None,
            verbs: Vec::new(),
            points: Vec::new(),
            geometry_dirty: false,
            fill_ink: None,
            fill_verbs: Vec::new(),
            fill_points: Vec::new(),
            fill_dirty: false,
            fill_stops: Vec::new(),
            fill_axis: GradientAxis::Vertical,
            fill_ramp_dirty: false,
            fill_motion: None,
            fill_extent: None,
        }
    }
}

/// Pending updates per control. A `Mutex` rather than a thread-local for
/// [`bar_field`](super::bar_field)'s reason: publishes originate wherever the
/// producer runs, which is never the thread that services them.
static PENDING: Mutex<Option<HashMap<ControlId, Pending>>> = Mutex::new(None);

/// Whether a service call is already on its way to the front thread. Gates the
/// post so a producer running at publish rate leaves at most one message in
/// flight.
static POSTED: AtomicBool = AtomicBool::new(false);

/// A handle to one retained trace, writable from any thread.
///
/// Obtained from a mounted element. Cheap to `Copy` and `Send`, because it holds
/// no COM: the control id names a node the front thread owns. A handle outliving
/// its control is harmless — the update is dropped when the id no longer resolves.
#[derive(Clone, Copy, Debug)]
pub struct LiveTrace {
    id: ControlId,
}

impl LiveTrace {
    pub(crate) fn new(id: ControlId) -> Self {
        Self { id }
    }

    /// State (or restate) the box, the ink and the width.
    ///
    /// Cheap to call redundantly — the front thread compares the layout by value
    /// and rebinds nothing when it matches.
    pub fn set_layout(&self, layout: &TraceLayout) {
        self.enqueue(|p| p.layout = Some(*layout));
    }

    /// Push one frame of geometry, in the host element's own DIPs. Every figure in
    /// `path` is stroked with the layout's one ink and width.
    ///
    /// Allocation-free after the first call: the pending buffers are reused.
    pub fn set_path(&self, path: &TracePath) {
        self.enqueue(|p| {
            p.verbs.clear();
            p.verbs.extend_from_slice(&path.verbs);
            p.points.clear();
            p.points.extend_from_slice(&path.points);
            p.geometry_dirty = true;
        });
    }

    /// Push one frame of the filled companion's geometry, in the same DIPs
    /// [`set_path`](Self::set_path) uses, washed in `ink`. Every figure is closed
    /// and filled; the first such push is what builds the fill layer at all.
    ///
    /// Allocation-free after the first call: the pending buffers are reused.
    pub fn set_fill_path(&self, ink: Color, path: &TracePath) {
        self.enqueue(|p| {
            p.fill_ink = Some(ink);
            p.fill_verbs.clear();
            p.fill_verbs.extend_from_slice(&path.verbs);
            p.fill_points.clear();
            p.fill_points.extend_from_slice(&path.points);
            p.fill_dirty = true;
        });
    }

    /// Give the underfill a colour RAMP instead of the flat ink
    /// [`set_fill_path`](Self::set_fill_path) carries. Empty `stops` restores the
    /// flat fill.
    ///
    /// The ramp is measured across the fill sprite's own box — so under a metered
    /// fill ([`set_fill_extent`](Self::set_fill_extent)) it rides the level and is
    /// fill-relative, and under a free-form area plot it spans the plot. Pushed on
    /// a theme flip or a threshold recolour, never per publish.
    pub fn set_fill_ramp(&self, stops: &[(f64, Color)], axis: GradientAxis) {
        self.enqueue(|p| {
            p.fill_stops.clear();
            p.fill_stops.extend_from_slice(stops);
            p.fill_axis = axis;
            p.fill_ramp_dirty = true;
        });
    }

    /// Declare the underfill a METER: which edge it grows from and how fast it
    /// travels each way. Push before the first [`set_fill_extent`](Self::set_fill_extent);
    /// restating it is one comparison.
    pub fn set_fill_motion(&self, motion: FillMotion) {
        self.enqueue(|p| p.fill_motion = Some(motion));
    }

    /// Retarget the metered fill to `frac` of its own box (`0.0..=1.0`).
    ///
    /// The geometry is whatever [`set_fill_path`](Self::set_fill_path) last stated —
    /// the WHOLE track, pushed once — so this moves one scalar. Under
    /// [`LiveAnim::Dwm`] that is one `InsertKeyFrame` and one `StartAnimation` on
    /// a cached animation, and the level then travels on the compositor with no
    /// further app frame: a publish that lands mid-flight redirects it, and a
    /// publish stream that stops lets it finish. Under [`LiveAnim::Front`] the
    /// level advances one one-pole step per publish instead, so a stream that
    /// stops leaves it where it stood.
    pub fn set_fill_extent(&self, frac: f32) {
        self.enqueue(|p| p.fill_extent = Some(frac));
    }

    fn enqueue(&self, edit: impl FnOnce(&mut Pending)) {
        {
            let Ok(mut q) = PENDING.lock() else { return };
            let map = q.get_or_insert_with(HashMap::new);
            let entry = map.entry(self.id).or_insert_with(Pending::new);
            edit(entry);
        }
        // One wake in flight, and the claim is the WHOLE gate — see the same
        // reasoning (and the bug that motivated it) in `bar_field::enqueue`. This
        // map retains its entries so their buffers keep their capacity, so its
        // size says nothing about whether work is pending; `service_live_traces`
        // retires ids the arena no longer resolves (see [`forget`]), which is what
        // keeps it bounded.
        if !POSTED.swap(true, Ordering::AcqRel) {
            let hwnd = super::live_text::front_hwnd();
            if hwnd != 0 {
                super::host::post_ui(hwnd, || {
                    if let Some(s) = super::host::shared() {
                        s.backend.borrow_mut().service_live_traces();
                    }
                });
            } else {
                POSTED.store(false, Ordering::Release);
            }
        }
    }
}

/// One drained trace: its id, any new layout, and the geometry buffers swapped out
/// of the queue. The front thread owns a `Vec` of these across services so both it
/// and the producer keep their allocations.
pub(crate) struct TraceBatch {
    pub id: ControlId,
    pub layout: Option<TraceLayout>,
    pub verbs: Vec<PathVerb>,
    pub points: Vec<f32>,
    /// Whether the buffers are this service's frame rather than the previous one's
    /// leftovers — a layout-only push carries no geometry.
    pub has_geometry: bool,
    pub fill_ink: Option<Color>,
    pub fill_verbs: Vec<PathVerb>,
    pub fill_points: Vec<f32>,
    /// [`has_geometry`](Self::has_geometry) for the filled companion. Separate
    /// because the two are pushed independently — a caller may reshape the line
    /// alone.
    pub has_fill: bool,
    pub fill_stops: Vec<(f64, Color)>,
    pub fill_axis: GradientAxis,
    pub has_fill_ramp: bool,
    pub fill_motion: Option<FillMotion>,
    pub fill_extent: Option<f32>,
}

impl TraceBatch {
    fn new(id: ControlId) -> Self {
        Self {
            id,
            layout: None,
            verbs: Vec::new(),
            points: Vec::new(),
            has_geometry: false,
            fill_ink: None,
            fill_verbs: Vec::new(),
            fill_points: Vec::new(),
            has_fill: false,
            fill_stops: Vec::new(),
            fill_axis: GradientAxis::Vertical,
            has_fill_ramp: false,
            fill_motion: None,
            fill_extent: None,
        }
    }
}

/// Drop a control's queue entry — the front thread found no node behind the id, so
/// nothing will ever consume its updates again.
pub(crate) fn forget(id: ControlId) {
    if let Ok(mut q) = PENDING.lock()
        && let Some(map) = q.as_mut()
    {
        map.remove(&id);
    }
}

/// Move the pending updates into `out` for the front thread to apply, and release
/// the wake claim so the next publish posts again.
///
/// The claim is released *before* the caller applies the batch, so a publish
/// landing during the apply schedules another service rather than being folded
/// into one already in progress and missed.
pub(crate) fn drain_into(out: &mut Vec<TraceBatch>) {
    POSTED.store(false, Ordering::Release);
    for e in out.iter_mut() {
        e.layout = None;
        e.has_geometry = false;
        e.has_fill = false;
        e.has_fill_ramp = false;
        e.fill_motion = None;
        e.fill_extent = None;
    }
    let Ok(mut q) = PENDING.lock() else { return };
    let Some(map) = q.as_mut() else { return };
    for (id, p) in map.iter_mut() {
        let slot = match out.iter_mut().position(|e| e.id == *id) {
            Some(i) => &mut out[i],
            None => {
                out.push(TraceBatch::new(*id));
                out.last_mut().expect("just pushed")
            }
        };
        slot.layout = p.layout.take();
        if p.geometry_dirty {
            std::mem::swap(&mut slot.verbs, &mut p.verbs);
            std::mem::swap(&mut slot.points, &mut p.points);
            p.geometry_dirty = false;
            slot.has_geometry = true;
        }
        if p.fill_dirty {
            slot.fill_ink = p.fill_ink;
            std::mem::swap(&mut slot.fill_verbs, &mut p.fill_verbs);
            std::mem::swap(&mut slot.fill_points, &mut p.fill_points);
            p.fill_dirty = false;
            slot.has_fill = true;
        }
        if p.fill_ramp_dirty {
            std::mem::swap(&mut slot.fill_stops, &mut p.fill_stops);
            // The producer's buffer keeps the previous frame's stops, so it is
            // cleared rather than left describing a ramp it no longer owns.
            p.fill_stops.clear();
            slot.fill_axis = p.fill_axis;
            p.fill_ramp_dirty = false;
            slot.has_fill_ramp = true;
        }
        slot.fill_motion = p.fill_motion.take();
        slot.fill_extent = p.fill_extent.take();
    }
}

// ── The front-thread trace ───────────────────────────────────────────────────

/// Extent movement below this is not worth a retarget — well under a pixel on any
/// meter this drives.
const EXTENT_EPS: f32 = 0.0005;

/// A node's retained trace: one stroked mask layer over an FP16 colour source,
/// and — when the layout names a fill — the filled companion beneath it.
///
/// No glow: the halo on an analyzer belongs to the modelled response, which is
/// not the thing moving.
pub(crate) struct LiveTraceField {
    /// The filled companion. Declared FIRST so that when a publish carrying both
    /// geometries builds both layers, the fill's sprite is parented before the
    /// stroke's and the line composites over its own wash.
    fill: Option<TraceLayer>,
    /// The wash's ink, carried by whichever push last brought fill geometry.
    fill_ink: Option<Color>,
    /// The wash's ramp, when it has one. Empty means the flat `fill_ink`.
    fill_stops: Vec<(f64, Color)>,
    fill_axis: GradientAxis,
    /// The metered fill's ballistics and pivot, once a producer declares them.
    motion: Option<FillMotion>,
    /// The extent the sprite was last STATED at, and the pivot it is scaled
    /// about — so a publish that changes neither costs nothing. Under
    /// [`LiveAnim::Front`] the extent is the level on screen; under
    /// [`LiveAnim::Dwm`] it is the last retarget's destination, which the
    /// compositor may still be carrying the sprite towards.
    extent: Option<f32>,
    pivot: Option<f32>,
    /// The level last PUBLISHED, held separately from [`extent`](Self::extent)
    /// because the two paths consume it differently: the DWM path hands it over
    /// once and is done, while the front path has to keep stepping towards it
    /// on publishes that carry only geometry.
    target: Option<f32>,
    /// When the front path last stepped, so it integrates real elapsed time
    /// rather than assuming a cadence. Untouched by the DWM path, which keeps
    /// no clock of its own.
    last_step: Option<Instant>,
    /// The two cached retarget animations and their easing, built on first use.
    /// One per direction, because their durations differ.
    rise: Option<ScalarKeyFrameAnimation>,
    fall: Option<ScalarKeyFrameAnimation>,
    easing: Option<CompositionEasingFunction>,
    layer: Option<TraceLayer>,
    /// The layout the layers were bound for; `None` until the first push.
    layout: Option<TraceLayout>,
    /// Whether each sprite currently has geometry to show. A trace whose every
    /// run was gated away hides rather than lingering as an empty shape.
    visible: bool,
    fill_visible: bool,
}

impl LiveTraceField {
    pub(crate) fn new() -> Self {
        Self {
            fill: None,
            fill_ink: None,
            fill_stops: Vec::new(),
            fill_axis: GradientAxis::Vertical,
            motion: None,
            extent: None,
            pivot: None,
            target: None,
            last_step: None,
            rise: None,
            fall: None,
            easing: None,
            layer: None,
            layout: None,
            visible: true,
            fill_visible: true,
        }
    }

    /// Reconcile the trace against one drained batch. Everything self-gates: a
    /// service that finds the layout unchanged and no new geometry issues no COM
    /// calls at all.
    pub(crate) fn sync(
        &mut self,
        comp: &Compositing,
        container: &windows_composition::ContainerVisual,
        batch: &TraceBatch,
        atlas_epoch: u32,
        scale: f32,
    ) {
        if let Some(l) = &batch.layout {
            self.layout = Some(*l);
        }
        let Some(layout) = self.layout else { return };

        // A flat wash needs no capture; a ramped one has nowhere else to put its
        // staircase. A trace that gains or loses its ramp therefore rebuilds the
        // fill rather than trying to re-role a layer built the other way.
        let flat_fill = self.fill_stops.is_empty() && !batch.has_fill_ramp;
        if self.fill.as_ref().is_some_and(|f| f.is_clipped() != flat_fill) {
            if let Some(f) = self.fill.take() {
                f.display().set_visible(false);
            }
            self.fill_visible = true;
        }

        // The fill first, so a first publish carrying both parents it below the
        // line (both layers insert at the top of the container's children).
        if batch.has_fill {
            self.fill_ink = batch.fill_ink;
            Self::reshape(
                comp,
                container,
                &mut self.fill,
                &mut self.fill_visible,
                &batch.fill_verbs,
                &batch.fill_points,
                Role::Fill,
                flat_fill.then_some(0.0),
            );
        }
        if batch.has_geometry {
            // The line is flat by construction (see its `set_source` below), so
            // it is always the capture-free layer — widened to `thickness`,
            // which is what makes its outline a region a clip can take.
            Self::reshape(
                comp,
                container,
                &mut self.layer,
                &mut self.visible,
                &batch.verbs,
                &batch.points,
                Role::Stroke,
                Some(layout.thickness),
            );
        }

        if batch.has_fill_ramp {
            self.fill_stops.clear();
            self.fill_stops.extend_from_slice(&batch.fill_stops);
            self.fill_axis = batch.fill_axis;
        }
        if let Some(m) = batch.fill_motion {
            self.motion = Some(m);
        }

        if let (Some(fill), Some(ink)) = (self.fill.as_mut(), self.fill_ink) {
            fill.resize(layout.width, layout.height, scale);
            match fill {
                TraceLayer::Clipped(l) => l.set_source(comp, ink, scale),
                TraceLayer::Masked(l) => {
                    l.set_source(comp, ink, &self.fill_stops, self.fill_axis, atlas_epoch, scale)
                }
            }
        }
        self.meter(layout, batch.fill_extent);
        let Some(layer) = self.layer.as_mut() else { return };
        layer.resize(layout.width, layout.height, scale);
        // A flat colour: the source is a solid FP16 raster, exactly as a bar
        // body's is. The thickness is not set here — it was baked into the
        // widened outline the clip takes its shape from.
        match layer {
            TraceLayer::Clipped(l) => l.set_source(comp, layout.color, scale),
            TraceLayer::Masked(l) => {
                l.set_source(comp, layout.color, &[], GradientAxis::Vertical, atlas_epoch, scale)
            }
        }
    }

    /// Move the metered fill towards this publish's extent.
    ///
    /// The geometry does not move: the fill sprite spans the whole track and its
    /// `Scale.X` about the anchored edge IS the level. The colour source is
    /// composited into that same sprite, which is what makes a ramp under a
    /// meter fill-relative.
    ///
    /// Which mechanism carries the level is [`live_anim`]'s choice, and the two
    /// differ in more than who interpolates. The DWM path acts only on a publish
    /// that CARRIES a level — one retarget hands over the whole flight, and a
    /// publish carrying only geometry has nothing to say about it. The front
    /// path has to step on every publish, because a step is the only thing that
    /// advances it and a producer reshaping a trace without restating its level
    /// would otherwise freeze the meter mid-travel.
    fn meter(&mut self, layout: TraceLayout, extent: Option<f32>) {
        let Some(motion) = self.motion else { return };
        let Some(fill) = self.fill.as_ref() else { return };
        let sprite = fill.display();

        // The pivot is the anchored edge, in the sprite's own DIPs. It moves only
        // with the box.
        let pivot = match motion.anchor {
            FillAnchor::Left => 0.0,
            FillAnchor::Right => layout.width,
        };
        if self.pivot != Some(pivot) {
            sprite.set_center_point(Vector3::new(pivot, 0.0, 0.0));
            self.pivot = Some(pivot);
        }

        if let Some(raw) = extent {
            self.target = Some(if raw.is_finite() { raw.clamp(0.0, 1.0) } else { 0.0 });
        }
        let Some(target) = self.target else { return };
        let Some(shown) = self.extent else {
            // The opening frame is where the meter starts, not a change. The
            // sprite is born at full extent, so easing down from it would read as
            // a level that was there and fell.
            sprite.stop_animation("Scale.X");
            sprite.set_scale(Vector3::new(target, 1.0, 1.0));
            note_property_writes(1);
            self.extent = Some(target);
            self.last_step = Some(Instant::now());
            return;
        };

        match live_anim() {
            LiveAnim::Front => {
                let now = Instant::now();
                let dt = self
                    .last_step
                    .map_or(0.0, |t| now.duration_since(t).as_secs_f32())
                    .min(MAX_STEP_SECS);
                self.last_step = Some(now);
                // The direction of travel picks the time constant, and here it
                // is measured against the level ON SCREEN — the front path knows
                // it, because it is the one that put it there.
                let k = closing(dt, if target > shown { motion.rise } else { motion.fall });
                let t = shown + (target - shown) * k;
                if (t - shown).abs() < EXTENT_EPS {
                    return;
                }
                sprite.set_scale(Vector3::new(t, 1.0, 1.0));
                note_property_writes(1);
                self.extent = Some(t);
            }
            LiveAnim::Dwm => {
                // Nothing to hand over on a publish that restated no level: the
                // flight already in the air is still the right one.
                if extent.is_none() || (target - shown).abs() < EXTENT_EPS {
                    return;
                }
                let compositor = sprite.compositor();
                let easing = self
                    .easing
                    .get_or_insert_with(|| retarget_easing(&compositor));
                let a = if target > shown {
                    let a = self
                        .rise
                        .get_or_insert_with(|| compositor.create_scalar_key_frame_animation());
                    a.set_duration(motion.rise);
                    &*a
                } else {
                    let a = self
                        .fall
                        .get_or_insert_with(|| compositor.create_scalar_key_frame_animation());
                    a.set_duration(motion.fall);
                    &*a
                };
                a.insert_key_frame_with_easing(1.0, target, easing);
                sprite.start_animation("Scale.X", a);
                note_anim_start();
                self.extent = Some(target);
            }
        }
    }

    /// Re-point one layer at this publish's geometry, building it on first use and
    /// hiding it when the geometry empties. Shared by the stroke and its fill so
    /// the two cannot drift in how an emptied frame is handled.
    #[allow(clippy::too_many_arguments)]
    fn reshape(
        comp: &Compositing,
        container: &windows_composition::ContainerVisual,
        slot: &mut Option<TraceLayer>,
        visible: &mut bool,
        verbs: &[PathVerb],
        points: &[f32],
        role: Role,
        // `Some(width)` asks for the capture-free construction. A stroke must
        // arrive WIDENED — a clip fills a region, and a stroke is a line plus a
        // width until something turns it into one.
        clipped: Option<f32>,
    ) {
        let empty = verbs.is_empty();
        // A path is minted only for geometry there is something to draw; an
        // emptied trace hides the sprite it already has.
        if !empty
            && let Some(path) = match (clipped, role) {
                (Some(width), Role::Stroke) => {
                    super::path_shape::build_widened_path(comp, verbs, points, width)
                }
                _ => super::path_shape::build_composition_path(
                    comp,
                    verbs,
                    points,
                    role == Role::Fill,
                ),
            }
        {
            match slot.as_mut() {
                Some(l) => l.set_path(&path),
                None => {
                    *slot = Some(match clipped {
                        Some(_) => TraceLayer::Clipped(ClipLayer::new(comp, container, &path)),
                        None => TraceLayer::Masked(PathLayer::new(comp, container, &path, role)),
                    })
                }
            }
        }
        if let Some(l) = slot.as_ref()
            && *visible == empty
        {
            l.display().set_visible(!empty);
            *visible = !empty;
        }
    }
}
