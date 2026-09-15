//! Walks a built element into the model. The only module in this crate that names `Model`.
//!
//! The arena is post-order — a child finishes before the call consuming it — and the model
//! needs pre-order, so this walks down from the root and emits as it goes. `after` is the
//! previous sibling, which is what makes child order paint order.
//!
//! A slot with exactly one sprite and no children is that sprite. Anything richer mints a
//! group whose chrome is absolute at inset zero, ahead of the laid-out children: chrome
//! paints under content, and since `border` is never set, inset zero covers the node rather
//! than the space inside a border.
//!
//! Nothing holds a borrow across a re-entry. `Effect::new` runs its closure immediately and
//! that closure borrows the host, so the arena is taken out of its thread-local for the walk
//! and every model call takes a fresh borrow.

use super::arena::{Act, Build, ChanSource, HaloSeed, MaskSeed, NIL, Part, Slot, SpriteSeed};
use super::host::{ControlRow, Host, MountRow};
use super::style::{Declaration, Recipe};
use super::{El, Site, View};
use crate::gesture::GestureDecl;
use crate::layout::{Edge, Layout, Len, Position, Preset};
use crate::role::{DataRole, Metric, Role, Scope, Silhouette};
use crate::widget::{
    Chrome, Flow, ModelState, Motion, RoleSet, StatePolicy, TextSource, UiaRole, Wash,
};
use core::cell::RefCell;
use windows_color::Radiance;
use windows_numerics::Vector2;
use windows_scene::taffy;
use windows_scene::{
    Anim, Bind, Cap, ControlId, Corners, Exit, GeomId, GroupId, Halo, HitDecl, HitFlags, Join,
    Mask, MeasureCtx, MeasureKey, NodeId, Paint, PathVerb, Prop, RampId, Spread, SpriteId, Tuning,
    Value,
};

/// Mints path geometry from `verbs`, in sprite-local DIPs.
///
/// The author of the verbs re-points them through [`set_geometry`] when the box changes.
/// This layer cannot do it for them: the verbs are in the sprite's own space, so at a new
/// size they are different verbs and only their author knows which — a response curve, a
/// knob arc and a routing wire each have a shape that depends on the width.
#[must_use]
pub fn geometry(verbs: &[PathVerb]) -> GeomId {
    let lease = Host::with(|h| super::geometry::Lease(h.model().geometry(verbs), h.identity));
    let id = lease.0;
    crate::signal::Owner::retain(lease);
    id
}

/// Returns the window's own scope: the palette's root, at the process polarity and the
/// accent and density the application installed.
///
/// What geometry authored in DIPs resolves its own metrics against. A sprite's verbs are
/// sprite-local DIPs and no container states them, so a wire's lane spacing cannot arrive as
/// a [`Len`](crate::layout::Len) — it is resolved here, from the same palette every
/// container reads.
///
/// It carries the **root** width class. A consumer drawing against a probed box narrows it
/// with [`Scope::at_width`](crate::role::Scope::at_width) and the class that box reported, so
/// both halves of one row come out at one density.
#[must_use]
pub fn root_scope() -> Scope {
    Host::with_output(|h| h.root_scope)
}

/// Re-points the geometry `id` names. Every sprite sharing the id moves together, whichever
/// construction each one uses, so a curve's fill, stroke and glow cannot diverge.
pub fn set_geometry(id: GeomId, verbs: &[PathVerb]) {
    Host::with_output(|h| h.model().set_geometry(id, verbs));
}

/// Mints the sink a presentation region's sprite paints. The buffer arrives out of band,
/// from the present thread, so the slot exists before anything fills it.
pub(crate) fn region_sink() -> windows_scene::RegionId {
    Host::with(|h| h.model().region())
}

/// One stop of a gradient: where it sits, in which role, and how much of that role.
///
/// A [`DataRole`] and not a [`Role`](crate::role::Role), because a gradient is a resource
/// rather than a sprite inside a tree and so has no scope to resolve against. A data role is
/// the one kind that needs none.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Stop {
    /// Where the stop sits along the ramp, `0..=1`.
    pub at: f32,
    /// The chromatic role this stop paints.
    pub role: DataRole,
    /// How much of that role, as an alpha in `0..=1`.
    pub strength: f32,
}

/// Mints a gradient over `stops`, spread across whatever box paints with it.
///
/// The stops are resolved here, so no colour reaches the caller — the same rule a sprite's
/// role follows, at the one place a resource rather than a node carries it.
///
/// The ramp rasterizes to a strip that is stretched to fill, so it costs nothing on a resize
/// and one id serves every box that shares its stops.
#[must_use]
pub fn ramp(stops: &[Stop], spread: Spread) -> RampId {
    let id = with_resolved(stops, |resolved| {
        Host::with(|h| h.model().ramp(resolved, spread))
    });
    let lease = Host::with(|h| {
        h.ramps.place(id, (stops.to_vec(), spread));
        super::geometry::Lease(id, h.identity)
    });
    struct RampLease(super::geometry::Lease<windows_scene::Ramp>);
    impl Drop for RampLease {
        fn drop(&mut self) {
            Host::try_with(|h| {
                if h.identity == self.0.1 {
                    h.ramps.take(self.0.0);
                }
            });
        }
    }
    crate::signal::Owner::retain(RampLease(lease));
    id
}

/// Re-points the gradient `id` names. Every sprite painting with it changes together.
pub fn set_ramp(id: RampId, stops: &[Stop], spread: Spread) {
    Host::with_output(|h| {
        if let Some((held, axis)) = h.ramps.get_mut(id) {
            held.clear();
            held.extend_from_slice(stops);
            *axis = spread;
        }
    });
    with_resolved(stops, |resolved| {
        Host::with_output(|h| h.model().set_ramp(id, resolved, spread));
    });
}

/// Resolves `stops` into a scratch buffer and hands it to `f`.
///
/// One buffer per thread, reused: minting a ramp is an event-rate operation, and a `Vec` per
/// call would put an allocation on the path a chain of rows takes when its document arrives.
fn with_resolved<T>(stops: &[Stop], f: impl FnOnce(&[(f32, Radiance)]) -> T) -> T {
    thread_local! {
        static SCRATCH: RefCell<Vec<(f32, Radiance)>> = const { RefCell::new(Vec::new()) };
    }
    SCRATCH.with(|scratch| {
        let mut resolved = scratch.borrow_mut();
        resolved.clear();
        resolved.extend(stops.iter().map(|stop| {
            let light = crate::role::data(stop.role, root_scope());
            (stop.at, light.with_alpha(light.a * stop.strength))
        }));
        f(&resolved)
    })
}

/// A hover wash's opacity, and a press's.
///
/// Opacities of a derived wash rather than colours, so they are held here and not in the
/// palette.
const HOVER_ALPHA: f32 = 0.06;
const PRESS_ALPHA: f32 = 0.12;

/// A thumb's resting opacity: ink at a fraction, an opacity over whatever it sits on rather
/// than a colour of its own.
pub(crate) const THUMB_ALPHA: f32 = 0.30;

