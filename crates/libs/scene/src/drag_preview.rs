use super::*;

pub(super) struct Lift {
    epoch: u64,
    node: NodeId,
    source: Visual,
    parent: ContainerVisual,
    carrier: ContainerVisual,
    previous: Option<Visual>,
    previous_id: Option<NodeId>,
    offset: Vector2,
    placeholder: Option<Placeholder>,
}

struct Placeholder {
    visual: SpriteVisual,
    content: ContainerVisual,
    origin: Vector3,
    offset: Vector2,
}

impl Drop for Placeholder {
    fn drop(&mut self) {
        // Removing the sprite breaks parent -> capture -> carrier -> transform-parent.
        if let Some(parent) = self.visual.parent() {
            let _ = parent.children().try_remove(&self.visual);
        }
        self.content.children().remove_all();
    }
}

impl Scene {
    /// Lifts a retained subtree into the overlay band until `end_drag_preview`.
    ///
    /// One carrier preserves the original parent's transform and size. The subtree's
    /// brushes, text tiles, channels and arena ownership remain unchanged. Its original
    /// hit geometry remains the drop-target geometry; the input owner retains capture.
    /// Window/overlay roots and empty or retired nodes are declined. Only one preview
    /// may be live in a scene. Beginning another restores the first.
    pub fn begin_drag_preview(&mut self, node: NodeId, back: &Backends) -> bool {
        self.end_drag_preview();
        let Some(source) = self.nodes.visual(node).cloned() else { return false };
        let Some(parent) = source.parent() else { return false };
        let size = self.nodes.size(node);
        if size.x <= 0.0 || size.y <= 0.0 { return false; }
        let links = self.nodes.links(node.index() as u32);
        if links.parent == NO_LINK { return false; }
        let prev = links.prev;
        let previous_id = (prev != NO_LINK).then(|| self.nodes.id_at(prev));
        let previous = previous_id.and_then(|id| self.nodes.visual(id)).cloned();
        let carrier = back.compositor.create_container_visual();
        carrier.set_parent_for_transform(&parent);
        carrier.set_pixel_snapping(true);
        let extent = back.compositor.create_expression_animation("parent.Size");
        extent.set_reference_parameter("parent", &*parent);
        carrier.start_animation("Size", &extent);
        parent.children().try_remove(&source).expect("the source belongs to its parent");
        carrier.children().insert_at_top(&source);
        self.overlay.children().insert_at_top(&carrier);
        self.census.visuals_minted += 1;
        self.census.visuals_live += 1;
        self.lift_epoch = self.lift_epoch.checked_add(1).expect("drag preview epoch exhausted");
        self.lift = Some(Box::new(Lift {
            epoch: self.lift_epoch, node, source, parent, carrier, previous, previous_id,
            offset: Vector2::zero(),
            placeholder: None,
        }));
        true
    }

    /// Shows a retained copy at the lifted subtree's original slot.
    /// The copy has no input identity and lives until the preview ends.
    /// The source must have axis-aligned layout bounds without an authored scale.
    pub fn show_drag_placeholder(&mut self, opacity: f32, back: &Backends) {
        let Some(lift) = self.lift.as_mut() else { return; };
        if lift.placeholder.is_some() || !opacity.is_finite() { return; }
        let size = self.nodes.size(lift.node);
        let origin = lift.source.offset();
        let scale = self.env.map_or(1.0, Env::scale);
        let content = back.compositor.create_container_visual();
        // Capture ignores its root transform. Scaling inside that root rasterizes
        // text at device resolution; the reciprocal outer scale preserves the lift.
        content.set_scale(Vector3::new(scale, scale, 1.0));
        let extent = back.compositor.create_expression_animation("parent.Size");
        extent.set_reference_parameter("parent", &*lift.parent);
        content.start_animation("Size", &extent);
        lift.carrier.set_scale(Vector3::new(1.0 / scale, 1.0 / scale, 1.0));
        lift.carrier.children().remove_all();
        content.children().insert_at_top(&lift.source);
        lift.carrier.children().insert_at_top(&content);
        let capture = back.compositor.capture(&lift.carrier, size, scale);
        capture.surface.set_source_offset(Vector2::new(origin.x * scale, origin.y * scale));
        let visual = back.compositor.create_sprite_visual();
        visual.set_brush(&capture.brush);
        visual.set_size(size.x, size.y);
        visual.set_offset(origin.x, origin.y, origin.z);
        visual.set_opacity(opacity.clamp(0.0, 1.0));
        visual.set_pixel_snapping(true);
        match &lift.previous {
            Some(previous) => lift.parent.children().insert_above(&visual, previous),
            None => lift.parent.children().insert_at_bottom(&visual),
        }
        lift.placeholder = Some(Placeholder { visual, content, origin, offset: Vector2::zero() });
        self.census.visuals_minted += 2;
        self.census.visuals_live += 2;
    }

