//! Compositor-evaluated property animations for the DComp backend.
//!
//! Everything here runs on the system compositor (DWM): the app builds an
//! animation object, starts it (or attaches it as an implicit trigger), and
//! goes back to its blocking message pump — there is **no app-side tick**, no
//! repaint, and no timer while an animation plays. This is the mechanism behind
//! three [`Backend`](crate::backend::Backend) entry points:
//!
//! - [`start`] — one-shot opacity/scale keyframe animations
//!   (`run_property_animation`, enter transitions, exit-ghost fades).
//! - [`build_implicit`] — an `ImplicitAnimationCollection` merging the
//!   element's declared transitions (`with_opacity_transition`, …) with its
//!   layout animation (`with_layout_animation`). The compositor plays these
//!   automatically whenever the targeted property is *set* to a new value, so
//!   the existing prop/layout writers stay the only writers — a property
//!   change simply becomes a glide instead of a jump.
//!
//! Coordination rules the rest of the backend upholds:
//! - Offset/size pushes are change-gated ([`Node::push_offset`]) so an
//!   unchanged layout pass never re-triggers an implicit animation.
//! - The collection is attached only after a node's **first** layout write
//!   ([`Node::attach_implicit_if_ready`]); attaching earlier would play the
//!   initial placement as a fly-in from the visual's zeroed defaults.
//! - Scale animations pivot around the node centre: any config touching scale
//!   flags the node (`wants_center`) and layout keeps `CenterPoint` at
//!   `size/2` from then on.
//!
//! ## Reduced motion
//!
//! Every entry point above is gated on [`reduced_motion`]. The rule the gate
//! upholds is that **reduced motion changes the path, never the destination**:
//! an animation that would have played is replaced by a direct write of its end
//! state, so the pixels land exactly where they would have — just immediately.
//! Returning early instead would leave an enter transition's opacity at its
//! `from` value and make the element permanently invisible, which is why
//! [`settle`] exists rather than a bare `return`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::node::Node;
use crate::style::{AnimationConfig, Easing, ImplicitTransitions, LayoutAnimationConfig};
use crate::system_bindings::{
    CompositionAnimation, CompositionBrush, CompositionEasingFunction, Compositor,
    ICompositionAnimation2, ICompositionAnimationBase, ICompositionObject, ICompositionSurface,
    ICompositor2, ICompositor4, ICompositorWithVisualSurface, IKeyFrameAnimation, IVisual,
    ImplicitAnimationCollection, SpriteVisual, TimeSpan, Visual,
};
use windows_core::{Interface, Result};
use windows_numerics::{Vector2, Vector3};

/// Debug-only backend diagnostic (mirrors the WinUI backend's `diag::warn`).
pub(crate) fn warn(args: std::fmt::Arguments<'_>) {
    if cfg!(debug_assertions) {
        eprintln!("windows-reactor: {args}");
    }
}

/// The user's system animation preference, cached. Refreshed by
/// [`refresh_reduced_motion`] at startup and on every `WM_SETTINGCHANGE`.
///
/// Process-global rather than per-backend: it is a user-level preference, the
/// same for every window, and the animation helpers here are free functions
/// that have no backend to reach through.
static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);

/// Whether the user has asked the system to minimise animation — Settings →
/// Accessibility → Visual effects → **Animation effects**, which is what
/// `SPI_GETCLIENTAREAANIMATION` reports.
pub(crate) fn reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Relaxed)
}

/// Re-read the system animation preference into the cache.
///
/// Returns `true` when the value **changed**, which is the caller's signal that
/// already-built implicit-animation collections are now stale and need
/// rebuilding (see `DCompBackend::refresh_motion`). A plain re-read that finds
/// the same value must not trigger that walk — `WM_SETTINGCHANGE` broadcasts
/// for every unrelated setting in the system.
pub(crate) fn refresh_reduced_motion() -> bool {
    let now = read_reduced_motion();
    REDUCED_MOTION.swap(now, Ordering::Relaxed) != now
}

/// Force the cached preference — test seam only, so a test does not depend on
/// the developer machine's accessibility settings.
pub(crate) fn set_reduced_motion_for_test(reduced: bool) {
    REDUCED_MOTION.store(reduced, Ordering::Relaxed);
}

