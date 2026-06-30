//! Pointer + keyboard input for the drawn control library: a z-ordered
//! (deepest-wins) AABB hit-test over the layout output, the hover/press ink
//! state machine, control activation (toggle / check / select / segmented /
//! slider / nav / expander), wheel scrolling, the keyboard focus ring with
//! Tab/Shift-Tab + Space/Enter activation, and popup-overlay routing (open /
//! light-dismiss / Up-Down-Enter-Esc). Coordinates arrive in DIPs.

use super::controls;
use super::popup::Popup;
use super::*;
use crate::backend::Event;
use crate::style::PointerEventInfo;
use windows_canvas_core::Rect as CanvasRect;

// Virtual-key codes used by keyboard handling.
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;

impl DCompBackend {
    // ── Hit-testing ──────────────────────────────────────────────────────────

    /// The deepest interactive node containing the point, accounting for the
    /// scroll offset of any ancestor scroll container.
    fn interactive_at(&self, x: f32, y: f32) -> Option<ControlId> {
        let root = self.root?;
        let mut best = None;
        self.hit_walk(root, x, y, &mut best, true);
        best
    }

    /// The deepest scroll container containing the point.
    fn scroll_at(&self, x: f32, y: f32) -> Option<ControlId> {
        let root = self.root?;
        let mut best = None;
        self.hit_walk(root, x, y, &mut best, false);
        best
    }

    /// Walk the tree; `y` is pre-adjusted for ancestor scroll. When
    /// `want_interactive` collect clickable nodes, else collect scroll nodes.
    fn hit_walk(&self, id: ControlId, x: f32, y: f32, out: &mut Option<ControlId>, want_interactive: bool) {
        let Some(node) = self.node(id) else { return };
        if node.rect.contains(x, y) {
            if want_interactive && node.is_clickable() {
                *out = Some(id);
            } else if !want_interactive && node.is_scroll() {
                *out = Some(id);
            }
        }
        let child_y = if node.is_scroll() { y + node.anim.x } else { y };
        for c in &node.children {
            self.hit_walk(*c, x, child_y, out, want_interactive);
        }
    }

    // ── Hover ────────────────────────────────────────────────────────────────

    /// Pointer moved to (x, y) DIPs. Returns `true` if a spring started.
    pub(crate) fn on_pointer_move(&mut self, x: f32, y: f32) -> bool {
        // While a popup is open, the move only re-highlights its rows.
        if self.popup.is_some() {
            let hit = self.popup.as_ref().and_then(|p| p.hit(x, y));
            if let Some(p) = &mut self.popup {
                p.set_hovered(hit, &self.comp);
            }
            return false;
        }

        // A pressed slider scrubs 1:1 with the pointer.
        if let Some(pid) = self.pressed_id
            && self.node(pid).map(|n| n.kind) == Some(ControlKind::Slider)
        {
            self.slider_to(pid, x);
            return true;
        }

        let now = self.interactive_at(x, y);
        if now == self.hovered_id {
            return false;
        }
        if let Some(old) = self.hovered_id.take()
            && let Some(n) = self.node_mut(old)
        {
            n.hovered = false;
            n.hover.target = 0.0;
            self.animating.insert(old);
        }
        if let Some(new) = now {
            if let Some(n) = self.node_mut(new) {
                n.hovered = true;
                n.hover.target = 1.0;
            }
            self.animating.insert(new);
            self.fire_pointer(new, x, y, |p| p.on_pointer_moved.as_ref());
        }
        self.hovered_id = now;
        !self.animating.is_empty()
    }

    pub(crate) fn on_pointer_leave(&mut self) -> bool {
        if let Some(old) = self.hovered_id.take()
            && let Some(n) = self.node_mut(old)
        {
            n.hovered = false;
            n.hover.target = 0.0;
            self.animating.insert(old);
        }
        !self.animating.is_empty()
    }

    // ── Press / release ──────────────────────────────────────────────────────

    /// Left button down. Returns `(captured, needs_timer)`.
    pub(crate) fn on_pointer_down(&mut self, x: f32, y: f32) -> (bool, bool) {
        // Popup open: outside-click light-dismisses; inside is handled on up.
        if self.popup.is_some() {
            let inside = self.popup.as_ref().is_some_and(|p| p.contains(x, y));
            if !inside {
                self.close_popup();
            }
            return (false, false);
        }

        let target = self.interactive_at(x, y);
        // Pointer focus (no visible ring) follows the click.
        self.set_focus(target.filter(|id| self.node(*id).is_some_and(|n| n.focusable)), false);

        if let Some(id) = target {
            if let Some(n) = self.node_mut(id) {
                n.pressed = true;
                n.press.target = 1.0;
            }
            self.animating.insert(id);
            self.pressed_id = Some(id);
            // Sliders scrub immediately on press.
            if self.node(id).map(|n| n.kind) == Some(ControlKind::Slider) {
                self.slider_to(id, x);
            }
            self.fire_pointer(id, x, y, |p| p.on_pointer_pressed.as_ref());
            (true, true)
        } else {
            (false, false)
        }
    }

