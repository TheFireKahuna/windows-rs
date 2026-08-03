//! The mount walk: the one place this crate touches `Model`.
//!
//! The arena is post-order — a child finishes before the call consuming it — and the model
//! needs pre-order, so this walks down from the root and emits as it goes. `after` is the
//! previous sibling, which is what makes child order paint order.
//!
//! A slot with exactly one sprite and no children *is* that sprite. Anything richer mints a
//! group whose chrome is absolute at inset zero, ahead of the laid-out children: chrome
//! paints under content, and since `border` is never set, inset zero covers the node rather
//! than the space inside a border.
//!
//! Nothing holds a borrow across a re-entry. `Effect::new` runs its closure immediately and
//! that closure borrows the host, so the arena is taken out of its thread-local for the walk
//! and every model call takes a fresh borrow.

use super::arena::{Act, Build, ChanSource, MaskSeed, NIL, Part, Slot, SpriteSeed};
use super::host::{ControlRow, Host, MountId, MountRow, ValueId, ValueRow};
use super::style::{OverStore, Recipe};
use super::{El, Site, View};
use crate::gesture::GestureDecl;
use crate::layout::{Len, Over, Preset, Rule};
use crate::role::{Metric, Role, Scope};
use crate::signal::Effect;
use crate::widget::{
    Chrome, ChromeRow, Flow, Interaction, ModelState, Motion, RoleSet, StatePolicy, TextSource,
    UiaRole, Wash,
};
use windows_scene::{
    Anim, Bind, Cap, ControlId, Corners, Exit, GeomId, GroupId, HitDecl, HitFlags, Join, Mask,
    MeasureCtx, MeasureKey, NodeId, Paint, PathVerb, Prop, SpriteId, Tuning, Value,
};

/// Mints path geometry, in sprite-local DIPs.
///
/// Lives here rather than beside the builder because this module is the only one that names
/// `Model`, and a second caller is how that rule stops being true.
///
/// **Whoever authors the verbs re-points them when the box changes**, through
/// [`set_geometry`]. This layer cannot do it for them: the verbs are in the sprite's own
/// space, so at a new size they are different verbs and only their author knows which. That
/// is not a gap in practice — a response curve, a knob arc and a routing wire all have a
/// shape that depends on the width, so the code that recomputes them is the code that
/// already knows the width moved.
#[must_use]
pub fn geometry(verbs: &[PathVerb]) -> GeomId {
    Host::with(|h| h.model().geometry(verbs))
}

/// Re-points geometry. **Every** sprite sharing the id moves together, whichever
/// construction each one uses, so a curve's fill, stroke and glow cannot diverge.
pub fn set_geometry(id: GeomId, verbs: &[PathVerb]) {
    Host::with(|h| h.model().set_geometry(id, verbs));
}

/// A hover wash's opacity, and a press's.
///
/// Two numbers, and they live here rather than in the palette because they are the
/// **opacity of a derived wash**, not a colour. A token named after the component that
/// wanted it is the bloat smell; a token that is not a colour at all is worse.
const HOVER_ALPHA: f32 = 0.06;
const PRESS_ALPHA: f32 = 0.12;

/// A thumb's resting opacity. Ink at a fraction, for the reason the washes are: it is an
/// opacity over whatever it sits on rather than a colour of its own.
const THUMB_ALPHA: f32 = 0.30;

/// A mounted subtree, and the only thing that unmounts it.
///
/// Dropping it destroys the node and releases every table row the walk claimed — the style
/// recipes, the control rows, the measured runs. Holding one *is* "this is on screen", and
/// forgetting to release is not expressible.
///
/// It does **not** own the effects the mount installed. Those belong to whatever
/// [`Owner`](crate::signal::Owner) was current, which is the right place: an application
/// scope, a keyed row's, a branch arm's — all three already dispose in the same breath as
/// they drop this.
#[must_use = "dropping a mount unmounts its subtree immediately"]
#[derive(Debug)]
pub struct Mount {
    node: NodeId,
    exit: Exit,
    /// The head of this subtree's chain through the mount table. A chain and not a `Vec`,
    /// so realizing a list row during a fling allocates nothing.
    rows: MountId,
}