/// Ask the OS whether client-area animation is enabled.
///
/// Uses the Win32 read rather than WinRT `UISettings.AnimationsEnabled`, which
/// is documented to surface this same setting: the value is identical, the
/// change signal (`WM_SETTINGCHANGE`) is already handled on the pump, and the
/// Win32 read is synchronous on the thread that needs it. `UISettings` would
/// add a WinRT activation and deliver its change event on a thread-pool thread,
/// requiring a marshal back for a value we can simply read here.
///
/// **Fails open** (animations enabled). A transient failure to read a
/// preference should not silently disable motion across the whole app; the
/// setting defaults to enabled, and this call does not realistically fail.
fn read_reduced_motion() -> bool {
    let mut enabled = windows_core::BOOL(1);
    let ok = unsafe {
        crate::system_bindings::SystemParametersInfoW(
            crate::system_bindings::SPI_GETCLIENTAREAANIMATION,
            0,
            (&raw mut enabled).cast(),
            0,
        )
    };
    ok.as_bool() && !enabled.as_bool()
}

/// Apply a one-shot config's **end state** with no animation.
///
/// Any in-flight animation on the property is stopped first: a compositor
/// property that has a running animation ignores a plain set, so without the
/// stop a preference flip mid-animation would leave the old animation owning
/// the property.
fn settle(target: &Visual, cfg: &AnimationConfig) -> Result<()> {
    let obj: ICompositionObject = target.cast()?;
    let vis: IVisual = target.cast()?;
    if let Some(opacity) = cfg.opacity {
        obj.StopAnimation("Opacity")?;
        vis.SetOpacity(opacity as f32)?;
    }
    if let Some(scale) = cfg.scale {
        obj.StopAnimation("Scale")?;
        let s = scale as f32;
        vis.SetScale(Vector3::new(s, s, 1.0))?;
    }
    Ok(())
}

/// WinRT `TimeSpan` (100 ns units) from a std `Duration`.
fn ts(d: Duration) -> TimeSpan {
    TimeSpan {
        duration: (d.as_nanos() / 100).min(i64::MAX as u128) as i64,
    }
}

/// Easing curves matching the CSS-standard ease-{out,in,in-out} beziers
/// (identical to the WinUI backend's mapping).
fn easing_for(comp: &Compositor, easing: Easing) -> Result<CompositionEasingFunction> {
    let (p1, p2) = match easing {
        Easing::Linear => {
            return comp
                .CreateLinearEasingFunction()?
                .cast::<CompositionEasingFunction>();
        }
        Easing::EaseOut => (Vector2::new(0.0, 0.0), Vector2::new(0.58, 1.0)),
        Easing::EaseIn => (Vector2::new(0.42, 0.0), Vector2::new(1.0, 1.0)),
        Easing::EaseInOut => (Vector2::new(0.42, 0.0), Vector2::new(0.58, 1.0)),
    };
    comp.CreateCubicBezierEasingFunction(p1, p2)?
        .cast::<CompositionEasingFunction>()
}

/// Run a one-shot [`AnimationConfig`] on any visual (a node container, or an
/// exit-ghost snapshot sprite). `center` supplies the scale pivot in DIPs (the
/// laid-out half-size) when known; a scale animation without one keeps the
/// visual's current `CenterPoint`.
///
/// The animated value HOLDS after completion (WinComp keyframe semantics), so
/// a fade to 1.0 leaves the visual at the same opacity a plain prop set would.
pub(crate) fn start(
    comp: &Compositor,
    target: &Visual,
    cfg: &AnimationConfig,
    center: Option<(f32, f32)>,
) {
    if let Err(e) = start_inner(comp, target, cfg, center) {
        warn(format_args!("dcomp property animation failed: {e:?}"));
    }
}

