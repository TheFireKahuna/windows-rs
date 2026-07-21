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

use std::time::Duration;

use crate::motion::reduced_motion;

use super::node::Node;
use crate::style::{AnimationConfig, Easing, ImplicitTransitions, LayoutAnimationConfig};
use windows_composition::{
    BorderMode, CompositionEasingFunction, Compositor, ImplicitAnimationCollection,
    SpringVector3NaturalMotionAnimation, SpriteVisual, Visual,
};
use windows_numerics::{Vector2, Vector3};

/// Debug-only backend diagnostic (mirrors the WinUI backend's `diag::warn`).
pub(crate) fn warn(args: std::fmt::Arguments<'_>) {
    if cfg!(debug_assertions) {
        eprintln!("windows-reactor: {args}");
    }
}

/// Apply a one-shot config's **end state** with no animation.
///
/// Any in-flight animation on the property is stopped first: a compositor
/// property that has a running animation ignores a plain set, so without the
/// stop a preference flip mid-animation would leave the old animation owning
/// the property.
fn settle(target: &Visual, cfg: &AnimationConfig) {
    if let Some(opacity) = cfg.opacity {
        target.stop_animation("Opacity");
        target.set_opacity(opacity as f32);
    }
    if let Some(scale) = cfg.scale {
        target.stop_animation("Scale");
        let s = scale as f32;
        target.set_scale(Vector3::new(s, s, 1.0));
    }
}

