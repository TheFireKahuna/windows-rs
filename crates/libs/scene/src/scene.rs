//! The scene: the retained tree, the patch applier, motion, trackers and the ground.
//!
//! `Windows.UI.Composition` has no commit method. Changes publish when the thread's
//! dispatcher queue finishes the current work item, so one pass is one publish and an idle
//! window publishes nothing because it never passes. Composition objects are therefore
//! touched only inside a pass.

use crate::arena::*;
use crate::hit::HitTable;
use crate::hit_entry::{ContactKind, Hit, pack_offset};
use crate::patch::{Attach, Op, SinkPatch};
use crate::realize::{Backends, BoxKey, Cache, Ctx, ResObj, Resources, fit, realize};
use crate::sink::*;
use core::cell::{Cell, RefCell};
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use std::rc::Rc;
use std::sync::Arc;
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

/// Restricted to trim endpoints, so a slider fill follows the thumb's exact compositor
/// position across zero without a second spring and cannot create an offset cycle.
const FOLLOW_EXPR: [&str; 2] = [
    "Clamp(v.Offset.X * m + c, lo, hi)",
    "Clamp(v.Offset.Y * m + c, lo, hi)",
];

/// Held once for the process.
///
/// Continuity across a retarget is a property of the *target*, because a natural-motion
/// animation starts from the property's current value and resets its velocity whichever
/// object drives it — which is also why a spring retargeted per pointer-move never leaves
/// rest. Six springs and five expressions serve the whole stack.
struct Templates {
    scalar: [SpringScalarNaturalMotionAnimation; 2],
    vec2: [SpringVector2NaturalMotionAnimation; 2],
    vec3: [SpringVector3NaturalMotionAnimation; 2],
    track: [ExpressionAnimation; 3],
    follow: [ExpressionAnimation; 2],
    linear: CompositionEasingFunction,
}

