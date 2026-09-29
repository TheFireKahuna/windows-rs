//! The node arena, the child splice and the property table. **Scene half.**
//!
//! One arena holds both kinds of node, because a sprite visual *is* a container visual and
//! only the mint branches on kind. Every animatable channel is described once, as `const`
//! data, so the shadow, the setter, the animation starter and the device-loss re-issue are
//! four readers of one table rather than four matches that must agree.
//!
//! `path` is DirectComposition's animation name and does not follow from the WinRT property
//! name: `"Offset.Y"` and `"Scale.X"` resolve, while a rounded clip's radii exist only as
//! `"TopLeftRadiusX"` and `"TopLeftRadiusY"`. A path the object rejects surfaces as a control
//! that never moves rather than as an error at any seam, so each row states its own path.

use crate::realize::BoxKey;
use crate::sink::*;
use core::num::NonZeroU32;
use windows_d2d::note;
use windows_composition::{
    Animatable, Captured, CompositionAnimation, CompositionBrush, CompositionEffectBrush,
    CompositionGeometricClip, CompositionMaskBrush, CompositionPathGeometry,
    CompositionPropertySet, CompositionSpriteShape, ContainerVisual, ExpressionAnimation,
    Geometry, InsetClip, RectangleClip, ShapeVisual, SpriteVisual, Visual,
};
use windows_numerics::{Vector2, Vector3};

/// How many channels a node's own visual carries: offset, size and scale as pairs, plus a
/// rotation, a centre pair and an opacity.
pub const CORE_CHANS: u8 = 10;
/// How many channels its side payloads carry: four clip sides, eight corner radii, a trim
/// pair, a stroke pair, a shadow pair, an anchor pair and a local translation pair.
pub const AUX_CHANS: u8 = 22;
/// The absence of a sibling, a parent or a first child.
pub const NO_LINK: u32 = u32::MAX;

// ── the child splice, shared by both halves ─────────────────────────────────────────

/// Intrusive sibling links.
///
/// A per-node `Vec` is 24 bytes and a heap allocation on every branch node, for a shadow
/// whose only consumers are the paint-order walk and the census. Four indices give O(1) link
/// and unlink with no allocation, and they mirror a visual collection's own vocabulary.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Links {
    pub parent: u32,
    pub first: u32,
    pub next: u32,
    pub prev: u32,
}

impl Default for Links {
    fn default() -> Self {
        Self {
            parent: NO_LINK,
            first: NO_LINK,
            next: NO_LINK,
            prev: NO_LINK,
        }
    }
}

/// A store of nodes that can be spliced, keyed by row index.
///
/// Written once and serving both halves: two hand-written splices put the same subtle bug in
/// two places, only one of which is covered. An index and not an id, so [`NO_LINK`] is a
/// sentinel the splice cannot confuse with a live row.
pub trait Forest {
    fn links(&self, at: u32) -> Links;
    fn links_mut(&mut self, at: u32) -> &mut Links;
    fn id_at(&self, at: u32) -> NodeId;
}

/// Splices `at` into `parent`'s children, directly above `after`.
///
/// `after` names the sibling below; `None` is the bottom of the stack.
pub fn link(f: &mut impl Forest, at: u32, parent: u32, after: Option<u32>) {
    unlink(f, at);
    f.links_mut(at).parent = parent;
    if let Some(prev) = after {
        let next = f.links(prev).next;
        let links = f.links_mut(at);
        links.prev = prev;
        links.next = next;
        f.links_mut(prev).next = at;
        if next != NO_LINK {
            f.links_mut(next).prev = at;
        }
    } else {
        let first = f.links(parent).first;
        f.links_mut(at).next = first;
        if first != NO_LINK {
            f.links_mut(first).prev = at;
        }
        f.links_mut(parent).first = at;
    }
}

/// Cuts `at` out of its parent's children, leaving its own subtree hanging off it.
pub fn unlink(f: &mut impl Forest, at: u32) {
    let held = f.links(at);
    if held.prev != NO_LINK {
        f.links_mut(held.prev).next = held.next;
    } else if held.parent != NO_LINK {
        f.links_mut(held.parent).first = held.next;
    }
    if held.next != NO_LINK {
        f.links_mut(held.next).prev = held.prev;
    }
    // The destroy walk descends from a node already cut out of its parent, so its own
    // children stay reachable from it.
    *f.links_mut(at) = Links {
        first: held.first,
        ..Links::default()
    };
}

/// A node's children, bottom to top — paint order, z-order, and the order a visual
/// collection holds them in.
pub fn children(f: &impl Forest, at: u32) -> impl Iterator<Item = NodeId> + '_ {
    let mut cursor = f.links(at).first;
    core::iter::from_fn(move || {
        if cursor == NO_LINK {
            return None;
        }
        let id = f.id_at(cursor);
        cursor = f.links(cursor).next;
        Some(id)
    })
}

// ── side payloads ───────────────────────────────────────────────────────────────────

/// A dense pool behind a [`NonZeroU32`] head, so a node that carries none of these pays four
/// bytes for the absence rather than the payload.
pub struct Pool<T>(Vec<Option<T>>, Vec<u32>);

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self(Vec::new(), Vec::new())
    }
}

impl<T> Pool<T> {
    pub fn insert(&mut self, value: T) -> NonZeroU32 {
        if let Some(at) = self.1.pop() {
            self.0[at as usize] = Some(value);
            // The head is the slot plus one, which is what makes zero the absence.
            return NonZeroU32::new(at + 1).expect("a slot index plus one is never zero");
        }
        self.0.push(Some(value));
        NonZeroU32::new(self.0.len() as u32).expect("a non-empty pool has a length above zero")
    }

    pub fn remove(&mut self, head: NonZeroU32) -> Option<T> {
        self.1.push(head.get() - 1);
        self.0[head.get() as usize - 1].take()
    }

    #[must_use]
    pub fn get(&self, head: NonZeroU32) -> Option<&T> {
        self.0[head.get() as usize - 1].as_ref()
    }

    pub fn get_mut(&mut self, head: NonZeroU32) -> Option<&mut T> {
        self.0[head.get() as usize - 1].as_mut()
    }
}

/// A node's clip object.
///
/// A rectangle clip carries four animatable sides and eight per-corner radius scalars; a
/// geometric clip carries a shape and nothing animatable.
pub enum ClipObj {
    Bounds(InsetClip),
    Rect(RectangleClip),
    /// Held rather than only set on the visual, so the clip this crate established is
    /// distinguishable from one a shape mask put there.
    Geom(CompositionGeometricClip),
}

/// The off-tree capture a stroked or trimmed shape mask is built from.
///
/// A sprite shape's fill and stroke brushes do not accept a surface brush, so an FP16 colour
/// cannot reach a shape directly and the captured shape carries alpha only.
pub struct ShapeState {
    pub host: ShapeVisual,
    pub shape: CompositionSpriteShape,
    pub geom: CompositionPathGeometry,
    pub capture: Captured,
    pub fitted: Option<Box<FittedShape>>,
}

pub struct FittedShape {
    pub stroke: CompositionPropertySet,
    pub geom: GeomId,
    pub style: Option<StrokeStyle>,
    pub built: Gen,
}

impl ShapeState {
    /// Restates every extent for a `size` DIP box at `scale`.
    ///
    /// `host` is sized `size * scale` and not `size`: a shape visual clips its shapes to its
    /// own size, and the scale lives on the shape because a visual surface captures
    /// *content* and ignores the source visual's own transform. Sized in DIPs, `host` cuts
    /// the figure at `1 / scale` of its extent on both axes. The three are stated together
    /// so none can be restated without the others.
    pub fn resize(&self, size: Vector2, scale: f32) {
        if self.fitted.is_some() { return; }
        self.host.set_size(size.x * scale, size.y * scale);
        self.shape.set_scale(Vector2 { x: scale, y: scale });
        self.capture.resize(size, scale);
    }
}

