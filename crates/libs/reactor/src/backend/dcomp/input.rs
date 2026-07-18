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
    pub(super) fn interactive_at(&self, x: f32, y: f32) -> Option<ControlId> {
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

    /// The deepest registered viz pointer surface (knob/slider/EQ canvas — see
    /// `pointer.rs`) under the point, with its sinks and the scroll-adjusted
    /// point for element-relative coordinates. Cheap `None` when nothing is
    /// registered.
    pub(super) fn surface_at(
        &self,
        x: f32,
        y: f32,
    ) -> Option<(ControlId, std::rc::Rc<super::PointerSinks>, f32, f32)> {
        if !super::pointer::has_listeners() {
            return None;
        }
        let root = self.root?;
        let mut best = None;
        self.surface_walk(root, x, y, &mut best);
        best
    }

    fn surface_walk(
        &self,
        id: ControlId,
        x: f32,
        y: f32,
        out: &mut Option<(ControlId, std::rc::Rc<super::PointerSinks>, f32, f32)>,
    ) {
        let Some(node) = self.node(id) else { return };
        if node.rect.contains(x, y)
            && !node.ident.is_null()
            && let Some(sinks) = super::pointer::sinks_for(node.ident)
        {
            *out = Some((id, sinks, x, y));
        }
        let child_y = if node.is_scroll() { y + node.scroll_off } else { y };
        for c in &node.children {
            self.surface_walk(*c, x, child_y, out);
        }
    }

    /// Deliver a pointer transition to a viz surface's sink with element-relative
    /// DIP coordinates. `(x, y)` must be in the node's layout space (scroll-
    /// adjusted, as returned by [`surface_at`](Self::surface_at)).
    fn fire_surface(
        &self,
        id: ControlId,
        cell: &std::cell::RefCell<Option<Box<dyn Fn(PointerEventInfo)>>>,
        x: f32,
        y: f32,
        left: bool,
        wheel_delta: i32,
    ) {
        let Some(node) = self.node(id) else { return };
        let info = PointerEventInfo {
            x: (x - node.rect.x) as f64,
            y: (y - node.rect.y) as f64,
            is_left_button_pressed: left,
            wheel_delta,
            ..PointerEventInfo::default()
        };
        if let Some(cb) = cell.borrow().as_ref() {
            cb(info);
        }
    }

    /// Whether `(x, y)` (absolute DIP) lies over scroll container `id`'s thumb.
    /// Returns the pointer→thumb-top offset (for drag tracking) when it does.
    fn thumb_at(&self, id: ControlId, x: f32, y: f32) -> Option<f32> {
        let n = self.node(id)?;
        let g = scroll::thumb_geom(n.rect.h, n.ctrl.content_h, n.scroll_off);
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

    /// Switch which scroll container's thumb is shown (fade the old out, the
    /// new in) — a direct edge-trigger of the compositor fade, no tick.
    fn update_hovered_scroll(&mut self, now: Option<ControlId>) {
        if now == self.hovered_scroll {
            return;
        }
        let old = self.hovered_scroll;
        self.hovered_scroll = now;
        if let Some(old) = old
            && self.dragging_thumb != Some(old)
        {
            self.set_thumb_shown(old, false);
        }
        if let Some(new) = now {
            self.set_thumb_shown(new, true);
        }
    }

    /// Edge-trigger the thumb reveal/conceal fade (played on the system
    /// compositor). A reveal only happens while the content actually overflows.
    fn set_thumb_shown(&mut self, id: ControlId, shown: bool) {
        let compositor = self.comp.compositor().clone();
        if let Some(n) = self.node_mut(id) {
            let g = scroll::thumb_geom(n.rect.h, n.ctrl.content_h, n.scroll_off);
            let show = shown && g.overflow;
            if show != n.thumb_shown {
                n.thumb_shown = show;
                if let Some(t) = &n.scroll_thumb {
                    animate::fade_thumb(&compositor, t, show);
                }
            }
        }
    }

    /// Drag the thumb of scroll container `id` so its top follows the pointer
    /// 1:1 — carrier and thumb move by plain property snaps (no repaint).
    fn drag_thumb_to(&mut self, id: ControlId, y: f32) {
        let (ny, vh, content_h, grab) = match self.node(id) {
            Some(n) => (n.rect.y, n.rect.h, n.ctrl.content_h, n.thumb_drag.unwrap_or(0.0)),
            None => return,
        };
        let thumb_y = (y - ny) - grab;
        let scroll = scroll::scroll_for_thumb_y(thumb_y, vh, content_h);
        if let Some(n) = self.node_mut(id) {
            n.scroll_off = scroll;
            n.scroll_snap(scroll);
            let g = scroll::thumb_geom(vh, content_h, scroll);
            let tx = n.rect.w - scroll::THUMB_W - scroll::THUMB_MARGIN;
            n.thumb_snap(tx, g.thumb_y);
        }
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
        let child_y = if node.is_scroll() { y + node.scroll_off } else { y };
        for c in &node.children {
            self.hit_walk(*c, x, child_y, out, want_interactive);
        }
    }

    // ── Hover ────────────────────────────────────────────────────────────────

    /// Pointer moved to (x, y) DIPs.
    pub(crate) fn on_pointer_move(&mut self, x: f32, y: f32) {
        #[cfg(debug_assertions)]
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            static SEEN_TOP: AtomicU32 = AtomicU32::new(0);
            let n = SEEN_TOP.fetch_add(1, Ordering::Relaxed);
            if n < 8 {
                eprintln!(
                    "reactor: [dev] move#{n} ({x:.0},{y:.0}) popup={} pressed={:?} psurf={}",
                    self.popup.is_some(),
                    self.pressed_id,
                    self.pressed_surface.is_some(),
                );
            }
        }
        // While a popup is open, the move only re-highlights its rows.
        if self.popup.is_some() {
            let hit = self.popup.as_ref().and_then(|p| p.hit(x, y));
            if let Some(p) = &mut self.popup {
                p.set_hovered(hit, &self.comp);
            }
            return;
        }

        // A pressed text field drag-selects 1:1 with the pointer.
        if let Some(pid) = self.pressed_id
            && self.node(pid).is_some_and(|n| n.editor.is_some())
        {
            self.extend_selection(pid, x);
            return;
        }

        // A pressed slider scrubs 1:1 with the pointer.
        if let Some(pid) = self.pressed_id
            && self.node(pid).map(|n| n.kind) == Some(ControlKind::Slider)
        {
            self.slider_to(pid, x);
            return;
        }

        // A pressed knob scrubs on a relative vertical drag (up = increase).
        if let Some((id, origin, y0)) = self.knob_drag {
            self.knob_drag_to(id, origin, y0, y);
            return;
        }

        // A dragged scroll thumb tracks the pointer 1:1.
        if let Some(sid) = self.dragging_thumb {
            self.drag_thumb_to(sid, y);
            return;
        }

        // A pressed viz pointer surface (knob/slider/EQ drag) receives every
        // move 1:1 — including outside its bounds — until release (capture
        // parity with XAML `CapturePointer`). Hover is frozen for the drag.
        if let Some((sid, sinks, dy)) = self.pressed_surface.clone() {
            self.fire_surface(sid, &sinks.moved, x, y + dy, true, 0);
            // Repaint the dragged surface with THIS move rather than on the next
            // paced frame message: moves are queue-coalesced, so this self-limits
            // to the pump's processing rate and shaves up to a frame of latency
            // off the drag.
            crate::drive_frame_ticks();
            return;
        }

        // Pointer capture: a pressed node with pointer handlers (declarative
        // `on_pointer_*` modifiers) receives every move 1:1 — including outside
        // its bounds — until release. Hover is frozen for the drag's duration.
        if let Some(pid) = self.pressed_id
            && self.node(pid).is_some_and(|n| {
                n.pointer.as_ref().is_some_and(|p| p.on_pointer_moved.is_some())
            })
        {
            self.fire_pointer(pid, x, y, |p| p.on_pointer_moved.as_ref());
            return;
        }

        // Fade the scrollbar thumb in for whichever scroll container is hovered.
        self.update_hovered_scroll(self.scroll_at(x, y));

        let now = self.interactive_at(x, y);

        // Per-segment hover on a SelectorBar: the hot segment changes while the
        // pointer stays on the same node, so track it before the same-node
        // early-out below.
        // Record the new hot segment only — ink placement and the label
        // repaint are deferred until after the hover flips below, so both
        // always see consistent hover state (an inline repaint here would run
        // with a stale `hovered` on re-entry and skip the label brightening).
        let mut seg_hot_moved = false;
        if let Some(id) = now
            && self
                .node(id)
                .is_some_and(|n| n.kind == ControlKind::SelectorBar && n.paint.is_enabled)
            && let Some(hot) = self.segment_at(id, x)
            && self.node(id).is_some_and(|n| n.ctrl.hot_index != hot)
        {
            if let Some(n) = self.node_mut(id) {
                n.ctrl.hot_index = hot;
                n.mark_dirty();
            }
            seg_hot_moved = true;
        }

        // Hover moves over a viz pointer surface (EQ node highlight etc.) —
        // XAML `PointerMoved` fires on hover, not only during a press, and it fires
        // on EVERY move: this must run before the same-interactive-node early-out
        // below, or a surface only ever hears the single move that crossed a button
        // boundary. Track which surface holds the hover so leaving it (to another
        // surface, to none) can fire its `exited` sink — there is no per-node exit
        // event otherwise.
        let surf = self.surface_at(x, y);
        let now_surface = surf.as_ref().map(|(sid, ..)| *sid);
        if self.hovered_surface != now_surface {
            self.fire_surface_exit();
            self.hovered_surface = now_surface;
        }
        if let Some((sid, sinks, ax, ay)) = surf {
            self.fire_surface(sid, &sinks.moved, ax, ay, false, 0);
        }

        if now == self.hovered_id {
            // Same node, new segment: hover state is already correct — snap
            // the ink to the new segment and refresh the labels.
            if seg_hot_moved
                && let Some(id) = now
            {
                if let Some(n) = self.node_mut(id) {
                    parts::seg_hot_changed(n);
                }
                self.repaint();
            }
            return;
        }
        let mut redraw = false;
        if let Some(old) = self.hovered_id.take() {
            redraw |= self.hover_flip(old, false);
        }
        if let Some(new) = now {
            redraw |= self.hover_flip(new, true);
            self.fire_pointer(new, x, y, |p| p.on_pointer_moved.as_ref());
        }
        self.hovered_id = now;
        if redraw {
            self.repaint();
        }
    }

    /// Flip a node's hover state. Converted (chrome-part) kinds retarget their
    /// compositor ink fades directly; kinds with painted hover feedback mark
    /// dirty and return `true` so the caller repaints once — AFTER the flip,
    /// so paint always sees the new hover state.
    fn hover_flip(&mut self, id: ControlId, hovered: bool) -> bool {
        let mut redraw = false;
        if let Some(n) = self.node_mut(id) {
            n.hovered = hovered;
            match n.kind {
                ControlKind::SelectorBar => {
                    // Label brightening is painted; entering keeps the hot
                    // segment recorded by the caller, leaving clears it.
                    if !hovered {
                        n.ctrl.hot_index = -1;
                    }
                    n.mark_dirty();
                    redraw = true;
                }
                // Painted hover feedback (outline brighten / link recolor /
                // NumberBox spin-chevron brighten): one event-driven repaint
                // per flip, no tick.
                ControlKind::CheckBox
                | ControlKind::HyperlinkButton
                | ControlKind::NumberBox => {
                    n.mark_dirty();
                    redraw = true;
                }
                _ => {}
            }
            if parts::converted(n.kind) {
                parts::ink_state_changed(n);
            }
        }
        redraw
    }

    /// Fire the `exited` sink of the surface that held the hover, if any (and if
    /// it is still mounted with a live registration).
    fn fire_surface_exit(&mut self) {
        if let Some(old) = self.hovered_surface.take()
            && let Some(node) = self.node(old)
            && !node.ident.is_null()
            && let Some(sinks) = super::pointer::sinks_for(node.ident)
            && let Some(cb) = sinks.exited.borrow().as_ref()
        {
            cb();
        }
    }

    pub(crate) fn on_pointer_leave(&mut self) {
        if let Some(old) = self.hovered_id.take() {
            let redraw = self.hover_flip(old, false);
            if redraw {
                self.repaint();
            }
        }
        // A hovered viz pointer surface loses the pointer at the window edge too.
        self.fire_surface_exit();
        // Fade out the scrollbar thumb when the pointer leaves the window.
        self.update_hovered_scroll(None);
    }

    // ── Press / release ──────────────────────────────────────────────────────

    /// Left button down. Returns whether the pointer should be captured.
    pub(crate) fn on_pointer_down(&mut self, x: f32, y: f32) -> bool {
        // Popup open: outside-click light-dismisses; inside is handled on up.
        if self.popup.is_some() {
            let inside = self.popup.as_ref().is_some_and(|p| p.contains(x, y));
            if !inside {
                self.close_popup();
            }
            return false;
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
            return true;
        }

        // A registered viz pointer surface wins over generic controls: it is the
        // deepest interactive thing under the point, and its press starts an
        // implicitly captured drag. But only when it actually LISTENS for presses —
        // a surface with no `down` sink is hover-only (e.g. a plot that lights up
        // under the pointer), and must stay click-transparent so buttons layered
        // over it keep working.
        if let Some((sid, sinks, ax, ay)) = self.surface_at(x, y)
            && sinks.down.borrow().is_some()
        {
            self.pressed_surface = Some((sid, std::rc::Rc::clone(&sinks), ay - y));
            self.fire_surface(sid, &sinks.down, ax, ay, true, 0);
            return true;
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
                return true;
            }
            self.place_caret(id, x);
            self.pressed_id = Some(id);
            return true;
        }

        if let Some(id) = target {
            if let Some(n) = self.node_mut(id) {
                n.pressed = true;
                if parts::converted(n.kind) {
                    // Press ink is a compositor fade — no tick.
                    parts::ink_state_changed(n);
                }
            }
            self.pressed_id = Some(id);
            // Sliders scrub immediately on press (drag capture announced
            // first, so a host's touch highlight is up before the value).
            if self.node(id).map(|n| n.kind) == Some(ControlKind::Slider) {
                self.fire_bool(id, Event::DragStateChanged, true);
                self.slider_to(id, x);
            }
            // Knobs latch a RELATIVE vertical-drag origin (no immediate value
            // change on press — only motion moves it).
            if self.node(id).map(|n| n.kind) == Some(ControlKind::Knob) {
                if let Some(n) = self.node(id) {
                    self.knob_drag = Some((id, n.ctrl.value, y));
                }
                self.fire_bool(id, Event::DragStateChanged, true);
            }
            self.fire_pointer(id, x, y, |p| p.on_pointer_pressed.as_ref());
            true
        } else {
            false
        }
    }

    /// Left button up.
    pub(crate) fn on_pointer_up(&mut self, x: f32, y: f32) {
        // End a viz pointer-surface drag: the surface always sees the release
        // (capture semantics), wherever the pointer is.
        if let Some((sid, sinks, dy)) = self.pressed_surface.take() {
            self.fire_surface(sid, &sinks.up, x, y + dy, false, 0);
            return;
        }

        // End a scrollbar-thumb drag; conceal the thumb unless the pointer is
        // still over its container (leaving conceals it via hover tracking).
        if let Some(sid) = self.dragging_thumb.take() {
            if let Some(n) = self.node_mut(sid) {
                n.thumb_drag = None;
            }
            if self.hovered_scroll != Some(sid) {
                self.set_thumb_shown(sid, false);
            }
            return;
        }

        // Popup open: a click on a row selects it, then dismisses.
        if self.popup.is_some() {
            let hit = self.popup.as_ref().and_then(|p| p.hit(x, y));
            if let Some(idx) = hit {
                self.commit_popup(idx);
            }
            return;
        }

        // A text-field drag ended: just drop the press (no activation / ink).
        if let Some(pid) = self.pressed_id
            && self.node(pid).is_some_and(|n| n.editor.is_some())
        {
            self.pressed_id = None;
            return;
        }

        let Some(id) = self.pressed_id.take() else {
            return;
        };
        if let Some(n) = self.node_mut(id) {
            n.pressed = false;
            if parts::converted(n.kind) {
                parts::ink_state_changed(n);
            }
        }

        // A slider drag ends on release wherever the pointer is (capture
        // semantics) — the mirror of the press-time `DragStateChanged(true)`.
        if self.node(id).map(|n| n.kind) == Some(ControlKind::Slider) {
            self.fire_bool(id, Event::DragStateChanged, false);
        }

        // A knob drag likewise ends on release; drop the latched origin.
        if self.knob_drag.take().is_some() {
            self.fire_bool(id, Event::DragStateChanged, false);
        }

        // Capture semantics: the pressed node always sees the release (a drag
        // must end even when the pointer strays off the control). Activation
        // still requires releasing over the control.
        self.fire_pointer(id, x, y, |p| p.on_pointer_released.as_ref());
        if self.is_over(id, x, y) {
            self.activate_pointer(id, x, y);
        }
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
                    n.mark_dirty();
                }
                // The knob/track glide runs on the compositor: the repaint's
                // parts sync sees the flipped state and retargets the springs.
                self.repaint();
                self.fire_bool(id, Event::Toggled, on);
            }
            ControlKind::CheckBox | ControlKind::ToggleButton => {
                let on = !self.node(id).map(|n| n.ctrl.is_checked).unwrap_or(false);
                if let Some(n) = self.node_mut(id) {
                    n.ctrl.is_checked = on;
                    n.mark_dirty();
                }
                // The CheckBox reveal fades on the compositor (the repaint's
                // parts sync); a ToggleButton's checked chrome just repaints.
                self.repaint();
                self.fire_bool(id, Event::Checked, on);
            }
            ControlKind::Expander => {
                let ex = !self.node(id).map(|n| n.ctrl.expanded).unwrap_or(false);
                if let Some(n) = self.node_mut(id) {
                    n.ctrl.expanded = ex;
                    n.mark_dirty();
                }
                self.fire_bool(id, Event::Expanding, ex);
                // The body subtree's `Display::None` flips with `expanded`, so
                // the layout must be recomputed for the body to reclaim/release
                // space (this also repaints the flipped chevron).
                self.relayout_and_paint();
            }
            ControlKind::ComboBox | ControlKind::DropDownButton | ControlKind::SplitButton => {
                self.open_popup(id);
            }
            ControlKind::Button | ControlKind::RepeatButton | ControlKind::HyperlinkButton => {
                // A Button carrying a MenuFlyout (e.g. "+ Add Processor") opens its
                // menu in the popup overlay, mirroring the native WinUI button-flyout.
                if self.node(id).is_some_and(|n| !n.ctrl.menu.is_empty()) {
                    self.open_popup(id);
                    return;
                }
                if let Some(h) = self.node(id).and_then(|n| n.handler(Event::Click)) {
                    h.invoke();
                }
                if let Some(p) = self.node(id).and_then(|n| n.pointer.as_ref())
                    && let Some(cb) = &p.on_tapped
                {
                    cb.invoke(());
                }
            }
            // Any other node that hit-tested as clickable (a Border/panel made
            // interactive via a Click handler or `on_tapped`, e.g. the nav-rail
            // items) activates the same way a Button does — the press wouldn't
            // have reached here otherwise.
            _ => {
                if let Some(h) = self.node(id).and_then(|n| n.handler(Event::Click)) {
                    h.invoke();
                }
                if let Some(p) = self.node(id).and_then(|n| n.pointer.as_ref())
                    && let Some(cb) = &p.on_tapped
                {
                    cb.invoke(());
                }
            }
        }
    }

    /// The segment index under window-relative `x`, or `None` for an empty bar.
    fn segment_at(&self, id: ControlId, x: f32) -> Option<i32> {
        let node = self.node(id)?;
        let n = node.ctrl.items.len();
        if n == 0 {
            return None;
        }
        let edges = controls::segment_edges(node);
        let rel = x - node.rect.x;
        let i = edges[1..n].iter().take_while(|&&e| rel >= e).count();
        Some(i as i32)
    }

    fn select_segment(&mut self, id: ControlId, x: f32) {
        if let Some(i) = self.segment_at(id, x) {
            self.set_segment(id, i);
        }
    }

    fn set_segment(&mut self, id: ControlId, i: i32) {
        let label = {
            let Some(n) = self.node_mut(id) else { return };
            if n.ctrl.selected_index == i || !n.paint.is_enabled {
                return;
            }
            n.ctrl.selected_index = i;
            n.mark_dirty();
            n.ctrl.items.get(i as usize).cloned().unwrap_or_default()
        };
        self.repaint();
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
        self.set_nav_index(id, i);
    }

    /// Select NavigationView item `i` (the by-index core `select_nav` and UIA
    /// `SelectionItem::Select` both route through).
    fn set_nav_index(&mut self, id: ControlId, i: i32) {
        let tag = {
            let Some(nd) = self.node_mut(id) else { return };
            if nd.ctrl.selected_index == i {
                return;
            }
            nd.ctrl.selected_index = i;
            nd.mark_dirty();
            nd.ctrl.tags.get(i as usize).cloned().unwrap_or_default()
        };
        // Indicator glide + glyph recolor both flow from the repaint (the
        // parts sync glides the tile/bar on the compositor).
        self.repaint();
        self.fire_string(id, Event::SelectionChanged, tag);
    }

    /// Select ComboBox item `i` directly (UIA `SelectionItem::Select`), mirroring
    /// the popup-commit path without opening the dropdown.
    fn set_combo_index(&mut self, id: ControlId, i: i32) {
        {
            let Some(n) = self.node_mut(id) else { return };
            if n.ctrl.selected_index == i {
                return;
            }
            n.ctrl.selected_index = i;
            n.mark_dirty();
        }
        self.repaint();
        self.fire_i32(id, Event::SelectionChanged, i);
    }

    /// Map pointer x to a slider value, clamp/quantize, and report it. The
    /// fill/halo/thumb parts snap 1:1 with the pointer — plain compositor
    /// property sets, no repaint and no tick.
    fn slider_to(&mut self, id: ControlId, x: f32) {
        let (value, recolor) = {
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
            // Crossing the fill origin flips the two-tone fill color — a
            // discrete edge, handled by one dirty repaint whose parts sync
            // rebinds the fill's atlas source. Scrub motion stays pure
            // property snaps.
            let old = n.ctrl.value;
            let recolor = n
                .ctrl
                .fill_origin
                .is_some_and(|o| (old <= o) != (v <= o));
            n.ctrl.value = v;
            if !parts::slider_drag(n, frac) || recolor {
                // Also the parts-not-built-yet fallback (first interaction
                // before first paint): the repaint's sync snaps them.
                n.mark_dirty();
            }
            (v, recolor)
        };
        if recolor {
            self.repaint();
        }
        self.fire_f64(id, Event::ValueChanged, value);
    }

    /// Map a relative vertical drag to a knob value: `dy` (up = increase) over
    /// `KNOB_DRAG_RANGE` DIPs covers the whole `[min, max]` domain, from the
    /// value latched at press. The needle/arc glide runs on the compositor via
    /// the repaint's knob sync — the drag itself just repaints (readout) and
    /// retargets the spring.
    fn knob_drag_to(&mut self, id: ControlId, origin: f64, y0: f32, y: f32) {
        /// A full-height drag of this many DIPs sweeps the whole domain.
        const KNOB_DRAG_RANGE: f32 = 200.0;
        let value = {
            let Some(n) = self.node_mut(id) else { return };
            let span = n.ctrl.max - n.ctrl.min;
            if span == 0.0 {
                return;
            }
            let dy = (y0 - y) as f64; // up (decreasing y) increases
            let mut v = (origin + (dy / KNOB_DRAG_RANGE as f64) * span).clamp(n.ctrl.min, n.ctrl.max);
            if let Some(step) = n.ctrl.step
                && step > 0.0
            {
                v = (v / step).round() * step;
                v = v.clamp(n.ctrl.min, n.ctrl.max);
            }
            n.ctrl.value = v;
            n.mark_dirty();
            v
        };
        self.repaint();
        self.fire_f64(id, Event::ValueChanged, value);
    }

    /// Advance a knob value by `detents` mouse-wheel detents (5% of the domain
    /// each), clamped/quantized, and report it.
    fn knob_wheel(&mut self, id: ControlId, detents: f64) {
        /// Fraction of the domain one wheel detent advances.
        const KNOB_WHEEL_FRAC: f64 = 0.05;
        let value = {
            let Some(n) = self.node_mut(id) else { return };
            let span = n.ctrl.max - n.ctrl.min;
            if span == 0.0 {
                return;
            }
            let mut v = (n.ctrl.value + detents * KNOB_WHEEL_FRAC * span).clamp(n.ctrl.min, n.ctrl.max);
            if let Some(step) = n.ctrl.step
                && step > 0.0
            {
                v = (v / step).round() * step;
                v = v.clamp(n.ctrl.min, n.ctrl.max);
            }
            n.ctrl.value = v;
            n.mark_dirty();
            v
        };
        self.repaint();
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
        match Popup::open(&self.comp, owner, rows, rect, self.dip_size, combo, selected, false) {
            Ok(p) => {
                self.close_popup();
                self.popup = Some(p);
            }
            Err(_) => {}
        }
    }

    /// Open, refresh, or dismiss the suggestion dropdown for AutoSuggestBox `owner`
    /// from its current `ctrl.items` (the app's filtered list, set via `Prop::Items`).
    /// Only the focused field shows its list; an empty list dismisses. The popup is
    /// refreshed in place while open so it does not re-pop on each keystroke.
    pub(crate) fn refresh_suggest(&mut self, owner: ControlId) {
        if self.node(owner).map(|n| n.kind) != Some(ControlKind::AutoSuggestBox) {
            return;
        }
        let focused = self.focused_id == Some(owner);
        let rows: Vec<MenuRow> = if focused {
            self.node(owner)
                .map(|n| {
                    n.ctrl
                        .items
                        .iter()
                        .map(|s| MenuRow {
                            text: s.clone(),
                            tag: s.clone(),
                            enabled: true,
                            ..Default::default()
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // An already-open suggestion popup for this field refreshes in place; a
        // different/closed popup is (re)opened only when there are rows to show.
        let mine = self
            .popup
            .as_ref()
            .is_some_and(|p| p.suggest && p.owner == owner);
        if rows.is_empty() {
            if mine {
                self.close_popup();
            }
            return;
        }
        if mine {
            if let Some(p) = &mut self.popup {
                p.update_items(&self.comp, rows);
            }
            return;
        }
        let rect = self
            .node(owner)
            .map(|n| CanvasRect::from_xywh(n.rect.x, n.rect.y, n.rect.w, n.rect.h));
        let Some(rect) = rect else { return };
        if let Ok(p) = Popup::open(&self.comp, owner, rows, rect, self.dip_size, false, -1, true) {
            self.close_popup();
            self.popup = Some(p);
        }
    }

    /// Commit a chosen suggestion to its AutoSuggestBox: set the field text to the
    /// suggestion, fire `SuggestionChosen`, and dismiss the dropdown (focus stays).
    fn choose_suggestion(&mut self, idx: usize) {
        let Some(p) = &self.popup else { return };
        let owner = p.owner;
        let Some(text) = p.row_tag(idx) else { return };
        self.close_popup();
        if let Some(n) = self.node_mut(owner) {
            if let Some(e) = &mut n.editor {
                e.set_text(&text);
                e.seeded = true;
                e.caret_moved = true;
            }
            n.mark_dirty();
        }
        self.repaint();
        self.fire_string(owner, Event::SuggestionChosen, text);
    }

    /// Close the open popup with a compositor-side dismiss fade. The overlay
    /// visual is parked as a [`Ghost`] and released when the scoped batch
    /// wrapping its fade completes — no app frames, no timer.
    fn close_popup(&mut self) {
        if let Some(p) = self.popup.take() {
            let batch = self
                .comp
                .compositor()
                .CreateScopedBatch(CompositionBatchTypes::Animation);
            let shown = p.into_exit(&self.comp);
            match (shown, batch) {
                (Some(v), Ok(batch)) => self.park_ghost(v, batch),
                (Some(v), Err(_)) => {
                    // No completion signal possible — drop the overlay now
                    // (skipping the fade) rather than leak it.
                    self.comp.remove_root_visual(&v);
                }
                (None, _) => {}
            }
        }
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
        if p.suggest {
            self.choose_suggestion(idx);
            return;
        }
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

    /// Mouse wheel at (x, y) DIPs, `delta` in WHEEL_DELTA (120) units. The
    /// scroll glide plays on the compositor — nothing here needs the timer.
    pub(crate) fn on_wheel(&mut self, x: f32, y: f32, delta: i32) {
        // A viz pointer surface that subscribed the wheel (EQ Q-adjust) consumes
        // it; surfaces without a wheel sink fall through to scrolling.
        if let Some((sid, sinks, ax, ay)) = self.surface_at(x, y)
            && sinks.wheel.borrow().is_some()
        {
            self.fire_surface(sid, &sinks.wheel, ax, ay, false, delta);
            return;
        }

        // A focused NumberBox under the pointer steps on the wheel.
        if let Some(id) = self.focused_editable()
            && self.node(id).map(|n| n.kind) == Some(ControlKind::NumberBox)
            && self.node(id).is_some_and(|n| n.rect.contains(x, y))
        {
            self.number_step(id, if delta > 0 { 1.0 } else { -1.0 }, false);
            return;
        }

        // A knob under the pointer adjusts on the wheel (5% of the domain per
        // detent), consuming it before it falls through to scrolling.
        if let Some(id) = self.interactive_at(x, y)
            && self.node(id).map(|n| n.kind) == Some(ControlKind::Knob)
        {
            self.knob_wheel(id, delta as f64 / 120.0);
            return;
        }

        if let Some(id) = self.scroll_at(x, y) {
            // 48 DIPs per detent, downward wheel scrolls content up. The glide
            // is a compositor spring on the content carrier (and the thumb),
            // retargeted per detent — no tick, no repaint. `scroll_off` jumps
            // to the destination immediately: it is the LOGICAL scroll offset
            // (hit-testing, thumb geometry), and mid-glide hits land where the
            // content is about to settle.
            let step = -(delta as f32 / 120.0) * 48.0;
            let scale = self.scale();
            let max = self.node(id).map(|n| (n.ctrl.content_h - n.rect.h).max(0.0)).unwrap_or(0.0);
            if let Some(n) = self.node_mut(id) {
                let target = layout::snap((n.scroll_off + step).clamp(0.0, max), scale);
                n.scroll_off = target;
                n.scroll_glide(target);
                let g = scroll::thumb_geom(n.rect.h, n.ctrl.content_h, target);
                let tx = n.rect.w - scroll::THUMB_W - scroll::THUMB_MARGIN;
                n.thumb_glide(tx, g.thumb_y);
            }
            // Reveal the thumb while scrolling (it conceals when the pointer
            // leaves the container).
            self.update_hovered_scroll(Some(id));
        }
    }

    // ── Keyboard ─────────────────────────────────────────────────────────────

    /// A key was pressed (`shift` / `ctrl` held?). Returns `true` if a
    /// spring/timer should run.
    pub(crate) fn on_key(&mut self, vk: u32, shift: bool, ctrl: bool) {
        // A focused text editor consumes editing keys before the generic ring.
        if let Some(id) = self.focused_editable()
            && self.editor_key(id, vk, shift, ctrl).is_some()
        {
            return;
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
            return;
        }

        match vk {
            VK_TAB => self.move_focus(if shift { -1 } else { 1 }),
            VK_SPACE | VK_RETURN => {
                if let Some(id) = self.focused_id {
                    self.activate(id);
                }
            }
            VK_LEFT | VK_UP => self.focus_arrow(-1),
            VK_RIGHT | VK_DOWN => self.focus_arrow(1),
            _ => {}
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

    /// Restart the focused field's caret blink so it picks up the current
    /// system blink period (`GetCaretBlinkTime` is re-read at animation
    /// start). Called on `WM_SETTINGCHANGE`, where a control-panel blink-rate
    /// change lands.
    pub(crate) fn refresh_caret_blink(&mut self) {
        if let Some(id) = self.focused_editable() {
            if let Some(n) = self.node_mut(id) {
                if let Some(e) = &mut n.editor {
                    e.caret_moved = true;
                }
                n.mark_dirty();
            }
            self.repaint();
        }
    }

    /// Show / hide the focused field's caret when the host window gains or
    /// loses activation (keyboard focus is retained either way). Re-activating
    /// restarts the blink solid-first.
    pub(crate) fn window_focus_changed(&mut self, focused: bool) {
        if let Some(id) = self.focused_editable() {
            if let Some(n) = self.node_mut(id) {
                if let Some(e) = &mut n.editor {
                    e.caret_shown = focused;
                    e.caret_moved = true;
                }
                n.mark_dirty();
            }
            self.repaint();
        }
    }

    fn with_editor<R>(&mut self, id: ControlId, f: impl FnOnce(&mut editor::Editor) -> R) -> Option<R> {
        self.node_mut(id).and_then(|n| n.editor.as_mut()).map(f)
    }

    /// Route an editing key to the focused editor. Returns `Some(())` when
    /// consumed, or `None` to let the generic ring handle it (e.g. Tab).
    fn editor_key(&mut self, id: ControlId, vk: u32, shift: bool, ctrl: bool) -> Option<()> {
        let kind = self.node(id)?.kind;
        if vk == VK_TAB {
            return None; // Tab leaves the field (commit happens in set_focus).
        }

        // An open suggestion dropdown for this field captures arrow / Enter / Esc
        // (printable input still falls through to the editor below).
        let suggesting = self
            .popup
            .as_ref()
            .is_some_and(|p| p.suggest && p.owner == id);
        if suggesting {
            match vk {
                VK_DOWN => {
                    if let Some(p) = &mut self.popup {
                        p.move_highlight(1, &self.comp);
                    }
                    return Some(());
                }
                VK_UP => {
                    if let Some(p) = &mut self.popup {
                        p.move_highlight(-1, &self.comp);
                    }
                    return Some(());
                }
                VK_ESCAPE => {
                    self.close_popup();
                    return Some(());
                }
                VK_RETURN => {
                    let h = self.popup.as_ref().map(|p| p.hovered);
                    if let Some(i) = h.filter(|&i| i != usize::MAX) {
                        self.choose_suggestion(i);
                    } else {
                        let t = self.with_editor(id, |e| e.text()).unwrap_or_default();
                        self.close_popup();
                        self.fire_string(id, Event::QuerySubmitted, t);
                    }
                    return Some(());
                }
                _ => {}
            }
        }
        if ctrl {
            match vk {
                VK_A => {
                    self.with_editor(id, |e| e.select_all());
                    self.editor_caret_moved(id);
                    return Some(());
                }
                VK_C => {
                    self.editor_copy(id);
                    return Some(());
                }
                VK_X => {
                    self.editor_cut(id);
                    return Some(());
                }
                VK_V => {
                    self.editor_paste(id);
                    return Some(());
                }
                VK_LEFT => {
                    self.with_editor(id, |e| e.move_left(true, shift));
                    self.editor_caret_moved(id);
                    return Some(());
                }
                VK_RIGHT => {
                    self.with_editor(id, |e| e.move_right(true, shift));
                    self.editor_caret_moved(id);
                    return Some(());
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
        Some(())
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
                e.caret_moved = true;
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
                    e.caret_moved = true;
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
            ControlKind::TextBox => self.fire_string(id, Event::TextChanged, text),
            ControlKind::AutoSuggestBox => {
                self.fire_string(id, Event::TextChanged, text);
                // Reflect the edit in the suggestion dropdown from whatever rows the
                // node currently carries; the app's filtered list (set on the next
                // render via `Prop::Items`) refreshes it again in place.
                self.refresh_suggest(id);
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
                e.caret_moved = true;
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
                e.caret_moved = true;
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
                e.caret_moved = true;
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
    fn focus_arrow(&mut self, dir: i32) {
        let Some(id) = self.focused_id else { return };
        match self.node(id).map(|n| n.kind) {
            Some(ControlKind::Slider | ControlKind::Knob) => {
                let value = {
                    let Some(n) = self.node_mut(id) else { return };
                    let step = n.ctrl.step.unwrap_or((n.ctrl.max - n.ctrl.min) / 20.0);
                    let v = (n.ctrl.value + dir as f64 * step).clamp(n.ctrl.min, n.ctrl.max);
                    n.ctrl.value = v;
                    n.mark_dirty();
                    v
                };
                // The fill/thumb (slider) or arc/needle (knob) glide runs on the
                // compositor via the repaint's sync — a keyboard nudge needs no tick.
                self.repaint();
                self.fire_f64(id, Event::ValueChanged, value);
            }
            Some(ControlKind::SelectorBar) => {
                let (cur, n) = self
                    .node(id)
                    .map(|nd| (nd.ctrl.selected_index, nd.ctrl.items.len() as i32))
                    .unwrap_or((0, 0));
                if n > 0 {
                    self.set_segment(id, (cur + dir).clamp(0, n - 1));
                }
            }
            _ => {}
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
        // Dismiss an AutoSuggestBox's open dropdown when its field loses focus.
        if let Some(old) = self.focused_id
            && Some(old) != id
            && self
                .popup
                .as_ref()
                .is_some_and(|p| p.suggest && p.owner == old)
        {
            self.close_popup();
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
                e.caret_moved = true;
            }
            n.mark_dirty();
        }
        self.repaint();
        // Notify any listening UI Automation client of the focus move (no-op when
        // no AT / test harness is attached — see `uia_raise_focus`).
        if let Some(new) = id {
            self.uia_raise_focus(new);
        }
    }

    // ── UIA action bridge ────────────────────────────────────────────────────
    //
    // Each method translates one UI-Automation pattern call into the *same* typed
    // event dispatch a pointer/keyboard interaction would take — there is a single
    // action path. Called on the UI thread (the provider marshals here).

    /// Invoke / Toggle: identical to a click or Space activation.
    pub(crate) fn uia_activate(&mut self, id: ControlId) {
        self.activate(id);
    }

    /// `IRawElementProviderFragment::SetFocus`: give the control keyboard focus
    /// (with the visible ring), the same as Tabbing to it.
    pub(crate) fn uia_focus_node(&mut self, id: ControlId) {
        if self.node(id).is_some_and(|n| n.focusable) {
            self.set_focus(Some(id), true);
        }
    }

    /// `Value::SetValue` for an editable text field (TextBox / PasswordBox /
    /// AutoSuggestBox / NumberBox): replace the buffer and fire the field's change
    /// event, as if typed and committed.
    pub(crate) fn uia_set_text(&mut self, id: ControlId, s: &str) {
        let kind = match self.node(id) {
            Some(n) if n.editor.is_some() => n.kind,
            _ => return,
        };
        if let Some(n) = self.node_mut(id) {
            if let Some(e) = &mut n.editor {
                e.set_text(s);
                e.seeded = true;
                e.caret_moved = true;
            }
            n.mark_dirty();
        }
        if kind == ControlKind::NumberBox {
            // Parse / clamp / round / format and fire ValueChanged.
            self.commit_number(id);
        } else {
            self.editor_after_edit(id);
        }
    }

    /// `RangeValue::SetValue` for a Slider (and NumberBox): clamp into range and
    /// fire `ValueChanged`, the same as an arrow-key nudge.
    pub(crate) fn uia_set_range(&mut self, id: ControlId, v: f64) {
        if self.node(id).map(|n| n.kind) == Some(ControlKind::NumberBox) {
            self.apply_number(id, v);
            return;
        }
        let value = {
            let Some(n) = self.node_mut(id) else { return };
            let v = v.clamp(n.ctrl.min, n.ctrl.max);
            n.ctrl.value = v;
            n.mark_dirty();
            v
        };
        // Compositor glide via the repaint's parts sync (see `focus_arrow`).
        self.repaint();
        self.fire_f64(id, Event::ValueChanged, value);
    }

    /// `SelectionItem::Select` on item `i` of a SelectorBar / ComboBox /
    /// NavigationView, routed through the existing per-kind selection path.
    pub(crate) fn uia_select_item(&mut self, id: ControlId, i: i32) {
        match self.node(id).map(|n| n.kind) {
            Some(ControlKind::SelectorBar) => self.set_segment(id, i),
            Some(ControlKind::NavigationView) => self.set_nav_index(id, i),
            Some(ControlKind::ComboBox) => self.set_combo_index(id, i),
            _ => {}
        }
    }

    /// `ExpandCollapse::Expand` / `Collapse`. Expander toggles through `activate`;
    /// the dropdown kinds open/close their popup.
    pub(crate) fn uia_set_expanded(&mut self, id: ControlId, want: bool) {
        match self.node(id).map(|n| n.kind) {
            Some(ControlKind::Expander) => {
                let cur = self.node(id).map(|n| n.ctrl.expanded).unwrap_or(false);
                if cur != want {
                    self.activate(id);
                }
            }
            Some(ControlKind::ComboBox | ControlKind::DropDownButton | ControlKind::SplitButton) => {
                if want {
                    self.open_popup(id);
                } else {
                    self.close_popup();
                }
            }
            _ => {}
        }
    }

    // ── Event dispatch ───────────────────────────────────────────────────────

    // The `fire_*` dispatchers double as the UIA notification choke point: every
    // state change (pointer, keyboard, or UIA-initiated) flows through exactly
    // one of them, so screen readers hear the change even when the app attached
    // no handler.
    fn fire_bool(&self, id: ControlId, event: Event, v: bool) {
        self.uia_notify_bool(id, event, v);
        if let Some(h) = self.node(id).and_then(|n| n.handler(event)) {
            h.invoke_bool(v);
        }
    }
    fn fire_string(&self, id: ControlId, event: Event, v: String) {
        self.uia_notify_string(id, event, &v);
        if let Some(h) = self.node(id).and_then(|n| n.handler(event)) {
            h.invoke_string(v);
        }
    }
    fn fire_f64(&self, id: ControlId, event: Event, v: f64) {
        self.uia_notify_f64(id, event, v);
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
