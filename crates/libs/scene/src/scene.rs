//! The scene: the retained tree, the patch applier, motion, trackers and the ground.
//!
//! Composition objects are touched inside scene passes. The driver requests publication
//! after a pass changes the scene; native animations advance independently of that driver.

use crate::arena::*;
use crate::hit::HitTable;
use crate::hit_entry::{ContactKind, Hit, pack_offset};
use crate::patch::{Attach, Op, SinkPatch};
use crate::realize::{
    Backends, Beneath, BoxKey, Cache, Ctx, GRAIN_ALPHA, GRAIN_CODES, GRAIN_STRIPS, GRAIN_TILE,
    ResObj, Resources, fit, realize,
};
use crate::sink::*;
use core::cell::{Cell, RefCell};
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use std::rc::Rc;
use std::sync::Arc;
use windows_d2d::note;
use windows_composition::{
    Animatable, Animation, BatchKind, ChainingMode, Clamping, CompositionAnimation,
    CompositionEasingFunction, CompositionPropertySet, CompositionScopedBatch, ContainerVisual,
    DesktopWindowTarget, ExpressionAnimation, InteractionTracker, ManipulationPointer,
    RedirectionMode, ScaleAnimationPolicy, SourceMode, SpringScalarNaturalMotionAnimation,
    SpringVector2NaturalMotionAnimation, SpringVector3NaturalMotionAnimation, SpriteVisual,
    Stretch, TrackerEvent, Visual, VisualInteractionSource, WheelMode,
};
use windows_core::{EventRevoker, Result};
use windows_numerics::{Vector2, Vector3};

#[path = "drag_preview.rs"]
mod drag_preview;

// ── the census ──────────────────────────────────────────────────────────────────────

/// Running tallies of what the scene has done since it was created.
///
/// At idle the compositor's cost is the tree walk rather than the drawing, so the number of
/// visuals alive and the number of property writes issued are first-class costs. Counted in
/// the applier, which sees every mint, destroy and write, and nowhere below it.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Census {
    /// Visuals this scene is holding, ghosts included. One life event each: a mint raises it
    /// and a destroy lowers it, and re-parenting is neither.
    pub visuals_live: u32,
    /// Visuals ever minted. A mint rate that climbs while the live count is flat is nodes
    /// being rebuilt instead of reused.
    pub visuals_minted: u64,
    /// Writes that survived the idempotent early return, and so cost something.
    pub props_written: u64,
    /// Writes the early return absorbed. A high ratio is the emitter sending a subtree where
    /// three nodes moved.
    pub props_skipped: u64,
    /// Interaction trackers this scene is holding.
    ///
    /// Watched because a tracker that was never built is invisible from every other angle:
    /// the bindings onto it apply, the ops addressed to it are dropped, and the surface
    /// never scrolls.
    pub trackers_live: u32,
    /// Ops applied, across every patch.
    pub ops_applied: u64,
    /// Animations started, which is the event-rate cost of motion.
    pub animations: u64,
    /// Patches applied under a different environment than they were solved under.
    ///
    /// Counted and not refused: an occasional bump is a display change landing between a
    /// flush and its apply, and a count that keeps pace with the patch count is the two
    /// halves deriving the environment independently.
    pub env_mismatches: u64,
}

impl Census {
    fn count(&mut self, written: bool) {
        if written {
            self.props_written += 1;
        } else {
            self.props_skipped += 1;
        }
    }

    /// Whether the pass did anything: an op applied, a property written, an animation
    /// started, or a visual minted.
    ///
    /// A woken pass that answers `false` names a producer that rang for work it did not
    /// have — the one form of idle waste the crate cannot detect from inside.
    #[must_use]
    pub fn changed_since(&self, previous: &Self) -> bool {
        self.visuals_live != previous.visuals_live
            || self.ops_applied != previous.ops_applied
            || self.props_written != previous.props_written
            || self.animations != previous.animations
            || self.visuals_minted != previous.visuals_minted
    }
}

/// What a walk of the tree found, against what the arena holds.
///
/// The two disagreeing means a link or a life event is wrong, which nothing else reports: an
/// orphaned node still renders.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Audit {
    /// Nodes the walk reached from the roots.
    pub reached: u32,
    /// Nodes the arena is holding. Above `reached` means something was orphaned rather than
    /// destroyed; below means the chain has a cycle or crosses parents.
    pub held: u32,
}

impl Audit {
    /// Whether the tree the walk found is the tree the arena holds.
    #[must_use]
    pub const fn agrees(&self) -> bool {
        self.reached == self.held
    }
}

// ── motion ──────────────────────────────────────────────────────────────────────────

/// The scroll carrier's spring period, in seconds: the tuning that carries momentum.
pub const SCROLL_PERIOD: f32 = 0.2756;
/// The scroll carrier's damping ratio.
pub const SCROLL_DAMPING: f32 = 0.877;
/// The chrome spring's period, in seconds: every indicator, ink, pill glide and trim.
///
/// The compositor plays a spring of this period for several times the settling time a
/// second-order model predicts, so it is not that model's undamped natural period and cannot
/// be computed from a stiffness and a damping ratio. It is fixed by eye. At fixed damping
/// the motion's duration does scale linearly with the period, which is what the travel
/// scaling relies on.
pub const CHROME_PERIOD: f32 = 0.0900;
/// The chrome spring's damping ratio.
pub const CHROME_DAMPING: f32 = 0.900;

/// `[period, damping]` per tuning, indexed by [`Tuning`].
const SPRING: [[f32; 2]; 2] = [
    [CHROME_PERIOD, CHROME_DAMPING],
    [SCROLL_PERIOD, SCROLL_DAMPING],
];

/// The travel [`CHROME_PERIOD`] is quoted against, in DIPs.
const CHROME_REF_TRAVEL: f32 = 120.0;

/// One expression per tracker axis, each mapping the tracker through `value * m + c`. The
/// parameters are scalars, so the strings are constants and one instance per axis serves
/// every binding.
const TRACK_EXPR: [&str; 3] = [
    "t.Position.X * m + c",
    "t.Position.Y * m + c",
    "t.Scale * m + c",
];

/// Restricted to trim and opacity, so a scalar's followers share its exact compositor
/// position across zero without a second spring and cannot create an offset cycle.
const FOLLOW_EXPR: [&str; 4] = [
    "Clamp(v.Offset.X * m + c, lo, hi)",
    "Clamp(v.Offset.Y * m + c, lo, hi)",
    "Clamp(b.Size.X > inset ? (v.Offset.X * m + c) / Max(b.Size.X - inset, 0.0001) : 0, lo, hi)",
    "Clamp(b.Size.Y > inset ? (v.Offset.Y * m + c) / Max(b.Size.Y - inset, 0.0001) : 0, lo, hi)",
];

/// Retains a native layout spring for one visual property.
pub(crate) enum LayoutSpring {
    Scalar(SpringScalarNaturalMotionAnimation),
    Vec2(SpringVector2NaturalMotionAnimation),
    Vec3(SpringVector3NaturalMotionAnimation),
}

impl LayoutSpring {
    fn new(back: &Backends, slot: u8) -> Self {
        let period = Duration::from_secs_f32(CHROME_PERIOD);
        match slot {
            1 => {
                let spring = back.compositor.create_spring_scalar_animation();
                spring.set_damping_ratio(CHROME_DAMPING);
                spring.set_period(period);
                Self::Scalar(spring)
            }
            2 => {
                let spring = back.compositor.create_spring_vector2_animation();
                spring.set_damping_ratio(CHROME_DAMPING);
                spring.set_period(period);
                Self::Vec2(spring)
            }
            _ => {
                let spring = back.compositor.create_spring_vector3_animation();
                spring.set_damping_ratio(CHROME_DAMPING);
                spring.set_period(period);
                Self::Vec3(spring)
            }
        }
    }

    fn retarget(&self, to: Value) -> CompositionAnimation {
        match (self, to) {
            (Self::Scalar(spring), Value::Scalar(value)) => {
                spring.set_final_value(value);
                spring.as_animation()
            }
            (Self::Vec2(spring), Value::Vec2(value)) => {
                spring.set_final_value(value);
                spring.as_animation()
            }
            (Self::Vec3(spring), Value::Vec2(value)) => {
                spring.set_final_value(v3(value));
                spring.as_animation()
            }
            _ => unreachable!("layout spring and property have matching value kinds"),
        }
    }
}

/// Holds the scene's shared animation templates.
struct Templates {
    settle: windows_composition::ScalarKeyFrameAnimation,
    scalar: [SpringScalarNaturalMotionAnimation; 2],
    vec2: [SpringVector2NaturalMotionAnimation; 2],
    vec3: [SpringVector3NaturalMotionAnimation; 2],
    track: [ExpressionAnimation; 3],
    follow: [ExpressionAnimation; 4],
    linear: CompositionEasingFunction,
}

impl Templates {
    fn new(back: &Backends) -> Self {
        let comp = &back.compositor;
        // The damping ratio is the tuning's own and never varies with travel, so it is set
        // once here and the period is what a retarget restates.
        Self {
            settle: {
                let animation = comp.create_scalar_key_frame_animation();
                animation.set_duration(Duration::from_millis(1));
                animation
            },
            scalar: core::array::from_fn(|at| {
                let spring = comp.create_spring_scalar_animation();
                spring.set_damping_ratio(SPRING[at][1]);
                spring
            }),
            vec2: core::array::from_fn(|at| {
                let spring = comp.create_spring_vector2_animation();
                spring.set_damping_ratio(SPRING[at][1]);
                spring
            }),
            vec3: core::array::from_fn(|at| {
                let spring = comp.create_spring_vector3_animation();
                spring.set_damping_ratio(SPRING[at][1]);
                spring
            }),
            track: core::array::from_fn(|at| comp.create_expression_animation(TRACK_EXPR[at])),
            follow: core::array::from_fn(|at| comp.create_expression_animation(FOLLOW_EXPR[at])),
            linear: comp.create_linear_easing_function(),
        }
    }

    /// Retargets the shared spring for a tuning and hands it back ready to start.
    ///
    /// A natural-motion entry never receives its target automatically, so the final value is
    /// set explicitly or the spring animates toward zero. The delay is restated on every call
    /// for the same reason the period is: the object is shared, so a wait left on it from one
    /// call would be inherited by the next.
    fn spring(
        &self,
        slot: u8,
        tuning: Tuning,
        to: Value,
        travel: f32,
        delay: Duration,
    ) -> CompositionAnimation {
        let at = usize::from(tuning == Tuning::Scroll);
        let period = Duration::from_secs_f64(f64::from(period_for(tuning, travel)));
        match (slot, to) {
            (1, Value::Scalar(v)) => {
                let spring = &self.scalar[at];
                spring.set_period(period);
                spring.set_delay(delay);
                spring.set_final_value(v);
                spring.as_animation()
            }
            (2, Value::Vec2(v)) => {
                let spring = &self.vec2[at];
                spring.set_period(period);
                spring.set_delay(delay);
                spring.set_final_value(v);
                spring.as_animation()
            }
            (_, value) => {
                let spring = &self.vec3[at];
                spring.set_period(period);
                spring.set_delay(delay);
                spring.set_final_value(match value {
                    Value::Vec2(v) => v3(v),
                    Value::Scalar(v) => Vector3 { x: v, y: v, z: v },
                });
                spring.as_animation()
            }
        }
    }

    /// Binds the shared expression for an axis.
    ///
    /// The instance is shared, so its parameters hold only until the next binding on the
    /// same axis, which is sound because starting snapshots them.
    fn track(
        &self,
        axis: TrackerAxis,
        tracker: &InteractionTracker,
        affine: Affine,
    ) -> CompositionAnimation {
        let expression = &self.track[axis as usize];
        expression.set_reference_parameter("t", tracker);
        expression.set_scalar_parameter("m", affine.m);
        expression.set_scalar_parameter("c", affine.c);
        expression.as_animation()
    }

    fn follow(
        &self,
        vertical: bool,
        source: &Visual,
        affine: Affine,
        extent: Option<(&Visual, f32)>,
        clamp: [f32; 2],
    ) -> CompositionAnimation {
        let expression = &self.follow[usize::from(vertical) + 2 * usize::from(extent.is_some())];
        expression.set_reference_parameter("v", source);
        if let Some((bounds, inset)) = extent {
            expression.set_reference_parameter("b", bounds);
            expression.set_scalar_parameter("inset", inset);
        }
        expression.set_scalar_parameter("m", affine.m);
        expression.set_scalar_parameter("c", affine.c);
        expression.set_scalar_parameter("lo", clamp[0]);
        expression.set_scalar_parameter("hi", clamp[1]);
        expression.as_animation()
    }

    /// The easing object for one key-frame segment.
    ///
    /// Linear is one shared instance, since every linear segment is the same curve; a cubic
    /// carries its own control points and is built per call.
    fn easing(&self, back: &Backends, easing: Easing) -> CompositionEasingFunction {
        match easing {
            Easing::Linear => self.linear.clone(),
            Easing::Cubic(c1, c2) => back.compositor.create_cubic_bezier_easing_function(c1, c2),
        }
    }

    /// Builds a key-frame animation over `frames`.
    ///
    /// Built per call rather than shared: a key-frame animation carries its frames and the
    /// platform offers no way to clear them. That is once per event, not once per frame.
    /// Frames that are all scalar build a scalar animation; any other mix builds a
    /// `Vector3` one, splatting a scalar across all three components.
    fn frames(
        &self,
        back: &Backends,
        frames: &[(f32, Value, Easing)],
        duration_ms: u32,
        iterations: Iterations,
        scalar: bool,
    ) -> CompositionAnimation {
        let duration = Duration::from_millis(u64::from(duration_ms));
        // Stated before the animation starts, and a count the platform's `i32` cannot hold
        // saturates rather than wrapping into a shorter run than was asked for.
        let count = |n: u32| i32::try_from(n).unwrap_or(i32::MAX);
        if scalar {
            let animation = back.compositor.create_scalar_key_frame_animation();
            for &(at, value, easing) in frames {
                let Value::Scalar(v) = value else { continue };
                animation.insert_key_frame_with_easing(at, v, &self.easing(back, easing));
            }
            animation.set_duration(duration);
            match iterations {
                Iterations::Forever => animation.set_iterate_forever(),
                Iterations::Count(n) => animation.set_iteration_count(count(n)),
            }
            animation.as_animation()
        } else {
            let animation = back.compositor.create_vector3_key_frame_animation();
            for &(at, value, easing) in frames {
                let v = match value {
                    Value::Vec2(v) => v3(v),
                    Value::Scalar(v) => Vector3 { x: v, y: v, z: v },
                };
                animation.insert_key_frame_with_easing(at, v, &self.easing(back, easing));
            }
            animation.set_duration(duration);
            match iterations {
                Iterations::Forever => animation.set_iterate_forever(),
                Iterations::Count(n) => animation.set_iteration_count(count(n)),
            }
            animation.as_animation()
        }
    }
}

/// The spring period for `tuning`.
///
/// Chrome scales with travel; layout uses that curve at its base period for every bound.
fn period_for(tuning: Tuning, travel: f32) -> f32 {
    let base = SPRING[usize::from(tuning == Tuning::Scroll)][0];
    match tuning {
        Tuning::Scroll | Tuning::Layout => base,
        Tuning::Chrome if !travel.is_finite() => base,
        Tuning::Chrome => base * (travel.abs() / CHROME_REF_TRAVEL).clamp(0.7, 1.4),
    }
}

/// What one in-flight batch is holding alive.
enum PendingKind {
    /// The flattened capture, on screen for as long as the exit plays. Held because nothing
    /// else does: a ghost is unparented from the model's tree by construction.
    Ghost(Visual),
    Collapse { id: NodeId, parent: NodeId, carrier: ContainerVisual, resources: Vec<Aux> },
    /// An animation whose target has been collected is dropped by the compositor and the
    /// batch then never reports, so the scratch target is held.
    Delay(
        DelayId,
        #[expect(
            dead_code,
            reason = "holds the animation's target alive until it reports"
        )]
        CompositionPropertySet,
    ),
    Frames(NodeId, Prop),
    /// Scalar settlement identities, released only after their finite animations finish.
    Restate(Vec<u64>),
}

/// A batch whose completion is the only report that the work it holds has run.
///
/// One shape serves an exit ghost, a timed reveal and a finite key-frame run: each holds
/// what it must keep alive, the batch, and the completion revoker. Dropped early the
/// completion never arrives, and a batch subscribed to but never sealed swallows later
/// animations while one sealed with no subscriber never reports, so the pair is armed
/// together or not at all.
struct Pending {
    done: Rc<Cell<bool>>,
    holds: PendingKind,
    _batch: CompositionScopedBatch,
    _revoker: EventRevoker,
}

struct Motion {
    templates: Templates,
    pending: Vec<Pending>,
    settle_serial: u64,
}

impl Motion {
    /// Starts an animation inside a scoped batch and holds the completion subscription.
    ///
    /// Released on the batch's own completion signal, never on a timer or an estimated
    /// deadline.
    fn watch(&mut self, back: &Backends, holds: PendingKind, start: impl FnOnce()) -> Result<()> {
        let batch = back.compositor.create_scoped_batch(BatchKind::Animation);
        let done = Rc::new(Cell::new(false));
        let signal = Rc::clone(&done);
        let revoker = batch.on_completed(move || signal.set(true))?;
        start();
        batch.try_end()?;
        self.pending.push(Pending {
            done,
            holds,
            _batch: batch,
            _revoker: revoker,
        });
        Ok(())
    }
}

// ── trackers ────────────────────────────────────────────────────────────────────────

/// How many requests can be outstanding before the oldest is forgotten.
///
/// A drag issues one request per frame and every values-changed callback clears the set, so
/// it holds a frame or two of latency and a linear scan resolves a reply; a map would
/// allocate on the drag path.
const PENDING_REQUESTS: usize = 8;

struct TrackerState {
    inner: InteractionTracker,
    /// Held because a captured contact is redirected into it, and because dropping it while
    /// the tracker is live leaves the source unreachable.
    source: Option<VisualInteractionSource>,
    /// The group this tracker scrolls, so a reported position has somewhere to land.
    viewport: NodeId,
    /// The last values a callback reported, and the only sound read: the tracker runs in
    /// another process, every call and callback is asynchronous, and its own getter answers
    /// with whatever was last set.
    position: Vector2,
    scale: f32,
    phase: Phase,
    /// The reported position again, in one word, for a hit test that runs wherever the
    /// contact arrived.
    shadow: Arc<AtomicU64>,
    /// The range the layout stated, before any extra extent is added to it.
    bounds: (Vector2, Vector2),
    /// Extent held past the stated maximum for as long as something occludes the viewport.
    extra: Vector2,
    pending: [Option<(i32, TrackerRequest)>; PENDING_REQUESTS],
}

impl TrackerState {
    /// Applies the stated range with whatever extra extent is held on top of it.
    ///
    /// The two are kept apart so that a layout restating its extents cannot drop the extra,
    /// and so that clearing the extra restores the layout's own maximum exactly.
    fn apply_bounds(&self) {
        let (min, max) = self.bounds;
        self.inner.set_position_bounds(
            v3(min),
            v3(Vector2 {
                x: max.x + self.extra.x,
                y: max.y + self.extra.y,
            }),
        );
    }

    /// Issues a request and holds it against its id.
    ///
    /// A request is not an assignment: a position update arriving while the user is
    /// manipulating is documented as dropped, and the tracker reports the drop only as an
    /// ignored request.
    fn request(&mut self, request: TrackerRequest) -> Result<i32> {
        let id = match request {
            TrackerRequest::By(delta) => self
                .inner
                .try_update_position_by(v3(delta), Clamping::Auto)?,
            // The scale policy is stated rather than defaulted, because the default
            // silently stops a running custom scale animation.
            TrackerRequest::To(to) => self.inner.try_update_position(
                v3(to),
                Clamping::Auto,
                ScaleAnimationPolicy::Keep,
            )?,
            TrackerRequest::Fling(velocity) => self
                .inner
                .try_update_position_with_additional_velocity(v3(velocity))?,
        };
        self.remember(id.0, request);
        Ok(id.0)
    }

    fn remember(&mut self, id: i32, request: TrackerRequest) {
        if let Some(slot) = self.pending.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some((id, request));
            return;
        }
        // A full set means the tracker has not reported in several frames. The oldest entry
        // is the least useful one to keep, and the drag path allocates nothing.
        self.pending.rotate_left(1);
        self.pending[PENDING_REQUESTS - 1] = Some((id, request));
    }

    /// The request is not re-applied: re-applying it would jump a second time once the
    /// user's manipulation ends.
    fn ignored(&mut self, id: i32) {
        for slot in &mut self.pending {
            if slot.is_some_and(|(pending, _)| pending == id) {
                *slot = None;
            }
        }
    }

    fn values_changed(&mut self, position: Vector2, scale: f32) {
        self.position = position;
        self.scale = scale;
        // Release, pairing with the acquire in the hit query's shadow read.
        self.shadow
            .store(pack_offset(position.x, position.y), Ordering::Release);
        self.pending = [None; PENDING_REQUESTS];
    }
}