impl Drop for ShapeState {
    fn drop(&mut self) {
        self.shape.stop_animation("Scale");
        self.host.stop_animation("Size");
        self.capture.surface.stop_animation("SourceSize");
        self.shape.stop_animation("StrokeThickness");
        self.shape.stop_animation("StrokeDashOffset");
        if let Some(fitted) = &self.fitted {
            for property in ["StrokeThickness", "StrokeDashOffset"] {
                fitted.stroke.stop_animation(property);
            }
        }
    }
}

/// The blurred light a node casts past its own silhouette.
///
/// **The blur carries alpha and never colour.** The caster paints the silhouette in the
/// alpha it already has, a compositor-evaluated Gaussian blurs it, and a composite node
/// in the same effect graph multiplies the coverage into an FP16 cell — the multiply the
/// rest of this crate performs with a mask brush happens inside the compositor's float
/// pipeline here, so the halo crosses no 8-bit surface after the blur. The silhouette
/// carries alpha, a float surface carries colour, and the graph multiplies them.
///
/// Its objects are one construction and are built, resized and dropped together.
pub struct GlowState {
    /// The off-tree root the capture reads, in physical pixels.
    ///
    /// A visual surface captures its source's content and not the source's own transform,
    /// so the display scale goes on the caster beneath it, as a shape capture puts it on the
    /// shape.
    pub host: ContainerVisual,
    /// The sprite the blur reads, sized in DIPs, scaled to physical pixels and displaced
    /// by the halo's offset.
    ///
    /// It paints the silhouette alone. The capture has to hold the light's input alone: a
    /// caster that drew anything else would put it, blurred or not, under the real paint.
    pub caster: SpriteVisual,
    /// The halo's alpha, read `bleed` DIPs outside the node's own box on every side.
    pub capture: Captured,
    /// The property set the effect's sigma expression reads: the direct-write frontier
    /// for the blur channel, which an effect brush takes only by animation.
    pub props: CompositionPropertySet,
    /// The sigma expression the construction started on the effect brush. Restated after
    /// every direct write, because a spring that settled displaced it and a settled
    /// spring is stopped before the write lands.
    pub sigma_expr: ExpressionAnimation,
    /// The group a [`Paint::Captured`] paints with, and the silhouette this blur reads.
    /// `None` for a halo, whose silhouette is the node's own brush.
    ///
    /// Held here because every captured paint is a lit one, so it costs nothing on a sprite
    /// that is neither: a capture states its region in the source's own space, and something
    /// has to restate it when the box it stands for moves.
    pub group: Option<Captured>,
    /// The in-tree sprite painting the halo, one child below the node's own paint.
    ///
    /// The tree holds it while it is mounted, so this is the construction staying whole
    /// rather than a second owner: the construction's objects are built, resized and dropped together,
    /// and one of them living only in a child collection is how a glow half-survives an
    /// unlight. Its opacity is the halo's own channel.
    pub sprite: SpriteVisual,
    /// The effect brush painting blurred coverage multiplied by the tint cell. Held so a
    /// re-declared tint re-points one source rather than minting the construction again.
    pub brush: CompositionEffectBrush,
    /// The sprite the node's paint moved to, which is the child above the halo.
    ///
    /// A visual's own brush draws *under* its children, so a node that paints itself and
    /// hosts a halo child would put the halo on top of the paint. A lit node therefore
    /// paints through a child of its own, and an unlit one keeps its brush where it was.
    pub paint: SpriteVisual,
    /// How far past the node's box the capture reads, in DIPs. Fixed at the blur it was
    /// built for, because the region is a property write and the blur is animatable.
    pub bleed: f32,
    /// The halo's displacement from the node's box, in DIPs. The caster carries it scaled
    /// into the capture's physical space, so a re-point restates it rather than animating
    /// it.
    pub offset: Vector2,
}

impl GlowState {
    /// Writes sigma through the frontier the effect's expression reads, and takes the
    /// effect property back: the restart is a no-op while the expression already owns
    /// it, and a spring that owned it was stopped before this write landed.
    pub fn set_sigma(&self, sigma: f32) {
        self.props.insert_scalar("Sigma", sigma);
        self.brush.start_animation("blur.BlurAmount", &self.sigma_expr);
    }

    /// Restates the capture's pixel scale, the caster's displacement and the separately
    /// captured group's extent.
    ///
    /// The three extents are stated here as authored values and not left to the
    /// expressions that track them: the host, the caster and the capture surface sit
    /// off the tree the node's sprite belongs to, and an extent nothing owns statically
    /// reads and renders as zero until an expression lands. The caster's size is DIPs
    /// because its scale carries the pixel factor, the host's is pixels because the
    /// capture reads the source's coordinate space in pixels, and the capture region is
    /// the node grown by the bleed on every side.
    pub fn resize(&self, size: Vector2, scale: f32) {
        let sigma = self.props.scalar("Sigma").unwrap_or(f32::NAN);
        let opacity = self.sprite.opacity();
        let host_size = self.host.size();
        let caster_size = self.caster.size();
        note!(
            "glow",
            "resize size=({:.0},{:.0}) sigma={} opacity={:.2} host=({:.0},{:.0}) caster=({:.0},{:.0})",
            size.x, size.y, sigma, opacity, host_size.x, host_size.y, caster_size.x, caster_size.y
        );
        self.host.set_size(size.x * scale, size.y * scale);
        self.caster.set_size(size.x, size.y);
        self.capture.surface.set_source_size(Vector2 {
            x: (size.x + 2.0 * self.bleed) * scale,
            y: (size.y + 2.0 * self.bleed) * scale,
        });
        self.host.properties().insert_scalar("Dpi", scale);
        self.caster.set_scale(Vector3 {
            x: scale,
            y: scale,
            z: 1.0,
        });
        self.caster.set_offset(
            self.offset.x * scale,
            self.offset.y * scale,
            0.0,
        );
        self.capture.surface.set_source_offset(Vector2::new(-self.bleed * scale, -self.bleed * scale));
        self.capture.brush.set_source_transform(Vector2::zero(), Vector2::new(1.0 / scale, 1.0 / scale));
        if let Some(group) = &self.group {
            group.resize(size, scale);
        }
    }
}

impl Drop for GlowState {
    fn drop(&mut self) {
        self.host.stop_animation("Size");
        self.caster.stop_animation("Size");
        self.capture.surface.stop_animation("SourceSize");
        self.brush.stop_animation("blur.BlurAmount");
    }
}

/// Everything a node may carry beyond its own visual, behind one head.
///
/// The eighteen channels live here rather than in three arrays, so a channel number is
/// absolute across the node and the property table's [`Owner`] selects only the COM object.
#[derive(Default)]
pub struct Aux {
    pub region_view: Option<Box<crate::realize::PresentedView>>,
    pub layout_springs: Vec<(Prop, crate::scene::LayoutSpring)>,
    pub chans: [f32; AUX_CHANS as usize],
    pub clip: Option<ClipObj>,
    pub shape: Option<ShapeState>,
    pub glow: Option<GlowState>,
    /// The last clip declared. A clip is declared rather than diffed, so layout re-states it
    /// on every node it touches and comparing here makes an unchanged re-statement free.
    pub decl: Clip,
}

/// Which of the two constructions realizes a shape mask. Derived, never authored.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Route {
    /// No mask brush: the paint binds directly and a geometric clip carries the shape, with
    /// a soft border mode for an antialiased edge. Four composition objects and one off-tree
    /// render cheaper than the capture.
    #[default]
    Clip,
    /// An off-tree shape visual captured through a visual surface. The only route that can
    /// stroke, trim or dash, because those properties live on a sprite shape.
    Capture,
}

