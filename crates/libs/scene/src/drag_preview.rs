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
    scroll: Option<CompositionPropertySet>,
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
    /// brushes, text tiles, channels and arena ownership remain unchanged. `owner` must name
    /// the subtree's hit root; its descendants share the lift's clip and paint order.
    /// A visual-only preview uses `ControlId::NONE`. The input owner retains capture.
    /// Window/overlay roots and empty or retired nodes are declined. Only one preview
    /// may be live in a scene. Beginning another restores the first.
    pub fn begin_drag_preview(&mut self, node: NodeId, owner: ControlId, back: &Backends) -> bool {
        self.end_drag_preview();
        let Some(source) = self.nodes.visual(node).cloned() else { return false };
        let Some(parent) = source.parent() else { return false };
        let size = self.nodes.size(node);
        if size.x <= 0.0 || size.y <= 0.0 { return false; }
        let links = self.nodes.links(node.index() as u32);
        if links.parent == NO_LINK { return false; }
        let prev = links.prev;
        // The sibling the source is restored above is the nearest one in its own band: a
        // split group holds its chrome and its content in two collections, and the chain's
        // previous sibling can be in the other. That sibling is the one watched, since it
        // is the one whose removal would leave the restore nowhere to go.
        let previous_id = self.nodes.below_in_band(
            self.nodes.id_at(links.parent),
            (prev != NO_LINK).then(|| self.nodes.id_at(prev)),
            self.nodes.is_chrome(node),
        );
        let previous = previous_id.and_then(|id| self.nodes.visual(id)).cloned();
        let carrier = back.compositor.create_container_visual();
        carrier.set_parent_for_transform(&parent);
        carrier.set_pixel_snapping(true);
        // The parent node's own visual, which a split group's content carrier shares its
        // space with but whose `Size` is the one that holds the extent.
        let sized = self.nodes.visual(self.nodes.id_at(links.parent)).cloned().unwrap_or_else(|| (*parent).clone());
        let extent = back.compositor.create_expression_animation("parent.Size");
        extent.set_reference_parameter("parent", &sized);
        carrier.start_animation("Size", &extent);
        parent.children().try_remove(&source).expect("the source belongs to its parent");
        carrier.children().insert_at_top(&source);
        self.overlay.children().insert_at_top(&carrier);
        self.census.visuals_minted += 1;
        self.census.visuals_live += 1;
        self.hits.lift(owner);
        self.lift_epoch = self.lift_epoch.checked_add(1).expect("drag preview epoch exhausted");
        self.lift = Some(Box::new(Lift {
            epoch: self.lift_epoch, node, source, parent, carrier, previous, previous_id,
            offset: Vector2::zero(),
            scroll: None,
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
        let links = self.nodes.links(lift.node.index() as u32);
        let sized = (links.parent != NO_LINK)
            .then(|| self.nodes.visual(self.nodes.id_at(links.parent)).cloned())
            .flatten()
            .unwrap_or_else(|| (*lift.parent).clone());
        let extent = back.compositor.create_expression_animation("parent.Size");
        extent.set_reference_parameter("parent", &sized);
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
        if let Some(properties) = &lift.scroll {
            properties.insert_vector3("PointerOffset", v3(by));
        } else {
            lift.carrier.set_offset(by.x, by.y, 0.0);
        }
        self.census.props_written += 1;
    }

    /// Keeps the lifted subtree under its pointer while `viewport` scrolls.
    /// The compensation runs in the compositor and lasts until the preview ends.
    /// A preview accepts one viewport; an absent tracker leaves it unchanged.
    pub fn follow_drag_scroll(&mut self, viewport: NodeId, back: &Backends) {
        let Some(lift) = self.lift.as_mut().filter(|lift| lift.scroll.is_none()) else { return; };
        let Some((_, tracker)) = self.trackers.iter().find(|(_, state)| state.viewport == viewport) else { return; };
        let properties = lift.carrier.properties();
        properties.insert_vector3("PointerOffset", v3(lift.offset));
        let animation = back.compositor.create_expression_animation(
            "pointer.PointerOffset + tracker.Position - origin");
        animation.set_reference_parameter("pointer", &properties);
        animation.set_reference_parameter("tracker", &tracker.inner);
        animation.set_vector3_parameter("origin", v3(tracker.position));
        lift.carrier.start_animation("Offset", &animation);
        lift.scroll = Some(properties);
        self.census.animations += 1;
    }

    /// Restores the lifted subtree's native parent and paint order.
    pub fn end_drag_preview(&mut self) {
        let Some(lift) = self.lift.take() else { return; };
        self.motion.pending.retain(|pending| !matches!(pending.holds,
            PendingKind::DragLanding(epoch) if epoch == lift.epoch));
        self.hits.lift(ControlId::NONE);
        if lift.scroll.is_some() { lift.carrier.stop_animation("Offset"); }
        lift.carrier.children().remove_all();
        if let Some(placeholder) = lift.placeholder {
            drop(placeholder);
            self.census.visuals_live -= 2;
        }
        // Resolved again rather than read from the lift: a clip or chrome that arrived
        // during the drag can have split the parent, moving its content into a carrier.
        let links = self.nodes.links(lift.node.index() as u32);
        let parent = (links.parent != NO_LINK).then(|| self.nodes.id_at(links.parent));
        let chrome = self.nodes.is_chrome(lift.node);
        let children = parent
            .and_then(|parent| self.nodes.band(parent, chrome))
            .unwrap_or_else(|| lift.parent.children());
        let previous = match parent {
            Some(parent) => self
                .nodes
                .below_in_band(parent, lift.previous_id, chrome)
                .and_then(|id| self.nodes.visual(id).cloned()),
            None => lift.previous,
        };
        match previous {
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

    /// Lands a replacement subtree from `by` DIPs away using the shared chrome springs.
    /// Input resolves at the target. Completion restores the native parent, and a new
    /// preview or structural invalidation cancels the landing. Zero travel parks directly.
    pub fn land_drag_preview(&mut self, node: NodeId, by: Vector2, back: &Backends) -> Result<()> {
        if !self.springs_enabled || by == Vector2::zero() || !by.x.is_finite() || !by.y.is_finite() {
            return Ok(());
        }
        if !self.begin_drag_preview(node, ControlId::NONE, back) { return Ok(()); }
        self.move_drag_preview(by);
        let lift = self.lift.as_ref().unwrap();
        let epoch = lift.epoch;
        let carrier = lift.carrier.clone();
        let result = self.motion.watch(back, PendingKind::DragLanding(epoch), |templates| {
            for (property, travel) in [("Offset.X", by.x), ("Offset.Y", by.y)] {
                if travel == 0.0 { continue; }
                let animation = templates.spring(1, Tuning::Chrome, Value::Scalar(0.0), travel, Duration::ZERO);
                carrier.start_animation(property, &animation);
            }
        });
        if let Err(error) = result { self.end_drag_preview(); return Err(error); }
        self.census.animations += u64::from(by.x != 0.0) + u64::from(by.y != 0.0);
        Ok(())
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