    /// Left button up. Returns `needs_timer`.
    pub(crate) fn on_pointer_up(&mut self, x: f32, y: f32) -> bool {
        // Popup open: a click on a row selects it, then dismisses.
        if self.popup.is_some() {
            let hit = self.popup.as_ref().and_then(|p| p.hit(x, y));
            if let Some(idx) = hit {
                self.commit_popup(idx);
            }
            return self.popup.is_some() && !self.popup_settled;
        }

        let Some(id) = self.pressed_id.take() else {
            return false;
        };
        if let Some(n) = self.node_mut(id) {
            n.pressed = false;
            n.press.target = 0.0;
        }
        self.animating.insert(id);

        // Activate only if the release is still over the pressed control.
        let over = self.is_over(id, x, y);
        if over {
            self.fire_pointer(id, x, y, |p| p.on_pointer_released.as_ref());
            self.activate_pointer(id, x, y);
        }
        true
    }

    /// Whether `(x, y)` still lies over node `id` (scroll-adjusted).
    fn is_over(&self, id: ControlId, x: f32, y: f32) -> bool {
        // Re-walk to find the topmost interactive node and compare.
        self.interactive_at(x, y) == Some(id)
            || self.node(id).is_some_and(|n| n.rect.contains(x, y))
    }

    // ── Activation ───────────────────────────────────────────────────────────

    /// Activate a control from a pointer release at `(x, y)` (segment/nav item
    /// resolved from the position).
    fn activate_pointer(&mut self, id: ControlId, x: f32, y: f32) {
        let kind = self.node(id).map(|n| n.kind);
        match kind {
            Some(ControlKind::SelectorBar) => self.select_segment(id, x),
            Some(ControlKind::NavigationView) => self.select_nav(id, y),
            _ => self.activate(id),
        }
    }

    /// Activate a control with no position dependency (keyboard or button).
    fn activate(&mut self, id: ControlId) {
        let Some(kind) = self.node(id).map(|n| n.kind) else { return };
        match kind {
            ControlKind::ToggleSwitch => {
                let on = !self.node(id).map(|n| n.ctrl.is_on).unwrap_or(false);
                if let Some(n) = self.node_mut(id) {
                    n.ctrl.is_on = on;
                    n.anim.target = if on { 1.0 } else { 0.0 };
                    n.mark_dirty();
                }
                self.animating.insert(id);
                self.fire_bool(id, Event::Toggled, on);
            }
            ControlKind::CheckBox | ControlKind::ToggleButton => {
                let on = !self.node(id).map(|n| n.ctrl.is_checked).unwrap_or(false);
                if let Some(n) = self.node_mut(id) {
                    n.ctrl.is_checked = on;
                    n.anim.target = if on { 1.0 } else { 0.0 };
                    n.mark_dirty();
                }
                self.animating.insert(id);
                self.fire_bool(id, Event::Checked, on);
            }
            ControlKind::Expander => {
                let ex = !self.node(id).map(|n| n.ctrl.expanded).unwrap_or(false);
                if let Some(n) = self.node_mut(id) {
                    n.ctrl.expanded = ex;
                    n.anim.target = if ex { 1.0 } else { 0.0 };
                    n.mark_dirty();
                }
                self.animating.insert(id);
                self.fire_bool(id, Event::Expanding, ex);
            }
            ControlKind::ComboBox | ControlKind::DropDownButton | ControlKind::SplitButton => {
                self.open_popup(id);
            }
            ControlKind::Button | ControlKind::RepeatButton | ControlKind::HyperlinkButton => {
                if let Some(h) = self.node(id).and_then(|n| n.handler(Event::Click)) {
                    h.invoke();
                }
                if let Some(p) = self.node(id).and_then(|n| n.pointer.as_ref())
                    && let Some(cb) = &p.on_tapped
                {
                    cb.invoke(());
                }
            }
            _ => {}
        }
    }