/// The realized brush chain of one sprite, and the declaration it was built from.
///
/// The chain is flat: a mask brush is never the mask or the source of another, which the
/// platform documents as throwing, so a gradient is one premultiplied FP16 strip rather than
/// a nested brush.
pub struct Painted {
    pub mask: Mask,
    pub paint: Paint,
    pub halo: Option<Halo>,
    /// `None` where the mask is [`Mask::None`] or a shape took the clip route: a mask brush
    /// in the chain disqualifies a presented buffer from a display plane.
    pub chain: Option<CompositionMaskBrush>,
    /// The alpha source, as the base brush type: a coverage tile and a shape capture are
    /// surface brushes, and a box cell reaches the slot through a nine-grid, which is not.
    pub alpha: Option<CompositionBrush>,
    /// The box cell this chain was realized against. The nine-grid insets are derived from
    /// it, so a resize compares two integers rather than two floats.
    pub key: Option<BoxKey>,
    pub route: Route,
    /// The generation this chain was realized at, checked against the generations its own
    /// mask and paint read.
    pub built: Gen,
}

impl Painted {
    /// The declaration alone, before anything is realized from it.
    pub fn declared(mask: Mask, paint: Paint, halo: Option<Halo>) -> Self {
        Self {
            mask,
            paint,
            halo,
            chain: None,
            alpha: None,
            key: None,
            route: Route::Clip,
            built: Gen::default(),
        }
    }

    /// Whether the realized chain still matches the generations it was built under.
    #[must_use]
    pub fn fresh(&self, now: Gen) -> bool {
        self.mask
            .deps()
            .union(self.paint.deps())
            .fresh(self.built, now)
    }

    /// True only for a shape on the clip route: a clip the sink declared lives in [`Aux`], so
    /// a route change tears down its own clip and leaves that one standing.
    #[must_use]
    pub fn owns_the_clip(&self) -> bool {
        self.route == Route::Clip && matches!(self.mask, Mask::Shape { .. })
    }
}

// ── the arena ───────────────────────────────────────────────────────────────────────

const F_SPRITE: u16 = 1 << 0;

/// How many bytes one node costs, across every column.
pub const NODE_ROW_BYTES: usize = size_of::<u32>()
    + size_of::<Option<Visual>>()
    + size_of::<Links>()
    + size_of::<[f32; CORE_CHANS as usize]>()
    + size_of::<u64>()
    + size_of::<u16>()
    + 2 * size_of::<Option<NonZeroU32>>();

/// How many bytes a sprite costs: its node row plus the one declaration row it carries.
pub const SPRITE_BYTES: usize = NODE_ROW_BYTES + size_of::<Painted>();

/// The row is eight columns wide and stays that width: a ninth column, or an `Option`
/// without a niche, costs it on every node in the tree.
const _: () = assert!(NODE_ROW_BYTES == 86);
const _: () = assert!(size_of::<Links>() == 16);
const _: () = assert!(size_of::<Option<NonZeroU32>>() == 4);
// A sprite's total: the declaration it was given and the chain realized from it.
const SPRITE_MEASURED: usize = 246;
const _: () = assert!(SPRITE_BYTES == SPRITE_MEASURED);

/// Both kinds of node in eight columns and two side pools.
#[derive(Default)]
pub struct Arena {
    generation: Vec<u32>,
    visual: Vec<Option<Visual>>,
    links: Vec<Links>,
    core: Vec<[f32; CORE_CHANS as usize]>,
    /// Two bits of binding state per scalar channel: twenty-eight channels in fifty-six
    /// bits, so ownership follows a channel even where its setter shares a composite.
    state: Vec<u64>,
    flags: Vec<u16>,
    auxh: Vec<Option<NonZeroU32>>,
    painth: Vec<Option<NonZeroU32>>,
    aux: Pool<Aux>,
    painted: Pool<Painted>,
    /// Groups needing a finite settlement after cancellation, as `(node, group)`.
    pub(crate) restate: Vec<(NodeId, u8)>,
    // Tokens identify individual scalar runs; replacing one axis leaves the other owned.
    settling: Vec<(NodeId, u8, u64)>,
}

impl Forest for Arena {
    fn links(&self, at: u32) -> Links {
        self.links
            .get(at as usize)
            .copied()
            .unwrap_or_else(Links::default)
    }

    fn links_mut(&mut self, at: u32) -> &mut Links {
        &mut self.links[at as usize]
    }

    fn id_at(&self, at: u32) -> NodeId {
        NodeId::raw(at, self.generation[at as usize])
    }
}

impl Arena {
    /// Seats a node's row, minting the declaration row every sprite carries.
    pub fn place(&mut self, id: NodeId, visual: Visual, kind: NodeKind) {
        let at = id.index();
        if self.generation.len() <= at {
            let n = at + 1;
            self.generation.resize(n, 0);
            self.visual.resize_with(n, || None);
            self.links.resize(n, Links::default());
            self.core.resize(n, [0.0; CORE_CHANS as usize]);
            self.state.resize(n, 0);
            self.flags.resize(n, 0);
            self.auxh.resize(n, None);
            self.painth.resize(n, None);
        }
        // A slot reached here occupied is one the app re-declared without destroying, so
        // its side rows are reclaimed rather than inherited by the node taking its place.
        if let Some(head) = self.auxh[at].take() {
            self.aux.remove(head);
        }
        if let Some(head) = self.painth[at].take() {
            self.painted.remove(head);
        }
        self.generation[at] = id.generation();
        self.visual[at] = Some(visual);
        self.links[at] = Links::default();
        // The identity transform: a node never bound keeps the box the compositor gives it,
        // at the origin, unrotated, unscaled and opaque rather than invisible.
        self.core[at] = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        self.state[at] = 0;
        self.flags[at] = if matches!(kind, NodeKind::Sprite) {
            F_SPRITE
        } else {
            0
        };
        if matches!(kind, NodeKind::Sprite) {
            let row = Painted::declared(Mask::None, Paint::None, None);
            self.painth[at] = Some(self.painted.insert(row));
        }
    }

    #[must_use]
    pub fn live(&self, id: NodeId) -> bool {
        !id.is_none()
            && self.generation.get(id.index()) == Some(&id.generation())
            && self.visual[id.index()].is_some()
    }

