//! The property table: one description, four consumers. **Front half.**
//!
//! A property appears in the shadow, in the setter, in the animation starter and in the
//! device-loss re-issue — four matches over the same set that have to agree. So there is
//! one `const` table that is **pure data**, and `set`, `animate`, `stop`, `track` and the
//! rebind are each *one* function over it.
//!
//! Two things make it data and not a table of closures. [`Owner`] partitions the shadow, so
//! `chan` *is* the slot and no accessor closure is needed — a node with no clip, shape or
//! shadow pays nothing for the twenty channels it lacks. And the writer is **per group, not
//! per channel**: `Offset.X` and `Offset.Y` share one writer that pushes the composite.
//!
//! # Where the rows come from
//!
//! No rule derives an animation property path from a WinRT property, and believing there is
//! one is how a control ends up never animating. The name space is *DirectComposition's*,
//! which `Windows.UI.Composition` projects unevenly: `"Offset.Y"` and `"Scale.X"` resolve,
//! while `"TopLeftRadius.X"` errors on the same call — a rounded clip's radii exist there
//! only as `TopLeftRadiusX` and `TopLeftRadiusY`.
//!
//! Rows are sourced from the class page's per-property *Animatable* annotation read **as a
//! contrast within one table**: `RectangleClip` annotates its four sides and leaves all four
//! radii bare, which is the finding. Read as an absence it is worthless — `Visual.Scale`
//! carries no annotation and animates. Then from DirectComposition's setter overloads, then
//! from the metadata for existence and type.
//!
//! None of those is complete and a wrong path is a control that never moves, so the table
//! validates itself. See `every_prop_row_animates` in this crate's device tests.

use crate::node::{
    CLIP_CHANS, CORE_CHANS, ClipState, Node, SHADOW_CHANS, STROKE_CHANS, TRIM_CHANS,
};
use crate::sink::{Prop, Value, ValueKind};
use windows_composition::{Animatable, CompositionAnimation, Geometry};
use windows_numerics::{Vector2, Vector3};

/// Which composition object holds a channel — and therefore which shadow it lives in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Visual,
    Clip,
    /// The **geometry** a shape mask is drawn from. A trim is its property, not the
    /// shape's, and aiming a trim animation at the shape is refused outright.
    Trim,
    /// The sprite shape itself: what a stroke's width and dash phase live on.
    Stroke,
    Shadow,
}

/// Which shared animation template drives a group, and therefore what a spring's final
/// value has to be built as.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Scalar,
    Vec2,
    /// Carried as a `Vec2` on the wire and driven as a `Vector3` with a zero third
    /// component, because the compositor's offset, scale and centre are all three-vectors
    /// while nothing in this stack has a third dimension to put in them.
    Vec3,
}

/// One row.
pub(crate) struct PropDesc {
    /// The animation name, which is what an animation and a tracker expression both
    /// target. Per row rather than derived, because subchannel support is a property of
    /// the property.
    pub(crate) path: &'static str,
    pub(crate) owner: Owner,
    /// The composite this channel belongs to. Two bits of binding state are packed per
    /// group, and the writer is per group.
    pub(crate) group: u8,
    /// This channel's slot in the owner's shadow.
    pub(crate) chan: u8,
    /// How many channels the group covers, starting at this row's own — one for a scalar
    /// row, two for a composite. Its only reader is the proof that no row can address past
    /// its owner's shadow, which is the one thing that could make the table unsound.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the shadow-bounds proof")
    )]
    pub(crate) span: u8,
    pub(crate) kind: ValueKind,
    pub(crate) anim: Slot,
}

/// Declares a row and keeps the group numbering in one place.
macro_rules! props {
    ($($prop:ident => $path:literal, $owner:ident, $group:expr, $chan:expr, $span:expr, $kind:ident, $anim:ident;)*) => {
        pub(crate) const PROPS: [PropDesc; PROP_COUNT] = {
            // Written positionally so a row's index and its `Prop` discriminant cannot
            // drift apart: the array is indexed by `prop as usize`.
            [$(PropDesc {
                path: $path,
                owner: Owner::$owner,
                group: $group,
                chan: $chan,
                span: $span,
                kind: ValueKind::$kind,
                anim: Slot::$anim,
            }),*]
        };

        #[cfg(test)]
        const ROW_ORDER: [Prop; PROP_COUNT] = [$(Prop::$prop),*];
    };
}

