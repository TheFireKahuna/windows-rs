//! A field of retained bars a producer thread drives without a reconcile —
//! the spectrum analyzer, as compositor sprites instead of a repainted surface.
//!
//! An analyzer's bars are the worst possible fit for a drawing surface. They
//! change every audio publish and read as motion the whole time, so a surface
//! carrying them is re-rasterized on the UI/pump thread inside DirectComposition's
//! commit for every display frame the window is up — measured, that commit
//! (`CBitmapInfoFront::CommitUpdate` → `CAtlasSurfacePool::D2DEndDraw` →
//! `d2d1!EndDraw`) is the single largest cost an otherwise idle window pays,
//! along with the atlas churn and the kernel composition token each present
//! takes. A controlled A/B of the same animated content — retained shape vs.
//! repainted surface, both paced on the compositor clock — measured 4.14% of a
//! core against 17.03%, with every one of those stack frames *absent* from the
//! retained profile rather than merely smaller.
//!
//! So the bars become what the knob's value arc and the curve layers already
//! are: retained sprites the compositor owns, coloured by an FP16 source and
//! moved by property writes. Between publishes the app does nothing at all.
//!
//! ## The seam
//!
//! Exactly [`live_text`](super::live_text)'s: the visual tree is thread-affine,
//! so the producer is handed nothing but a control id. Values are queued from
//! whatever thread computed them, coalesced per control (a producer that
//! outruns the front thread overwrites its own pending frame — the queue holds
//! one entry per field however fast it publishes), and applied on the front
//! thread as property sets on sprites that already exist.
//!
//! ## What one publish costs
//!
//! Two property writes per bar — the body's extent and the cap that rides its
//! top edge. No visual, brush or surface is created per publish: those are
//! built once per *layout*, and a layout changes only on a resize, a DPI move,
//! a bar-count change or a recolour. There is no per-publish allocation on
//! either side of the queue: the pending value buffer is swapped with the front
//! thread's, so both keep their capacity.
//!
//! ## How a bar moves
//!
//! A bar is two sprites — a body and the brighter cap that gives it a defined
//! top edge — and each is one scalar. The body is a full-height sprite whose
//! `CenterPoint` sits on its bottom edge, so its whole extent is `Scale.Y` in
//! `0..=1` and no offset has to move with it. The cap is a fixed-height strip
//! whose `Offset.Y` is the body's top edge: `offset = top + (1 - scale) *
//! height`, one number seen two ways.
//!
//! ## Ballistics — stepped here, or scheduled DWM-side
//!
//! An analyzer that rises and falls at the same rate reads as jitter, so the
//! caller states an asymmetric pair ([`BarFieldLayout::rise`] /
//! [`BarFieldLayout::fall`]) and each bar picks one per push from its own
//! direction of travel. WHO evaluates that curve between two published values
//! is a process-wide choice ([`live_anim`]):
//!
//! - [`LiveAnim::Front`] — the envelope is a one-pole stepped **on this
//!   thread**, from the wall clock, as each push is applied. Two property
//!   writes per moved bar, both derived from the same stepped value in the same
//!   commit, which is what pins the cap to the body.
//! - [`LiveAnim::Dwm`] — each push retargets a key-frame animation on the
//!   body's `Scale.Y` and the compositor flies it there. The cap is not
//!   animated at all: an expression derives its `Offset.Y` from the body's
//!   *live* `Scale.Y` (see [`bind_cap`]), so the weld is a definition the
//!   compositor evaluates rather than two curves that have to agree.
//!
//! Both land in the same place at rest and both leave the sprites retained —
//! the rasterization this module exists to avoid is untouched either way. They
//! differ only in who interpolates, which is exactly what the switch is for:
//! one run can be measured against another without a rebuild.
//!
//! The scheduled path was once removed outright. Measured with a frame-differ
//! against a live analyzer, a field driven that way appeared to animate for
//! ≈50 seconds after its sprites were created and then stop for good, while
//! direct property writes on the very same sprites never stopped. That reading
//! is not safe: a frame-differ cannot tell "DWM stopped evaluating the
//! animation" from "the composition clock stopped", which is what a locked
//! session or a powered-down display does — and a value stepped in-process
//! keeps changing captured frames through both. So the path is back, behind the
//! switch, alongside counters ([`live_anim_starts`], [`live_anim_property_writes`])
//! that say what each path actually issued, so a repeat of the measurement can
//! tell a stalled compositor from a stalled animation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows_composition::{
    CompositionEasingFunction, CompositionSurfaceBrush, Compositor, ScalarKeyFrameAnimation,
    SpriteVisual,
};
use windows_numerics::{Vector2, Vector3};