/// Owns a mounted subtree and unmounts it on drop.
///
/// Dropping destroys the node and releases every table row the walk claimed: the style
/// recipes, the control rows, the measured runs. A live handle is what keeps the subtree on
/// screen, so a subtree cannot be left mounted with nothing holding it.
///
/// It does not own the effects the mount installed. Those belong to whatever
/// [`Owner`](crate::signal::Owner) was current — an application scope, a keyed row's, a
/// branch arm's — each of which disposes them as it drops this handle.
#[must_use = "dropping a mount unmounts its subtree immediately"]
#[derive(Debug)]
pub struct Mount {
    node: NodeId,
    exit: Exit,
    /// The head of this subtree's chain through the mount table. A chain and not a `Vec`,
    /// so realizing a list row during a fling allocates nothing.
    rows: NodeId,
}

impl Mount {
    pub(super) fn new(node: NodeId, rows: NodeId) -> Self {
        Self {
            node,
            rows,
            exit: Exit::None,
        }
    }

    pub(crate) fn set_exit(&mut self, exit: Exit) {
        self.exit = exit;
    }

    /// Returns the node this subtree is rooted at.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// Gives up the right to unmount, for a root that lives as long as the process.
    ///
    /// The subtree's table rows stay claimed for the life of the thread.
    pub fn leak(self) -> NodeId {
        let node = self.node;
        core::mem::forget(self);
        node
    }
}

impl Drop for Mount {
    fn drop(&mut self) {
        // Non-panicking: a drop during teardown can run after the host has gone, and a
        // panic in a drop takes the process with it.
        Host::try_with(|h| h.unmount(self.node, self.exit, self.rows));
    }
}

/// Mounts a built element under `parent`.
pub fn mount<K>(el: El<K>, parent: GroupId) -> Mount {
    let scope = Host::with(|h| h.root_scope);
    mount_at(el.erase(), parent, None, scope)
}

/// Mounts `el` under `parent`, after the sibling `after`, at `scope`. What a structural
/// adapter calls for each row or arm it realizes.
///
/// # Panics
///
/// Panics if `el` was built before an earlier mount: the arena is cleared after each mount,
/// so the slot the element indexes is gone.
pub fn mount_at(el: View, parent: GroupId, after: Option<NodeId>, scope: Scope) -> Mount {
    mount_scoped(el, parent, after, scope, None)
}

pub(crate) fn mount_scoped(
    el: View,
    parent: GroupId,
    after: Option<NodeId>,
    scope: Scope,
    hover_scope: Option<ControlId>,
) -> Mount {
    let mut build = Build::take();
    // The one place a stale element can be named, so the message names the call site that
    // held the `El` across a mount rather than leaving a raw bounds panic in the arena.
    let exit = build
        .nodes
        .get(el.at as usize)
        .expect("this element was built before an earlier mount and does not survive it")
        .exit;
    let mut rows = Rows::default();
    let node = walk(
        &mut build,
        Where {
            hover_scope,
            ..Where::new(el.at, parent, after, scope)
        },
        &mut rows,
        &mut Claim::default(),
    );
    build.restore();
    Mount {
        node,
        exit,
        rows: rows.head,
    }
}

/// Carries what a subtree hands up to the control enclosing it.
///
/// A control's moving part is rarely its own sprite — a slider's knob, a toggle's knob and a
/// meter's level are all children of the control they belong to — so the parts a control
/// needs are collected on the way back up rather than read off its own seed list. Read off
/// the seed list instead, a `ChromeRow` carries `thumb: None` for every control that has one
/// and the front thread computes a value it cannot show.
#[derive(Default)]
struct Claim {
    scalar_parts: [Option<(NodeId, crate::widget::ScalarPart)>; 4],
    thumb: Option<SpriteId>,
    trail: Option<(SpriteId, f32)>,
    /// The first label sprite this subtree minted, which the enclosing control repaints when
    /// its chrome row changes.
    ///
    /// Collected on the way back up for the same reason the thumb and the accessible name
    /// are: a control's label is rarely its own sprite. Read off the control's own row
    /// instead, `ControlRow::label` is `None` for every button in the tree and the row's
    /// text colour reaches nothing.
    label: Option<SpriteId>,
    /// The first text this subtree laid out, which an enclosing control derives its
    /// accessible name from.
    ///
    /// Collected on the way back up for the same reason the thumb is: a control's label is
    /// rarely its own sprite — `button` is a control with a text child — so reading the
    /// control's own row instead leaves every button in the stack unnamed.
    text: Option<MeasureKey>,
}

impl Claim {
    /// Takes what a subtree offered, without displacing what this node already found.
    fn part(&mut self, part: (NodeId, crate::widget::ScalarPart)) {
        *self
            .scalar_parts
            .iter_mut()
            .find(|p| p.is_none())
            .expect("a scalar control supports at most four parts") = Some(part);
    }
    fn absorb(&mut self, inner: Self) {
        for part in inner.scalar_parts.into_iter().flatten() {
            self.part(part);
        }

        self.thumb = self.thumb.or(inner.thumb);
        self.trail = self.trail.or(inner.trail);
        self.label = self.label.or(inner.label);
        self.text = self.text.or(inner.text);
    }
}

/// Names where one node in the walk goes: which slot, under which parent, after which
/// sibling, at which scope.
#[derive(Copy, Clone)]
struct Where {
    at: u32,
    parent: GroupId,
    after: Option<NodeId>,
    scope: Scope,
    hover_scope: Option<ControlId>,
}

impl Where {
    const fn new(at: u32, parent: GroupId, after: Option<NodeId>, scope: Scope) -> Self {
        Self {
            at,
            parent,
            after,
            scope,
            hover_scope: None,
        }
    }
}

/// The chain of mount-table rows one walk claimed, threaded as it goes.
#[derive(Default)]
struct Rows {
    head: NodeId,
    tail: NodeId,
}

impl Rows {
    fn push(&mut self, at: NodeId) {
        if self.head.is_none() {
            self.head = at;
        } else {
            Host::with(|h| {
                if let Some(row) = h.mounts.get_mut(self.tail) {
                    row.next = at;
                }
            });
        }
        self.tail = at;
    }
}

fn bind(node: NodeId, update: impl FnMut() + 'static) {
    Host::with(|h| {
        h.binding(node, update);
    });
}