/// Easing curves matching the CSS-standard ease-{out,in,in-out} beziers
/// (identical to the WinUI backend's mapping).
fn easing_for(comp: &Compositor, easing: Easing) -> CompositionEasingFunction {
    let (p1, p2) = match easing {
        Easing::Linear => return comp.create_linear_easing_function(),
        Easing::EaseOut => (Vector2::new(0.0, 0.0), Vector2::new(0.58, 1.0)),
        Easing::EaseIn => (Vector2::new(0.42, 0.0), Vector2::new(1.0, 1.0)),
        Easing::EaseInOut => (Vector2::new(0.42, 0.0), Vector2::new(0.58, 1.0)),
    };
    comp.create_cubic_bezier_easing_function(p1, p2)
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
    if reduced_motion() {
        settle(target, cfg);
        return;
    }

    // Built unconditionally and shared by both animations below, exactly as
    // before: one easing object per call, whatever the config asks for.
    let easing = easing_for(comp, cfg.easing);

    if let Some(opacity) = cfg.opacity {
        let a = comp.create_scalar_key_frame_animation();
        a.set_duration(cfg.duration);
        // Easing on a progress-0 keyframe is inert (easing shapes the segment
        // *ending* at a keyframe); reuse the end easing rather than allocating
        // a linear function for it.
        if let Some(from) = cfg.from_opacity {
            a.insert_key_frame_with_easing(0.0, from as f32, &easing);
        }
        a.insert_key_frame_with_easing(1.0, opacity as f32, &easing);
        target.start_animation("Opacity", &a);
    }

    if let Some(scale) = cfg.scale {
        if let Some((cx, cy)) = center {
            target.set_center_point(Vector3::new(cx, cy, 0.0));
        }
        let a = comp.create_vector3_key_frame_animation();
        a.set_duration(cfg.duration);
        if let Some(from) = cfg.from_scale {
            let f = from as f32;
            a.insert_key_frame_with_easing(0.0, Vector3::new(f, f, 1.0), &easing);
        }
        let s = scale as f32;
        a.insert_key_frame_with_easing(1.0, Vector3::new(s, s, 1.0), &easing);
        target.start_animation("Scale", &a);
    }
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
) -> Option<ImplicitAnimationCollection> {
    // Under reduced motion there is nothing to attach: the prop and layout
    // writers are the only writers, so with no collection their sets simply
    // take effect immediately. The end state is unchanged — only the glide is.
    if reduced_motion() {
        return None;
    }

    let has_transitions = transitions.is_some_and(|t| !t.is_empty());
    let layout_offset = layout.is_some_and(|l| l.animate_offset);
    let layout_size = layout.is_some_and(|l| l.animate_size);
    if !has_transitions && !layout_offset && !layout_size {
        return None;
    }

    let collection = comp.create_implicit_animation_collection();

    // `this.StartingValue -> this.FinalValue` over `duration`, EaseOut — the
    // standard implicit-transition curve. The compositor retargets a running
    // instance smoothly when the property changes again mid-flight.
    //
    // Each arm inserts into the collection itself rather than yielding one
    // common animation value: the three key-frame animations are distinct
    // types here (there is no shared base to widen them to), and
    // `ImplicitAnimationCollection::insert` takes them generically. The build
    // order per arm — duration, expression key frame, target, insert — is the
    // order the raw path used, and is what the compositor sees.
    let insert_keyframe = |target: &str, duration: Duration, dims: Dims| {
        let easing = easing_for(comp, Easing::EaseOut);
        match dims {
            Dims::Scalar => {
                let a = comp.create_scalar_key_frame_animation();
                a.set_duration(duration);
                a.insert_expression_key_frame_with_easing(1.0, "this.FinalValue", &easing);
                a.set_target(target);
                collection.insert(target, &a);
            }
            Dims::Vector2 => {
                let a = comp.create_vector2_key_frame_animation();
                a.set_duration(duration);
                a.insert_expression_key_frame_with_easing(1.0, "this.FinalValue", &easing);
                a.set_target(target);
                collection.insert(target, &a);
            }
            Dims::Vector3 => {
                let a = comp.create_vector3_key_frame_animation();
                a.set_duration(duration);
                a.insert_expression_key_frame_with_easing(1.0, "this.FinalValue", &easing);
                a.set_target(target);
                collection.insert(target, &a);
            }
        }
    };

    if let Some(t) = transitions.filter(|t| !t.is_empty()) {
        if let Some(s) = t.opacity {
            insert_keyframe("Opacity", s.duration, Dims::Scalar);
        }
        if let Some(s) = t.rotation {
            insert_keyframe("RotationAngle", s.duration, Dims::Scalar);
        }
        if let Some(v) = t.scale {
            insert_keyframe("Scale", v.duration, Dims::Vector3);
        }
        // On this backend there is no separate Translation channel: Offset is
        // the layout-owned position, so a translation transition IS a layout
        // glide (per-axis masking is not supported — all axes animate).
        // Skipped when a layout animation targets Offset below.
        if let Some(v) = t.translation
            && !layout_offset
        {
            insert_keyframe("Offset", v.duration, Dims::Vector3);
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
            insert_keyframe("Offset", l.duration, Dims::Vector3);
        }
        if l.animate_size {
            // Size is a Vector2 property; animating it live-resizes the clip
            // and any surface stretch, so it defaults off in the config.
            insert_keyframe("Size", l.duration, Dims::Vector2);
        }
    }

    Some(collection)
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
        target.stop_animation("Opacity");
        target.set_opacity(to);
        return;
    }

    let a = comp.create_scalar_key_frame_animation();
    a.set_duration(duration);
    a.insert_key_frame_with_easing(1.0, to, &easing_for(comp, easing));
    target.start_animation("Opacity", &a);
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
/// just `set_final_value` + `start_animation` on the cached object, so
/// continuous relayout (interactive resize) allocates nothing per pass. The
/// `get_or_insert_with` below is the whole of that discipline: on every pass
/// after the first it is a discriminant test, and the two calls under it write
/// through the animation the cache already holds. Nothing here may be changed
/// to mint a fresh animation per retarget — that would both allocate per
/// layout pass and reset the spring's velocity, replacing a continuous
/// redirection with a visible restart.
pub(crate) fn spring_offset(
    target: &Visual,
    cache: &mut Option<SpringVector3NaturalMotionAnimation>,
    x: f32,
    y: f32,
    damping_ratio: f32,
    period: f32,
) {
    // The spring's whole purpose is the overshoot; under reduced motion the
    // destination is written straight to Offset. Layout knows that destination
    // exactly, so this is lossless — the element simply arrives without travel.
    if reduced_motion() {
        target.stop_animation("Offset");
        target.set_offset(x, y, 0.0);
        return;
    }

    let a = cache.get_or_insert_with(|| {
        let a = target.compositor().create_spring_vector3_animation();
        a.set_damping_ratio(damping_ratio);
        a.set_period(Duration::from_secs_f32(period.max(0.001)));
        a
    });
    a.set_final_value(Vector3::new(x, y, 0.0));
    target.start_animation("Offset", a);
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
pub(crate) fn snapshot_sprite(comp: &Compositor, source: &Visual, w: f32, h: f32) -> SpriteVisual {
    // `source` is rasterized DETACHED, and a detached visual's default
    // `BorderMode::Inherit` has no parent to inherit from — DWM then renders its
    // edges unantialiased, which is how a flattened ghost picked up hard, stepped
    // edges mid-fade. Stating it on the captured root covers the whole subtree:
    // children left at `Inherit` inherit this.
    source.set_border_mode(BorderMode::Soft);
    let vs = comp.create_visual_surface();
    vs.set_source_visual(source);
    vs.set_source_offset(Vector2::new(0.0, 0.0));
    vs.set_source_size(Vector2::new(w, h));
    let brush = comp.create_surface_brush(&vs);
    let sprite = comp.create_sprite_visual();
    sprite.set_brush(&brush);
    sprite.set_size(w, h);
    sprite
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
        node.container
            .set_center_point(Vector3::new(node.rect.w / 2.0, node.rect.h / 2.0, 0.0));
    }
}
