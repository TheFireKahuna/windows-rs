//! Patch application. **Front half.**
//!
//! Strictly in order, which is what the seam rests on: one emitter upstream is one
//! ordering authority, so a slot freed by a destroy can be reused by a create *in the same
//! patch*, and a resource re-pointed mid-patch is seen by every sprite that binds it after.
//!
//! Every span is read through the patch's own bounds check, once, here — not at each use.

use crate::Scene;
use crate::backends::Backends;
use crate::cache::Gen;
use crate::env::Env;
use crate::id::Id;
use crate::node::{ClipState, Dashes, Node, Painted, Route};
use crate::patch::{Op, SinkPatch};
use crate::prop::{self, Absent, Held};
use crate::res::Res;
use crate::sink::*;
use windows_color::Radiance;
use windows_composition::{Animation, BorderMode, CompositionAnimation, Stretch};
use windows_core::Result;
use windows_numerics::Vector3;

impl Scene {
    /// Applies a patch under `env`, and reports whether the tick changed anything.
    ///
    /// `&mut` on the patch is the pooling: the caller gets its allocations back.
    ///
    /// The environment is stated here rather than pushed in advance, so a display that
    /// moved cannot be applied against stale rasters — [`sync`](Scene::sync) runs first and
    /// rebinds whatever the move invalidated. Answering `false` on a woken tick means
    /// something requested a frame it did not need, which is the one shape of idle waste
    /// this crate cannot see from the inside.
    pub fn apply(&mut self, patch: &mut SinkPatch, back: &Backends, env: Env) -> Result<bool> {
        let before = self.census;
        // Geometry solved under a different display than the one being applied to. The
        // scene still syncs to `env` — the rasters must match the display that is actually
        // there — so this is recorded, not refused. See `Census::env_mismatches`.
        if patch.env().is_some_and(|solved| solved != env) {
            self.census.env_mismatches += 1;
        }
        self.sync(back, env)?;
        // Exits that the compositor has reported complete. Swept at the top of a tick, so a
        // ghost is released on its batch's own completion signal and never on a timer.
        self.motion.ghosts.retain(|ghost| !ghost.finished());

        for index in 0..patch.ops().len() {
            let op = patch.ops()[index];
            self.apply_one(op, patch, back, env)?;
            self.census.ops_applied += 1;
        }
        patch.clear();
        Ok(self.census.changed_since(&before))
    }

    fn apply_one(&mut self, op: Op, patch: &SinkPatch, back: &Backends, env: Env) -> Result<()> {
        match op {
            Op::New {
                id,
                kind,
                parent,
                after,
            } => self.new_node(id, kind, parent, after, back),
            Op::Move { id, parent, after } => self.move_node(id, parent, after),
            Op::Drop { id, exit } => self.drop_node(id, exit, back, env),
            Op::Clip { id, clip } => self.set_clip(id, clip, back, env),
            Op::Mask { id, mask } => {
                let dashes = Dashes::from(match mask {
                    Mask::Shape {
                        stroke: Some(stroke),
                        ..
                    } => patch.dashes(stroke.dashes),
                    _ => &[][..],
                });
                self.declare(id, Some((mask, dashes)), None, back, env)
            }
            Op::Paint { id, paint } => self.declare(id, None, Some(paint), back, env),
            Op::Bind { id, prop, bind } => {
                self.check_not_front_owned(id, prop);
                self.bind_channel(id, prop, bind, patch, back, env)
            }
            Op::Res { id, op } => self.apply_res(id, op, patch, back, env),
            Op::Tracker { id, op } => {
                self.apply_tracker(id, op);
                Ok(())
            }
            Op::Hits { entries } => {
                self.hits.replace(patch.hits(entries));
                Ok(())
            }
        }
    }

    // ── structure ─────────────────────────────────────────────────────────────────