/// Emits one slot and its subtree, and returns the node it minted.
///
/// `rows` collects every mount row the whole walk claimed, which is what the unmount
/// releases. `claim` receives the parts this subtree did not consume itself.
fn walk(b: &mut Build, at: Where, rows: &mut Rows, claim: &mut Claim) -> NodeId {
    let slot = b.nodes[at.at as usize];
    let scope = at.scope.in_theme(Host::with(|h| h.root_scope));
    let inner = slot.elevate.map_or(scope, |e| scope.elevate(e));
    let roles = slot.chrome.map(|chrome| chrome.roles);

    // Counted through the chain rather than collected: a `Vec` of seeds here is one
    // allocation per node per mount, on the path a list row realized during a fling takes.
    // The seeds are `Copy`, so the chain answers every question a collection would have.
    let seed_count = b.seed_count(slot.seeds);
    // Chrome is a table row and not sprites yet, so a variant with no fill costs one visual
    // fewer rather than one invisible one.
    //
    // Counted over the states this node can reach, not over its resting row alone.
    // `ModelState::Selected` supplies a fill whatever the row says, so a ghost control that
    // can be selected needs the sprite the row itself does not ask for — without it,
    // selection has nowhere to paint and the control looks identical in both states. A node
    // that never declares selection still costs the resting row's sprites and no more.
    let selects = selectable(b, &slot);
    let chrome = slot.chrome;
    let chrome_count = chrome_seeds(roles, chrome, inner, selects).count();
    // Wrapped and trimmed runs take the group's allocated width; their glyph tiles keep
    // their own coverage extents inside it.
    let run = run_seed(b, &slot);
    let grouped =
        run.is_some_and(|(text, flow)| flow != Flow::Line || b.texts[text as usize].vertical);
    let leaf = seed_count == 1
        && chrome_count == 0
        && slot.kids.len == 0
        && slot.adapter.is_none()
        && slot.responsive.is_none()
        && slot.state == StatePolicy::None
        && !grouped;

    // Collected locally, then either consumed by this node — if it is a control — or handed
    // up. Two nested controls therefore cannot claim one thumb.
    let mut own_claim = Claim::default();
    let mut parts = Parts::default();
    let (node, group) = if leaf {
        let id = Host::with(|h| h.model().sprite(at.parent, at.after));
        let seed = *b
            .chain_seeds(slot.seeds)
            .next()
            .expect("a leaf is its one sprite");
        emit_sprite(id, &seed, slot.geom, inner, roles);
        parts.set(seed.part, id, &mut own_claim);
        (id.node(), None)
    } else {
        let id = Host::with(|h| h.model().group(at.parent, at.after));
        (id.node(), Some(id))
    };

    // ── style, and the recipe that can re-lower it ────────────────────────────────
    // The scope is stored class-free: `at.scope` carries the class in force where this node
    // was built, and the solve supplies the current one through the restyle seam.
    //
    // An adapter's node is an anchor rather than a box. Its rows are the enclosing
    // container's children, and this node exists only to give the position they insert at an
    // identity, so it takes one const style and carries no recipe; `restyle` answers `None`
    // for it and leaves it alone, saving a `lower()` and a table entry per list.
    //
    // `Display::None` in the style rather than `Model::hide`, which is reversible without
    // knowing a node's display and keeps a hidden node as one of its parent's flex items —
    // a column would gap around the anchor and sit one gap short of its box. `Display::None`
    // takes it out of the item list, and an anchor is never revealed.
    let style = if slot.adapter.is_some() {
        Some(taffy::Style {
            display: taffy::Display::None,
            ..taffy::Style::DEFAULT
        })
    } else {
        let recipe = Recipe {
            preset: slot.preset,
            scope,
            layout: core::mem::take(&mut b.layouts[at.at as usize]),
        };
        let style = recipe.lower(scope.width);
        Host::with(|h| h.styles.place(node, recipe));
        Some(style)
    };
    let row = Host::with(|h| {
        if let Some(style) = &style {
            h.model().style(node, style);
        }
        let row = h.mint_mount(MountRow::new(node));
        if let Some(cell) = slot.probe {
            h.mint_probe(
                row,
                crate::layout::ProbeRow {
                    node,
                    cell,
                    scope: inner,
                },
            );
        }
        row
    });
    rows.push(row);
    if leaf {
        Host::with(|h| {
            if let Some(paint) = h.appearances.get(node) {
                let id = paint.id;
                h.own_appearance(id, row, None);
            }
        });
    }

    // ── the node's own sprites, where it is not one itself ────────────────────────
    let mut previous: Option<NodeId> = None;
    if let Some(group) = group {
        for (part, seed) in chrome_seeds(roles, slot.chrome, inner, selects) {
            let sprite = Host::with(|h| h.model().sprite(group, previous));
            cover_chrome(sprite.node(), inner, part, slot.chrome);
            emit_sprite(sprite, &seed, None, inner, roles);
            Host::with(|h| h.own_appearance(sprite, row, slot.chrome));
            parts.set(part, sprite, &mut own_claim);
            previous = Some(sprite.node());
        }
        // A wrapping run has no sprite of its own: its lines are minted as they are shaped.
        if !grouped {
            for &seed in b.chain_seeds(slot.seeds) {
                let sprite = Host::with(|h| h.model().sprite(group, previous));
                cover(sprite.node(), inner, Len::Zero);
                emit_sprite(sprite, &seed, slot.geom, inner, roles);
                Host::with(|h| h.own_appearance(sprite, row, None));
                parts.set(seed.part, sprite, &mut own_claim);
                previous = Some(sprite.node());
            }
        }
    }

    // ── interaction chrome ────────────────────────────────────────────────────────
    if let StatePolicy::Wash { hover, .. } = slot.state {
        let group = group.expect("a control with a wash is never a bare sprite");
        let sprite = Host::with(|h| h.model().sprite(group, previous));
        cover(sprite.node(), inner, Len::Zero);
        emit_wash(
            sprite,
            hover,
            inner,
            surface_corners(radius_of(b, &slot, inner), slot.chrome),
        );
        Host::with(|h| {
            h.own_appearance(sprite, row, slot.chrome);
            h.appearances.get_mut(sprite.node()).unwrap().wash = true;
        });
        previous = Some(sprite.node());
        parts.wash = Some(sprite);
    }

    // ── the halo, cast by the fill ────────────────────────────────────────────────
    // After the fill exists, because the silhouette is what that sprite paints.
    if let Some(seed) = slot.halo {
        let silhouette = if parts.fill.is_some() {
            Silhouette::Area
        } else {
            Silhouette::Ink
        };
        let source = parts.fill.or(parts.label).or(parts.border);
        if let HaloSeed::Reactive(index) = seed {
            if let Some(read) = b.halo_roles[index as usize].take() {
                bind(node, move || {
                    mount_halo(HaloSeed::Glow(read()), source, inner, silhouette)
                });
            }
        } else {
            mount_halo(seed, source, inner, silhouette);
        }
    }

    // ── styles that follow a value ────────────────────────────────────────────────
    // Its own pass over the act chain, taking only its own variants: a spacer has a style
    // that moves and no hit entry at all, so this cannot be folded into the control pass.
    mount_style_acts(b, &slot, node, row);

    // ── channels: one reactive lowering ───────────────────────────────────────────
    mount_channels(b, &slot, node, parts.fill, inner);

    // ── measured text ─────────────────────────────────────────────────────────────
    if let Some((text, _)) = run {
        let target = if grouped {
            None
        } else {
            Some(parts.label.expect("a run seed mints its own sprite"))
        };
        let key = mount_text(b, node, group, target, text, inner, roles, row);
        // Set before the children walk, so this node's own text wins over anything they
        // offer: `absorb` keeps the value already present.
        own_claim.text.get_or_insert(key);
    }

    // The observer's identity is needed by children before its control parts are complete.
    let observer = slot
        .hover_scope
        .map(|_| Host::with(|h| h.reserve_control()));
    let hover_scope = observer.or(at.hover_scope);

    // ── children ──────────────────────────────────────────────────────────────────
    if slot.kids.len > 0 {
        let group = group.expect("a node with children is a group");
        for index in 0..slot.kids.len {
            let kid = b.kids[(slot.kids.at + index) as usize];
            previous = Some(walk(
                b,
                Where {
                    hover_scope,
                    ..Where::new(kid, group, previous, inner)
                },
                rows,
                &mut own_claim,
            ));
        }
    }

    if let Some(part) = slot.scalar_part {
        let mut channel = slot.chans.head;
        while channel != NIL {
            let entry = &b.chans[channel as usize];
            assert!(
                entry.prop != part.property(),
                "a scalar part cannot also bind its driven property"
            );
            channel = entry.next;
        }
        own_claim.part((node, part));
    }

    // ── the control row, once the walk has found the parts it names ───────────────
    // After the children, because a control's moving part is one of them. Nothing above
    // depends on the row existing, and the hit array is a declaration rather than an order.
    if slot.hit.is_some() {
        mount_control(
            b,
            &slot,
            node,
            group,
            parts,
            own_claim,
            inner,
            row,
            observer,
            hover_scope,
        );
    } else {
        // Not a control, so what the subtree offered belongs to whichever control encloses
        // this node.
        claim.absorb(own_claim);
    }

    if let Some(decl) = slot.scroll {
        let group = group.expect("a scroll container is a group");
        let content = previous.expect("a scroll container has a content group");
        mount_scroll(group, content, decl, inner, row);
    }

    // After the sprite that paints it exists, so a region is never registered for a node the
    // walk went on to fail to give one. Taken out of the arena, so the builder reaches the
    // present thread exactly once.
    if let Some(at) = slot.region
        && let Some(build) = b.regions[at as usize].build.take()
    {
        let seed = &b.regions[at as usize];
        let (sink, key, queue, live) = (seed.sink, seed.key, seed.queue, seed.live.clone());
        Host::with(|h| {
            // After `mount_control` above, so the row already names the control the region's
            // hit entry minted — which is the id every pointer report about this region
            // carries, and the only handle the picking path has to find it by.
            let control = h.mounts.get(row).and_then(|row| row.control);
            h.mint_region(
                row,
                crate::present::RegionRow {
                    theme: std::sync::Arc::new(crate::present::Published::new(inner)),
                    node,
                    sink,
                    key,
                    queue,
                    live,
                    control,
                    build: Some(build),
                    extent: None,
                },
            );
        });
    }

    if let Some(bounds) = slot.responsive {
        let group = group.expect("a responsive container is a group");
        Host::with(|h| h.model().responsive(group, bounds));
    }

    if let Some(index) = slot.geometry_job {
        if let Some(draw) = b.geometry_jobs[index as usize].take() {
            super::geometry::mount(node, inner, draw);
        }
    }

    // Last, and outside every borrow: an adapter builds application views, so it runs where
    // a nested `Build::with` is legal. By here this node's own slot is finished with.
    if let Some(adapter) = slot.adapter
        && let Some(install) = b.adapters[adapter as usize].install.take()
    {
        // The enclosing container and this node's own position in it, not the group minted
        // for this slot: rows and arms are laid out by the container the list was passed to,
        // and this node is only the anchor they insert after. `at.scope` rather than `inner`
        // for the same reason — an adapter pushes no scope, so the two are equal in every
        // case an adapter can be in.
        install(Site {
            parent: at.parent,
            after: Some(node),
            scope: at.scope,
            hover_scope,
        });
    }

    node
}