fn start_inner(
    comp: &Compositor,
    target: &Visual,
    cfg: &AnimationConfig,
    center: Option<(f32, f32)>,
) -> Result<()> {
    if reduced_motion() {
        return settle(target, cfg);
    }

    let obj: ICompositionObject = target.cast()?;
    let easing = easing_for(comp, cfg.easing)?;

    if let Some(opacity) = cfg.opacity {
        let a = comp.CreateScalarKeyFrameAnimation()?;
        a.cast::<IKeyFrameAnimation>()?.SetDuration(ts(cfg.duration))?;
        // Easing on a progress-0 keyframe is inert (easing shapes the segment
        // *ending* at a keyframe); reuse the end easing rather than allocating
        // a linear function for it.
        if let Some(from) = cfg.from_opacity {
            a.InsertKeyFrameWithEasingFunction(0.0, from as f32, &easing)?;
        }
        a.InsertKeyFrameWithEasingFunction(1.0, opacity as f32, &easing)?;
        obj.StartAnimation("Opacity", &a.cast::<CompositionAnimation>()?)?;
    }

    if let Some(scale) = cfg.scale {
        if let Some((cx, cy)) = center
            && let Ok(v) = target.cast::<IVisual>()
        {
            let _ = v.SetCenterPoint(Vector3::new(cx, cy, 0.0));
        }
        let a = comp.CreateVector3KeyFrameAnimation()?;
        a.cast::<IKeyFrameAnimation>()?.SetDuration(ts(cfg.duration))?;
        if let Some(from) = cfg.from_scale {
            let f = from as f32;
            a.InsertKeyFrameWithEasingFunction(0.0, Vector3::new(f, f, 1.0), &easing)?;
        }
        let s = scale as f32;
        a.InsertKeyFrameWithEasingFunction(1.0, Vector3::new(s, s, 1.0), &easing)?;
        obj.StartAnimation("Scale", &a.cast::<CompositionAnimation>()?)?;
    }

    Ok(())
}

/// Build the merged implicit-animation collection for a node from its declared
/// transitions and layout animation. Returns `None` when neither contributes
/// anything (the caller then clears the visual's collection).
///
/// On an Offset conflict (a translation transition AND a layout animation) the
/// layout animation wins — it is the more specific request.
pub(crate) fn build_implicit(
    comp: &Compositor,
    transitions: Option<&ImplicitTransitions>,
    layout: Option<&LayoutAnimationConfig>,
) -> Result<Option<ImplicitAnimationCollection>> {
    // Under reduced motion there is nothing to attach: the prop and layout
    // writers are the only writers, so with no collection their sets simply
    // take effect immediately. The end state is unchanged — only the glide is.
    if reduced_motion() {
        return Ok(None);
    }

    let has_transitions = transitions.is_some_and(|t| !t.is_empty());
    let layout_offset = layout.is_some_and(|l| l.animate_offset);
    let layout_size = layout.is_some_and(|l| l.animate_size);
    if !has_transitions && !layout_offset && !layout_size {
        return Ok(None);
    }

    let collection = comp
        .cast::<ICompositor2>()?
        .CreateImplicitAnimationCollection()?;
    let map = collection
        .cast::<windows_collections::IMap<windows_core::HSTRING, ICompositionAnimationBase>>()?;

    // `this.StartingValue -> this.FinalValue` over `duration`, EaseOut — the
    // standard implicit-transition curve. The compositor retargets a running
    // instance smoothly when the property changes again mid-flight.
    let insert_keyframe = |target: &str, duration: Duration, dims: Dims| -> Result<()> {
        let easing = easing_for(comp, Easing::EaseOut)?;
        let anim: ICompositionAnimationBase = match dims {
            Dims::Scalar => {
                let a = comp.CreateScalarKeyFrameAnimation()?;
                let kf: IKeyFrameAnimation = a.cast()?;
                kf.SetDuration(ts(duration))?;
                kf.InsertExpressionKeyFrameWithEasingFunction(1.0, "this.FinalValue", &easing)?;
                a.cast::<ICompositionAnimation2>()?.SetTarget(target)?;
                a.cast()?
            }
            Dims::Vector2 => {
                let a = comp.CreateVector2KeyFrameAnimation()?;
                let kf: IKeyFrameAnimation = a.cast()?;
                kf.SetDuration(ts(duration))?;
                kf.InsertExpressionKeyFrameWithEasingFunction(1.0, "this.FinalValue", &easing)?;
                a.cast::<ICompositionAnimation2>()?.SetTarget(target)?;
                a.cast()?
            }
            Dims::Vector3 => {
                let a = comp.CreateVector3KeyFrameAnimation()?;
                let kf: IKeyFrameAnimation = a.cast()?;
                kf.SetDuration(ts(duration))?;
                kf.InsertExpressionKeyFrameWithEasingFunction(1.0, "this.FinalValue", &easing)?;
                a.cast::<ICompositionAnimation2>()?.SetTarget(target)?;
                a.cast()?
            }
        };
        map.Insert(&windows_core::HSTRING::from(target), &anim)?;
        Ok(())
    };

    if let Some(t) = transitions.filter(|t| !t.is_empty()) {
        if let Some(s) = t.opacity {
            insert_keyframe("Opacity", s.duration, Dims::Scalar)?;
        }
        if let Some(s) = t.rotation {
            insert_keyframe("RotationAngle", s.duration, Dims::Scalar)?;
        }
        if let Some(v) = t.scale {
            insert_keyframe("Scale", v.duration, Dims::Vector3)?;
        }
        // On this backend there is no separate Translation channel: Offset is
        // the layout-owned position, so a translation transition IS a layout
        // glide (per-axis masking is not supported — all axes animate).
        // Skipped when a layout animation targets Offset below.
        if let Some(v) = t.translation
            && !layout_offset
        {
            insert_keyframe("Offset", v.duration, Dims::Vector3)?;
        }
    }

    if let Some(l) = layout {
        // A spring glide is NOT registered implicitly: a NaturalMotionAnimation
        // in an implicit collection does not get its FinalValue populated from
        // the property set on the system compositor — it settles at the
        // starting value and PINS the property there (observed live). Instead
        // the offset writer starts an explicit spring with the known
        // destination (see [`spring_offset`]); only the duration-based glide
        // uses the implicit `this.FinalValue` keyframe mechanism here.
        if l.animate_offset && !l.use_spring {
            insert_keyframe("Offset", l.duration, Dims::Vector3)?;
        }
        if l.animate_size {
            // Size is a Vector2 property; animating it live-resizes the clip
            // and any surface stretch, so it defaults off in the config.
            insert_keyframe("Size", l.duration, Dims::Vector2)?;
        }
    }

    Ok(Some(collection))
}