use super::bootstrap::Compositing;
use crate::backend::ControlId;
use crate::Color;

// ── Who interpolates, and what each path costs ───────────────────────────────

/// Which mechanism carries a live field between two published values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveAnim {
    /// Stepped on the front thread as each push is applied, and written
    /// straight to the sprite's properties.
    Front,
    /// Retargeted onto a compositor animation, and evaluated by DWM.
    Dwm,
}

/// The mechanism this process uses, read ONCE from `REACTOR_LIVE_ANIM`
/// (`dwm` selects [`LiveAnim::Dwm`]; anything else, including an unset
/// variable, selects [`LiveAnim::Front`]).
///
/// Process-wide and fixed for the run rather than per field: the two paths
/// animate different properties and write different ones, so a field cannot
/// change mechanism without rebuilding its bindings, and no caller wants two
/// analyzers in one window moving by different means. It is consulted once per
/// publish, which is why the environment is read behind a `OnceLock` rather
/// than each time.
#[must_use]
pub fn live_anim() -> LiveAnim {
    static MODE: OnceLock<LiveAnim> = OnceLock::new();
    *MODE.get_or_init(|| {
        let dwm = std::env::var_os("REACTOR_LIVE_ANIM")
            .as_deref()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|s| s.eq_ignore_ascii_case("dwm"));
        if dwm { LiveAnim::Dwm } else { LiveAnim::Front }
    })
}

// These three are a SCOPED subset of what `windows_composition::census`
// already counts process-wide. The census bumps a property write for every
// `set_*` any subsystem makes and an animation start for every
// `StartAnimation`, which is the authoritative traffic figure — and exactly
// why it cannot answer the question these are for. The live fields' publishes
// are a few thousand calls a second inside a stream that also carries chrome,
// layout and text, so "did the analyzer keep retargeting while the screen
// stopped moving" is not extractable from a total. These count that one
// intent and nothing else; when both are reported, the census is the
// denominator and these are the numerator.
static ANIM_STARTS: AtomicU64 = AtomicU64::new(0);
static PROP_WRITES: AtomicU64 = AtomicU64::new(0);
static EXPR_BINDS: AtomicU64 = AtomicU64::new(0);

/// How many compositor animations the live fields have started: every
/// `StartAnimation` this module and [`live_trace`](super::live_trace) issue to
/// move a published value, whether it begins a flight or redirects one already
/// in the air. Stays at zero under [`LiveAnim::Front`].
///
/// Process-wide and monotonic — read it twice and difference it to get a rate.
/// Together with [`live_anim_property_writes`] it says which mechanism a run
/// actually used, which is what tells a compositor that stopped evaluating
/// apart from one that was never asked to.
#[must_use]
pub fn live_anim_starts() -> u64 {
    ANIM_STARTS.load(Ordering::Relaxed)
}

/// How many property writes the live fields have issued to MOVE a published
/// value — the front path's two per moved bar, and the opening snap either path
/// makes on its first frame.
///
/// Placement writes are deliberately not counted: a resize or a recolour writes
/// every sprite's offset, size and pivot, and folding those in would bury the
/// number this exists to show, which is how much the app does per publish.
/// Under [`LiveAnim::Dwm`] it must stop climbing once every field has taken its
/// first frame — a run where it keeps climbing is not running the DWM path.
#[must_use]
pub fn live_anim_property_writes() -> u64 {
    PROP_WRITES.load(Ordering::Relaxed)
}

/// How many expression animations the live fields have bound to derive one
/// property from another — the DWM path's cap weld and floor gate. Per layout,
/// not per publish, so this settles once a field is up and moves again only on
/// a resize, a DPI change, a bar-count change or a recolour.
#[must_use]
pub fn live_anim_expression_binds() -> u64 {
    EXPR_BINDS.load(Ordering::Relaxed)
}

/// Record one `StartAnimation` — see [`live_anim_starts`].
pub(crate) fn note_anim_start() {
    ANIM_STARTS.fetch_add(1, Ordering::Relaxed);
}

/// Record `n` value-moving property writes — see [`live_anim_property_writes`].
pub(crate) fn note_property_writes(n: u64) {
    PROP_WRITES.fetch_add(n, Ordering::Relaxed);
}

/// Record `n` expression bindings — see [`live_anim_expression_binds`].
pub(crate) fn note_expression_binds(n: u64) {
    EXPR_BINDS.fetch_add(n, Ordering::Relaxed);
}

