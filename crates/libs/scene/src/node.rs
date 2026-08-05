//! The node arena. **Front half.**
//!
//! One arena holds both kinds of node: a sprite visual is a container visual, so only the
//! mint branches on [`NodeKind`] and every other operation is kind-agnostic. That branch is
//! what backs `SpriteId` and `GroupId`.

use crate::id::Slots;
use crate::sink::{Clip, Mask, NodeId, NodeKind, Paint};
use crate::tree::{Forest, Links};
use windows_composition::{
    Captured, CompositionBrush, CompositionGeometricClip, CompositionMaskBrush,
    CompositionPathGeometry, CompositionSpriteShape, CompositionSurfaceBrush, DropShadow,
    RectangleClip, ShapeVisual, SpriteVisual, Visual,
};

/// How many channels a node's own visual carries: offset, size and scale as pairs, plus a
/// rotation, a centre pair and an opacity.
pub(crate) const CORE_CHANS: usize = 10;
/// A rectangle clip's: four sides and eight per-corner radii.
pub(crate) const CLIP_CHANS: usize = 12;
/// The trim window, which lives on the **geometry** and not on the shape.
pub(crate) const TRIM_CHANS: usize = 2;
/// A stroke's: its width and its dash phase, both on the sprite shape.
pub(crate) const STROKE_CHANS: usize = 2;
/// A glow's: the blur radius and the shadow's own opacity.
pub(crate) const SHADOW_CHANS: usize = 2;

/// Splices the arena's nodes as a child list.
///
/// The model mints ids and this side stores them: the free list and the generation counter
/// live on the app side, so an id freed by a destroy can be reused by a create in the same
/// patch with no round trip, and this side only validates.
impl Forest for Slots<crate::sink::Node, Node> {
    fn links(&self, id: NodeId) -> Option<&Links> {
        self.get(id).map(|node| &node.links)
    }

    fn links_mut(&mut self, id: NodeId) -> Option<&mut Links> {
        self.get_mut(id).map(|node| &mut node.links)
    }
}

/// A dash pattern, held inline: eight runs, or four dash-and-gap pairs.
///
/// Held on the node because a rebind can be provoked by something carrying no patch — a
/// device loss, a DPI change — and the pattern has to survive that. A longer pattern is
/// truncated.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct Dashes {
    runs: [f32; 8],
    len: u8,
}

impl From<&[f32]> for Dashes {
    fn from(source: &[f32]) -> Self {
        let len = source.len().min(8);
        debug_assert!(
            source.len() <= 8,
            "a dash pattern longer than eight runs was truncated"
        );
        let mut runs = [0.0; 8];
        runs[..len].copy_from_slice(&source[..len]);
        Self {
            runs,
            len: len as u8,
        }
    }
}

impl Dashes {
    pub(crate) fn as_slice(&self) -> &[f32] {
        &self.runs[..self.len as usize]
    }
}

/// The realized brush chain for a sprite, and the declaration it was built from.
///
/// The chain is always flat: a mask brush is never the mask or source of another, which the
/// platform documents as throwing, so a gradient is one FP16 strip.
pub(crate) struct Painted {
    /// The realized chain, held so this crate owns what the compositor paints with rather
    /// than inferring it from what the compositor kept alive. `None` when the mask is
    /// [`Mask::None`] or a shape took the clip route, where the paint binds directly: a mask
    /// brush in the chain disqualifies a presented buffer from a display plane.
    #[expect(dead_code, reason = "owns the chain the compositor is painting with")]
    pub(crate) combined: Option<CompositionMaskBrush>,
    /// The alpha source, as the base brush type: a coverage tile and a shape capture are
    /// surface brushes, and a box cell reaches the slot through a nine-grid, which is not.
    #[expect(dead_code, reason = "owns the chain the compositor is painting with")]
    pub(crate) mask_brush: Option<CompositionBrush>,
    #[expect(dead_code, reason = "owns the chain the compositor is painting with")]
    pub(crate) paint_brush: Option<CompositionSurfaceBrush>,
    pub(crate) mask: Mask,
    pub(crate) paint: Paint,
    pub(crate) dashes: Dashes,
    /// Which construction realized the mask, so a route change can be detected rather than
    /// guessed.
    pub(crate) route: Route,
    /// The generation this chain was realized at, checked against the generations its own
    /// mask and paint read. A chain gone stale rebinds down exactly the path its first bind
    /// took.
    pub(crate) built_at: crate::cache::Gen,
}

impl Painted {
    /// Returns whether this chain put the clip on the node's visual.
    ///
    /// True only for a [`Mask::Shape`] on the clip route. A clip the sink declared lives in
    /// [`Node::clip`] instead, so a route change tears down its own clip and leaves that one
    /// standing.
    pub(crate) fn owns_the_clip(&self) -> bool {
        self.route == Route::Clip && matches!(self.mask, Mask::Shape { .. })
    }

    /// Returns the declaration this chain was built from, for comparing against a
    /// re-declaration.
    pub(crate) fn declaration(&self) -> (Mask, Dashes, Paint) {
        (self.mask, self.dashes, self.paint)
    }

    /// Returns whether the realized chain still matches the generations it was built under.
    pub(crate) fn fresh(&self, now: crate::cache::Gen) -> bool {
        self.mask
            .deps()
            .union(self.paint.deps())
            .fresh(self.built_at, now)
    }
}