/// Every row in the alphabet.
pub(crate) const PROP_COUNT: usize = 32;
/// Every property group. Two bits of state each, packed into one word.
pub(crate) const GROUP_COUNT: usize = 20;

const _: () = assert!(GROUP_COUNT * 2 <= u64::BITS as usize);

props! {
    // ── the visual ────────────────────────────────────────────────────────────────
    Offset             => "Offset",              Visual, 0,  0, 2, Vec2,   Vec3;
    OffsetX            => "Offset.X",            Visual, 0,  0, 1, Scalar, Scalar;
    OffsetY            => "Offset.Y",            Visual, 0,  1, 1, Scalar, Scalar;
    Size               => "Size",                Visual, 1,  2, 2, Vec2,   Vec2;
    SizeX              => "Size.X",              Visual, 1,  2, 1, Scalar, Scalar;
    SizeY              => "Size.Y",              Visual, 1,  3, 1, Scalar, Scalar;
    Scale              => "Scale",               Visual, 2,  4, 2, Vec2,   Vec3;
    ScaleX             => "Scale.X",             Visual, 2,  4, 1, Scalar, Scalar;
    ScaleY             => "Scale.Y",             Visual, 2,  5, 1, Scalar, Scalar;
    Opacity            => "Opacity",             Visual, 3,  9, 1, Scalar, Scalar;
    RotationAngle      => "RotationAngle",       Visual, 4,  6, 1, Scalar, Scalar;
    Center             => "CenterPoint",         Visual, 5,  7, 2, Vec2,   Vec3;
    CenterX            => "CenterPoint.X",       Visual, 5,  7, 1, Scalar, Scalar;
    CenterY            => "CenterPoint.Y",       Visual, 5,  8, 1, Scalar, Scalar;
    // ── the clip ──────────────────────────────────────────────────────────────────
    ClipL              => "Left",                Clip,   6,  0, 1, Scalar, Scalar;
    ClipT              => "Top",                 Clip,   7,  1, 1, Scalar, Scalar;
    ClipR              => "Right",               Clip,   8,  2, 1, Scalar, Scalar;
    ClipB              => "Bottom",              Clip,   9,  3, 1, Scalar, Scalar;
    CornerTopLeftX     => "TopLeftRadiusX",      Clip,  10,  4, 1, Scalar, Scalar;
    CornerTopLeftY     => "TopLeftRadiusY",      Clip,  10,  5, 1, Scalar, Scalar;
    CornerTopRightX    => "TopRightRadiusX",     Clip,  11,  6, 1, Scalar, Scalar;
    CornerTopRightY    => "TopRightRadiusY",     Clip,  11,  7, 1, Scalar, Scalar;
    CornerBottomRightX => "BottomRightRadiusX",  Clip,  12,  8, 1, Scalar, Scalar;
    CornerBottomRightY => "BottomRightRadiusY",  Clip,  12,  9, 1, Scalar, Scalar;
    CornerBottomLeftX  => "BottomLeftRadiusX",   Clip,  13, 10, 1, Scalar, Scalar;
    CornerBottomLeftY  => "BottomLeftRadiusY",   Clip,  13, 11, 1, Scalar, Scalar;
    // ── the shape mask ────────────────────────────────────────────────────────────
    TrimStart          => "TrimStart",           Trim,  14,  0, 1, Scalar, Scalar;
    TrimEnd            => "TrimEnd",             Trim,  15,  1, 1, Scalar, Scalar;
    StrokeThickness    => "StrokeThickness",     Stroke, 16, 0, 1, Scalar, Scalar;
    DashOffset         => "StrokeDashOffset",    Stroke, 17, 1, 1, Scalar, Scalar;
    // ── the glow ──────────────────────────────────────────────────────────────────
    BlurRadius         => "BlurRadius",          Shadow, 18, 0, 1, Scalar, Scalar;
    ShadowOpacity      => "Opacity",             Shadow, 19, 1, 1, Scalar, Scalar;
}