/// Glide a visual's Opacity from its current (possibly mid-flight) value to
/// `to`, entirely on the system compositor. Restarting mid-fade retargets
/// smoothly: the new animation picks up from the current animated value, so a
/// reveal interrupted by a conceal (or vice versa) reverses without a jump.
pub(crate) fn fade_opacity(
    comp: &Compositor,
    target: &Visual,
    to: f32,
    duration: Duration,
    easing: Easing,
) {
    if reduced_motion() {
        let set = || -> Result<()> {
            target.cast::<ICompositionObject>()?.StopAnimation("Opacity")?;
            target.cast::<IVisual>()?.SetOpacity(to)
        };
        if let Err(e) = set() {
            warn(format_args!("dcomp opacity settle failed: {e:?}"));
        }
        return;
    }

    let run = || -> Result<()> {
        let a = comp.CreateScalarKeyFrameAnimation()?;
        a.cast::<IKeyFrameAnimation>()?.SetDuration(ts(duration))?;
        a.InsertKeyFrameWithEasingFunction(1.0, to, &easing_for(comp, easing)?)?;
        target
            .cast::<ICompositionObject>()?
            .StartAnimation("Opacity", &a.cast::<CompositionAnimation>()?)
    };
    if let Err(e) = run() {
        warn(format_args!("dcomp opacity fade failed: {e:?}"));
    }
}

/// Overlay-scrollbar thumb auto-hide fade: a quick reveal, a gentler conceal —
/// both played on the system compositor, so the auto-hide costs zero app
/// frames (the tick loop only edge-triggers it).
pub(crate) fn fade_thumb(comp: &Compositor, surf: &super::bootstrap::NodeSurface, shown: bool) {
    let (to, dur) = if shown {
        (1.0, Duration::from_millis(100))
    } else {
        (0.0, Duration::from_millis(300))
    };
    if let Ok(v) = surf.sprite.cast::<Visual>() {
        fade_opacity(comp, &v, to, dur, Easing::EaseOut);
    }
}