    /// The **one** place in the crate that branches on node kind.
    ///
    /// A sprite visual *is* a container visual, so everything below treats them alike: the
    /// destroy, the reorder, the bind and the device-loss rebind lose a two-arm match.
    fn new_node(
        &mut self,
        id: NodeId,
        kind: NodeKind,
        parent: NodeId,
        after: Option<NodeId>,
        back: &Backends,
    ) -> Result<()> {
        let node = match kind {
            NodeKind::Group => {
                let group = back.compositor.create_container_visual();
                Node::new(crate::base_of_group(&group), None, kind)
            }
            NodeKind::Sprite => {
                let sprite = back.compositor.create_sprite_visual();
                Node::new(crate::base_of_sprite(&sprite), Some(sprite), kind)
            }
        };
        self.nodes.insert(id, node);
        self.census.visuals_minted += 1;
        self.census.visuals_live += 1;
        self.link(id, parent, after);
        Ok(())
    }

    fn move_node(&mut self, id: NodeId, parent: NodeId, after: Option<NodeId>) -> Result<()> {
        self.unlink(id);
        self.link(id, parent, after);
        Ok(())
    }

    /// Parents `id` under `parent`, above `after`, in the compositor and in the arena.
    ///
    /// In that order: the chain mirrors what the compositor holds, so if the collection
    /// insert cannot happen the chain must not claim it did.
    fn link(&mut self, id: NodeId, parent: NodeId, after: Option<NodeId>) {
        let (Some(visual), Some(children)) = (
            self.nodes.get(id).map(|n| n.visual.clone()),
            self.nodes
                .get(parent)
                .and_then(|n| n.visual.as_container())
                .map(|c| c.children()),
        ) else {
            debug_assert!(
                false,
                "a node was parented under one that is no longer live"
            );
            return;
        };
        match after.and_then(|sibling| self.nodes.get(sibling).map(|n| n.visual.clone())) {
            Some(sibling) => children.insert_above(&visual, &sibling),
            None => children.insert_at_bottom(&visual),
        }
        crate::tree::link(&mut self.nodes, id, parent, after);
    }

    fn unlink(&mut self, id: NodeId) {
        let Some((parent, visual)) = self
            .nodes
            .get(id)
            .map(|n| (n.links.parent, n.visual.clone()))
        else {
            return;
        };
        if let Some(children) = self
            .nodes
            .get(parent)
            .and_then(|n| n.visual.as_container())
            .map(|c| c.children())
        {
            // Fallible: a caller can hold a node whose parent was torn down between two
            // operations, and "already gone" is the goal state.
            let _ = children.try_remove(&visual);
        }
        crate::tree::unlink(&mut self.nodes, id);
    }

    /// Destroys a node **and its subtree**, releasing every resource on the way down.
    fn drop_node(&mut self, id: NodeId, exit: Exit, back: &Backends, env: Env) -> Result<()> {
        self.unlink(id);
        if exit != Exit::None
            && let Some(ghost) = self.ghost(id, exit, back, env)?
        {
            self.motion.ghosts.push(ghost);
        }
        self.destroy_subtree(id);
        self.release_front_claims();
        Ok(())
    }

    /// Depth is layout nesting, so this recurses on the chain rather than carrying a stack.
    fn destroy_subtree(&mut self, id: NodeId) {
        let mut child = self.nodes.get(id).map_or(NodeId::NONE, |n| n.links.first);
        while let Some(node) = self.nodes.get(child) {
            let next = node.links.next;
            self.destroy_subtree(child);
            child = next;
        }
        let Some(node) = self.nodes.remove(id) else {
            return;
        };
        self.census.visuals_live -= 1;
        if let Some(painted) = node.painted {
            self.res.release(painted.mask.holds());
            self.res.release(painted.paint.holds());
        }
    }

    // ── values ────────────────────────────────────────────────────────────────────