/// Returns the slot's text seed and how it flows, or `None` where it carries no run.
fn run_seed(b: &Build, slot: &Slot) -> Option<(u32, Flow)> {
    b.chain_seeds(slot.seeds).find_map(|s| match s.mask {
        MaskSeed::Run { text } => Some((text, b.texts[text as usize].flow)),
        _ => None,
    })
}

/// Casts `seed`'s halo from the sprite carrying the node's own ink: its fill, or its glyphs
/// where it paints no fill.
///
/// The fallback is what lets a label glow. A run's coverage tile *is* an alpha profile, so
/// the compositor derives a halo from a word the same way it derives one from a box, and a
/// lit legend needs no second mechanism.
///
/// A node with neither has no silhouette to cast, and a halo declared on one is an authoring
/// mistake rather than a no-op: the modifier is on a variant that paints nothing.
fn mount_halo(seed: HaloSeed, source: Option<SpriteId>, scope: Scope, silhouette: Silhouette) {
    let Some(fill) = source else {
        debug_assert!(false, "a halo was declared on a node that paints nothing");
        return;
    };
    Host::with(|h| {
        let scope = scope.in_theme(h.root_scope);
        if let Some(recipe) = h.appearances.get_mut(fill.node()) {
            recipe.halo = Some((seed, silhouette));
        }
        emit_halo(h, seed, fill, scope, silhouette);
    });
}

pub(super) fn emit_halo(
    h: &mut Host,
    seed: HaloSeed,
    fill: SpriteId,
    scope: Scope,
    silhouette: Silhouette,
) {
    let paint = scope.for_paint();
    let halo = match seed {
        HaloSeed::Reactive(_) => unreachable!("reactive halo role is resolved by its effect"),
        HaloSeed::Glow(role) => {
            let halo = halo_of(
                crate::role::emission(role, paint),
                silhouette,
                crate::role::resolve(role, paint),
            );
            debug_assert!(
                halo.is_some(),
                "a halo was declared in a role the palette gives no light"
            );
            halo
        }
        HaloSeed::Shadow(edge) => {
            let shadow = crate::role::shadow(paint);
            let offset = match edge {
                Edge::Left => Vector2 {
                    x: -shadow.offset,
                    y: 0.0,
                },
                Edge::Right => Vector2 {
                    x: shadow.offset,
                    y: 0.0,
                },
                Edge::Top => Vector2 {
                    x: 0.0,
                    y: -shadow.offset,
                },
                Edge::Bottom => Vector2 {
                    x: 0.0,
                    y: shadow.offset,
                },
            };
            Some(Halo {
                blur: shadow.blur,
                tint: shadow.tint,
                offset,
            })
        }
    };
    h.model().halo(fill, halo);
}

/// Which sprite plays which part, so a state change re-paints exactly what changed.
#[derive(Copy, Clone, Debug, Default)]
struct Parts {
    fill: Option<SpriteId>,
    label: Option<SpriteId>,
    border: Option<SpriteId>,
    wash: Option<SpriteId>,
}