/// The row for a property.
pub(crate) fn desc(prop: Prop) -> &'static PropDesc {
    &PROPS[prop as usize]
}

/// Who owns a channel right now.
///
/// Four states, because "animated" is two different facts and "bound" is a third.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Held {
    /// The shadow is authoritative, so an unchanged value costs a comparison and nothing
    /// else. That early return is what lets the app thread emit a whole subtree without
    /// diffing it first.
    Free = 0,
    /// An animation was stopped without a value being written, so what the compositor
    /// reached is not knowable and the next set must write whatever it is given.
    Stale = 1,
    /// A one-shot animation owns it and will finish. An equal-valued set must still write,
    /// which makes "a snap is stop-then-set, never a zero-duration spring" code.
    Playing = 2,
    /// A tracker expression owns it **permanently** — not until it settles. Layout writes
    /// an offset on every node it touches, so without a state that refuses the write, the
    /// first layout pass after a scroll container is wired kills the binding. The symptom
    /// is "scrolling stops after a resize", found by hand, far from the cause.
    Bound = 3,
}

impl Held {
    fn from_bits(bits: u64) -> Self {
        match bits & 0b11 {
            0 => Self::Free,
            1 => Self::Stale,
            2 => Self::Playing,
            _ => Self::Bound,
        }
    }
}

pub(crate) fn held(node: &Node, group: u8) -> Held {
    Held::from_bits(node.state >> (u64::from(group) * 2))
}

pub(crate) fn set_held(node: &mut Node, group: u8, state: Held) {
    let shift = u64::from(group) * 2;
    node.state = (node.state & !(0b11 << shift)) | ((state as u64) << shift);
}

/// Hands a channel to the compositor and records who now owns it.
///
/// `held` is the state the channel enters: [`Held::Playing`] for a one-shot that finishes,
/// [`Held::Bound`] for a tracker expression that never does. Pairing them here is the point
/// — an animation begun without the bit set is a channel the next layout pass overwrites.
pub(crate) fn start(node: &mut Node, d: &PropDesc, animation: &CompositionAnimation, held: Held) {
    let Some(object) = animatable(node, d.owner) else {
        return;
    };
    object.start(d.path, animation);
    set_held(node, d.group, held);
}

/// Takes a channel back from the compositor.
///
/// It lands in [`Held::Stale`] and not [`Held::Free`]: what the compositor reached is not
/// knowable, so the next set must write even if the shadow says nothing changed.
pub(crate) fn stop(node: &mut Node, d: &PropDesc) {
    if let Some(object) = animatable(node, d.owner) {
        object.stop(d.path);
    }
    set_held(node, d.group, Held::Stale);
}

/// Writes one channel. Answers whether it actually reached a composition object.
///
/// **The only way a channel value reaches the compositor.** A set, a declared clip, a
/// device-loss re-issue and a snap out of an animation all come through here, so the four
/// binding states are honoured once.
pub(crate) fn set(node: &mut Node, prop: Prop, value: Value) -> bool {
    let d = desc(prop);
    debug_assert_eq!(value.kind(), d.kind, "{} takes a different value", d.path);
    if value.kind() != d.kind {
        return false;
    }
    match held(node, d.group) {
        // A tracker expression owns its channel permanently, and layout writes an offset on
        // every node it touches.
        Held::Bound => return false,
        // The shadow is authoritative, so an unchanged value costs a comparison. This early
        // return is what lets the emitter send a subtree without diffing it.
        Held::Free => {
            if shadow_eq(node, d, value) {
                return false;
            }
        }
        // What the compositor reached is not knowable, so an equal-valued set must write.
        Held::Stale => {}
        // A hard set beats the compositor: stop first, then write.
        Held::Playing => {
            if let Some(object) = animatable(node, d.owner) {
                object.stop(d.path);
            }
        }
    }
    if !write_shadow(node, d, value) {
        return false;
    }
    write_group(node, d.group);
    set_held(node, d.group, Held::Free);
    true
}

