//! The app-side tree, and the one emitter. **App half.**
//!
//! Every app-side act — structure, style, paint, bind, resource — is recorded into the
//! pending patch here, and [`flush`](Model::flush) appends the layout-derived ops and hands
//! the buffer over. One emitter is one ordering authority, which lets the front half apply
//! ops strictly in order and reuse a slot freed earlier in the same patch.
//!
//! Nothing here touches a composition object, and nothing can: the types are ids, plain
//! values and spans.

use crate::env::Env;
use crate::hit_build::{HitBuilder, HitDecl};
use crate::id::{Id, Ids};
use crate::layout::{LayoutKind, LayoutTree, Measure, MeasureCtx, Restyle, Solved};
use crate::patch::{Attach, Op, SinkPatch, Span};
use crate::responsive::Bounds;
use crate::sink::*;
use crate::tree::{self, Forest, Links};
use windows_color::Radiance;
use windows_numerics::Vector2;
use windows_text::GlyphSeg;

/// One node, app-side.
#[derive(Debug, Default)]
struct ModelNode {
    /// The live id of whatever occupies this slot. A placement is emitted from the slot,
    /// not from a walk, and a reused slot must not address itself by the previous
    /// occupant's generation.
    id: NodeId,
    links: Links,
    hit: Option<HitDecl>,
    live: bool,
}

/// The app thread's half of the scene: structure, layout, and the patch.
///
/// `Send` and owns no COM, so all of it — snapping, responsive classification, hit-array
/// construction, the virtualization window — is testable with no device and no compositor.
pub struct Model {
    ids: Ids<Node>,
    res_ids: Ids<()>,
    tracker_ids: Ids<Tracker>,
    delay_ids: Ids<Delay>,
    nodes: Vec<ModelNode>,
    layout: LayoutTree,
    hits: HitBuilder,
    root: GroupId,

    pending: SinkPatch,
    solved: Vec<Solved>,
    previous: Vec<Solved>,
    /// Parents whose child order the layout tree has not been told about yet.
    dirty_children: Vec<NodeId>,
    /// Scratch for one parent's children, reused across every parent in a pass.
    scratch: Vec<NodeId>,
    /// Slot roots, in the order they opened. They occupy the end of the hit array.
    slots: Vec<SlotRootEntry>,

    window: Vector2,
    /// The environment the last solve ran under.
    ///
    /// A **watermark**, not an authority: its only reader is the comparison that decides
    /// whether the pixel grid moved. `None` until the first flush states one.
    env: Option<Env>,
    /// Set by anything that can move a rect. A pass that changed nothing solves nothing.
    solve_dirty: bool,
    /// Set by anything that changes what the hit array *says* without moving a rect — a
    /// widget becoming interactive on hover, an overlay opening.
    ///
    /// Separate from the solve, which is the point: hover flips a flag on one entry, and
    /// one flag would put a full taffy pass behind every pointer move over a control.
    hits_dirty: bool,
}

/// A parentless root that has been minted but not yet placed in the hit array.
///
/// Not `Copy` and not constructible outside this crate: the only source is
/// [`Model::orphan_group`] and the only sink is [`Model::open_slot`], which consumes it.
/// A root that is never opened is unreachable by the disposal walk and leaks its subtree
/// on every unmount, so that mistake is a `must_use` warning at the call site rather than
/// a rule someone else's crate has to lint for.
#[must_use = "a parentless root that is never opened leaks its subtree"]
#[derive(Debug)]
pub struct SlotRoot(GroupId);

#[derive(Copy, Clone, Debug)]
struct SlotRootEntry {
    root: GroupId,
    blocker: Option<crate::hit_build::ControlId>,
    /// Where the layer above placed it, in absolute window DIPs.
    ///
    /// An input to the solve rather than something applied after one: a detached subtree's
    /// rects have to be absolute, because the hit array is one array scanned in one space.
    offset: Vector2,
}

impl Forest for Model {
    fn links(&self, id: NodeId) -> Option<&Links> {
        self.nodes
            .get(id.index())
            .filter(|node| node.id == id)
            .map(|node| &node.links)
    }

    fn links_mut(&mut self, id: NodeId) -> Option<&mut Links> {
        self.nodes
            .get_mut(id.index())
            .filter(|node| node.id == id)
            .map(|node| &mut node.links)
    }
}

impl core::fmt::Debug for Model {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Model")
            .field("nodes", &self.nodes.len())
            .field("pending_ops", &self.pending.len())
            .field("window", &(self.window.x, self.window.y))
            .finish_non_exhaustive()
    }
}

impl Model {
    /// A model with one root group, styled by `root`.
    pub fn new(root: taffy::Style) -> Self {
        let mut model = Self {
            ids: Ids::new(),
            res_ids: Ids::new(),
            tracker_ids: Ids::new(),
            delay_ids: Ids::new(),
            nodes: Vec::new(),
            layout: LayoutTree::new(),
            hits: HitBuilder::default(),
            root: GroupId(NodeId::NONE),
            pending: SinkPatch::new(),
            solved: Vec::new(),
            previous: Vec::new(),
            dirty_children: Vec::new(),
            scratch: Vec::new(),
            slots: Vec::new(),
            window: Vector2 { x: 0.0, y: 0.0 },
            env: None,
            solve_dirty: true,
            hits_dirty: true,
        };
        let id = model.mint(NodeKind::Group, Attach::Window, None);
        model.root = GroupId(id);
        model.layout.set_style(id, &root);
        model
    }