impl Parts {
    /// Records which sprite plays `part`.
    ///
    /// A thumb goes to `claim` rather than into [`Parts`]: the control that owns a moving
    /// part is the one enclosing the sprite, not the node that minted it.
    fn set(&mut self, part: Part, id: SpriteId, claim: &mut Claim) {
        match part {
            Part::Fill => self.fill = Some(id),
            // Recorded here *and* offered upward. This node needs it to place its own
            // glyphs; the control enclosing it needs it to repaint the run when its chrome
            // row moves, and for `button` and `segmented` alike that control is the parent.
            Part::Label => {
                self.label = Some(id);
                claim.label = claim.label.or(Some(id));
            }
            Part::Border => self.border = Some(id),
            Part::Wash => self.wash = Some(id),
            Part::Thumb => claim.thumb = claim.thumb.or(Some(id)),
            Part::Trail { origin } => claim.trail = Some((id, origin)),
            Part::Static => {}
        }
    }
}

/// Returns the sprites a chrome row expands to, bottom first.
///
/// Outline coverage uses a cached nine-grid with a transparent center. Parts needed by
/// reachable states are allocated once, transparent until that state becomes active.
pub(super) fn chrome_seeds(
    roles: Option<RoleSet>,
    chrome: Option<Chrome>,
    scope: Scope,
    selectable: bool,
) -> impl Iterator<Item = (Part, SpriteSeed)> {
    let radius = chrome.map_or(0.0, |c| crate::role::metric(c.radius, scope));
    let width = crate::role::metric(Metric::HairlineW, scope);
    let selected = selectable
        .then(|| chrome.map(|c| c.in_state(ModelState::Selected)))
        .flatten()
        .or_else(|| {
            selectable
                .then(|| roles.map(|r| r.in_state(ModelState::Selected)))
                .flatten()
        });
    let disabled = chrome.and_then(|c| c.disabled);
    let stroke = roles
        .and_then(|r| r.stroke)
        .or_else(|| selected.and_then(|r| r.stroke))
        .or_else(|| disabled.and_then(|r| r.stroke));
    [Part::Border, Part::Fill]
        .into_iter()
        .filter_map(move |part| {
            let role = |row: RoleSet| match part {
                Part::Border => row.stroke.map(Role::Stroke),
                _ => row.fill.map(Role::Fill),
            };
            let rest = roles.and_then(role);
            let paint = rest
                .or_else(|| selected.and_then(role))
                .or_else(|| disabled.and_then(role))?;
            let mask = if part == Part::Border {
                MaskSeed::Outline {
                    radius: surface_corners(radius, chrome),
                    width,
                    open: chrome.and_then(|c| c.attached).map(|edge| match edge {
                        Edge::Left => windows_scene::Side::Left,
                        Edge::Top => windows_scene::Side::Top,
                        Edge::Right => windows_scene::Side::Right,
                        Edge::Bottom => windows_scene::Side::Bottom,
                    }),
                }
            } else {
                MaskSeed::Radius {
                    dips: surface_corners(
                        (radius - if stroke.is_some() { width } else { 0.0 }).max(0.0),
                        chrome,
                    ),
                }
            };
            Some((
                part,
                SpriteSeed {
                    strength: f32::from(rest.is_some()),
                    ..SpriteSeed::new(mask, paint, part)
                },
            ))
        })
}

/// The attached side shares its neighbour's edge, including the interaction wash.
pub(super) fn surface_corners(radius: f32, chrome: Option<Chrome>) -> Corners {
    let mut corners = Corners::all(radius);
    match chrome.and_then(|c| c.attached) {
        Some(Edge::Left) => {
            corners.tl = 0.0;
            corners.bl = 0.0;
        }
        Some(Edge::Right) => {
            corners.tr = 0.0;
            corners.br = 0.0;
        }
        Some(Edge::Top) => {
            corners.tl = 0.0;
            corners.tr = 0.0;
        }
        Some(Edge::Bottom) => {
            corners.bl = 0.0;
            corners.br = 0.0;
        }
        None => {}
    }
    corners
}

/// Extends the fill to the attached edge so no border remains against its neighbour.
fn cover_chrome(node: NodeId, scope: Scope, part: Part, chrome: Option<Chrome>) {
    let inset = if part == Part::Fill {
        Len::Metric(Metric::HairlineW)
    } else {
        Len::Zero
    };
    let mut insets = [inset; 4];
    if let Some(edge) = chrome.and_then(|c| c.attached) {
        insets[match edge {
            Edge::Left => 0,
            Edge::Right => 1,
            Edge::Top => 2,
            Edge::Bottom => 3,
        }] = Len::Zero;
    }
    let recipe = Recipe {
        preset: Preset::Bare,
        scope,
        layout: Declaration {
            base: Layout {
                position: Some(Position::Absolute(insets)),
                ..Layout::default()
            },
            ..Declaration::default()
        },
    };
    let style = recipe.lower(scope.width);
    Host::with(|h| {
        h.styles.place(node, recipe);
        h.model().style(node, &style);
    });
}

/// Positions derived paint within its owner's box.
fn cover(node: NodeId, scope: Scope, inset: Len) {
    let layout = Layout {
        position: Some(Position::Absolute([inset; 4])),
        ..Layout::default()
    };
    Host::with(|h| {
        h.model()
            .style(node, &layout.lower(Preset::Bare, None, scope))
    });
}

/// Resolves one sprite's mask and paint and writes both to the model.
///
/// The only caller of [`role::resolve`](crate::role::resolve) in this layer: neither
/// `Radiance` nor [`Paint`] is reachable from a widget, so a widget cannot accept a colour.
/// `for_paint` pins the scope's width axis, so a resize cannot re-key a single cell.
fn emit_sprite(
    id: SpriteId,
    seed: &SpriteSeed,
    geom: Option<GeomId>,
    scope: Scope,
    roles: Option<RoleSet>,
) {
    Host::with(|h| {
        let scope = scope.in_theme(h.root_scope);
        let prior = h.appearances.get(id.node());
        let source = match (seed.source, seed.part, roles) {
            (super::theme::PaintSource::Role(_), Part::Label, Some(roles)) => {
                super::theme::PaintSource::Role(Role::Text(roles.text))
            }
            (source, _, _) => source,
        };
        let appearance = super::theme::Appearance {
            id,
            mask: seed.mask,
            source,
            part: seed.part,
            strength: seed.strength,
            geom,
            scope,
            next: prior.map_or(NodeId::NONE, |p| p.next),
            chrome: prior.and_then(|p| p.chrome),
            halo: prior.and_then(|p| p.halo),
            wash: prior.is_some_and(|p| p.wash),
        };
        h.appearances.place(id.node(), appearance);
        appearance.publish(h, true);
    });
}

pub(super) fn emit_mask(
    h: &mut Host,
    id: SpriteId,
    mask: MaskSeed,
    geom: Option<GeomId>,
    scope: Scope,
) {
    let mask = match mask {
        MaskSeed::Box { radius } => Mask::Box {
            radius: Corners::all(radius.and_then(|r| r.dips(scope)).unwrap_or(0.0)),
        },
        MaskSeed::Radius { dips } => Mask::Box { radius: dips },
        MaskSeed::Outline {
            radius,
            width,
            open,
        } => Mask::Outline {
            radius,
            width,
            open,
        },
        MaskSeed::Border { radius, width } => Mask::Outline {
            radius: Corners::all(crate::role::metric(radius, scope)),
            width: width.dips(scope).unwrap_or(0.0),
            open: None,
        },
        // A run's coverage tile is minted when its text is shaped, which cannot happen
        // until layout has said how wide it is. Until then the sprite draws nothing.
        MaskSeed::Run { .. } | MaskSeed::Bare => Mask::None,
        MaskSeed::Shape { stroke } => Mask::Shape {
            geom: geom.unwrap_or_default(),
            stroke: stroke
                .and_then(|w| w.dips(scope))
                .map(|width| h.model().stroke(width, Cap::Round, Join::Round, &[])),
        },
    };
    h.model().mask(id, mask);
}