/// The shadow slice a row's owner keeps, if the node has that owner at all.
fn shadow(node: &mut Node, owner: Owner) -> Option<&mut [f32]> {
    match owner {
        Owner::Visual => Some(&mut node.core[..CORE_CHANS]),
        Owner::Clip => node
            .clip
            .as_mut()
            .and_then(|c| c.chans_mut())
            .map(|chans| &mut chans[..CLIP_CHANS]),
        Owner::Trim => node.shape.as_mut().map(|s| &mut s.trim[..TRIM_CHANS]),
        Owner::Stroke => node.shape.as_mut().map(|s| &mut s.stroke[..STROKE_CHANS]),
        Owner::Shadow => node.shadow.as_mut().map(|s| &mut s.chans[..SHADOW_CHANS]),
    }
}

/// What has to happen before a channel can be written, when its owner object is absent.
///
/// The rule is **asymmetric**, because the three owners are not alike:
///
/// - A **clip** is free-standing. Its sides and radii mean something with no other object
///   in play, so a bind that arrives before any clip was declared *mints* one, and op
///   ordering genuinely does not matter.
/// - A **shape**'s trim, stroke width and dash phase live on a sprite shape that exists
///   only on the capture route. A bind here is exactly the promotion trigger: the sprite
///   rebuilds onto the capture keeping the same geometry, and the write then lands.
/// - A **glow**'s blur and opacity live on a drop shadow *derived from* a captured paint.
///   There is nothing meaningful to mint without one, so this is a category error rather
///   than an ordering accident.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Absent {
    /// Mint a rectangle clip and write.
    MintClip,
    /// Rebuild the sprite onto the capture route, then write.
    Promote,
    /// Nothing to address. Refused, not silently dropped.
    Refuse,
}

/// What to do about a bind whose owner object does not exist.
pub(crate) fn absent(owner: Owner) -> Absent {
    match owner {
        // A visual always exists — the node is one.
        Owner::Visual => Absent::Refuse,
        Owner::Clip => Absent::MintClip,
        Owner::Trim | Owner::Stroke => Absent::Promote,
        Owner::Shadow => Absent::Refuse,
    }
}

/// Whether a row's owner object exists on this node.
pub(crate) fn has_owner(node: &Node, owner: Owner) -> bool {
    match owner {
        Owner::Visual => true,
        Owner::Clip => node.clip.as_ref().is_some_and(|c| c.rect().is_some()),
        Owner::Trim | Owner::Stroke => node.shape.is_some(),
        Owner::Shadow => node.shadow.is_some(),
    }
}

/// Whether the shadow already holds `value` for this row.
pub(crate) fn shadow_eq(node: &mut Node, d: &PropDesc, value: Value) -> bool {
    let Some(chans) = shadow(node, d.owner) else {
        return false;
    };
    let at = d.chan as usize;
    match value {
        Value::Scalar(v) => chans.get(at) == Some(&v),
        Value::Vec2(v) => chans.get(at) == Some(&v.x) && chans.get(at + 1) == Some(&v.y),
    }
}

/// Writes `value` into the shadow. Returns whether the row's owner exists at all — a bind
/// to a clip channel on a node with no clip is a no-op rather than a panic, because the
/// clip arrives with the mask and the two can be emitted in either order.
pub(crate) fn write_shadow(node: &mut Node, d: &PropDesc, value: Value) -> bool {
    let Some(chans) = shadow(node, d.owner) else {
        return false;
    };
    let at = d.chan as usize;
    match value {
        Value::Scalar(v) => {
            if let Some(slot) = chans.get_mut(at) {
                *slot = v;
            }
        }
        Value::Vec2(v) => {
            if let Some(slot) = chans.get_mut(at) {
                *slot = v.x;
            }
            if let Some(slot) = chans.get_mut(at + 1) {
                *slot = v.y;
            }
        }
    }
    true
}