    /// Moves the placeholder between slots using the shared chrome springs.
    /// Displacement is relative to the source slot, in its parent's DIPs.
    /// Unchanged and nonfinite targets issue no native writes.
    pub fn move_drag_placeholder(&mut self, by: Vector2) {
        if !by.x.is_finite() || !by.y.is_finite() { return; }
        let Some(p) = self.lift.as_mut().and_then(|l| l.placeholder.as_mut()) else { return; };
        if p.offset == by { return; }
        for (property, before, after, origin) in [
            ("Offset.X", p.offset.x, by.x, p.origin.x),
            ("Offset.Y", p.offset.y, by.y, p.origin.y),
        ] {
            if before == after { continue; }
            if self.springs_enabled {
                let animation = self.motion.templates.spring(1, Tuning::Chrome,
                    Value::Scalar(origin + after), after - before, Duration::ZERO);
                p.visual.start_animation(property, &animation);
                self.census.animations += 1;
            }
        }
        if !self.springs_enabled {
            p.visual.set_offset(p.origin.x + by.x, p.origin.y + by.y, p.origin.z);
        }
        p.offset = by;
        self.census.props_written += 1;
    }

    /// Moves the lifted subtree by a cumulative displacement in its parent's DIPs.
    /// Nonfinite samples are ignored. Repeated positions issue no native writes.
    pub fn move_drag_preview(&mut self, by: Vector2) {
        if !by.x.is_finite() || !by.y.is_finite() { return; }
        let Some(lift) = self.lift.as_mut() else { return; };
        if lift.offset == by { return; }
        lift.offset = by;
        lift.carrier.set_offset(by.x, by.y, 0.0);
        self.census.props_written += 1;
    }

    /// Restores the lifted subtree's native parent and paint order.
    pub fn end_drag_preview(&mut self) {
        let Some(lift) = self.lift.take() else { return; };
        lift.carrier.children().remove_all();
        let children = lift.parent.children();
        if let Some(placeholder) = lift.placeholder {
            drop(placeholder);
            self.census.visuals_live -= 2;
        }
        match lift.previous {
            Some(previous) => children.insert_above(&lift.source, &previous),
            None => children.insert_at_bottom(&lift.source),
        }
        let _ = self.overlay.children().try_remove(&lift.carrier);
        self.census.visuals_live -= 1;
    }

    /// Returns the live preview's identity for the application acknowledgement.
    pub fn drag_preview_epoch(&self) -> Option<u64> {
        self.lift.as_ref().map(|lift| lift.epoch)
    }

    /// Restores a released preview after its application's scene patch has applied.
    /// Acknowledgements for an earlier gesture cannot restore a newer preview.
    pub fn finish_drag_preview(&mut self, epoch: u64) {
        if self.drag_preview_epoch() == Some(epoch) { self.end_drag_preview(); }
    }

    pub(super) fn preview_before(&mut self, op: Op) {
        let Some(lift) = self.lift.as_ref() else { return; };
        let ancestor = |id: NodeId| {
            let mut at = lift.node;
            loop {
                if at == id { return true; }
                let parent = self.nodes.links(at.index() as u32).parent;
                if parent == NO_LINK { return false; }
                at = self.nodes.id_at(parent);
            }
        };
        // Structural changes can retire the saved parent or sibling. Geometry writes
        // on the source ancestry invalidate the drag's original placement.
        let invalid = match op {
            Op::Drop { id, .. } => ancestor(id) || Some(id) == lift.previous_id,
            Op::Move { id, after, .. } => ancestor(id) || Some(id) == lift.previous_id || after == Some(lift.node),
            Op::New { after, .. } => after == Some(lift.node),
            Op::Bind { id, prop, .. } => ancestor(id) && matches!(prop,
                Prop::Offset | Prop::OffsetX | Prop::OffsetY | Prop::Size |
                Prop::SizeX | Prop::SizeY | Prop::Scale | Prop::ScaleX | Prop::ScaleY |
                Prop::RotationAngle | Prop::Center | Prop::CenterX | Prop::CenterY |
                Prop::AnchorX | Prop::AnchorY),
            _ => false,
        };
        if invalid { self.end_drag_preview(); }
    }
}