/// Drive a visual's Offset to a known destination with a TRUE compositor
/// spring (`SpringVector3NaturalMotionAnimation`), started explicitly with the
/// destination as `FinalValue`.
///
/// This is how `LayoutAnimationConfig::spring()` glides run. The implicit
/// route is closed to natural motion (see [`build_implicit`]) — but layout is
/// the only Offset writer and KNOWS the destination, so it hands the spring an
/// explicit target instead of setting the property. The spring starts from the
/// property's current (possibly mid-flight) value, so successive layout passes
/// retarget it continuously; physics run entirely on the compositor.
///
/// The spring object is built ONCE into `cache` (damping/period are fixed per
/// config — the owner clears the cache when the config changes); a retarget is
/// just `SetFinalValue` + `StartAnimation` on the cached object, so continuous
/// relayout (interactive resize) allocates nothing per pass.
pub(crate) fn spring_offset(
    target: &ICompositionObject,
    cache: &mut Option<crate::system_bindings::SpringVector3NaturalMotionAnimation>,
    x: f32,
    y: f32,
    damping_ratio: f32,
    period: f32,
) -> Result<()> {
    // The spring's whole purpose is the overshoot; under reduced motion the
    // destination is written straight to Offset. Layout knows that destination
    // exactly, so this is lossless — the element simply arrives without travel.
    if reduced_motion() {
        target.StopAnimation("Offset")?;
        return target
            .cast::<IVisual>()?
            .SetOffset(Vector3::new(x, y, 0.0));
    }

    if cache.is_none() {
        let comp = target.Compositor()?;
        let a = comp.cast::<ICompositor4>()?.CreateSpringVector3Animation()?;
        a.SetDampingRatio(damping_ratio)?;
        a.SetPeriod(ts(Duration::from_secs_f32(period.max(0.001))))?;
        *cache = Some(a);
    }
    let a = cache.as_ref().expect("cache filled above");
    a.cast::<crate::system_bindings::IVector3NaturalMotionAnimation>()?
        .SetFinalValue(Some(Vector3::new(x, y, 0.0)))?;
    target.StartAnimation("Offset", &a.cast::<CompositionAnimation>()?)
}

enum Dims {
    Scalar,
    Vector2,
    Vector3,
}

/// Snapshot a visual subtree into a single-layer sprite.
///
/// A `CompositionVisualSurface` re-composites `source` — which may be detached
/// from the visual tree; the surface holds its own reference and renders the
/// subtree independently — into ONE surface. Animating the returned sprite's
/// opacity is therefore a FLATTENED group fade: overlapping translucent
/// children cannot bleed through mid-fade the way per-visual container opacity
/// lets them. The sprite keeps the whole chain alive
/// (sprite → brush → visual surface → source subtree), so the caller only
/// needs to hold the sprite.
///
/// Note the mirror is live, not a frozen bitmap — if the source subtree still
/// repaints, the sprite follows. For exit ghosts the source is a destroyed
/// subtree that nothing updates, so the content is effectively frozen.
pub(crate) fn snapshot_sprite(
    comp: &Compositor,
    source: &Visual,
    w: f32,
    h: f32,
) -> Result<SpriteVisual> {
    let vs = comp
        .cast::<ICompositorWithVisualSurface>()?
        .CreateVisualSurface()?;
    vs.SetSourceVisual(source)?;
    vs.SetSourceOffset(Vector2::new(0.0, 0.0))?;
    vs.SetSourceSize(Vector2::new(w, h))?;
    let brush = comp.CreateSurfaceBrushWithSurface(&vs.cast::<ICompositionSurface>()?)?;
    let sprite = comp.CreateSpriteVisual()?;
    sprite.SetBrush(&brush.cast::<CompositionBrush>()?)?;
    sprite.cast::<IVisual>()?.SetSize(Vector2::new(w, h))?;
    Ok(sprite)
}

/// A one-shot config wants a centre pivot maintained (it animates scale).
pub(crate) fn wants_center(cfg: &AnimationConfig) -> bool {
    cfg.scale.is_some() || cfg.from_scale.is_some()
}

/// Note an animation intent on the node: flag the centre pivot when scale is
/// involved and pre-set it if the node is already laid out (a later layout
/// pass keeps it current via `push_size`).
pub(crate) fn note_scale_intent(node: &mut Node) {
    node.wants_center = true;
    if node.rect.w > 0.0 && node.rect.h > 0.0 {
        let _ = node
            .vis
            .SetCenterPoint(Vector3::new(node.rect.w / 2.0, node.rect.h / 2.0, 0.0));
    }
}