/// Returns the halo `emission` states for a sprite painting `light`, or `None` where the role
/// spends none.
///
/// The tint is the sprite's own resolved light at the emission's strength, not a colour of
/// its own: a role emits *itself*, and a second authored colour here would be a way for a
/// halo to disagree with the thing casting it.
pub(super) fn halo_of(
    emission: crate::role::Emission,
    of: Silhouette,
    light: Radiance,
) -> Option<Halo> {
    let spend = emission.of(of);
    spend.is_lit().then(|| Halo {
        blur: spend.sigma,
        tint: light.with_alpha(light.a * spend.strength),
        offset: Vector2 { x: 0.0, y: 0.0 },
    })
}

/// Returns whether this node ever resolves in [`ModelState::Selected`].
///
/// Read off the act chain rather than off a flag on the slot: `El::selected` is the one
/// declaration that reaches this state, and it records an act.
fn selectable(b: &Build, slot: &Slot) -> bool {
    let mut at = slot.acts.head;
    while at != NIL {
        let entry = &b.acts[at as usize];
        if matches!(entry.act, Some(Act::SelectedWhen(_))) {
            return true;
        }
        at = entry.next;
    }
    false
}

/// Emits the wash sprite a hover or a press fades in.
///
/// The paint is the wash at full strength and the opacity carries the alpha, so hover and
/// press share one channel and one spring rather than two colours. A colour animation is not
/// available: a sprite's colour is an FP16 cell, a composition colour brush is 8-bit, and no
/// brush interpolates between two FP16 sources.
fn emit_wash(id: SpriteId, wash: Wash, scope: Scope, radius: Corners) {
    let role = match wash {
        Wash::Ink => Role::Text(crate::role::Text::Primary),
        Wash::Accent => Role::Fill(crate::role::Fill::Accent),
    };
    emit_sprite(
        id,
        &SpriteSeed::new(MaskSeed::Radius { dips: radius }, role, Part::Static),
        None,
        scope,
        None,
    );
    Host::with(|h| {
        // Parked at zero with a `Set` and not a spring: a control that has never been
        // hovered must not play an animation to arrive at invisible.
        h.model()
            .bind(id.node(), Prop::Opacity, Bind::Set(Value::Scalar(0.0)));
    });
}

/// Returns the radius a wash matches, taken from the chrome row or from the fill seed, so a
/// pill's wash is a pill and a card's is a card with nothing declared twice.
fn radius_of(b: &Build, slot: &Slot, scope: Scope) -> f32 {
    if let Some(chrome) = slot.chrome {
        return crate::role::metric(chrome.radius, scope);
    }
    b.chain_seeds(slot.seeds)
        .find(|s| s.part == Part::Fill)
        .and_then(|s| match s.mask {
            MaskSeed::Box { radius } => radius.and_then(|r| r.dips(scope)),
            MaskSeed::Radius { dips } => Some(dips.max()),
            _ => None,
        })
        .unwrap_or(0.0)
}

/// Moves the slot's actions into the host's control table and declares the node to the hit
/// array.
///
/// The handlers move rather than being cloned or borrowed. They live on this thread for the
/// node's lifetime and reach the front thread only as a presence bit in [`HitFlags`], which
/// is what keeps `SinkPatch: Send`.
fn mount_control(
    b: &mut Build,
    slot: &Slot,
    node: NodeId,
    group: Option<GroupId>,
    parts: Parts,
    claim: Claim,
    scope: Scope,
    row: NodeId,
    observer: Option<ControlId>,
    hover_scope: Option<ControlId>,
) -> Option<ControlId> {
    let hit = slot.hit?;

    // This node's own run where it has one, otherwise the first its subtree offered.
    let label = parts.label.or(claim.label);
    let mut control = ControlRow {
        fill: parts.fill,
        label,
        border: parts.border,
        front: crate::widget::ChromeRow {
            wash: parts.wash,
            hover_scope,
            hover: HOVER_ALPHA,
            press: PRESS_ALPHA,
            scalar_parts: claim.scalar_parts,
            thumb: claim.thumb.map(SpriteId::node),
            trail: claim.trail.map(|(id, origin)| (id.node(), origin)),
            drive: slot.interaction,
            ..Default::default()
        },
        chrome: slot.chrome,
        hovered: slot.hover_scope,
        uia: slot.uia,
        name: slot.name,
        text: claim.text,
        key: slot.key,
        ..ControlRow::new(node, scope)
    };

    let mut validation = None;
    let mut scalar_source = None;
    let mut field_source = None;
    let mut text_commit = None;
    let mut disabled = None;
    let mut selected = None;
    let mut at = slot.acts.head;
    while at != NIL {
        let entry = &mut b.acts[at as usize];
        at = entry.next;
        match entry.act.take() {
            Some(Act::Validation(f)) => validation = Some(f),
            Some(Act::FieldSource(s)) => field_source = Some(s),
            Some(Act::CommitText(f)) => text_commit = Some(f),
            Some(Act::Click(f)) => control.click = Some(f),
            Some(Act::ChangeF64(f)) => control.change = Some(f),
            Some(Act::Cancel(f)) => control.cancel = Some(f),
            Some(Act::ScalarSource(f)) => scalar_source = Some(f),
            Some(Act::CommitF64(f)) => control.commit = Some(f),
            Some(Act::Drag(f)) => {
                control.drag = Some(f);
                control.front.drags = true;
            }
            Some(Act::Tip(t, side)) => control.tip = Some((std::rc::Rc::new(t), side)),
            Some(Act::Flyout(f)) => control.flyout = Some(f),
            Some(Act::DisabledWhen(f)) => disabled = Some(f),
            Some(Act::SelectedWhen(f)) => selected = Some(f),
            Some(Act::HideWhen(_) | Act::Restyle(..) | Act::Escape(_) | Act::Popup { .. })
            | None => {}
        }
    }

    // Folded in here, so declining an inflation is order-independent at the call site and
    // cannot make a target of a node that declared none.
    let flags = hit.flags | uia_flag(slot.uia) | inflate_flag(slot.no_inflate);
    let inflate = hit.inflate.and_then(|l| l.dips(scope));
    // A control that refined nothing still declares the default set — tap, right-tap and
    // hold — which is what gives a touch user the context menu a mouse user reaches with the
    // secondary button. Gated on `HitFlags::GESTURE`, so a node that declared no gesture gets
    // no recogniser; this walk also runs for nodes that exist only for automation.
    let gesture = b
        .gesture(slot.gesture)
        .or_else(|| flags.contains(HitFlags::GESTURE).then(GestureDecl::default));
    let caption = slot.caption;
    let id = Host::with(move |h| {
        let id = if let Some(id) = observer {
            h.place_control(id, control);
            id
        } else {
            h.mint_control(control)
        };
        if let Some(row) = h.mounts.get_mut(row) {
            row.control = Some(id);
        }
        h.claim_control_paint(id);
        // The front thread's half, shipped as numbers and ids: its own copy stays here so a
        // solve that changed this control's room can re-send a corrected one.
        if let Some(control) = h.control_mut(id) {
            control.front.id = id;
            let front = control.front;
            h.chrome.push(front);
        }
        h.model().hit(
            node,
            Some(HitDecl {
                flags,
                id,
                touch_inflate: inflate,
            }),
        );
        if let Some(decl) = gesture {
            h.gestures.push((id, decl));
        }
        if let Some(button) = caption {
            h.caption.set(button, id);
        }
        id
    });

    if let Some(read) = validation {
        bind(node, move || {
            let message = read();
            Host::with(|h| {
                if let Some(control) = h.control_mut(id) {
                    control.validation = message;
                }
                h.uia_restale();
            });
        });
    }
    if let Some(source) = scalar_source {
        bind(node, move || {
            let (fraction, epoch) = source();
            Host::with(|h| h.publish_fraction(id, fraction, epoch));
        });
    }

    if let (Some(source), Some(group), Some(key), Some(input_scope)) =
        (field_source, group, claim.text, slot.field_scope)
    {
        Host::with(|h| h.install_field(id, group, key, input_scope, scope, text_commit));
        let mut scratch = String::new();
        bind(node, move || {
            scratch.clear();
            source.append(&mut scratch);
            Host::with(|h| h.field_source(id, &scratch));
        });
    }

    // One derived state: disablement takes precedence, regardless of effect order.
    if disabled.is_some() || selected.is_some() {
        bind(node, move || {
            let off = disabled.as_ref().is_some_and(|read| read());
            let on = selected.as_ref().is_some_and(|read| read());
            Host::with(|h| {
                h.model().hit(
                    node,
                    Some(HitDecl {
                        flags: if off { uia_only(flags) } else { flags },
                        id,
                        touch_inflate: inflate,
                    }),
                );
                h.set_state(
                    id,
                    if off {
                        Some(ModelState::Disabled)
                    } else if on {
                        Some(ModelState::Selected)
                    } else {
                        None
                    },
                );
            });
        });
    }
    Some(id)
}