    /// Records half of a sprite's declaration and realizes the chain if it moved.
    ///
    /// A mask and a paint arrive as separate ops in either order, so this records one and
    /// rebuilds from *both*; a half-declared sprite waits.
    ///
    /// **A declaration that changed nothing rebuilds nothing.** The emitter states a
    /// sprite's appearance without diffing it, so an unmoved control re-declares on every
    /// flush that touches it — which without this costs a mask brush, two cache lookups and
    /// a `set_brush` per sprite per frame at rest. The property shadow's idempotence, for
    /// the half of a sprite that is not a channel.
    fn declare(
        &mut self,
        id: SpriteId,
        mask: Option<(Mask, Dashes)>,
        paint: Option<Paint>,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        let node = id.node();
        // Resolved before anything is retained: a claim taken on behalf of a node that is
        // no longer live is a resource nothing will ever release.
        let Some(target) = self.nodes.get(node) else {
            return Ok(());
        };
        debug_assert!(
            target.kind == NodeKind::Sprite,
            "a mask or paint was addressed to a group"
        );
        let previous = target.painted.as_ref().map(Painted::declaration);

        // Retain before release, unconditionally: re-declaring the same resource must not
        // let its count touch zero on the way through. The test below decides whether to
        // rebuild, not whether the resource survives.
        if let Some((mask, _)) = mask {
            self.res.retain(mask.holds());
            self.res.release(previous.and_then(|(old, ..)| old.holds()));
        }
        if let Some(paint) = paint {
            self.res.retain(paint.holds());
            self.res.release(previous.and_then(|(.., old)| old.holds()));
        }

        let generation = self.generation;
        let Some(target) = self.nodes.get_mut(node) else {
            return Ok(());
        };
        let painted = target.painted.get_or_insert_with(|| Painted {
            combined: None,
            mask_brush: None,
            paint_brush: None,
            mask: Mask::None,
            // A sprite with no paint yet is transparent rather than a guessed colour.
            paint: Paint::Solid(Radiance::TRANSPARENT),
            dashes: Dashes::default(),
            route: Route::Clip,
            // Behind any generation that has ever moved, so a sprite declared before its
            // first realization cannot be mistaken for a fresh one.
            built_at: Gen::default(),
        });
        if let Some((mask, dashes)) = mask {
            painted.mask = mask;
            painted.dashes = dashes;
        }
        if let Some(paint) = paint {
            painted.paint = paint;
        }
        if previous == Some(painted.declaration()) && painted.fresh(generation) {
            self.census.props_skipped += 1;
            return Ok(());
        }
        self.rebind(id, back, env)
    }

    /// Establishes a node's clip.
    ///
    /// A clip is *declared*, so layout re-states it on every node it touches and most
    /// re-statements change nothing. Two shadows keep that free: the declaration itself,
    /// compared here, and the twelve channels, compared in [`prop::set`]. Only a change in
    /// which *object* occupies the slot can move a shape mask between its two
    /// constructions, so only that re-routes — a resize writes four sides and rebuilds
    /// nothing.
    fn set_clip(&mut self, id: NodeId, clip: Clip, back: &Backends, env: Env) -> Result<()> {
        let Some(node) = self.nodes.get_mut(id) else {
            return Ok(());
        };
        if node.declared_clip == clip {
            self.census.props_skipped += 1;
            return Ok(());
        }
        node.declared_clip = clip;

        if self.write_clip(id, clip, back)? {
            self.reroute_after_clip(id, back, env)?;
        }
        Ok(())
    }

    /// Re-realizes a shape mask after the clip slot changed hands.
    fn reroute_after_clip(&mut self, id: NodeId, back: &Backends, env: Env) -> Result<()> {
        let contested = self
            .nodes
            .get(id)
            .and_then(|n| n.painted.as_ref())
            .is_some_and(|p| matches!(p.mask, Mask::Shape { .. }));
        if contested {
            self.rebind(SpriteId(id), back, env)?;
        }
        Ok(())
    }