/// Configures the source a manipulation is collected on.
///
/// The source visual is both the hit-test target and the gesture's coordinate space, so it
/// must not move during the manipulation: it is the viewport and never the content scrolling
/// inside it.
fn configure_source(viewport: &Visual, axes: Axes) -> Result<VisualInteractionSource> {
    // A source visual with a zero size does not hit-test correctly, and a zero-size viewport
    // is a bug rather than a no-op: it ignores every wheel notch while every call reports
    // success.
    debug_assert!(
        viewport.size().x > 0.0 && viewport.size().y > 0.0,
        "a tracker's viewport must be sized before its source is created"
    );
    let mode = |on: bool| {
        if on {
            SourceMode::EnabledWithInertia
        } else {
            SourceMode::Disabled
        }
    };
    let source = VisualInteractionSource::for_visual(viewport)?;
    source.set_axis_modes(mode(axes.x), mode(axes.y), mode(axes.scale));
    // A pan started primarily on one axis locks to it, so a vertical list does not drift.
    // Meaningful only while both axes are live.
    source.set_rails(axes.x && axes.y, axes.x && axes.y);
    // Touch and pen must be explicitly redirected and mouse cannot be redirected at all;
    // precision-touchpad input arrives automatically.
    source.set_redirection_mode(RedirectionMode::TouchpadAndWheel);
    // Nested scrollers hand off at bounds with no hand-written plumbing.
    source.set_chaining(ChainingMode::Auto, ChainingMode::Auto, ChainingMode::Auto);
    // Wheel drives Y only, compositor-side, so a wheel over a scroll container needs no
    // handling on any thread of ours.
    source.set_wheel_modes(
        WheelMode::Disabled,
        if axes.y {
            WheelMode::Enabled
        } else {
            WheelMode::Disabled
        },
        WheelMode::Disabled,
    )?;
    Ok(source)
}

/// Translates a wrapper tracker event into this crate's, tagged with the tracker it came
/// from.
fn translate(tracker: TrackerId<()>, event: TrackerEvent) -> SceneEvent {
    let flat = |v: Vector3| Vector2 { x: v.x, y: v.y };
    match event {
        TrackerEvent::ValuesChanged {
            position, scale, ..
        } => SceneEvent::TrackerValues {
            tracker,
            position: flat(position),
            scale,
        },
        TrackerEvent::IdleStateEntered { .. } => SceneEvent::TrackerPhase {
            tracker,
            phase: Phase::Idle,
        },
        TrackerEvent::InteractingStateEntered { .. } => SceneEvent::TrackerPhase {
            tracker,
            phase: Phase::Interacting,
        },
        TrackerEvent::CustomAnimationStateEntered { .. } => SceneEvent::TrackerPhase {
            tracker,
            phase: Phase::CustomAnimation,
        },
        TrackerEvent::InertiaStateEntered {
            modified_resting_position,
            from_impulse,
            ..
        } => SceneEvent::InertiaBegan {
            tracker,
            rest: flat(modified_resting_position),
            from_wheel: from_impulse,
        },
        TrackerEvent::RequestIgnored { request } => SceneEvent::RequestIgnored {
            tracker,
            request: request.0,
        },
    }
}

// ── the window's ground ─────────────────────────────────────────────────────────────

/// One radial layer of the ground.
#[derive(Clone, PartialEq, Debug)]
pub struct Glow {
    /// Centre to edge. The last stop should be transparent, or the blob ends on a visible
    /// edge where its tile does.
    pub stops: Vec<(u16, windows_color::Radiance)>,
    pub at: Vector2,
    pub size: Vector2,
}

/// What an application says the window's ground looks like.
///
/// Minted before the window is shown and not an arena node: it sits under everything the
/// model names, layout cannot reach it, and it is not in the hit array. A content surface
/// arrives only after its size is delivered and the request is serviced on a later commit,
/// so a backdrop authored as content cannot exist on the first composited frame.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct BackdropSpec {
    /// Top to bottom, covering the whole window. Empty for no base, which leaves whatever is
    /// behind the window showing through.
    pub base: Vec<(u16, windows_color::Radiance)>,
    pub glows: Vec<Glow>,
    /// Whether the ground carries a dither.
    ///
    /// A ground is the one surface in a window wide enough for an eight-bit quantiser to
    /// show: a near-black ramp over a whole window height spans a handful of codes, and
    /// the flat region between two of them is hundreds of pixels wide. Setting this asks
    /// the scene for a grain that breaks those contours — on the desktops that have them.
    /// Where composition is float there is no quantiser and the flag costs nothing, and a
    /// ground with no opaque base has nothing to correct, so both answer no on their own.
    pub dither: bool,
}

/// The window's ground, as one sprite per layer.
///
/// Every layer is a ramp stretched to fill, so no layer carries the window's extent: a
/// resize re-points no surface and re-rasterizes nothing, because every box is stated as
/// fractions of the band above it and the compositor re-derives them from the one extent the
/// root carries. Layers composite source-over, which is associative, so splitting them
/// across sprites is the identical composite one surface would perform in sequence.
#[derive(Default)]
struct Backdrop {
    spec: BackdropSpec,
    sprites: Vec<SpriteVisual>,
    /// The dither, on top of the layers it corrected.
    grain: Option<SpriteVisual>,
    /// Physical pixels the grain's surface covers. **Grown and never shrunk**: it is the
    /// one thing in the ground that carries an extent, so a drag-resize would re-rasterize
    /// a screen-sized surface per step. Allocated in whole tiles past the window instead,
    /// and the sprite clips it back to the window.
    px: (i32, i32),
}

impl Backdrop {
    fn build(&mut self, back: &Backends, env: Env) -> Result<()> {
        self.sprites.clear();
        self.grain = None;
        // The grain and the correction under it are one decision. A float desktop has no
        // quantiser to break, and a ground with no opaque base is not a convex combination,
        // so the correction would not compose. Either answer leaves the ground as it was.
        let grain = self
            .spec
            .dither
            .then(|| env.output().quantum())
            .flatten()
            .zip(self.spec.base.first())
            .map(|(quantum, &(_, mid))| (quantum, env.apply(mid)));
        let beneath = grain.map(|(_, mid)| Beneath {
            alpha: GRAIN_ALPHA,
            mid,
        });
        let base = (!self.spec.base.is_empty()).then(|| {
            (
                &self.spec.base[..],
                Spread::Vertical,
                Vector2 { x: 0.5, y: 0.5 },
                Vector2 { x: 1.0, y: 1.0 },
            )
        });
        let glows = self
            .spec
            .glows
            .iter()
            .map(|glow| (&glow.stops[..], Spread::Radial, glow.at, glow.size));
        for (stops, spread, at, size) in base.into_iter().chain(glows) {
            let Some(surface) = back.raster_ramp(stops, spread, env, beneath)? else {
                continue;
            };
            let sprite = back.compositor.create_sprite_visual();
            sprite.set_brush(&back.brush(&surface, Stretch::Fill));
            place(&sprite, at, size);
            self.sprites.push(sprite);
        }
        if let Some((_, mid)) = grain {
            self.cut_grain(back, env, mid)?;
        }
        Ok(())
    }

    /// Cuts the grain for the extent the ground was last told about.
    ///
    /// The sprite tracks the window through its relative size, and its brush paints the
    /// surface one texel to one pixel from the top left, so an allocation larger than the
    /// window is simply clipped by the sprite and a smaller window needs no work at all.
    fn cut_grain(
        &mut self,
        back: &Backends,
        env: Env,
        mid: windows_color::Scrgb,
    ) -> Result<()> {
        let px = (self.px.0.max(GRAIN_TILE as i32), self.px.1.max(GRAIN_TILE as i32));
        let bands = self.grain_bands(env);
        let Some(surface) = back.raster_grain(px, mid, &bands)? else {
            return Ok(());
        };
        let sprite = back.compositor.create_sprite_visual();
        let brush = back.brush(&surface, Stretch::None);
        // One texel on one PHYSICAL pixel. The sprite's box is in DIPs under a root
        // carrying the display scale, so a brush left at unit scale paints one texel per
        // DIP and the grain comes out magnified — resampled, which is a blur of a grain
        // and not a dither. The same reciprocal a presented region takes, for the same
        // reason; the surface is already sized in pixels.
        scale_pixels(&brush, env);
        sprite.set_brush(&brush);
        sprite.set_relative_size_adjustment(Vector2 { x: 1.0, y: 1.0 });
        sprite.set_opacity(GRAIN_ALPHA);
        self.grain = Some(sprite);
        Ok(())
    }

    /// The grain's peak-to-peak amplitude for each horizontal strip, per channel, in
    /// display-referred light.
    ///
    /// A code spans more linear light the higher it sits, so an amplitude picked at one
    /// level is the right size there and short everywhere brighter — over this ground, by
    /// half as much again. The step the quantiser takes at the ground's own level is
    /// therefore read per strip, which is enough because the ground's level follows its
    /// glows: down a column it moves by about a fifth, across a row by a twentieth.
    ///
    /// A strip takes its own **maximum**, because the two errors are not the same. Too much
    /// grain costs a little more of something already below a code; too little lets a
    /// contour stand, which is the whole reason the layer is here.
    fn grain_bands(&self, env: Env) -> Vec<[f32; 3]> {
        let out = env.output();
        let Some(base) = self.ladder(&self.spec.base) else {
            return Vec::new();
        };
        let glows: Vec<(Vec<(f32, windows_color::Radiance)>, Vector2, Vector2)> = self
            .spec
            .glows
            .iter()
            .filter_map(|glow| Some((self.ladder(&glow.stops)?, glow.at, glow.size)))
            .collect();
        // The ground's own colour where the window is `at`, as a fraction of it. An
        // estimate: the compositor samples each layer from a stretched tile and this walks
        // the stops directly, which agree far closer than an amplitude needs.
        // Channels rather than a colour: this composites values the transform has already
        // produced, the way the compositor does, and the only thing read back out of it is
        // how big a code is at that level.
        let ground = |at: Vector2| -> [f32; 3] {
            let base = env.apply(windows_color::Radiance::sample(&base, at.y));
            let mut v = [base.r, base.g, base.b];
            for (ladder, centre, size) in &glows {
                let dx = (at.x - centre.x) / (size.x * 0.5).max(f32::EPSILON);
                let dy = (at.y - centre.y) / (size.y * 0.5).max(f32::EPSILON);
                let light = env.apply(windows_color::Radiance::sample(
                    ladder,
                    dx.hypot(dy).clamp(0.0, 1.0),
                ));
                for (v, s) in v.iter_mut().zip([light.r, light.g, light.b]) {
                    *v = light.a * s + (1.0 - light.a) * *v;
                }
            }
            v
        };
        (0..GRAIN_STRIPS)
            .map(|strip| {
                let mut peak = [0.0f32; 3];
                // The corners and the middle of the strip. A glow lifts hardest over its own
                // centre and sinks hardest there too, so the extremes of a row are either
                // under a centre or out at the edges, and both are sampled.
                for step in 0..=2 {
                    let y = (strip as f32 + 0.5 * step as f32) / GRAIN_STRIPS as f32;
                    for column in 0..=4 {
                        let at = Vector2 {
                            x: column as f32 / 4.0,
                            y: y.min(1.0),
                        };
                        for (peak, v) in peak.iter_mut().zip(ground(at)) {
                            *peak = peak.max(v);
                        }
                    }
                }
                peak.map(|level| GRAIN_CODES * out.quantum_at(level).unwrap_or(0.0))
            })
            .collect()
    }

    /// A layer's stops as fractions, or `None` where it has none to sample.
    fn ladder(
        &self,
        stops: &[(u16, windows_color::Radiance)],
    ) -> Option<Vec<(f32, windows_color::Radiance)>> {
        (!stops.is_empty())
            .then(|| stops.iter().map(|&(at, l)| (stop_fraction(at), l)).collect())
    }

    /// Tells the ground how much room it has, and re-cuts the grain where it outgrew its
    /// allocation.
    ///
    /// Returns whether anything was rebuilt, so the caller can reseat the band only when
    /// the tree changed.
    fn resize(&mut self, px: (i32, i32), back: &Backends, env: Env) -> Result<bool> {
        // Rounded up to whole tiles, so a drag past the edge re-cuts once per tile rather
        // than once per pixel, and the grain stays aligned to the same texel grid.
        let step = GRAIN_TILE as i32;
        let up = |v: i32| (v.max(1) + step - 1) / step * step;
        let want = (up(px.0), up(px.1));
        if want.0 <= self.px.0 && want.1 <= self.px.1 {
            return Ok(false);
        }
        self.px = (self.px.0.max(want.0), self.px.1.max(want.1));
        if self.grain.is_none() {
            return Ok(false);
        }
        let (Some(&(_, mid)), Some(_)) = (self.spec.base.first(), env.output().quantum())
        else {
            return Ok(false);
        };
        let mid = env.apply(mid);
        self.grain = None;
        self.cut_grain(back, env, mid)?;
        Ok(true)
    }

    /// The ground's layers in composite order: the base, its glows, and the grain on top.
    ///
    /// The grain is last because it corrects what is under it and nothing else — a layer
    /// added above it would arrive un-pre-compensated and read `1/16` of the way toward
    /// the grain's own midpoint.
    fn layers(&self) -> impl Iterator<Item = &SpriteVisual> {
        self.sprites.iter().chain(self.grain.as_ref())
    }

    /// `index` is into the spec's glows; out of range is ignored rather than panicking.
    fn move_glow(&mut self, index: usize, at: Vector2) {
        let base = usize::from(!self.spec.base.is_empty());
        if let (Some(glow), Some(sprite)) = (
            self.spec.glows.get_mut(index),
            self.sprites.get(index + base),
        ) {
            glow.at = at;
            place(sprite, at, glow.size);
        }
    }
}

/// Centres a layer on its fractional position.
///
/// Both the extent and the offset are pure fractions: centring `size` on `at` is
/// `window * at - window * size / 2` and the window cancels, so the compositor re-derives the
/// box from its parent's extent and a resize writes no property here, for any number of
/// glows.
fn place(sprite: &SpriteVisual, at: Vector2, size: Vector2) {
    sprite.set_relative_size_adjustment(size);
    sprite.set_relative_offset_adjustment(Vector3 {
        x: at.x - size.x * 0.5,
        y: at.y - size.y * 0.5,
        z: 0.0,
    });
}

// ── what the scene reports upward ───────────────────────────────────────────────────

/// The only channel up, as the patch is the only channel down. Solved layout crosses in
/// neither direction: it becomes bind and hit ops inside the patch.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SceneEvent {
    /// A finite key-frame binding's scoped batch completed. Node generations reject stale
    /// reports.
    AnimationCompleted {
        node: NodeId,
        prop: Prop,
    },
    /// The only accurate read of a tracker: it evaluates in another process and its getter
    /// answers with the value last set rather than the value being evaluated.
    TrackerValues {
        tracker: TrackerId<()>,
        position: Vector2,
        scale: f32,
    },
    TrackerPhase {
        tracker: TrackerId<()>,
        phase: Phase,
    },
    /// Inertia began with the resting position already known, so a consumer can prefetch the
    /// destination while the compositor animates toward it. `rest` is the position with snap
    /// points applied.
    InertiaBegan {
        tracker: TrackerId<()>,
        rest: Vector2,
        /// Whether the motion came from a wheel notch rather than a fling, which a caller
        /// can decay faster.
        from_wheel: bool,
    },
    /// Not an error: a position update arriving while the user is manipulating is ignored. A
    /// caller drops the request and reconciles against the next values report.
    RequestIgnored {
        tracker: TrackerId<()>,
        request: i32,
    },
    /// A timed reveal has run. Raised from the completion of the delay's own compositor
    /// animation, so nothing on this thread measures the wait; a cancelled delay is never
    /// reported.
    DelayElapsed(DelayId),
    DeviceRebuilt,
    /// The pixel grid moved, leaving resources rasterized at device resolution built for the
    /// wrong one. Coverage tiles are the only such resource — a gradient is a fixed strip
    /// stretched to fill, geometry is vector, a colour cell is four texels of one value — so
    /// the consumer re-emits every run.
    ScaleChanged {
        scale: f32,
    },
}

// ── the scene ───────────────────────────────────────────────────────────────────────

/// The retained tree, the patch applier, motion, trackers and the ground.
///
/// `!Send` by construction, so the thread rule is a compile error rather than a convention.
pub struct Scene {
    #[expect(
        dead_code,
        reason = "held to keep the visual tree attached to the window"
    )]
    target: DesktopWindowTarget,
    /// The root carries the DIP-to-pixel factor; the three bands under it are not arena
    /// nodes, because the arena holds only the nodes the model named.
    root: ContainerVisual,
    ground: ContainerVisual,
    content: ContainerVisual,
    /// Slot roots and the ghosts an exit leaves behind, so an overlay sits above content by
    /// its position in the tree rather than by an ordering every caller keeps.
    overlay: ContainerVisual,
    lift: Option<Box<drag_preview::Lift>>,
    lift_epoch: u64,
    backdrop: Backdrop,
    nodes: Arena,
    /// Every node with no parent node, in attachment order. A forest and not a tree: a slot
    /// root is a second root rather than a child placed oddly.
    roots: Vec<NodeId>,
    res: Resources,
    cache: Cache,
    generation: Gen,
    /// `None` until the first operation states one; read only by the comparison that decides
    /// what a display move invalidated.
    env: Option<Env>,
    motion: Motion,
    springs_enabled: bool,
    size_observers: Vec<(NodeId, crate::size_observer::Observer)>,
    trackers: Slots<TRACKER, TrackerState>,
    events: Rc<RefCell<Vec<SceneEvent>>>,
    hits: HitTable,
    census: Census,
    /// Channels a front-side retarget has claimed.
    ///
    /// A front-side write cannot tear the shadow — one shadow, one setter, one thread — so
    /// the set catches the semantic conflict instead: the app writing a channel the router
    /// is driving, such as a thumb offset set mid-drag. Debug builds only.
    #[cfg(debug_assertions)]
    claimed: rustc_hash::FxHashSet<(NodeId, Prop)>,
    _not_send: core::marker::PhantomData<*const ()>,
}

impl Scene {
    /// Observes native layout bounds without writing layout or animated properties.
    pub fn observe_size(&mut self, node: NodeId, region: Option<RegionId>, publish: impl FnMut(Vector2, Vector2, Vector2, bool) + 'static, back: &Backends) -> Result<()> {
        let brush = region.map(|region| self.res.brush(region.erased()).ok_or_else(invalid_arg)).transpose()?;
        let observer = crate::size_observer::Observer::new(&back.compositor, node, &self.nodes, brush, publish)?;
        self.size_observers.retain(|(held, _)| *held != node);
        self.size_observers.push((node, observer));
        Ok(())
    }

    /// Suspends or resumes the native observer for a mounted region.
    pub fn observe_size_active(&mut self, node: NodeId, active: bool) -> Result<()> {
        if let Some((_, observer)) = self.size_observers.iter_mut().find(|(held, _)| *held == node) {
            observer.active(active)?;
        }
        Ok(())
    }

    /// Releases an observer and rejects its late native callbacks.
    pub fn forget_size(&mut self, node: NodeId) {
        self.size_observers.retain(|(held, _)| *held != node);
    }

    /// Brings up the retained tree for a window another thread owns.
    ///
    /// The compositor is agile and a desktop target asks only for a dispatcher queue on the
    /// thread that created it, so the token names the window and the calling thread owns
    /// everything made here; the window's own thread keeps the pump and the input and holds
    /// no composition object at all.
    ///
    /// # Errors
    ///
    /// Fails if the window target or the backdrop cannot be created.
    pub fn new_at(
        window: windows_window::Hwnd,
        back: &Backends,
        env: Env,
        spec: BackdropSpec,
    ) -> Result<Self> {
        let target = back
            .compositor
            .create_desktop_window_target_for(window, false)?;
        let root = back.compositor.create_container_visual();
        target.set_root(&root);
        set_dip_space(&root, env.scale());
        // Three bands, bottom to top: the ground, the content, the overlays. Their order is
        // fixed by the tree, so `after: None` means the bottom of one band's collection
        // rather than the bottom of the window. Each is the window, stated once as a
        // fraction of the root, so a resize writes no property here.
        let ground = back.compositor.create_container_visual();
        let content = back.compositor.create_container_visual();
        let overlay = back.compositor.create_container_visual();
        let bands = root.children();
        for band in [&ground, &content, &overlay] {
            band.set_relative_size_adjustment(Vector2 { x: 1.0, y: 1.0 });
            bands.insert_at_top(band);
        }
        let mut backdrop = Backdrop {
            spec,
            sprites: Vec::new(),
            grain: None,
            px: (0, 0),
        };
        backdrop.build(back, env)?;
        let layers = ground.children();
        for sprite in backdrop.layers() {
            layers.insert_at_top(sprite);
        }
        Ok(Self {
            target,
            root,
            ground,
            content,
            overlay,
            lift: None,
            lift_epoch: 0,
            backdrop,
            nodes: Arena::default(),
            roots: Vec::new(),
            res: Resources::default(),
            cache: Cache::default(),
            generation: Gen::default(),
            env: Some(env),
            springs_enabled: true,
            size_observers: Vec::new(),
            motion: Motion {
                templates: Templates::new(back),
                pending: Vec::new(),
                settle_serial: 0,
            },
            trackers: Slots::default(),
            events: Rc::default(),
            hits: HitTable::default(),
            census: Census::default(),
            #[cfg(debug_assertions)]
            claimed: rustc_hash::FxHashSet::default(),
            _not_send: core::marker::PhantomData,
        })
    }

    /// The hit array, which every consumer resolves a contact through.
    #[must_use]
    pub fn hits(&self) -> &HitTable {
        &self.hits
    }