impl Templates {
    fn new(back: &Backends) -> Self {
        let comp = &back.compositor;
        // The damping ratio is the tuning's own and never varies with travel, so it is set
        // once here and the period is what a retarget restates.
        Self {
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
        let at = tuning as usize;
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
        clamp: [f32; 2],
    ) -> CompositionAnimation {
        let expression = &self.follow[usize::from(vertical)];
        expression.set_reference_parameter("v", source);
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
/// The chrome period is scaled by how far the value travels, so a long pill slide is not
/// instantaneous and a short one is not sluggish; that is why a caller states a tuning and
/// never a period. The scroll carrier is not scaled: it carries momentum, which does not
/// depend on how far the content is from a bound.
fn period_for(tuning: Tuning, travel: f32) -> f32 {
    let base = SPRING[tuning as usize][0];
    match tuning {
        Tuning::Scroll => base,
        Tuning::Chrome if !travel.is_finite() => base,
        Tuning::Chrome => base * (travel.abs() / CHROME_REF_TRAVEL).clamp(0.7, 1.4),
    }
}

/// What one in-flight batch is holding alive.
enum PendingKind {
    /// The flattened capture, on screen for as long as the exit plays. Held because nothing
    /// else does: a ghost is unparented from the model's tree by construction.
    Ghost(Visual),
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
}

impl Backdrop {
    fn build(&mut self, back: &Backends, env: Env) -> Result<()> {
        self.sprites.clear();
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
            let Some(surface) = back.raster_ramp(stops, spread, env)? else {
                continue;
            };
            let sprite = back.compositor.create_sprite_visual();
            sprite.set_brush(&back.brush(&surface, Stretch::Fill));
            place(&sprite, at, size);
            self.sprites.push(sprite);
        }
        Ok(())
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
        };
        backdrop.build(back, env)?;
        let layers = ground.children();
        for sprite in &backdrop.sprites {
            layers.insert_at_top(sprite);
        }
        Ok(Self {
            target,
            root,
            ground,
            content,
            overlay,
            backdrop,
            nodes: Arena::default(),
            roots: Vec::new(),
            res: Resources::default(),
            cache: Cache::default(),
            generation: Gen::default(),
            env: Some(env),
            motion: Motion {
                templates: Templates::new(back),
                pending: Vec::new(),
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
        patch.clear();
        Ok(self.census.changed_since(&before))
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
        if was.geometry_moved(env) {
            self.generation.dpi = self.generation.dpi.wrapping_add(1);
            set_dip_space(&self.root, env.scale());
            self.rescale_regions(env);
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
        }
        self.events.borrow_mut().push(SceneEvent::DeviceRebuilt);
        Ok(())
    }

    fn reseat_backdrop(&mut self) {
        let layers = self.ground.children();
        layers.remove_all();
        for sprite in &self.backdrop.sprites {
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
        };
        realize(&mut self.nodes, id, glow.as_ref(), &mut ctx)
    }

    fn op(&mut self, op: Op, patch: &SinkPatch, back: &Backends, env: Env) -> Result<()> {
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
        let collection = match parent {
            Attach::Window => self.content.children(),
            Attach::Overlay => self.overlay.children(),
            Attach::Node(at) => match self.nodes.visual(at).and_then(Visual::as_container) {
                Some(group) => group.children(),
                None => return,
            },
        };
        // Fallible: a caller can hold a node whose parent was torn down between two
        // operations, and already-removed is the wanted state.
        let _ = self.content.children().try_remove(&visual);
        let _ = self.overlay.children().try_remove(&visual);
        let _ = collection.try_remove(&visual);
        match after.and_then(|sibling| self.nodes.visual(sibling).cloned()) {
            Some(sibling) => collection.insert_above(&visual, &sibling),
            None if matches!(parent, Attach::Overlay) => collection.insert_at_top(&visual),
            None => collection.insert_at_bottom(&visual),
        }
        if let Attach::Node(at) = parent {
            self.roots.retain(|root| *root != id);
            let after = after.map(|sibling| sibling.index() as u32);
            link(&mut self.nodes, id.index() as u32, at.index() as u32, after);
        } else {
            unlink(&mut self.nodes, id.index() as u32);
            if !self.roots.contains(&id) {
                self.roots.push(id);
            }
        }
    }

    /// Destroys a node *and its subtree*, releasing every resource on the way down, so a
    /// subtree removal is one op and a partial destroy is unrepresentable.
    ///
    /// Recurses over the child chain; the depth it reaches is layout nesting.
    fn destroy(&mut self, id: NodeId) {
        for child in children(&self.nodes, id.index() as u32).collect::<Vec<_>>() {
            self.destroy(child);
        }
        let Some(visual) = self.nodes.visual(id).cloned() else {
            return;
        };
        let parent = self.nodes.links(id.index() as u32).parent;
        let held = (parent != NO_LINK)
            .then(|| self.nodes.id_at(parent))
            .and_then(|at| self.nodes.visual(at))
            .and_then(Visual::as_container);
        if let Some(group) = held {
            let _ = group.children().try_remove(&visual);
        } else {
            // A root sits in a band rather than under a node, and which band is not recorded.
            let _ = self.content.children().try_remove(&visual);
            let _ = self.overlay.children().try_remove(&visual);
        }
        unlink(&mut self.nodes, id.index() as u32);
        self.roots.retain(|root| *root != id);
        self.pending_retain(
            |pending| !matches!(pending.holds, PendingKind::Frames(node, _) if node == id),
        );
        let (_, painted) = self.nodes.free(id);
        if let Some(painted) = painted {
            self.res.release(painted.mask.holds());
            self.res.release(painted.mask.holds_dash());
            self.res.release(painted.paint.holds());
        }
        self.census.visuals_live = self.census.visuals_live.saturating_sub(1);
    }

    /// Detaches the subtree and keeps it on screen for the length of the exit.
    ///
    /// It is flattened into one capture mounted as a single top-level sprite, so the original
    /// visuals unparent at once and a dying panel of sixty visuals fades as one; the
    /// capture's brush chain keeps the detached source alive while it plays.
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
        if matches!(exit, Exit::None) {
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
            Exit::None => unreachable!("returned above"),
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

    /// How far past its own box the subtree's halos reach, in DIPs.
    ///
    /// Three sigmas covers a Gaussian's visible tail, and an offset shadow moves that tail
    /// with it.
    fn halo_margin(&self, id: NodeId) -> f32 {
        let blur = PROPS[Prop::BlurRadius as usize].chan;
        let own = match self.nodes.aux(id).and_then(|aux| aux.glow.as_ref()) {
            Some(_) => {
                let offset = self
                    .nodes
                    .painted(id)
                    .and_then(|painted| painted.halo)
                    .map_or(0.0, |halo| halo.offset.x.abs().max(halo.offset.y.abs()));
                self.nodes.chan(id, blur).max(0.0) * 3.0 + offset
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
        // A rectangle clip already in the slot takes the new sides through the channels, so
        // a resize rebuilds nothing.
        if was_rect && matches!(clip, Clip::Rect { .. }) {
            self.nodes.aux_mut(id).decl = clip;
            return self.write_clip(id, clip);
        }
        let next = match clip {
            Clip::None => None,
            // Rounded clipping needs no brush slot and no capture: a rectangle clip carries
            // its own radii.
            Clip::Rect { .. } => Some(ClipObj::Rect(back.compositor.create_rectangle_clip())),
            Clip::Geom(geom) => self.res.geom(geom).map(|geometry| {
                // Soft border mode antialiases the clip edge, which is what makes a
                // geometric clip usable as a shape.
                visual.set_border_mode(windows_composition::BorderMode::Soft);
                ClipObj::Geom(back.compositor.create_geometric_clip(geometry))
            }),
        };
        match &next {
            Some(ClipObj::Rect(clip)) => visual.set_clip(Some(clip)),
            Some(ClipObj::Geom(clip)) => visual.set_clip(Some(clip)),
            // Clears only what the *sink* established: a clip-route shape mask writes its
            // geometric clip straight onto the visual without claiming this slot.
            None if was_rect || self.nodes.aux(id).is_some_and(|aux| aux.clip.is_some()) => {
                visual.clear_clip();
            }
            None => {}
        }
        let aux = self.nodes.aux_mut(id);
        aux.clip = next;
        aux.decl = clip;
        self.write_clip(id, clip)?;
        // The sink's clip and a clip-route shape mask compete for the visual's one slot, so
        // a slot changing hands costs a promotion rather than a wrong render.
        if self.nodes.painted(id).is_some_and(Painted::owns_the_clip) {
            self.realize(id, back, env)?;
        }
        Ok(())
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
                self.census.count(written);
                // Only a size change can invalidate a capture, so the property is tested
                // before the walk: a move, an opacity and a rotation land through the same
                // setter and none of them moves the region.
                if written && matches!(prop, Prop::Size | Prop::SizeX | Prop::SizeY) {
                    self.resize_captures(id, env);
                    self.reclamp_box_mask(id, back, env)?;
                }
            }
            Bind::Animate(anim) => self.animate(id, prop, row, anim, patch, back)?,
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
                clamp,
            } => {
                // Only a trim endpoint, and never its own source: any other target would let
                // one visual's offset drive another's and close an expression cycle.
                if !matches!(prop, Prop::TrimStart | Prop::TrimEnd)
                    || source == id
                    || !affine.m.is_finite()
                    || !affine.c.is_finite()
                    || !clamp[0].is_finite()
                    || !clamp[1].is_finite()
                    || clamp[0] > clamp[1]
                {
                    return Err(invalid_arg());
                }
                let Some(from) = self.nodes.visual(source).cloned() else {
                    return Ok(());
                };
                let animation = self.motion.templates.follow(vertical, &from, affine, clamp);
                self.nodes.start(id, row, &animation, None, Held::Bound);
                self.census.animations += 1;
            }
            Bind::Stop => self.nodes.stop(id, row),
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
                // Travel is measured from the shadow, which is the channel's current value
                // whichever mechanism carries it, so the chrome period scales scene-side and
                // no call site holds one. Starting also resets the spring's velocity.
                let travel = travel(&self.nodes, id, row, to);
                let animation = self.motion.templates.spring(
                    row.spring_slot(),
                    tuning,
                    to,
                    travel,
                    Duration::from_millis(u64::from(delay_ms)),
                );
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
                self.nodes.start(id, row, &animation, None, Held::Playing);
                self.census.animations += 1;
                if !matches!(iterations, Iterations::Count(_)) {
                    return Ok(());
                }
                // A finite run keeps its animation and subscription until the batch reports,
                // which queues a completion and requests one tick; the node generation
                // rejects a completion for a slot that has since been reused.
                self.motion
                    .watch(back, PendingKind::Frames(id, prop), move || drop(animation))
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
        let Some(aux) = self.nodes.aux(id) else {
            return;
        };
        if let Some(shape) = &aux.shape {
            shape.resize(size, scale);
        }
        if let Some(capture) = aux.glow.as_ref().and_then(|glow| glow.capture.as_ref()) {
            capture.resize(size, scale);
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
                let Some(surface) = back.raster_ramp(patch.stops(stops), spread, env)? else {
                    return Ok(());
                };
                let brush = back.brush(&surface, Stretch::Fill);
                ResObj::Brush(brush, Some(surface))
            }
            // A tile's pixel extent is its ink at the current scale, so a sprite sized to
            // that same ink samples one texel per physical pixel and nothing resamples.
            // Fill is what makes that identity hold at every scale.
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
                let brush = back.brush(&surface, Stretch::Fill);
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
        scale_region(&brush, env);
        self.res
            .declare(region.erased(), ResObj::Brush(brush, None));
        self.rebind_holders(region.erased(), back, env)
    }

    /// Re-scales every bound region's brush after the pixel grid moved.
    ///
    /// A brush is not cache-backed and carries no generation, so nothing else re-derives it:
    /// the sprites rebind to the same object and would keep the factor of the display the
    /// region was bound on, drawing at half size in its own box with the rest of the tree
    /// correct.
    fn rescale_regions(&mut self, env: Env) {
        for row in self.res.rows_mut() {
            if let ResObj::Brush(brush, None) = row {
                scale_region(brush, env);
            }
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
        self.res.disclaim(region.erased());
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
        self.pending_retain(|pending| !matches!(pending.holds, PendingKind::Ghost(_)));
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
        self.motion.pending.retain(|pending| {
            if !pending.done.get() {
                return true;
            }
            match &pending.holds {
                PendingKind::Ghost(visual) => {
                    let _ = overlay.try_remove(visual);
                    census.visuals_live = census.visuals_live.saturating_sub(1);
                }
                PendingKind::Delay(id, _) => reports.push(SceneEvent::DelayElapsed(*id)),
                PendingKind::Frames(node, prop) => reports.push(SceneEvent::AnimationCompleted {
                    node: *node,
                    prop: *prop,
                }),
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
        self.bind(node, prop, bind, &empty, back, env)
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

/// Maps a region's buffer one texel to one physical pixel inside a visual measured in DIPs.
///
/// A sprite's box is in DIPs and the whole tree hangs under a root carrying the display
/// scale, so a brush left at unit scale paints one texel per *DIP* and the region comes out
/// magnified. The offset is zero: the region owns the whole surface.
fn scale_region(brush: &windows_composition::CompositionSurfaceBrush, env: Env) {
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
        let Ok(gpu) = windows_d2d::Gpu::for_window() else {
            eprintln!("skipped: no Direct2D device in this session");
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

    /// A halo casts at the blur it declared and at full opacity until a channel says
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
                blur: 12.0,
                tint: Radiance::new(0.2, 0.6, 0.9, 1.0),
                offset: Vector2::zero(),
            }),
        });
        rig.apply(&mut patch);
        let blur = PROPS[Prop::BlurRadius as usize].chan;
        assert_eq!(rig.scene.nodes.chan(id, blur), 12.0);
        assert_eq!(rig.scene.nodes.chan(id, blur + 1), 1.0);
        assert!(rig.scene.nodes.aux(id).is_some_and(|aux| aux.glow.is_some()));

        patch.push(Op::Bind {
            id,
            prop: Prop::ShadowOpacity,
            bind: Bind::Set(Value::Scalar(0.3)),
        });
        rig.apply(&mut patch);
        rig.scene.device_lost(&rig.back, rig.env).expect("rebuilt");
        assert_eq!(rig.scene.nodes.chan(id, blur + 1), 0.3);
        assert_eq!(rig.scene.nodes.chan(id, blur), 12.0);
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
        // A follow expression is clamped, which is what stops an offset cycle closing.
        for expression in FOLLOW_EXPR {
            assert!(expression.starts_with("Clamp(v.Offset."), "{expression}");
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