    fn select_segment(&mut self, id: ControlId, x: f32) {
        let Some(node) = self.node(id) else { return };
        let n = node.ctrl.items.len();
        if n == 0 {
            return;
        }
        let seg_w = controls::segment_width(node);
        let rel = x - node.rect.x - theme::BORDER_W;
        let i = ((rel / seg_w).floor() as i32).clamp(0, n as i32 - 1);
        self.set_segment(id, i);
    }

    fn set_segment(&mut self, id: ControlId, i: i32) {
        let label = {
            let Some(n) = self.node_mut(id) else { return };
            if n.ctrl.selected_index == i {
                return;
            }
            n.ctrl.selected_index = i;
            n.anim.target = i as f32;
            n.mark_dirty();
            n.ctrl.items.get(i as usize).cloned().unwrap_or_default()
        };
        self.animating.insert(id);
        self.fire_string(id, Event::SelectionChanged, label);
    }

    fn select_nav(&mut self, id: ControlId, y: f32) {
        let Some(node) = self.node(id) else { return };
        let n = node.ctrl.items.len();
        if n == 0 {
            return;
        }
        let rel = y - node.rect.y;
        let i = ((rel / controls::NAV_ITEM_H).floor() as i32).clamp(0, n as i32 - 1);
        let tag = {
            let Some(nd) = self.node_mut(id) else { return };
            if nd.ctrl.selected_index == i {
                return;
            }
            nd.ctrl.selected_index = i;
            nd.anim.target = i as f32;
            nd.mark_dirty();
            nd.ctrl.tags.get(i as usize).cloned().unwrap_or_default()
        };
        self.animating.insert(id);
        self.fire_string(id, Event::SelectionChanged, tag);
    }

    /// Map pointer x to a slider value, clamp/quantize, and report it.
    fn slider_to(&mut self, id: ControlId, x: f32) {
        let value = {
            let Some(n) = self.node_mut(id) else { return };
            let inset = theme::SLIDER_THUMB / 2.0;
            let w = (n.rect.w - 2.0 * inset).max(1.0);
            let mut frac = ((x - n.rect.x - inset) / w).clamp(0.0, 1.0);
            let mut v = n.ctrl.min + frac as f64 * (n.ctrl.max - n.ctrl.min);
            if let Some(step) = n.ctrl.step
                && step > 0.0
            {
                v = (v / step).round() * step;
                v = v.clamp(n.ctrl.min, n.ctrl.max);
                let span = n.ctrl.max - n.ctrl.min;
                frac = if span.abs() < f64::EPSILON { 0.0 } else { ((v - n.ctrl.min) / span) as f32 };
            }
            n.ctrl.value = v;
            n.anim.x = frac; // 1:1 tracking (bypass the settle spring during drag)
            n.anim.target = frac;
            n.mark_dirty();
            v
        };
        self.animating.insert(id);
        self.fire_f64(id, Event::ValueChanged, value);
    }

    // ── Popup ────────────────────────────────────────────────────────────────

    fn open_popup(&mut self, owner: ControlId) {
        let Some(node) = self.node(owner) else { return };
        let combo = node.kind == ControlKind::ComboBox;
        let rect = CanvasRect::from_xywh(node.rect.x, node.rect.y, node.rect.w, node.rect.h);
        let rows: Vec<MenuRow> = if combo {
            node.ctrl
                .items
                .iter()
                .map(|s| MenuRow {
                    text: s.clone(),
                    tag: s.clone(),
                    enabled: true,
                    ..Default::default()
                })
                .collect()
        } else {
            node.ctrl.menu.clone()
        };
        if rows.is_empty() {
            return;
        }
        let selected = node.ctrl.selected_index;
        match Popup::open(&self.comp, owner, rows, rect, self.dip_size, combo, selected) {
            Ok(p) => {
                self.close_popup();
                self.popup = Some(p);
                self.popup_settled = false;
            }
            Err(_) => {}
        }
    }

    fn close_popup(&mut self) {
        if let Some(p) = self.popup.take() {
            p.dismiss(&self.comp);
        }
        self.popup_settled = true;
    }

    /// Window lost activation: light-dismiss any open popup.
    pub(crate) fn on_focus_lost(&mut self) {
        self.close_popup();
    }

    /// Commit the selected popup row to its owner and close.
    fn commit_popup(&mut self, idx: usize) {
        let Some(p) = &self.popup else { return };
        let owner = p.owner;
        let combo = p.combo;
        let tag = p.row_tag(idx);
        self.close_popup();
        if combo {
            if let Some(n) = self.node_mut(owner) {
                n.ctrl.selected_index = idx as i32;
                n.mark_dirty();
            }
            self.fire_i32(owner, Event::SelectionChanged, idx as i32);
        } else if let Some(t) = tag {
            self.fire_string(owner, Event::ItemClicked, t);
        }
    }