/// The retarget easing shared by every live field: a decelerating ramp, so a
/// value arrives rather than stopping dead. One definition because a meter and
/// an analyzer bar in the same panel must move alike.
const EASE_C1: (f32, f32) = (0.0, 0.0);
const EASE_C2: (f32, f32) = (0.58, 1.0);

/// Mint that easing. Cached by each field — the curve is fixed, so one object
/// serves every retarget the field ever makes.
pub(crate) fn retarget_easing(compositor: &Compositor) -> CompositionEasingFunction {
    compositor.create_cubic_bezier_easing_function(
        Vector2::new(EASE_C1.0, EASE_C1.1),
        Vector2::new(EASE_C2.0, EASE_C2.1),
    )
}

/// The fraction of the remaining gap a one-pole closes over `dt` seconds for a
/// stated settling `duration`.
///
/// `1 - e^(-Δ/τ)`, with `τ` half that duration (see [`BarFieldLayout::rise`]);
/// a zero or absurd duration degenerates to a snap, which is the honest reading
/// of "no ballistics". Shared with [`live_trace`](super::live_trace) so an
/// analyzer bar and a meter under one pair of durations trace one envelope.
pub(crate) fn closing(dt: f32, duration: Duration) -> f32 {
    let tau = duration.as_secs_f32() * 0.5;
    if tau <= 0.0 || dt <= 0.0 {
        return 1.0;
    }
    (1.0 - (-dt / tau).exp()).clamp(0.0, 1.0)
}

/// One bar's horizontal placement within the field's host element, in DIPs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarRect {
    /// Left edge, relative to the host element's own origin.
    pub x: f32,
    /// Width. A bar narrower than a pixel still renders (antialiased); a bar of
    /// zero or negative width is skipped.
    pub w: f32,
}

/// Everything about a bar field except the values: the geometry, the colours
/// and the motion. Pushed only when one of them actually changes — a resize, a
/// DPI move, a different bar count, a theme flip.
///
/// It is compared by value before anything is rebuilt, so a producer that
/// re-pushes an identical layout costs one comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct BarFieldLayout {
    /// One entry per bar, in draw order. Its length IS the bar count.
    pub bars: Vec<BarRect>,
    /// Top of the plot band, in the host element's DIPs — where a full-scale
    /// bar's top edge lands.
    pub top: f32,
    /// Height of the plot band, in DIPs. A bar's value scales this.
    pub height: f32,
    /// The bar body's colour at its own TOP edge, and at its own bottom.
    ///
    /// Linear scRGB like every other reactor colour, and rasterized into the same
    /// FP16 display-mapped source every other sprite takes — a bar and a curve
    /// stroke authored from one token land on one colour.
    ///
    /// The ramp runs down the body sprite, whose extent IS the bar: the sprite
    /// spans the whole plot band and `Scale.Y` squashes it (and the raster
    /// stretched over it) onto the bar's own height. So the fade is anchored to
    /// each bar's top edge and rides it up and down, which is the per-bar fade the
    /// mockup draws and the one thing a single gradient anchored in the plot could
    /// not express.
    pub body_top: Color,
    pub body_bottom: Color,
    /// The brighter cap that gives each bar its top edge.
    pub cap: Color,
    /// Cap thickness, in DIPs. It overlays the top of the body rather than
    /// sitting above it.
    pub cap_h: f32,
    /// How long a bar takes to reach a target ABOVE where it is — the attack.
    ///
    /// The envelope is a one-pole, `1 - e^(-Δ/τ)` per push against the wall
    /// clock, and this duration is `2τ` — the settling time an ease-out of the
    /// same feel would be given, which is the unit every caller already passes.
    /// Stating it as a duration rather than a bare time constant also keeps the
    /// asymmetry readable at the call site: `rise: 24ms, fall: 100ms` says what
    /// an analyzer does; two time constants do not.
    pub rise: Duration,
    /// The same, for a target BELOW where the bar is — the release. Longer than
    /// [`rise`](Self::rise) in every analyzer worth looking at.
    pub fall: Duration,
}

// ── The cross-thread queue ───────────────────────────────────────────────────

/// One field's pending update. The value buffer is retained (and swapped, never
/// cloned, on drain) so neither side allocates after warmup.
struct Pending {
    layout: Option<BarFieldLayout>,
    values: Vec<f32>,
    values_dirty: bool,
}

/// Pending updates per control. A `Mutex` rather than a thread-local for
/// [`live_text`](super::live_text)'s reason: publishes originate wherever the
/// producer runs, which is never the thread that services them.
static PENDING: Mutex<Option<HashMap<ControlId, Pending>>> = Mutex::new(None);

