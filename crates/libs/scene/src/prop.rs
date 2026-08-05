//! The property table: one `const` row per animatable channel, and the operations over it.
//!
//! A row describes a channel for each consumer that needs it — the shadow slot, the setter,
//! the animation starter, the device-loss re-issue — so [`set`], [`start`], [`stop`] and the
//! rebind are each one function over [`PROPS`]. The rows are pure data: [`Owner`] names the
//! shadow a channel lives in and `chan` is its slot, so a node with no clip, shape or shadow
//! carries none of their channels. Writes are per group rather than per channel, so
//! `Offset.X` and `Offset.Y` push one composite.
//!
//! `path` is DirectComposition's animation name and does not follow from the WinRT property
//! name: `"Offset.Y"` and `"Scale.X"` resolve, while a rounded clip's radii exist only as
//! `"TopLeftRadiusX"` and `"TopLeftRadiusY"`. A path the object rejects surfaces as a
//! control that never moves rather than as an error at any seam, so each row states its own
//! path.

use crate::node::{
    CLIP_CHANS, CORE_CHANS, ClipState, Node, SHADOW_CHANS, STROKE_CHANS, TRIM_CHANS,
};
use crate::sink::{Prop, Value, ValueKind};
use windows_composition::{Animatable, CompositionAnimation, Geometry};
use windows_numerics::{Vector2, Vector3};

/// Identifies the composition object a channel lives on, and so which shadow holds it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Visual,
    Clip,
    /// The geometry a shape mask is drawn from. A trim is the geometry's property, and the
    /// sprite shape rejects the name.
    Trim,
    /// The sprite shape, which carries the stroke width and the dash phase.
    Stroke,
    Shadow,
}

/// Selects the shared animation template a group is driven by, and so the type a spring's
/// final value takes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Scalar,
    Vec2,
    /// Carried as a `Vec2` and driven as a `Vector3`: the compositor's offset, scale and
    /// centre point are three-vectors, and the writer supplies the third component.
    Vec3,
}

/// Describes one animatable channel.
pub(crate) struct PropDesc {
    /// The DirectComposition animation name an animation or a tracker expression targets.
    /// Stated per row: which subchannel names resolve varies by property.
    pub(crate) path: &'static str,
    pub(crate) owner: Owner,
    /// The composite this channel belongs to. Binding state is two bits per group, and
    /// [`write_group`] pushes a whole group at once.
    pub(crate) group: u8,
    /// This channel's slot in the owner's shadow.
    pub(crate) chan: u8,
    /// How many channels this row covers, starting at `chan`: one for a scalar row, two for
    /// a composite. Checked against the owner's shadow width, so no row can address past it.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the shadow-bounds proof")
    )]
    pub(crate) span: u8,
    pub(crate) kind: ValueKind,
    pub(crate) anim: Slot,
}

