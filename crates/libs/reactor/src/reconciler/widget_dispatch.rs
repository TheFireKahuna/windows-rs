use super::*;

/// A trailing canvas inset as a prop value. `None` means the child never asked to
/// fill that axis, and must be told so explicitly — a silently-omitted write would
/// leave the last child's inset in place on a recycled control.
fn canvas_edge(o: Option<CanvasOffset>) -> PropValue {
    o.map_or(PropValue::Unset, PropValue::CanvasOffset)
}

impl<B: Backend + 'static> Reconciler<B> {
    pub fn mount_widget(&mut self, w: &dyn Widget) -> ControlId {
        let id = self.acquire_control(w.kind());
        self.apply_props(id, &w.bindings());
        self.apply_modifiers(id, w.modifiers());
        self.apply_attached(id, w.attached());
        self.mount_widget_children(id, w.children());
        // Mounted, never appended: a slot's subtree is a parentless root the
        // backend places itself (a popup, a pane region), so it takes no part in
        // this node's layout and appears in no child list.
        for slot in Slot::ALL {
            if let Some(el) = w.slot_element(slot)
                && let Some(root) = self.mount(el)
            {
                self.attach_slot(id, slot, Some(root));
            }
        }
        if let Some(cb) = w.on_mounted_callback() {
            let native = self.backend.get_native_element(id);
            cb.invoke(MountInfo { id, native });
        }
        if let Some(cb) = w.on_unmounted_callback() {
            self.unmount_callbacks.insert(id, cb.clone());
        }
        id
    }

    pub fn update_widget(&mut self, old: &dyn Widget, new: &dyn Widget, id: ControlId) {
        self.diff_props(id, new.kind(), &old.bindings(), &new.bindings());
        self.diff_modifiers(id, old.modifiers(), new.modifiers());
        self.diff_attached(id, old.attached(), new.attached());
        self.update_widget_children(id, old.children(), new.children());
        for slot in Slot::ALL {
            self.update_slot(id, slot, old.slot_element(slot), new.slot_element(slot));
        }
        if let Some(cb) = new.on_unmounted_callback() {
            self.unmount_callbacks.insert(id, cb.clone());
        } else {
            self.unmount_callbacks.remove(&id);
        }
    }

    fn mount_widget_children(&mut self, id: ControlId, children: Children<'_>) {
        match children {
            Children::None => {}
            Children::PositionalSingle(child) => {
                if let Some(child_id) = self.mount(child) {
                    self.append_child_tracked(id, child_id);
                }
            }
            Children::Keyed(list) => {
                for child in collect_live(list) {
                    if let Some(child_id) = self.mount(child) {
                        self.append_child_tracked(id, child_id);
                    }
                }
            }
            Children::Tabs(tabs) => {
                for tab in tabs {
                    self.mount_tab_item(id, tab);
                }
            }
            Children::PivotItems(items) => {
                for item in items {
                    self.mount_pivot_item(id, item);
                }
            }
        }
    }

    fn update_widget_children(&mut self, id: ControlId, old: Children<'_>, new: Children<'_>) {
        match (old, new) {
            (Children::None, Children::None) => {}
            (Children::PositionalSingle(o), Children::PositionalSingle(n)) => {
                let oc = std::slice::from_ref(o);
                let nc = std::slice::from_ref(n);
                self.reconcile_children_positional(id, oc, nc);
            }
            (Children::Keyed(o), Children::Keyed(n)) => {
                self.reconcile_children(id, o, n);
            }
            (Children::Tabs(o), Children::Tabs(n)) => {
                self.reconcile_tabs(id, o, n);
            }
            (Children::PivotItems(o), Children::PivotItems(n)) => {
                self.reconcile_pivot_items(id, o, n);
            }
            _ => {
                debug_assert!(
                    false,
                    "update_widget_children: child-layout shape changed across update"
                );
            }
        }
    }

    /// One slot, reconciled: mounted, updated in place, or torn down.
    ///
    /// In place whenever the element kind matches, so a slot that is showing while
    /// its owner re-renders — an open flyout, most visibly — keeps its focus and
    /// its state instead of being rebuilt underneath the user.
    ///
    /// Generic over [`Slot`] on purpose. This logic was previously written out once
    /// per slot kind, and the copies drifted: the flyout one was missing from
    /// teardown entirely, which leaked a whole subtree per unmount. One
    /// implementation cannot drift from itself.
    fn update_slot(
        &mut self,
        id: ControlId,
        slot: Slot,
        old: Option<&Element>,
        new: Option<&Element>,
    ) {
        match (old, new) {
            (None, None) => {}
            (None, Some(el)) => {
                if let Some(root) = self.mount(el) {
                    self.attach_slot(id, slot, Some(root));
                }
            }
            (Some(_), None) => {
                if let Some(root) = self.slot_root(id, slot) {
                    self.attach_slot(id, slot, None);
                    self.unmount(root);
                }
            }
            (Some(old_el), Some(new_el)) => {
                if let Some(root) = self.slot_root(id, slot) {
                    if old_el.kind_matches(new_el) {
                        match self.update(old_el, new_el, root) {
                            // A replacement control: the backend must be pointed at
                            // the new root before the old id goes stale.
                            Some(new_root) if new_root != root => {
                                self.attach_slot(id, slot, Some(new_root));
                            }
                            None => self.attach_slot(id, slot, None),
                            _ => {}
                        }
                        return;
                    }
                    // Kind changed: the control cannot be reused. Drop the
                    // bookkeeping before unmounting so the teardown walk does not
                    // see a root that is already being destroyed, then mount fresh.
                    self.put_slot_root(id, slot, None);
                    self.unmount(root);
                }
                if let Some(root) = self.mount(new_el) {
                    self.attach_slot(id, slot, Some(root));
                }
            }
        }
    }

    /// The root currently mounted in `slot`, if any.
    fn slot_root(&self, id: ControlId, slot: Slot) -> Option<ControlId> {
        self.slot_roots.get(&id).and_then(|roots| roots[slot.index()])
    }

    /// Record (or clear) `slot`'s root without telling the backend — the teardown
    /// paths that destroy a root themselves want exactly this half.
    fn put_slot_root(&mut self, id: ControlId, slot: Slot, root: Option<ControlId>) {
        match root {
            Some(root) => {
                self.slot_roots.entry(id).or_default()[slot.index()] = Some(root);
            }
            None => {
                if let Some(roots) = self.slot_roots.get_mut(&id) {
                    roots[slot.index()] = None;
                    // Don't keep an all-empty array alive: `slot_roots` is what
                    // teardown iterates, and an empty entry per node that ever had
                    // a slot is pure overhead.
                    if roots.iter().all(Option::is_none) {
                        self.slot_roots.remove(&id);
                    }
                }
            }
        }
    }

    /// Point the backend at `slot`'s root and record it — the two halves that must
    /// not disagree, so they are never written apart.
    ///
    /// The match is the second compile-time gate on adding a [`Slot`]: a new
    /// variant fails to compile here until it has a backend setter.
    fn attach_slot(&mut self, id: ControlId, slot: Slot, root: Option<ControlId>) {
        match slot {
            Slot::Header => self.backend.set_header_element(id, root),
            Slot::Pane => self.backend.set_pane_element(id, root),
            Slot::Flyout => self.backend.set_flyout_element(id, root),
        }
        self.put_slot_root(id, slot, root);
    }


    fn mount_tab_item(&mut self, parent: ControlId, tab: &TabItem) {
        let tab_id = self.acquire_control(ControlKind::TabViewItem);
        self.backend
            .set_prop(tab_id, Prop::Header, &PropValue::Str(tab.header.clone()));
        if let Some(key) = &tab.key {
            self.backend
                .set_prop(tab_id, Prop::ItemKey, &PropValue::Str(key.clone()));
        }
        if let Some(closable) = tab.is_closable {
            self.backend
                .set_prop(tab_id, Prop::IsClosable, &PropValue::Bool(closable));
        }
        if let Some(content_id) = self.mount(&tab.content) {
            self.append_child_tracked(tab_id, content_id);
        }
        self.append_child_tracked(parent, tab_id);
    }

    fn mount_pivot_item(&mut self, parent: ControlId, item: &PivotItem) {
        let item_id = self.acquire_control(ControlKind::PivotItem);
        self.backend.set_prop(
            item_id,
            Prop::ItemHeader,
            &PropValue::Str(item.header.clone()),
        );
        if let Some(content_id) = self.mount(&item.content) {
            self.append_child_tracked(item_id, content_id);
        }
        self.append_child_tracked(parent, item_id);
    }

    fn reconcile_tabs(&mut self, parent: ControlId, old: &[TabItem], new: &[TabItem]) {
        let common = old.len().min(new.len());

        for i in 0..common {
            let Some(tab_id) = self.child_at(parent, i) else {
                continue;
            };
            let o = &old[i];
            let n = &new[i];
            if o.header != n.header {
                self.backend
                    .set_prop(tab_id, Prop::Header, &PropValue::Str(n.header.clone()));
            }
            if o.key != n.key
                && let Some(key) = &n.key
            {
                self.backend
                    .set_prop(tab_id, Prop::ItemKey, &PropValue::Str(key.clone()));
            }
            if o.is_closable != n.is_closable {
                // Either explicit value (set new), or transition to default
                // (re-enable platform default = true).
                let v = n.is_closable.unwrap_or(true);
                self.backend
                    .set_prop(tab_id, Prop::IsClosable, &PropValue::Bool(v));
            }
            let oc = std::slice::from_ref(&o.content);
            let nc = std::slice::from_ref(&n.content);
            self.reconcile_children_positional(tab_id, oc, nc);
        }

        if new.len() > old.len() {
            for n in &new[old.len()..] {
                self.mount_tab_item(parent, n);
            }
        }

        while self.child_at(parent, new.len()).is_some() {
            let extra_id = self.child_at(parent, new.len()).unwrap();
            self.remove_child_tracked(parent, new.len());
            self.unmount(extra_id);
        }
    }

    fn reconcile_pivot_items(&mut self, parent: ControlId, old: &[PivotItem], new: &[PivotItem]) {
        let common = old.len().min(new.len());

        for i in 0..common {
            let Some(item_id) = self.child_at(parent, i) else {
                continue;
            };
            let o = &old[i];
            let n = &new[i];
            if o.header != n.header {
                self.backend
                    .set_prop(item_id, Prop::ItemHeader, &PropValue::Str(n.header.clone()));
            }
            let oc = std::slice::from_ref(&o.content);
            let nc = std::slice::from_ref(&n.content);
            self.reconcile_children_positional(item_id, oc, nc);
        }

        if new.len() > old.len() {
            for n in &new[old.len()..] {
                self.mount_pivot_item(parent, n);
            }
        }

        while self.child_at(parent, new.len()).is_some() {
            let extra_id = self.child_at(parent, new.len()).unwrap();
            self.remove_child_tracked(parent, new.len());
            self.unmount(extra_id);
        }
    }

    fn apply_attached(&mut self, id: ControlId, attached: Option<&AttachedProps>) {
        let Some(att) = attached else { return };
        // GridPlacement is now on Modifiers::grid — handled by apply_modifiers.
        if let Some(p) = att.get::<CanvasPosition>() {
            self.apply_canvas_position(id, *p);
        }
        if let Some(p) = att.get::<RelativePanelAlignment>() {
            self.apply_relative_panel_alignment(id, *p);
        }
    }

    pub fn apply_grid_placement(&mut self, id: ControlId, p: GridPlacement) {
        if p.row != 0 {
            self.backend
                .set_prop(id, Prop::AttachedGridRow, &PropValue::I32(p.row));
        }
        if p.column != 0 {
            self.backend
                .set_prop(id, Prop::AttachedGridColumn, &PropValue::I32(p.column));
        }
        if p.row_span > 1 {
            self.backend
                .set_prop(id, Prop::AttachedGridRowSpan, &PropValue::I32(p.row_span));
        }
        if p.column_span > 1 {
            self.backend.set_prop(
                id,
                Prop::AttachedGridColumnSpan,
                &PropValue::I32(p.column_span),
            );
        }
    }

    /// Unconditionally emits all four grid attached props — used in the diff
    /// path to clear stale values when placement changes or is removed.
    pub fn apply_grid_placement_full(&mut self, id: ControlId, p: GridPlacement) {
        self.backend
            .set_prop(id, Prop::AttachedGridRow, &PropValue::I32(p.row));
        self.backend
            .set_prop(id, Prop::AttachedGridColumn, &PropValue::I32(p.column));
        self.backend
            .set_prop(id, Prop::AttachedGridRowSpan, &PropValue::I32(p.row_span));
        self.backend.set_prop(
            id,
            Prop::AttachedGridColumnSpan,
            &PropValue::I32(p.column_span),
        );
    }

    fn apply_canvas_position(&mut self, id: ControlId, p: CanvasPosition) {
        // Emit left AND top unconditionally, even at 0.0. This method runs only
        // when the element actually set a `CanvasPosition` (it is gated on the
        // attached prop being present), and either prop is what flips the child
        // to `Position::Absolute` in the backend. A Canvas is a `Display::Block`
        // panel, so a child that stays relative BLOCK-FLOWS — stacking under its
        // siblings — instead of pinning to (0,0). Skipping the 0.0 emit left a
        // child explicitly placed at the origin flowing; a zero-size sibling hid
        // it (the flow cursor never moved), but a sized one (a filled area)
        // pushed every later child down by its height.
        self.backend
            .set_prop(id, Prop::AttachedCanvasLeft, &PropValue::CanvasOffset(p.left));
        self.backend
            .set_prop(id, Prop::AttachedCanvasTop, &PropValue::CanvasOffset(p.top));
        self.backend
            .set_prop(id, Prop::AttachedCanvasRight, &canvas_edge(p.right));
        self.backend
            .set_prop(id, Prop::AttachedCanvasBottom, &canvas_edge(p.bottom));
        if p.z_index != 0 {
            self.backend
                .set_prop(id, Prop::AttachedCanvasZIndex, &PropValue::I32(p.z_index));
        }
    }

    fn diff_attached(
        &mut self,
        id: ControlId,
        old: Option<&AttachedProps>,
        new: Option<&AttachedProps>,
    ) {
        // GridPlacement is now on Modifiers::grid — handled by diff_modifiers.

        let old_canvas = old.and_then(|a| a.get::<CanvasPosition>()).copied();
        let new_canvas = new.and_then(|a| a.get::<CanvasPosition>()).copied();
        if old_canvas != new_canvas {
            let p = new_canvas.unwrap_or_default();
            self.backend
                .set_prop(id, Prop::AttachedCanvasLeft, &PropValue::CanvasOffset(p.left));
            self.backend
                .set_prop(id, Prop::AttachedCanvasTop, &PropValue::CanvasOffset(p.top));
                // Emitted as `Unset` when absent so a child that STOPS filling
                // returns to stating its own size, rather than keeping a stale
                // trailing inset.
                self.backend
                    .set_prop(id, Prop::AttachedCanvasRight, &canvas_edge(p.right));
                self.backend
                    .set_prop(id, Prop::AttachedCanvasBottom, &canvas_edge(p.bottom));
            self.backend
                .set_prop(id, Prop::AttachedCanvasZIndex, &PropValue::I32(p.z_index));
        }

        let old_rp = old.and_then(|a| a.get::<RelativePanelAlignment>()).copied();
        let new_rp = new.and_then(|a| a.get::<RelativePanelAlignment>()).copied();
        if old_rp != new_rp {
            let p = new_rp.unwrap_or_default();
            self.apply_relative_panel_alignment_full(id, p);
        }
    }

    fn apply_relative_panel_alignment(&mut self, id: ControlId, p: RelativePanelAlignment) {
        if p.align_left_with_panel {
            self.backend
                .set_prop(id, Prop::AlignLeftWithPanel, &PropValue::Bool(true));
        }
        if p.align_right_with_panel {
            self.backend
                .set_prop(id, Prop::AlignRightWithPanel, &PropValue::Bool(true));
        }
        if p.align_top_with_panel {
            self.backend
                .set_prop(id, Prop::AlignTopWithPanel, &PropValue::Bool(true));
        }
        if p.align_bottom_with_panel {
            self.backend
                .set_prop(id, Prop::AlignBottomWithPanel, &PropValue::Bool(true));
        }
        if p.align_h_center_with_panel {
            self.backend
                .set_prop(id, Prop::AlignHCenterWithPanel, &PropValue::Bool(true));
        }
        if p.align_v_center_with_panel {
            self.backend
                .set_prop(id, Prop::AlignVCenterWithPanel, &PropValue::Bool(true));
        }
    }

    fn apply_relative_panel_alignment_full(&mut self, id: ControlId, p: RelativePanelAlignment) {
        self.backend.set_prop(
            id,
            Prop::AlignLeftWithPanel,
            &PropValue::Bool(p.align_left_with_panel),
        );
        self.backend.set_prop(
            id,
            Prop::AlignRightWithPanel,
            &PropValue::Bool(p.align_right_with_panel),
        );
        self.backend.set_prop(
            id,
            Prop::AlignTopWithPanel,
            &PropValue::Bool(p.align_top_with_panel),
        );
        self.backend.set_prop(
            id,
            Prop::AlignBottomWithPanel,
            &PropValue::Bool(p.align_bottom_with_panel),
        );
        self.backend.set_prop(
            id,
            Prop::AlignHCenterWithPanel,
            &PropValue::Bool(p.align_h_center_with_panel),
        );
        self.backend.set_prop(
            id,
            Prop::AlignVCenterWithPanel,
            &PropValue::Bool(p.align_v_center_with_panel),
        );
    }
}