    /// Writes the clip, and answers whether a different kind of object now occupies the
    /// slot — which is the only thing a shape mask's route depends on.
    fn write_clip(&mut self, id: NodeId, clip: Clip, back: &Backends) -> Result<bool> {
        match clip {
            Clip::None => {
                let Some(node) = self.nodes.get_mut(id) else {
                    return Ok(false);
                };
                // Only what the *sink* established. A clip-route shape mask puts its
                // geometric clip straight on the visual without claiming this slot, and
                // clearing that here would tear the mask down and rebuild it every pass.
                if node.clip.is_none() {
                    return Ok(false);
                }
                node.visual.clear_clip();
                node.clip = None;
                Ok(true)
            }
            // The twelve values go through the one setter, one channel each. `set` already
            // refuses a channel a tracker holds, skips one the shadow says is unchanged,
            // and stops an animation before overwriting it. Re-stating those rules here is
            // how the two drift apart.
            Clip::Rect { l, t, r, b, radius } => {
                let minted = self.mint_rect_clip(id, back);
                let Some(node) = self.nodes.get_mut(id) else {
                    return Ok(minted);
                };
                let declared = [
                    (Prop::ClipL, l),
                    (Prop::ClipT, t),
                    (Prop::ClipR, r),
                    (Prop::ClipB, b),
                    (Prop::CornerTopLeftX, radius.tl),
                    (Prop::CornerTopLeftY, radius.tl),
                    (Prop::CornerTopRightX, radius.tr),
                    (Prop::CornerTopRightY, radius.tr),
                    (Prop::CornerBottomRightX, radius.br),
                    (Prop::CornerBottomRightY, radius.br),
                    (Prop::CornerBottomLeftX, radius.bl),
                    (Prop::CornerBottomLeftY, radius.bl),
                ];
                for (prop, value) in declared {
                    self.census
                        .count(prop::set(node, prop, Value::Scalar(value)));
                }
                Ok(minted)
            }
            Clip::Geom(geom) => {
                let Some(geometry) = self.res.geoms.value(geom).cloned() else {
                    return Ok(false);
                };
                let clip = back.compositor.create_geometric_clip(&geometry);
                let Some(node) = self.nodes.get_mut(id) else {
                    return Ok(false);
                };
                node.visual.set_clip(Some(&clip));
                // Soft, for an antialiased edge — the whole reason a geometric clip is
                // usable as a shape at all.
                node.visual.set_border_mode(BorderMode::Soft);
                let replaced = node.clip.is_none();
                node.clip = Some(ClipState::Geom(clip));
                Ok(replaced)
            }
        }
    }

