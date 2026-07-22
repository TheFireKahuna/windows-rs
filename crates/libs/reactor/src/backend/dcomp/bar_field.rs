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
//! are: retained sprites the compositor owns, coloured by an FP16 source, moved
//! DWM-side. Between publishes the app does nothing at all.
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
//! Two `InsertKeyFrame`s and two `StartAnimation`s per bar — the body's extent
//! and the cap that rides its top edge — on two animation objects shared by the
//! whole field. No visual, brush, animation or easing
//! function is created per publish — those are built once per *layout*, and a
//! layout changes only on a resize, a DPI move, a bar-count change or a
//! recolour. There is no per-publish allocation on either side of the queue:
//! the pending value buffer is swapped with the front thread's, so both keep
//! their capacity.
//!
//! ## How a bar moves
//!
//! A bar is two sprites — a body and the brighter cap that gives it a defined
//! top edge — and each is one animated scalar. The body is a full-height sprite
//! whose `CenterPoint` sits on its bottom edge, so its whole extent is `Scale.Y`
//! in `0..=1` and no offset has to move with it. The cap is a fixed-height strip
//! whose `Offset.Y` is the body's top edge.
//!
//! The two are retargeted together, from the same value, with the same duration
//! and the same easing, in the same commit — and that is what pins the cap to
//! the body. Both curves are affine in the value, so interpolating each from its
//! own start under one shared curve keeps `offset = top + (1 - scale) * height`
//! true at every instant of the flight, not merely at the ends.
//!
//! An earlier version instead tied the cap to `body.Scale.Y` through an
//! [`ExpressionAnimation`](windows_composition::ExpressionAnimation), started
//! once at layout time — one retarget per bar per publish rather than two, and
//! the cap free. On screen the cap did NOT stay on the body: it ran ahead of it
//! during an attack by several dB, which is what an expression reading the
//! animation's destination rather than its current value looks like. Two
//! explicit animations are correct by construction and cost one extra property
//! set per bar, so that is what ships.
//!
//! ## Ballistics
//!
//! An analyzer that rises and falls at the same rate reads as jitter, so the
//! caller states an asymmetric pair ([`BarFieldLayout::rise`] /
//! [`BarFieldLayout::fall`]) and each bar picks one per push from its own
//! direction of travel. See [`BarFieldLayout::rise`] for how a duration maps
//! onto the one-pole envelope this replaces.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use windows_composition::{
    CompositionEasingFunction, CompositionSurfaceBrush, ScalarKeyFrameAnimation, SpriteVisual,
};
use windows_numerics::{Vector2, Vector3};

use super::bootstrap::Compositing;
use crate::backend::ControlId;
use crate::Color;

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
    /// The envelope this replaces is a one-pole stepped per display frame with
    /// a time constant `τ`, and the retargeting here reproduces it: each publish
    /// starts a fresh ease-out from wherever the bar currently is, so the gap
    /// closes by a fixed fraction per publish exactly as the one-pole's does,
    /// and the two agree when the fraction agrees. Over a 10 ms publish
    /// interval a `2τ` ease-out covers within a couple of percent of what
    /// `1 - e^(-Δ/τ)` does, so a caller porting from a one-pole should pass
    /// twice its time constant.
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

// ── The front-thread field ───────────────────────────────────────────────────

/// A value change smaller than this is not worth a retarget: at a 200 DIP plot
/// it is a twentieth of a pixel, and skipping it is what makes a silent
/// analyzer cost nothing per publish.
const CHANGE_EPS: f32 = 0.0005;

/// Below this a bar is at the floor and its cap is hidden, so a silent field is
/// EMPTY rather than a hairline along the bottom edge — matching the painted
/// analyzer, which drew no bar whose top had reached the baseline.
const FLOOR_EPS: f32 = 0.0005;