/// Expands one line per row into [`PROPS`], keeping the group and channel numbering in one
/// place.
macro_rules! props {
    ($($prop:ident => $path:literal, $owner:ident, $group:expr, $chan:expr, $span:expr, $kind:ident, $anim:ident;)*) => {
        pub(crate) const PROPS: [PropDesc; PROP_COUNT] = {
            // Positional: `desc` indexes by `prop as usize`, so a row's position here has
            // to equal its `Prop` discriminant.
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

/// How many rows [`PROPS`] holds, one per [`Prop`] variant.
pub(crate) const PROP_COUNT: usize = 32;
/// How many property groups the rows cover. Each holds two bits of binding state in
/// `Node::state`.
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

/// Returns the row describing `prop`.
pub(crate) fn desc(prop: Prop) -> &'static PropDesc {
    &PROPS[prop as usize]
}

/// Records which writer owns a property group's channels.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Held {
    /// Nothing owns it: the shadow is authoritative, so a [`set`] whose value already
    /// matches the shadow writes nothing.
    Free = 0,
    /// An animation was stopped without a value being written. The value the compositor
    /// reached is not knowable, so the next [`set`] writes even when the shadow matches.
    Stale = 1,
    /// A one-shot animation owns it and finishes on its own. A [`set`] stops the animation
    /// first, then writes.
    Playing = 2,
    /// A tracker expression owns it until the binding is stopped, not until it settles.
    /// Layout re-states an offset on every node it touches, and a [`set`] on a bound
    /// channel is refused so it cannot displace the expression.
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

/// Returns the binding state of `group` on `node`.
pub(crate) fn held(node: &Node, group: u8) -> Held {
    Held::from_bits(node.state >> (u64::from(group) * 2))
}

/// Records `state` as the binding state of `group` on `node`.
pub(crate) fn set_held(node: &mut Node, group: u8, state: Held) {
    let shift = u64::from(group) * 2;
    node.state = (node.state & !(0b11 << shift)) | ((state as u64) << shift);
}

/// Starts `animation` on the channel `d` names and records the state the channel enters.
///
/// `held` is [`Held::Playing`] for a one-shot animation, or [`Held::Bound`] for a tracker
/// expression that owns the channel until it is stopped; [`set`] reads that state before
/// writing. Does nothing when the node carries no object of `d.owner`.
pub(crate) fn start(node: &mut Node, d: &PropDesc, animation: &CompositionAnimation, held: Held) {
    let Some(object) = animatable(node, d.owner) else {
        return;
    };
    object.start(d.path, animation);
    set_held(node, d.group, held);
}

/// Stops the animation on the channel `d` names and leaves its group [`Held::Stale`].
///
/// [`Held::Stale`] rather than [`Held::Free`] because the value the compositor reached is
/// not knowable, so the next [`set`] must write even where the shadow already matches.
pub(crate) fn stop(node: &mut Node, d: &PropDesc) {
    if let Some(object) = animatable(node, d.owner) {
        object.stop(d.path);
    }
    set_held(node, d.group, Held::Stale);
}

/// Writes `value` into one channel. Returns whether it reached a composition object.
///
/// Every channel write goes through here — a bound set, a declared clip, a device-loss
/// re-issue, a snap out of an animation — so [`Held`] is honoured in one place. Returns
/// `false` when the group is [`Held::Bound`], when the shadow already holds `value`, when
/// `value`'s kind does not match the row, and when the node carries no object of the row's
/// owner.
pub(crate) fn set(node: &mut Node, prop: Prop, value: Value) -> bool {
    let d = desc(prop);
    debug_assert_eq!(value.kind(), d.kind, "{} takes a different value", d.path);
    if value.kind() != d.kind {
        return false;
    }
    match held(node, d.group) {
        // A tracker expression owns the channel; a set must not displace it.
        Held::Bound => return false,
        // The shadow is authoritative, so an unchanged value stops here.
        Held::Free => {
            if shadow_eq(node, d, value) {
                return false;
            }
        }
        // What the compositor reached is not knowable, so an equal-valued set must write.
        Held::Stale => {}
        // The animation would keep writing after this set, so it is stopped first.
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

/// Returns the channel shadow `owner` keeps on `node`, or `None` when the node carries no
/// object of that owner.
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

/// What a bind must do first when the channel's owner object is absent from the node.
///
/// The answer differs by owner:
///
/// - A clip stands alone: its sides and radii are meaningful with no other object present,
///   so a bind arriving before any clip was declared mints one and the two ops may be
///   emitted in either order.
/// - A trim, stroke width or dash phase lives on a sprite shape, which exists only on the
///   capture route. The bind promotes the sprite onto that route, keeping the same
///   geometry, and the write then lands.
/// - A glow's blur and opacity live on a drop shadow derived from a captured paint. With no
///   paint there is nothing to address, so the bind is refused.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Absent {
    /// Mints a rectangle clip, then writes.
    MintClip,
    /// Rebuilds the sprite onto the capture route, then writes.
    Promote,
    /// Refuses the bind: there is no object the channel could be written to.
    Refuse,
}

/// Returns what a bind on `owner` must do when that object is absent from the node.
pub(crate) fn absent(owner: Owner) -> Absent {
    match owner {
        // A visual always exists — the node is one.
        Owner::Visual => Absent::Refuse,
        Owner::Clip => Absent::MintClip,
        Owner::Trim | Owner::Stroke => Absent::Promote,
        Owner::Shadow => Absent::Refuse,
    }
}

/// Returns whether `owner`'s object exists on `node`.
pub(crate) fn has_owner(node: &Node, owner: Owner) -> bool {
    match owner {
        Owner::Visual => true,
        Owner::Clip => node.clip.as_ref().is_some_and(|c| c.rect().is_some()),
        Owner::Trim | Owner::Stroke => node.shape.is_some(),
        Owner::Shadow => node.shadow.is_some(),
    }
}

/// Returns whether the shadow already holds `value` in `d`'s channels. `false` when the
/// node carries no object of `d.owner`.
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

/// Writes `value` into `d`'s shadow channels. Returns whether the row's owner exists.
///
/// A write to a channel whose owner is absent is a no-op rather than a panic: a clip and the
/// mask that carries it may be emitted in either order.
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

/// Pushes a whole group from the shadow onto the composition object that holds it.
///
/// Per group rather than per channel: a vector setter takes every component and the shadow
/// holds them. Does nothing when the group's owner object is absent.
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
                // The platform's setter takes all four sides, so one write covers whichever
                // of them changed.
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
                // The geometry's setter takes both ends, so one write covers whichever of
                // them changed.
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

/// Returns the composition object an animation for `owner` is started on, or `None` when
/// the node carries no such object.
///
/// Every owner exposes the same start and stop methods, so one call site drives a corner
/// radius on a clip, a trim on a geometry and a blur on a shadow alike.
pub(crate) fn animatable(node: &Node, owner: Owner) -> Option<&dyn AnimatableRef> {
    match owner {
        Owner::Visual => Some(&node.visual),
        Owner::Clip => node
            .clip
            .as_ref()
            .and_then(ClipState::rect)
            .map(|c| c as &dyn AnimatableRef),
        // The geometry, not the shape: a trim is the geometry's property and the shape
        // rejects the name.
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

/// Starts and stops animations on a composition object.
///
/// An object-safe view of [`Animatable`], so [`animatable`] can hand back any owner's object
/// behind one type.
pub(crate) trait AnimatableRef {
    /// Starts `animation` on the property `path` names.
    fn start(&self, path: &str, animation: &CompositionAnimation);
    /// Stops whatever animation drives the property `path` names.
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

    /// Starts and stops every row's animation against a real compositor.
    ///
    /// A path the object rejects and a value type it will not take are the two ways a row
    /// can be wrong, and neither surfaces at any seam — the control simply never moves. Both
    /// are covered here by starting the animation kind the row declares: `start_animation`
    /// unwraps, so either failure fails the test.
    ///
    /// Skipped where no dispatcher queue can be created, since a compositor needs one.
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
            // The animation kind the row declares: the platform answers a mismatched type
            // and a misspelt name with the same error.
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
        // The platform rejects the WinRT `Vector2` radius name and its subchannels, so a
        // radius row has to name one per-channel scalar.
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