/// Pushes a whole group from the shadow to the composition object that holds it.
///
/// Per group rather than per channel because a vector setter needs every component, and
/// the shadow has them. Roughly a dozen arms cover thirty channels.
pub(crate) fn write_group(node: &Node, group: u8) {
    let core = &node.core;
    match group {
        0 => node.visual.set_offset(core[0], core[1], 0.0),
        1 => node.visual.set_size(core[2], core[3]),
        2 => node.visual.set_scale(Vector3 {
            x: core[4],
            y: core[5],
            z: 1.0,
        }),
        3 => node.visual.set_opacity(core[9]),
        4 => node.visual.set_rotation_angle(core[6]),
        5 => node.visual.set_center_point(Vector3 {
            x: core[7],
            y: core[8],
            z: 0.0,
        }),
        6..=13 => {
            let Some(ClipState::Rect { clip, chans }) = node.clip.as_ref() else {
                return;
            };
            let (state, c) = (clip, chans);
            match group {
                // The four sides move together on the platform's own setter, so one write
                // covers whichever of them changed.
                6..=9 => state.set_sides(c[0], c[1], c[2], c[3]),
                _ => state.set_corner_radii(
                    Vector2 { x: c[4], y: c[5] },
                    Vector2 { x: c[6], y: c[7] },
                    Vector2 { x: c[8], y: c[9] },
                    Vector2 { x: c[10], y: c[11] },
                ),
            }
        }
        14..=17 => {
            let Some(state) = node.shape.as_ref() else {
                return;
            };
            match group {
                // Both ends move together on the geometry's own setter, so one write
                // covers whichever of them changed.
                14 | 15 => state
                    .geometry
                    .as_geometry()
                    .set_trim(state.trim[0], state.trim[1]),
                16 => state.shape.set_stroke_thickness(state.stroke[0]),
                _ => state.shape.set_stroke_dash_offset(state.stroke[1]),
            }
        }
        18 | 19 => {
            let Some(state) = node.shadow.as_ref() else {
                return;
            };
            if group == 18 {
                state.shadow.set_blur_radius(state.chans[0]);
            } else {
                state.shadow.set_opacity(state.chans[1]);
            }
        }
        _ => {}
    }
}

/// The composition object a row's animation is started on.
///
/// This is where the wrapper's one animation trait pays off: starting an animation is a
/// composition-object method, so a corner radius on a clip, a trim on a geometry and a blur
/// on a shadow all animate through the identical line.
pub(crate) fn animatable(node: &Node, owner: Owner) -> Option<&dyn AnimatableRef> {
    match owner {
        Owner::Visual => Some(&node.visual),
        Owner::Clip => node
            .clip
            .as_ref()
            .and_then(ClipState::rect)
            .map(|c| c as &dyn AnimatableRef),
        // The geometry, not the shape: a trim is the geometry's property and the shape
        // rejects the name outright.
        Owner::Trim => node
            .shape
            .as_ref()
            .map(|s| &s.geometry as &dyn AnimatableRef),
        Owner::Stroke => node.shape.as_ref().map(|s| &s.shape as &dyn AnimatableRef),
        Owner::Shadow => node
            .shadow
            .as_ref()
            .map(|s| &s.shadow as &dyn AnimatableRef),
    }
}

/// Object-safe view of the wrapper's sealed animation trait, so one line can start an
/// animation on whichever of the four objects a row names.
pub(crate) trait AnimatableRef {
    fn start(&self, path: &str, animation: &CompositionAnimation);
    fn stop(&self, path: &str);
}

impl<T: Animatable> AnimatableRef for T {
    fn start(&self, path: &str, animation: &CompositionAnimation) {
        self.start_animation(path, animation);
    }