/// The ease-out the retargeting plays. `cubic-bezier(0, 0, 0.58, 1)` — the same
/// curve the backend's implicit transitions use, and the one whose shape a
/// one-pole's decay is being matched against (see [`BarFieldLayout::rise`]).
const EASE_C1: (f32, f32) = (0.0, 0.0);
const EASE_C2: (f32, f32) = (0.58, 1.0);

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
    /// The last value pushed — the retarget gate, and the direction of travel
    /// the next push picks its duration from.
    shown: f32,
    /// Whether the cap is currently on screen.
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
    /// Kept alive for the sprites that reference them.
    _body_brush: Option<CompositionSurfaceBrush>,
    _cap_brush: Option<CompositionSurfaceBrush>,
    /// The two retarget animations, one per direction of travel, SHARED by
    /// every bar in the field. A composition animation's configuration is
    /// captured when it is started, so restating the key frame for the next bar
    /// cannot disturb one already in flight — which is what lets a whole field
    /// retarget through two objects instead of one per bar.
    rise: Option<ScalarKeyFrameAnimation>,
    fall: Option<ScalarKeyFrameAnimation>,
    easing: Option<CompositionEasingFunction>,
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
            rise: None,
            fall: None,
            easing: None,
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
            // A fresh geometry restates both properties, so any animation
            // holding one must be stopped or the write is ignored.
            bar.body.stop_animation("Scale.Y");
            bar.cap.stop_animation("Offset.Y");
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
        // The same display-mapped FP16 rasters every other sprite in the backend
        // takes — a colour brush is 8-bit and cannot carry this palette's
        // above-paper-white values, and its stops would posterize a fade this
        // faint. The body's is the VERTICAL ramp (the curve underfill's source
        // builder), stretched down the sprite so the fade lands on the bar rather
        // than on the plot.
        let (Some(body), Some(cap)) = (
            super::parts::build_vgradient_surface(
                comp,
                &[(0.0, layout.body_top), (1.0, layout.body_bottom)],
                scale,
            ),
            super::parts::build_solid_surface(comp, layout.cap, scale),
        ) else {
            return;
        };
        for bar in &self.bars {
            bar.body.set_brush(&body);
            bar.cap.set_brush(&cap);
        }
        self._body_brush = Some(body);
        self._cap_brush = Some(cap);
        self.sources = Some(want);
    }

    /// Retarget every bar that moved. One `InsertKeyFrame` + one
    /// `StartAnimation` each, on the field's two shared animations.
    fn push(&mut self, values: &[f32], layout: &BarFieldLayout) {
        let compositor = match self.bars.first() {
            Some(b) => b.body.compositor(),
            None => return,
        };
        let easing = self.easing.get_or_insert_with(|| {
            compositor.create_cubic_bezier_easing_function(
                Vector2::new(EASE_C1.0, EASE_C1.1),
                Vector2::new(EASE_C2.0, EASE_C2.1),
            )
        });
        let rise = self.rise.get_or_insert_with(|| {
            let a = compositor.create_scalar_key_frame_animation();
            a.set_duration(layout.rise);
            a
        });
        rise.set_duration(layout.rise);
        let fall = self.fall.get_or_insert_with(|| {
            let a = compositor.create_scalar_key_frame_animation();
            a.set_duration(layout.fall);
            a
        });
        fall.set_duration(layout.fall);

        let priming = self.priming;
        for (bar, &raw) in self.bars.iter_mut().zip(values) {
            let t = if raw.is_finite() { raw.clamp(0.0, 1.0) } else { 0.0 };
            let cap_visible = t > FLOOR_EPS;
            if bar.cap_visible != cap_visible {
                bar.cap.set_visible(cap_visible);
                bar.cap_visible = cap_visible;
            }
            if !priming && (t - bar.shown).abs() < CHANGE_EPS {
                continue;
            }
            let y = cap_y(layout, t);
            if priming {
                // The opening frame is not a change; it is where the analyzer
                // starts. Snap, and let the next publish be the first motion.
                bar.body.stop_animation("Scale.Y");
                bar.cap.stop_animation("Offset.Y");
                bar.body.set_scale(Vector3::new(1.0, t, 1.0));
                bar.cap.set_offset(bar.cap.offset().x, y, 0.0);
            } else {
                // ONE direction for the pair — the cap is not moving
                // independently, it is the body's top edge — and one duration,
                // so the two stay welded for the whole flight.
                let a = if t > bar.shown { &*rise } else { &*fall };
                a.insert_key_frame_with_easing(1.0, t, easing);
                bar.body.start_animation("Scale.Y", a);
                a.insert_key_frame_with_easing(1.0, y, easing);
                bar.cap.start_animation("Offset.Y", a);
            }
            bar.shown = t;
        }
        self.priming = false;
    }
}