/// Whether a service call is already on its way to the front thread. Gates the
/// post so a producer running at publish rate leaves at most one message in
/// flight.
static POSTED: AtomicBool = AtomicBool::new(false);

/// A handle to one bar field, writable from any thread.
///
/// Obtained from a mounted element. Cheap to `Copy` and `Send`, because it
/// holds no COM: the control id names a node the front thread owns, and nothing
/// here can touch that node directly. A handle outliving its control is
/// harmless — the update is dropped when the id no longer resolves.
#[derive(Clone, Copy, Debug)]
pub struct LiveBars {
    id: ControlId,
}

impl LiveBars {
    pub(crate) fn new(id: ControlId) -> Self {
        Self { id }
    }

    /// State (or restate) the field's geometry, colours and ballistics.
    ///
    /// Cheap to call redundantly — the front thread compares the layout by
    /// value and rebuilds nothing when it matches — but it is the one call here
    /// that allocates, so a producer that can tell its layout has not moved
    /// should skip it.
    pub fn set_layout(&self, layout: &BarFieldLayout) {
        self.enqueue(|p| p.layout = Some(layout.clone()));
    }

    /// Push one frame of bar values, each the bar's height as a fraction of the
    /// plot band (`0.0` = floor, `1.0` = full scale). Values outside that range
    /// are clamped; a bar with no value holds where it is.
    ///
    /// Allocation-free after the first call: the pending buffer is reused.
    pub fn set_values(&self, values: &[f32]) {
        self.enqueue(|p| {
            p.values.clear();
            p.values.extend_from_slice(values);
            p.values_dirty = true;
        });
    }

    fn enqueue(&self, edit: impl FnOnce(&mut Pending)) {
        {
            let Ok(mut q) = PENDING.lock() else { return };
            let map = q.get_or_insert_with(HashMap::new);
            let entry = map.entry(self.id).or_insert_with(|| Pending {
                layout: None,
                values: Vec::new(),
                values_dirty: false,
            });
            edit(entry);
        }
        // One wake in flight, and the claim is the WHOLE gate.
        //
        // `live_text` also requires the batch to have been empty, which it can
        // do because it `drain`s its map — an empty map there means no pending
        // work. This map is not drained: entries are kept so their value buffers
        // keep their capacity, so "the map has one entry" says nothing about
        // whether anything is pending. Reading it as though it did was a real
        // bug with a nasty shape: a field that unmounted and remounted (a mode
        // switch, a re-keyed host) left its dead id in the map forever, every
        // later publish found a second entry, and NO wake was ever posted again
        // — the analyzer simply stopped, with the queue filling correctly behind
        // it. `service_live_bars` retires ids the arena no longer resolves (see
        // [`forget`]), so the map still cannot grow without bound; the wake no
        // longer depends on that being true.
        if !POSTED.swap(true, Ordering::AcqRel) {
            let hwnd = super::live_text::front_hwnd();
            if hwnd != 0 {
                super::host::post_ui(hwnd, || {
                    if let Some(s) = super::host::shared() {
                        s.backend.borrow_mut().service_live_bars();
                    }
                });
            } else {
                POSTED.store(false, Ordering::Release);
            }
        }
    }
}

/// One drained field: its id, any new layout, and the value buffer swapped out
/// of the queue. The front thread owns a `Vec` of these across services so both
/// it and the producer keep their allocations.
pub(crate) struct BarBatch {
    pub id: ControlId,
    pub layout: Option<BarFieldLayout>,
    pub values: Vec<f32>,
    /// Whether `values` is this service's frame rather than the previous one's
    /// leftovers — a layout-only push carries no values.
    pub has_values: bool,
}

/// Drop a control's queue entry — the front thread found no node behind the id,
/// so nothing will ever consume its updates again.
///
/// Called from the service for every batch entry that fails to resolve. Without
/// it a remounting field would leak one entry (and its value buffer) per mount.
pub(crate) fn forget(id: ControlId) {
    if let Ok(mut q) = PENDING.lock()
        && let Some(map) = q.as_mut()
    {
        map.remove(&id);
    }
}