/// Delegates a container's scrolling to a tracker, and gives it a thumb.
///
/// Two bindings on one tracker: the content rides it negated, since a tracker's position
/// increases for up and left, and the thumb rides the same tracker at the ratio of the two
/// extents, so it follows the content with no front-thread work at all. The tracker is a
/// composition object, so it is named here and created on the front thread.
fn mount_scroll(
    viewport: GroupId,
    content: NodeId,
    decl: crate::layout::ScrollDecl,
    scope: Scope,
    row: NodeId,
) {
    let reveal = decl.reveal;
    Host::with(|h| {
        let tracker = h.model().tracker_id::<windows_scene::Observed>();
        h.trackers.push(super::host::TrackerSpec {
            id: tracker,
            viewport,
            content,
            axes: windows_scene::Axes::VERTICAL,
        });
        // The scrollbar lives in the viewport rather than in the content, so it does not
        // scroll with what it reports on, and above the content, because child order is paint
        // order and the order the hit array is scanned in. Below it, the bar paints under
        // whatever the list draws and a grab resolves to the row behind it.
        //
        // The rail is static geometry and carries the hit target; the thumb is moved by the
        // compositor and carries none. A hit entry on the thumb would name a rect the solve
        // fixed and the tracker then moved away from.
        let bar = (reveal != crate::layout::Reveal::Never).then(|| {
            let rail = h.model().group(viewport, Some(content));
            h.model().style(rail.node(), &crate::layout::rail_style());
            let thumb = h.model().sprite(rail, None);
            h.model().mask(
                thumb,
                Mask::Box {
                    radius: Corners::all(crate::layout::THUMB_W * 0.5),
                },
            );
            h.model().paint(
                thumb,
                Paint::Solid(crate::role::ink(THUMB_ALPHA, scope.for_paint())),
            );
            // Hidden from the mount rather than shown and faded out: a surface whose content
            // fits never overflows, and a thumb visible for one frame to say so is a flash on
            // every screen that opens.
            if reveal == crate::layout::Reveal::OnDemand {
                h.model()
                    .bind(thumb.node(), Prop::Opacity, Bind::Set(Value::Scalar(0.0)));
            }
            // The rail's control carries a hit entry and a drag and no chrome row: the
            // thumb's opacity belongs to the reveal policy, and a row the front table adopted
            // would give that channel two owners. The hit entry itself is written by
            // `publish_scrolls`, because whether the rail is a target at all depends on
            // whether there is anything to scroll, which is a solve output.
            let id = h.mint_control(ControlRow::new(rail.node(), scope));
            h.gestures.push((id, crate::layout::grab_decl()));
            (rail, thumb, id)
        });
        let control = h.mounts.get(row).and_then(|row| row.control);
        h.mint_scroll(
            row,
            crate::layout::ScrollRow {
                tracker,
                viewport: viewport.node(),
                content,
                thumb: bar.map(|(_, thumb, _)| thumb),
                rail: bar.map(|(rail, ..)| rail),
                control,
                grab: bar.map(|(.., id)| id),
                reveal,
                state: decl.state,
                last: crate::layout::ThumbGeom::default(),
                front_added: false,
            },
        );
    });
}

/// Reactive writers update the retained declaration, including its reusable track buffers.
fn mount_style_acts(b: &mut Build, slot: &Slot, node: NodeId, row: NodeId) {
    let mut acts = Vec::new();
    let mut at = slot.acts.head;
    while at != NIL {
        let entry = &mut b.acts[at as usize];
        let next = entry.next;
        match entry.act.take() {
            Some(Act::Escape(f)) => Host::with(|h| h.set_escape(row, f)),
            Some(Act::Popup {
                shown,
                spec,
                body,
                closed,
            }) => {
                Host::with(|h| h.mounts.get_mut(row).expect("mounted row").popup = true);
                bind(node, move || {
                    let next = shown();
                    let request = if next {
                        crate::overlay::Request::Show {
                            key: row,
                            spec,
                            body: body.clone(),
                            closed: closed.clone(),
                        }
                    } else {
                        crate::overlay::Request::Close(row)
                    };
                    Host::with(|h| h.request_popup(request));
                });
            }
            Some(act @ (Act::HideWhen(_) | Act::Restyle(..))) => acts.push(act),
            // Put back: the control pass owns the remaining variants.
            other => entry.act = other,
        }
        at = next;
    }
    if acts.is_empty() {
        return;
    }
    bind(node, move || {
        Host::with(|h| {
            let class = h.model.solved(node).class;
            if let Some(recipe) = h.styles.get_mut(node) {
                for act in &acts {
                    match act {
                        Act::HideWhen(hidden) => recipe.layout.base.hidden = Some(hidden()),
                        Act::Restyle(class, write) => write(recipe.layout.at(*class)),
                        _ => {}
                    }
                }
                h.model.style(node, &recipe.lower(class));
            }
        });
    });
}

