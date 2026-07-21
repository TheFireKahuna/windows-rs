//! `ProgressRing` — a track circle and a value arc, both retained.
//!
//! Two [`PathLayer`]s over ONE tessellated circle. The layers are separate
//! because a mask brush carries one colour source and the track and the arc are
//! different colours; sharing a mask would render the track in the accent, and
//! an untrimmed full circle composited into the arc's mask alpha would swallow
//! the arc's trim entirely. What IS shared is the tessellated path — the
//! expensive half — exactly as the knob shares its arc with its thumb.
//!
//! Both of the ring's modes are compositor-side, so a spinning ring costs the
//! app nothing per frame:
//!
//! - **determinate** — the value is `TrimEnd` on the arc's geometry, sprung, so
//!   the arc grows to a new value DWM-side;
//! - **indeterminate** — the arc is trimmed to a fixed extent and the whole
//!   layer is revolved by a forever `RotationAngle` keyframe.
//!
//! The revolve targets the arc's IN-TREE sprite, not the off-tree mask visual:
//! rotating the mask would re-rasterize its alpha through a changing transform
//! every composition frame, where rotating the composited layer is one cheap
//! transform on a bitmap that is already correct. Only the arc turns — a full
//! circle is rotation-invariant, so revolving the track too would be two
//! animations to buy what one already gets.

use windows_numerics::Vector3;

use super::bootstrap::Compositing;
use super::node::Node;
use super::path_shape::{arc_path, PathLayer, Role};
use super::theme;
use crate::system_bindings::{
    AnimationIterationBehavior, CompositionAnimation, CompositionEasingFunction,
    ICompositionObject, IKeyFrameAnimation,
};
use windows_core::Interface;

/// How far inside the node's half-extent the track centreline sits. Leaves room
/// for the stroke, which is clamped to at most 5 DIP and so reaches at most 2.5
/// DIP outward — always inside the bounds, except on a degenerate sub-6-DIP ring
/// where the radius floor takes over and the shape visual clips it. That was
/// equally true of the surface this replaces.
const RING_INSET: f32 = 3.0;

/// The arc's extent while indeterminate, as a fraction of a full turn.
///
/// The retired painter swept `FRAC_PI_2 * 2.4`, i.e. `1.2π`, and `1.2π / τ` is
/// exactly `0.6` — the equivalence is arithmetic, not taste.
const INDETERMINATE_FRAC: f32 = 0.6;

/// Where a ring's arc begins: twelve o'clock.
const START_ANGLE: f32 = -std::f32::consts::FRAC_PI_2;

/// The ring's geometry from the node — one definition, so the track and the arc
/// can never disagree about where the circle is.
fn ring_geom(node: &Node) -> (f32, f32, f32, f32) {
    let (w, h) = (node.rect.w, node.rect.h);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = (cx.min(cy) - RING_INSET).max(2.0);
    let thick = (r * 0.18).clamp(2.0, 5.0);
    (cx, cy, r, thick)
}

pub(crate) struct RingParts {
    /// Bottom: the full, never-trimmed circle.
    track: PathLayer,
    /// Top: the value arc, trimmed, and revolved while indeterminate.
    arc: PathLayer,
    /// `(cx, cy, r)` in DIPs — gates re-tessellating the shared path.
    geom: (f32, f32, f32),
    /// `(w, h, scale)` — gates the layer resize. `scale` is in the gate because
    /// it is an input to the mask raster, so a display move with an unchanged
    /// rect still has to re-rasterize.
    size: (f32, f32, f32),
    /// Whether a forever revolve is live on the arc's sprite.
    looping: bool,
    /// Last mode seen, so the frame it flips can force a snap.
    indeterminate: Option<bool>,
    init: bool,
}

impl RingParts {
    fn new(comp: &Compositing, node: &Node) -> Option<Self> {
        let (cx, cy, r, _) = ring_geom(node);
        let path = arc_path(&comp.gpu, cx, cy, r, START_ANGLE, START_ANGLE + std::f32::consts::TAU)?;
        // Track first: `PathLayer::new` inserts at the top of the container, so
        // creation order IS z-order and the arc must be built second.
        let track = PathLayer::new(comp, node, &path, Role::Stroke)?;
        let arc = PathLayer::new(comp, node, &path, Role::Stroke)?;
        Some(Self {
            track,
            arc,
            geom: (f32::NAN, f32::NAN, f32::NAN),
            size: (f32::NAN, f32::NAN, f32::NAN),
            looping: false,
            indeterminate: None,
            init: false,
        })
    }