    /// Shares a subtree's target displacement with hit consumers.
    pub fn install_translation(&mut self, owner: ControlId, state: &crate::Translation) {
        self.hits.install_translation(owner, state);
    }

    /// Removes a retired subtree's target displacement.
    pub fn remove_translation(&mut self, owner: ControlId) {
        self.hits.remove_translation(owner);
    }

    /// Makes subsequent animation targets land immediately and omits exit transitions
    /// when client-area animation is disabled. Dwell delays retain their timing.
    pub fn set_springs_enabled(&mut self, enabled: bool) {
        self.springs_enabled = enabled;
    }

    /// What is under `p` for `contact`, or `None` if nothing is.
    #[must_use]
    pub fn hit(&self, p: Point, contact: ContactKind) -> Option<Hit> {
        self.hits.hit(p, contact)
    }

    /// The running tallies of what this scene has realized.
    #[must_use]
    pub const fn census(&self) -> &Census {
        &self.census
    }

    /// Walks the forest and reports how many nodes it reaches against how many the arena
    /// holds. Recurses over every node, so it is O(nodes).
    #[must_use]
    pub fn audit(&self) -> Audit {
        fn walk(nodes: &Arena, id: NodeId, reached: &mut u32) {
            *reached += 1;
            for child in children(nodes, id.index() as u32) {
                walk(nodes, child, reached);
            }
        }
        let mut reached = 0;
        for root in &self.roots {
            walk(&self.nodes, *root, &mut reached);
        }
        Audit {
            reached,
            held: self.nodes.len() as u32,
        }
    }

    // ── the pass ────────────────────────────────────────────────────────────────────

    /// Applies a patch under `env` and reports whether anything changed.
    ///
    /// `false` means the pass wrote nothing, so something requested a frame it did not need.
    /// On success the patch is cleared and keeps its buffers, so the caller reuses the
    /// allocations.
    ///
    /// # Errors
    ///
    /// Fails if an op's composition work failed; the ops before it have already applied.
    pub fn apply(&mut self, patch: &mut SinkPatch, back: &Backends, env: Env) -> Result<bool> {
        // Geometry solved under a different display than the one being applied to. The
        // rasters must match the display that is there, so the divergence is counted rather
        // than refused.
        if patch.env.is_some_and(|solved| solved != env) {
            self.census.env_mismatches += 1;
        }
        self.sync(back, env)?;
        // Swept at the top of a pass, so everything in flight is released on its batch's own
        // completion signal and never on a timer.
        self.retire();
        let before = self.census;
        for at in 0..patch.ops().len() {
            self.op(patch.ops()[at], patch, back, env)?;
            self.census.ops_applied += 1;
        }
        self.settle_stopped(back)?;
        patch.clear();
        Ok(self.census.changed_since(&before))
    }

    fn settle_stopped(&mut self, back: &Backends) -> Result<()> {
        self.prune_settlements();
        if self.nodes.restate.is_empty() { return Ok(()); }
        let mut groups = core::mem::take(&mut self.nodes.restate);
        let batch = back.compositor.create_scoped_batch(BatchKind::Animation);
        let done = Rc::new(Cell::new(false));
        let signal = Rc::clone(&done);
        let revoker = batch.on_completed(move || signal.set(true))?;
        let mut tokens = Vec::new();
        for &(id, group) in &groups {
            if !self.nodes.live(id) { continue; }
            for (at, row) in PROPS.iter().enumerate() {
                if row.group != group || row.count != 1 { continue; }
                let value = self.nodes.chan(id, row.chan);
                let animation = &self.motion.templates.settle;
                animation.insert_key_frame(0.0, value);
                animation.insert_key_frame(1.0, value);
                self.motion.settle_serial = self.motion.settle_serial.checked_add(1)
                    .expect("settlement identity exhausted");
                let token = self.motion.settle_serial;
                if self.nodes.begin_settle(id, at as u8, token, &animation.as_animation()) {
                    tokens.push(token);
                    self.census.animations += 1;
                }
            }
        }
        batch.try_end()?;
        groups.clear();
        self.nodes.restate = groups;
        if !tokens.is_empty() {
            self.motion.pending.push(Pending {
                done,
                holds: PendingKind::Restate(tokens),
                _batch: batch,
                _revoker: revoker,
            });
        }
        Ok(())
    }

    fn prune_settlements(&mut self) {
        let nodes = &self.nodes;
        self.motion.pending.retain_mut(|pending| {
            if let PendingKind::Restate(tokens) = &mut pending.holds {
                tokens.retain(|&token| nodes.has_settle(token));
                return !tokens.is_empty();
            }
            true
        });
    }

    /// Brings the tree up to date with `env`, rebinding whatever a display move invalidated.
    ///
    /// Every operation that can rasterize calls this first, so no raster is built against an
    /// environment the scene has not synced to. The first environment is not a change:
    /// nothing has been realized under an older one.
    fn sync(&mut self, back: &Backends, env: Env) -> Result<()> {
        let Some(was) = self.env.replace(env) else {
            return Ok(());
        };
        if was == env {
            return Ok(());
        }
        self.end_drag_preview();
        if was.geometry_moved(env) {
            self.generation.dpi = self.generation.dpi.wrapping_add(1);
            set_dip_space(&self.root, env.scale());
            self.rescale_pixels(env);
            self.events
                .borrow_mut()
                .push(SceneEvent::ScaleChanged { scale: env.scale() });
        }
        // The backdrop is outside the cell cache, so no generation reaches it: the same
        // authored light lands on a different display as a different value.
        if was.light_moved(env) {
            self.generation.color = self.generation.color.wrapping_add(1);
            self.backdrop.build(back, env)?;
            self.reseat_backdrop();
        }
        self.refresh(back, env)
    }

    /// Rebinds every sprite whose realized chain reads a generation that has moved.
    ///
    /// The one response to an invalidation, whichever generation moved: a sprite reads only
    /// the generations its own mask and paint declare, so a light change leaves shapes alone
    /// and a grid change leaves solid fills alone.
    fn refresh(&mut self, back: &Backends, env: Env) -> Result<()> {
        let now = self.generation;
        let stale: Vec<NodeId> = self
            .nodes
            .ids()
            .filter(|id| {
                self.nodes
                    .painted(*id)
                    .is_some_and(|painted| !painted.fresh(now))
            })
            .collect();
        for id in stale {
            self.realize(id, back, env)?;
        }
        Ok(())
    }

    /// Rebuilds everything under a lost device.
    ///
    /// Every brush is a pure function of a cache key or a resource id, so recovery bumps the
    /// device generation, drops the cells and refreshes — the path a DPI change and a first
    /// bind both take, with no per-kind recovery code. Shadowed values, tracker positions
    /// and the hit table live in Rust and need none; the surfaces *behind* shared resources
    /// are the model's to re-emit.
    ///
    /// The device itself is repaired by whoever owns it, before this is called.
    ///
    /// # Errors
    ///
    /// Fails if the ground or a sprite's chain cannot be rebuilt.
    pub fn device_lost(&mut self, back: &Backends, env: Env) -> Result<()> {
        note!("scene", "device loss recovery: the device generation bumps and every cell re-rasterizes");
        self.end_drag_preview();
        self.generation.device = self.generation.device.wrapping_add(1);
        self.cache.clear();
        self.env = Some(env);
        self.backdrop.build(back, env)?;
        self.reseat_backdrop();
        self.refresh(back, env)?;
        // Tracker expressions do not survive the device, so every bound channel is marked
        // for re-issue down the same path its first binding took.
        for id in self.nodes.ids().collect::<Vec<_>>() {
            for desc in &PROPS {
                if self.nodes.held(id, desc) == Held::Bound {
                    self.nodes.set_held(id, desc, Held::Stale);
                }
            }
            if self.nodes.aux(id).is_some_and(|aux| matches!(aux.decl, Clip::RoundedBounds(_))) {
                self.bind_rounded_bounds(id, back);
            }
        }
        self.events.borrow_mut().push(SceneEvent::DeviceRebuilt);
        Ok(())
    }

    fn reseat_backdrop(&mut self) {
        let layers = self.ground.children();
        layers.remove_all();
        for sprite in self.backdrop.layers() {
            layers.insert_at_top(sprite);
        }
    }

    fn realize(&mut self, id: NodeId, back: &Backends, env: Env) -> Result<()> {
        let glow = match self.nodes.painted(id).map(|painted| painted.paint) {
            Some(Paint::Captured { group, .. }) => self.nodes.visual(group.0).cloned(),
            _ => None,
        };
        let mut ctx = Ctx {
            back,
            env,
            generation: self.generation,
            res: &mut self.res,
            cache: &mut self.cache,
            minted: 0,
            freed: 0,
        };
        let realized = realize(&mut self.nodes, id, glow.as_ref(), &mut ctx);
        self.census.visuals_minted += ctx.minted as u64;
        self.census.visuals_live = self
            .census
            .visuals_live
            .saturating_add_signed(ctx.minted - ctx.freed);
        realized
    }

    fn op(&mut self, op: Op, patch: &SinkPatch, back: &Backends, env: Env) -> Result<()> {
        self.preview_before(op);
        match op {
            // The one place that branches on node kind: a sprite visual *is* a container
            // visual, so the destroy, the reorder, the bind and the device-loss rebind all
            // treat the two alike.
            Op::New {
                id,
                kind,
                parent,
                after,
            } => {
                if let Some(parent) = parent.node() {
                    self.pending_retain(|pending| !matches!(pending.holds,
                        PendingKind::Collapse { parent: held, .. } if held == parent));
                }
                let visual = match kind {
                    NodeKind::Group => (*back.compositor.create_container_visual()).clone(),
                    NodeKind::Sprite => (**back.compositor.create_sprite_visual()).clone(),
                };
                self.nodes.place(id, visual, kind);
                self.census.visuals_minted += 1;
                self.census.visuals_live += 1;
                self.reparent(id, parent, after);
            }
            Op::Move { id, parent, after } => self.reparent(id, parent, after),
            Op::Drop {
                id,
                exit,
                origin,
                bounds,
            } => {
                self.exit(id, exit, origin, bounds, back)?;
                self.destroy(id);
                // A claim is keyed by node and only the tree knows which nodes a cascade
                // took; ids are generational, so a surviving claim is never applied to a
                // different node.
                #[cfg(debug_assertions)]
                {
                    let nodes = &self.nodes;
                    self.claimed.retain(|(node, _)| nodes.live(*node));
                }
            }
            Op::Mask { id, mask } => self.declare(id.0, Some(mask), None, back, env)?,
            Op::Paint { id, paint, halo } => {
                self.declare(id.0, None, Some((paint, halo)), back, env)?;
            }
            Op::Clip { id, clip } => self.set_clip(id, clip, back, env)?,
            Op::Bind { id, prop, bind } => {
                // Asserted here and not in `bind`, which the front-side retarget shares: this
                // arm is the only way an application write arrives.
                #[cfg(debug_assertions)]
                assert!(
                    !self.claimed.contains(&(id, prop)),
                    "{prop:?} on {id:?} is driven from the front thread, and the app has just written it"
                );
                self.bind(id, prop, bind, patch, back, env)?;
            }
            Op::Res { id, op } => self.resource(id, op, patch, back, env)?,
            Op::Tracker { id, op } => self.tracker(id, op, back)?,
            Op::Hits { entries, index } => {
                self.hits.replace(patch.hits(entries), patch.index(index));
            }
            Op::Delay { id, ms } => match ms {
                Some(ms) => self.start_delay(id, ms, back)?,
                None => self.cancel_delay(id),
            },
        }
        Ok(())
    }

    /// Parents in the compositor and then in the arena, because the arena chain mirrors what
    /// the compositor holds: a collection insert that cannot happen must not leave the chain
    /// claiming it did.
    ///
    /// Detached roots go to the top of the overlay band, where they stack in the order they
    /// opened, which is the order the hit array scans its tail in.
    fn reparent(&mut self, id: NodeId, parent: Attach, after: Option<NodeId>) {
        let Some(visual) = self.nodes.visual(id).cloned() else {
            return;
        };
        let chrome = matches!(parent, Attach::Chrome(_));
        if let Some(at) = parent.node()
            && !self.nodes.visual(at).is_some_and(|group| group.as_container().is_some())
        {
            return;
        }
        // Fallible: a caller can hold a node whose parent was torn down between two
        // operations, and already-removed is the wanted state. Removed from whichever
        // collection holds it, because a node can move between a group's two bands.
        let _ = self.content.children().try_remove(&visual);
        let _ = self.overlay.children().try_remove(&visual);
        if let Some(held) = visual.parent() {
            let _ = held.children().try_remove(&visual);
        }
        let collection = match parent {
            Attach::Window => self.content.children(),
            Attach::Overlay => self.overlay.children(),
            Attach::Node(at) | Attach::Chrome(at) => {
                // Linked before the visual is placed, so the chain already holds this node
                // when the split below asks whether the group carries chrome.
                self.nodes.set_chrome(id, chrome);
                self.roots.retain(|root| *root != id);
                let after = after.map(|sibling| sibling.index() as u32);
                link(&mut self.nodes, id.index() as u32, at.index() as u32, after);
                // Chrome arriving under a clipped group is what makes the group need its
                // two bands; content arriving never does.
                if chrome {
                    self.split_bands(at);
                }
                match self.nodes.band(at, chrome) {
                    Some(collection) => collection,
                    None => return,
                }
            }
        };
        let below = match parent {
            Attach::Node(at) | Attach::Chrome(at) => self.nodes.below_in_band(at, after, chrome),
            Attach::Window | Attach::Overlay => after,
        }
        .and_then(|sibling| self.nodes.visual(sibling).cloned());
        match below {
            Some(sibling) => collection.insert_above(&visual, &sibling),
            None if matches!(parent, Attach::Overlay) => collection.insert_at_top(&visual),
            None => collection.insert_at_bottom(&visual),
        }
        if parent.node().is_none() {
            unlink(&mut self.nodes, id.index() as u32);
            if !self.roots.contains(&id) {
                self.roots.push(id);
            }
        }
    }

    /// Splits a clipped group carrying chrome into its two bands: the chrome stays on the
    /// group's own visual, and the content and the clip move onto a carrier above it.
    ///
    /// A no-op for a group already split, unclipped, or carrying no chrome, so it is
    /// called from both of the transitions that can make one need splitting — a clip
    /// arriving and chrome arriving — without either knowing about the other.
    ///
    /// The carrier states no transform and no size of its own: its extent is the group's
    /// through a relative size adjustment, so the group's own channels — offset, size,
    /// scale, rotation, opacity, and the springs and expressions driving any of them — go
    /// on moving chrome and content as one. The clip object moves rather than being
    /// rebuilt, so a clip side or radius mid-animation keeps running on the carrier.
    fn split_bands(&mut self, at: NodeId) {
        let Some(group) = self.nodes.visual(at).and_then(Visual::as_container) else {
            return;
        };
        let Some(aux) = self.nodes.aux(at) else {
            return;
        };
        if aux.content.is_some() || aux.clip.is_none() {
            return;
        }
        let kids: Vec<NodeId> = children(&self.nodes, at.index() as u32).collect();
        if !kids.iter().any(|kid| self.nodes.is_chrome(*kid)) {
            return;
        }
        let carrier = group.compositor().create_container_visual();
        carrier.set_relative_size_adjustment(Vector2 { x: 1.0, y: 1.0 });
        let band = carrier.children();
        let own = group.children();
        // Bottom to top, so the carrier holds its content in the order the chain states.
        // Only a visual the group itself holds moves: one lifted into a drag preview or
        // held by a collapse carrier is somewhere else for now, and returns to the band
        // its parent resolves when that ends.
        for kid in kids.iter().filter(|kid| !self.nodes.is_chrome(**kid)) {
            if let Some(visual) = self.nodes.visual(*kid)
                && own.try_remove(visual).is_ok()
                && visual.parent().is_none()
            {
                band.insert_at_top(visual);
            }
        }
        own.insert_at_top(&carrier);
        let aux = self.nodes.aux_mut(at);
        if let Some(clip) = &aux.clip {
            group.clear_clip();
            clip.apply(&carrier);
        }
        aux.content = Some(carrier);
        note!("scene", "clip id={} split: chrome stays on the group, content and clip move to a carrier", at.index());
        self.census.visuals_minted += 1;
        self.census.visuals_live += 1;
    }

    /// Raises a detached overlay visual above the other overlays.
    ///
    /// `id` must identify a root attached to the overlay band.
    pub fn raise_overlay(&mut self, id: NodeId) {
        if self.roots.contains(&id) {
            self.reparent(id, Attach::Overlay, None);
        }
    }

    /// Destroys a node *and its subtree*, releasing every resource on the way down, so a
    /// subtree removal is one op and a partial destroy is unrepresentable.
    ///
    /// Recurses over the child chain; the depth it reaches is layout nesting.
    fn destroy(&mut self, id: NodeId) {
        let Some(visual) = self.nodes.visual(id).cloned() else {
            return;
        };
        let parent = self.nodes.links(id.index() as u32).parent;
        // The band the node was attached in: a split group holds its content in a carrier.
        let held = (parent != NO_LINK)
            .then(|| self.nodes.id_at(parent))
            .and_then(|at| self.nodes.band(at, self.nodes.is_chrome(id)));
        if let Some(band) = held {
            let _ = band.try_remove(&visual);
        } else {
            // A root sits in a band rather than under a node, and which band is not recorded.
            let _ = self.content.children().try_remove(&visual);
            let _ = self.overlay.children().try_remove(&visual);
        }
        self.release_subtree(id);
    }

    fn release_subtree(&mut self, id: NodeId) {
        let mut resources = self.collapsing(id).then(Vec::new);
        self.release_subtree_held(id, &mut resources);
        if let Some(Pending { holds: PendingKind::Collapse { resources: held, .. }, .. }) =
            self.motion.pending.iter_mut().find(|pending| matches!(pending.holds,
                PendingKind::Collapse { id: root, .. } if root == id))
        {
            *held = resources.unwrap_or_default();
        }
    }

    fn release_subtree_held(&mut self, id: NodeId, resources: &mut Option<Vec<Aux>>) {
        self.pending_retain(|pending| !matches!(pending.holds,
            PendingKind::Collapse { parent, .. } if parent == id));
        self.size_observers.retain(|(node, _)| *node != id);
        for child in children(&self.nodes, id.index() as u32).collect::<Vec<_>>() {
            self.release_subtree_held(child, resources);
        }
        // Captures retain the COM tree; releasing arena rows must not remove its children.
        unlink(&mut self.nodes, id.index() as u32);
        self.roots.retain(|root| *root != id);
        self.pending_retain(
            |pending| !matches!(pending.holds, PendingKind::Frames(node, _) if node == id),
        );
        let (aux, painted) = self.nodes.free(id);
        // A split group's content carrier goes with its visual.
        if aux.as_ref().is_some_and(|aux| aux.content.is_some()) {
            self.census.visuals_live = self.census.visuals_live.saturating_sub(1);
        }
        if let (Some(aux), Some(resources)) = (aux, resources) { resources.push(aux); }
        self.prune_settlements();
        if let Some(painted) = painted {
            self.res.release(painted.mask.holds());
            self.res.release(painted.mask.holds_dash());
            self.res.release(painted.paint.holds());
        }
        self.census.visuals_live = self.census.visuals_live.saturating_sub(1);
    }

    /// Detaches the subtree and keeps it on screen for the length of the exit.
    ///
    /// Fade, scale and slide use a top-level capture. Collapse keeps the native descendants
    /// under their original parent and contracts a clip without resampling their pixels.
    ///
    /// The geometry comes from the op: the app already solved this rect and this clip chain,
    /// so nothing here re-derives them from the tree it is dismantling.
    fn exit(
        &mut self,
        id: NodeId,
        exit: Exit,
        origin: Point,
        bounds: Option<[f32; 4]>,
        back: &Backends,
    ) -> Result<()> {
        if !self.springs_enabled || matches!(exit, Exit::None) {
            return Ok(());
        }
        let Some(source) = self.nodes.visual(id).cloned() else {
            return Ok(());
        };
        let size = self.nodes.size(id);
        // Nothing to capture, so nothing to fade: a zero-size ghost would be a visual and a
        // batch held open for an animation with no pixels in it.
        if size.x <= 0.0 || size.y <= 0.0 {
            return Ok(());
        }
        if exit == Exit::Collapse {
            return self.collapse_exit(id, &source, size, back);
        }
        // Capture bounds include the static halos the subtree casts: a blur reaches past the
        // box it is cast from, and the app's solved rect does not know the sigma.
        let pad = self.halo_margin(id);
        let extent = Vector2 {
            x: size.x + pad * 2.0,
            y: size.y + pad * 2.0,
        };
        // The source subtree's geometry is in DIPs and its window scale is inherited by the
        // new sprite, so multiplying the capture region by that scale would shrink it.
        let capture = back.compositor.capture(&source, extent, 1.0);
        capture
            .surface
            .set_source_offset(Vector2 { x: -pad, y: -pad });
        let sprite = back.compositor.create_sprite_visual();
        sprite.set_brush(&capture.brush);
        let at = Vector3 {
            x: origin.x - pad,
            y: origin.y - pad,
            z: 0.0,
        };
        sprite.set_offset(at.x, at.y, at.z);
        sprite.set_size(extent.x, extent.y);
        sprite.set_center_point(Vector3 {
            x: pad + size.x * 0.5,
            y: pad + size.y * 0.5,
            z: 0.0,
        });
        // An inherited rectangle clip stays on a stationary parent while the one sprite
        // translates inside it, which is the only case a ghost costs a second visual.
        let mounted = match bounds {
            Some(rect) => {
                let host = back.compositor.create_container_visual();
                let clip = back.compositor.create_rectangle_clip();
                clip.set_sides(rect[0], rect[1], rect[2].max(rect[0]), rect[3].max(rect[1]));
                host.set_clip(Some(&clip));
                host.children().insert_at_top(&sprite);
                (*host).clone()
            }
            None => (**sprite).clone(),
        };
        self.overlay.children().insert_at_top(&mounted);
        let minted = 1 + u32::from(bounds.is_some());
        self.census.visuals_minted += u64::from(minted);
        self.census.visuals_live += minted;

        // A slide's `by` is a multiple of the dying subtree's own size.
        let (path, frame) = match exit {
            Exit::Fade { ms } => ("Opacity", (Value::Scalar(0.0), Easing::Linear, ms)),
            Exit::Scale { to, ms } => (
                "Scale",
                (Value::Vec2(Vector2 { x: to, y: to }), Easing::Linear, ms),
            ),
            Exit::Slide { by, ms, easing } => (
                "Offset",
                (
                    Value::Vec2(Vector2 {
                        x: at.x + by.x * size.x,
                        y: at.y + by.y * size.y,
                    }),
                    easing,
                    ms,
                ),
            ),
            Exit::None | Exit::Collapse => unreachable!("returned above"),
        };
        let scalar = matches!(frame.0, Value::Scalar(_));
        let animation = self.motion.templates.frames(
            back,
            &[(1.0, frame.0, frame.1)],
            frame.2,
            Iterations::Count(1),
            scalar,
        );
        self.census.animations += 1;
        self.motion.watch(back, PendingKind::Ghost(mounted), || {
            sprite.start_animation(path, &animation);
        })
    }