    // ── Wheel ────────────────────────────────────────────────────────────────

    /// Mouse wheel at (x, y) DIPs, `delta` in WHEEL_DELTA (120) units. Returns
    /// `true` if a scroll spring started.
    pub(crate) fn on_wheel(&mut self, x: f32, y: f32, delta: i32) -> bool {
        if let Some(id) = self.scroll_at(x, y) {
            // 48 DIPs per detent, downward wheel scrolls content up.
            let step = -(delta as f32 / 120.0) * 48.0;
            let max = self.node(id).map(|n| (n.ctrl.content_h - n.rect.h).max(0.0)).unwrap_or(0.0);
            if let Some(n) = self.node_mut(id) {
                n.anim.target = (n.anim.target + step).clamp(0.0, max);
            }
            self.animating.insert(id);
            return true;
        }
        false
    }

    // ── Keyboard ─────────────────────────────────────────────────────────────

    /// A key was pressed (`shift` held?). Returns `true` if a spring/timer
    /// should run.
    pub(crate) fn on_key(&mut self, vk: u32, shift: bool) -> bool {
        // Popup keyboard navigation takes priority.
        if self.popup.is_some() {
            match vk {
                VK_ESCAPE => self.close_popup(),
                VK_DOWN => {
                    if let Some(p) = &mut self.popup {
                        p.move_highlight(1, &self.comp);
                    }
                }
                VK_UP => {
                    if let Some(p) = &mut self.popup {
                        p.move_highlight(-1, &self.comp);
                    }
                }
                VK_RETURN | VK_SPACE => {
                    let h = self.popup.as_ref().map(|p| p.hovered);
                    if let Some(i) = h
                        && i != usize::MAX
                    {
                        self.commit_popup(i);
                    }
                }
                _ => {}
            }
            return self.popup.is_some() && !self.popup_settled;
        }

        match vk {
            VK_TAB => {
                self.move_focus(if shift { -1 } else { 1 });
                false
            }
            VK_SPACE | VK_RETURN => {
                if let Some(id) = self.focused_id {
                    self.activate(id);
                }
                !self.animating.is_empty()
            }
            VK_LEFT | VK_UP => self.focus_arrow(-1),
            VK_RIGHT | VK_DOWN => self.focus_arrow(1),
            _ => false,
        }
    }

    /// Left/Right (or Up/Down) on the focused control: nudge a slider or move a
    /// segmented selection.
    fn focus_arrow(&mut self, dir: i32) -> bool {
        let Some(id) = self.focused_id else { return false };
        match self.node(id).map(|n| n.kind) {
            Some(ControlKind::Slider) => {
                let value = {
                    let Some(n) = self.node_mut(id) else { return false };
                    let step = n.ctrl.step.unwrap_or((n.ctrl.max - n.ctrl.min) / 20.0);
                    let v = (n.ctrl.value + dir as f64 * step).clamp(n.ctrl.min, n.ctrl.max);
                    n.ctrl.value = v;
                    n.anim.target = ctrl_value_frac(n) as f32;
                    n.mark_dirty();
                    v
                };
                self.animating.insert(id);
                self.fire_f64(id, Event::ValueChanged, value);
                true
            }
            Some(ControlKind::SelectorBar) => {
                let (cur, n) = self
                    .node(id)
                    .map(|nd| (nd.ctrl.selected_index, nd.ctrl.items.len() as i32))
                    .unwrap_or((0, 0));
                if n > 0 {
                    self.set_segment(id, (cur + dir).clamp(0, n - 1));
                }
                true
            }
            _ => false,
        }
    }

    /// Collect focusable nodes in document (DFS) order.
    fn focus_order(&self) -> Vec<ControlId> {
        let mut out = Vec::new();
        if let Some(root) = self.root {
            self.focus_collect(root, &mut out);
        }
        out
    }

    fn focus_collect(&self, id: ControlId, out: &mut Vec<ControlId>) {
        let Some(n) = self.node(id) else { return };
        if n.focusable && n.paint.is_enabled {
            out.push(id);
        }
        for c in &n.children {
            self.focus_collect(*c, out);
        }
    }

    fn move_focus(&mut self, dir: i32) {
        let order = self.focus_order();
        if order.is_empty() {
            return;
        }
        let cur = self.focused_id.and_then(|f| order.iter().position(|i| *i == f));
        let next = match cur {
            Some(i) => ((i as i32 + dir).rem_euclid(order.len() as i32)) as usize,
            None => if dir > 0 { 0 } else { order.len() - 1 },
        };
        self.set_focus(Some(order[next]), true);
    }