/// Move the pending updates into `out` for the front thread to apply, and
/// release the wake claim so the next publish posts again.
///
/// The claim is released *before* the caller applies the batch, so a publish
/// landing during the apply schedules another service rather than being folded
/// into one already in progress and missed.
///
/// Entries in `out` are matched by id and their value buffers **swapped** with
/// the queue's, so the producer gets a buffer with capacity back and neither
/// side allocates per publish.
pub(crate) fn drain_into(out: &mut Vec<BarBatch>) {
    POSTED.store(false, Ordering::Release);
    for e in out.iter_mut() {
        e.layout = None;
        e.has_values = false;
    }
    let Ok(mut q) = PENDING.lock() else { return };
    let Some(map) = q.as_mut() else { return };
    for (id, p) in map.iter_mut() {
        let slot = match out.iter_mut().position(|e| e.id == *id) {
            Some(i) => &mut out[i],
            None => {
                out.push(BarBatch {
                    id: *id,
                    layout: None,
                    values: Vec::new(),
                    has_values: false,
                });
                out.last_mut().expect("just pushed")
            }
        };
        slot.layout = p.layout.take();
        if p.values_dirty {
            std::mem::swap(&mut slot.values, &mut p.values);
            p.values_dirty = false;
            slot.has_values = true;
        }
    }
}

// ── The field the front thread owns ──────────────────────────────────────────

/// A value change smaller than this is not worth a retarget: at a 200 DIP plot
/// it is a twentieth of a pixel, and skipping it is what makes a silent
/// analyzer cost nothing per publish.
const CHANGE_EPS: f32 = 0.0005;

/// Below this a bar is at the floor and its cap is hidden, so a silent field is
/// EMPTY rather than a hairline along the bottom edge — matching the painted
/// analyzer, which drew no bar whose top had reached the baseline.
const FLOOR_EPS: f32 = 0.0005;

/// Longest step the envelope will honour, in seconds. A push that arrives after
/// a stall (the producer blocked, the window was minimized) must not be
/// integrated as one enormous `Δ` — clamping it makes the bar arrive at its
/// target in one frame, which is what a resumed analyzer should look like,
/// instead of some interpolated state that was never true.
pub(crate) const MAX_STEP_SECS: f32 = 0.25;

/// Where a bar's cap sits for value `t` — the body's top edge, which is
/// `top + height` at the floor and `top` at full scale. The body expresses the
/// same edge as `Scale.Y` about its bottom, so the two are one number seen two
/// ways, and every writer here derives both from this.
fn cap_y(layout: &BarFieldLayout, t: f32) -> f32 {
    layout.top + (1.0 - t) * layout.height.max(0.0)
}

/// One bar: the body whose `Scale.Y` is the value, and the cap whose `Offset.Y`
/// is the body's top edge.
struct Bar {
    body: SpriteVisual,
    cap: SpriteVisual,
    /// The value this bar was last STATED at — the write gate, and the direction
    /// of travel the next push picks its time constant from.
    ///
    /// Under [`LiveAnim::Front`] that is also the value on screen, because the
    /// front thread wrote it. Under [`LiveAnim::Dwm`] it is the last retarget's
    /// destination, and the value on screen is wherever the compositor has
    /// carried it to — which cannot be read back, so a push landing mid-flight
    /// picks its direction against the previous TARGET rather than against the
    /// drawn value. The two disagree only on a reversal inside one flight; the
    /// flight itself still starts from wherever the property currently is,
    /// because that is what a retarget does.
    shown: f32,
    /// Whether the cap is currently on screen. Front-path state: the DWM path
    /// gates the cap by an expression on its `Opacity` instead (see
    /// [`bind_cap`]), so nothing here toggles it.
    cap_visible: bool,
}

/// A node's retained bar field: its sprites, the FP16 sources they share, and
/// the two animation objects every retarget goes through.
pub(crate) struct BarField {
    bars: Vec<Bar>,
    /// The layout the sprites were built for; `None` until the first push.
    layout: Option<BarFieldLayout>,
    /// `(atlas epoch, raster scale bits)` the two brushes were rasterized at. A
    /// device loss clears the atlas and bumps its epoch, and a display move
    /// changes the scale — either rebuilds the sources, exactly as
    /// [`Part::bind`](super::parts::Part::bind) does.
    sources: Option<(u32, u32)>,
    /// Kept alive for the sprites that reference them. The body's ramp is a
    /// compositor gradient masking one flat FP16 source, and `MappingMode::Relative`
    /// measures it against each bar's own sprite — so one brush serves every bar
    /// whatever its height, and a bar growing under its `Scale.Y` animation does
    /// not disturb the fade.
    _body_brush: Option<super::gradient::RampSource>,
    _cap_brush: Option<CompositionSurfaceBrush>,
    /// The two cached retarget animations and their easing — the DWM path only,
    /// built on first use and dropped by a [`rebuild`](Self::rebuild) so a
    /// layout stating different ballistics mints them again. One pair per field
    /// rather than per bar: `StartAnimation` takes the animation's state as it
    /// stands, so the same object retargets every bar in turn.
    rise_anim: Option<ScalarKeyFrameAnimation>,
    fall_anim: Option<ScalarKeyFrameAnimation>,
    easing: Option<CompositionEasingFunction>,
    /// When the last value push was applied, so the envelope integrates real
    /// elapsed time rather than assuming a cadence. `None` until the first one,
    /// and untouched by the DWM path, which keeps no clock of its own.
    last_push: Option<Instant>,
    /// True until the first value push has landed. The first frame SNAPS: a
    /// field mounting must not play its opening spectrum as a rise from the
    /// floor.
    priming: bool,
}