    fn collapse_exit(&mut self, id: NodeId, source: &Visual, size: Vector2, back: &Backends) -> Result<()> {
        let at = self.nodes.links(id.index() as u32).parent;
        if at == NO_LINK { return Ok(()); }
        let parent = self.nodes.id_at(at);
        let Some(group) = source.parent() else { return Ok(()); };
        let carrier = back.compositor.create_container_visual();
        // The parent node's box, from the shadow: the collection holding the source can be
        // a split group's content carrier, whose own `Size` is zero because it takes the
        // group's extent through a relative size adjustment.
        let parent_size = self.nodes.size(parent);
        carrier.set_size(parent_size.x, parent_size.y);
        let offset = source.offset();
        let clip = back.compositor.create_rectangle_clip();
        clip.set_sides(offset.x, offset.y, offset.x + size.x, offset.y + size.y);
        carrier.set_clip(Some(&clip));
        group.children().insert_above(&carrier, source);
        group.children().remove(source);
        carrier.children().insert_at_top(source);
        let animation = self.motion.templates.spring(
            1, Tuning::Layout, Value::Scalar(offset.y), size.y, Duration::ZERO,
        );
        self.census.visuals_minted += 1;
        self.census.visuals_live += 1;
        self.census.animations += 1;
        self.motion.watch(back, PendingKind::Collapse { id, parent, carrier, resources: Vec::new() }, || {
            clip.start_animation("Bottom", &animation);
        })
    }

    /// How far past its own box the subtree's halos reach, in DIPs.
    ///
    /// Three sigmas covers a Gaussian's visible tail, and an offset shadow moves that tail
    /// with it.
    fn halo_margin(&self, id: NodeId) -> f32 {
        let sigma = PROPS[Prop::GlowSigma as usize].chan;
        let own = match self.nodes.aux(id).and_then(|aux| aux.glow.as_ref()) {
            Some(_) => {
                let offset = self
                    .nodes
                    .painted(id)
                    .and_then(|painted| painted.halo)
                    .map_or(0.0, |halo| halo.offset.x.abs().max(halo.offset.y.abs()));
                self.nodes.chan(id, sigma).max(0.0) * 3.0 + offset
            }
            None => 0.0,
        };
        children(&self.nodes, id.index() as u32)
            .map(|child| self.halo_margin(child))
            .fold(own, f32::max)
    }

    /// Records half of a sprite's declaration and realizes the chain if it changed.
    ///
    /// A mask and a paint arrive as separate ops in either order, so this records one and
    /// rebuilds from *both*; a sprite with only one half recorded holds the default for the
    /// other. A declaration equal to the one already held rebuilds nothing, which is what
    /// keeps an unmoved control from costing a mask brush, two cache lookups and a
    /// `set_brush` on every flush that touches it.
    fn declare(
        &mut self,
        id: NodeId,
        mask: Option<Mask>,
        paint: Option<(Paint, Option<Halo>)>,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        debug_assert!(
            self.nodes.is_sprite(id),
            "a mask or paint was addressed to a group"
        );
        let Some(held) = self.nodes.painted(id) else {
            return Ok(());
        };
        let was = (held.mask, held.paint, held.halo);
        let next = (
            mask.unwrap_or(was.0),
            paint.map_or(was.1, |(paint, _)| paint),
            paint.map_or(was.2, |(_, halo)| halo),
        );
        if let Paint::PresentedView { view, .. } = next.1
            && !view.is_valid()
        {
            return Err(invalid_arg());
        }
        if was == next && held.fresh(self.generation) {
            self.census.props_skipped += 1;
            return Ok(());
        }
        // Retain before release, unconditionally: re-declaring the same resource must not
        // let its count touch zero on the way through.
        if was.0 != next.0 {
            // A stroked shape claims two resources, so both are retained before either is
            // released: re-declaring the same one must not let its count touch zero.
            self.res.retain(next.0.holds());
            self.res.retain(next.0.holds_dash());
            self.res.release(was.0.holds());
            self.res.release(was.0.holds_dash());
        }
        if was.1 != next.1 {
            self.res.retain(next.1.holds());
            self.res.release(was.1.holds());
        }
        if let Some(row) = self.nodes.painted_mut(id) {
            row.mask = next.0;
            row.paint = next.1;
            row.halo = next.2;
        }
        self.realize(id, back, env)
    }

    /// Establishes a node's clip.
    ///
    /// A clip is declared, so layout re-states it on every node it touches and most
    /// re-statements change nothing: two shadows keep that free — the declaration compared
    /// here, and the twelve channels compared in the setter. Only a change in which *object*
    /// occupies the slot can move a shape mask between its two constructions, so only that
    /// re-routes and a resize writes four sides.
    fn set_clip(&mut self, id: NodeId, clip: Clip, back: &Backends, env: Env) -> Result<()> {
        if self.nodes.aux(id).is_some_and(|aux| aux.decl == clip) {
            self.census.props_skipped += 1;
            return Ok(());
        }
        let Some(visual) = self.nodes.visual(id).cloned() else {
            return Ok(());
        };
        let was_rect = matches!(
            self.nodes.aux(id).and_then(|aux| aux.clip.as_ref()),
            Some(ClipObj::Rect(_))
        );
        let was_rounded = self.nodes.aux(id).is_some_and(|aux| matches!(aux.decl, Clip::RoundedBounds(_)));
        let rounded = matches!(clip, Clip::RoundedBounds(_));
        // A rectangle clip already in the slot takes the new sides through the channels, so
        // a resize rebuilds nothing.
        if was_rect && was_rounded == rounded && matches!(clip, Clip::Rect { .. } | Clip::RoundedBounds(_)) {
            self.nodes.aux_mut(id).decl = clip;
            return self.write_clip(id, clip);
        }
        if was_rounded {
            for prop in [Prop::ClipR, Prop::ClipB] {
                self.nodes.stop(id, desc(prop));
            }
        }
        // Where the clip lands: a split group's content carrier, a lit sprite's paint, or
        // the node's own visual. Never the visual that holds the node's own chrome or halo.
        let host = self.nodes.clip_host(id).unwrap_or_else(|| visual.clone());
        let next = match clip {
            Clip::None => None,
            Clip::Bounds => Some(ClipObj::Bounds(back.compositor.create_inset_clip())),
            // Rounded clipping needs no brush slot and no capture: a rectangle clip carries
            // its own radii.
            Clip::Rect { .. } | Clip::RoundedBounds(_) => Some(ClipObj::Rect(back.compositor.create_rectangle_clip())),
            Clip::Geom(geom) => self.res.geom(geom).map(|geometry| {
                ClipObj::Geom(back.compositor.create_geometric_clip(geometry))
            }),
        };
        match &next {
            Some(next) => next.apply(&host),
            // Clears only what the *sink* established: a clip-route shape mask writes its
            // geometric clip straight onto the visual without claiming this slot.
            None if was_rect || self.nodes.aux(id).is_some_and(|aux| aux.clip.is_some()) => {
                host.clear_clip();
            }
            None => {}
        }
        let aux = self.nodes.aux_mut(id);
        aux.clip = next;
        aux.decl = clip;
        // A clip arriving on a group that already paints its own box is the other way a
        // group comes to need its two bands; chrome arriving is the first.
        self.split_bands(id);
        self.write_clip(id, clip)?;
        if rounded {
            self.bind_rounded_bounds(id, back);
        }
        // The sink's clip and a clip-route shape mask compete for the visual's one slot, so
        // a slot changing hands costs a promotion rather than a wrong render.
        if self.nodes.painted(id).is_some_and(Painted::owns_the_clip) {
            self.realize(id, back, env)?;
        }
        Ok(())
    }

    fn bind_rounded_bounds(&mut self, id: NodeId, back: &Backends) {
        let Some(visual) = self.nodes.visual(id).cloned() else { return };
        for (prop, source) in [(Prop::ClipR, "v.Size.X"), (Prop::ClipB, "v.Size.Y")] {
            let expression = back.compositor.create_expression_animation(source);
            expression.set_reference_parameter("v", &visual);
            self.nodes.start(id, desc(prop), &expression.as_animation(), None, Held::Bound);
            self.census.animations += 1;
        }
    }

    /// Writes the clip's twelve numbers through the property table.
    ///
    /// They are not a second write path: each goes through the setter one channel at a time.
    /// A second path would re-implement the shadow comparison and the refusal, and a declared
    /// clip would then write on every layout pass and kill any expression holding a side.
    ///
    /// A fresh rectangle clip has every side at zero, which clips everything, so a `None`
    /// declaration on a node that has one seeds it to the node's own box.
    fn write_clip(&mut self, id: NodeId, clip: Clip) -> Result<()> {
        let size = self.nodes.size(id);
        let (l, t, r, b, radius) = match clip {
            Clip::Rect { l, t, r, b, radius } => (l, t, r, b, radius),
            Clip::RoundedBounds(radius) => (0.0, 0.0, size.x, size.y, radius),
            Clip::None if self.nodes.has_owner(id, Owner::Clip) => {
                (0.0, 0.0, size.x, size.y, Corners::default())
            }
            _ => return Ok(()),
        };
        let rows = [
            (Prop::ClipL, l),
            (Prop::ClipT, t),
            (Prop::ClipR, r),
            (Prop::ClipB, b),
            (Prop::CornerTopLeftX, radius.tl),
            (Prop::CornerTopLeftY, radius.tl),
            (Prop::CornerTopRightX, radius.tr),
            (Prop::CornerTopRightY, radius.tr),
            (Prop::CornerBottomRightX, radius.br),
            (Prop::CornerBottomRightY, radius.br),
            (Prop::CornerBottomLeftX, radius.bl),
            (Prop::CornerBottomLeftY, radius.bl),
        ];
        for (prop, value) in rows {
            if matches!(clip, Clip::RoundedBounds(_)) && matches!(prop, Prop::ClipR | Prop::ClipB) {
                continue;
            }
            let written = self.nodes.set(id, prop, Value::Scalar(value));
            self.census.count(written);
        }
        Ok(())
    }

    fn bind(
        &mut self,
        id: NodeId,
        prop: Prop,
        bind: Bind,
        patch: &SinkPatch,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        let row = desc(prop);
        let completed = !self.springs_enabled && matches!(bind,
            Bind::Animate(Anim::Frames { iterations: Iterations::Count(_), .. }));
        let bind = match bind {
            Bind::Animate(Anim::Spring { to, .. }) if !self.springs_enabled => Bind::Set(to),
            Bind::Animate(Anim::Frames { frames, .. }) if !self.springs_enabled => {
                let Some((_, to, _)) = patch.frames(frames).last() else { return Ok(()) };
                Bind::Set(*to)
            }
            other => other,
        };
        // The owner may not exist yet, and what that means differs per owner.
        if !self.nodes.has_owner(id, row.owner) {
            match absent(row.owner) {
                Absent::MintClip => self.set_clip(id, Clip::None, back, env)?,
                // The shape state does not exist yet, so the route function cannot see the
                // channel about to be bound; marking it stale makes the rebind observe a
                // live channel and take the capture.
                Absent::Promote => {
                    self.nodes.set_held(id, desc(Prop::TrimEnd), Held::Stale);
                    self.realize(id, back, env)?;
                }
                Absent::Refuse => return Ok(()),
            }
            if !self.nodes.has_owner(id, row.owner) {
                debug_assert!(
                    false,
                    "{prop:?} was bound on {id:?} before its object existed"
                );
                return Ok(());
            }
        }
        // Replacing an overlapping channel cancels the held subscription, so a completion
        // cannot report for a run another binding has already displaced.
        self.pending_retain(|pending| {
            !matches!(pending.holds, PendingKind::Frames(node, held)
                if node == id && desc(held).overlaps(row))
        });
        match bind {
            Bind::Set(value) => {
                let written = self.nodes.set(id, prop, value);
                if written {
                    for (_, observer) in &mut self.size_observers {
                        if observer.affected(id,prop) { observer.direct(id,prop,&self.nodes)?; }
                    }
                }
                self.census.count(written);
                // Only a size change can invalidate a capture, so the property is tested
                // before the walk: a move, an opacity and a rotation land through the same
                // setter and none of them moves the region.
                if written && matches!(prop, Prop::Size | Prop::SizeX | Prop::SizeY) {
                    self.resize_captures(id, env);
                    self.reclamp_box_mask(id, back, env)?;
                }
                if completed {
                    self.events.borrow_mut().push(SceneEvent::AnimationCompleted { node: id, prop });
                }
            }
            Bind::Animate(anim) => {
                let observed: Vec<_> = self.size_observers.iter().enumerate()
                    .filter(|(_,(_,observer))| observer.affected(id,prop)).map(|(i,_)| i).collect();
                for &at in &observed { self.size_observers[at].1.arm()?; }
                let batch = (!observed.is_empty()).then(|| Rc::new(back.compositor.create_scoped_batch(BatchKind::Animation)));
                if let Some(batch)=&batch {
                    for &at in &observed { self.size_observers[at].1.watch(id,prop,batch)?; }
                }
                let result = self.animate(id, prop, row, anim, patch, back);
                for &at in &observed { self.size_observers[at].1.target(&self.nodes); }
                if let Some(batch)=batch { batch.try_end()?; }
                result?;
                if matches!(prop, Prop::Size | Prop::SizeX | Prop::SizeY) {
                    self.resize_captures(id, env);
                    self.reclamp_box_mask(id, back, env)?;
                }
            }
            Bind::Track {
                tracker,
                axis,
                affine,
            } => {
                // One axis drives one channel, and the expression evaluates to a scalar:
                // aimed at a composite the compositor refuses it outright.
                if row.count != 1 {
                    return Err(invalid_arg());
                }
                let Some(state) = self.trackers.get(tracker.id()) else {
                    return Ok(());
                };
                let animation = self.motion.templates.track(axis, &state.inner, affine);
                // Permanent: nothing but an explicit stop leaves `Bound`.
                self.nodes.start(id, row, &animation, None, Held::Bound);
                self.census.animations += 1;
            }
            Bind::FollowOffset {
                source,
                vertical,
                affine,
                extent,
                clamp,
            } => {
                // Trim and opacity cannot feed back into the source's offset.
                if !matches!(prop, Prop::TrimStart | Prop::TrimEnd | Prop::Opacity)
                    || source == id
                    || !affine.m.is_finite()
                    || !affine.c.is_finite()
                    || !clamp[0].is_finite()
                    || !clamp[1].is_finite()
                    || clamp[0] > clamp[1]
                    || extent.is_some_and(|(_, inset)| !inset.is_finite() || inset < 0.0)
                {
                    return Err(invalid_arg());
                }
                let Some(from) = self.nodes.visual(source).cloned() else {
                    return Ok(());
                };
                let bounds = match extent {
                    Some((node, inset)) => {
                        let Some(visual) = self.nodes.visual(node) else { return Ok(()); };
                        Some((visual, inset))
                    }
                    None => None,
                };
                let animation = self.motion.templates.follow(vertical, &from, affine, bounds, clamp);
                self.nodes.start(id, row, &animation, None, Held::Bound);
                self.census.animations += 1;
            }
            Bind::Stop => {
                self.nodes.stop(id, row);
                for (_,observer) in &mut self.size_observers {
                    if observer.affected(id,prop) { observer.direct(id,prop,&self.nodes)?; }
                }
            }
        }
        Ok(())
    }

    fn animate(
        &mut self,
        id: NodeId,
        prop: Prop,
        row: &PropDesc,
        anim: Anim,
        patch: &SinkPatch,
        back: &Backends,
    ) -> Result<()> {
        match anim {
            Anim::Spring {
                to,
                tuning,
                delay_ms,
            } => {
                if to.kind() != row.kind() {
                    return Ok(());
                }
                // The shadow holds the previous target, not a sampled presentation value.
                let travel = travel(&self.nodes, id, row, to);
                let animation = if tuning == Tuning::Layout && delay_ms == 0 {
                    let springs = &mut self.nodes.aux_mut(id).layout_springs;
                    let index = springs.iter().position(|(held, _)| *held == prop)
                        .unwrap_or_else(|| {
                            springs.push((prop, LayoutSpring::new(back, row.spring_slot())));
                            springs.len() - 1
                        });
                    springs[index].1.retarget(to)
                } else {
                    self.motion.templates.spring(
                        row.spring_slot(),
                        tuning,
                        to,
                        travel,
                        Duration::from_millis(u64::from(delay_ms)),
                    )
                };
                self.nodes
                    .start(id, row, &animation, Some(to), Held::Playing);
                self.census.animations += 1;
                Ok(())
            }
            // A key-framed run states no single settling value, so it leaves the shadow
            // alone.
            Anim::Frames {
                frames,
                duration_ms,
                iterations,
            } => {
                let scalar = row.count == 1;
                let animation = self.motion.templates.frames(
                    back,
                    patch.frames(frames),
                    duration_ms,
                    iterations,
                    scalar,
                );
                self.census.animations += 1;
                if !matches!(iterations, Iterations::Count(_)) {
                    self.nodes.start(id, row, &animation, None, Held::Playing);
                    return Ok(());
                }
                // The start must occur inside the scoped batch for completion to cover it.
                let nodes = &mut self.nodes;
                self.motion.watch(back, PendingKind::Frames(id, prop), || {
                    nodes.start(id, row, &animation, None, Held::Playing);
                })
            }
        }
    }

    /// Brings a node's captures up to date with the box it now occupies.
    ///
    /// A capture states its region in the source's own space, so it does not follow its
    /// sprite: a shape or a glow whose box moved keeps describing the old one and draws at
    /// the wrong scale. Correcting it is three property sets and no re-tessellation — the
    /// geometry object is untouched, no verbs cross the seam, and the app thread is not
    /// involved.
    fn resize_captures(&mut self, id: NodeId, env: Env) {
        let (size, scale) = (self.nodes.size(id), env.scale());
        note!("scene", "resize_captures id={} size=({:.0},{:.0})", id.index(), size.x, size.y);
        let Some(aux) = self.nodes.aux(id) else {
            return;
        };
        if let Some(shape) = &aux.shape {
            shape.resize(size, scale);
        }
        if let Some(glow) = &aux.glow {
            glow.resize(id, size, scale);
        }
    }

    /// Rebuilds a rounded-box mask whose nine-grid insets the new box has changed.
    ///
    /// The insets are clamped against the extent, so a box that crossed twice its own radius
    /// needs a different brush; gated on the key actually moving, because every node's size
    /// is re-bound on every solve and rebuilding the chain there would put a brush rebuild on
    /// the resize path for every rounded surface in the tree.
    fn reclamp_box_mask(&mut self, id: NodeId, back: &Backends, env: Env) -> Result<()> {
        let scale = env.scale();
        let size = self.nodes.size(id);
        let Some(painted) = self.nodes.painted(id) else {
            return Ok(());
        };
        let (Mask::Box { radius } | Mask::Outline { radius, .. }) = painted.mask else {
            return Ok(());
        };
        let (width, open) = match painted.mask {
            Mask::Outline { width, open, .. } => (
                width
                    .max(0.0)
                    .min((size.x.min(size.y) * 0.5 - 1.0 / scale).max(0.0)),
                open,
            ),
            _ => (-1.0, None),
        };
        if painted.key
            == Some(BoxKey::outline(
                fit(radius, size, scale),
                width,
                open,
                scale,
            ))
        {
            return Ok(());
        }
        // The declaration is unchanged, so this re-realizes the chain the node already holds
        // rather than declaring a new one.
        self.realize(id, back, env)
    }