    /// The window subtree's root.
    #[must_use]
    pub const fn root(&self) -> GroupId {
        self.root
    }

    /// Installs what measures content-sized nodes — shaped text, and anything else whose
    /// size this crate cannot know.
    pub fn on_measure(&mut self, measure: impl Measure + 'static) {
        self.layout.on_measure(measure);
    }

    /// Installs what re-lowers a style whose metrics depend on the class in scope.
    ///
    /// Called during the solve for the subtree of a container that just changed class, so the
    /// styles layout runs on are the ones that class implies — and nothing above needs a
    /// second pass to correct them.
    pub fn on_restyle(&mut self, restyle: impl Restyle + 'static) {
        self.layout.on_restyle(restyle);
    }

    /// The window's size in DIPs, as the last [`set_window`](Model::set_window) stated it.
    #[must_use]
    pub const fn window(&self) -> Vector2 {
        self.window
    }

    /// The window's size in DIPs.
    ///
    /// Window size crosses *upward*, written from the window's own resize message. The one
    /// thing that travels against the patch, and a value rather than a channel. The scale
    /// it is solved against is not here — that arrives with [`flush`](Model::flush), so the
    /// grid layout snaps to and the grid the front half rasterizes for cannot disagree.
    pub fn set_window(&mut self, size: Vector2) {
        if self.window != size {
            self.window = size;
            self.solve_dirty = true;
        }
    }

    // ── structure ─────────────────────────────────────────────────────────────────

    /// A group: it positions and clips its children and paints nothing.
    pub fn group(&mut self, parent: GroupId, after: Option<NodeId>) -> GroupId {
        GroupId(self.mint(NodeKind::Group, Attach::Node(parent.0), after))
    }

    /// A sprite: one composition sprite visual on screen.
    pub fn sprite(&mut self, parent: GroupId, after: Option<NodeId>) -> SpriteId {
        SpriteId(self.mint(NodeKind::Sprite, Attach::Node(parent.0), after))
    }

    /// A group with **no parent** — a flyout, a popup, a tooltip, a ghost.
    ///
    /// Every parentless root must be reachable by the disposal walk, or it leaks a subtree
    /// per unmount with nothing to notice. The returned [`SlotRoot`] is what enforces that:
    /// it is `#[must_use]`, and the only thing that accepts one is
    /// [`open_slot`](Model::open_slot), which is what puts it in the array the walk reads.
    /// Opening it twice is inexpressible because opening consumes it.
    pub fn orphan_group(&mut self) -> SlotRoot {
        SlotRoot(GroupId(self.mint(NodeKind::Group, Attach::Detached, None)))
    }

    /// Places a slot root in the hit array, says whether a press outside it dismisses, and
    /// hands back the group to build into.
    ///
    /// Slot roots occupy the **end** of the array, in the order they opened. The array is
    /// the z-order and the scan takes the first hit from the back, so that places every
    /// overlay above what it covers, nests a submenu over its menu, and gives "press
    /// outside dismisses" for free — no capture, no z-index, no case in the router.
    pub fn open_slot(
        &mut self,
        root: SlotRoot,
        blocker: Option<crate::hit_build::ControlId>,
    ) -> GroupId {
        self.slots.push(SlotRootEntry {
            root: root.0,
            blocker,
            offset: Vector2 { x: 0.0, y: 0.0 },
        });
        self.solve_dirty = true;
        self.hits_dirty = true;
        root.0
    }