    /// Set keyboard focus. `visible` shows the focus ring (keyboard) vs. not
    /// (pointer). Repaints the old + new node.
    fn set_focus(&mut self, id: Option<ControlId>, visible: bool) {
        if self.focused_id == id
            && self.focused_id.is_some_and(|f| self.node(f).is_some_and(|n| n.focused == visible))
        {
            return;
        }
        if let Some(old) = self.focused_id
            && let Some(n) = self.node_mut(old)
        {
            n.focused = false;
            n.mark_dirty();
        }
        self.focused_id = id;
        if let Some(new) = id
            && let Some(n) = self.node_mut(new)
        {
            n.focused = visible;
            n.mark_dirty();
        }
        self.repaint();
    }

    // ── Event dispatch ───────────────────────────────────────────────────────

    fn fire_bool(&self, id: ControlId, event: Event, v: bool) {
        if let Some(h) = self.node(id).and_then(|n| n.handler(event)) {
            h.invoke_bool(v);
        }
    }
    fn fire_string(&self, id: ControlId, event: Event, v: String) {
        if let Some(h) = self.node(id).and_then(|n| n.handler(event)) {
            h.invoke_string(v);
        }
    }
    fn fire_f64(&self, id: ControlId, event: Event, v: f64) {
        if let Some(h) = self.node(id).and_then(|n| n.handler(event)) {
            h.invoke_f64(v);
        }
    }
    fn fire_i32(&self, id: ControlId, event: Event, v: i32) {
        if let Some(h) = self.node(id).and_then(|n| n.handler(event)) {
            h.invoke_i32(v);
        }
    }

    fn fire_pointer(
        &self,
        id: ControlId,
        x: f32,
        y: f32,
        pick: impl Fn(&PointerHandlers) -> Option<&crate::Callback<PointerEventInfo>>,
    ) {
        let Some(node) = self.node(id) else { return };
        let Some(handlers) = &node.pointer else { return };
        let Some(cb) = pick(handlers) else { return };
        let info = PointerEventInfo {
            x: (x - node.rect.x) as f64,
            y: (y - node.rect.y) as f64,
            is_left_button_pressed: node.pressed,
            ..PointerEventInfo::default()
        };
        cb.invoke(info);
    }

    // ── Animation tick ───────────────────────────────────────────────────────

    /// Advance all in-flight springs by `dt` and repaint. Returns `true` while
    /// any animation (node spring, indeterminate progress, or popup) remains.
    pub(crate) fn tick(&mut self, dt: f32) -> bool {
        let ids: Vec<ControlId> = self.animating.iter().copied().collect();
        for id in ids {
            let (settled, scroll) = match self.node_mut(id) {
                Some(n) => {
                    let h = n.hover.step(dt);
                    let p = n.press.step(dt);
                    let a = n.anim.step(dt);
                    let indeterminate = n.ctrl.indeterminate
                        && matches!(n.kind, ControlKind::ProgressBar | ControlKind::ProgressRing);
                    if indeterminate {
                        n.phase = (n.phase + dt * 0.6) % 1_000_000.0;
                    }
                    n.mark_dirty();
                    let settled = h && p && a && !indeterminate;
                    (settled, n.is_scroll())
                }
                None => (true, false),
            };
            if scroll {
                self.apply_scroll(id);
            }
            if settled {
                self.animating.remove(&id);
            }
        }

        // Advance the popup open animation.
        if let Some(p) = &mut self.popup
            && !self.popup_settled
            && p.tick(dt)
        {
            self.popup_settled = true;
        }

        self.repaint();
        self.is_animating()
    }

    /// Re-apply a scroll container's offset to its children (compositor move).
    fn apply_scroll(&mut self, id: ControlId) {
        let (children, nx, ny, scroll) = match self.node(id) {
            Some(n) => (n.children.clone(), n.rect.x, n.rect.y, n.anim.x),
            None => return,
        };
        for c in children {
            if let Some(cn) = self.node_mut(c) {
                use windows_numerics::Vector3;
                let _ = cn.vis.SetOffset(Vector3::new(cn.rect.x - nx, cn.rect.y - ny - scroll, 0.0));
            }
        }
    }

    /// True while any spring, indeterminate progress, or the popup is animating.
    pub(crate) fn is_animating(&self) -> bool {
        !self.animating.is_empty() || (self.popup.is_some() && !self.popup_settled)
    }
}
