//! Pointer input: a z-ordered (deepest-wins) AABB hit-test over the layout
//! output, mapped to button hover/press ink (self-stopping springs) and `Click`
//! / pointer-handler dispatch. Coordinates arrive in DIPs (physical pixels /
//! scale) so they line up with the laid-out rects.

use super::*;
use crate::backend::Event;
use crate::style::PointerEventInfo;

impl DCompBackend {
    /// The deepest clickable node (a Button or a node with a click/tap handler)
    /// whose rect contains the point.
    fn clickable_at(&self, x: f32, y: f32) -> Option<ControlId> {
        // The arena has no parent links; re-walk collecting the clickable chain.
        let root = self.root?;
        let mut best = None;
        self.clickable_walk(root, x, y, &mut best);
        best
    }

    fn clickable_walk(&self, id: ControlId, x: f32, y: f32, out: &mut Option<ControlId>) {
        let Some(node) = self.node(id) else { return };
        if node.rect.contains(x, y) && node.is_clickable() {
            *out = Some(id);
        }
        for c in &node.children {
            self.clickable_walk(*c, x, y, out);
        }
    }

    /// Pointer moved to (x, y) DIPs. Updates hover ink; returns `true` if a
    /// spring started (so the host should start the animation timer).
    pub(crate) fn on_pointer_move(&mut self, x: f32, y: f32) -> bool {
        let now = self.clickable_at(x, y);
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

    /// Pointer left the window: clear hover. Returns `true` if a spring started.
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

    /// Left button down at (x, y) DIPs. Presses the clickable under the pointer.
    /// Returns `(captured, needs_timer)` — `captured` means a control took the
    /// press (the host should `SetCapture`).
    pub(crate) fn on_pointer_down(&mut self, x: f32, y: f32) -> (bool, bool) {
        let target = self.clickable_at(x, y);
        if let Some(id) = target {
            if let Some(n) = self.node_mut(id) {
                n.pressed = true;
                n.press.target = 1.0;
            }
            self.animating.insert(id);
            self.pressed_id = Some(id);
            self.fire_pointer(id, x, y, |p| p.on_pointer_pressed.as_ref());
            (true, true)
        } else {
            (false, false)
        }
    }

    /// Left button up at (x, y) DIPs. Releases the press and, if still over the
    /// pressed control, fires its `Click` (and `on_tapped`). Returns
    /// `needs_timer` (the release spring is in flight).
    pub(crate) fn on_pointer_up(&mut self, x: f32, y: f32) -> bool {
        let Some(id) = self.pressed_id.take() else {
            return false;
        };
        if let Some(n) = self.node_mut(id) {
            n.pressed = false;
            n.press.target = 0.0;
        }
        self.animating.insert(id);

        let over = self.node(id).is_some_and(|n| n.rect.contains(x, y));
        if over {
            self.fire_pointer(id, x, y, |p| p.on_pointer_released.as_ref());
            if let Some(n) = self.node(id) {
                if let Some(h) = n.handler(Event::Click) {
                    h.invoke();
                }
                if let Some(p) = &n.pointer
                    && let Some(cb) = &p.on_tapped
                {
                    cb.invoke(());
                }
            }
        }
        true
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

    /// Advance all in-flight springs by `dt` seconds and repaint. Returns `true`
    /// while any spring is still animating (so the host keeps the timer alive).
    pub(crate) fn tick(&mut self, dt: f32) -> bool {
        let ids: Vec<ControlId> = self.animating.iter().copied().collect();
        for id in ids {
            let settled = match self.node_mut(id) {
                Some(n) => {
                    let h = n.hover.step(dt);
                    let p = n.press.step(dt);
                    h && p
                }
                None => true,
            };
            if settled {
                self.animating.remove(&id);
            }
        }
        self.repaint();
        !self.animating.is_empty()
    }

    /// Whether any spring is currently animating (the host uses this to decide
    /// if it must (re)start the timer after a reconcile touched a sprung node).
    pub(crate) fn is_animating(&self) -> bool {
        !self.animating.is_empty()
    }
}
