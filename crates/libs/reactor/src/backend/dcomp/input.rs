//! Pointer + keyboard input for the drawn control library: a z-ordered
//! (deepest-wins) AABB hit-test over the layout output, the hover/press ink
//! state machine, control activation (toggle / check / select / segmented /
//! slider / nav / expander), wheel scrolling, the keyboard focus ring with
//! Tab/Shift-Tab + Space/Enter activation, and popup-overlay routing (open /
//! light-dismiss / Up-Down-Enter-Esc). Coordinates arrive in DIPs.

use super::controls;
use super::editor;
use super::popup::Popup;
use super::*;
use crate::backend::Event;
use crate::style::PointerEventInfo;
use crate::system_bindings::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GlobalAlloc, GlobalLock, GlobalUnlock,
    OpenClipboard, SetClipboardData, CF_UNICODETEXT, GMEM_MOVEABLE, HWND,
};
use windows_canvas_core::Rect as CanvasRect;

// Virtual-key codes used by keyboard handling.
const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_PRIOR: u32 = 0x21; // PageUp
const VK_NEXT: u32 = 0x22; // PageDown
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_DELETE: u32 = 0x2E;
const VK_A: u32 = 0x41;
const VK_C: u32 = 0x43;
const VK_V: u32 = 0x56;
const VK_X: u32 = 0x58;

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

    /// Whether `(x, y)` (absolute DIP) lies over scroll container `id`'s thumb.
    /// Returns the pointer→thumb-top offset (for drag tracking) when it does.
    fn thumb_at(&self, id: ControlId, x: f32, y: f32) -> Option<f32> {
        let n = self.node(id)?;
        let g = scroll::thumb_geom(n.rect.h, n.ctrl.content_h, n.anim.x);
        if !g.overflow {
            return None;
        }
        let tx0 = n.rect.x + n.rect.w - scroll::THUMB_W - scroll::THUMB_MARGIN;
        let ty0 = n.rect.y + g.thumb_y;
        if x >= tx0 && x < tx0 + scroll::THUMB_W && y >= ty0 && y < ty0 + g.thumb_h {
            Some(y - ty0)
        } else {
            None
        }
    }

    /// Switch which scroll container's thumb is shown (fade the old out, the new
    /// in). The actual targets are (re)applied per tick; this only records the
    /// hovered id and ensures both are stepped.
    fn update_hovered_scroll(&mut self, now: Option<ControlId>) {
        if now == self.hovered_scroll {
            return;
        }
        if let Some(old) = self.hovered_scroll {
            self.animating.insert(old);
        }
        if let Some(new) = now {
            self.animating.insert(new);
        }
        self.hovered_scroll = now;
    }

    /// Drag the thumb of scroll container `id` so its top follows the pointer.
    fn drag_thumb_to(&mut self, id: ControlId, y: f32) {
        let (ny, vh, content_h, grab) = match self.node(id) {
            Some(n) => (n.rect.y, n.rect.h, n.ctrl.content_h, n.thumb_drag.unwrap_or(0.0)),
            None => return,
        };
        let thumb_y = (y - ny) - grab;
        let scroll = scroll::scroll_for_thumb_y(thumb_y, vh, content_h);
        if let Some(n) = self.node_mut(id) {
            n.anim.x = scroll; // 1:1 tracking — bypass the settle spring while dragging
            n.anim.target = scroll;
        }
        self.animating.insert(id);
        self.apply_scroll(id);
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

        // A pressed text field drag-selects 1:1 with the pointer.
        if let Some(pid) = self.pressed_id
            && self.node(pid).is_some_and(|n| n.editor.is_some())
        {
            self.extend_selection(pid, x);
            return true;
        }

        // A pressed slider scrubs 1:1 with the pointer.
        if let Some(pid) = self.pressed_id
            && self.node(pid).map(|n| n.kind) == Some(ControlKind::Slider)
        {
            self.slider_to(pid, x);
            return true;
        }

        // A dragged scroll thumb tracks the pointer 1:1.
        if let Some(sid) = self.dragging_thumb {
            self.drag_thumb_to(sid, y);
            return true;
        }

        // Fade the scrollbar thumb in for whichever scroll container is hovered.
        self.update_hovered_scroll(self.scroll_at(x, y));

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
        // Fade out the scrollbar thumb when the pointer leaves the window.
        self.update_hovered_scroll(None);
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

        // Pressing the overlay scrollbar thumb starts a drag-to-scroll (the thumb
        // sits above the content, so it wins over any node beneath it).
        if let Some(sid) = self.scroll_at(x, y)
            && let Some(grab) = self.thumb_at(sid, x, y)
        {
            if let Some(n) = self.node_mut(sid) {
                n.thumb_drag = Some(grab);
            }
            self.dragging_thumb = Some(sid);
            self.update_hovered_scroll(Some(sid));
            return (true, true);
        }

        let target = self.interactive_at(x, y);
        // Pointer focus (no visible ring) follows the click.
        self.set_focus(target.filter(|id| self.node(*id).is_some_and(|n| n.focusable)), false);

        // Editable text field: place the caret / handle a spin-button press
        // (no press ink, and the click starts a possible drag-select).
        if let Some(id) = target
            && self.node(id).is_some_and(|n| n.editor.is_some())
        {
            if self.node(id).map(|n| n.kind) == Some(ControlKind::NumberBox)
                && self.spin_hit(id, x, y)
            {
                return (true, false);
            }
            self.place_caret(id, x);
            self.pressed_id = Some(id);
            return (true, false);
        }

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
        // End a scrollbar-thumb drag (keep the timer so the thumb can fade out).
        if let Some(sid) = self.dragging_thumb.take() {
            if let Some(n) = self.node_mut(sid) {
                n.thumb_drag = None;
            }
            self.animating.insert(sid);
            return true;
        }

        // Popup open: a click on a row selects it, then dismisses.
        if self.popup.is_some() {
            let hit = self.popup.as_ref().and_then(|p| p.hit(x, y));
            if let Some(idx) = hit {
                self.commit_popup(idx);
            }
            return self.popup.is_some() && !self.popup_settled;
        }

        // A text-field drag ended: just drop the press (no activation / ink).
        if let Some(pid) = self.pressed_id
            && self.node(pid).is_some_and(|n| n.editor.is_some())
        {
            self.pressed_id = None;
            return false;
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
                // The body subtree's `Display::None` flips with `expanded`, so the
                // layout must be recomputed for the body to reclaim/release space
                // (the chevron keeps animating via the timer).
                self.relayout_and_paint();
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

    /// Window lost activation: commit a focused NumberBox and light-dismiss any
    /// open popup. The keyboard focus itself is retained so re-activating the
    /// window resumes editing where it left off.
    pub(crate) fn on_focus_lost(&mut self) {
        if let Some(id) = self.focused_editable()
            && self.node(id).map(|n| n.kind) == Some(ControlKind::NumberBox)
        {
            self.commit_number(id);
        }
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
        // A focused NumberBox under the pointer steps on the wheel.
        if let Some(id) = self.focused_editable()
            && self.node(id).map(|n| n.kind) == Some(ControlKind::NumberBox)
            && self.node(id).is_some_and(|n| n.rect.contains(x, y))
        {
            self.number_step(id, if delta > 0 { 1.0 } else { -1.0 }, false);
            return false;
        }

        if let Some(id) = self.scroll_at(x, y) {
            // 48 DIPs per detent, downward wheel scrolls content up.
            let step = -(delta as f32 / 120.0) * 48.0;
            let max = self.node(id).map(|n| (n.ctrl.content_h - n.rect.h).max(0.0)).unwrap_or(0.0);
            if let Some(n) = self.node_mut(id) {
                n.anim.target = (n.anim.target + step).clamp(0.0, max);
            }
            // Reveal the thumb while scrolling (it auto-hides once the pointer
            // leaves and the scroll spring settles).
            self.update_hovered_scroll(Some(id));
            self.animating.insert(id);
            return true;
        }
        false
    }

    // ── Keyboard ─────────────────────────────────────────────────────────────

    /// A key was pressed (`shift` / `ctrl` held?). Returns `true` if a
    /// spring/timer should run.
    pub(crate) fn on_key(&mut self, vk: u32, shift: bool, ctrl: bool) -> bool {
        // A focused text editor consumes editing keys before the generic ring.
        if let Some(id) = self.focused_editable()
            && let Some(needs) = self.editor_key(id, vk, shift, ctrl)
        {
            return needs;
        }

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

    // ── Text editor ────────────────────────────────────────────────────────

    /// The focused node, if it is an editable text field.
    fn focused_editable(&self) -> Option<ControlId> {
        let id = self.focused_id?;
        self.node(id)
            .is_some_and(|n| n.editor.is_some())
            .then_some(id)
    }

    /// Whether the caret-blink timer should be running (a text field is focused).
    pub(crate) fn wants_blink_timer(&self) -> bool {
        self.focused_editable().is_some()
    }

    /// Show / hide the focused field's caret when the host window gains or
    /// loses activation (keyboard focus is retained either way).
    pub(crate) fn window_focus_changed(&mut self, focused: bool) {
        if let Some(id) = self.focused_editable() {
            if let Some(n) = self.node_mut(id) {
                if let Some(e) = &mut n.editor {
                    e.blink_on = focused;
                }
                n.mark_dirty();
            }
            self.repaint();
        }
    }

    /// Flip the focused field's caret-blink phase and repaint just that field.
    pub(crate) fn blink_tick(&mut self) {
        if let Some(id) = self.focused_editable() {
            if let Some(n) = self.node_mut(id) {
                if let Some(e) = &mut n.editor {
                    e.blink_on = !e.blink_on;
                }
                n.mark_dirty();
            }
            self.repaint();
        }
    }

    fn with_editor<R>(&mut self, id: ControlId, f: impl FnOnce(&mut editor::Editor) -> R) -> Option<R> {
        self.node_mut(id).and_then(|n| n.editor.as_mut()).map(f)
    }

    /// Route an editing key to the focused editor. Returns `Some(needs_timer)`
    /// when consumed, or `None` to let the generic ring handle it (e.g. Tab).
    fn editor_key(&mut self, id: ControlId, vk: u32, shift: bool, ctrl: bool) -> Option<bool> {
        let kind = self.node(id)?.kind;
        if vk == VK_TAB {
            return None; // Tab leaves the field (commit happens in set_focus).
        }
        if ctrl {
            match vk {
                VK_A => {
                    self.with_editor(id, |e| e.select_all());
                    self.editor_caret_moved(id);
                    return Some(false);
                }
                VK_C => {
                    self.editor_copy(id);
                    return Some(false);
                }
                VK_X => {
                    self.editor_cut(id);
                    return Some(false);
                }
                VK_V => {
                    self.editor_paste(id);
                    return Some(false);
                }
                VK_LEFT => {
                    self.with_editor(id, |e| e.move_left(true, shift));
                    self.editor_caret_moved(id);
                    return Some(false);
                }
                VK_RIGHT => {
                    self.with_editor(id, |e| e.move_right(true, shift));
                    self.editor_caret_moved(id);
                    return Some(false);
                }
                _ => {}
            }
        }
        match vk {
            VK_LEFT => {
                self.with_editor(id, |e| e.move_left(false, shift));
                self.editor_caret_moved(id);
            }
            VK_RIGHT => {
                self.with_editor(id, |e| e.move_right(false, shift));
                self.editor_caret_moved(id);
            }
            VK_HOME => {
                self.with_editor(id, |e| e.home(shift));
                self.editor_caret_moved(id);
            }
            VK_END => {
                self.with_editor(id, |e| e.end(shift));
                self.editor_caret_moved(id);
            }
            VK_BACK => {
                self.with_editor(id, |e| e.backspace());
                self.editor_after_edit(id);
            }
            VK_DELETE => {
                self.with_editor(id, |e| e.delete_forward());
                self.editor_after_edit(id);
            }
            VK_RETURN => {
                if kind == ControlKind::NumberBox {
                    self.commit_number(id);
                } else if kind == ControlKind::AutoSuggestBox {
                    let t = self.with_editor(id, |e| e.text()).unwrap_or_default();
                    self.fire_string(id, Event::QuerySubmitted, t);
                }
            }
            VK_UP if kind == ControlKind::NumberBox => self.number_step(id, 1.0, false),
            VK_DOWN if kind == ControlKind::NumberBox => self.number_step(id, -1.0, false),
            VK_PRIOR if kind == ControlKind::NumberBox => self.number_step(id, 1.0, true),
            VK_NEXT if kind == ControlKind::NumberBox => self.number_step(id, -1.0, true),
            _ => {} // consume; printable input arrives via WM_CHAR
        }
        Some(false)
    }

    /// IME composition started (IMM32 fallback): anchor the composition span.
    /// Returns `true` if a focused editor will handle composition (so the host
    /// can suppress the default IME composition window).
    pub(crate) fn ime_begin(&mut self) -> bool {
        if let Some(id) = self.focused_editable() {
            self.with_editor(id, |e| e.ime_begin());
            true
        } else {
            false
        }
    }

    /// IME composition update (the in-progress, underlined run).
    pub(crate) fn ime_update(&mut self, s: &str) {
        if let Some(id) = self.focused_editable() {
            self.with_editor(id, |e| e.ime_replace(s, true));
            self.editor_caret_moved(id);
        }
    }

    /// IME committed a result string into the field.
    pub(crate) fn ime_commit(&mut self, s: &str) {
        if let Some(id) = self.focused_editable() {
            self.with_editor(id, |e| e.ime_replace(s, false));
            self.editor_after_edit(id);
        }
    }

    /// IME composition ended (cancelled / finished).
    pub(crate) fn ime_end(&mut self) {
        if let Some(id) = self.focused_editable() {
            self.with_editor(id, |e| e.ime_end());
            self.editor_caret_moved(id);
        }
    }

    /// Whether a text field currently has keyboard focus (host gates IME).
    pub(crate) fn has_text_focus(&self) -> bool {
        self.focused_editable().is_some()
    }

    /// A printable character (WM_CHAR, UTF-16 code unit). Returns `true` if a
    /// focused editor consumed it.
    pub(crate) fn on_char(&mut self, ch: u16) -> bool {
        let Some(id) = self.focused_editable() else {
            return false;
        };
        // Drop control characters (Tab/Enter/Backspace handled in `on_key`).
        if ch < 0x20 || ch == 0x7F {
            return false;
        }
        let Some(c) = char::from_u32(ch as u32) else {
            return false;
        };
        let numeric = self
            .node(id)
            .and_then(|n| n.editor.as_ref())
            .is_some_and(|e| e.numeric);
        if numeric && !editor::numeric_char_ok(c) {
            return false;
        }
        let s = c.to_string();
        self.with_editor(id, |e| e.insert(&s));
        self.editor_after_edit(id);
        true
    }

    /// Caret moved (no text change): reset blink, repaint the field.
    fn editor_caret_moved(&mut self, id: ControlId) {
        if let Some(n) = self.node_mut(id) {
            if let Some(e) = &mut n.editor {
                e.blink_on = true;
            }
            n.mark_dirty();
        }
        self.repaint();
    }

    /// Text changed: reset blink, repaint, and fire the per-kind change event
    /// (NumberBox fires only on commit).
    fn editor_after_edit(&mut self, id: ControlId) {
        let (kind, text) = match self.node_mut(id) {
            Some(n) => {
                let kind = n.kind;
                if let Some(e) = &mut n.editor {
                    e.blink_on = true;
                    e.seeded = true;
                }
                let text = n.editor.as_ref().map(|e| e.text()).unwrap_or_default();
                n.mark_dirty();
                (kind, text)
            }
            None => return,
        };
        self.repaint();
        match kind {
            ControlKind::TextBox | ControlKind::AutoSuggestBox => {
                self.fire_string(id, Event::TextChanged, text)
            }
            ControlKind::PasswordBox => self.fire_string(id, Event::PasswordChanged, text),
            _ => {}
        }
    }

    /// Commit a NumberBox: parse (with inline arithmetic) → clamp → round →
    /// format → write back → fire `ValueChanged`.
    fn commit_number(&mut self, id: ControlId) {
        let (text, fallback) = match self.node(id) {
            Some(n) => (
                n.editor.as_ref().map(|e| e.text()).unwrap_or_default(),
                n.ctrl.value,
            ),
            None => return,
        };
        let value = editor::eval_numeric(&text).unwrap_or(fallback);
        self.apply_number(id, value);
    }

    /// Step a NumberBox value by ±`dir`·(step|largeChange), folding in any
    /// pending text edit first, then commit.
    fn number_step(&mut self, id: ControlId, dir: f64, large: bool) {
        let value = match self.node(id) {
            Some(n) => {
                let text = n.editor.as_ref().map(|e| e.text()).unwrap_or_default();
                let cur = editor::eval_numeric(&text).unwrap_or(n.ctrl.value);
                let step = n.ctrl.step.unwrap_or(1.0);
                let inc = if large {
                    n.ctrl.large_change.unwrap_or(step * 10.0)
                } else {
                    step
                };
                cur + dir * inc
            }
            None => return,
        };
        self.apply_number(id, value);
    }

    /// Clamp/round/format `value`, write it into the NumberBox, repaint, and
    /// fire `ValueChanged`.
    fn apply_number(&mut self, id: ControlId, value: f64) {
        let (min, max, precision) = match self.node(id) {
            Some(n) => (n.ctrl.min, n.ctrl.max, n.ctrl.precision),
            None => return,
        };
        let (v, s) = editor::commit_format(value, min, max, precision);
        if let Some(n) = self.node_mut(id) {
            n.ctrl.value = v;
            if let Some(e) = &mut n.editor {
                e.set_text(&s);
                e.seeded = true;
                e.blink_on = true;
            }
            n.mark_dirty();
        }
        self.repaint();
        self.fire_f64(id, Event::ValueChanged, v);
    }

    // ── Editor pointer + clipboard ───────────────────────────────────────────

    /// The caret index for an absolute-DIP x over editable node `id`.
    fn caret_index_at(&self, id: ControlId, x: f32) -> Option<usize> {
        let n = self.node(id)?;
        let ed = n.editor.as_ref()?;
        let (pad_left, _w) = editor::editor_content(n.kind, n.rect.w);
        Some(ed.index_at_x(x - n.rect.x, pad_left - ed.scroll_x))
    }

    /// Place the caret (collapsing the selection) from a pointer press.
    fn place_caret(&mut self, id: ControlId, x: f32) {
        let Some(idx) = self.caret_index_at(id, x) else {
            return;
        };
        if let Some(n) = self.node_mut(id) {
            if let Some(e) = &mut n.editor {
                e.caret = idx;
                e.anchor = idx;
                e.blink_on = true;
            }
            n.mark_dirty();
        }
        self.repaint();
    }

    /// Extend the selection to a pointer position (drag-select).
    fn extend_selection(&mut self, id: ControlId, x: f32) {
        let Some(idx) = self.caret_index_at(id, x) else {
            return;
        };
        if let Some(n) = self.node_mut(id) {
            if let Some(e) = &mut n.editor {
                e.caret = idx;
                e.blink_on = true;
            }
            n.mark_dirty();
        }
        self.repaint();
    }

    /// Press on a wide NumberBox's spin column: step up (top half) or down.
    fn spin_hit(&mut self, id: ControlId, x: f32, y: f32) -> bool {
        let (col_x, mid, wide) = match self.node(id) {
            Some(n) => (
                n.rect.x + n.rect.w - editor::SPIN_W,
                n.rect.y + n.rect.h / 2.0,
                n.rect.w >= editor::SPIN_MIN_BOX_W,
            ),
            None => return false,
        };
        if !wide || x < col_x {
            return false;
        }
        self.number_step(id, if y < mid { 1.0 } else { -1.0 }, false);
        true
    }

    fn editor_copy(&self, id: ControlId) {
        if let Some(n) = self.node(id)
            && let Some(e) = &n.editor
            && e.has_selection()
            && !e.mask
        {
            clipboard_set(self.hwnd(), &e.selected_text());
        }
    }

    fn editor_cut(&mut self, id: ControlId) {
        self.editor_copy(id);
        let removed = self
            .with_editor(id, |e| {
                if e.has_selection() {
                    e.backspace();
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if removed {
            self.editor_after_edit(id);
        }
    }

    fn editor_paste(&mut self, id: ControlId) {
        let Some(text) = clipboard_get(self.hwnd()) else {
            return;
        };
        let numeric = self
            .node(id)
            .and_then(|n| n.editor.as_ref())
            .is_some_and(|e| e.numeric);
        // Single-line: strip newlines; numeric: keep only admissible chars.
        let s: String = text
            .chars()
            .filter(|c| *c != '\n' && *c != '\r')
            .filter(|c| !numeric || editor::numeric_char_ok(*c))
            .collect();
        if s.is_empty() {
            return;
        }
        self.with_editor(id, |e| e.insert(&s));
        self.editor_after_edit(id);
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
        // Commit a NumberBox losing focus before clearing it.
        if let Some(old) = self.focused_id
            && Some(old) != id
            && self.node(old).map(|n| n.kind) == Some(ControlKind::NumberBox)
        {
            self.commit_number(old);
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
            // Keyboard focus (Tab) selects the whole field for quick replace.
            if visible
                && let Some(e) = &mut n.editor
            {
                e.select_all();
                e.blink_on = true;
            }
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
        let hovered_scroll = self.hovered_scroll;
        let ids: Vec<ControlId> = self.animating.iter().copied().collect();
        for id in ids {
            let (settled, scroll) = match self.node_mut(id) {
                Some(n) => {
                    let h = n.hover.step(dt);
                    let p = n.press.step(dt);
                    let a = n.anim.step(dt);
                    // Scroll thumb auto-hide: visible while hovered, dragging, or the
                    // scroll spring is still in flight; fades out otherwise.
                    let mut tf = true;
                    if n.is_scroll() {
                        let active = hovered_scroll == Some(id)
                            || n.thumb_drag.is_some()
                            || (n.anim.x - n.anim.target).abs() > 1e-3;
                        n.thumb_fade.target = if active { 1.0 } else { 0.0 };
                        tf = n.thumb_fade.step(dt);
                    }
                    let indeterminate = n.ctrl.indeterminate
                        && matches!(n.kind, ControlKind::ProgressBar | ControlKind::ProgressRing);
                    if indeterminate {
                        n.phase = (n.phase + dt * 0.6) % 1_000_000.0;
                    }
                    n.mark_dirty();
                    let settled = h && p && a && tf && !indeterminate;
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

    fn hwnd(&self) -> HWND {
        self.hwnd as HWND
    }
}

/// Read CF_UNICODETEXT from the clipboard as a `String`.
fn clipboard_get(hwnd: HWND) -> Option<String> {
    unsafe {
        if !OpenClipboard(hwnd).as_bool() {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT as u32);
        let result = if !handle.is_null() {
            let ptr = GlobalLock(handle as _) as *const u16;
            if ptr.is_null() {
                None
            } else {
                let mut len = 0usize;
                while *ptr.add(len) != 0 {
                    len += 1;
                }
                let s = String::from_utf16_lossy(core::slice::from_raw_parts(ptr, len));
                let _ = GlobalUnlock(handle as _);
                Some(s)
            }
        } else {
            None
        };
        let _ = CloseClipboard();
        result
    }
}

/// Write `s` to the clipboard as CF_UNICODETEXT.
fn clipboard_set(hwnd: HWND, s: &str) {
    let utf16: Vec<u16> = s.encode_utf16().chain(core::iter::once(0)).collect();
    unsafe {
        if !OpenClipboard(hwnd).as_bool() {
            return;
        }
        let _ = EmptyClipboard();
        let bytes = utf16.len() * 2; // UTF-16 code units
        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if !hmem.is_null() {
            let dst = GlobalLock(hmem) as *mut u16;
            if !dst.is_null() {
                core::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
                let _ = GlobalUnlock(hmem);
                // Ownership transfers to the clipboard on success.
                if SetClipboardData(CF_UNICODETEXT as u32, hmem as _).is_null() {
                    // Failed to set: the system did not take ownership; free it.
                    let _ = crate::system_bindings::GlobalFree(hmem);
                }
            }
        }
        let _ = CloseClipboard();
    }
}
