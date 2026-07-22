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

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use super::bootstrap::Compositing;
use super::path_shape::{PathLayer, Role};
use crate::backend::ControlId;
use crate::{Color, GradientAxis, PathVerb};

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

// ── The cross-thread queue ───────────────────────────────────────────────────

/// One trace's pending update. The geometry buffers are retained (and swapped,
/// never cloned, on drain) so neither side allocates after warmup.
struct Pending {
    layout: Option<TraceLayout>,
    verbs: Vec<PathVerb>,
    points: Vec<f32>,
    geometry_dirty: bool,
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

    fn enqueue(&self, edit: impl FnOnce(&mut Pending)) {
        {
            let Ok(mut q) = PENDING.lock() else { return };
            let map = q.get_or_insert_with(HashMap::new);
            let entry = map.entry(self.id).or_insert_with(|| Pending {
                layout: None,
                verbs: Vec::new(),
                points: Vec::new(),
                geometry_dirty: false,
            });
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
    }
    let Ok(mut q) = PENDING.lock() else { return };
    let Some(map) = q.as_mut() else { return };
    for (id, p) in map.iter_mut() {
        let slot = match out.iter_mut().position(|e| e.id == *id) {
            Some(i) => &mut out[i],
            None => {
                out.push(TraceBatch {
                    id: *id,
                    layout: None,
                    verbs: Vec::new(),
                    points: Vec::new(),
                    has_geometry: false,
                });
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
    }
}

// ── The front-thread trace ───────────────────────────────────────────────────

/// A node's retained trace: one stroked mask layer over an FP16 colour source.
///
/// No glow and no fill. This draws a plain hairline — the halo on an analyzer
/// belongs to the modelled response, which is not the thing moving.
pub(crate) struct LiveTraceField {
    layer: Option<PathLayer>,
    /// The layout the layer was bound for; `None` until the first push.
    layout: Option<TraceLayout>,
    /// Whether the sprite currently has geometry to show. A trace whose every run
    /// was gated away hides rather than lingering as an empty shape.
    visible: bool,
}

impl LiveTraceField {
    pub(crate) fn new() -> Self {
        Self { layer: None, layout: None, visible: true }
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

        if batch.has_geometry {
            let empty = batch.verbs.is_empty();
            // A path is minted only for geometry there is something to draw; an
            // emptied trace hides the sprite it already has.
            if !empty
                && let Some(path) =
                    super::path_shape::build_composition_path(comp, &batch.verbs, &batch.points, false)
            {
                match &mut self.layer {
                    Some(l) => l.set_path(&path),
                    slot => *slot = Some(PathLayer::new(comp, container, &path, Role::Stroke)),
                }
            }
            if let Some(l) = &self.layer
                && self.visible == empty
            {
                l.display().set_visible(!empty);
                self.visible = !empty;
            }
        }

        let Some(layer) = self.layer.as_mut() else { return };
        layer.resize(layout.width, layout.height, scale);
        layer.set_thickness(layout.thickness);
        // A flat colour, so the axis is inert — the source is a solid FP16 raster
        // stretched under the mask, exactly as a bar body's is.
        layer.set_source(comp, layout.color, &[], GradientAxis::Vertical, atlas_epoch, scale);
    }
}