impl Mount {
    /// The node this subtree is rooted at.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// Gives up the right to unmount, for a root that lives as long as the process.
    ///
    /// The rows stay claimed, which is the honest outcome: a deliberate leak of a bounded
    /// amount, rather than a release.
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

/// Mounts under `parent` at an explicit sibling position and scope. What a structural
/// adapter calls for each row or arm it realizes.
pub fn mount_at(el: View, parent: GroupId, after: Option<NodeId>, scope: Scope) -> Mount {
    let mut build = Build::take();
    // The one place a stale element can be named. The arena is cleared after each mount, so
    // an `El` held across one indexes a slot that is no longer there — and a raw bounds
    // panic here would point at the arena rather than at the call site that kept it.
    let exit = build
        .nodes
        .get(el.at as usize)
        .expect("this element was built before an earlier mount and does not survive it")
        .exit;
    let mut rows = Rows::default();
    let node = walk(
        &mut build,
        Where::new(el.at, parent, after, scope),
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

/// What a subtree hands up to the control enclosing it.
///
/// A control's moving part is almost never its own sprite — a slider's knob, a toggle's knob
/// and a meter's level are all children of the thing they belong to — so the parts a control
/// needs are collected on the way back up rather than read off its own seed list. Reading
/// them off the seed list is how a `ChromeRow` ends up with `thumb: None` for every control
/// that has one, and how the front thread ends up computing a value it cannot show.
#[derive(Default)]
struct Claim {
    thumb: Option<SpriteId>,
    /// The value row a [`Unit::Travel`] channel opened, still waiting for the track it runs
    /// in — which is the enclosing control, and is not known until that control mounts.
    value: Option<ValueId>,
    /// The first text this subtree laid out, which is what an enclosing control derives its
    /// accessible name from.
    ///
    /// Collected on the way back up for the same reason the thumb is: a control's label is
    /// almost never its own sprite — `button` is a control with a text *child* — so reading
    /// the control's own row instead leaves every button in the stack unnamed.
    text: Option<MeasureKey>,
}

impl Claim {
    /// Takes what a subtree offered, without displacing what this node already found.
    fn absorb(&mut self, inner: Self) {
        self.thumb = self.thumb.or(inner.thumb);
        self.value = self.value.or(inner.value);
        self.text = self.text.or(inner.text);
    }
}

/// Where one node in the walk goes: which slot, under what, after what, at which scope.
#[derive(Copy, Clone)]
struct Where {
    at: u32,
    parent: GroupId,
    after: Option<NodeId>,
    scope: Scope,
}

impl Where {
    const fn new(at: u32, parent: GroupId, after: Option<NodeId>, scope: Scope) -> Self {
        Self {
            at,
            parent,
            after,
            scope,
        }
    }
}

/// The chain of mount-table rows one walk claimed, threaded as it goes.
#[derive(Default)]
struct Rows {
    head: MountId,
    tail: MountId,
}

impl Rows {
    fn push(&mut self, at: MountId) {
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

/// Emits one slot and its subtree.
///
/// `rows` collects every index the whole walk claimed, which is what the unmount releases.
fn walk(b: &mut Build, at: Where, rows: &mut Rows, claim: &mut Claim) -> NodeId {
    let slot = b.nodes[at.at as usize];
    let inner = slot.elevate.map_or(at.scope, |e| at.scope.elevate(e));
    let roles = slot.chrome.map(Chrome::roles);

    // Walked, never collected. A `Vec` of seeds here is one allocation per node per mount,
    // on the path a list row realized during a fling takes — and the seeds are `Copy`, so
    // the chain answers every question the collection would have.
    let seed_count = b.seed_count(slot.seeds);
    // Chrome is a table row and not sprites yet, so a variant with no fill costs one visual
    // fewer rather than one invisible one.
    let chrome_count = roles.map_or(0, |r| {
        usize::from(r.fill.is_some()) + usize::from(r.stroke.is_some())
    });
    // A run that can break needs one sprite per line, so it is a group whatever else it is.
    let run = run_seed(b, &slot);
    let wraps = run.is_some_and(|(_, flow)| flow == Flow::Wrap);
    let leaf = seed_count == 1
        && chrome_count == 0
        && slot.kids.len == 0
        && slot.adapter.is_none()
        && slot.responsive.is_none()
        && slot.state == StatePolicy::None
        && !wraps;

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
    let recipe = Recipe {
        preset: slot.preset,
        over: OverStore::collect(b.chain_over(slot.over).map(|entry| entry.rule)),
        scope: at.scope,
    };
    let style = crate::layout::lower(recipe.preset, recipe.over.as_slice(), at.scope);
    super::style::with(|table| table.place(node, recipe));
    let row = Host::with(|h| {
        h.model().style(node, &style);
        h.mint_mount(MountRow {
            node,
            next: MountId::NONE,
            control: None,
            text: None,
            values: ValueId::NONE,
            scroll: None,
        })
    });
    rows.push(row);

    // ── the node's own sprites, where it is not one itself ────────────────────────
    let mut previous: Option<NodeId> = None;
    if let Some(group) = group {
        for (part, seed) in chrome_seeds(roles, slot.chrome, inner) {
            let sprite = Host::with(|h| h.model().sprite(group, previous));
            cover(sprite.node(), inner, chrome_inset(part));
            emit_sprite(sprite, &seed, None, inner, roles);
            parts.set(part, sprite, &mut own_claim);
            previous = Some(sprite.node());
        }
        // A wrapping run has no sprite of its own: its lines are minted as they are shaped.
        if !wraps {
            for &seed in b.chain_seeds(slot.seeds) {
                let sprite = Host::with(|h| h.model().sprite(group, previous));
                cover(sprite.node(), inner, Len::Zero);
                emit_sprite(sprite, &seed, slot.geom, inner, roles);
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
        emit_wash(sprite, hover, inner, radius_of(b, &slot, inner));
        previous = Some(sprite.node());
        parts.wash = Some(sprite);
    }

    // ── styles that follow a value ────────────────────────────────────────────────
    // Its own pass over the act chain, taking only its own variants: a spacer has a style
    // that moves and no hit entry at all, so this and the control row cannot be one pass.
    mount_style_acts(b, &slot, node);

    // ── channels: one reactive lowering ───────────────────────────────────────────
    mount_channels(b, &slot, node, row, &mut own_claim);

    // ── measured text ─────────────────────────────────────────────────────────────
    if let Some((text, _)) = run {
        let target = if wraps {
            None
        } else {
            Some(parts.label.expect("a run seed mints its own sprite"))
        };
        let key = mount_text(b, node, group, target, text, inner, roles, row);
        // This node's own text wins over anything its children offer, which is what
        // `absorb` means one level up — and the order here is what makes it true.
        own_claim.text.get_or_insert(key);
    }

    // ── children ──────────────────────────────────────────────────────────────────
    if slot.kids.len > 0 {
        let group = group.expect("a node with children is a group");
        for index in 0..slot.kids.len {
            let kid = b.kids[(slot.kids.at + index) as usize];
            previous = Some(walk(
                b,
                Where::new(kid, group, previous, inner),
                rows,
                &mut own_claim,
            ));
        }
    }

    // ── the control row, once the walk has found the parts it names ───────────────
    // After the children, because a control's moving part is one of them. Nothing above
    // depends on the row existing, and the hit array is a declaration rather than an order.
    if slot.hit.is_some() {
        mount_control(b, &slot, node, parts, own_claim, inner, row);
    } else {
        // Not a control, so whatever the subtree offered belongs to whoever encloses this.
        claim.absorb(own_claim);
    }

    if let Some(decl) = slot.scroll {
        let group = group.expect("a scroll container is a group");
        let content = previous.expect("a scroll container has a content group");
        mount_scroll(group, content, decl, inner, row);
    }

    if let Some(bounds) = slot.responsive {
        let group = group.expect("a responsive container is a group");
        Host::with(|h| h.model().responsive(group, bounds));
    }

    // Last, and outside every borrow: an adapter builds application views, so it runs where
    // a nested `Build::with` is legal. By here this node's own slot is finished with.
    if let Some(adapter) = slot.adapter
        && let Some(install) = b.adapters[adapter as usize].install.take()
    {
        let group = group.expect("a node with an adapter is a group");
        install(Site {
            parent: group,
            scope: inner,
        });
    }

    node
}

/// The text seed a slot carries, and how it flows.
fn run_seed(b: &Build, slot: &Slot) -> Option<(u32, Flow)> {
    b.chain_seeds(slot.seeds).find_map(|s| match s.mask {
        MaskSeed::Run { text } => Some((text, b.texts[text as usize].flow)),
        _ => None,
    })
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
    /// The moving part goes to the claim rather than here, because it is the one part a
    /// control usually does not own: it belongs to whichever control encloses it.
    fn set(&mut self, part: Part, id: SpriteId, claim: &mut Claim) {
        match part {
            Part::Fill => self.fill = Some(id),
            Part::Label => self.label = Some(id),
            Part::Border => self.border = Some(id),
            Part::Wash => self.wash = Some(id),
            Part::Thumb => claim.thumb = claim.thumb.or(Some(id)),
            Part::Static => {}
        }
    }
}

/// The sprites a chrome row expands to, bottom first.
///
/// A stroked surface is **two boxes and not one outlined box**: the alphabet has no
/// outlined rectangle, only outlined geometry, and geometry is authored in sprite-local
/// DIPs and must be re-emitted whenever the box moves. An outer box in the stroke colour
/// with the fill inset by a hairline over it draws the same ring, keeps the nine-grid's
/// exact corners, is shared through the same raster cache, and costs nothing on a resize.
fn chrome_seeds(
    roles: Option<RoleSet>,
    chrome: Option<Chrome>,
    scope: Scope,
) -> impl Iterator<Item = (Part, SpriteSeed)> {
    let radius = chrome.map_or(0.0, |c| crate::role::metric(c.radius, scope));
    let hairline = crate::role::metric(Metric::HairlineW, scope);
    let stroke = roles.and_then(|r| r.stroke).map(move |role| {
        (
            Part::Border,
            SpriteSeed {
                mask: MaskSeed::Radius { dips: radius },
                role: Role::Stroke(role),
                part: Part::Border,
                next: NIL,
            },
        )
    });
    let inset = if stroke.is_some() { hairline } else { 0.0 };
    let fill = roles.and_then(|r| r.fill).map(move |role| {
        (
            Part::Fill,
            SpriteSeed {
                // Concentric with the ring it sits in, so the hairline is one width all the
                // way round instead of pinching at the corners.
                mask: MaskSeed::Radius {
                    dips: (radius - inset).max(0.0),
                },
                role: Role::Fill(role),
                part: Part::Fill,
                next: NIL,
            },
        )
    });
    stroke.into_iter().chain(fill)
}

/// How far a chrome sprite is inset from the node it covers.
const fn chrome_inset(part: Part) -> Len {
    match part {
        // The fill sits inside the ring the border draws.
        Part::Fill => Len::Metric(Metric::HairlineW),
        _ => Len::Zero,
    }
}

/// Styles a sprite as chrome for its parent rather than as a laid-out child of it.
///
/// Absolute at `inset`. Because `border` is never set on any style this crate produces, the
/// parent's padding box is its border box, so a zero inset covers the node exactly.
fn cover(node: NodeId, scope: Scope, inset: Len) {
    let style = crate::layout::lower(
        Preset::Bare,
        &[
            Rule::always(Over::Absolute),
            Rule::always(Over::Inset(inset)),
        ],
        scope,
    );
    Host::with(|h| h.model().style(node, &style));
}

/// Resolves one sprite's mask and paint.
///
/// **`resolve` is called here and nowhere above it.** Neither `Radiance` nor `Paint` is
/// reachable from a widget, which makes "a widget may not accept a colour" a property of
/// the module graph rather than of a lint. The scope's width axis is pinned on the way in,
/// so a resize cannot re-key a single cell.
fn emit_sprite(
    id: SpriteId,
    seed: &SpriteSeed,
    geom: Option<GeomId>,
    scope: Scope,
    roles: Option<RoleSet>,
) {
    let light = crate::role::resolve(role_of(seed, roles), scope.for_paint());
    // One borrow: the stroke resource, the mask and the paint are three model calls about
    // one sprite, and the walk takes this borrow once per sprite already.
    Host::with(|h| {
        let mask = match seed.mask {
            MaskSeed::Box { radius } => Mask::Box {
                radius: Corners::all(radius.and_then(|r| r.dips(scope)).unwrap_or(0.0)),
            },
            MaskSeed::Radius { dips } => Mask::Box {
                radius: Corners::all(dips),
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
        h.model().paint(id, Paint::Solid(light));
    });
}

/// A sprite's role: its own, unless it is the label of a widget whose chrome row owns the
/// text colour. That is what lets `.accent()` reach the text without the text seed knowing
/// there are variants.
fn role_of(seed: &SpriteSeed, roles: Option<RoleSet>) -> Role {
    match (seed.part, roles) {
        (Part::Label, Some(roles)) => Role::Text(roles.text),
        _ => seed.role,
    }
}

/// The wash a hover or a press fades in.
///
/// Its paint is the wash at **full** strength and its opacity carries the alpha, so hover
/// and press are one channel and one spring rather than two colours. A colour animation is
/// not available here at all: a sprite's colour is an FP16 cell, a composition colour brush
/// is 8-bit, and no brush interpolates between two FP16 sources.
fn emit_wash(id: SpriteId, wash: Wash, scope: Scope, radius: f32) {
    let light = match wash {
        Wash::Ink => crate::role::ink(1.0, scope),
        Wash::Accent => crate::role::accent_wash(1.0, scope),
    };
    Host::with(|h| {
        h.model().mask(
            id,
            Mask::Box {
                radius: Corners::all(radius),
            },
        );
        h.model().paint(id, Paint::Solid(light));
        // Parked at zero with a `Set` and not a spring: a control that has never been
        // hovered must not play an animation to arrive at invisible.
        h.model()
            .bind(id.node(), Prop::Opacity, Bind::Set(Value::Scalar(0.0)));
    });
}

/// The radius the wash should match, so a pill's wash is a pill and a card's is a card with
/// nothing declared twice.
fn radius_of(b: &Build, slot: &Slot, scope: Scope) -> f32 {
    if let Some(chrome) = slot.chrome {
        return crate::role::metric(chrome.radius, scope);
    }
    b.chain_seeds(slot.seeds)
        .find(|s| s.part == Part::Fill)
        .and_then(|s| match s.mask {
            MaskSeed::Box { radius } => radius.and_then(|r| r.dips(scope)),
            MaskSeed::Radius { dips } => Some(dips),
            _ => None,
        })
        .unwrap_or(0.0)
}

/// Moves the slot's actions into the host's dense table and declares the node to the one
/// hit array.
///
/// The handlers **move**; they are not cloned and not referenced. They live on this thread
/// for the node's lifetime and reach the front thread only as a presence bit in
/// [`HitFlags`], which is what keeps `SinkPatch: Send` provable rather than aspirational.
fn mount_control(
    b: &mut Build,
    slot: &Slot,
    node: NodeId,
    parts: Parts,
    claim: Claim,
    scope: Scope,
    row: MountId,
) -> Option<ControlId> {
    let hit = slot.hit?;

    let mut control = ControlRow {
        node,
        fill: parts.fill,
        label: parts.label,
        border: parts.border,
        front: ChromeRow {
            // Overwritten below, once the id exists. The row and its identity are minted in
            // the same breath so the two halves cannot disagree about what a control is.
            id: ControlId::default(),
            wash: parts.wash,
            hover: HOVER_ALPHA,
            press: PRESS_ALPHA,
            thumb: claim.thumb.map(SpriteId::node),
            // The room is a solve output, so it arrives with the first publish rather than
            // here. Until then a fraction moves the part nowhere, which is where it starts.
            travel: 0.0,
            drive: slot.interaction,
            fraction: 0.0,
        },
        chrome: slot.chrome,
        scope,
        state: ModelState::Rest,
        click: None,
        change: None,
        commit: None,
        tip: None,
        flyout: None,
        uia: slot.uia,
        name: slot.name,
        text: claim.text,
        key: slot.key,
    };

    let mut disabled = None;
    let mut selected = None;
    let mut at = slot.acts.head;
    while at != NIL {
        let entry = &mut b.acts[at as usize];
        at = entry.next;
        match entry.act.take() {
            Some(Act::Click(f)) => control.click = Some(f),
            Some(Act::ChangeF64(f)) => control.change = Some(f),
            Some(Act::CommitF64(f)) => control.commit = Some(f),
            Some(Act::Tip(t, side)) => control.tip = Some((std::rc::Rc::new(t), side)),
            Some(Act::Flyout(f)) => control.flyout = Some(f),
            Some(Act::DisabledWhen(f)) => disabled = Some(f),
            Some(Act::SelectedWhen(f)) => selected = Some(f),
            Some(Act::HideWhen(_) | Act::Restyle(_)) | None => {}
        }
    }

    // Folded in here, so declining an inflation is order-independent at the call site and
    // still cannot conjure a target on a node that declared none.
    let flags = hit.flags | uia_flag(slot.uia) | inflate_flag(slot.no_inflate);
    let inflate = hit.inflate.and_then(|l| l.dips(scope));
    // A control that refined nothing still declares the default: tap, right-tap and hold,
    // which is what gives a touch user the context menu a mouse user gets from the secondary
    // button. Gated on the flag whose whole meaning is "has a gesture declaration", so the
    // entry cannot claim one it does not have — and so this walk, which also runs for nodes
    // that exist only for automation, does not put a recogniser behind a static label.
    let gesture = b
        .gesture(slot.gesture)
        .or_else(|| flags.contains(HitFlags::GESTURE).then(GestureDecl::default));
    let value = claim.value;
    let caption = slot.caption;
    let id = Host::with(move |h| {
        let id = h.mint_control(control);
        if let Some(row) = h.mounts.get_mut(row) {
            row.control = Some(id);
        }
        // The front thread's half, shipped as numbers and ids: its own copy stays here so a
        // solve that changed this control's room can re-send a corrected one.
        if let Some(control) = h.control_mut(id) {
            control.front.id = id;
            let front = control.front;
            h.chrome.push(front);
        }
        // The moving part's track is this control's own box, and this is the first moment
        // both are known — as is **who moves it**: a control the router drives owns the
        // channel outright from here, so this thread corrects its geometry by re-sending the
        // room rather than by writing the property behind the router's back.
        if let Some(value) = value {
            h.own_value(value, id, node, front_driven(slot.interaction));
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

    // Model state, and therefore a discrete paint swap at event rate — not a wash. Both
    // arms answer one question through one setter, so the last writer of a frame wins in
    // the order the effects were created and there is no pair of them to disagree.
    if let Some(disabled) = disabled {
        Effect::new(move || {
            let off = disabled();
            let decl = HitDecl {
                // A disabled control keeps its automation peer and loses everything that
                // routes a pointer: a screen reader still finds it, a click does not.
                flags: if off { uia_only(flags) } else { flags },
                id,
                touch_inflate: inflate,
            };
            Host::with(|h| {
                h.model().hit(node, Some(decl));
                h.set_state(
                    id,
                    if off {
                        Some(ModelState::Disabled)
                    } else {
                        None
                    },
                );
            });
        });
    }
    if let Some(selected) = selected {
        Effect::new(move || {
            let on = selected();
            Host::with(|h| h.set_state(id, on.then_some(ModelState::Selected)));
        });
    }
    Some(id)
}

/// Delegates a container's scrolling to a tracker, and gives it a thumb.
///
/// **Two bindings and one tracker.** The content rides it negated, because a tracker's
/// position increases for up and left; the thumb rides the same one at the ratio of the two
/// extents, which is what makes the thumb follow the content with no front-thread work at
/// all. The tracker itself is a composition object, so it is named here and created on the
/// other side.
fn mount_scroll(
    viewport: GroupId,
    content: NodeId,
    decl: crate::layout::ScrollDecl,
    scope: Scope,
    row: MountId,
) {
    let reveal = decl.reveal;
    Host::with(|h| {
        let tracker = h.model().tracker_id::<windows_scene::Observed>();
        h.trackers.push(super::host::TrackerSpec {
            id: tracker,
            viewport,
            axes: windows_scene::Axes::VERTICAL,
        });
        h.model().bind(
            content,
            Prop::OffsetY,
            Bind::Track {
                tracker,
                axis: windows_scene::TrackerAxis::PositionY,
                affine: windows_scene::Affine::CONTENT,
            },
        );
        // The scrollbar lives in the viewport rather than in the content, because it must
        // not scroll with what it reports on — and **above** the content, because child
        // order is paint order and is the order the hit array is scanned in. Below it, the
        // bar is painted under whatever the list draws and a grab resolves to the row
        // behind it.
        //
        // A rail and a thumb, and the split is not decoration: the rail is static geometry
        // and carries the target, the thumb is moved by the compositor and carries none. A
        // hit entry on the thumb would name a rect the solve fixed and the tracker then
        // left behind.
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
            // Concealed from the mount rather than shown and faded: a surface whose content
            // fits never overflows, and a thumb that appeared for one frame to say so is a
            // flash on every screen that opens.
            if reveal == crate::layout::Reveal::OnDemand {
                h.model()
                    .bind(thumb.node(), Prop::Opacity, Bind::Set(Value::Scalar(0.0)));
            }
            // The rail's control: a hit entry and a drag, and deliberately no chrome row.
            // The thumb's opacity belongs to the reveal policy, and a control the front
            // table adopted would give one channel two owners.
            // The hit entry itself is `publish_scrolls`', because whether the rail is a
            // target at all depends on whether there is anything to scroll — which is a
            // solve output.
            let id = h.mint_control(thumb_control(rail.node(), scope));
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
                grabbed_at: None,
                shown: reveal == crate::layout::Reveal::Always,
            },
        );
    });
}

/// The rail's row: an identity for the hit array, and nothing that paints.
fn thumb_control(node: NodeId, scope: Scope) -> ControlRow {
    ControlRow {
        node,
        fill: None,
        label: None,
        border: None,
        front: ChromeRow {
            id: ControlId::default(),
            wash: None,
            hover: 0.0,
            press: 0.0,
            thumb: None,
            travel: 0.0,
            drive: None,
            fraction: 0.0,
        },
        chrome: None,
        scope,
        state: ModelState::Rest,
        click: None,
        change: None,
        commit: None,
        tip: None,
        flyout: None,
        uia: UiaRole::None,
        name: None,
        text: None,
        key: None,
    }
}

/// Installs the effects behind a style that follows a value.
///
/// Each re-lowers from the node's **own recipe** with one override appended, rather than
/// from a style it has to remember, and at the class the last solve resolved for the node
/// rather than one captured here — so neither the recipe nor the class can fall out of date.
fn mount_style_acts(b: &mut Build, slot: &Slot, node: NodeId) {
    let mut at = slot.acts.head;
    while at != NIL {
        let entry = &mut b.acts[at as usize];
        let next = entry.next;
        let act = match entry.act.take() {
            Some(act @ (Act::HideWhen(_) | Act::Restyle(_))) => act,
            // Put back: this pass owns two variants, and the control pass owns the rest.
            other => {
                entry.act = other;
                at = next;
                continue;
            }
        };
        at = next;
        // Installed outside every borrow, because creating an effect runs it.
        Effect::new(move || {
            let extra = match &act {
                Act::HideWhen(hidden) => hidden().then_some(Over::Hidden),
                Act::Restyle(over) => Some(over()),
                _ => None,
            };
            let class = Host::with(|h| h.model().solved(node).class);
            let Some(style) = super::style::lower_with(node, class, extra) else {
                return;
            };
            Host::with(|h| h.model().style(node, &style));
        });
    }
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

/// Everything routing, removed; the automation peer, kept.
fn uia_only(flags: HitFlags) -> HitFlags {
    if flags.contains(HitFlags::UIA) {
        HitFlags::UIA
    } else {
        HitFlags::NONE
    }
}

/// The one reactive lowering.
///
/// A **constant** is one `Bind::Set` at mount and produces no graph node, no `Effect` and
/// no allocation — which is the whole of "static content costs one sprite and nothing
/// else", decided here rather than once per widget. Anything else becomes exactly one
/// effect, and the boxed reader **moves** into it.
fn mount_channels(b: &mut Build, slot: &Slot, node: NodeId, row: MountId, claim: &mut Claim) {
    let mut at = slot.chans.head;
    while at != NIL {
        let entry = &mut b.chans[at as usize];
        let (prop, motion, unit) = (entry.prop, entry.motion, entry.unit);
        let source = entry.source.take();
        at = entry.next;
        // A value is finished by whoever is moving the part — this thread, or the router.
        // A slid one also waits on the room it is a fraction *of*, which is a solve output.
        let value = unit.is_value().then(|| {
            let id = Host::with(|h| {
                h.mint_value(ValueRow {
                    node,
                    // Its own box until the enclosing control claims it, which reads as zero
                    // room — the honest answer before layout has said anything.
                    track: node,
                    control: None,
                    unit,
                    prop,
                    motion,
                    vertical: prop == Prop::OffsetY,
                    fraction: 0.0,
                    travel: 0.0,
                    front_driven: false,
                    row,
                    next: ValueId::NONE,
                })
            });
            claim.value = claim.value.or(Some(id));
            id
        });
        match source {
            Some(ChanSource::Const(constant)) => match value {
                Some(id) => Host::with(|h| h.set_fraction(id, scalar(constant))),
                None => Host::with(|h| h.model().bind(node, prop, Bind::Set(constant))),
            },
            Some(ChanSource::Dynamic(read)) => {
                Effect::new(move || {
                    let next = read();
                    if let Some(id) = value {
                        Host::with(|h| h.set_fraction(id, scalar(next)));
                        return;
                    }
                    let bind = match motion {
                        Motion::Snap => Bind::Set(next),
                        Motion::Chrome => Bind::Animate(Anim::Spring {
                            to: next,
                            tuning: Tuning::Chrome,
                            delay_ms: 0,
                        }),
                    };
                    Host::with(|h| h.model().bind(node, prop, bind));
                });
            }
            None => {}
        }
    }
}

/// Whether the router moves this control's part, rather than the application.
///
/// A press has no value to move, so its part follows the app's own channel; a slide and a
/// turn are read off the pointer, and from the first contact the router is the only writer.
const fn front_driven(interaction: Option<Interaction>) -> bool {
    matches!(
        interaction,
        Some(Interaction::Slide(_) | Interaction::Turn(_))
    )
}

/// A fraction's number. A value channel is scalar by construction — there is no
/// two-component fraction — so anything else is a widget seeding the wrong channel.
fn scalar(value: Value) -> f32 {
    if let Value::Scalar(v) = value {
        return v;
    }
    debug_assert!(false, "a fraction is a scalar");
    0.0
}

/// Registers a measured run and points layout at it.
///
/// The measure path cannot read a signal — it runs inside the solve, and `Measure` is
/// `Send` — so a dynamic string is snapshotted here and re-snapshotted by its own effect.
/// The glyphs are placed later, once, at the width layout chose.
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
    row: MountId,
) -> MeasureKey {
    let seed = &mut b.texts[text as usize];
    let (ramp, flow, source) = (seed.ramp, seed.flow, seed.source.take());
    let ink = seed.ink.or(roles.map(|r| r.text));
    // Snapshotted once, here. A `&'static str` crosses as a borrow rather than as a copy,
    // which is what keeps a screen of chrome labels free.
    //
    // **Untracked**, and that is not an optimisation. A mount can run from inside an
    // effect — a keyed list reconciling — and a read taken here would subscribe *that*
    // effect. A row with a bound label would then rebuild the whole list every time its own
    // label changed. The dependency belongs to the effect installed below, which is the one
    // that can act on it.
    let initial = crate::signal::untracked(|| {
        source
            .as_ref()
            .map_or(super::text::Source::Static(""), Into::into)
    });
    let key = Host::with(|h| {
        let key = super::text::with(|table| {
            table.mint(super::text::Mint {
                text: initial,
                ramp,
                flow,
                scope,
                ink,
                sprite: sprite.unwrap_or_default(),
                group: group.filter(|_| flow == Flow::Wrap),
            })
        });
        if let Some(row) = h.mounts.get_mut(row) {
            row.text = Some(key);
        }
        h.model().measure(node, MeasureCtx::Measured(key));
        key
    });
    // A constant string is already in the table, so only a reactive one needs an effect —
    // the same gate every other value goes through, in the same place.
    if let Some(TextSource::Dynamic(read)) = source {
        Effect::new(move || set_text(key, &read()));
    }
    key
}

fn set_text(key: MeasureKey, text: &str) {
    let moved = super::text::with(|table| table.set_text(key, text));
    // An accessible name is a **copy**, taken into the published tree's own string blob,
    // so a string that changes here is one the tree is now wrong about. Marking the tree
    // stale is what republishes it — and text that changes faster than event rate does not
    // live in the retained tree at all, so this cannot be a per-frame cost.
    if moved {
        Host::with(|h| h.uia_restale());
    }
}

const _: () = {
    // The wash alphas are opacities. Outside the unit range, or inverted, a press would
    // resolve to something other than a wash over the surface it covers.
    assert!(HOVER_ALPHA > 0.0);
    assert!(HOVER_ALPHA < PRESS_ALPHA);
    assert!(PRESS_ALPHA < 1.0);
};