    fn resource(
        &mut self,
        id: ResId,
        op: ResOp,
        patch: &SinkPatch,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        let obj = match op {
            // Drops only the model's own claim: a sprite still painting with the resource
            // keeps it alive until that sprite is destroyed or re-declares.
            ResOp::Drop => {
                self.res.disclaim(id);
                return Ok(());
            }
            ResOp::Geom { verbs } => {
                let path = back.path(patch.verbs(verbs))?;
                ResObj::Geom(back.compositor.create_path_geometry(&path), path)
            }
            ResOp::Ramp { stops, spread } => {
                let Some(surface) = back.raster_ramp(patch.stops(stops), spread, env, None)? else {
                    return Ok(());
                };
                let brush = back.brush(&surface, Stretch::Fill);
                ResObj::Brush(brush, Some(surface))
            }
            ResOp::Run { segs, ink } => {
                let Some(surface) = back.raster_run(
                    patch.segs(segs),
                    patch.glyph_pool(),
                    patch.float_pool(),
                    ink,
                    env,
                )?
                else {
                    return Ok(());
                };
                let brush = back.brush(&surface, Stretch::None);
                scale_pixels(&brush, env);
                brush.set_nearest_sampling();
                ResObj::Brush(brush, Some(surface))
            }
            ResOp::Dash { runs } => {
                let source = patch.floats(runs);
                let mut held = [0.0; 8];
                // A longer pattern is truncated: the shape's dash vector is a fixed
                // vocabulary here, and four dash-and-gap pairs is what it carries.
                let len = source.len().min(8);
                held[..len].copy_from_slice(&source[..len]);
                ResObj::Dash(held, len as u8)
            }
            ResOp::Region => ResObj::Pending,
        };
        self.res.declare(id, obj);
        self.rebind_holders(id, back, env)
    }

    /// Re-points the object every sprite already holds, so each of them moves together; a
    /// newly minted object still needs its holders bound to it.
    fn rebind_holders(&mut self, id: ResId, back: &Backends, env: Env) -> Result<()> {
        let holders: Vec<NodeId> = self
            .nodes
            .ids()
            .filter(|node| {
                self.nodes.painted(*node).is_some_and(|painted| {
                    painted.mask.holds().map(Holding::id) == Some(id)
                        || painted.paint.holds().map(Holding::id) == Some(id)
                })
            })
            .collect();
        for node in holders {
            self.realize(node, back, env)?;
        }
        Ok(())
    }

    /// Points a region's slot at a buffer the producer presents into, and rebinds every
    /// sprite painting with it.
    ///
    /// The buffer is already at device resolution, so the brush samples one texel per
    /// physical pixel — every pixel guarantee a presented region makes rests on that.
    ///
    /// # Safety
    ///
    /// Behavior is undefined if any of the following conditions are violated:
    ///
    /// - `handle` must be a composition surface handle.
    /// - `handle` must stay live for as long as the binding does; the compositor does not
    ///   take ownership of it.
    ///
    /// # Errors
    ///
    /// Fails when the compositor rejects `handle` or a dependent sprite cannot be rebound.
    pub unsafe fn set_region(
        &mut self,
        region: RegionId,
        handle: *mut core::ffi::c_void,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        self.sync(back, env)?;
        // SAFETY: `handle` is a composition surface handle that stays live for as long as
        // the binding does, which is this function's obligation on its caller.
        let surface = unsafe { back.compositor.create_surface_for_handle(handle) }?;
        let brush = back.brush(&surface, Stretch::None);
        scale_pixels(&brush, env);
        brush.set_nearest_sampling();
        self.res
            .declare(region.erased(), ResObj::Brush(brush, None));
        self.rebind_holders(region.erased(), back, env)
    }

    /// Updates pixel sampling after a DPI change.
    ///
    /// Run re-rasterization retains the brush and replaces its surface; regions retain
    /// both. Their brush transforms must follow the new pixel grid in either case.
    fn rescale_pixels(&self, env: Env) {
        for brush in self.res.pixel_brushes() {
            scale_pixels(brush, env);
        }
    }

    /// Releases a region's buffer and rebinds every sprite painting with it.
    ///
    /// The compositor holds a reference to whatever a visual paints with, so a brush over a
    /// handle the producer is about to close must leave the tree before the handle closes.
    ///
    /// # Errors
    ///
    /// Fails when a dependent sprite cannot be rebound.
    pub fn clear_region(&mut self, region: RegionId, back: &Backends, env: Env) -> Result<()> {
        self.sync(back, env)?;
        self.res.clear_region(region);
        self.rebind_holders(region.erased(), back, env)
    }

    // ── trackers ────────────────────────────────────────────────────────────────────

    fn tracker(&mut self, id: TrackerId<()>, op: TrackerOp, back: &Backends) -> Result<()> {
        match op {
            TrackerOp::Create {
                viewport,
                axes,
                owned,
            } => {
                let Some(visual) = self.nodes.visual(viewport.0).cloned() else {
                    return Ok(());
                };
                // The owner is supplied at construction with no per-callback subscription,
                // so a tracker that needs one event pays for all six — measured at ~19x the
                // callback cost of an ownerless tracker over the same fling. A callback
                // lands through the owning thread's own message queue, so by the time that
                // thread is back in its loop the event is already here.
                let inner = if owned {
                    let sink = Rc::clone(&self.events);
                    back.compositor
                        .create_interaction_tracker_with_owner(move |event| {
                            sink.borrow_mut().push(translate(id, event));
                        })?
                } else {
                    back.compositor.create_interaction_tracker()?
                };
                let source = configure_source(&visual, axes)?;
                inner.add_source(&source)?;
                self.trackers.place(
                    id.id(),
                    TrackerState {
                        inner,
                        source: Some(source),
                        viewport: viewport.0,
                        position: Vector2::zero(),
                        scale: 1.0,
                        phase: Phase::Idle,
                        shadow: Arc::new(AtomicU64::new(pack_offset(0.0, 0.0))),
                        bounds: (Vector2::zero(), Vector2::zero()),
                        extra: Vector2::zero(),
                        pending: [None; PENDING_REQUESTS],
                    },
                );
                self.census.trackers_live += 1;
            }
            TrackerOp::Bounds { min, max } => {
                if let Some(state) = self.trackers.get_mut(id.id()) {
                    state.bounds = (min, max);
                    state.apply_bounds();
                }
            }
            // A wheel-originated motion is distinguishable from a fling at inertia entry and
            // is given a shorter decay, which is what this states.
            TrackerOp::Decay(rate) => {
                if let Some(state) = self.trackers.get(id.id()) {
                    state.inner.set_position_inertia_decay_rate(rate.map(v3));
                }
            }
            TrackerOp::Drop => {
                if let Some(state) = self.trackers.take(id.id()) {
                    let _ = state.inner.clear_sources();
                    // So the hit query stops resolving that node's descendants through an
                    // offset nothing updates any more.
                    self.hits.clear_scroll(state.viewport);
                    self.census.trackers_live = self.census.trackers_live.saturating_sub(1);
                }
            }
        }
        Ok(())
    }

    /// Holds `extra` extent past the maximum the layout stated for this tracker.
    ///
    /// What the layout states is how far the content can travel inside the viewport. An
    /// occlusion over the viewport takes room off it without shortening the content, so the
    /// position a surface at the end of the content has to reach is past that maximum. The
    /// extra is held here rather than folded into the stated bounds, so a resize while the
    /// occlusion stands keeps it and clearing it restores the stated maximum exactly.
    ///
    /// # Errors
    ///
    /// Fails when `id` names no live tracker.
    pub fn extend_bounds<O>(&mut self, id: TrackerId<O>, extra: Vector2) -> Result<()> {
        let Some(state) = self.trackers.get_mut(id.id()) else {
            return Err(invalid_arg());
        };
        state.extra = extra;
        state.apply_bounds();
        Ok(())
    }

    /// Asks an observed tracker to move, returning the request's id.
    ///
    /// The tracker may drop the request, which arrives back as an ignored-request event
    /// naming that id.
    ///
    /// # Errors
    ///
    /// Fails when `id` names no live tracker, or when the compositor rejects the request.
    pub fn request(&mut self, id: TrackerId<Observed>, request: TrackerRequest) -> Result<i32> {
        match self.trackers.get_mut(id.id()) {
            Some(state) => state.request(request),
            None => Err(invalid_arg()),
        }
    }

    /// Offers a captured contact to a live tracker's compositor-owned input source.
    ///
    /// Touch and pen must be explicitly redirected and mouse cannot be redirected at all, so
    /// this is the only route a captured touch contact reaches a tracker by.
    ///
    /// # Errors
    ///
    /// Fails when the compositor refuses the contact.
    pub fn redirect_manipulation<O>(
        &self,
        id: TrackerId<O>,
        pointer: &ManipulationPointer,
    ) -> Result<()> {
        if let Some(source) = self
            .trackers
            .get(id.id())
            .and_then(|state| state.source.as_ref())
        {
            source.try_redirect_for_manipulation(pointer)?;
        }
        Ok(())
    }

    /// The word each tracker publishes its reported position into, held by a hit test that
    /// runs off this thread.
    ///
    /// It reads the word rather than asking the scene, and the handle keeps the word alive
    /// past the tracker's own drop. The set changes only when a tracker is created or
    /// dropped, so a caller lists it on those edges.
    pub fn tracker_shadows(&self, out: &mut Vec<(NodeId, Arc<AtomicU64>)>) {
        out.clear();
        out.extend(
            self.trackers
                .iter()
                .map(|(_, state)| (state.viewport, Arc::clone(&state.shadow))),
        );
    }

    /// Appends everything the trackers and the batches have reported, reconciling as it
    /// drains: a reported position updates the tracker's shadow and the hit array's scroll
    /// offset, and an ignored request is dropped from its pending set.
    ///
    /// Only the range appended here is reconciled: `out` may still hold a previous drain's
    /// events, and applying a tracker position twice is not idempotent.
    pub fn drain_events(&mut self, out: &mut Vec<SceneEvent>) {
        self.retire();
        let from = out.len();
        out.append(&mut self.events.borrow_mut());
        for event in &out[from..] {
            match *event {
                SceneEvent::TrackerValues {
                    tracker,
                    position,
                    scale,
                } => {
                    let viewport = self.trackers.get_mut(tracker.id()).map(|state| {
                        state.values_changed(position, scale);
                        state.viewport
                    });
                    if let Some(viewport) = viewport {
                        self.hits.set_scroll(viewport, position);
                    }
                }
                SceneEvent::RequestIgnored { tracker, request } => {
                    if let Some(state) = self.trackers.get_mut(tracker.id()) {
                        state.ignored(request);
                    }
                }
                SceneEvent::TrackerPhase { tracker, phase } => {
                    if let Some(state) = self.trackers.get_mut(tracker.id()) {
                        state.phase = phase;
                    }
                }
                _ => {}
            }
        }
    }

    // ── delays and retirement ───────────────────────────────────────────────────────

    /// Starts a timed reveal, reported once the delay has run.
    ///
    /// Nothing on this thread measures the wait: the animation moves a scratch property no
    /// visual reads, so its only observable effect is the batch's completion, which the
    /// compositor raises. One property set per delay, because two delays sharing a key would
    /// be two animations on one property, and starting the second ends the first.
    fn start_delay(&mut self, id: DelayId, ms: u32, back: &Backends) -> Result<()> {
        self.cancel_delay(id);
        let target = back.compositor.create_property_set();
        target.insert_vector2(DELAY_KEY, Vector2::zero());
        // A `Vector2` key frame, because that is the animation kind the wrapper exposes a
        // delay on; nothing reads the value.
        let animation = back.compositor.create_vector2_key_frame_animation();
        animation.insert_key_frame(1.0, Vector2::one());
        animation.set_delay(Duration::from_millis(u64::from(ms)));
        // The batch reports at the end of the run rather than at the end of the wait.
        animation.set_duration(Duration::from_millis(DELAY_RUN_MS));
        let started = target.clone();
        self.census.animations += 1;
        self.motion.watch(back, PendingKind::Delay(id, target), || {
            started.start_animation(DELAY_KEY, &animation);
        })
    }

    /// Cancels the delay registered under `id`.
    ///
    /// Dropping the record unsubscribes its completion, so a cancelled delay never reports
    /// and a tooltip swapping between targets neither reports the old delay nor waits twice.
    fn cancel_delay(&mut self, id: DelayId) {
        self.pending_retain(
            |pending| !matches!(pending.holds, PendingKind::Delay(held, _) if held == id),
        );
    }

    /// Ends transient exit snapshots, which a window resize invalidates.
    pub fn cancel_exits(&mut self) {
        self.pending_retain(|pending| !matches!(pending.holds,
            PendingKind::Ghost(_) | PendingKind::Collapse { .. }));
    }

    /// Returns the collapsing ancestor that keeps a presented region visible in this patch.
    pub fn collapsing_region(&self, region: RegionId, patch: &SinkPatch) -> Option<NodeId> {
        for op in patch.ops() {
            let Op::Drop { id: root, exit: Exit::Collapse, .. } = *op else { continue };
            for node in self.nodes.ids().filter(|node| self.nodes.painted(*node)
                .is_some_and(|row| row.paint.holds().map(Holding::id) == Some(region.erased())))
            {
                let mut at = node;
                loop {
                    if at == root { return Some(root); }
                    let parent = self.nodes.links(at.index() as u32).parent;
                    if parent == NO_LINK { break; }
                    at = self.nodes.id_at(parent);
                }
            }
        }
        None
    }

    /// Reports whether a retired subtree still has a native clipped exit.
    pub fn collapsing(&self, id: NodeId) -> bool {
        self.motion.pending.iter().any(|pending| matches!(pending.holds,
            PendingKind::Collapse { id: root, .. } if root == id))
    }

    fn pending_retain(&mut self, keep: impl Fn(&Pending) -> bool) {
        let overlay = self.overlay.children();
        let census = &mut self.census;
        self.motion.pending.retain(|pending| {
            if keep(pending) {
                return true;
            }
            // Explicitly removed from its parent collection, since that collection owns a
            // strong reference.
            if let PendingKind::Ghost(visual) = &pending.holds {
                let _ = overlay.try_remove(visual);
                census.visuals_live = census.visuals_live.saturating_sub(1);
            }
            if let PendingKind::Collapse { carrier, .. } = &pending.holds {
                if let Some(parent) = carrier.parent() {
                    let _ = parent.children().try_remove(carrier);
                }
                census.visuals_live = census.visuals_live.saturating_sub(1);
            }
            false
        });
    }

    /// Releases everything whose batch has reported.
    ///
    /// A batch reports once, so a released record cannot be waited on again. Completion
    /// requests one cleanup pass; nothing here keeps the clock awake through playback.
    fn retire(&mut self) {
        let mut reports = Vec::new();
        let overlay = self.overlay.children();
        let census = &mut self.census;
        let nodes = &mut self.nodes;
        self.motion.pending.retain(|pending| {
            if !pending.done.get() {
                return true;
            }
            match &pending.holds {
                PendingKind::Ghost(visual) => {
                    let _ = overlay.try_remove(visual);
                    census.visuals_live = census.visuals_live.saturating_sub(1);
                }
                PendingKind::Collapse { carrier, .. } => {
                    if let Some(parent) = carrier.parent() {
                        let _ = parent.children().try_remove(carrier);
                    }
                    census.visuals_live = census.visuals_live.saturating_sub(1);
                }
                PendingKind::Delay(id, _) => reports.push(SceneEvent::DelayElapsed(*id)),
                PendingKind::Frames(node, prop) => reports.push(SceneEvent::AnimationCompleted {
                    node: *node,
                    prop: *prop,
                }),
                PendingKind::Restate(groups) => {
                    for &token in groups {
                        census.count(nodes.finish_settle(token));
                    }
                }
            }
            false
        });
        self.events.borrow_mut().extend(reports);
    }

    /// Retargets a channel from the front thread, inside the pass that decided to.
    ///
    /// It writes through the same property table, shadow and setter the applier uses, so the
    /// router is that same writer reached from the other side rather than a second one:
    /// without it the router resolves a hover and cannot move a pixel until the app thread
    /// next runs.
    ///
    /// Refuses key frames, whose frames live in a patch buffer the front thread has none of.
    ///
    /// # Errors
    ///
    /// Fails on a key-frame binding, and on whatever the binding itself refuses.
    pub fn retarget(
        &mut self,
        node: NodeId,
        prop: Prop,
        bind: Bind,
        back: &Backends,
    ) -> Result<()> {
        if matches!(bind, Bind::Animate(Anim::Frames { .. })) {
            return Err(invalid_arg());
        }
        #[cfg(debug_assertions)]
        self.claimed.insert((node, prop));
        // The environment the tree was last realized under: a front-side retarget states no
        // new one, and a channel write rasterizes nothing.
        let Some(env) = self.env else {
            return Err(invalid_arg());
        };
        // A fresh patch allocates nothing, since every buffer of one is an empty `Vec`.
        let empty = SinkPatch::default();
        self.bind(node, prop, bind, &empty, back, env)?;
        self.settle_stopped(back)
    }

    /// Replaces the window ground within the caller's pass.
    ///
    /// # Errors
    ///
    /// Fails if a layer cannot be rasterized.
    pub fn set_backdrop(&mut self, spec: BackdropSpec, back: &Backends, env: Env) -> Result<()> {
        self.backdrop.spec = spec;
        self.backdrop.build(back, env)?;
        self.reseat_backdrop();
        Ok(())
    }

    /// Tells the ground the client extent, in DIPs.
    ///
    /// Every other layer is stated as a fraction of the window and needs no telling. The
    /// grain does: it is a dither, so it has to land one texel on one pixel, and that is an
    /// extent. It is re-cut only where the window outgrew the allocation, so an ordinary
    /// resize does nothing here at all.
    ///
    /// # Errors
    ///
    /// Fails if the grain's surface cannot be rasterized.
    pub fn set_ground_extent(&mut self, size: Vector2, back: &Backends, env: Env) -> Result<()> {
        let scale = env.scale();
        let px = (
            (size.x * scale).ceil() as i32,
            (size.y * scale).ceil() as i32,
        );
        if self.backdrop.resize(px, back, env)? {
            self.reseat_backdrop();
        }
        Ok(())
    }

    /// Moves one glow's centre, as a fraction of the window.
    ///
    /// A front-side write, like a retarget: the backdrop carries no patch ops, so an
    /// application driving a glow from a cell calls this from its effect.
    pub fn move_glow(&mut self, index: usize, at: Vector2) {
        self.backdrop.move_glow(index, at);
    }
}

/// The scratch key a delay's animation moves. Nothing binds it: the value is written only so
/// that the batch has a piece of work whose completion it can report.
const DELAY_KEY: &str = "Elapsed";

/// How long a delay's animation runs once its wait is over, in milliseconds.
const DELAY_RUN_MS: u64 = 1;

/// Establishes the tree's DIP space.
///
/// The extent is stated relative to the composition target rather than written by this side,
/// so the compositor re-derives it as the window changes — including through a drag-resize,
/// where the system's modal loop owns the thread and this side may not publish at all. The
/// adjustment is the reciprocal of the scale, because a relative adjustment multiplies the
/// parent's own extent and the target's extent is in physical pixels. Both halves change on a
/// DPI change and on nothing else, so they are set together and only here.
fn set_dip_space(root: &ContainerVisual, scale: f32) {
    root.set_scale(Vector3 {
        x: scale,
        y: scale,
        z: 1.0,
    });
    let dips = 1.0 / scale;
    root.set_relative_size_adjustment(Vector2 { x: dips, y: dips });
}

/// Maps a pixel surface one texel to one physical pixel inside a visual measured in DIPs.
///
/// A sprite's box is in DIPs and the whole tree hangs under a root carrying the display
/// scale, so a brush left at unit scale paints one texel per *DIP* and the content comes out
/// magnified. The brush addresses the whole surface from its top-left corner.
fn scale_pixels(brush: &windows_composition::CompositionSurfaceBrush, env: Env) {
    let dips = 1.0 / env.scale();
    brush.set_source_transform(Vector2::zero(), Vector2 { x: dips, y: dips });
}

/// How far a spring travels, from the shadow's current value.
///
/// A channel on a clip, a shape or a glow moves in its own units — a trim fraction, a blur
/// sigma — so there is no travel in DIPs for a period to be scaled by.
fn travel(nodes: &Arena, id: NodeId, row: &PropDesc, to: Value) -> f32 {
    if row.owner != Owner::Visual {
        return 0.0;
    }
    match to {
        Value::Scalar(v) => (v - nodes.chan(id, row.chan)).abs(),
        Value::Vec2(v) => {
            let dx = v.x - nodes.chan(id, row.chan);
            let dy = v.y - nodes.chan(id, row.chan + 1);
            (dx * dx + dy * dy).sqrt()
        }
    }
}

