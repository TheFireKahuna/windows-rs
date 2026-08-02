//! The node arena. **Front half.**
//!
//! One arena holds both kinds of node: a sprite visual *is* a container visual, so **only
//! the mint branches on kind** and everything else loses a two-arm match. That one branch
//! is also what gives `SpriteId` and `GroupId` real backing.

use crate::id::Id;
use crate::sink::{Clip, Mask, NodeId, NodeKind, Paint};
use crate::tree::{Forest, Links};
use windows_composition::{
    CompositionBrush, CompositionGeometricClip, CompositionMaskBrush, CompositionPathGeometry,
    CompositionSpriteShape, CompositionSurfaceBrush, DropShadow, RectangleClip, ShapeVisual,
    SpriteVisual, Visual,
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

/// A generational arena over one family.
///
/// The model mints and the scene stores: the free list and the generation counter live on
/// the app side, so an id freed by a destroy can be reused by a create in the *same* patch
/// with no round trip. This side only validates.
#[derive(Debug)]
pub(crate) struct Slots<T> {
    items: Vec<Option<(u32, T)>>,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> Slots<T> {
    pub(crate) fn insert<F>(&mut self, id: Id<F>, value: T) {
        let index = id.index();
        if index >= self.items.len() {
            self.items.resize_with(index + 1, || None);
        }
        self.items[index] = Some((id.generation(), value));
    }

    pub(crate) fn get<F>(&self, id: Id<F>) -> Option<&T> {
        match self.items.get(id.index()) {
            Some(Some((generation, value))) if *generation == id.generation() => Some(value),
            _ => None,
        }
    }

    pub(crate) fn get_mut<F>(&mut self, id: Id<F>) -> Option<&mut T> {
        match self.items.get_mut(id.index()) {
            Some(Some((generation, value))) if *generation == id.generation() => Some(value),
            _ => None,
        }
    }

    pub(crate) fn remove<F>(&mut self, id: Id<F>) -> Option<T> {
        match self.items.get_mut(id.index()) {
            Some(slot @ Some(_)) if slot.as_ref().is_some_and(|(g, _)| *g == id.generation()) => {
                slot.take().map(|(_, value)| value)
            }
            _ => None,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.items.iter().filter(|slot| slot.is_some()).count()
    }

    /// Every live id in the arena, for the walks that genuinely need all of them.
    pub(crate) fn iter_ids<F>(&self) -> impl Iterator<Item = Id<F>> + '_ {
        self.items.iter().enumerate().filter_map(|(index, slot)| {
            slot.as_ref()
                .map(|(generation, _)| Id::raw(index as u32, *generation))
        })
    }
}

impl Forest for Slots<Node> {
    fn links(&self, id: NodeId) -> Option<&Links> {
        self.get(id).map(|node| &node.links)
    }

    fn links_mut(&mut self, id: NodeId) -> Option<&mut Links> {
        self.get_mut(id).map(|node| &mut node.links)
    }
}

/// A dash pattern, inline for the lengths a design actually uses.
///
/// Held on the node because a rebind can be provoked by something carrying no patch — a
/// device loss, a DPI change — and a stroked shape silently losing its dashes on a monitor
/// change is a bug nothing reports. Eight runs is four dash-and-gap pairs; longer is not
/// expressible, because a dash array is a design token and not data.
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
/// The chain is **flat, always**: a mask brush is never the mask or source of another — the
/// platform documents that combination as throwing — so a gradient is one FP16 strip.
pub(crate) struct Painted {
    /// The realized chain, held so this crate owns what the compositor paints with rather
    /// than inferring it from what the compositor kept alive. `combined` is `None` when the
    /// mask is [`Mask::None`] or a shape took the clip route — the paint then binds
    /// directly, which is what a presented buffer requires, since a mask brush in the chain
    /// disqualifies it from a display plane.
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
    /// Whether this chain is what put a clip on the node's visual.
    ///
    /// True for exactly one shape: a [`Mask::Shape`] on the clip route. A clip the *sink*
    /// declared lives in [`Node::clip`] and is never this, which lets a route change tear
    /// down its own clip without touching one it does not own.
    pub(crate) fn owns_the_clip(&self) -> bool {
        self.route == Route::Clip && matches!(self.mask, Mask::Shape { .. })
    }

    /// What this declaration is, for deciding whether a re-declaration changed anything.
    pub(crate) fn declaration(&self) -> (Mask, Dashes, Paint) {
        (self.mask, self.dashes, self.paint)
    }

    /// Whether the realized chain still matches the generations it was built under.
    pub(crate) fn fresh(&self, now: crate::cache::Gen) -> bool {
        self.mask
            .deps()
            .union(self.paint.deps())
            .fresh(self.built_at, now)
    }
}

/// Which of the two constructions realizes a [`Mask::Shape`].
///
/// **The author never names one.** A stroke, a bound trim or dash phase, or an occupied
/// clip slot each force the capture; everything else takes the clip, four composition
/// objects and an off-tree render cheaper. A clip-route sprite acquiring any of them is
/// *promoted* in place onto the same geometry, so correctness never depends on the author
/// having foreseen an animation.
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

/// A node's clip, and — for the one kind that has any — the shadow of its channels.
///
/// Two kinds, because they are different objects: a rectangle clip carries four animatable
/// sides and eight per-corner radius scalars, a geometric clip carries a shape and nothing
/// animatable. One type would mean a channel table live for half its values.
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
    /// The channel shadow, or `None` for a kind that has none.
    pub(crate) fn chans_mut(&mut self) -> Option<&mut [f32; CLIP_CHANS]> {
        match self {
            Self::Rect { chans, .. } => Some(chans),
            Self::Geom(_) => None,
        }
    }

    pub(crate) fn rect(&self) -> Option<&RectangleClip> {
        match self {
            Self::Rect { clip, .. } => Some(clip),
            Self::Geom(_) => None,
        }
    }
}

/// The off-tree capture a stroked or trimmed shape mask is built from.
///
/// **Two channel groups, because they are two objects.** A trim is a property of the
/// geometry and a stroke is a property of the shape drawn from it, so an animation aimed at
/// the wrong one is refused outright — which is what splits the shadow here rather than
/// keeping one array over both.
pub(crate) struct ShapeState {
    /// The off-tree visual the capture reads. Held because nothing else does, and a resize
    /// has to re-size it.
    #[expect(
        dead_code,
        reason = "anchors the captured subtree; resize will read it"
    )]
    pub(crate) host: ShapeVisual,
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
    pub(crate) chans: [f32; SHADOW_CHANS],
}

/// One node.
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
    /// touches. This is the shadow that makes an unchanged re-statement free, exactly as
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

    /// The node's own box, in DIPs.
    pub(crate) fn size(&self) -> windows_numerics::Vector2 {
        windows_numerics::Vector2 {
            x: self.core[2],
            y: self.core[3],
        }
    }
}