    fn stop(&self, path: &str) {
        self.stop_animation(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row's animation path, against the real compositor.
    ///
    /// **The table's only true oracle.** Every other test here proves internal
    /// consistency — that a row sits at its discriminant, that a channel is inside its
    /// owner's shadow — and none of them can tell whether a path *resolves*. The name
    /// space is DirectComposition's and `Windows.UI.Composition` projects it unevenly, so
    /// a wrong path is not an error anywhere: it is a control that silently never moves.
    ///
    /// `start_animation` unwraps, so a path the object rejects and a type it will not take
    /// both fail here rather than at the first hover of a shipped build.
    ///
    /// Needs a compositor, and therefore a session with one — it is skipped where a
    /// dispatcher queue cannot be created rather than failing a build that has no display.
    #[test]
    fn every_prop_row_animates() {
        use windows_composition::{Animation, Compositor, DispatcherQueueController};

        let Ok(_queue) = DispatcherQueueController::create_on_current_thread() else {
            eprintln!("skipped: no dispatcher queue in this session");
            return;
        };
        let compositor = Compositor::new().expect("a compositor");

        // One object per owner, each the type the table says carries that row's channels.
        let sprite = compositor.create_sprite_visual();
        let visual = crate::base_of_sprite(&sprite);
        let clip = compositor.create_rectangle_clip();
        let geometry = compositor.create_ellipse_geometry();
        let shape = compositor.create_sprite_shape(&geometry);
        let shadow = compositor.create_drop_shadow();

        for row in PROPS {
            let target: &dyn AnimatableRef = match row.owner {
                Owner::Visual => &visual,
                Owner::Clip => &clip,
                Owner::Trim => &geometry,
                Owner::Stroke => &shape,
                Owner::Shadow => &shadow,
            };
            // The animation's type is part of what a path accepts: the platform answers a
            // mismatched type and a misspelt name with the same error, so both are covered
            // by starting the kind the row declares.
            let animation = match row.anim {
                Slot::Scalar => {
                    let a = compositor.create_spring_scalar_animation();
                    a.set_final_value(1.0);
                    a.as_animation()
                }
                Slot::Vec2 => {
                    let a = compositor.create_spring_vector2_animation();
                    a.set_final_value(Vector2 { x: 1.0, y: 1.0 });
                    a.as_animation()
                }
                Slot::Vec3 => {
                    let a = compositor.create_spring_vector3_animation();
                    a.set_final_value(Vector3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    });
                    a.as_animation()
                }
            };
            // Panics inside the wrapper if the object refuses the name or the type.
            target.start(row.path, &animation);
            target.stop(row.path);
        }
    }

    #[test]
    fn every_row_sits_at_its_own_discriminant() {
        for (index, prop) in ROW_ORDER.iter().enumerate() {
            assert_eq!(
                *prop as usize, index,
                "{prop:?} is declared at row {index} but discriminates to {}",
                *prop as usize
            );
        }
    }

    #[test]
    fn every_group_is_within_the_packed_state_word() {
        for row in PROPS {
            assert!(
                usize::from(row.group) < GROUP_COUNT,
                "{} is in group {}",
                row.path,
                row.group
            );
        }
    }

    #[test]
    fn every_channel_is_within_its_owners_shadow() {
        for row in PROPS {
            let width = match row.owner {
                Owner::Visual => CORE_CHANS,
                Owner::Clip => CLIP_CHANS,
                Owner::Trim => TRIM_CHANS,
                Owner::Stroke => STROKE_CHANS,
                Owner::Shadow => SHADOW_CHANS,
            };
            assert!(
                usize::from(row.chan) + usize::from(row.span) <= width,
                "{} spans past its owner's shadow",
                row.path
            );
        }
    }

    #[test]
    fn a_group_never_spans_two_owners() {
        let mut owner_of = [None; GROUP_COUNT];
        for row in PROPS {
            let slot = &mut owner_of[usize::from(row.group)];
            match slot {
                None => *slot = Some(row.owner),
                Some(existing) => assert_eq!(
                    *existing, row.owner,
                    "group {} spans two owners, so its writer cannot be one function",
                    row.group
                ),
            }
        }
    }

    #[test]
    fn no_corner_radius_is_addressed_as_a_vector() {
        // The whole point of the per-channel rows: naming the WinRT `Vector2` radius, or
        // its subchannel, is rejected by the platform — so a vector row here would be a
        // control that silently never moves.
        for row in PROPS {
            if row.owner == Owner::Clip && row.path.contains("Radius") {
                assert_eq!(row.kind, ValueKind::Scalar, "{}", row.path);
                assert!(
                    row.path.ends_with('X') || row.path.ends_with('Y'),
                    "{} is not a per-channel radius name",
                    row.path
                );
            }
        }
    }

    #[test]
    fn binding_state_is_two_bits_per_group_and_they_do_not_overlap() {
        let mut node_state = 0u64;
        for group in 0..GROUP_COUNT as u8 {
            let shift = u64::from(group) * 2;
            node_state = (node_state & !(0b11 << shift)) | ((Held::Bound as u64) << shift);
        }
        for group in 0..GROUP_COUNT as u8 {
            assert_eq!(
                Held::from_bits(node_state >> (u64::from(group) * 2)),
                Held::Bound
            );
        }
    }
}