    /// Mints a rectangle clip if the node has none, leaving an existing one alone.
    fn mint_rect_clip(&mut self, id: NodeId, back: &Backends) -> bool {
        if self
            .nodes
            .get(id)
            .and_then(|n| n.clip.as_ref())
            .is_some_and(|c| c.rect().is_some())
        {
            return false;
        }
        let clip = back.compositor.create_rectangle_clip();
        if let Some(node) = self.nodes.get_mut(id) {
            node.visual.set_clip(Some(&clip));
            // A fresh rectangle clip has every side at zero, which clips *everything* —
            // unlike an inset clip's harmless default. So it is seeded to the node's own
            // box, which is the identity a caller means by "a clip".
            let (w, h) = (node.core[2], node.core[3]);
            node.clip = Some(ClipState::Rect {
                clip,
                chans: [0.0, 0.0, w, h, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            });
            prop::write_group(node, 6);
        }
        true
    }

    // ── channels ──────────────────────────────────────────────────────────────────

    /// Retargets a channel from the front thread, inside the tick that decided to.
    ///
    /// The same property table, the same shadow and the same setter [`apply`](Scene::apply)
    /// uses — so this is not a second writer, it is the same writer reached from the other
    /// side. Nothing about it is speculative: `Model` is the app half and `SinkPatch` is
    /// filled there, so without this the router can resolve a hover and then has no way to
    /// move a pixel before the app thread next runs.
    ///
    /// A **fourth [`Bind`] variant was considered and rejected.** The obvious hazard — a
    /// front write desynchronising an app-side shadow — does not exist: `Model::bind` is a
    /// passthrough and the shadow is entirely front-side, on this thread. Declaring
    /// delegation on the wire would be state saying what the architecture already
    /// guarantees. What is left is the app and the router writing one channel, and that is
    /// caught by a debug-only claim rather than encoded in the alphabet.
    ///
    /// `env` is stated rather than held, for the reason every other entry point states it:
    /// a channel whose owner has to be minted first rasterizes, and a scene that cached the
    /// display could be *not told* when the window hops one.
    ///
    /// # Errors
    ///
    /// `bind` is [`Anim::Frames`]. A key-frame curve's frames live in a patch's own buffer
    /// and the front thread has no patch, so there is nothing to read them from — refused
    /// rather than played empty, because an animation that silently holds still is
    /// indistinguishable from one that was never started.
    pub fn retarget(
        &mut self,
        id: NodeId,
        prop: Prop,
        bind: Bind,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        if matches!(bind, Bind::Animate(Anim::Frames { .. })) {
            return Err(crate::invalid_arg());
        }
        #[cfg(debug_assertions)]
        self.front_owned.insert((id, prop));
        // Empty, and not a cost: every buffer on a fresh patch is an unallocated `Vec`, and
        // the one thing this path could read from one — a key-frame curve's frames — was
        // refused above.
        let patch = SinkPatch::new();
        self.sync(back, env)?;
        self.bind_channel(id, prop, bind, &patch, back, env)
    }

    /// Drops the claims of nodes that no longer exist.
    ///
    /// Called after a destroy, because a destroy cascades: the claim is keyed by node and
    /// only the tree knows which ones went with it. Ids are generational, so a surviving
    /// claim is never *wrong* — this keeps the set proportional to the screen rather than to
    /// the session, which is the difference between a debug aid and a debug leak.
    #[cfg(debug_assertions)]
    fn release_front_claims(&mut self) {
        let nodes = &self.nodes;
        self.front_owned.retain(|&(id, _)| nodes.get(id).is_some());
    }

    #[cfg(not(debug_assertions))]
    #[expect(
        clippy::unused_self,
        reason = "the debug twin edits the claim set; matching signatures is the point"
    )]
    const fn release_front_claims(&mut self) {}

    /// Fires where the app writes a channel [`retarget`](Scene::retarget) claimed, with both
    /// writers' identities in hand. Ids are generational, so a destroyed and reminted node
    /// does not inherit a claim.
    #[cfg(debug_assertions)]
    fn check_not_front_owned(&self, id: NodeId, prop: Prop) {
        assert!(
            !self.front_owned.contains(&(id, prop)),
            "{prop:?} on {id:?} is driven from the front thread, and the app has just \
             written it"
        );
    }

    #[cfg(not(debug_assertions))]
    #[expect(
        clippy::unused_self,
        reason = "the debug twin reads the claim set; matching signatures is the point"
    )]
    const fn check_not_front_owned(&self, _: NodeId, _: Prop) {}

    fn bind_channel(
        &mut self,
        id: NodeId,
        prop: Prop,
        bind: Bind,
        patch: &SinkPatch,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        let desc = prop::desc(prop);
        // The owner may not exist yet, and what that means differs per owner — see
        // `prop::absent`.
        if !self
            .nodes
            .get(id)
            .is_some_and(|n| prop::has_owner(n, desc.owner))
        {
            match prop::absent(desc.owner) {
                Absent::MintClip => {
                    if self.mint_rect_clip(id, back) {
                        self.reroute_after_clip(id, back, env)?;
                    }
                }
                Absent::Promote => {
                    if !self.promote(id, back, env)? {
                        debug_assert!(
                            false,
                            "a shape channel was bound on a sprite with no shape mask"
                        );
                        return Ok(());
                    }
                }
                Absent::Refuse => {
                    debug_assert!(
                        false,
                        "a channel was bound before the object that carries it existed"
                    );
                    return Ok(());
                }
            }
        }

        match bind {
            Bind::Set(value) => {
                let written = self
                    .nodes
                    .get_mut(id)
                    .is_some_and(|node| prop::set(node, prop, value));
                if written {
                    self.resize_captures(id, prop, env);
                }
                self.census.count(written);
            }
            Bind::Animate(anim) => self.animate(id, prop, anim, patch, back),
            Bind::Track {
                tracker,
                axis,
                affine,
            } => self.track_channel(id, prop, tracker, axis, affine),
            Bind::Stop => self.stop(id, prop),
        }
        Ok(())
    }

    /// Brings a node's captures up to date with the box it now occupies.
    ///
    /// A capture states a region in the source's own space, so it is the one realized thing
    /// that does not follow its sprite: a shape or a glow whose box moved keeps describing
    /// the old one and draws at the wrong scale. Correcting it is **three property sets and
    /// no re-tessellation** — the geometry object is untouched, no verbs cross the seam, and
    /// the app thread is not involved. That is what makes a live window-edge drag over a
    /// screen of paths cost what a resize should cost.
    ///
    /// Only a size can invalidate one, so this asks before it looks: a move, an opacity and
    /// a rotation all land through the same setter and none of them changes the region.
    fn resize_captures(&mut self, id: NodeId, prop: Prop, env: Env) {
        if !matches!(prop, Prop::Size | Prop::SizeX | Prop::SizeY) {
            return;
        }
        let scale = env.scale();
        let Some(node) = self.nodes.get_mut(id) else {
            return;
        };
        let size = node.size();
        if let Some(shape) = node.shape.as_ref() {
            shape.host.set_size(size.x, size.y);
            shape.captured.resize(size, scale);
        }
        if let Some(shadow) = node.shadow.as_ref() {
            shadow.captured.resize(size, scale);
        }
    }

    fn animate(&mut self, id: NodeId, prop: Prop, anim: Anim, patch: &SinkPatch, back: &Backends) {
        let desc = prop::desc(prop);
        let Some(animation) = self.animation(id, desc, anim, patch, back) else {
            return;
        };
        if let Some(node) = self.nodes.get_mut(id) {
            prop::start(node, desc, &animation, Held::Playing);
            self.census.animations += 1;
        }
    }

    /// Builds the animation an [`Anim`] describes, against the shared templates.
    fn animation(
        &self,
        id: NodeId,
        desc: &prop::PropDesc,
        anim: Anim,
        patch: &SinkPatch,
        back: &Backends,
    ) -> Option<CompositionAnimation> {
        let templates = &self.motion.templates;
        match anim {
            Anim::Spring { to, tuning, .. } => {
                // The travel is measured from the shadow, which is why a caller states a
                // tuning and never a period.
                let travel = self.nodes.get(id).map_or(0.0, |n| travel_of(n, desc, to));
                Some(match desc.anim {
                    prop::Slot::Scalar => {
                        let Value::Scalar(v) = to else { return None };
                        templates.spring_scalar(tuning, v, travel).as_animation()
                    }
                    prop::Slot::Vec2 => {
                        let Value::Vec2(v) = to else { return None };
                        templates.spring_vec2(tuning, v, travel).as_animation()
                    }
                    prop::Slot::Vec3 => {
                        let Value::Vec2(v) = to else { return None };
                        let z = if desc.group == 2 { 1.0 } else { 0.0 };
                        let to = Vector3 { x: v.x, y: v.y, z };
                        templates.spring_vec3(tuning, to, travel).as_animation()
                    }
                })
            }
            Anim::Frames {
                frames,
                duration_ms,
                iterations,
            } => templates.frames(
                &back.compositor,
                patch.frames(frames),
                duration_ms,
                iterations,
            ),
        }
    }

    fn track_channel(
        &mut self,
        id: NodeId,
        prop: Prop,
        tracker: TrackerId,
        axis: TrackerAxis,
        affine: Affine,
    ) {
        let desc = prop::desc(prop);
        let Some(state) = self.trackers.get(tracker.raw) else {
            return;
        };
        let expression = self
            .motion
            .templates
            .track(axis, &state.tracker, affine.m, affine.c)
            .as_animation();
        if let Some(node) = self.nodes.get_mut(id) {
            // Permanent: nothing but an explicit stop leaves `Bound`.
            prop::start(node, desc, &expression, Held::Bound);
            self.census.animations += 1;
        }
    }

    fn stop(&mut self, id: NodeId, prop: Prop) {
        if let Some(node) = self.nodes.get_mut(id) {
            prop::stop(node, prop::desc(prop));
        }
    }

    /// Rebuilds a clip-route shape onto the capture, keeping the same geometry.
    ///
    /// Returns whether there was a shape mask to promote at all.
    fn promote(&mut self, id: NodeId, back: &Backends, env: Env) -> Result<bool> {
        let has_shape = self
            .nodes
            .get(id)
            .and_then(|n| n.painted.as_ref())
            .is_some_and(|p| matches!(p.mask, Mask::Shape { .. }));
        if !has_shape {
            return Ok(false);
        }
        // The shape state does not exist yet, so `draws_on` cannot see the channel about to
        // be bound. Seeding it makes the route function total: the rebind then observes live
        // channels and takes the capture.
        if let Some(node) = self.nodes.get_mut(id) {
            prop::set_held(node, prop::desc(Prop::TrimEnd).group, Held::Stale);
        }
        self.rebind(SpriteId(id), back, env)?;
        Ok(true)
    }

    // ── resources ─────────────────────────────────────────────────────────────────

    fn apply_res(
        &mut self,
        id: ResId,
        op: ResOp,
        patch: &SinkPatch,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        match op {
            // Re-pointing the *held* geometry, not replacing it: every sprite sharing this
            // id reads the same object, so fill, stroke and glow move together and any trim
            // already running survives.
            ResOp::Geom { verbs } => {
                let path = back.path(patch.verbs(verbs))?;
                match self.res.geoms.get_mut(id.cast::<Geom>()) {
                    Some(res) => res.value.set_path(&path),
                    None => self.res.geoms.insert(
                        id.cast::<Geom>(),
                        Res::new(back.compositor.create_path_geometry(&path)),
                    ),
                }
            }
            ResOp::Ramp { stops, axis } => {
                if let Some(surface) = back.raster_ramp(env, patch.stops(stops), axis)? {
                    self.res.ramps.point(id.cast::<Ramp>(), &surface, || {
                        back.brush(&surface, Stretch::Fill)
                    });
                }
            }
            ResOp::Run { segs, ink } => {
                let run = back.raster_run(env, patch.segs(segs), patch.glyphs(), ink)?;
                if let Some(surface) = run {
                    // A tile's pixel extent is its `Ink` at the current scale, so a
                    // sprite sized to that same `Ink` samples one texel per physical pixel
                    // and nothing resamples. Fill is what makes that identity hold at every
                    // scale; a natural-size stretch would map a texel to a DIP.
                    self.res.runs.point(id.cast::<Run>(), &surface, || {
                        back.brush(&surface, Stretch::Fill)
                    });
                }
            }
            ResOp::Region => {
                if self.res.regions.get(id.cast::<Region>()).is_none() {
                    self.res.regions.insert(id.cast::<Region>(), Res::new(None));
                }
            }
            // Only the model's own claim — a sprite still painting with it keeps it alive
            // until its own destroy or re-declare, which is the whole point of counting.
            ResOp::Drop => self.res.disclaim(id),
        }
        Ok(())
    }

    fn apply_tracker(&mut self, id: Id<Tracker>, op: TrackerOp) {
        let Some(state) = self.trackers.get_mut(id) else {
            return;
        };
        match op {
            TrackerOp::Bounds { min, max } => state.tracker.set_position_bounds(
                Vector3 {
                    x: min.x,
                    y: min.y,
                    z: 0.0,
                },
                Vector3 {
                    x: max.x,
                    y: max.y,
                    z: 0.0,
                },
            ),
            TrackerOp::Decay(rate) => {
                state
                    .tracker
                    .set_position_inertia_decay_rate(rate.map(|r| Vector3 {
                        x: r.x,
                        y: r.y,
                        z: 0.0,
                    }));
            }
            TrackerOp::Drop => {
                // The viewport this tracker was scrolling, so the hit query stops resolving
                // that node's descendants through an offset nothing updates any more.
                let viewport = state.viewport;
                self.trackers.remove(id);
                if let Some(node) = viewport {
                    self.hits.clear_scroll(node);
                }
            }
        }
    }
}

/// How far a spring has to travel, from the shadow's current value to its target.
fn travel_of(node: &Node, desc: &prop::PropDesc, to: Value) -> f32 {
    let at = desc.chan as usize;
    let current = |slot: usize| node.core.get(slot).copied().unwrap_or(0.0);
    match (desc.owner, to) {
        (prop::Owner::Visual, Value::Scalar(v)) => (v - current(at)).abs(),
        (prop::Owner::Visual, Value::Vec2(v)) => {
            let (dx, dy) = (v.x - current(at), v.y - current(at + 1));
            (dx * dx + dy * dy).sqrt()
        }
        // A channel on a clip, a shape or a glow moves in its own units — a trim fraction,
        // a blur sigma — so there is no travel in DIPs to scale a period by.
        _ => 0.0,
    }
}