    #[must_use]
    pub fn visual(&self, id: NodeId) -> Option<&Visual> {
        if self.live(id) {
            self.visual[id.index()].as_ref()
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_sprite(&self, id: NodeId) -> bool {
        self.flags
            .get(id.index())
            .is_some_and(|flags| flags & F_SPRITE != 0)
    }

    /// Frees the row and hands back what the caller must release.
    pub fn free(&mut self, id: NodeId) -> (Option<Aux>, Option<Painted>) {
        self.settling.retain(|&(node, _, _)| node != id);
        let at = id.index();
        self.visual[at] = None;
        self.generation[at] = 0;
        self.flags[at] = 0;
        let aux = self.auxh[at].take().and_then(|head| self.aux.remove(head));
        let painted = self.painth[at]
            .take()
            .and_then(|head| self.painted.remove(head));
        (aux, painted)
    }

    #[must_use]
    pub fn aux(&self, id: NodeId) -> Option<&Aux> {
        self.auxh
            .get(id.index())
            .copied()
            .flatten()
            .and_then(|head| self.aux.get(head))
    }

    /// Whether the node carries a side row at all. A sprite that only ever paints a box
    /// carries none, which is what the head costing four bytes is for.
    #[must_use]
    pub fn has_aux(&self, id: NodeId) -> bool {
        self.auxh.get(id.index()).copied().flatten().is_some()
    }

    /// The node's side row, minting it on first use.
    pub fn aux_mut(&mut self, id: NodeId) -> &mut Aux {
        let at = id.index();
        let head = if let Some(head) = self.auxh[at] {
            head
        } else {
            let head = self.aux.insert(Aux::default());
            self.auxh[at] = Some(head);
            head
        };
        self.aux.get_mut(head).expect("just placed")
    }

    #[must_use]
    pub fn painted(&self, id: NodeId) -> Option<&Painted> {
        self.painth
            .get(id.index())
            .copied()
            .flatten()
            .and_then(|head| self.painted.get(head))
    }

    pub fn painted_mut(&mut self, id: NodeId) -> Option<&mut Painted> {
        let head = self.painth.get(id.index()).copied().flatten()?;
        self.painted.get_mut(head)
    }

    /// The node's own box, in DIPs, from the shadow rather than from the compositor.
    #[must_use]
    pub fn size(&self, id: NodeId) -> Vector2 {
        let core = &self.core[id.index()];
        Vector2 {
            x: core[2],
            y: core[3],
        }
    }

    /// One channel of the node's flat 28-slot shadow.
    #[must_use]
    pub fn chan(&self, id: NodeId, chan: u8) -> f32 {
        if chan < CORE_CHANS {
            self.core[id.index()][chan as usize]
        } else {
            self.aux(id)
                .map_or(0.0, |aux| aux.chans[(chan - CORE_CHANS) as usize])
        }
    }

    pub fn set_chan(&mut self, id: NodeId, chan: u8, value: f32) {
        if chan < CORE_CHANS {
            self.core[id.index()][chan as usize] = value;
        } else {
            self.aux_mut(id).chans[(chan - CORE_CHANS) as usize] = value;
        }
    }

    /// Every live node, in row order.
    pub fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        (0..self.generation.len() as u32)
            .filter(|at| self.visual[*at as usize].is_some())
            .map(|at| self.id_at(at))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.visual.iter().filter(|slot| slot.is_some()).count()
    }
}

// ── the property table ──────────────────────────────────────────────────────────────

/// Which composition object a channel lives on.
///
/// It selects the object and nothing else: the shadow is one flat channel space, so no row
/// needs an owner to find its own slot.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Owner {
    Visual,
    Clip,
    /// The geometry. A trim is the geometry's property and a sprite shape rejects the name
    /// outright, so a trim aimed at the shape is a control that never moves.
    Trim,
    /// The sprite shape, which carries the stroke width and the dash phase.
    Stroke,
    /// The effect brush the glow's sigma animates on. An effect property is reachable
    /// only by animation, so a direct write goes through the property set its expression
    /// reads instead.
    Shadow,
    /// The sprite painting the halo, whose opacity scales the halo alone.
    Glow,
}

/// Describes one animatable channel.
pub struct PropDesc {
    /// The DirectComposition animation name. No rule derives it from the WinRT property.
    pub path: &'static str,
    pub owner: Owner,
    /// The composite this channel belongs to; the writer pushes a whole group at once.
    pub group: u8,
    /// This channel's absolute slot in the node's shadow.
    pub chan: u8,
    /// How many channels the row covers: one for a scalar, two for a composite.
    pub count: u8,
}

impl PropDesc {
    #[must_use]
    pub const fn kind(&self) -> ValueKind {
        if self.count == 2 {
            ValueKind::Vec2
        } else {
            ValueKind::Scalar
        }
    }

    /// How many channels the spring driving this row takes, and so which of the three shared
    /// springs it is.
    ///
    /// A property of the *row*, not of its group. The compositor treats a composite name and
    /// its per-channel names as separate targets of separate types: `Offset` takes a
    /// three-vector and `Offset.X` takes a scalar, and starting an animation whose output
    /// type does not match the property it names fails outright.
    #[must_use]
    pub const fn spring_slot(&self) -> u8 {
        if self.count == 1 {
            1
        } else {
            COMPOSITE_SLOT[self.group as usize]
        }
    }

    /// Whether two rows write any of the same property channels.
    #[must_use]
    pub const fn overlaps(&self, other: &Self) -> bool {
        self.group == other.group
            && self.chan < other.chan + other.count
            && other.chan < self.chan + self.count
    }
}

const fn d(path: &'static str, owner: Owner, group: u8, chan: u8, count: u8) -> PropDesc {
    PropDesc {
        path,
        owner,
        group,
        chan,
        count,
    }
}

use Owner::{Clip as C, Glow as G, Shadow as H, Stroke as S, Trim as T, Visual as V};

/// Positional: a row's place here equals its [`Prop`] discriminant.
pub const PROPS: [PropDesc; 36] = [
    d("Offset", V, 0, 0, 2),
    d("Offset.X", V, 0, 0, 1),
    d("Offset.Y", V, 0, 1, 1),
    d("Size", V, 1, 2, 2),
    d("Size.X", V, 1, 2, 1),
    d("Size.Y", V, 1, 3, 1),
    d("Scale", V, 2, 4, 2),
    d("Scale.X", V, 2, 4, 1),
    d("Scale.Y", V, 2, 5, 1),
    d("RotationAngle", V, 3, 6, 1),
    d("CenterPoint", V, 4, 7, 2),
    d("CenterPoint.X", V, 4, 7, 1),
    d("CenterPoint.Y", V, 4, 8, 1),
    d("Opacity", V, 5, 9, 1),
    d("Left", C, 6, 10, 1),
    d("Top", C, 6, 11, 1),
    d("Right", C, 6, 12, 1),
    d("Bottom", C, 6, 13, 1),
    d("TopLeftRadiusX", C, 7, 14, 1),
    d("TopLeftRadiusY", C, 7, 15, 1),
    d("TopRightRadiusX", C, 7, 16, 1),
    d("TopRightRadiusY", C, 7, 17, 1),
    d("BottomRightRadiusX", C, 7, 18, 1),
    d("BottomRightRadiusY", C, 7, 19, 1),
    d("BottomLeftRadiusX", C, 7, 20, 1),
    d("BottomLeftRadiusY", C, 7, 21, 1),
    d("TrimStart", T, 8, 22, 1),
    d("TrimEnd", T, 8, 23, 1),
    d("StrokeThickness", S, 9, 24, 1),
    d("StrokeDashOffset", S, 10, 25, 1),
    d("blur.BlurAmount", H, 11, 26, 1),
    d("Opacity", G, 12, 27, 1),
    d("AnchorPoint.X", V, 13, 28, 1),
    d("AnchorPoint.Y", V, 13, 29, 1),
    d("TransformMatrix._41", V, 14, 30, 1),
    d("TransformMatrix._42", V, 14, 31, 1),
];

/// How many property groups the rows cover.
pub const GROUP_COUNT: usize = 15;

/// How many channels the *composite* of each group takes: the compositor's offset, scale and
/// centre point are three-vectors and its size is a pair.
///
/// Private, and read only through [`PropDesc::spring_slot`]. A group holds its composite row
/// and its per-channel rows together — `Offset`, `Offset.X` and `Offset.Y` are one group —
/// so the group alone cannot say what type a row's animation takes, and indexing this
/// directly is how a scalar channel comes to be driven by a three-vector spring.
const COMPOSITE_SLOT: [u8; GROUP_COUNT] = [3, 2, 3, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];

/// Every channel's ownership has to fit the one state word.
const _: () = assert!((CORE_CHANS + AUX_CHANS) as usize * 2 <= u64::BITS as usize);

/// A row's animation takes exactly the channels its property does.
///
/// The compositor refuses an animation whose output type is not the property's, and the
/// wrapper unwraps that refusal, so a row driven by the wrong spring is a panic on the scene
/// thread the first time a caller animates it. Checked here rather than against a device,
/// because the table is `const` and the rule is arithmetic over it.
const _: () = {
    let mut at = 0;
    while at < PROPS.len() {
        assert!((PROPS[at].count == 1) == (PROPS[at].spring_slot() == 1));
        at += 1;
    }
};

/// The row describing `prop`.
#[must_use]
pub fn desc(prop: Prop) -> &'static PropDesc {
    &PROPS[prop as usize]
}