const fn uia_flag(role: UiaRole) -> HitFlags {
    match role {
        UiaRole::None => HitFlags::NONE,
        _ => HitFlags::UIA,
    }
}

const fn inflate_flag(declined: bool) -> HitFlags {
    if declined {
        HitFlags::NO_INFLATE
    } else {
        HitFlags::NONE
    }
}

/// Returns `flags` with everything that routes a pointer removed and the automation peer
/// kept.
fn uia_only(flags: HitFlags) -> HitFlags {
    if flags.contains(HitFlags::UIA) {
        HitFlags::UIA
    } else {
        HitFlags::NONE
    }
}

/// Lowers a slot's channels into bindings. The one reactive lowering in this crate.
///
/// A constant becomes one `Bind::Set` at mount and produces no graph node, no `Effect` and
/// no allocation, so static content costs one sprite and nothing else. Anything else becomes
/// exactly one effect, and the boxed reader moves into it.
fn mount_channels(b: &mut Build, slot: &Slot, node: NodeId, fill: Option<SpriteId>, scope: Scope) {
    let mut at = slot.chans.head;
    while at != NIL {
        let entry = &mut b.chans[at as usize];
        let (prop, motion) = (entry.prop, entry.motion);
        let source = entry.source.take();
        at = entry.next;
        // A shadow's channels belong to the sprite casting it, which on a container is its
        // fill and not the group the author wrote the modifier on. Retargeted here rather
        // than at the seam: the scene refuses a property its node cannot own, so without
        // this a card's halo would simply never widen.
        let target = match prop {
            Prop::BlurRadius | Prop::ShadowOpacity => fill.map_or(node, SpriteId::node),
            _ => node,
        };
        match source {
            Some(ChanSource::RelativePivot(pivot)) => Host::with(|h| {
                h.geometry_jobs.place(
                    node,
                    super::geometry::Row {
                        scope,
                        local: None,
                        effect: None,
                        pivot: Some(pivot),
                    },
                );
            }),
            Some(ChanSource::Const(constant)) => {
                Host::with(|h| h.model().bind(target, prop, Bind::Set(constant)))
            }
            Some(ChanSource::Dynamic(read)) => {
                // The first value a channel produces is the state it mounts in, not a
                // transition into it. Animating it would sweep every bound property up from
                // whatever the compositor happens to hold — a meter would fill on mount, and
                // a layer declared invisible would fade *out* of a value it never had.
                let previous = std::cell::Cell::new(None);
                bind(node, move || {
                    let next = read();
                    // Dependencies can change without changing this channel: editing an
                    // enabled processor must not restart its card's opacity spring.
                    // Read first so equal output still refreshes dependency tracking.
                    let before = previous.replace(Some(next));
                    if before == Some(next) {
                        return;
                    }
                    let bind = match motion {
                        Motion::Snap => Bind::Set(next),
                        Motion::Chrome if before.is_none() => Bind::Set(next),
                        Motion::Chrome => Bind::Animate(Anim::Spring {
                            to: next,
                            tuning: Tuning::Chrome,
                            delay_ms: 0,
                        }),
                    };
                    Host::with(|h| h.model().bind(target, prop, bind));
                });
            }
            None => {}
        }
    }
}

/// Registers a measured run and points layout at it.
///
/// The measure path cannot read a signal: it runs inside the solve, and `Measure` is `Send`.
/// A dynamic string is therefore snapshotted here and re-snapshotted by its own effect. The
/// glyphs are placed later, once, at the width layout chose.
#[expect(
    clippy::too_many_arguments,
    reason = "one call site, and every argument is a distinct fact the entry records"
)]
fn mount_text(
    b: &mut Build,
    node: NodeId,
    group: Option<GroupId>,
    sprite: Option<SpriteId>,
    text: u32,
    scope: Scope,
    roles: Option<RoleSet>,
    row: NodeId,
) -> MeasureKey {
    let seed = &mut b.texts[text as usize];
    let (ramp, flow, caps, source) = (seed.ramp, seed.flow, seed.caps, seed.source.take());
    let ink = roles.map(|r| Role::Text(r.text)).or(seed.ink);
    // Snapshotted once, here. A `&'static str` crosses as a borrow rather than as a copy, so
    // a screen of chrome labels allocates nothing.
    //
    // Untracked: a mount can run from inside an effect — a keyed list reconciling — and a
    // read taken here would subscribe that effect, so a row with a bound label would rebuild
    // the whole list every time its own label changed. The dependency belongs to the effect
    // this function installs for a dynamic source, which is the one that can act on it.
    let initial = crate::signal::untracked(|| {
        source
            .as_ref()
            .map_or(super::text::Source::Static(""), Into::into)
    });
    let key = Host::with(|h| {
        let key = h.text.mint(super::text::Mint {
            text: initial,
            ramp,
            flow,
            caps,
            vertical: seed.vertical,
            scope,
            ink,
            sprite: sprite.unwrap_or_default(),
            group: group.filter(|_| flow != Flow::Line || seed.vertical),
        });
        if let Some(row) = h.mounts.get_mut(row) {
            row.text = Some(key);
        }
        h.model().measure(node, MeasureCtx::Measured(key));
        key
    });
    // A constant string is already in the table, so only a reactive one needs an effect —
    // the same gate every other value goes through, in the same place.
    //
    // The scratch buffer is the effect's own and outlives every run of it, so it reaches its
    // high-water mark once and an unchanged readout costs a format and a compare: `set_text`
    // declines to reshape a string that did not move.
    if let Some(TextSource::Dynamic(read)) = source {
        let mut scratch = String::new();
        bind(node, move || {
            scratch.clear();
            read(&mut scratch);
            set_text(key, &scratch);
        });
    }
    key
}

/// Replaces a run's text, re-measuring its node and marking the accessible tree stale.
fn set_text(key: MeasureKey, text: &str) {
    Host::with(|h| {
        let Some(node) = h.text.set_text(key, text) else {
            return;
        };
        // The measure's input moved and its context did not, which the model holds no copy
        // of and therefore cannot notice. Without this the run stays stale: reshaping runs
        // from the measure function, and the measure function runs for a dirty node. A row in
        // a list or an arm of a branch is dirtied by being built; a caption that outlives its
        // own value is not.
        h.model().remeasure(node);
        // An accessible name is copied into the published tree's own string blob, so a
        // string that changes here leaves the tree wrong until it is republished. Text that
        // changes faster than event rate does not live in the retained tree at all, so this
        // is not a per-frame cost.
        h.uia_restale();
    });
}

const _: () = {
    // The wash alphas are opacities. Outside the unit range, or inverted, a press would
    // resolve to something other than a wash over the surface it covers.
    assert!(HOVER_ALPHA > 0.0);
    assert!(HOVER_ALPHA < PRESS_ALPHA);
    assert!(PRESS_ALPHA < 1.0);
};