impl BarField {
    pub(crate) fn new() -> Self {
        Self {
            bars: Vec::new(),
            layout: None,
            sources: None,
            _body_brush: None,
            _cap_brush: None,
            rise_anim: None,
            fall_anim: None,
            easing: None,
            last_push: None,
            priming: true,
        }
    }

    /// Reconcile the field against one drained batch. Everything self-gates: a
    /// service that finds the layout unchanged and every value settled issues no
    /// COM calls at all.
    pub(crate) fn sync(
        &mut self,
        comp: &Compositing,
        container: &windows_composition::ContainerVisual,
        batch: &BarBatch,
        atlas_epoch: u32,
        scale: f32,
    ) {
        if let Some(l) = &batch.layout
            && self.layout.as_ref() != Some(l)
        {
            self.rebuild(comp, container, l.clone());
        }
        // Detached for the duration rather than borrowed: everything below
        // mutates `self.bars` while reading the layout, which are disjoint
        // fields the borrow checker cannot see through `&self.layout`. A move
        // out and back allocates nothing.
        let Some(layout) = self.layout.take() else { return };
        self.bind_sources(comp, atlas_epoch, scale, &layout);
        if batch.has_values {
            self.push(&batch.values, &layout);
        }
        self.layout = Some(layout);
    }

    /// Rebuild the sprite geometry for a new layout. The ONLY place a visual is
    /// created or destroyed — a value push never touches the tree.
    fn rebuild(
        &mut self,
        comp: &Compositing,
        container: &windows_composition::ContainerVisual,
        layout: BarFieldLayout,
    ) {
        let n = layout.bars.len();
        // Shrink first so the retired sprites leave the tree before the
        // survivors are re-placed.
        if n < self.bars.len() {
            for bar in self.bars.drain(n..) {
                container.children().remove(&bar.body);
                container.children().remove(&bar.cap);
            }
        }
        while self.bars.len() < n {
            let body = comp.new_sprite();
            let cap = comp.new_sprite();
            // The body goes in FIRST so the cap, inserted above it, is never
            // covered by the body's own top edge.
            container.children().insert_at_top(&body);
            container.children().insert_at_top(&cap);
            self.bars.push(Bar {
                body,
                cap,
                shown: 0.0,
                cap_visible: true,
            });
        }

        for (bar, rect) in self.bars.iter_mut().zip(&layout.bars) {
            let w = rect.w.max(0.0);
            let h = layout.height.max(0.0);
            bar.body.set_offset(rect.x, layout.top, 0.0);
            bar.body.set_size(w, h);
            // The pivot is the bar's BOTTOM edge, so `Scale.Y` grows it upward
            // from the floor — the one property that expresses a bar's value.
            bar.body.set_center_point(Vector3::new(0.0, h, 0.0));
            bar.body.set_scale(Vector3::new(1.0, bar.shown.clamp(0.0, 1.0), 1.0));
            bar.cap.set_offset(rect.x, cap_y(&layout, bar.shown.clamp(0.0, 1.0)), 0.0);
            bar.cap.set_size(w, layout.cap_h.max(0.0));
            // A zero-width bar has nothing to draw; hiding it is cheaper than
            // asking the compositor to composite an empty sprite.
            let live = w > 0.0;
            bar.body.set_visible(live);
            bar.cap.set_visible(live && bar.cap_visible);
        }

        if live_anim() == LiveAnim::Dwm {
            // The placement above wrote `Offset` and `Scale` DIRECTLY, and a
            // direct write disconnects whatever animation held the property —
            // so the bindings are (re)established here, after those writes, and
            // the two cached retargets are dropped because the layout may have
            // restated the durations they were built with.
            self.rise_anim = None;
            self.fall_anim = None;
            for bar in &self.bars {
                bind_cap(bar, &layout);
            }
        }

        // The sources are sized against the raster scale, not the layout, but
        // the colours live in the layout — so a recolour must re-rasterize.
        if self.layout.as_ref().map(|l| (l.body_top, l.body_bottom, l.cap))
            != Some((layout.body_top, layout.body_bottom, layout.cap))
        {
            self.sources = None;
        }
        self.layout = Some(layout);
    }