/// Records which writer may still own a channel.
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Held {
    /// Nothing owns it: the shadow is authoritative, so a set whose value already matches
    /// writes nothing. That early return is what lets the app emit a subtree undiffed.
    Free = 0,
    /// An animation was stopped without a value being written, so the value the compositor
    /// reached is not knowable and the next set writes even where the shadow matches.
    Stale = 1,
    /// A one-shot animation owns it and finishes on its own. A set stops it, then writes: a
    /// drag snap is a stop and a plain set, never a zero-duration spring.
    Playing = 2,
    /// A tracker expression owns it until the binding is stopped, not until it settles.
    /// Layout re-states an offset on every node it touches, so a set here is refused —
    /// without the refusal the first layout pass after a scroll is wired kills it silently.
    Bound = 3,
}

const HELD: [Held; 4] = [Held::Free, Held::Stale, Held::Playing, Held::Bound];

/// What a bind must do first when the channel's owner object is absent from the node.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Absent {
    /// A clip stands alone, so a bind arriving before any clip was declared mints one and
    /// the two ops may be emitted in either order.
    MintClip,
    /// A trim, width or dash phase lives on a sprite shape, which exists only on the capture
    /// route, so the bind promotes the sprite onto it with the same geometry.
    Promote,
    /// There is no object the channel could be written to.
    Refuse,
}

/// What a bind on `owner` must do when that object is absent from the node.
#[must_use]
pub const fn absent(owner: Owner) -> Absent {
    match owner {
        Owner::Clip => Absent::MintClip,
        Owner::Trim | Owner::Stroke => Absent::Promote,
        // A visual always exists, so its absence is unreachable; a glow has nothing to
        // address before its paint exists.
        Owner::Visual | Owner::Shadow | Owner::Glow => Absent::Refuse,
    }
}

impl Arena {
    /// Whether `owner`'s object exists on the node.
    #[must_use]
    pub fn has_owner(&self, id: NodeId, owner: Owner) -> bool {
        let Some(aux) = self.aux(id) else {
            return owner == Owner::Visual;
        };
        match owner {
            Owner::Visual => true,
            // A geometric clip carries no animatable channel, so it is not an owner.
            Owner::Clip => matches!(aux.clip, Some(ClipObj::Rect(_))),
            Owner::Trim | Owner::Stroke => aux.shape.is_some(),
            Owner::Shadow | Owner::Glow => aux.glow.is_some(),
        }
    }

    /// The strongest ownership of the channels a row addresses, so a composite write is
    /// refused if either axis is bound and a spring on one axis survives a write to the
    /// other.
    #[must_use]
    pub fn held(&self, id: NodeId, desc: &PropDesc) -> Held {
        let word = self.state[id.index()];
        (0..desc.count)
            .map(|at| HELD[(word >> (2 * u32::from(desc.chan + at)) & 3) as usize])
            .max()
            .expect("every row covers at least one channel")
    }

    pub fn set_held(&mut self, id: NodeId, desc: &PropDesc, held: Held) {
        let word = &mut self.state[id.index()];
        for at in 0..desc.count {
            let shift = 2 * u32::from(desc.chan + at);
            *word = (*word & !(3 << shift)) | ((held as u64) << shift);
        }
    }

    /// Starts a finite settlement on a free scalar channel.
    ///
    /// `token` must uniquely identify this run for the lifetime of the arena.
    pub(crate) fn begin_settle(
        &mut self,
        id: NodeId,
        at: u8,
        token: u64,
        animation: &CompositionAnimation,
    ) -> bool {
        let row = &PROPS[at as usize];
        if !self.live(id) || self.held(id, row) != Held::Free
            || self.settling.iter().any(|&(node, prop, _)| node == id && prop == at) {
            return false;
        }
        let Some(object) = self.animatable(id, row.owner) else { return false };
        object.start(row.path, animation);
        self.settling.push((id, at, token));
        true
    }

    pub(crate) fn finish_settle(&mut self, token: u64) -> bool {
        let Some(at) = self.settling.iter().position(|&(_, _, run)| run == token) else {
            return false;
        };
        let (id, prop, _) = self.settling.swap_remove(at);
        let row = &PROPS[prop as usize];
        if !self.live(id) { return false; }
        if let Some(object) = self.animatable(id, row.owner) { object.stop(row.path); }
        self.write_group(id, row.group);
        true
    }

    pub(crate) fn has_settle(&self, token: u64) -> bool {
        self.settling.iter().any(|&(_, _, run)| run == token)
    }

    fn cancel_settles(&mut self, id: NodeId, row: &PropDesc) {
        let mut at = 0;
        while at < self.settling.len() {
            let (node, prop, _) = self.settling[at];
            let held = &PROPS[prop as usize];
            if node != id || !held.overlaps(row) { at += 1; continue; }
            self.settling.swap_remove(at);
            if let Some(object) = self.animatable(id, held.owner) { object.stop(held.path); }
            self.restate.push((id, held.group));
        }
    }

    fn chans_eq(&self, id: NodeId, desc: &PropDesc, value: Value) -> bool {
        match value {
            Value::Scalar(v) => self.chan(id, desc.chan) == v,
            Value::Vec2(v) => {
                self.chan(id, desc.chan) == v.x && self.chan(id, desc.chan + 1) == v.y
            }
        }
    }

    fn write_chans(&mut self, id: NodeId, desc: &PropDesc, value: Value) {
        match value {
            Value::Scalar(v) => self.set_chan(id, desc.chan, v),
            Value::Vec2(v) => {
                self.set_chan(id, desc.chan, v.x);
                self.set_chan(id, desc.chan + 1, v.y);
            }
        }
    }

    /// Writes one channel and reports whether it reached a composition object.
    ///
    /// Every write goes through here — a bound set, a declared clip, a device-loss re-issue,
    /// a snap out of an animation — so [`Held`] is honoured in one place. Returns `false`
    /// when any addressed channel is [`Held::Bound`], when the shadow already holds `value`,
    /// when `value`'s kind does not match the row, and when the node carries no object of
    /// the row's owner.
    pub fn set(&mut self, id: NodeId, prop: Prop, value: Value) -> bool {
        let desc = desc(prop);
        debug_assert_eq!(
            value.kind(),
            desc.kind(),
            "{} takes a different value",
            desc.path
        );
        if value.kind() != desc.kind() || !self.has_owner(id, desc.owner) {
            return false;
        }
        match self.held(id, desc) {
            // A tracker expression owns the channel; a set must not displace it. `held` takes
            // the strongest state over the row's channels, so this covers the composite that
            // contains a bound axis as well as the axis itself.
            Held::Bound => return false,
            // The shadow is authoritative, so an unchanged value stops here.
            Held::Free if self.chans_eq(id, desc, value) => return false,
            // A stopped spring can retain its presentation value. A finite replacement
            // must finish before the channel returns to direct property writes.
            Held::Playing => {
                self.stop_overlapping(id, desc, None);
                self.restate.push((id, desc.group));
            }
            _ => {}
        }
        self.write_chans(id, desc, value);
        self.write_group(id, desc.group);
        self.set_held(id, desc, Held::Free);
        true
    }

    /// Stops every animation whose channels overlap the ones this row names.
    ///
    /// The compositor treats a composite name and its per-channel names as separate targets,
    /// so an animation left on `Offset.X` keeps that channel while a stop aimed at `Offset`
    /// reaches the other; two disjoint channels are left alone.
    fn stop_overlapping(&mut self, id: NodeId, desc: &PropDesc, replacing: Option<&str>) {
        for other in &PROPS {
            let composite = self.flags[id.index()] & (1 << (other.group + 1)) != 0;
            if replacing == Some(other.path)
                || !other.overlaps(desc)
                || (other.count > 1) != composite
                || self.held(id, other) != Held::Playing
            {
                continue;
            }
            if let Some(object) = self.animatable(id, other.owner) {
                object.stop(other.path);
            }
            self.set_held(id, other, Held::Stale);
            self.flags[id.index()] &= !(1 << (other.group + 1));
        }
    }