/// How this crate refuses an id or a binding it cannot serve.
#[must_use]
pub fn invalid_arg() -> windows_core::Error {
    windows_core::Error::from(windows_core::HRESULT(-2147024809))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::patch::Op;
    use windows_color::{DisplayCapability, OutputTransform, Radiance};
    use windows_composition::{Compositor, DispatcherQueueController};

    /// A window, a device and a scene over them, or `None` where the session can build none.
    ///
    /// The applier's gates are properties of a real compositor tree — an absorbed write is
    /// one the setter refused after comparing a shadow it actually pushed — so they are
    /// exercised against one rather than against a stub.
    struct Rig {
        _queue: DispatcherQueueController,
        _window: windows_window::Window,
        back: Backends,
        scene: Scene,
        ids: Ids<NODE>,
        env: Env,
    }

    fn rig() -> Option<Rig> {
        let Ok(queue) = DispatcherQueueController::create_on_current_thread() else {
            eprintln!("skipped: no dispatcher queue in this session");
            return None;
        };
        let Ok(window) = windows_window::Window::new("windows-scene applier")
            .size(400, 300)
            .hidden()
            .create()
        else {
            eprintln!("skipped: no window in this session");
            return None;
        };
        let Ok(gpu) = windows_d2d::Gpu::for_window() else {            eprintln!("skipped: no Direct2D device in this session");
            return None;
        };
        let comp = Compositor::new().expect("a compositor");
        let back = Backends::new(comp, &gpu, FontLadder::default()).expect("the backends");
        let env = Env::new(
            96.0,
            OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
        );
        let scene =
            Scene::new_at(window.handle(), &back, env, BackdropSpec::default()).expect("a scene");
        Some(Rig {
            _queue: queue,
            _window: window,
            back,
            scene,
            ids: Ids::default(),
            env,
        })
    }

    impl Rig {
        fn apply(&mut self, patch: &mut SinkPatch) -> bool {
            self.scene
                .apply(patch, &self.back, self.env)
                .expect("the pass applied")
        }

        /// A sprite of `size` attached to the window band, already sized so its mask has a
        /// box to be cut for.
        fn sprite(&mut self, patch: &mut SinkPatch, size: f32) -> NodeId {
            let id = self.ids.mint();
            patch.push(Op::New {
                id,
                kind: NodeKind::Sprite,
                parent: Attach::Window,
                after: None,
            });
            patch.push(Op::Bind {
                id,
                prop: Prop::Size,
                bind: Bind::Set(Value::Vec2(Vector2 { x: size, y: size })),
            });
            id
        }
    }

    const FILL: Mask = Mask::Box {
        radius: Corners::all(4.0),
    };

    #[test]
    fn native_size_observer_suspends_resumes_and_detaches() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let node = rig.sprite(&mut patch, 20.0);
        rig.apply(&mut patch);
        let samples = Rc::new(RefCell::new(Vec::new()));
        let published = samples.clone();
        rig.scene.observe_size(node, None, move |_, size, target, moving| {
            published.borrow_mut().push((size, target, moving));
        }, &rig.back).unwrap();
        rig.scene.observe_size_active(node, false).unwrap();
        rig.scene.retarget(node, Prop::SizeX, Bind::Set(Value::Scalar(40.0)), &rig.back).unwrap();
        assert_eq!(samples.borrow().len(), 1);
        rig.scene.observe_size_active(node, true).unwrap();
        assert_eq!(samples.borrow().last().unwrap().0, Vector2::new(40.0,20.0));
        rig.scene.retarget(node, Prop::SizeY, Bind::Animate(Anim::Spring {
            to: Value::Scalar(80.0), tuning: Tuning::Layout, delay_ms: 0,
        }), &rig.back).unwrap();
        assert_eq!(samples.borrow().last().unwrap().1, Vector2::new(40.0,80.0));
        assert!(samples.borrow().last().unwrap().2);
        rig.scene.retarget(node, Prop::SizeY, Bind::Set(Value::Scalar(30.0)), &rig.back).unwrap();
        assert_eq!(*samples.borrow().last().unwrap(), (Vector2::new(40.0,30.0), Vector2::new(40.0,30.0), false));
        rig.scene.forget_size(node);
        let count = samples.borrow().len();
        rig.scene.retarget(node, Prop::SizeX, Bind::Set(Value::Scalar(50.0)), &rig.back).unwrap();
        assert_eq!(samples.borrow().len(), count);
        assert!(rig.scene.size_observers.is_empty());
    }

    #[test]
    fn native_bounds_keep_the_opposite_edge_when_parent_and_size_animate_together() {
        let Some(mut rig)=rig() else { return; };
        let parent=rig.ids.mint(); let node=rig.ids.mint();
        let mut patch=SinkPatch::default();
        patch.push(Op::New { id:parent,kind:NodeKind::Group,parent:Attach::Window,after:None });
        patch.push(Op::New { id:node,kind:NodeKind::Sprite,parent:Attach::Node(parent),after:None });
        patch.push(Op::Bind { id:node,prop:Prop::Size,bind:Bind::Set(Value::Vec2(Vector2::new(300.0,100.0))) });
        rig.apply(&mut patch);
        let samples=Rc::new(RefCell::new(Vec::new())); let output=samples.clone();
        rig.scene.observe_size(node,None,move |origin,size,_,moving| output.borrow_mut().push((origin,size,moving)),&rig.back).unwrap();
        rig._window.show();
        for (id,prop,value) in [(parent,Prop::Offset,Vector2::new(100.0,30.0)),(node,Prop::Size,Vector2::new(200.0,70.0))] {
            rig.scene.retarget(id,prop,Bind::Animate(Anim::Spring { to:Value::Vec2(value),tuning:Tuning::Layout,delay_ms:0 }),&rig.back).unwrap();
        }
        drop(rig.back.compositor.request_commit().unwrap());
        let start=std::time::Instant::now();
        while start.elapsed()<Duration::from_secs(2) {
            windows_window::pump();
            std::thread::sleep(Duration::from_millis(2));
        }
        let count=samples.borrow().len();
        assert!(count>5,"native callbacks must run");
        for (origin,size,_) in samples.borrow().iter() {
            assert!((origin.x+size.x-300.0).abs()<0.05,"horizontal bounds {origin:?}, {size:?}");
            assert!((origin.y+size.y-100.0).abs()<0.05,"vertical bounds {origin:?}, {size:?}");
        }
        assert!(!samples.borrow().last().unwrap().2);
        let quiet=std::time::Instant::now();
        while quiet.elapsed()<Duration::from_millis(100) {
            windows_window::pump();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(samples.borrow().len(),count,"settled observers must stop publishing");
    }

    #[test]
    fn offset_followers_accept_live_extents_on_both_axes_and_refuse_feedback() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let source = rig.sprite(&mut patch, 12.0);
        let bounds = rig.sprite(&mut patch, 120.0);
        let follower = rig.sprite(&mut patch, 120.0);
        rig.apply(&mut patch);
        let bind = |vertical, inset| Bind::FollowOffset {
            source,
            vertical,
            affine: Affine { m: 1.0, c: -6.0 },
            extent: Some((bounds, inset)),
            clamp: [0.0, 1.0],
        };
        for vertical in [false, true] {
            for size in [0.0, 12.0, 120.0] {
                rig.scene.retarget(bounds, Prop::Size,
                    Bind::Set(Value::Vec2(Vector2::new(size, size))), &rig.back).unwrap();
                rig.scene.retarget(follower, Prop::Opacity, bind(vertical, 12.0), &rig.back).unwrap();
            }
        }
        for inset in [f32::NAN, f32::INFINITY, -1.0] {
            assert!(rig.scene.retarget(follower, Prop::Opacity, bind(false, inset), &rig.back).is_err());
        }
        assert!(rig.scene.retarget(follower, Prop::SizeX, bind(false, 12.0), &rig.back).is_err());
        assert!(rig.scene.retarget(source, Prop::Opacity, bind(false, 12.0), &rig.back).is_err());
    }

    #[test]
    fn unit_paths_reuse_captures_and_keep_stroke_channels_through_resizes() {
        eprintln!("unit path storage: mask={}, sprite={}, aux={}, shape={}, fitted={}",
            size_of::<Mask>(), crate::arena::SPRITE_BYTES, size_of::<crate::arena::Aux>(),
            size_of::<crate::arena::ShapeState>(), size_of::<crate::arena::FittedShape>());
        let Some(mut rig) = rig() else { return };
        let geom = GeomId::raw(20, 1);
        let path = rig.back.path(&[PathVerb::Segment {
            from: Vector2::new(0.1, 0.2), to: Vector2::new(0.9, 0.8),
        }]).unwrap();
        rig.scene.res.declare(geom.erased(), ResObj::Geom(rig.back.compositor.create_path_geometry(&path), path));
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 120.0);
        let mask = Mask::Shape { geom, space: PathSpace::Unit, stroke: Some(StrokeStyle {
            width: 2.0, cap: Cap::Round, join: Join::Round, dash: DashId::NONE,
        }) };
        patch.push(Op::Mask { id: SpriteId(id), mask });
        patch.push(Op::Paint { id: SpriteId(id), paint: Paint::Solid(Radiance::new(30.0, 140.0, 170.0, 1.0)), halo: None });
        rig.apply(&mut patch);
        let shape = rig.scene.nodes.aux(id).unwrap().shape.as_ref().unwrap();
        let capture = shape.capture.brush.clone();
        let channels = shape.fitted.as_ref().unwrap().stroke.clone();
        assert_eq!(channels.scalar("StrokeThickness"), Some(2.0));
        rig.scene.retarget(id, Prop::StrokeThickness, Bind::Set(Value::Scalar(3.0)), &rig.back).unwrap();
        rig.scene.retarget(id, Prop::DashOffset, Bind::Set(Value::Scalar(0.25)), &rig.back).unwrap();
        for width in [0.0, 400.0, 60.0, 250.0] {
            rig.scene.retarget(id, Prop::Size, Bind::Set(Value::Vec2(Vector2::new(width, 120.0))), &rig.back).unwrap();
            rig.scene.rebind_holders(geom.erased(), &rig.back, rig.env).unwrap();
            let shape = rig.scene.nodes.aux(id).unwrap().shape.as_ref().unwrap();
            assert!(shape.capture.brush == capture);
            assert_eq!(channels.scalar("StrokeThickness"), Some(3.0));
            assert_eq!(channels.scalar("StrokeDashOffset"), Some(0.25));
        }
        for dpi in [144.0, 192.0, 96.0] {
            rig.env = Env::new(dpi, OutputTransform::for_display(DisplayCapability::Sdr, 203.0));
            patch.clear();
            rig.apply(&mut patch);
            let shape = rig.scene.nodes.aux(id).unwrap().shape.as_ref().unwrap();
            assert!(shape.capture.brush != capture);
            assert_eq!(shape.fitted.as_ref().unwrap().stroke.scalar("StrokeThickness"), Some(3.0));
            assert_eq!(shape.fitted.as_ref().unwrap().stroke.scalar("StrokeDashOffset"), Some(0.25));
            assert_eq!(shape.fitted.as_ref().unwrap().style, match mask { Mask::Shape { stroke, .. } => stroke, _ => None });
        }
        patch.clear();
        patch.push(Op::Mask { id: SpriteId(id), mask: Mask::None });
        rig.apply(&mut patch);
        assert!(rig.scene.nodes.aux(id).unwrap().shape.is_none());
    }

    #[test]
    fn presented_views_reuse_brushes_and_clear_all_holders_before_source_retirement() {
        eprintln!("atlas scene storage: paint={}, aux={}, view={}", size_of::<Paint>(),
            size_of::<crate::arena::Aux>(), size_of::<crate::realize::PresentedView>());
        let Some(mut rig) = rig() else { return };
        let surface = rig.back.raster_run(&[], &[], &[],
            Ink { size: Vector2::new(320.0, 44.0), ..Ink::default() }, rig.env)
            .unwrap().unwrap();
        let region = RegionId::raw(20, 1);
        rig.scene.res.declare(region.erased(), ResObj::Brush(rig.back.brush(&surface, Stretch::None), None));
        let mut patch = SinkPatch::default();
        let ids = [rig.sprite(&mut patch, 100.0), rig.sprite(&mut patch, 200.0)];
        for (id, sampling) in ids.into_iter().zip([RegionSampling::Pixels, RegionSampling::Fit]) {
            patch.push(Op::Paint { id: SpriteId(id), paint: Paint::PresentedView {
                region, view: RegionView { rect: [10.0, 2.0, 110.0, 22.0], sampling },
            }, halo: None });
        }
        rig.apply(&mut patch);
        let brushes = ids.map(|id| rig.scene.nodes.aux(id).unwrap().region_view.as_ref().unwrap().brush.clone());
        let live = rig.scene.census().visuals_live;
        for rect in [[f32::NAN, 0.0, 10.0, 10.0], [0.0, 0.0, 0.0, 10.0], [-1.0, 0.0, 10.0, 10.0]] {
            assert!(rig.scene.declare(ids[0], None, Some((Paint::PresentedView {
                region, view: RegionView { rect, sampling: RegionSampling::Fit },
            }, None)), &rig.back, rig.env).is_err());
        }
        for width in [400.0, 100.0, 250.0] {
            for (id, brush) in ids.into_iter().zip(&brushes) {
                rig.scene.retarget(id, Prop::Size, Bind::Set(Value::Vec2(Vector2::new(width, 20.0))), &rig.back).unwrap();
                rig.scene.realize(id, &rig.back, rig.env).unwrap();
                assert!(&rig.scene.nodes.aux(id).unwrap().region_view.as_ref().unwrap().brush == brush);
            }
        }
        for dpi in [144.0, 192.0, 96.0] {
            rig.scene.sync(&rig.back, Env::new(dpi, rig.env.output())).unwrap();
            assert_eq!(rig.scene.census().visuals_live, live);
        }
        rig.scene.clear_region(region, &rig.back, rig.env).unwrap();
        assert!(rig.scene.res.brush(region.erased()).is_none());
        for id in ids {
            assert!(rig.scene.nodes.aux(id).unwrap().region_view.is_none());
            assert!(rig.scene.nodes.painted(id).unwrap().chain.is_none());
        }
        rig.scene.res.declare(region.erased(), ResObj::Brush(rig.back.brush(&surface, Stretch::None), None));
        rig.scene.rebind_holders(region.erased(), &rig.back, rig.env).unwrap();
        for id in ids {
            assert!(rig.scene.nodes.aux(id).unwrap().region_view.is_some());
            rig.scene.release_subtree(id);
        }
        rig.scene.clear_region(region, &rig.back, rig.env).unwrap();
        assert!(rig.scene.res.obj(region.erased()).is_none());
    }

    #[test]
    fn pixel_sampling_sweep_tracks_run_and_region_lifetimes_but_not_ramps() {
        let Some(mut rig) = rig() else { return };
        let surface = rig.back.raster_run(&[], &[], &[],
            Ink { size: Vector2::new(20.0, 20.0), ..Ink::default() }, rig.env)
            .unwrap().unwrap();
        let run = RunId::raw(1, 1);
        let region = RegionId::raw(2, 1);
        let ramp = RampId::raw(3, 1);
        let run_brush = rig.back.brush(&surface, Stretch::None);
        let region_brush = rig.back.brush(&surface, Stretch::None);
        rig.scene.res.declare(run.erased(), ResObj::Brush(run_brush.clone(), Some(surface.clone())));
        rig.scene.res.declare(region.erased(), ResObj::Brush(region_brush.clone(), None));
        rig.scene.res.declare(ramp.erased(), ResObj::Brush(rig.back.brush(&surface, Stretch::Fill), Some(surface.clone())));
        for dpi in [144.0, 192.0, 96.0] {
            let env = Env::new(dpi, rig.env.output());
            rig.scene.sync(&rig.back, env).unwrap();
            let next = rig.back.brush(&surface, Stretch::None);
            rig.scene.res.declare(run.erased(), ResObj::Brush(next, Some(surface.clone())));
            assert!(rig.scene.res.brush(run.erased()).unwrap() == &run_brush);
            let brushes: Vec<_> = rig.scene.res.pixel_brushes().collect();
            assert_eq!(brushes.len(), 2);
            assert!(brushes.contains(&&run_brush));
            assert!(brushes.contains(&&region_brush));
        }
        let held = Some(Holding::Run(run));
        rig.scene.res.retain(held);
        rig.scene.res.disclaim(run.erased());
        assert_eq!(rig.scene.res.pixel_brushes().count(), 2);
        rig.scene.res.release(held);
        assert_eq!(rig.scene.res.pixel_brushes().count(), 1);
        rig.scene.res.disclaim(region.erased());
        assert_eq!(rig.scene.res.pixel_brushes().count(), 0);
    }

    #[test]
    fn layout_springs_are_retained_per_node_and_property() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let a = rig.sprite(&mut patch, 100.0);
        let b = rig.sprite(&mut patch, 200.0);
        rig.apply(&mut patch);
        for width in 101..141 {
            for id in [a, b] {
                for prop in [Prop::Size, Prop::Offset] {
                    rig.scene.retarget(id, prop, Bind::Animate(Anim::Spring {
                        to: Value::Vec2(Vector2::new(width as f32, 40.0)),
                        tuning: Tuning::Layout,
                        delay_ms: 0,
                    }), &rig.back).unwrap();
                }
                let springs = &rig.scene.nodes.aux(id).unwrap().layout_springs;
                assert_eq!(springs.len(), 2);
                assert!(matches!(springs[0].1, LayoutSpring::Vec2(_)));
                assert!(matches!(springs[1].1, LayoutSpring::Vec3(_)));
            }
        }
        for id in [a, b] {
            patch.push(Op::Drop { id, exit: Exit::None, origin: Vector2::zero(), bounds: None });
        }
        rig.apply(&mut patch);
        assert_eq!(rig.scene.audit().held, 0);
    }

    #[test]
    fn a_set_over_a_playing_spring_is_restated_after_its_batch() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);
        let spring = |x: f32| Op::Bind {
            id,
            prop: Prop::Offset,
            bind: Bind::Animate(Anim::Spring {
                to: Value::Vec2(Vector2::new(x, 0.0)),
                tuning: Tuning::Layout,
                delay_ms: 0,
            }),
        };
        patch.push(spring(80.0));
        rig.apply(&mut patch);
        patch.push(Op::Bind {
            id,
            prop: Prop::Offset,
            bind: Bind::Set(Value::Vec2(Vector2::new(20.0, 0.0))),
        });
        rig.apply(&mut patch);
        let tokens = rig.scene.motion.pending.iter().find_map(|pending| {
            match &pending.holds { PendingKind::Restate(tokens) => Some(tokens.clone()), _ => None }
        }).expect("the stopped spring has no settlement batch");
        assert_eq!(tokens.len(), 2);
        assert!(rig.scene.nodes.restate.is_empty(), "the pass kept groups it had handed on");
        patch.push(spring(60.0));
        rig.apply(&mut patch);
        for token in tokens { assert!(!rig.scene.nodes.finish_settle(token)); }
    }

    #[test]
    fn settlement_completion_cannot_overwrite_a_new_edit_or_a_recycled_node() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);
        patch.push(Op::Bind {
            id, prop: Prop::Size,
            bind: Bind::Animate(Anim::Spring {
                to: Value::Vec2(Vector2::new(80.0, 80.0)),
                tuning: Tuning::Layout, delay_ms: 0,
            }),
        });
        patch.push(Op::Bind {
            id, prop: Prop::Size,
            bind: Bind::Set(Value::Vec2(Vector2::new(60.0, 60.0))),
        });
        rig.apply(&mut patch);
        let first = rig.scene.motion.pending.iter().find_map(|pending| {
            match &pending.holds { PendingKind::Restate(tokens) => Some(tokens.clone()), _ => None }
        }).unwrap();
        assert_eq!(first.len(), 2);
        patch.push(Op::Bind {
            id, prop: Prop::SizeX, bind: Bind::Set(Value::Scalar(20.0)),
        });
        rig.apply(&mut patch);
        assert!(rig.scene.nodes.finish_settle(first[0]));
        assert!(rig.scene.nodes.finish_settle(first[1]));
        assert_eq!(rig.scene.nodes.size(id), Vector2::new(20.0, 60.0));
        patch.push(Op::Bind {
            id, prop: Prop::Size,
            bind: Bind::Animate(Anim::Spring {
                to: Value::Vec2(Vector2::new(80.0, 80.0)),
                tuning: Tuning::Layout, delay_ms: 0,
            }),
        });
        patch.push(Op::Bind {
            id, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2::new(30.0, 30.0))),
        });
        rig.apply(&mut patch);
        let latest = rig.scene.motion.pending.iter().rev().find_map(|pending| {
            match &pending.holds { PendingKind::Restate(tokens) => Some(tokens.clone()), _ => None }
        }).unwrap();
        rig.scene.release_subtree(id);
        for token in latest { assert!(!rig.scene.nodes.finish_settle(token)); }
    }

    #[test]
    fn a_re_declared_sprite_realizes_nothing_a_second_time() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        patch.push(Op::Mask {
            id: SpriteId(id),
            mask: FILL,
        });
        patch.push(Op::Paint {
            id: SpriteId(id),
            paint: Paint::Solid(Radiance::new(0.5, 0.5, 0.5, 1.0)),
            halo: None,
        });
        assert!(rig.apply(&mut patch));

        // The same declaration again: both halves are equal and the chain is fresh, so
        // neither rebuilds and both are counted as absorbed.
        let before = *rig.scene.census();
        patch.push(Op::Mask {
            id: SpriteId(id),
            mask: FILL,
        });
        patch.push(Op::Paint {
            id: SpriteId(id),
            paint: Paint::Solid(Radiance::new(0.5, 0.5, 0.5, 1.0)),
            halo: None,
        });
        rig.apply(&mut patch);
        let after = *rig.scene.census();
        assert_eq!(after.props_skipped, before.props_skipped + 2);
        assert_eq!(after.visuals_minted, before.visuals_minted);
    }

    /// A sprite that paints a box and nothing else carries no side row.
    ///
    /// The row is four bytes of absence, and the eighteen aux channels, the clip, the shape
    /// and the glow all live behind it. A pass that minted one per sprite would put the side
    /// row on every node in the tree while reporting nothing.
    #[test]
    fn a_plain_sprite_carries_no_side_row_and_a_clipped_one_does() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let plain = rig.sprite(&mut patch, 40.0);
        patch.push(Op::Mask {
            id: SpriteId(plain),
            mask: FILL,
        });
        patch.push(Op::Paint {
            id: SpriteId(plain),
            paint: Paint::Solid(Radiance::new(0.5, 0.5, 0.5, 1.0)),
            halo: None,
        });
        let clipped = rig.sprite(&mut patch, 40.0);
        patch.push(Op::Clip {
            id: clipped,
            clip: Clip::Rect {
                l: 0.0,
                t: 0.0,
                r: 40.0,
                b: 40.0,
                radius: Corners::default(),
            },
        });
        rig.apply(&mut patch);

        assert!(
            !rig.scene.nodes.has_aux(plain),
            "a box mask minted a side row"
        );
        assert!(rig.scene.nodes.has_aux(clipped), "a clip needs one");
    }

    /// A halo casts at the sigma it declared and at full opacity until a channel says
    /// otherwise, and a rebind under a lost device restates the channel rather than the
    /// declaration.
    #[test]
    fn a_declared_halo_seeds_its_channels_once_and_a_rebind_keeps_them() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        patch.push(Op::Mask {
            id: SpriteId(id),
            mask: FILL,
        });
        patch.push(Op::Paint {
            id: SpriteId(id),
            paint: Paint::Solid(Radiance::new(0.5, 0.5, 0.5, 1.0)),
            halo: Some(Halo {
                sigma: 12.0,
                tint: Radiance::new(0.2, 0.6, 0.9, 1.0),
                offset: Vector2::zero(),
            }),
        });
        rig.apply(&mut patch);
        let sigma = PROPS[Prop::GlowSigma as usize].chan;
        assert_eq!(rig.scene.nodes.chan(id, sigma), 12.0);
        assert_eq!(rig.scene.nodes.chan(id, sigma + 1), 1.0);
        assert!(rig.scene.nodes.aux(id).is_some_and(|aux| aux.glow.is_some()));

        patch.push(Op::Bind {
            id,
            prop: Prop::GlowOpacity,
            bind: Bind::Set(Value::Scalar(0.3)),
        });
        rig.apply(&mut patch);
        rig.scene.device_lost(&rig.back, rig.env).expect("rebuilt");
        assert_eq!(rig.scene.nodes.chan(id, sigma + 1), 0.3);
        assert_eq!(rig.scene.nodes.chan(id, sigma), 12.0);
    }

    /// A group's clip bounds its content and not its own chrome: once a clipped group
    /// carries chrome, the chrome stays on the group's visual and the content moves onto a
    /// carrier holding the clip, in either order of arrival, and later content lands in
    /// the carrier whatever sibling its `after` names.
    #[test]
    fn a_clipped_group_keeps_its_chrome_and_halo_outside_its_content_clip() {
        let Some(mut rig) = rig() else { return };
        for clip_first in [true, false] {
            let mut patch = SinkPatch::default();
            let group = rig.ids.mint();
            patch.push(Op::New { id: group, kind: NodeKind::Group, parent: Attach::Window, after: None });
            patch.push(Op::Bind { id: group, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2::new(120.0, 80.0))) });
            let content = rig.ids.mint();
            patch.push(Op::New { id: content, kind: NodeKind::Sprite, parent: Attach::Node(group), after: None });
            let clip = Op::Clip { id: group, clip: Clip::RoundedBounds(Corners::all(8.0)) };
            if clip_first {
                patch.push(clip);
            }
            let fill = rig.ids.mint();
            patch.push(Op::New { id: fill, kind: NodeKind::Sprite, parent: Attach::Chrome(group), after: None });
            patch.push(Op::Bind { id: fill, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2::new(120.0, 80.0))) });
            patch.push(Op::Mask { id: SpriteId(fill), mask: FILL });
            patch.push(Op::Paint {
                id: SpriteId(fill),
                paint: Paint::Solid(Radiance::new(0.5, 0.5, 0.5, 1.0)),
                halo: Some(Halo { sigma: 8.0, tint: Radiance::new(0.2, 0.6, 0.9, 1.0), offset: Vector2::zero() }),
            });
            if !clip_first {
                patch.push(clip);
            }
            rig.apply(&mut patch);

            let own = rig.scene.nodes.visual(group).and_then(Visual::as_container).expect("a group");
            let carrier = rig.scene.nodes.aux(group).and_then(|aux| aux.content.clone())
                .expect("a clipped group with chrome is split");
            assert!(rig.scene.nodes.is_chrome(fill) && !rig.scene.nodes.is_chrome(content));
            // The group holds its chrome, beneath the carrier; the carrier holds the content.
            assert_eq!(own.children().count(), 2, "chrome and the carrier");
            assert_eq!(carrier.children().count(), 1, "the content alone");
            assert!(rig.scene.nodes.visual(content).and_then(Visual::parent).is_some_and(|held| held.children().count() == 1));
            assert!(rig.scene.nodes.visual(fill).and_then(Visual::parent).is_some_and(|held| held.children().count() == 2));
            // The lit chrome kept its halo, and its clip host is its paint, not the group.
            assert!(rig.scene.nodes.aux(fill).is_some_and(|aux| aux.glow.is_some()));

            // Later content whose `after` names the chrome still lands in the carrier.
            let late = rig.ids.mint();
            patch.push(Op::New { id: late, kind: NodeKind::Sprite, parent: Attach::Node(group), after: Some(fill) });
            rig.apply(&mut patch);
            assert_eq!(carrier.children().count(), 2);
            assert_eq!(own.children().count(), 2);

            // Unclipping keeps the carrier, cleared, and destroying content empties it.
            patch.push(Op::Clip { id: group, clip: Clip::None });
            patch.push(Op::Drop { id: late, exit: Exit::None, origin: Point::default(), bounds: None });
            rig.apply(&mut patch);
            assert!(rig.scene.nodes.aux(group).is_some_and(|aux| aux.content.is_some()));
            assert_eq!(carrier.children().count(), 1);

            patch.push(Op::Drop { id: group, exit: Exit::None, origin: Point::default(), bounds: None });
            rig.apply(&mut patch);
        }
    }

    /// A group with a clip and no chrome is not split: an ordinary clipped container pays
    /// no carrier.
    #[test]
    fn a_clipped_group_without_chrome_stays_one_visual() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let group = rig.ids.mint();
        patch.push(Op::New { id: group, kind: NodeKind::Group, parent: Attach::Window, after: None });
        let content = rig.ids.mint();
        patch.push(Op::New { id: content, kind: NodeKind::Sprite, parent: Attach::Node(group), after: None });
        patch.push(Op::Clip { id: group, clip: Clip::Bounds });
        rig.apply(&mut patch);
        assert!(rig.scene.nodes.aux(group).is_some_and(|aux| aux.content.is_none()));
        let own = rig.scene.nodes.visual(group).and_then(Visual::as_container).expect("a group");
        assert_eq!(own.children().count(), 1);
    }

    /// Renders two haloed sprites and the glow pipeline's every intermediate, so the
    /// pipeline can be bisected by eye in one capture.
    ///
    /// Ignored because the window stays open: run it alone, capture the window with
    /// guishot while the five-second hold runs, and read the panels:
    ///
    /// 1. the glow the full graph paints,
    /// 2. the raw capture, the blur's input, painted sharp,
    /// 3. the caster's brush — the silhouette — painted sharp.
    ///
    /// A panel dark where the one above it is bright names the first dead link.
    #[test]
    #[ignore]
    fn glow_pipeline_debug() {
        // Held for the run: dropping it would shut the dispatcher queue down under the
        // window's own session.
        let Ok(_queue) = DispatcherQueueController::create_on_current_thread() else {
            eprintln!("skipped: no dispatcher queue in this session");
            return;
        };
        let Ok(window) = windows_window::Window::new("glow pipeline debug")
            .size(1100, 900)
            .create()
        else {
            eprintln!("skipped: no window in this session");
            return;
        };
        let Ok(gpu) = windows_d2d::Gpu::for_window() else {
            eprintln!("skipped: no Direct2D device in this session");
            return;
        };
        let comp = Compositor::new().expect("a compositor");
        let back = Backends::new(comp.clone(), &gpu, FontLadder::default()).expect("the backends");
        // Display scale 1.5, the case the GUI cards run at.
        let env = Env::new(
            144.0,
            OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
        );
        let mut scene =
            Scene::new_at(window.handle(), &back, env, BackdropSpec::default()).expect("a scene");

        // A ramp resource, the paint the GUI's card headers carry.
        let ramp = RampId::raw(3, 1);
        let ramp_stops = [
            (0u16, Radiance::new(0.5, 0.55, 0.65, 1.0)),
            (65535, Radiance::new(0.2, 0.6, 0.9, 1.0)),
        ];
        let ramp_surface = back
            .raster_ramp(&ramp_stops, Spread::Vertical, env, None)
            .expect("the ramp rasterized")
            .expect("a ramp surface");
        scene.res.declare(
            ramp.erased(),
            ResObj::Brush(back.brush(&ramp_surface, Stretch::Fill), Some(ramp_surface.clone())),
        );

        // Four sprite nodes: [1] the solid control, [2] the ramp paint, [3] a displaced
        // halo, [4] one that stage two re-points and drives through the channels.
        let mut patch = SinkPatch::default();
        let mut ids = Ids::default();
        let mut node = |paint: Paint, offset: (f32, f32), halo: Halo, patch: &mut SinkPatch| {
            let id = ids.mint();
            patch.push(Op::New { id, kind: NodeKind::Sprite, parent: Attach::Window, after: None });
            patch.push(Op::Bind { id, prop: Prop::Offset, bind: Bind::Set(Value::Vec2(Vector2 { x: offset.0, y: offset.1 })) });
            patch.push(Op::Bind { id, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2 { x: 300.0, y: 200.0 })) });
            patch.push(Op::Mask { id: SpriteId(id), mask: Mask::Box { radius: Corners::all(12.0) } });
            patch.push(Op::Paint {
                id: SpriteId(id),
                paint,
                halo: Some(halo),
            });
            id
        };
        let control = node(
            Paint::Solid(Radiance::new(0.5, 0.55, 0.65, 1.0)),
            (40.0, 40.0),
            Halo { sigma: 14.0, tint: Radiance::new(0.2, 0.6, 0.9, 1.0), offset: Vector2::zero() },
            &mut patch,
        );
        let ramp_node = node(
            Paint::Ramp(ramp),
            (40.0, 320.0),
            Halo { sigma: 14.0, tint: Radiance::new(0.2, 0.6, 0.9, 1.0), offset: Vector2::zero() },
            &mut patch,
        );
        let displaced = node(
            Paint::Solid(Radiance::new(0.5, 0.55, 0.65, 1.0)),
            (40.0, 700.0),
            Halo { sigma: 14.0, tint: Radiance::new(0.2, 0.6, 0.9, 1.0), offset: Vector2::new(10.0, 6.0) },
            &mut patch,
        );
        let repoint = node(
            Paint::Solid(Radiance::new(0.5, 0.55, 0.65, 1.0)),
            (640.0, 700.0),
            Halo { sigma: 14.0, tint: Radiance::new(0.2, 0.6, 0.9, 1.0), offset: Vector2::zero() },
            &mut patch,
        );
        scene.apply(&mut patch, &back, env).expect("the pass applied");

        // The pipeline laid out beside itself for the ramp and the re-point nodes, one row
        // each: [left] the silhouette the caster paints, [middle] the glow as built,
        // [right] the raw capture, the blur's input.
        fn facts(scene: &Scene, back: &Backends, id: NodeId, tag: &str) {
            let Some(glow) = scene.nodes.aux(id).and_then(|aux| aux.glow.as_ref()) else {
                println!("{tag}: no glow minted");
                return;
            };
            let (host, caster, halo) = (glow.host.size(), glow.caster.size(), glow.sprite.size());
            let sigma = glow.props.scalar("Sigma");
            let status = back
                .minted_glow_factory()
                .map_or_else(|| "no factory".to_string(), |f| format!("{:?}", f.load_status()));
            println!(
                "{tag}: host=({:.0},{:.0}) caster=({:.0},{:.0}) halo=({:.0},{:.0}) sigma={sigma:?} opacity={:.2} factory-load={status}",
                host.x, host.y, caster.x, caster.y, halo.x, halo.y, glow.sprite.opacity(),
            );
        }
        fn panels(scene: &Scene, comp: &Compositor, id: NodeId, tag: &str) {
            let Some(glow) = scene.nodes.aux(id).and_then(|aux| aux.glow.as_ref()) else {
                return;
            };
            let bleed = glow.bleed;
            let (span_x, span_y) = (300.0 + 2.0 * bleed, 200.0 + 2.0 * bleed);
            let base = glow.sprite.offset();
            let content = scene.content.children();
            if let Some(chain) = scene.nodes.painted(id).and_then(|row| row.chain.clone()) {
                let sprite = comp.create_sprite_visual();
                sprite.set_size(span_x, span_y);
                sprite.set_offset(base.x, base.y + 260.0, 0.0);
                sprite.set_brush(&chain);
                content.insert_at_top(&sprite);
            } else {
                println!("{tag}: no chain to paint");
            }
            let sprite = comp.create_sprite_visual();
            sprite.set_size(span_x, span_y);
            sprite.set_offset(base.x + 400.0, base.y + 260.0, 0.0);
            sprite.set_brush(&glow.brush);
            content.insert_at_top(&sprite);
            let sprite = comp.create_sprite_visual();
            sprite.set_size(span_x, span_y);
            sprite.set_offset(base.x + 800.0, base.y + 260.0, 0.0);
            sprite.set_brush(&glow.capture.brush);
            content.insert_at_top(&sprite);
        }

        // Commit and pump: the effect factory's load completes with the commit, and
        // the window draws only while this thread pumps.
        fn pump_hold(comp: &Compositor, seconds: u64, tag: &str, scene: &Scene, back: &Backends, nodes: &[NodeId]) {
            let until = std::time::Instant::now() + core::time::Duration::from_secs(seconds);
            while std::time::Instant::now() < until {
                let _ = comp.request_commit();
                windows_window::pump();
                std::thread::sleep(core::time::Duration::from_millis(10));
            }
            for (i, id) in nodes.iter().enumerate() {
                facts(scene, back, *id, &format!("{tag} node{i}"));
            }
        }

        // First hold: the panel row sits 260 below each node, so stage two's captures
        // land after this one.
        panels(&scene, &comp, ramp_node, "ramp");
        panels(&scene, &comp, repoint, "repoint");
        pump_hold(&comp, 5, "stage1", &scene, &back, &[control, ramp_node, displaced, repoint]);

        // Stage two: a re-point of the control through a changed paint, and channel-driven
        // sigma and opacity writes on it — its panel row is the one on the backdrop, so the
        // widening and the dimming land where the capture can read them.
        let mut patch = SinkPatch::default();
        patch.push(Op::Paint {
            id: SpriteId(control),
            paint: Paint::Solid(Radiance::new(0.45, 0.5, 0.6, 1.0)),
            halo: Some(Halo { sigma: 14.0, tint: Radiance::new(0.2, 0.6, 0.9, 1.0), offset: Vector2::zero() }),
        });
        patch.push(Op::Bind { id: control, prop: Prop::GlowSigma, bind: Bind::Set(Value::Scalar(24.0)) });
        patch.push(Op::Bind { id: control, prop: Prop::GlowOpacity, bind: Bind::Set(Value::Scalar(0.68)) });
        scene.apply(&mut patch, &back, env).expect("the pass applied");
        pump_hold(&comp, 10, "stage2", &scene, &back, &[control, ramp_node, displaced, repoint]);
    }

    #[test]
    fn rounded_bounds_keep_native_size_bindings_across_radius_resize_and_recovery() {
        let mut rig = rig().expect("native compositor");
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        patch.push(Op::Clip { id, clip: Clip::RoundedBounds(Corners::all(6.0)) });
        let initial = rig.scene.census().animations;
        rig.apply(&mut patch);
        assert_eq!(rig.scene.census().animations, initial + 2);
        for radius in [6.0, 8.0] {
            patch.push(Op::Clip { id, clip: Clip::RoundedBounds(Corners::all(radius)) });
            patch.push(Op::Bind { id, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2::new(100.0, 60.0))) });
            rig.apply(&mut patch);
            assert_eq!(rig.scene.census().animations, initial + 2);
            for prop in [Prop::ClipR, Prop::ClipB] {
                assert_eq!(rig.scene.nodes.held(id, desc(prop)), Held::Bound);
            }
            assert_eq!(rig.scene.nodes.chan(id, desc(Prop::CornerTopLeftX).chan), radius);
        }
        rig.scene.device_lost(&rig.back, rig.env).expect("device recovery");
        for prop in [Prop::ClipR, Prop::ClipB] {
            assert_eq!(rig.scene.nodes.held(id, desc(prop)), Held::Bound);
        }
        assert_eq!(rig.scene.census().animations, initial + 4);
        patch.push(Op::Clip { id, clip: Clip::Rect { l: 0.0, t: 0.0, r: 20.0, b: 30.0, radius: Corners::default() } });
        rig.apply(&mut patch);
        assert_eq!(rig.scene.nodes.held(id, desc(Prop::ClipR)), Held::Free);
        assert_eq!(rig.scene.nodes.chan(id, desc(Prop::ClipR).chan), 20.0);
        patch.push(Op::Clip { id, clip: Clip::RoundedBounds(Corners::all(6.0)) });
        rig.apply(&mut patch);
        assert_eq!(rig.scene.nodes.held(id, desc(Prop::ClipB)), Held::Bound);
        patch.push(Op::Clip { id, clip: Clip::None });
        rig.apply(&mut patch);
        assert!(rig.scene.nodes.aux(id).unwrap().clip.is_none());
        let before = rig.scene.census().animations;
        for _ in 0..20 { rig.apply(&mut patch); }
        assert_eq!(rig.scene.census().animations, before);
    }

    #[test]
    fn a_restated_clip_writes_nothing_the_second_time() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        let clip = Clip::Rect {
            l: 0.0,
            t: 0.0,
            r: 40.0,
            b: 40.0,
            radius: Corners::all(4.0),
        };
        patch.push(Op::Clip { id, clip });
        rig.apply(&mut patch);

        let before = *rig.scene.census();
        patch.push(Op::Clip { id, clip });
        rig.apply(&mut patch);
        let after = *rig.scene.census();
        // The declaration shadow absorbs it before the twelve channels are even compared.
        assert_eq!(after.props_skipped, before.props_skipped + 1);
        assert_eq!(after.props_written, before.props_written);
    }

    #[test]
    fn an_unchanged_bind_writes_nothing() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);

        let before = *rig.scene.census();
        patch.push(Op::Bind {
            id,
            prop: Prop::Size,
            bind: Bind::Set(Value::Vec2(Vector2 { x: 40.0, y: 40.0 })),
        });
        let changed = rig.apply(&mut patch);
        let after = *rig.scene.census();
        assert_eq!(after.props_written, before.props_written);
        assert_eq!(after.props_skipped, before.props_skipped + 1);
        // An op was applied, so the pass is a change even though nothing was written.
        assert!(changed);
    }

    #[test]
    fn disabled_animation_snaps_fades_and_omits_exit_ghosts() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);
        rig.scene.set_springs_enabled(false);
        let before = rig.scene.census().animations;
        let frames = patch.push_frames(&[
            (0.0, Value::Scalar(0.0), Easing::Linear),
            (1.0, Value::Scalar(1.0), Easing::Linear),
        ]);
        patch.push(Op::Bind { id, prop: Prop::Opacity, bind: Bind::Animate(Anim::Frames {
            frames, duration_ms: 200, iterations: Iterations::Count(1),
        }) });
        rig.apply(&mut patch);
        assert_eq!(rig.scene.census().animations, before);
        assert_eq!(rig.scene.nodes.chan(id, desc(Prop::Opacity).chan), 1.0);
        let mut events = Vec::new();
        rig.scene.drain_events(&mut events);
        assert!(events.iter().any(|event| matches!(event,
            SceneEvent::AnimationCompleted { node, prop: Prop::Opacity } if *node == id)));
        patch.push(Op::Drop { id, exit: Exit::Fade { ms: 200 }, origin: Vector2::zero(), bounds: None });
        rig.apply(&mut patch);
        assert_eq!(rig.scene.census().animations, before);
    }

    #[test]
    fn disabled_springs_snap_app_and_front_targets_and_can_resume() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);
        let spring = |to| Bind::Animate(Anim::Spring {
            to: Value::Scalar(to), tuning: Tuning::Chrome, delay_ms: 0,
        });
        rig.scene.set_springs_enabled(false);
        let before = rig.scene.census().animations;
        patch.push(Op::Bind { id, prop: Prop::OffsetX, bind: spring(12.0) });
        rig.apply(&mut patch);
        rig.scene.retarget(id, Prop::Opacity, spring(0.4), &rig.back).unwrap();
        assert_eq!(rig.scene.census().animations, before);
        assert_eq!(rig.scene.nodes.chan(id, desc(Prop::OffsetX).chan), 12.0);
        assert_eq!(rig.scene.nodes.chan(id, desc(Prop::Opacity).chan), 0.4);
        assert_eq!(rig.scene.nodes.held(id, desc(Prop::OffsetX)), Held::Free);
        rig.scene.set_springs_enabled(true);
        rig.scene.retarget(id, Prop::Opacity, spring(1.0), &rig.back).unwrap();
        assert_eq!(rig.scene.census().animations, before + 1);
    }

    /// A spring states its wait to the compositor, which measures it, so a held fade costs
    /// nothing on this thread and a retarget arriving inside the wait replaces it.
    ///
    /// Run against a real spring object because the wait is a property of one: a natural-motion
    /// animation that refused a delay would fail here and nowhere else.
    #[test]
    fn a_spring_carries_its_wait_and_a_retarget_inside_it_replaces_the_animation() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);

        let before = *rig.scene.census();
        let held = Bind::Animate(Anim::Spring {
            to: Value::Scalar(0.0),
            tuning: Tuning::Chrome,
            delay_ms: 700,
        });
        rig.scene
            .retarget(id, Prop::Opacity, held, &rig.back)
            .expect("the compositor refused a held spring");
        let now = Bind::Animate(Anim::Spring {
            to: Value::Scalar(1.0),
            tuning: Tuning::Chrome,
            delay_ms: 0,
        });
        rig.scene
            .retarget(id, Prop::Opacity, now, &rig.back)
            .expect("the compositor refused the replacement");
        let after = *rig.scene.census();
        assert_eq!(
            after.animations,
            before.animations + 2,
            "one of the two springs never started"
        );
    }

    #[test]
    fn finite_keyframes_complete_without_another_patch() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);
        let frames = patch.push_frames(&[
            (0.0, Value::Scalar(0.0), Easing::Linear),
            (1.0, Value::Scalar(1.0), Easing::Linear),
        ]);
        patch.push(Op::Bind {
            id,
            prop: Prop::Opacity,
            bind: Bind::Animate(Anim::Frames {
                frames,
                duration_ms: 20,
                iterations: Iterations::Count(1),
            }),
        });
        rig.apply(&mut patch);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut events = Vec::new();
        loop {
            windows_window::pump();
            rig.scene.drain_events(&mut events);
            if events.iter().any(|event| matches!(event,
                SceneEvent::AnimationCompleted { node, prop: Prop::Opacity } if *node == id)) {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "finite keyframes never completed");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rig.scene.motion.pending.is_empty());
    }

    #[test]
    fn local_translation_survives_layout_and_anchor_writes_without_scaling() {
        let mut rig = rig().expect("native compositor");
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        for (prop, value) in [(Prop::TranslationX, 2.0), (Prop::TranslationY, -3.0)] {
            patch.push(Op::Bind { id, prop, bind: Bind::Set(Value::Scalar(value)) });
        }
        rig.apply(&mut patch);
        let matrix = rig.scene.nodes.visual(id).unwrap().transform_matrix();
        assert_eq!(matrix, windows_numerics::Matrix4x4::translation(2.0, -3.0, 0.0));
        for prop in [Prop::TranslationX, Prop::TranslationY] {
            let frames = patch.push_frames(&[
                (0.0, Value::Scalar(0.0), Easing::Linear),
                (1.0, Value::Scalar(-3.0), Easing::Linear),
            ]);
            patch.push(Op::Bind { id, prop, bind: Bind::Animate(Anim::Frames {
                frames, duration_ms: 100, iterations: Iterations::Count(1),
            }) });
        }
        rig.apply(&mut patch);
        for step in 1..20 {
            for (prop, value) in [
                (Prop::Offset, Value::Vec2(Vector2::new(step as f32, 10.0))),
                (Prop::Size, Value::Vec2(Vector2::new(40.0 + step as f32, 60.0))),
                (Prop::AnchorY, Value::Scalar(step as f32 / 100.0)),
            ] {
                patch.push(Op::Bind { id, prop, bind: Bind::Set(value) });
            }
            rig.apply(&mut patch);
            assert_eq!(rig.scene.motion.pending.len(), 2);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut events = Vec::new();
        while events.len() < 2 {
            windows_window::pump();
            rig.scene.drain_events(&mut events);
            assert!(std::time::Instant::now() < deadline, "translation completion was lost");
            std::thread::sleep(Duration::from_millis(5));
        }
        for prop in [Prop::TranslationX, Prop::TranslationY] {
            assert!(events.iter().any(|event| matches!(event,
                SceneEvent::AnimationCompleted { node, prop: completed } if *node == id && *completed == prop)));
            rig.scene.retarget(id, prop, Bind::Animate(Anim::Spring {
                to: Value::Scalar(-3.0), tuning: Tuning::Chrome, delay_ms: 0,
            }), &rig.back).unwrap();
            assert_eq!(rig.scene.nodes.held(id, desc(prop)), Held::Playing);
        }
        rig.scene.set_springs_enabled(false);
        let reduced = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);
        let before = rig.scene.census().animations;
        rig.scene.retarget(reduced, Prop::TranslationY, Bind::Animate(Anim::Spring {
            to: Value::Scalar(-7.0), tuning: Tuning::Chrome, delay_ms: 0,
        }), &rig.back).unwrap();
        assert_eq!(rig.scene.census().animations, before);
        assert_eq!(rig.scene.nodes.visual(reduced).unwrap().transform_matrix().m42, -7.0);
        assert_eq!(rig.scene.nodes.visual(id).unwrap().scale(), Vector3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn anchor_slide_survives_layout_offset_and_size_writes() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        rig.apply(&mut patch);
        for prop in [Prop::AnchorX, Prop::AnchorY] {
            let frames = patch.push_frames(&[
                (0.0, Value::Scalar(-1.0), Easing::Linear),
                (1.0, Value::Scalar(0.0), Easing::Linear),
            ]);
            patch.push(Op::Bind {
                id, prop,
                bind: Bind::Animate(Anim::Frames {
                    frames, duration_ms: 100, iterations: Iterations::Count(1),
                }),
            });
        }
        rig.apply(&mut patch);
        for step in 1..20 {
            for (prop, value) in [
                (Prop::Offset, Vector2 { x: step as f32, y: 10.0 }),
                (Prop::Size, Vector2 { x: 40.0 + step as f32, y: 40.0 }),
            ] {
                patch.push(Op::Bind { id, prop, bind: Bind::Set(Value::Vec2(value)) });
            }
            rig.apply(&mut patch);
            assert_eq!(rig.scene.motion.pending.len(), 2);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut events = Vec::new();
        while events.len() < 2 {
            windows_window::pump();
            rig.scene.drain_events(&mut events);
            assert!(std::time::Instant::now() < deadline, "slide completion was lost");
            std::thread::sleep(Duration::from_millis(5));
        }
        for prop in [Prop::AnchorX, Prop::AnchorY] {
            assert!(events.iter().any(|event| matches!(event,
                SceneEvent::AnimationCompleted { node, prop: completed } if *node == id && *completed == prop)));
        }
    }

    #[test]
    fn drag_preview_preserves_subtree_and_restores_before_geometry_or_retirement() {
        let mut rig = rig().expect("native composition is required");
        let mut patch = SinkPatch::default();
        let parent = rig.ids.mint();
        patch.push(Op::New { id: parent, kind: NodeKind::Group, parent: Attach::Window, after: None });
        let mut previous = None;
        let mut ids = Vec::new();
        for _ in 0..3 {
            let id = rig.ids.mint();
            patch.push(Op::New { id, kind: NodeKind::Group, parent: Attach::Node(parent), after: previous });
            patch.push(Op::Bind { id, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2::new(80.0, 40.0))) });
            ids.push(id);
            previous = Some(id);
        }
        let child = rig.ids.mint();
        patch.push(Op::New { id: child, kind: NodeKind::Sprite, parent: Attach::Node(ids[1]), after: None });
        rig.apply(&mut patch);
        let source = rig.scene.nodes.visual(ids[1]).unwrap().clone();
        let held_child = rig.scene.nodes.visual(child).unwrap().clone();
        let original = source.parent().unwrap();
        let source_offset = source.offset();
        let source_size = source.size();
        let count = rig.scene.census().visuals_live;
        for _ in 0..3 {
            assert!(rig.scene.begin_drag_preview(ids[1], &rig.back));
            assert_eq!(original.children().count(), 2);
            assert_eq!(rig.scene.census().visuals_live, count + 1);
            assert_eq!(source.as_container().unwrap().children().count(), 1);
            assert_eq!(held_child.parent().unwrap().children().count(), 1);
            let minted = rig.scene.census().visuals_minted;
            rig.scene.move_drag_preview(Vector2::new(16.0, 24.0));
            let moved = *rig.scene.census();
            for _ in 0..100 { rig.scene.move_drag_preview(Vector2::new(16.0, 24.0)); }
            rig.scene.move_drag_preview(Vector2::new(f32::NAN, 0.0));
            assert_eq!(*rig.scene.census(), moved);
            assert_eq!(rig.scene.census().visuals_minted, minted);
            assert_eq!(source.offset(), source_offset);
            assert_eq!(source.size(), source_size);
            assert!(rig.scene.audit().agrees());
            rig.scene.end_drag_preview();
            assert_eq!(rig.scene.census().visuals_live, count);
            assert_eq!(original.children().count(), 3);
            assert_eq!(source.parent().unwrap().children().count(), 3);
        }
        assert!(rig.scene.begin_drag_preview(ids[1], &rig.back));
        let unrelated = rig.ids.mint();
        patch.push(Op::New { id: unrelated, kind: NodeKind::Group, parent: Attach::Overlay, after: None });
        patch.push(Op::Drop { id: unrelated, exit: Exit::None, origin: Point::zero(), bounds: None });
        rig.apply(&mut patch);
        assert!(rig.scene.lift.is_some(), "unrelated overlays cannot cancel the lift");
        patch.push(Op::Bind { id: parent, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2::new(300.0, 100.0))) });
        rig.apply(&mut patch);
        assert!(rig.scene.lift.is_none());
        assert!(rig.scene.begin_drag_preview(ids[1], &rig.back));
        rig.env = Env::new(144.0, rig.env.output());
        rig.apply(&mut patch);
        assert!(rig.scene.lift.is_none());
        assert!(rig.scene.begin_drag_preview(ids[1], &rig.back));
        patch.push(Op::Drop { id: parent, exit: Exit::None, origin: Point::zero(), bounds: None });
        rig.apply(&mut patch);
        assert!(rig.scene.lift.is_none());
        assert_eq!(rig.scene.census().visuals_live, 0);
        assert!(rig.scene.audit().agrees());
        assert!(!rig.scene.begin_drag_preview(ids[1], &rig.back));
    }

    #[test]
    fn collapse_exit_keeps_native_descendants_and_cancels_on_reopen_or_retirement() {
        let Some(mut rig) = rig() else { return };
        for finish in 0..4 {
            let mut patch = SinkPatch::default();
            let parent = rig.sprite(&mut patch, 200.0);
            let root = rig.ids.mint();
            let child = rig.ids.mint();
            patch.push(Op::New { id: root, kind: NodeKind::Group, parent: Attach::Node(parent), after: None });
            patch.push(Op::Bind { id: root, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2::new(100.0, 80.0))) });
            patch.push(Op::New { id: child, kind: NodeKind::Sprite, parent: Attach::Node(root), after: None });
            rig.apply(&mut patch);
            let source = rig.scene.nodes.visual(root).unwrap().as_container().unwrap();
            let group = rig.scene.nodes.visual(parent).unwrap().as_container().unwrap();
            patch.clear();
            patch.push(Op::Drop { id: root, exit: Exit::Collapse, origin: Vector2::zero(), bounds: None });
            rig.apply(&mut patch);
            assert!(!rig.scene.nodes.live(root));
            assert!(!rig.scene.nodes.live(child));
            assert_eq!(source.children().count(), 1);
            assert_eq!(source.size(), Vector2::new(100.0, 80.0));
            let carrier = rig.scene.motion.pending.iter().find_map(|pending| match &pending.holds {
                PendingKind::Collapse { carrier, .. } => Some(carrier.clone()), _ => None,
            }).expect("native exit remains under its original parent");
            group.properties().insert_scalar("Identity", 1.0);
            carrier.properties().insert_scalar("Identity", 2.0);
            assert_eq!(carrier.parent().unwrap().properties().scalar("Identity"), Some(1.0));
            assert_eq!(source.parent().unwrap().properties().scalar("Identity"), Some(2.0));
            assert!(rig.scene.audit().agrees());
            patch.clear();
            match finish {
                0 => {
                    for pending in &rig.scene.motion.pending { pending.done.set(true); }
                    rig.scene.retire();
                }
                1 => {
                    let replacement = rig.ids.mint();
                    patch.push(Op::New { id: replacement, kind: NodeKind::Group, parent: Attach::Node(parent), after: None });
                    rig.apply(&mut patch);
                }
                2 => {
                    patch.push(Op::Drop { id: parent, exit: Exit::None, origin: Vector2::zero(), bounds: None });
                    rig.apply(&mut patch);
                }
                _ => rig.scene.cancel_exits(),
            }
            assert!(carrier.parent().is_none());
            assert!(!rig.scene.motion.pending.iter().any(|p| matches!(p.holds, PendingKind::Collapse { .. })));
            patch.clear();
            if rig.scene.nodes.live(parent) {
                patch.push(Op::Drop { id: parent, exit: Exit::None, origin: Vector2::zero(), bounds: None });
                rig.apply(&mut patch);
            }
            assert_eq!(rig.scene.census().visuals_live, 0);
        }
    }

    #[test]
    fn exit_capture_keeps_descendants_after_arena_retirement() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let root = rig.ids.mint();
        let child = rig.ids.mint();
        patch.push(Op::New { id: root, kind: NodeKind::Group, parent: Attach::Window, after: None });
        patch.push(Op::Bind { id: root, prop: Prop::Size, bind: Bind::Set(Value::Vec2(Vector2 { x: 100.0, y: 100.0 })) });
        patch.push(Op::New { id: child, kind: NodeKind::Sprite, parent: Attach::Node(root), after: None });
        rig.apply(&mut patch);
        let source = rig.scene.nodes.visual(root).unwrap().as_container().unwrap();
        assert_eq!(source.children().count(), 1);
        patch.push(Op::Drop {
            id: root,
            exit: Exit::Slide { by: Vector2 { x: 1.0, y: 0.0 }, ms: 200, easing: Easing::Linear },
            origin: Vector2::zero(), bounds: None,
        });
        rig.apply(&mut patch);
        assert!(!rig.scene.nodes.live(root));
        assert!(!rig.scene.nodes.live(child));
        assert_eq!(source.children().count(), 1, "the live capture source was dismantled");
        assert!(rig.scene.motion.pending.iter().any(|p| matches!(p.holds, PendingKind::Ghost(_))));
        assert!(rig.scene.audit().agrees());
    }

    #[test]
    fn destroying_a_node_takes_its_whole_subtree_and_reclaims_every_row() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let root = rig.ids.mint();
        patch.push(Op::New {
            id: root,
            kind: NodeKind::Group,
            parent: Attach::Window,
            after: None,
        });
        let mut leaves = Vec::new();
        for _ in 0..3 {
            let leaf = rig.ids.mint();
            patch.push(Op::New {
                id: leaf,
                kind: NodeKind::Sprite,
                parent: Attach::Node(root),
                after: None,
            });
            leaves.push(leaf);
        }
        // One grandchild, so the cascade has a level to descend.
        let deep = rig.ids.mint();
        patch.push(Op::New {
            id: deep,
            kind: NodeKind::Sprite,
            parent: Attach::Node(leaves[0]),
            after: None,
        });
        rig.apply(&mut patch);
        assert_eq!(rig.scene.audit().held, 5);
        assert!(rig.scene.audit().agrees());
        assert_eq!(rig.scene.census().visuals_live, 5);

        patch.push(Op::Drop {
            id: root,
            exit: Exit::None,
            origin: Vector2::zero(),
            bounds: None,
        });
        rig.apply(&mut patch);
        let audit = rig.scene.audit();
        assert_eq!(audit.held, 0, "the cascade left a row behind");
        assert!(audit.agrees());
        assert_eq!(rig.scene.census().visuals_live, 0);
    }

    #[test]
    fn a_patch_solved_under_another_display_is_counted_and_still_applied() {
        let Some(mut rig) = rig() else { return };
        let mut patch = SinkPatch::default();
        let id = rig.sprite(&mut patch, 40.0);
        // Solved at 150% and applied at 100%: a display change landing between the flush and
        // the pass. The rasters must match the display that is there, so the divergence is
        // counted rather than refused.
        patch.env = Some(Env::new(
            144.0,
            OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
        ));
        let before = rig.scene.census().env_mismatches;
        rig.apply(&mut patch);
        assert_eq!(rig.scene.census().env_mismatches, before + 1);
        assert_eq!(rig.scene.audit().held, 1, "the ops applied anyway");

        // A patch stating the environment it is applied under is not a mismatch.
        patch.env = Some(rig.env);
        patch.push(Op::Bind {
            id,
            prop: Prop::Opacity,
            bind: Bind::Set(Value::Scalar(0.5)),
        });
        rig.apply(&mut patch);
        assert_eq!(rig.scene.census().env_mismatches, before + 1);
    }

    #[test]
    fn a_pass_that_only_skipped_writes_is_not_a_change() {
        let before = Census::default();
        let after = Census {
            props_skipped: 40,
            ..before
        };
        assert!(
            !after.changed_since(&before),
            "an absorbed write is not a change"
        );
    }

    #[test]
    fn a_pass_that_wrote_anything_is_a_change() {
        let before = Census::default();
        for after in [
            Census {
                props_written: 1,
                ..before
            },
            Census {
                ops_applied: 1,
                ..before
            },
            Census {
                animations: 1,
                ..before
            },
            Census {
                visuals_minted: 1,
                ..before
            },
        ] {
            assert!(after.changed_since(&before));
        }
    }

    #[test]
    fn an_environment_mismatch_is_counted_and_not_refused() {
        // The count is what separates the race — a display change landing between a flush
        // and its apply — from the two halves deriving the environment independently.
        let mut census = Census::default();
        let before = census;
        census.env_mismatches += 1;
        assert!(
            !census.changed_since(&before),
            "a mismatch alone is not work the pass did"
        );
    }

    #[test]
    fn a_walk_that_reaches_what_the_arena_holds_agrees() {
        assert!(
            Audit {
                reached: 4,
                held: 4
            }
            .agrees()
        );
        assert!(
            !Audit {
                reached: 3,
                held: 4
            }
            .agrees()
        );
    }

    #[test]
    fn the_two_tunings_are_independent_values() {
        // Both sides are constants, so the ordering is proved at compile time: a chrome
        // period derived from the scroll tuning comes out several times too long.
        const _: () = assert!(CHROME_PERIOD < SCROLL_PERIOD);
        assert_ne!(CHROME_DAMPING, SCROLL_DAMPING);
    }

    #[test]
    fn chrome_period_scales_with_travel_and_is_bounded_at_both_ends() {
        let short = period_for(Tuning::Chrome, 1.0);
        let reference = period_for(Tuning::Chrome, CHROME_REF_TRAVEL);
        let long = period_for(Tuning::Chrome, 10_000.0);
        assert!(short < reference && reference < long);
        assert!(short >= CHROME_PERIOD * 0.7);
        assert!(long <= CHROME_PERIOD * 1.4);
    }

    #[test]
    fn the_scroll_carrier_does_not_scale_with_distance() {
        assert_eq!(
            period_for(Tuning::Scroll, 1.0),
            period_for(Tuning::Scroll, 10_000.0)
        );
    }

    #[test]
    fn a_non_finite_travel_does_not_produce_a_non_finite_period() {
        for travel in [f32::NAN, f32::INFINITY, -0.0] {
            let period = period_for(Tuning::Chrome, travel);
            assert!(period.is_finite());
            assert!(period >= CHROME_PERIOD * 0.7);
            assert!(period <= CHROME_PERIOD * 1.4);
        }
    }

    #[test]
    fn there_are_exactly_three_tracker_expressions_and_each_maps_affinely() {
        for expression in TRACK_EXPR {
            assert!(expression.contains("* m + c"), "{expression}");
            assert!(expression.starts_with("t."), "{expression}");
        }
        for expression in FOLLOW_EXPR {
            assert!(expression.starts_with("Clamp("), "{expression}");
            assert!(expression.ends_with(", lo, hi)"), "{expression}");
        }
    }

    #[test]
    fn every_property_group_names_a_spring_of_the_type_its_setter_takes() {
        for row in &PROPS {
            let slot = row.spring_slot();
            assert!((1..=3).contains(&slot), "{}", row.path);
            // A composite row is driven as a pair or a triple, never as a scalar; a
            // per-channel row is a scalar whatever composite shares its group.
            if row.count == 2 {
                assert!(slot > 1, "{} is a composite driven by a scalar", row.path);
            } else {
                assert_eq!(slot, 1, "{} is one channel driven by a vector", row.path);
            }
        }
        // The compositor's size is a pair and its offset, scale and centre are triples,
        // while every per-channel name under them is a scalar.
        assert_eq!(desc(Prop::Size).spring_slot(), 2);
        assert_eq!(desc(Prop::SizeX).spring_slot(), 1);
        assert_eq!(desc(Prop::Offset).spring_slot(), 3);
        assert_eq!(desc(Prop::OffsetX).spring_slot(), 1);
        assert_eq!(desc(Prop::Scale).spring_slot(), 3);
        assert_eq!(desc(Prop::ScaleY).spring_slot(), 1);
        assert_eq!(desc(Prop::Center).spring_slot(), 3);
        assert_eq!(desc(Prop::CenterX).spring_slot(), 1);
    }
}