    /// (Re)rasterize and bind the two FP16 colour sources. Gated on the atlas
    /// epoch and the raster scale, so a steady field binds nothing.
    fn bind_sources(
        &mut self,
        comp: &Compositing,
        atlas_epoch: u32,
        scale: f32,
        layout: &BarFieldLayout,
    ) {
        let want = (atlas_epoch, scale.to_bits());
        if self.sources == Some(want) {
            return;
        }
        // Colour is a display-mapped FP16 source, as everywhere else in the
        // backend — a colour brush is 8-bit and cannot carry this palette's
        // above-paper-white values. The body's FADE is a compositor gradient
        // masking that source, running down each bar's own sprite so it lands on
        // the bar rather than on the plot. Its stops are NORMALIZED to the full
        // alpha range with the peak folded into the source's brightness: the
        // compositor's alpha intermediate is 8-bit, and a fade authored across
        // the 0.15 the tokens actually span would posterize into ~37 steps.
        let (Some(body), Some(cap)) = (
            super::gradient::RampSource::build(
                comp,
                &[(0.0, layout.body_top), (1.0, layout.body_bottom)],
                crate::GradientAxis::Vertical,
                scale,
            ),
            super::parts::build_solid_surface(comp, layout.cap, scale),
        ) else {
            return;
        };
        for bar in &self.bars {
            bar.body.set_brush(body.brush());
            bar.cap.set_brush(&cap);
        }
        self._body_brush = Some(body);
        self._cap_brush = Some(cap);
        self.sources = Some(want);
    }

    /// Apply one frame of values by whichever mechanism this process animates
    /// with. Both are self-gating: a field whose every value has settled issues
    /// no COM calls at all.
    fn push(&mut self, values: &[f32], layout: &BarFieldLayout) {
        if self.bars.is_empty() {
            return;
        }
        match live_anim() {
            LiveAnim::Front => self.push_front(values, layout),
            LiveAnim::Dwm => self.push_dwm(values, layout),
        }
        self.priming = false;
    }

    /// Step every bar's envelope by the real elapsed time and write the two
    /// sprites that moved. Two property writes per moved bar, nothing else.
    fn push_front(&mut self, values: &[f32], layout: &BarFieldLayout) {
        let now = Instant::now();
        let dt = self
            .last_push
            .map_or(0.0, |t| now.duration_since(t).as_secs_f32())
            .min(MAX_STEP_SECS);
        self.last_push = Some(now);
        let (k_rise, k_fall) = (closing(dt, layout.rise), closing(dt, layout.fall));

        let priming = self.priming;
        for (bar, &raw) in self.bars.iter_mut().zip(values) {
            let target = if raw.is_finite() { raw.clamp(0.0, 1.0) } else { 0.0 };
            // The opening frame is not a change; it is where the analyzer
            // starts. Snap, and let the next publish be the first motion.
            let t = if priming {
                target
            } else {
                let k = if target > bar.shown { k_rise } else { k_fall };
                bar.shown + (target - bar.shown) * k
            };
            // The cap follows the DRAWN value, not the target: a bar still on
            // its way down to the floor keeps its top edge until it gets there.
            let cap_visible = t > FLOOR_EPS;
            if bar.cap_visible != cap_visible {
                bar.cap.set_visible(cap_visible);
                bar.cap_visible = cap_visible;
            }
            if !priming && (t - bar.shown).abs() < CHANGE_EPS {
                continue;
            }
            // One value, two writes, one commit — which is what welds the cap
            // to the body's top edge at every frame rather than only at the
            // ends of a flight.
            bar.body.set_scale(Vector3::new(1.0, t, 1.0));
            bar.cap.set_offset(bar.cap.offset().x, cap_y(layout, t), 0.0);
            note_property_writes(2);
            bar.shown = t;
        }
    }