    /// Starts an animation on the channel and records the state it enters.
    ///
    /// The shadow is moved to `to` because **the shadow is the channel's value, whichever
    /// mechanism carries it**: left behind, the next retarget measures its travel from a
    /// value the channel left long ago, and any later group write pushes the stale value and
    /// undoes the animation. `None` for a tracker expression, which has no target to settle
    /// at.
    pub fn start(
        &mut self,
        id: NodeId,
        desc: &PropDesc,
        animation: &CompositionAnimation,
        to: Option<Value>,
        held: Held,
    ) {
        if self.animatable(id, desc.owner).is_none() {
            return;
        }
        self.cancel_settles(id, desc);
        // Replacing the same path samples its live compositor value. Stopping it first
        // restores the base value; only overlapping aliases need an explicit stop.
        self.stop_overlapping(id, desc, Some(desc.path));
        let Some(object) = self.animatable(id, desc.owner) else {
            return;
        };
        object.start(desc.path, animation);
        if let Some(value) = to {
            self.write_chans(id, desc, value);
        }
        let bit = 1 << (desc.group + 1);
        let flags = &mut self.flags[id.index()];
        *flags = if desc.count > 1 { *flags | bit } else { *flags & !bit };
        self.set_held(id, desc, held);
    }

    /// Stops the animation on the channel and leaves it [`Held::Stale`]: the value the
    /// compositor reached is not knowable, so the next set must write even where the shadow
    /// already matches.
    pub fn stop(&mut self, id: NodeId, desc: &PropDesc) {
        self.cancel_settles(id, desc);
        if let Some(object) = self.animatable(id, desc.owner) {
            object.stop(desc.path);
        }
        self.set_held(id, desc, Held::Stale);
    }

    /// The object an animation for this owner is started on.
    ///
    /// Every owner exposes the same start and stop, so one call site drives a corner radius
    /// on a clip, a trim on a geometry and a blur on a shadow alike.
    #[must_use]
    pub fn animatable(&self, id: NodeId, owner: Owner) -> Option<&dyn AnimatableRef> {
        match owner {
            Owner::Visual => self.visual(id).map(|visual| visual as &dyn AnimatableRef),
            Owner::Clip => match self.aux(id)?.clip.as_ref()? {
                ClipObj::Rect(clip) => Some(clip),
                ClipObj::Geom(_) | ClipObj::Bounds(_) => None,
            },
            // The geometry, not the shape: a trim is the geometry's property.
            Owner::Trim => Some(&self.aux(id)?.shape.as_ref()?.geom),
            Owner::Stroke => {
                let shape = self.aux(id)?.shape.as_ref()?;
                match &shape.fitted {
                    Some(fitted) => Some(&fitted.stroke),
                    None => Some(&shape.shape),
                }
            }
            // The effect brush, whose blur animates as an effect property rather than a
            // visual one.
            Owner::Shadow => Some(&self.aux(id)?.glow.as_ref()?.brush),
            // The halo sprite paints the halo alone, so its opacity scales the halo and
            // nothing else. The state's field is the sprite the tree holds; the property
            // lives on the visual base type.
            Owner::Glow => {
                let glow = self.aux(id)?.glow.as_ref()?;
                let sprite: &Visual = &glow.sprite;
                Some(sprite)
            }
        }
    }

    /// Pushes a whole group from the shadow onto the object that holds it.
    ///
    /// Per group rather than per channel: a vector setter takes every component and the
    /// shadow has them. Does nothing where the group's owner object is absent.
    pub fn write_group(&self, id: NodeId, group: u8) {
        let c = |chan: u8| self.chan(id, chan);
        let v2 = |chan: u8| Vector2 {
            x: c(chan),
            y: c(chan + 1),
        };
        let Some(visual) = self.visual(id) else {
            return;
        };
        let aux = self.aux(id);
        match group {
            14 => visual.set_transform_matrix(windows_numerics::Matrix4x4::translation(c(30), c(31), 0.0)),
            13 => visual.set_anchor_point(v2(28)),
            0 => visual.set_offset(c(0), c(1), 0.0),
            1 => visual.set_size(c(2), c(3)),
            2 => visual.set_scale(Vector3 {
                x: c(4),
                y: c(5),
                z: 1.0,
            }),
            // Radians: `RotationAngle` is the name the animation space carries, and a shadow
            // in degrees over a property in radians is a 57x error that reads as a broken
            // control rather than as a unit bug.
            3 => visual.set_rotation_angle(c(6)),
            4 => visual.set_center_point(v3(v2(7))),
            5 => visual.set_opacity(c(9)),
            // The platform's setter takes all four sides, so one write covers whichever of
            // them changed.
            6 => {
                if let Some(ClipObj::Rect(clip)) = aux.and_then(|aux| aux.clip.as_ref()) {
                    clip.set_sides(c(10), c(11), c(12), c(13));
                }
            }
            // Corner radii animate only as per-channel scalars, so the shadow holds eight
            // and the setter takes four pairs.
            7 => {
                if let Some(ClipObj::Rect(clip)) = aux.and_then(|aux| aux.clip.as_ref()) {
                    clip.set_corner_radii(v2(14), v2(16), v2(18), v2(20));
                }
            }
            8 => {
                if let Some(shape) = aux.and_then(|aux| aux.shape.as_ref()) {
                    shape.geom.as_geometry().set_trim(c(22), c(23));
                }
            }
            9 => {
                if let Some(shape) = aux.and_then(|aux| aux.shape.as_ref()) {
                    match &shape.fitted {
                        Some(fitted) => fitted.stroke.insert_scalar("StrokeThickness", c(24)),
                        None => shape.shape.set_stroke_thickness(c(24)),
                    }
                }
            }
            10 => {
                if let Some(shape) = aux.and_then(|aux| aux.shape.as_ref()) {
                    match &shape.fitted {
                        Some(fitted) => fitted.stroke.insert_scalar("StrokeDashOffset", c(25)),
                        None => shape.shape.set_stroke_dash_offset(c(25)),
                    }
                }
            }
            11 => {
                if let Some(glow) = aux.and_then(|aux| aux.glow.as_ref()) {
                    note!("glow", "id={} sigma write {} lands on the effect property", id.index(), c(26));
                    glow.set_sigma(c(26));
                }
                else {
                    note!("glow", "id={} sigma write {} had no glow to land on", id.index(), c(26));
                }
            }
            12 => {
                if let Some(glow) = aux.and_then(|aux| aux.glow.as_ref()) {
                    note!("glow", "id={} opacity write {} lands on the halo sprite", id.index(), c(27));
                    glow.sprite.set_opacity(c(27));
                }
                else {
                    note!("glow", "id={} opacity write {} had no glow to land on", id.index(), c(27));
                }
            }
            _ => debug_assert!(false, "group {group} has no writer"),
        }
    }
}

/// Starts and stops animations on a composition object.
///
/// An object-safe view of the wrapper's `Animatable`, whose own start is generic over the
/// animation type, so [`Arena::animatable`] can hand back any owner's object behind one
/// type.
pub trait AnimatableRef {
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
    use std::collections::HashMap;

    /// A [`Forest`] belonging to neither half, so the splice is exercised through the trait
    /// rather than through one half's storage.
    #[derive(Default)]
    struct Bare(HashMap<u32, Links>);

    impl Forest for Bare {
        fn links(&self, at: u32) -> Links {
            self.0.get(&at).copied().unwrap_or_default()
        }
        fn links_mut(&mut self, at: u32) -> &mut Links {
            self.0.entry(at).or_default()
        }
        fn id_at(&self, at: u32) -> NodeId {
            NodeId::raw(at, 1)
        }
    }

    impl Bare {
        fn add(&mut self, at: u32) -> u32 {
            self.0.insert(at, Links::default());
            at
        }
    }