/// Which of the two constructions realizes a [`Mask::Shape`].
///
/// The route is derived and never authored. A stroke, a bound trim or dash phase, or an
/// occupied clip slot each force [`Route::Capture`]; everything else takes [`Route::Clip`],
/// which is four composition objects and an off-tree render cheaper. A clip-route sprite
/// that acquires any of them is promoted in place onto the same geometry.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum Route {
    /// No mask brush at all: the paint binds directly and a geometric clip carries the
    /// shape, with a soft border mode for an antialiased edge.
    #[default]
    Clip,
    /// An off-tree shape visual captured through a visual surface. The only route that can
    /// stroke, trim or dash, because those properties live on a sprite shape.
    Capture,
}

/// A node's clip, and the channel shadow for the kind that has one.
///
/// A rectangle clip carries four animatable sides and eight per-corner radius scalars; a
/// geometric clip carries a shape and nothing animatable.
pub(crate) enum ClipState {
    Rect {
        clip: RectangleClip,
        chans: [f32; CLIP_CHANS],
    },
    /// Held rather than only set on the visual, so the clip this crate established is
    /// distinguishable from one a shape mask put there.
    Geom(#[expect(dead_code, reason = "identifies the slot's occupant")] CompositionGeometricClip),
}

impl ClipState {
    /// Returns the channel shadow, or `None` for a geometric clip.
    pub(crate) fn chans_mut(&mut self) -> Option<&mut [f32; CLIP_CHANS]> {
        match self {
            Self::Rect { chans, .. } => Some(chans),
            Self::Geom(_) => None,
        }
    }

    /// Returns the rectangle clip, or `None` for a geometric one.
    pub(crate) fn rect(&self) -> Option<&RectangleClip> {
        match self {
            Self::Rect { clip, .. } => Some(clip),
            Self::Geom(_) => None,
        }
    }
}

/// The off-tree capture a stroked or trimmed shape mask is built from.
///
/// The channel shadow is split in two because the properties live on two objects: a trim
/// belongs to the geometry and a stroke to the shape drawn from it, and an animation aimed
/// at the wrong object is refused.
pub(crate) struct ShapeState {
    /// The off-tree visual the capture reads. Held because nothing else does, and because a
    /// resize has to re-size it.
    pub(crate) host: ShapeVisual,
    /// The capture, kept so a box that moves can correct its extent. Left uncorrected, the
    /// capture holds the extent it was built at and the path draws at the wrong scale.
    pub(crate) captured: Captured,
    pub(crate) shape: CompositionSpriteShape,
    pub(crate) geometry: CompositionPathGeometry,
    /// Start and end of the draw-on window, on the geometry.
    pub(crate) trim: [f32; TRIM_CHANS],
    /// Stroke width and dash phase, on the shape.
    pub(crate) stroke: [f32; STROKE_CHANS],
}

/// The blur a [`Paint::Captured`] glow rides on.
pub(crate) struct ShadowState {
    pub(crate) shadow: DropShadow,
    /// The silhouette being blurred, kept so a box that moves can correct its extent: the
    /// halo is a capture of the box.
    pub(crate) captured: Captured,
    pub(crate) chans: [f32; SHADOW_CHANS],
}

/// One node: its visual, its place in the child list, and the shadow of everything bound
/// onto it.
pub(crate) struct Node {
    pub(crate) visual: Visual,
    /// The same object as the type that can be painted; `None` on a group. A base visual
    /// cannot be narrowed back to a sprite from outside the wrapper, so this costs a pointer
    /// on the nodes that paint and nothing elsewhere.
    pub(crate) sprite: Option<SpriteVisual>,
    pub(crate) kind: NodeKind,
    pub(crate) links: Links,
    /// The shadow of the visual's own channels.
    pub(crate) core: [f32; CORE_CHANS],
    /// Two bits of binding state per property group.
    pub(crate) state: u64,
    pub(crate) painted: Option<Painted>,
    /// The clip object the *sink* established, if any. A clip-route shape mask puts its own
    /// geometric clip straight on the visual and never claims this, which is what lets a
    /// route change tear down its own clip without touching one it does not own.
    pub(crate) clip: Option<ClipState>,
    /// The last clip declared for this node.
    ///
    /// A clip is declared rather than diffed, so layout re-states it on every node it
    /// touches; comparing against this shadow makes an unchanged re-statement free, as
    /// [`prop::set`](crate::prop::set) does for the twelve channels underneath it.
    pub(crate) declared_clip: Clip,
    pub(crate) shape: Option<ShapeState>,
    pub(crate) shadow: Option<ShadowState>,
}

impl Node {
    pub(crate) fn new(visual: Visual, sprite: Option<SpriteVisual>, kind: NodeKind) -> Self {
        Self {
            visual,
            sprite,
            kind,
            links: Links::default(),
            // The identity transform, matching `Xform::default` — so a node that is never
            // bound is at the origin, unrotated, unscaled and opaque rather than invisible.
            core: [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            state: 0,
            painted: None,
            clip: None,
            declared_clip: Clip::None,
            shape: None,
            shadow: None,
        }
    }

    /// Returns the node's own box, in DIPs.
    pub(crate) fn size(&self) -> windows_numerics::Vector2 {
        windows_numerics::Vector2 {
            x: self.core[2],
            y: self.core[3],
        }
    }
}