    /// Retarget every bar whose value moved and let the compositor fly it there.
    ///
    /// One `InsertKeyFrame` and one `StartAnimation` per moved bar, on a pair of
    /// animations the field already holds — and NOTHING for the cap, whose
    /// offset and floor gate are expressions over the body's live `Scale.Y`
    /// (see [`bind_cap`]). Between publishes this path issues nothing at all.
    ///
    /// A key frame rather than a spring, though a spring is the shape that
    /// "retarget from where you are, with the velocity you have" describes.
    /// Two reasons. [`BarFieldLayout::rise`] and [`BarFieldLayout::fall`] are
    /// stated as DURATIONS, which a key frame consumes directly and a spring
    /// only through an invented damping/period mapping — so this path animates
    /// on the numbers the caller actually gave, and an A/B against the front
    /// path measures the mechanism instead of a second set of ballistics. And a
    /// spring carrying velocity into a reversal overshoots: past `1.0` a bar
    /// leaves the plot band, and past `0.0` a negative `Scale.Y` flips the
    /// sprite through its own baseline. Clamping that back would take a per
    /// frame app-side correction, which is the one thing this path exists not
    /// to do.
    fn push_dwm(&mut self, values: &[f32], layout: &BarFieldLayout) {
        let compositor = self.bars[0].body.compositor();
        let easing: &CompositionEasingFunction = self
            .easing
            .get_or_insert_with(|| retarget_easing(&compositor));
        // The durations are baked in at construction, not restated per push: a
        // layout that changes them drops both objects (see
        // [`rebuild`](Self::rebuild)), so the pair standing here always carries
        // the ballistics the current layout asked for.
        let rise: &ScalarKeyFrameAnimation = self.rise_anim.get_or_insert_with(|| {
            let a = compositor.create_scalar_key_frame_animation();
            a.set_duration(layout.rise);
            a
        });
        let fall: &ScalarKeyFrameAnimation = self.fall_anim.get_or_insert_with(|| {
            let a = compositor.create_scalar_key_frame_animation();
            a.set_duration(layout.fall);
            a
        });

        let priming = self.priming;
        for (bar, &raw) in self.bars.iter_mut().zip(values) {
            let target = if raw.is_finite() { raw.clamp(0.0, 1.0) } else { 0.0 };
            if !priming && (target - bar.shown).abs() < CHANGE_EPS {
                continue;
            }
            if priming {
                // The opening frame is not a change; it is where the analyzer
                // starts. Written directly — no animation owns the property
                // yet, and a field mounting must not play its opening spectrum
                // as a rise from the floor.
                bar.body.set_scale(Vector3::new(1.0, target, 1.0));
                note_property_writes(1);
            } else {
                // `StartAnimation` on a property that already has one replaces
                // it and picks up from the value in flight, so a retarget is
                // this and nothing else — no `StopAnimation` first, which would
                // strand the bar at wherever it had reached and make the new
                // flight start from a standstill.
                let a = if target > bar.shown { rise } else { fall };
                a.insert_key_frame_with_easing(1.0, target, easing);
                bar.body.start_animation("Scale.Y", a);
                note_anim_start();
            }
            bar.shown = target;
        }
    }
}

/// Weld one bar's cap to its body, compositor-side, for the whole life of a
/// layout.
///
/// The cap is not animated: its `Offset.Y` is [`cap_y`]'s identity restated as
/// an expression the compositor evaluates from the body's LIVE `Scale.Y`. Two
/// independently retargeted animations would meet at the ends of a flight and
/// could part anywhere in between — a change gate that skips one bar's cap and
/// not its body is all it takes — whereas a value derived from another cannot
/// drift from it. It also halves what a publish costs: only the body is
/// retargeted, and the cap follows for free.
///
/// The floor gate rides that same live value. The cap's `Opacity` carries the
/// [`FLOOR_EPS`] test against the DRAWN height, so a bar on its way down keeps
/// its top edge until it arrives, rather than shedding it the moment a floor
/// target is published. The front path hides the sprite outright, which is
/// cheaper than compositing a transparent one — but `IsVisible` is not an
/// animatable property, so DWM-side this is what the same rule costs.
fn bind_cap(bar: &Bar, layout: &BarFieldLayout) {
    let compositor = bar.body.compositor();
    let (top, h) = (layout.top, layout.height.max(0.0));
    let offset =
        compositor.create_expression_animation(&format!("{top:.3} + {h:.3} * (1.0 - b.Scale.Y)"));
    offset.set_reference_parameter("b", &**bar.body);
    bar.cap.start_animation("Offset.Y", &offset);
    let gate = compositor.create_expression_animation(&format!(
        "b.Scale.Y > {FLOOR_EPS} ? 1.0 : 0.0"
    ));
    gate.set_reference_parameter("b", &**bar.body);
    bar.cap.start_animation("Opacity", &gate);
    // Both animations are dropped here: the compositor holds the ones it is
    // running, and the reference parameter holds the body — the same discipline
    // every other expression binding in the backend keeps.
    note_expression_binds(2);
}