    /// `n` children linked bottom-to-top under one parent.
    fn stack(n: u32) -> (Bare, u32, Vec<u32>) {
        let mut f = Bare::default();
        let parent = f.add(0);
        let mut rows = Vec::new();
        let mut top = None;
        for at in 1..=n {
            let row = f.add(at);
            link(&mut f, row, parent, top);
            rows.push(row);
            top = Some(row);
        }
        (f, parent, rows)
    }

    /// The children in order, asserting that the forward and backward walks agree.
    fn order(f: &Bare, parent: u32) -> Vec<u32> {
        let forwards: Vec<_> = children(f, parent).map(|id| id.index() as u32).collect();
        let mut backwards = Vec::new();
        let mut at = forwards.last().copied().unwrap_or(NO_LINK);
        while at != NO_LINK {
            backwards.push(at);
            at = f.links(at).prev;
        }
        backwards.reverse();
        assert_eq!(forwards, backwards, "the chain disagrees with itself");
        assert_eq!(
            f.links(parent).first,
            forwards.first().copied().unwrap_or(NO_LINK)
        );
        forwards
    }

    #[test]
    fn linking_above_each_sibling_in_turn_stacks_them_in_order() {
        let (f, parent, rows) = stack(4);
        assert_eq!(order(&f, parent), rows);
    }

    #[test]
    fn linking_with_no_sibling_goes_to_the_bottom() {
        let (mut f, parent, rows) = stack(3);
        let bottom = f.add(9);
        link(&mut f, bottom, parent, None);
        assert_eq!(order(&f, parent), [bottom, rows[0], rows[1], rows[2]]);
    }

    #[test]
    fn unlinking_holds_the_chain_together_wherever_it_is_cut() {
        // Head, tail and middle take different arms of the splice.
        for cut in 0..4 {
            let (mut f, parent, rows) = stack(4);
            unlink(&mut f, rows[cut]);
            let expected: Vec<_> = rows
                .iter()
                .enumerate()
                .filter(|(at, _)| *at != cut)
                .map(|(_, row)| *row)
                .collect();
            assert_eq!(order(&f, parent), expected, "cutting at {cut}");
            assert_eq!(
                f.links(rows[cut]),
                Links::default(),
                "an unlinked node still points at its old siblings"
            );
        }
    }

    #[test]
    fn unlinking_the_only_child_empties_the_parent() {
        let (mut f, parent, rows) = stack(1);
        unlink(&mut f, rows[0]);
        assert!(order(&f, parent).is_empty());
    }

    #[test]
    fn a_move_is_an_unlink_and_a_link_and_nets_to_a_reorder() {
        let (mut f, parent, rows) = stack(4);
        // Lift the bottom one to the top, which is what a reorder emits.
        link(&mut f, rows[0], parent, Some(rows[3]));
        assert_eq!(order(&f, parent), [rows[1], rows[2], rows[3], rows[0]]);
    }

    #[test]
    fn a_subtree_survives_its_root_being_cut_out() {
        let (mut f, parent, rows) = stack(2);
        let grandchild = f.add(7);
        link(&mut f, grandchild, rows[0], None);
        unlink(&mut f, rows[0]);
        assert_eq!(order(&f, parent), [rows[1]]);
        // The destroy walk descends from a node already cut out of its parent, so its own
        // children have to still be reachable from it.
        assert_eq!(
            children(&f, rows[0])
                .map(|id| id.index() as u32)
                .collect::<Vec<_>>(),
            [grandchild]
        );
    }

    #[test]
    fn a_node_row_and_a_sprite_stay_at_the_width_the_arena_is_sized_for() {
        assert_eq!(NODE_ROW_BYTES, 86);
        assert_eq!(SPRITE_BYTES, SPRITE_MEASURED);
        assert_eq!(size_of::<Painted>(), SPRITE_BYTES - NODE_ROW_BYTES);
        assert_eq!(size_of::<Links>(), 16);
        assert_eq!(size_of::<Option<NonZeroU32>>(), 4);
    }

    #[test]
    fn every_row_sits_at_its_own_discriminant() {
        const ORDER: [Prop; 36] = [
            Prop::Offset,
            Prop::OffsetX,
            Prop::OffsetY,
            Prop::Size,
            Prop::SizeX,
            Prop::SizeY,
            Prop::Scale,
            Prop::ScaleX,
            Prop::ScaleY,
            Prop::RotationAngle,
            Prop::Center,
            Prop::CenterX,
            Prop::CenterY,
            Prop::Opacity,
            Prop::ClipL,
            Prop::ClipT,
            Prop::ClipR,
            Prop::ClipB,
            Prop::CornerTopLeftX,
            Prop::CornerTopLeftY,
            Prop::CornerTopRightX,
            Prop::CornerTopRightY,
            Prop::CornerBottomRightX,
            Prop::CornerBottomRightY,
            Prop::CornerBottomLeftX,
            Prop::CornerBottomLeftY,
            Prop::TrimStart,
            Prop::TrimEnd,
            Prop::StrokeThickness,
            Prop::DashOffset,
            Prop::GlowSigma,
            Prop::GlowOpacity,
            Prop::AnchorX,
            Prop::AnchorY,
            Prop::TranslationX,
            Prop::TranslationY,
        ];
        for (at, prop) in ORDER.iter().enumerate() {
            assert_eq!(
                *prop as usize, at,
                "{prop:?} is declared at row {at} but discriminates to {}",
                *prop as usize
            );
        }
    }

    #[test]
    fn every_row_is_within_the_packed_state_word_and_its_own_group() {
        for row in &PROPS {
            assert!(usize::from(row.group) < GROUP_COUNT, "{}", row.path);
            assert!(
                row.chan + row.count <= CORE_CHANS + AUX_CHANS,
                "{}",
                row.path
            );
            assert!(u32::from(row.chan + row.count) * 2 <= 64, "{}", row.path);
        }
    }

