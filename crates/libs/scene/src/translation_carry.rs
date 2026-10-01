//! Native translation continuity across replacement of a retained subtree.

use super::*;

/// Retains a node's native translation until its replacement is installed.
/// The handle belongs to its originating scene and carries no CPU presentation sample.
pub struct TranslationCarry {
    node: NodeId,
    pub(super) visual: Visual,
    target: Vector2,
}

impl Scene {
    /// Detaches animated translation channels before `patch` retires their subtree.
    /// `patch` must be applied next in the same scene pass. Only removals without an
    /// exit transition are eligible. Unanimated nodes and disabled animation are declined.
    pub fn hold_translation(&self, node: NodeId, patch: &SinkPatch) -> Option<TranslationCarry> {
        let visual = self.nodes.visual(node)?;
        if !self.springs_enabled || ![Prop::TranslationX, Prop::TranslationY].into_iter()
            .any(|prop| self.nodes.held(node, desc(prop)) == Held::Playing)
        { return None; }
        let target = Vector2::new(self.nodes.chan(node, desc(Prop::TranslationX).chan),
            self.nodes.chan(node, desc(Prop::TranslationY).chan));
        if !target.x.is_finite() || !target.y.is_finite()
            || [Prop::TranslationX, Prop::TranslationY].into_iter()
                .any(|prop| self.nodes.held(node, desc(prop)) == Held::Bound)
        { return None; }
        let mut ancestor = node;
        loop {
            if let Some(exit) = patch.ops().iter().find_map(|op| match op {
                Op::Drop { id, exit, .. } if *id == ancestor => Some(*exit), _ => None,
            }) {
                if exit != Exit::None { return None; }
                break;
            }
            let parent = self.nodes.links(ancestor.index() as u32).parent;
            if parent == NO_LINK { return None; }
            ancestor = self.nodes.id_at(parent);
        }
        // Retiring an ancestor closes native descendants even when a COM handle remains.
        // The detached root keeps its channels through the patch's native tree retirement.
        if let Some(parent) = visual.parent() { parent.children().try_remove(visual).ok()?; }
        Some(TranslationCarry { node, visual: visual.clone(), target })
    }

    /// Continues a retired node's translation on its replacement until native completion.
    /// `carry` must originate in this scene. The replacement must occupy the old node's
    /// target position, in the same coordinate space, with zero target translation and
    /// no permanent translation binding.
    /// Live sources, retired replacements and disabled animation are declined.
    pub fn continue_translation(&mut self, carry: TranslationCarry, node: NodeId, back: &Backends) -> Result<()> {
        if !self.springs_enabled || self.nodes.live(carry.node) || !self.nodes.live(node) {
            return Ok(());
        }
        if [Prop::TranslationX, Prop::TranslationY].into_iter()
            .any(|prop| self.nodes.chan(node, desc(prop).chan) != 0.0
                || self.nodes.held(node, desc(prop)) == Held::Bound)
        { return Ok(()); }
        for prop in [Prop::TranslationX, Prop::TranslationY] {
            self.cancel_translation_carry(node, prop);
        }
        // Only the retired root's channels survive. Its rendered descendants have new owners.
        if let Some(container) = carry.visual.as_container() { container.children().remove_all(); }
        carry.visual.clear_clip();
        if let Some(sprite) = carry.visual.as_sprite() {
            sprite.clear_brush();
            sprite.clear_shadow();
        }
        let source = carry.visual.clone();
        let target = carry.target;
        let nodes = &mut self.nodes;
        let result = self.motion.watch(back, PendingKind::TranslationCarry { node, axes: 3, source: carry }, |templates| {
            for (prop, value, expression) in [
                (Prop::TranslationX, target.x, "source.TransformMatrix._41 - target"),
                (Prop::TranslationY, target.y, "source.TransformMatrix._42 - target"),
            ] {
                let row = desc(prop);
                let spring = templates.spring(1, Tuning::Chrome, Value::Scalar(value), value, Duration::ZERO);
                source.start_animation(row.path, &spring);
                let follow = back.compositor.create_expression_animation(expression);
                follow.set_reference_parameter("source", &source);
                follow.set_scalar_parameter("target", value);
                nodes.start(node, row, &follow.as_animation(), Some(Value::Scalar(0.0)), Held::Playing);
            }
        });
        if let Err(error) = result {
            for prop in [Prop::TranslationX, Prop::TranslationY] {
                self.nodes.stop(node, desc(prop));
                self.census.count(self.nodes.set(node, prop, Value::Scalar(0.0)));
            }
            return Err(error);
        }
        self.census.visuals_live += 1;
        self.census.animations += 4;
        Ok(())
    }

    pub(super) fn cancel_translation_carry(&mut self, node: NodeId, prop: Prop) {
        let axis = match prop { Prop::TranslationX => 1, Prop::TranslationY => 2, _ => return };
        for pending in &mut self.motion.pending {
            if let PendingKind::TranslationCarry { node: held, axes, .. } = &mut pending.holds {
                if *held == node { *axes &= !axis; }
            }
        }
        self.pending_retain(|pending| !matches!(pending.holds, PendingKind::TranslationCarry { axes: 0, .. }));
    }
}