    fn sync(&mut self, comp: &Compositing, node: &Node, atlas_epoch: u32, scale: f32) {
        let (w, h) = (node.rect.w, node.rect.h);
        let (cx, cy, r, thick) = ring_geom(node);
        let ind = node.ctrl().indeterminate;
        let dim = super::parts::dim_of(node);

        // ── Geometry ──
        //
        // Authored in DIPs, like every other consumer of `PathLayer`: the layer
        // puts the DIP→px scale on its shape (see `PathLayer::resize`), so a
        // display change is a property set rather than a re-tessellation. This
        // used to bake `scale` into every coordinate here, back when the layer
        // could not carry it.
        let resized = self.geom != (cx, cy, r);
        if resized
            && let Some(path) = arc_path(
                &comp.gpu,
                cx,
                cy,
                r,
                START_ANGLE,
                START_ANGLE + std::f32::consts::TAU,
            )
        {
            self.track.set_path(&path);
            self.arc.set_path(&path);
            self.geom = (cx, cy, r);
        }
        if self.size != (w, h, scale) {
            self.track.resize(w, h, scale);
            self.arc.resize(w, h, scale);
            self.size = (w, h, scale);
        }
        self.track.set_thickness(thick);
        self.arc.set_thickness(thick);
        self.track.set_source(comp, theme::w(0.08), &[], atlas_epoch, scale);
        self.arc.set_source(comp, theme::accent(), &[], atlas_epoch, scale);
        self.track.set_opacity(dim);
        self.arc.set_opacity(dim);

        // ── Trim ──
        //
        // A mode flip REDEFINES what the arc means, so it snaps; a value change
        // within determinate mode springs. The first sync snaps for the same
        // reason — there is no previous value to have travelled from.
        let mode_changed = self.indeterminate != Some(ind);
        let snap = mode_changed || !self.init || resized;
        let end = if ind { INDETERMINATE_FRAC } else { super::ctrl_value_frac(node) as f32 };
        if snap {
            self.arc.snap_trim(0.0, end);
        } else {
            self.arc.set_trim(0.0, end);
        }

        // ── The revolve ──
        if ind {
            if !self.looping || resized {
                self.looping = self.start_revolve(cx, cy).is_some();
            }
        } else if self.looping {
            self.stop_revolve();
        }

        self.indeterminate = Some(ind);
        self.init = true;
    }

    fn start_revolve(&self, cx: f32, cy: f32) -> Option<()> {
        let (sprite, vis) = self.arc.display();
        vis.SetCenterPoint(Vector3::new(cx, cy, 0.0)).ok()?;
        let obj: ICompositionObject = sprite.cast().ok()?;
        let c = obj.Compositor().ok()?;
        let lin: CompositionEasingFunction = c.CreateLinearEasingFunction().ok()?.cast().ok()?;
        let a = c.CreateScalarKeyFrameAnimation().ok()?;
        a.InsertKeyFrameWithEasingFunction(0.0, 0.0, &lin).ok()?;
        a.InsertKeyFrameWithEasingFunction(1.0, std::f32::consts::TAU, &lin).ok()?;
        let kf: IKeyFrameAnimation = a.cast().ok()?;
        kf.SetDuration(super::parts::progress_cycle()).ok()?;
        kf.SetIterationBehavior(AnimationIterationBehavior::Forever).ok()?;
        let _ = obj.StopAnimation("RotationAngle");
        obj.StartAnimation("RotationAngle", &a.cast::<CompositionAnimation>().ok()?).ok()
    }

    /// Stop the revolve AND return the arc to twelve o'clock.
    ///
    /// `StopAnimation` leaves `RotationAngle` wherever the animation had reached,
    /// so without the explicit reset a ring flipping to determinate would draw
    /// its value arc from a random start angle.
    fn stop_revolve(&mut self) {
        let (sprite, vis) = self.arc.display();
        if let Ok(obj) = sprite.cast::<ICompositionObject>() {
            let _ = obj.StopAnimation("RotationAngle");
        }
        let _ = vis.SetRotationAngle(0.0);
        self.looping = false;
    }
}

/// Ensure the node has its ring layers and reconcile them (the paint-pass entry).
pub(crate) fn sync_ring(comp: &Compositing, node: &mut Node, atlas_epoch: u32, scale: f32) {
    if node.ring.is_none() {
        node.ring = RingParts::new(comp, node).map(Box::new);
    }
    if let Some(mut rp) = node.ring.take() {
        rp.sync(comp, node, atlas_epoch, scale);
        node.ring = Some(rp);
    }
}