    #[test]
    fn a_group_never_spans_two_owners() {
        let mut owner_of: [Option<Owner>; GROUP_COUNT] = [None; GROUP_COUNT];
        for row in &PROPS {
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
        for row in &PROPS {
            if row.owner == Owner::Clip && row.path.contains("Radius") {
                assert_eq!(row.kind(), ValueKind::Scalar, "{}", row.path);
                assert!(
                    row.path.ends_with('X') || row.path.ends_with('Y'),
                    "{} is not a per-channel radius name",
                    row.path
                );
            }
        }
    }

    #[test]
    fn two_rows_overlap_exactly_where_their_channels_do() {
        for a in &PROPS {
            let a_mask = ((1u64 << (a.count * 2)) - 1) << (2 * u32::from(a.chan));
            for b in &PROPS {
                let b_mask = ((1u64 << (b.count * 2)) - 1) << (2 * u32::from(b.chan));
                let same_object = a.group == b.group;
                assert_eq!(
                    same_object && a_mask & b_mask != 0,
                    a.overlaps(b),
                    "{} / {}",
                    a.path,
                    b.path
                );
            }
        }
    }

    /// Builds a compositor on a queue of its own, or `None` where the session has none.
    fn device() -> Option<(
        windows_composition::DispatcherQueueController,
        windows_composition::Compositor,
    )> {
        let queue = windows_composition::DispatcherQueueController::create_on_current_thread()
            .inspect_err(|_| eprintln!("skipped: no dispatcher queue in this session"))
            .ok()?;
        Some((
            queue,
            windows_composition::Compositor::new().expect("a compositor"),
        ))
    }

    /// Starts and stops every row's animation against a real compositor.
    ///
    /// A path the object rejects and a value type it will not take are the two ways a row
    /// can be wrong, and neither surfaces at any seam - the control simply never moves. Both
    /// are covered here by starting the animation kind the row declares: the wrapper unwraps
    /// the platform's answer, so either failure fails the test.
    #[test]
    fn every_prop_row_animates() {
        use windows_composition::Animation;
        let Some((_queue, comp)) = device() else {
            return;
        };
        // One object per owner, each the type the table says carries that row's channels.
        let sprite = comp.create_sprite_visual();
        let visual: &Visual = &sprite;
        let clip = comp.create_rectangle_clip();
        let geometry = comp.create_ellipse_geometry();
        let shape = comp.create_sprite_shape(&geometry);
        // The blur row's object: an effect brush whose sigma is animatable by name.
        let graph = windows_composition::EffectGraph::GaussianBlur {
            name: "blur",
            sigma: 4.0,
            input: Box::new(windows_composition::EffectGraph::Parameter("s")),
        };
        let factory = comp
            .create_effect_factory(&graph, &["blur.BlurAmount"])
            .expect("the blur graph loads");
        let effect = factory.create_brush();

        for (at, row) in PROPS.iter().enumerate() {
            let target: &dyn AnimatableRef = match row.owner {
                Owner::Visual => visual,
                Owner::Clip => &clip,
                Owner::Trim => &geometry,
                Owner::Stroke => &shape,
                Owner::Shadow => &effect,
                Owner::Glow => visual,
            };
            // The animation kind the row declares: the platform answers a mismatched type
            // and a misspelt name with the same error.
            let slot = row.spring_slot();
            let animation = match slot {
                1 => {
                    let spring = comp.create_spring_scalar_animation();
                    spring.set_final_value(1.0);
                    spring.as_animation()
                }
                2 => {
                    let spring = comp.create_spring_vector2_animation();
                    spring.set_final_value(Vector2 { x: 1.0, y: 1.0 });
                    spring.as_animation()
                }
                _ => {
                    let spring = comp.create_spring_vector3_animation();
                    spring.set_final_value(Vector3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    });
                    spring.as_animation()
                }
            };
            // The wrapper unwraps the platform's answer, so a refused name or type panics
            // inside the call. Caught here so the failure names the row rather than a line
            // of the wrapper.
            let started = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                target.start(row.path, &animation);
                target.stop(row.path);
            }));
            assert!(
                started.is_ok(),
                "row {at} ({:?} {:?}, channels {}..{}) was refused a {}-channel animation",
                row.owner,
                row.path,
                row.chan,
                row.chan + row.count,
                slot
            );
        }
    }

    /// Seats one sprite node and hands back the arena and its id.
    fn one_sprite(comp: &windows_composition::Compositor) -> (Arena, NodeId) {
        let mut arena = Arena::default();
        let id = NodeId::FIRST;
        let sprite = comp.create_sprite_visual();
        arena.place(id, (**sprite).clone(), NodeKind::Sprite);
        (arena, id)
    }

    #[test]
    fn retargeting_a_composite_does_not_stop_its_inactive_scalar_aliases() {
        use windows_composition::Animation;
        let Some((_queue, comp)) = device() else { return };
        let (mut arena, id) = one_sprite(&comp);
        let row = desc(Prop::Size);
        arena.set(id, Prop::Size, Value::Vec2(Vector2::new(200.0, 100.0)));
        let spring = comp.create_spring_vector2_animation();
        for width in [300.0, 180.0, 400.0] {
            let value = Vector2::new(width, 100.0);
            spring.set_final_value(value);
            arena.start(id, row, &spring.as_animation(), Some(Value::Vec2(value)), Held::Playing);
            arena.stop_overlapping(id, row, Some(row.path));
            assert_eq!(arena.held(id, row), Held::Playing);
        }
        arena.stop_overlapping(id, desc(Prop::SizeX), None);
        assert_eq!(arena.held(id, row), Held::Stale);
    }

    /// The four binding states, through the one setter that honours them.
    #[test]
    fn the_setter_honours_every_binding_state_per_channel() {
        use windows_composition::Animation;
        let Some((_queue, comp)) = device() else {
            return;
        };
        let (mut arena, id) = one_sprite(&comp);
        let spring = comp.create_spring_scalar_animation();
        spring.set_final_value(40.0);
        let animation = spring.as_animation();

        for (pair, x, y) in [
            (Prop::Offset, Prop::OffsetX, Prop::OffsetY),
            (Prop::Size, Prop::SizeX, Prop::SizeY),
            (Prop::Scale, Prop::ScaleX, Prop::ScaleY),
            (Prop::Center, Prop::CenterX, Prop::CenterY),
        ] {
            // Playing on one axis: a write to the other leaves it playing.
            arena.start(
                id,
                desc(x),
                &animation,
                Some(Value::Scalar(40.0)),
                Held::Playing,
            );
            assert!(arena.set(id, y, Value::Scalar(12.0)));
            assert_eq!(arena.held(id, desc(x)), Held::Playing);
            assert_eq!(arena.held(id, desc(y)), Held::Free);
            // A set on the playing axis stops it first, then writes.
            assert!(
                arena.set(id, x, Value::Scalar(40.0)),
                "the snap must stop the old spring"
            );
            // Free and equal: the shadow is authoritative, so nothing is written.
            assert!(
                !arena.set(id, x, Value::Scalar(40.0)),
                "an unchanged settled value writes nothing"
            );

            // Bound refuses, on the axis and on the composite that covers it.
            arena.start(id, desc(x), &animation, None, Held::Bound);
            assert!(arena.set(id, y, Value::Scalar(24.0)));
            assert!(!arena.set(id, pair, Value::Vec2(Vector2 { x: 40.0, y: 24.0 })));
            assert_eq!(arena.held(id, desc(x)), Held::Bound);

            // Stale always writes, even where the shadow already matches.
            arena.stop(id, desc(x));
            assert_eq!(arena.held(id, desc(x)), Held::Stale);
            assert!(arena.set(id, x, Value::Scalar(40.0)));
            assert_eq!(arena.held(id, desc(pair)), Held::Free);
        }
    }

    #[test]
    fn a_write_to_an_absent_owner_reaches_nothing() {
        let Some((_queue, comp)) = device() else {
            return;
        };
        let (mut arena, id) = one_sprite(&comp);
        // A clip, a trim and a blur all address an object this node does not carry.
        for prop in [Prop::ClipL, Prop::TrimStart, Prop::GlowSigma] {
            assert!(!arena.has_owner(id, desc(prop).owner));
            assert!(!arena.set(id, prop, Value::Scalar(4.0)));
            assert_eq!(arena.chan(id, desc(prop).chan), 0.0);
        }
        // The visual always exists, so a write to it lands.
        assert!(arena.set(id, Prop::Opacity, Value::Scalar(0.5)));
    }

    #[test]
    fn a_freed_row_reclaims_its_side_pools() {
        let Some((_queue, comp)) = device() else {
            return;
        };
        let (mut arena, id) = one_sprite(&comp);
        arena.aux_mut(id).decl = Clip::Rect {
            l: 0.0,
            t: 0.0,
            r: 1.0,
            b: 1.0,
            radius: Corners::all(2.0),
        };
        assert!(arena.aux(id).is_some());
        assert!(arena.painted(id).is_some());
        assert_eq!(arena.len(), 1);

        let (aux, painted) = arena.free(id);
        assert!(aux.is_some() && painted.is_some());
        assert_eq!(arena.len(), 0);
        assert!(!arena.live(id));
        // The slot is reusable at the next generation, and the freed row leaves nothing.
        let next = NodeId::raw(id.index() as u32, id.generation() + 1);
        let sprite = comp.create_sprite_visual();
        arena.place(next, (**sprite).clone(), NodeKind::Sprite);
        assert!(arena.aux(next).is_none());
        assert_eq!(arena.chan(next, desc(Prop::Opacity).chan), 1.0);
    }

    #[test]
    fn a_bind_on_an_absent_owner_does_what_that_owner_needs() {
        assert_eq!(absent(Owner::Clip), Absent::MintClip);
        assert_eq!(absent(Owner::Trim), Absent::Promote);
        assert_eq!(absent(Owner::Stroke), Absent::Promote);
        assert_eq!(absent(Owner::Visual), Absent::Refuse);
        assert_eq!(absent(Owner::Shadow), Absent::Refuse);
    }
}
