use super::*;

pub(super) struct Lift {
    node: NodeId,
    source: Visual,
    parent: ContainerVisual,
    carrier: ContainerVisual,
    previous: Option<Visual>,
    previous_id: Option<NodeId>,
    offset: Vector2,
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
        self.lift = Some(Box::new(Lift {
            node, source, parent, carrier, previous, previous_id, offset: Vector2::zero(),
        }));
        true
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
        match lift.previous {
            Some(previous) => children.insert_above(&lift.source, &previous),
            None => children.insert_at_bottom(&lift.source),
        }
        let _ = self.overlay.children().try_remove(&lift.carrier);
        self.census.visuals_live -= 1;
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