    /// Places a slot root, in absolute window DIPs, and answers whether it moved.
    ///
    /// The placement is an **input to the next solve**, not a bind emitted beside one: the
    /// solve gathers the detached subtree from here, so its rects are absolute and the hit
    /// array needs nothing said twice. The root's own offset then travels as the ordinary
    /// [`Prop::Offset`] bind every other node's placement does.
    ///
    /// It does not re-lay-out anything — an overlay's size never depends on where it landed,
    /// or the two would be a cycle.
    pub fn place_slot(&mut self, root: GroupId, offset: Vector2) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.root == root) else {
            return false;
        };
        if slot.offset == offset {
            return false;
        }
        slot.offset = offset;
        self.solve_dirty = true;
        true
    }

    /// Removes a slot root from the hit array's tail. Its subtree is destroyed separately.
    pub fn close_slot(&mut self, root: GroupId) {
        self.slots.retain(|slot| slot.root != root);
        self.solve_dirty = true;
        self.hits_dirty = true;
    }

    /// Reparents or reorders a node.
    pub fn place(&mut self, id: NodeId, parent: GroupId, after: Option<NodeId>) {
        if !self.ids.is_live(id) {
            return;
        }
        self.unlink(id);
        self.link(id, parent.0, after);
        self.pending.push_op(Op::Move {
            id,
            parent: parent.0,
            after,
        });
        self.solve_dirty = true;
    }

    /// Destroys a node **and its subtree**.
    ///
    /// One op: the destroy cascades on the far side, which is fewer ops and the only
    /// encoding under which a partial destroy is inexpressible.
    pub fn destroy(&mut self, id: NodeId, exit: Exit) {
        if !self.ids.is_live(id) {
            return;
        }
        self.unlink(id);
        self.pending.push_op(Op::Drop { id, exit });
        self.release_subtree(id);
        self.solve_dirty = true;
    }

    fn mint(&mut self, kind: NodeKind, parent: Attach, after: Option<NodeId>) -> NodeId {
        let id: NodeId = self.ids.mint();
        let index = id.index();
        if index >= self.nodes.len() {
            self.nodes.resize_with(index + 1, ModelNode::default);
        }
        self.nodes[index] = ModelNode {
            id,
            live: true,
            ..ModelNode::default()
        };
        // A reused slot must not be compared against its previous occupant's placement.
        if index < self.previous.len() {
            self.previous[index] = Solved::default();
        }
        self.layout.create(id, LayoutKind::Container);
        // The model's own forest holds only node-to-node edges; the two parentless
        // attachments are the front half's to seat, and carry no parent here.
        self.link(id, parent.node().unwrap_or(NodeId::NONE), after);
        self.pending.push_op(Op::New {
            id,
            kind,
            parent,
            after,
        });
        self.solve_dirty = true;
        id
    }

    fn link(&mut self, id: NodeId, parent: NodeId, after: Option<NodeId>) {
        if parent.is_none() {
            self.nodes[id.index()].links.parent = parent;
            return;
        }
        tree::link(self, id, parent, after);
        self.mark_children_dirty(parent);
    }

    fn unlink(&mut self, id: NodeId) {
        let parent = self.nodes[id.index()].links.parent;
        if parent.is_none() {
            return;
        }
        tree::unlink(self, id);
        self.mark_children_dirty(parent);
    }

    fn mark_children_dirty(&mut self, parent: NodeId) {
        if !parent.is_none() && !self.dirty_children.contains(&parent) {
            self.dirty_children.push(parent);
        }
    }

    /// Releases a subtree's ids without emitting an op per node: the destroy cascades on
    /// the far side, so this only reclaims what the model itself is holding.
    fn release_subtree(&mut self, id: NodeId) {
        let mut child = self.nodes[id.index()].links.first;
        while !child.is_none() {
            let next = self.nodes[child.index()].links.next;
            self.release_subtree(child);
            child = next;
        }
        self.layout.destroy(id);
        self.nodes[id.index()] = ModelNode::default();
        self.ids.release(id);
    }

    // ── declarations ──────────────────────────────────────────────────────────────

    /// Pushes a layout style. Only a style that actually differs reaches the tree.
    pub fn style(&mut self, id: NodeId, style: &taffy::Style) {
        if self.layout.set_style(id, style) {
            self.solve_dirty = true;
        }
    }

    /// Makes a container classify its own inline size for its subtree.
    ///
    /// Declares how the node lays out and nothing else, so it composes in any order with
    /// the style and the children — a node does not lose either by becoming responsive.
    pub fn responsive(&mut self, id: GroupId, bounds: Bounds) {
        if self.layout.set_kind(id.0, LayoutKind::Responsive(bounds)) {
            self.solve_dirty = true;
        }
    }

    /// Hides a node without unmounting it: `Display::None`, so its subtree, its state and
    /// anything half-typed into it survive.
    pub fn hide(&mut self, id: NodeId, hidden: bool) {
        if self.layout.set_hidden(id, hidden) {
            self.solve_dirty = true;
        }
    }

    /// Where a node's intrinsic size comes from.
    pub fn measure(&mut self, id: NodeId, ctx: MeasureCtx) {
        self.layout.set_measure(id, ctx);
        self.solve_dirty = true;
    }

    /// What a node participates in, for the hit array. `None` removes it from routing
    /// entirely.
    pub fn hit(&mut self, id: NodeId, decl: Option<HitDecl>) {
        if let Some(node) = self.nodes.get_mut(id.index())
            && node.hit != decl
        {
            node.hit = decl;
            // A declaration change moves nothing, so this rebuilds the array and does not
            // touch layout — which is the whole cost of a hover flipping `INTERACTIVE`.
            self.hits_dirty = true;
        }
    }

    /// What a node's subtree may draw inside.
    ///
    /// A clip costs no brush slot and no second visual, which is what makes it the only way
    /// to round the corners of a sprite whose one brush is already spent on a mask.
    pub fn clip(&mut self, id: NodeId, clip: Clip) {
        self.pending.push_op(Op::Clip { id, clip });
    }

    /// A sprite's alpha.
    pub fn mask(&mut self, id: SpriteId, mask: Mask) {
        self.pending.push_op(Op::Mask { id, mask });
    }

    /// A sprite's colour, in authored scene light.
    pub fn paint(&mut self, id: SpriteId, paint: Paint) {
        self.pending.push_op(Op::Paint { id, paint });
    }

    /// Binds a channel: a set, an animation, a tracker expression, or a release.
    pub fn bind(&mut self, id: NodeId, prop: Prop, bind: Bind) {
        self.pending.push_op(Op::Bind { id, prop, bind });
    }

    /// Sets a whole transform at once: six binds and, if it has one, a clip.
    pub fn xform(&mut self, id: NodeId, x: &Xform) {
        self.clip(id, x.clip);
        self.bind(id, Prop::Offset, Bind::Set(Value::Vec2(x.offset)));
        self.bind(id, Prop::Size, Bind::Set(Value::Vec2(x.size)));
        self.bind(id, Prop::Scale, Bind::Set(Value::Vec2(x.scale)));
        self.bind(
            id,
            Prop::RotationAngle,
            Bind::Set(Value::Scalar(x.rotation)),
        );
        self.bind(id, Prop::Center, Bind::Set(Value::Vec2(x.center)));
        self.bind(id, Prop::Opacity, Bind::Set(Value::Scalar(x.opacity)));
    }

    // ── payloads ──────────────────────────────────────────────────────────────────

    /// Records a dash pattern and returns the stroke naming it.
    ///
    /// Runs are multiples of the stroke width, as the platform's are — so the same array on
    /// a 1-DIP rule and a 4-DIP rule draws dashes four times as long on the second.
    pub fn stroke(&mut self, width: f32, cap: Cap, join: Join, dashes: &[f32]) -> StrokeStyle {
        StrokeStyle {
            width,
            cap,
            join,
            dashes: self.pending.push_dashes(dashes),
        }
    }

    /// Records key frames and returns the animation naming them.
    pub fn frames(
        &mut self,
        frames: &[(f32, Value, Easing)],
        duration_ms: u32,
        iterations: Iterations,
    ) -> Anim {
        Anim::Frames {
            frames: self.pending.push_frames(frames),
            duration_ms,
            iterations,
        }
    }

    /// Copies a shaped run's fallback segments in and returns the span naming them.
    ///
    /// For a caller holding segments it built elsewhere. A `ShapedRun` appends straight
    /// into [`glyphs`](Model::glyphs) and returns its own span, which is one copy fewer.
    pub fn segments(&mut self, segs: &[GlyphSeg]) -> Span {
        self.pending.push_segs(segs)
    }

    /// The glyph buffers a shaped run is appended into.
    ///
    /// `ShapedRun::segments` writes here and returns the span to hand to [`run`](Model::run),
    /// so a run crosses without an intermediate.
    pub fn glyphs(&mut self) -> &mut windows_text::SegBuffers {
        self.pending.text()
    }

    // ── shared resources ──────────────────────────────────────────────────────────

    /// Mints path geometry, in sprite-local DIPs.
    pub fn geometry(&mut self, verbs: &[PathVerb]) -> GeomId {
        let id: ResId = self.res_ids.mint();
        let span = self.pending.push_verbs(verbs);
        self.pending.push_op(Op::Res {
            id,
            op: ResOp::Geom { verbs: span },
        });
        id.cast()
    }

    /// Re-points geometry. **Every** sprite sharing the id moves together, whichever
    /// construction each one uses, so a curve's fill, stroke and glow cannot diverge.
    pub fn set_geometry(&mut self, id: GeomId, verbs: &[PathVerb]) {
        let span = self.pending.push_verbs(verbs);
        self.pending.push_op(Op::Res {
            id: id.cast(),
            op: ResOp::Geom { verbs: span },
        });
    }

    /// Mints a gradient. Stops are interpolated perceptually where they are rasterized, so
    /// what travels is the authored stops and not a sampled ramp.
    pub fn ramp(&mut self, stops: &[(f32, Radiance)], spread: Spread) -> RampId {
        let id: ResId = self.res_ids.mint();
        let span = self.pending.push_stops(stops);
        self.pending.push_op(Op::Res {
            id,
            op: ResOp::Ramp {
                stops: span,
                spread,
            },
        });
        id.cast()
    }

    /// Re-points a gradient.
    pub fn set_ramp(&mut self, id: RampId, stops: &[(f32, Radiance)], spread: Spread) {
        let span = self.pending.push_stops(stops);
        self.pending.push_op(Op::Res {
            id: id.cast(),
            op: ResOp::Ramp {
                stops: span,
                spread,
            },
        });
    }

    /// Mints a shaped run's coverage tile. `ink` is `ShapedRun::line_ink`, in DIPs.
    pub fn run(&mut self, segs: Span, ink: windows_text::Ink) -> RunId {
        let id: ResId = self.res_ids.mint();
        self.pending.push_op(Op::Res {
            id,
            op: ResOp::Run { segs, ink },
        });
        id.cast()
    }

    /// Re-shapes a run. Changing a line's *text* is structural — reshape, re-rasterize,
    /// re-point — and that is an event-rate operation by construction, which is what the
    /// no-live-retained-path rule requires. Text that changes at display rate belongs in a
    /// presentation region.
    ///
    /// Also the response to [`SceneEvent::ScaleChanged`](crate::SceneEvent): a coverage
    /// tile is the one resource rasterized at device resolution, so it is the one the model
    /// re-emits when the grid moves.
    pub fn set_run(&mut self, id: RunId, segs: Span, ink: windows_text::Ink) {
        self.pending.push_op(Op::Res {
            id: id.cast(),
            op: ResOp::Run { segs, ink },
        });
    }

    /// Declares a region slot. The buffer arrives out of band.
    pub fn region(&mut self) -> RegionId {
        let id: ResId = self.res_ids.mint();
        self.pending.push_op(Op::Res {
            id,
            op: ResOp::Region,
        });
        id.cast()
    }

    /// Releases the model's claim on a resource. Sprites refcount it on the far side, so it
    /// cannot be freed out from under one.
    pub fn release<T>(&mut self, id: Id<T>) {
        let raw: ResId = id.cast();
        if self.res_ids.release(raw) {
            self.pending.push_op(Op::Res {
                id: raw,
                op: ResOp::Drop,
            });
        }
    }

    // ── timed reveals ─────────────────────────────────────────────────────────────

    /// Starts a delay of `ms`, reported back as
    /// [`SceneEvent::DelayElapsed`](crate::SceneEvent::DelayElapsed).
    ///
    /// The only "after N milliseconds" in the system, and it is a monotonic deadline read on
    /// the frame clock rather than a timer — there is no fourth clock. A pending delay holds
    /// the frame clock awake for its own duration, which is bounded and user-initiated, and
    /// that request is its whole cost.
    ///
    /// Re-issuing a live id restarts it, so a tooltip swapping between targets neither
    /// leaks a delay nor re-delays.
    pub fn delay(&mut self, ms: u32) -> DelayId {
        let id: DelayId = self.delay_ids.mint();
        self.pending.push_op(Op::Delay { id, ms });
        id
    }

    /// Cancels a delay by stopping its batch. A cancelled delay never reports.
    pub fn cancel_delay(&mut self, id: DelayId) {
        if self.delay_ids.release(id) {
            self.pending.push_op(Op::CancelDelay { id });
        }
    }

    /// Releases a delay's id once it has reported, without emitting a cancel for a batch
    /// that has already completed.
    pub fn delay_elapsed(&mut self, id: DelayId) {
        _ = self.delay_ids.release(id);
    }

    // ── trackers ──────────────────────────────────────────────────────────────────

    /// Mints the id a tracker will be created under. The tracker itself is a composition
    /// object and is built on the front thread; this reserves the slot both sides name it
    /// by.
    pub fn tracker_id<O: Observe>(&mut self) -> TrackerId<O> {
        TrackerId::new(self.tracker_ids.mint())
    }

    /// Builds the tracker `id` names, sourced from `viewport`.
    ///
    /// **Emit this after the solve that sizes the viewport**, never at mount: the source
    /// takes its hit region from the visual's size at creation, and a zero-size one
    /// hit-tests nothing while reporting success.
    pub fn create_tracker<O: Observe>(&mut self, id: TrackerId<O>, viewport: GroupId, axes: Axes) {
        self.pending.push_op(Op::Tracker {
            id: id.raw,
            op: TrackerOp::Create {
                viewport: viewport.node(),
                axes,
                owned: O::OWNED,
            },
        });
    }

    /// Sets a tracker's bounds. The position may travel outside them during a manipulation
    /// or inertia — that overpan is the bounce, and it is wanted.
    pub fn tracker_bounds<O>(&mut self, id: TrackerId<O>, min: Vector2, max: Vector2) {
        self.pending.push_op(Op::Tracker {
            id: id.raw,
            op: TrackerOp::Bounds { min, max },
        });
    }

    /// Sets how fast a tracker's inertia decays, or restores the system default.
    pub fn tracker_decay<O>(&mut self, id: TrackerId<O>, rate: Option<Vector2>) {
        self.pending.push_op(Op::Tracker {
            id: id.raw,
            op: TrackerOp::Decay(rate),
        });
    }

    /// Destroys a tracker.
    pub fn drop_tracker<O>(&mut self, id: TrackerId<O>) {
        if self.tracker_ids.release(id.raw) {
            self.pending.push_op(Op::Tracker {
                id: id.raw,
                op: TrackerOp::Drop,
            });
        }
    }

    // ── the flush ─────────────────────────────────────────────────────────────────

    /// Solves, snaps, rebuilds the hit array, appends the layout-derived ops, and hands the
    /// buffer over.
    ///
    /// `patch` is expected drained and comes back full; the model keeps the one it was
    /// given, which is the whole of the pooling.
    ///
    /// `env` is stated here rather than pushed in advance, and is the same value
    /// [`Scene::apply`](crate::Scene::apply) is given: every snapped edge in the solve
    /// below lands on the grid the rasters on the other side of the seam are built for,
    /// because there is one number and not two.
    pub fn flush(&mut self, patch: &mut SinkPatch, env: Env) {
        self.solve(env);
        // Stamped with what it was solved under, so the far side can tell geometry
        // snapped to this pixel grid from geometry snapped to another.
        self.pending.env = Some(env);
        core::mem::swap(&mut self.pending, patch);
        self.pending.clear();
    }

    /// Brings the solved layout and the hit array up to date, **without** handing the patch
    /// over.
    ///
    /// Split out of [`flush`](Model::flush) for one caller and one reason: a consumer whose
    /// declarations depend on solved geometry — shaped text, which is laid out at the width
    /// layout gave it — would otherwise have to emit into the *next* patch and arrive a
    /// frame after the box it was measured for. Calling this, reading [`solved`](Model::solved),
    /// declaring, and then flushing puts both in the patch that carries the layout they
    /// agree with.
    ///
    /// Idempotent: a second call with nothing changed does nothing, so `flush` needs no
    /// flag to say whether this already ran.
    pub fn solve(&mut self, env: Env) {
        // A pixel grid that moved re-snaps every edge in the tree, so it is a solve.
        if self.env.replace(env).is_some_and(|last| last != env) {
            self.solve_dirty = true;
        }
        self.push_dirty_children();

        if self.solve_dirty {
            let (window, scale) = (self.window, env.scale());
            self.layout.begin(&mut self.solved);
            let origin = Vector2 { x: 0.0, y: 0.0 };
            self.layout
                .solve_root(self.root.0, window, origin, scale, &mut self.solved);
            // Then every open overlay, each its own root, each measured against the window
            // box and gathered at where it was placed. By index, because the slot list and
            // the layout tree are disjoint fields of the same borrow.
            for index in 0..self.slots.len() {
                let slot = self.slots[index];
                self.layout
                    .solve_root(slot.root.0, window, slot.offset, scale, &mut self.solved);
            }
            // A solve that moved something changes the array too; one that moved nothing
            // leaves it exactly as it was, so the rebuild follows the placements rather
            // than the pass.
            self.hits_dirty |= self.emit_placements();
            self.solve_dirty = false;
        }
        if self.hits_dirty {
            self.build_hits();
            self.hits_dirty = false;
        }
    }

    fn push_dirty_children(&mut self) {
        let mut scratch = core::mem::take(&mut self.scratch);
        for index in 0..self.dirty_children.len() {
            let parent = self.dirty_children[index];
            if !self.ids.is_live(parent) {
                continue;
            }
            scratch.clear();
            let mut child = self.nodes[parent.index()].links.first;
            while !child.is_none() {
                scratch.push(child);
                child = self.nodes[child.index()].links.next;
            }
            self.layout.set_children(parent, &scratch);
        }
        self.dirty_children.clear();
        scratch.clear();
        self.scratch = scratch;
    }

    /// Emits an offset and a size for the nodes that moved, and for no others.
    ///
    /// The front half's idempotent early return is the safety net for this, not the
    /// strategy: leaning on it would put six ops per node per pass on the wire for a pass
    /// in which three nodes moved.
    fn emit_placements(&mut self) -> bool {
        let mut moved = false;
        self.previous.resize(self.solved.len(), Solved::default());
        for index in 0..self.solved.len() {
            let now = self.solved[index];
            if now == self.previous[index] {
                continue;
            }
            self.previous[index] = now;
            let Some(node) = self.nodes.get(index) else {
                continue;
            };
            if !node.live {
                continue;
            }
            moved = true;
            let id = node.id;
            self.pending.push_op(Op::Bind {
                id,
                prop: Prop::Offset,
                bind: Bind::Set(Value::Vec2(now.local)),
            });
            self.pending.push_op(Op::Bind {
                id,
                prop: Prop::Size,
                bind: Bind::Set(Value::Vec2(now.size)),
            });
        }
        moved
    }

    fn build_hits(&mut self) {
        let mut builder = core::mem::take(&mut self.hits);
        {
            let out = self.pending.hits_mut();
            builder.begin(out);
        }
        // The window subtree first, in paint order.
        self.walk_hits(&mut builder, self.root.0, 0);
        // Then every slot root, in the order it opened, each light-dismissing one preceded
        // by its full-window blocker.
        for index in 0..self.slots.len() {
            let slot = self.slots[index];
            if let Some(id) = slot.blocker {
                let out = self.pending.hits_mut();
                builder.blocker(out, id, (self.window.x, self.window.y));
            }
            self.walk_hits(&mut builder, slot.root.0, 0);
        }
        self.hits = builder;

        let entries = Span::new(
            0,
            u32::try_from(self.pending.hits_len()).unwrap_or(u32::MAX),
        );
        self.pending.push_op(Op::Hits { entries });
    }

    fn walk_hits(&mut self, builder: &mut HitBuilder, id: NodeId, depth: usize) {
        if !self.ids.is_live(id) {
            return;
        }
        let (decl, first) = {
            let node = &self.nodes[id.index()];
            (node.hit, node.links.first)
        };
        let solved = self.solved.get(id.index()).copied().unwrap_or_default();
        {
            let out = self.pending.hits_mut();
            builder.push(out, depth, id, &solved, decl);
        }
        let mut child = first;
        while !child.is_none() {
            let next = self.nodes[child.index()].links.next;
            self.walk_hits(builder, child, depth + 1);
            child = next;
        }
    }

    /// The last solved placement of a node, for a caller that needs geometry it just
    /// declared — a tracker's bounds, a thumb's travel.
    #[must_use]
    pub fn solved(&self, id: NodeId) -> Solved {
        self.solved.get(id.index()).copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hit_build::{ControlId, HitFlags};
    use taffy::prelude::{length, percent};
    use windows_color::{DisplayCapability, OutputTransform};

    /// One display, at 96 DPI: every test here is about structure and ordering, and the
    /// environment is an input rather than a variable.
    fn env() -> Env {
        Env::new(
            96.0,
            OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
        )
    }

    fn root_style() -> taffy::Style {
        taffy::Style {
            size: taffy::Size {
                width: percent(1.0_f32),
                height: percent(1.0_f32),
            },
            ..taffy::Style::DEFAULT
        }
    }

    fn box_style(w: f32, h: f32) -> taffy::Style {
        taffy::Style {
            size: taffy::Size {
                width: length(w),
                height: length(h),
            },
            ..taffy::Style::DEFAULT
        }
    }

    #[test]
    fn a_flush_hands_over_the_buffer_and_keeps_a_drained_one() {
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });
        let sprite = model.sprite(model.root(), None);
        model.paint(sprite, Paint::Solid(Radiance::new(1.0, 1.0, 1.0, 1.0)));

        let mut patch = SinkPatch::new();
        model.flush(&mut patch, env());
        assert!(!patch.is_empty());
        assert!(model.pending.is_empty(), "the model kept a drained buffer");

        // A second flush with nothing changed emits nothing.
        let mut second = SinkPatch::new();
        model.flush(&mut second, env());
        assert!(second.is_empty());
    }

    #[test]
    fn a_flush_stamps_the_patch_with_what_it_solved_under() {
        // The stamp is what lets the far side tell geometry snapped to this pixel grid
        // from geometry snapped to another — the `Env` seam's own failure, surviving one
        // level up where the two halves meet.
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });

        let mut patch = SinkPatch::new();
        assert_eq!(
            patch.env(),
            None,
            "an unflushed patch claims no environment"
        );
        model.flush(&mut patch, env());
        assert_eq!(patch.env(), Some(env()));

        // Even a pass that emitted nothing carries it: an output transform that moved
        // without moving a rect still has to reach the front half, and the stamp is how
        // an empty patch says which display it is empty *for*.
        let mut second = SinkPatch::new();
        model.flush(&mut second, env());
        assert!(second.is_empty());
        assert_eq!(second.env(), Some(env()));

        // And it goes with the contents, so a pooled patch cannot claim a stale one.
        second.clear();
        assert_eq!(second.env(), None);
    }

    #[test]
    fn a_patch_solved_under_a_different_environment_is_distinguishable() {
        // What the far side compares against. Two derivations of one fact disagree here
        // and nowhere else — there is no other signal that layout snapped to one grid
        // while the rasters were built for another.
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });

        let hidpi = Env::new(
            192.0,
            OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
        );
        let mut patch = SinkPatch::new();
        model.flush(&mut patch, hidpi);
        assert_ne!(patch.env(), Some(env()));
        assert_eq!(hidpi.scale(), 2.0);
    }

    #[test]
    fn children_are_ordered_bottom_first_and_after_places_above() {
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });
        let root = model.root();
        let a = model.group(root, None);
        let b = model.group(root, Some(a.node()));
        let c = model.group(root, Some(a.node()));

        model.style(a.node(), &box_style(10.0, 10.0));
        model.style(b.node(), &box_style(20.0, 10.0));
        model.style(c.node(), &box_style(30.0, 10.0));

        let mut patch = SinkPatch::new();
        model.flush(&mut patch, env());
        // Paint order is a, c, b — `c` was inserted directly above `a`.
        assert_eq!(model.solved(a.node()).rect.x0, 0.0);
        assert_eq!(model.solved(c.node()).rect.x0, 10.0);
        assert_eq!(model.solved(b.node()).rect.x0, 40.0);
    }

    #[test]
    fn destroying_a_subtree_is_one_op_and_reclaims_every_id() {
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });
        let parent = model.group(model.root(), None);
        let child = model.sprite(parent, None);
        let grandchild = model.sprite(parent, None);

        let mut patch = SinkPatch::new();
        model.flush(&mut patch, env());

        model.destroy(parent.node(), Exit::Fade { ms: 120 });
        model.flush(&mut patch, env());
        let drops = patch
            .ops()
            .iter()
            .filter(|op| matches!(op, Op::Drop { .. }))
            .count();
        assert_eq!(drops, 1, "a subtree destroy is one op, not one per node");

        // Every id came back, so the next mints reuse the slots.
        let reused = model.sprite(model.root(), None);
        assert!(
            [parent.node(), child.node(), grandchild.node()]
                .iter()
                .any(|old| old.index() == reused.node().index())
        );
    }

    #[test]
    fn a_hover_flag_rebuilds_the_array_and_solves_nothing() {
        // The waste this guards: a pointer moving over a control flips `INTERACTIVE`, and
        // folding that into the solve flag would put a full taffy pass and a full placement
        // diff behind every hover.
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });
        let button = model.sprite(model.root(), None);
        model.style(button.node(), &box_style(80.0, 24.0));
        let decl = |flags| {
            Some(HitDecl {
                flags,
                id: ControlId::raw(1, 1),
                touch_inflate: None,
            })
        };
        model.hit(button.node(), decl(HitFlags::INTERACTIVE));

        let mut patch = SinkPatch::new();
        model.flush(&mut patch, env());
        assert!(patch.ops().iter().any(|op| matches!(op, Op::Bind { .. })));

        // Hover: one flag, on one entry.
        model.hit(
            button.node(),
            decl(HitFlags::INTERACTIVE | HitFlags::GESTURE),
        );
        model.flush(&mut patch, env());
        assert!(
            patch.ops().iter().all(|op| matches!(op, Op::Hits { .. })),
            "a hit declaration change emitted more than the array: {:?}",
            patch.ops()
        );

        // And a pass with nothing at all changed emits nothing.
        model.flush(&mut patch, env());
        assert!(patch.is_empty());
    }

    #[test]
    fn the_hit_array_is_paint_order_with_slot_roots_at_the_end() {
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });
        let root = model.root();
        let under = model.sprite(root, None);
        model.style(under.node(), &box_style(100.0, 100.0));
        model.hit(
            under.node(),
            Some(HitDecl {
                flags: HitFlags::INTERACTIVE,
                id: ControlId::raw(1, 1),
                touch_inflate: None,
            }),
        );

        // Opening is what yields the group to build into, so a parentless root cannot be
        // populated and then forgotten.
        let root = model.orphan_group();
        let menu = model.open_slot(root, Some(ControlId::raw(3, 1)));
        model.style(menu.node(), &box_style(80.0, 60.0));
        model.hit(
            menu.node(),
            Some(HitDecl {
                flags: HitFlags::INTERACTIVE,
                id: ControlId::raw(2, 1),
                touch_inflate: None,
            }),
        );

        let mut patch = SinkPatch::new();
        model.flush(&mut patch, env());
        let entries = patch.hit_entries();
        let ids: Vec<usize> = entries.iter().map(|e| e.id.index()).collect();
        assert_eq!(
            ids,
            vec![1, 3, 2],
            "content, then blocker, then the overlay"
        );
        assert!(entries[1].flags.contains(HitFlags::BLOCKER));
    }

    #[test]
    fn a_delay_is_started_once_and_cancelled_once() {
        // A cancel has to be idempotent: the layer above cancels on leave, on press, on
        // `Esc` and on focus moving, and any two of those can arrive for one hover. A
        // second `CancelDelay` would reach the front half naming a batch already gone.
        let mut model = Model::new(root_style());
        let delay = model.delay(400);
        model.cancel_delay(delay);
        model.cancel_delay(delay);

        let mut patch = SinkPatch::new();
        model.flush(&mut patch, env());
        let delays: Vec<Op> = patch
            .ops()
            .iter()
            .filter(|op| matches!(op, Op::Delay { .. } | Op::CancelDelay { .. }))
            .copied()
            .collect();
        assert_eq!(
            delays.len(),
            2,
            "a second cancel is not a second op: {delays:?}"
        );
        assert!(matches!(delays[1], Op::CancelDelay { id } if id == delay));
    }

    #[test]
    fn an_overlay_is_solved_where_it_was_placed() {
        // The gap this closes: a slot root has no parent, so a solve that walked only the
        // window subtree left the whole overlay at `Solved::default()` — no size, no
        // offset op, and a hit entry at the origin with zero area. Visibly on screen and
        // unhittable is the shape that would produce.
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });

        let root = model.orphan_group();
        let menu = model.open_slot(root, None);
        model.style(menu.node(), &box_style(80.0, 60.0));
        model.hit(
            menu.node(),
            Some(HitDecl {
                flags: HitFlags::INTERACTIVE,
                id: ControlId::raw(7, 1),
                touch_inflate: None,
            }),
        );
        let item = model.sprite(menu, None);
        model.style(item.node(), &box_style(80.0, 20.0));

        let mut patch = SinkPatch::new();
        model.flush(&mut patch, env());
        assert_eq!(model.solved(menu.node()).size, Vector2 { x: 80.0, y: 60.0 });

        // Placed: the subtree's rects move with it, absolutely, and the array reads them.
        assert!(model.place_slot(menu, Vector2 { x: 120.0, y: 40.0 }));
        assert!(
            !model.place_slot(menu, Vector2 { x: 120.0, y: 40.0 }),
            "placing where it already is is not a solve"
        );
        model.flush(&mut patch, env());

        assert_eq!(model.solved(menu.node()).rect.x0, 120.0);
        assert_eq!(model.solved(item.node()).rect.y0, 40.0);
        let entry = patch
            .hit_entries()
            .iter()
            .find(|e| e.id == ControlId::raw(7, 1))
            .copied()
            .expect("the overlay declared a hit entry");
        assert_eq!(
            (entry.x0, entry.y0, entry.x1, entry.y1),
            (120.0, 40.0, 200.0, 100.0)
        );

        // And the placement travelled as the ordinary offset bind, with no second mechanism.
        assert!(patch.ops().iter().any(|op| matches!(
            op,
            Op::Bind {
                id,
                prop: Prop::Offset,
                bind: Bind::Set(Value::Vec2(v)),
            } if *id == menu.node() && *v == (Vector2 { x: 120.0, y: 40.0 })
        )));
    }

    #[test]
    fn closing_a_slot_stops_solving_it() {
        // Nothing is retained hidden, so a closed overlay must leave no cost behind — not
        // in the array, and not in the pass either.
        let mut model = Model::new(root_style());
        model.set_window(Vector2 { x: 400.0, y: 300.0 });
        let root = model.orphan_group();
        let menu = model.open_slot(root, Some(ControlId::raw(3, 1)));
        model.style(menu.node(), &box_style(80.0, 60.0));

        let mut patch = SinkPatch::new();
        model.flush(&mut patch, env());
        assert_eq!(patch.hit_entries().len(), 1, "the blocker");

        model.close_slot(menu);
        model.destroy(menu.node(), Exit::None);
        model.flush(&mut patch, env());
        assert!(patch.hit_entries().is_empty());

        // And a pass after it is empty: a closed slot is not solved, so it cannot keep
        // reporting that something moved.
        model.flush(&mut patch, env());
        assert!(patch.is_empty());
    }
}
