//! Pointer + keyboard input for the drawn control library: a z-ordered
//! (deepest-wins) AABB hit-test over the layout output, the hover/press ink
//! state machine, control activation (toggle / check / select / segmented /
//! slider / nav / expander), wheel scrolling, the keyboard focus ring with
//! Tab/Shift-Tab + Space/Enter activation, and popup-overlay routing (open /
//! light-dismiss / Up-Down-Enter-Esc). Coordinates arrive in DIPs.

use super::controls;
use super::editor;
use super::host;
use super::popup::{Popup, PopupBody};
use super::*;
use crate::backend::Event;
use crate::style::{PointerEventInfo, WheelAxis};
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
const VK_F1: u32 = 0x70;
const VK_F24: u32 = 0x87;

/// An F1..F24 function key — never a printable/editing key, so it is never
/// claimed by a focused editor and always available to an accelerator (§7.3).
pub(crate) fn is_function_key(vk: u32) -> bool {
    (VK_F1..=VK_F24).contains(&vk)
}

/// The §7.3 fixed conflict policy, editor half: whether a **focused** editor
/// legitimately claims this key, so an accelerator must NOT pre-empt it.
///
/// The editor keeps its own `Ctrl+A`/`C`/`X`/`V` (select-all / clipboard) and
/// every *unmodified* printable/editing key. A modifier-chorded binding (any
/// other `Ctrl`-chord, any `Alt`-chord) and every F-key win over the editor.
/// Tab is not decided here — it always traverses and is never an accelerator.
pub(crate) fn editor_claims_key(vk: u32, ctrl: bool, alt: bool) -> bool {
    if is_function_key(vk) {
        return false;
    }
    if ctrl {
        // Only the editor's own clipboard/select chords, and only as a plain
        // Ctrl-chord: Ctrl+Alt (AltGr) is printable input, not select-all, and
        // other Ctrl-chords (Ctrl+S, Ctrl+Z, …) are the app's to bind.
        return !alt && matches!(vk, VK_A | VK_C | VK_X | VK_V);
    }
    if alt {
        return false;
    }
    // Unmodified: the editor owns it (typing, arrows, Home/End, Backspace, …).
    true
}

/// The sys-key half of §7.3's "return-0 vs `DefWindowProc`" decision, which
/// must stay synchronous. A `WM_SYSKEYDOWN`/`WM_SYSKEYUP` the backend did not
/// consume must fall through to `DefWindowProcW` so Alt+F4, F10 and Alt+Space
/// reach the system; a consumed one (an accelerator match, an editor claim)
/// stays swallowed. Regular (`WM_KEYDOWN`) messages never fall through.
pub(crate) fn sys_key_falls_through(is_sys: bool, consumed: bool) -> bool {
    is_sys && !consumed
}

// ── Host-thread input state ─────────────────────────────────────────────────
//
// Both cells below are per-host-thread rather than `DCompBackend` fields: one
// `DCompHost` (and therefore one backend) exists per thread — see the `DCOMP`
// thread-local in `host` — so thread-local and per-backend are the same scope
// here.

thread_local! {
    /// The last pointer position this backend actually processed, in absolute
    /// client DIPs. Read by [`DCompBackend::on_pointer_cancel`], which is
    /// driven by `WM_CAPTURECHANGED` — a message that carries no coordinates
    /// and may arrive with the cursor already off over some other window.
    static LAST_POINTER: std::cell::Cell<(f32, f32)> = const { std::cell::Cell::new((0.0, 0.0)) };

    /// Bitmap of currently-held virtual keys (VK 0..256, one bit each).
    /// Maintained by `on_key` / `on_key_up` and cleared wholesale when the
    /// window loses activation.
    ///
    /// Its job is to tell a fresh press from an auto-repeat: a key that is
    /// already down when its `WM_KEYDOWN` arrives is the keyboard repeating.
    static KEYS_DOWN: std::cell::Cell<[u64; 4]> = const { std::cell::Cell::new([0; 4]) };

    /// The node a right-button press landed on, so the release can require
    /// down and up on the *same* element before reporting a right-tap.
    static RIGHT_PRESSED: std::cell::Cell<Option<ControlId>> = const { std::cell::Cell::new(None) };
}

fn set_last_pointer(x: f32, y: f32) {
    LAST_POINTER.with(|c| c.set((x, y)));
}

fn last_pointer() -> (f32, f32) {
    LAST_POINTER.with(|c| c.get())
}

/// Mark `vk` held; returns whether it was **already** held — i.e. whether this
/// key-down is an auto-repeat rather than a fresh press.
fn key_press(vk: u32) -> bool {
    if vk >= 256 {
        return false;
    }
    let (word, bit) = ((vk / 64) as usize, 1u64 << (vk % 64));
    KEYS_DOWN.with(|c| {
        let mut m = c.get();
        let was = m[word] & bit != 0;
        m[word] |= bit;
        c.set(m);
        was
    })
}

/// Mark `vk` released.
fn key_release(vk: u32) {
    if vk >= 256 {
        return;
    }
    let (word, bit) = ((vk / 64) as usize, 1u64 << (vk % 64));
    KEYS_DOWN.with(|c| {
        let mut m = c.get();
        m[word] &= !bit;
        c.set(m);
    });
}

/// Forget every held key. Keys released while another window has focus never
/// deliver a `WM_KEYUP` to us, so without this the next genuine press of such a
/// key would look like an auto-repeat and be suppressed.
fn keys_clear() {
    KEYS_DOWN.with(|c| c.set([0; 4]));
}

/// What a [`DCompBackend::hit_test`] walk is looking for.
///
/// The variants differ only in which nodes are eligible to *win* the hit —
/// traversal, coordinate mapping and clipping are identical for all of them, so
/// every consumer resolves the same point to the same place in the tree.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum HitKind {
    /// Nodes that respond to a press ([`Node::is_clickable`]) — pointer routing.
    Interactive,
    /// Scroll containers ([`Node::is_scroll`]) — wheel and thumb routing.
    Scroll,
    /// Every node, whatever its kind. This is the arm UI Automation's
    /// `ElementProviderFromPoint` wants: it must resolve to the topmost element
    /// at the point, interactive or not.
    Any,
}

impl HitKind {
    /// Whether `n` is eligible to win a hit for this kind.
    fn eligible(self, n: &Node) -> bool {
        match self {
            Self::Interactive => n.is_clickable(),
            Self::Scroll => n.is_scroll(),
            Self::Any => true,
        }
    }
}

impl DCompBackend {
    // ── Hit-testing ──────────────────────────────────────────────────────────

    /// Resolve absolute client-DIP `(x, y)` to the node that owns that point,
    /// for the given [`HitKind`]. This is the single hit-test authority — every
    /// consumer (pointer routing, wheel routing, UI Automation) must go through
    /// it so a click and an `ElementProviderFromPoint` can never disagree about
    /// what is under the cursor.
    ///
    /// Contract:
    ///
    /// * **Coordinates** are absolute, in client DIPs, as delivered by the host
    ///   wndproc. The walk maps them into each node's own layout space as it
    ///   descends, adding the `scroll_off` of every ancestor scroll container
    ///   (whose children are laid out unscrolled).
    /// * **Z-order is paint order.** Children paint over their parent and later
    ///   siblings paint over earlier ones, so the *last* eligible node in the
    ///   DFS wins — i.e. the visually topmost one. Two overlapping siblings
    ///   resolve to the one drawn on top.
    /// * **A plain miss prunes nothing.** A child may legitimately extend past
    ///   its parent's box (a knob's overhanging halo, an absolutely-positioned
    ///   overlay), so a subtree is still searched when the point is outside the
    ///   parent's rect.
    /// * **A clipped miss prunes the whole subtree.** A node that clips its
    ///   children to its own bounds (a scroll viewport, a progress track — the
    ///   nodes carrying a composition `InsetClip`) does not *draw* content
    ///   outside those bounds, so that content must not hit-test either.
    ///   Without this, rows scrolled out of a `ScrollViewer` stay clickable.
    ///
    /// Returns `None` when the point lands on nothing eligible, or when no tree
    /// is mounted.
    pub(crate) fn hit_test(&self, x: f32, y: f32, kind: HitKind) -> Option<ControlId> {
        let root = self.root?;
        let mut best = None;
        self.hit_walk(root, x, y, &mut best, kind);
        best
    }

    /// The deepest interactive node containing the point, accounting for the
    /// scroll offset of any ancestor scroll container.
    pub(super) fn interactive_at(&self, x: f32, y: f32) -> Option<ControlId> {
        self.hit_test(x, y, HitKind::Interactive)
    }

    /// The deepest scroll container containing the point.
    fn scroll_at(&self, x: f32, y: f32) -> Option<ControlId> {
        self.hit_test(x, y, HitKind::Scroll)
    }

    /// The deepest registered viz pointer surface (knob/slider/EQ canvas — see
    /// `pointer.rs`) under the point, with its declared presence bits and the
    /// scroll-adjusted point for element-relative coordinates. The router reads
    /// only the bits — the sink closures live app-side. Cheap `None` when nothing
    /// is registered.
    pub(super) fn surface_at(
        &self,
        x: f32,
        y: f32,
    ) -> Option<(ControlId, pointer::SurfaceInterest, f32, f32)> {
        if !pointer::has_listeners() {
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
        out: &mut Option<(ControlId, pointer::SurfaceInterest, f32, f32)>,
    ) {
        let Some(node) = self.node(id) else { return };
        let inside = node.rect.contains(x, y);
        if inside && let Some(interest) = pointer::interest_for(id) {
            *out = Some((id, interest, x, y));
        }
        // Same clip rule as `hit_walk`: a surface scrolled out of an ancestor
        // viewport is not drawn, so it must not take the pointer either.
        if !inside && node.clip.is_some() {
            return;
        }
        let child_y = if node.is_scroll() { y + node.scroll_off } else { y };
        for c in &node.children {
            self.surface_walk(*c, x, child_y, out);
        }
    }

    /// Queue a pointer transition to a viz surface's sink with element-relative
    /// DIP coordinates. `(x, y)` must be in the node's layout space (scroll-
    /// adjusted, as returned by [`surface_at`](Self::surface_at)). The closure is
    /// never invoked here: this pushes an [`Intent::Surface`] the recorder drains
    /// against the app-side sink map after the input borrow is released.
    ///
    /// Reports the vertical wheel axis — correct for every pointer transition
    /// (which carries `wheel_delta` 0) and for the classic wheel. The
    /// horizontal tilt goes through [`queue_surface_wheel`](Self::queue_surface_wheel).
    fn queue_surface(
        &mut self,
        id: ControlId,
        kind: record::SurfaceIntentKind,
        x: f32,
        y: f32,
        left: bool,
        wheel_delta: i32,
    ) {
        self.queue_surface_wheel(id, kind, x, y, left, wheel_delta, WheelAxis::Vertical);
    }

    /// [`queue_surface`](Self::queue_surface) with an explicit wheel axis, so a
    /// surface sink can tell a sideways tilt from a wheel turn and opt in to
    /// (or ignore) each independently.
    #[allow(clippy::too_many_arguments)]
    fn queue_surface_wheel(
        &mut self,
        id: ControlId,
        kind: record::SurfaceIntentKind,
        x: f32,
        y: f32,
        left: bool,
        wheel_delta: i32,
        wheel_axis: WheelAxis,
    ) {
        let Some(node) = self.node(id) else { return };
        let info = PointerEventInfo {
            x: (x - node.rect.x) as f64,
            y: (y - node.rect.y) as f64,
            is_left_button_pressed: left,
            wheel_delta,
            wheel_axis,
            ..PointerEventInfo::default()
        };
        self.intents.push(record::Intent::Surface { id, kind, info });
    }

    /// Whether `(x, y)` (absolute DIP) lies over scroll container `id`'s thumb.
    /// Returns the pointer→thumb-top offset (for drag tracking) when it does.
    fn thumb_at(&self, id: ControlId, x: f32, y: f32) -> Option<f32> {
        let n = self.node(id)?;
        let g = scroll::thumb_geom(n.rect.h, n.ctrl().content_h, n.scroll_off);
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
            let g = scroll::thumb_geom(n.rect.h, n.ctrl().content_h, n.scroll_off);
            // The app's visibility policy overrides the hover edge — an
            // always-visible bar ignores the conceal, a hidden one ignores the
            // reveal. Overflow still gates both: there is nothing to indicate
            // when the content fits.
            let show = g.overflow
                && match scroll::reveal_policy(n.extras().v_scrollbar) {
                    scroll::Reveal::Always => true,
                    scroll::Reveal::Never => false,
                    scroll::Reveal::OnDemand => shown,
                };
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
            Some(n) => (n.rect.y, n.rect.h, n.ctrl().content_h, n.thumb_drag.unwrap_or(0.0)),
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

    /// The recursive body of [`hit_test`](Self::hit_test) — see that method for
    /// the full contract. `y` arrives pre-adjusted for ancestor scroll, and
    /// `out` accumulates the last (topmost) eligible node seen.
    fn hit_walk(&self, id: ControlId, x: f32, y: f32, out: &mut Option<ControlId>, kind: HitKind) {
        let Some(node) = self.node(id) else { return };
        let inside = node.rect.contains(x, y);
        if inside && kind.eligible(node) {
            // Later in the DFS == painted later == on top: overwrite freely.
            *out = Some(id);
        }
        // A node that clips its children to its own bounds (the composition
        // `InsetClip` minted for scroll viewports and progress tracks) hides
        // everything below it that falls outside those bounds — so a miss here
        // ends the subtree. A miss on a NON-clipping node prunes nothing: its
        // children may legitimately overhang it.
        if !inside && node.clip.is_some() {
            return;
        }
        let child_y = if node.is_scroll() { y + node.scroll_off } else { y };
        for c in &node.children {
            self.hit_walk(*c, x, child_y, out, kind);
        }
    }

    // ── Hover ────────────────────────────────────────────────────────────────

    /// Pointer moved to (x, y) DIPs.
    pub(crate) fn on_pointer_move(&mut self, x: f32, y: f32) {
        set_last_pointer(x, y);
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
            // The gesture is now streaming updates, so every value chrome —
            // this slider, the dial trailing it, the output meter — tracks 1:1
            // instead of springing (see `mod::scrubbing`).
            self.scrubbing = true;
            self.slider_to(pid, x);
            return;
        }

        // A pressed knob scrubs on a relative vertical drag (up = increase).
        if let Some((id, origin, y0)) = self.knob_drag {
            self.scrubbing = true;
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
        //
        // The move queues a `moved` intent; the host drives one frame tick after
        // the drained sink runs (`IntentJob::drives_frame_tick`), so the preview
        // repaints from this move within the same message rather than waiting for
        // the next paced frame — moves are queue-coalesced, so it self-limits to
        // the pump's processing rate and shaves up to a frame of latency off the
        // drag.
        if let Some((sid, dy)) = self.pressed_surface {
            self.queue_surface(sid, record::SurfaceIntentKind::Move, x, y + dy, true, 0);
            return;
        }

        // Pointer capture: a pressed node with a declared `on_pointer_moved`
        // receives every move 1:1 — including outside its bounds — until
        // release. Hover is frozen for the drag's duration.
        if let Some(pid) = self.pressed_id
            && self.node(pid).is_some_and(|n| n.pointer.moved)
        {
            self.fire_pointer(pid, x, y, record::PointerIntentKind::Moved);
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
            && self.node(id).is_some_and(|n| n.ctrl().hot_index != hot)
        {
            if let Some(n) = self.node_mut(id) {
                n.ctrl_mut().hot_index = hot;
                n.mark_dirty();
            }
            seg_hot_moved = true;
        }

        // Per-row hover in a nav pane, tracked for the same reason and before
        // the same early-out: the hot row changes while the pointer stays on the
        // one NavigationView node, so the node-level hover flip below never sees
        // it. The row ink is a compositor sprite placed by the parts sync, and
        // the two chrome buttons repaint their flat wash — both keyed on this
        // one index (see `nav::HOT_BACK` and friends for why chrome sits at
        // sentinel values rather than in the item range).
        // An InfoBar's close button, tracked for the same reason and before the
        // same early-out: the pointer crosses onto the button while staying on
        // the one InfoBar node, so the node-level hover flip below never sees
        // it. Its wash is a flat state fill, so recording the hot slot and
        // marking dirty IS the whole affordance — no sprite, no tick.
        if let Some(id) = now
            && self
                .node(id)
                .is_some_and(|n| n.kind == ControlKind::InfoBar && n.paint.is_enabled)
        {
            let hot = if self
                .node(id)
                .is_some_and(|n| info_bar::hit_close(n, x - n.rect.x, y - n.rect.y))
            {
                info_bar::HOT_CLOSE
            } else {
                -1
            };
            if self.node(id).is_some_and(|n| n.ctrl().hot_index != hot)
                && let Some(n) = self.node_mut(id)
            {
                n.ctrl_mut().hot_index = hot;
                n.mark_dirty();
                self.repaint();
            }
        }

        let mut nav_hot_moved = false;
        if let Some(id) = now
            && self
                .node(id)
                .is_some_and(|n| n.kind == ControlKind::NavigationView && n.paint.is_enabled)
        {
            let hot = match self.nav_hit_at(id, x, y) {
                Some(nav::Hit::Item(i)) => i,
                Some(nav::Hit::Back) => nav::HOT_BACK,
                Some(nav::Hit::Toggle) => nav::HOT_TOGGLE,
                Some(nav::Hit::Settings) => nav::SETTINGS_INDEX,
                None => -1,
            };
            if self.node(id).is_some_and(|n| n.ctrl().hot_index != hot) {
                if let Some(n) = self.node_mut(id) {
                    n.ctrl_mut().hot_index = hot;
                    n.mark_dirty();
                }
                nav_hot_moved = true;
            }
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
            self.queue_surface_exit();
            self.hovered_surface = now_surface;
        }
        if let Some((sid, interest, ax, ay)) = surf
            && interest.moved
        {
            self.queue_surface(sid, record::SurfaceIntentKind::Move, ax, ay, false, 0);
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
            // Same node, new pane row: snap the ink onto it (the chrome wash
            // and the row labels repaint from the dirty flag the caller set).
            if nav_hot_moved
                && let Some(id) = now
            {
                if let Some(n) = self.node_mut(id) {
                    parts::nav_hot_changed(n);
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
            self.fire_pointer(new, x, y, record::PointerIntentKind::Moved);
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
                // Both track a hot child index the node-level hover does not
                // capture: leaving the node clears it, entering keeps whatever
                // the caller just recorded.
                // All three track a hot sub-element the node-level hover does
                // not capture: leaving the node clears it, entering keeps
                // whatever the caller just recorded.
                ControlKind::SelectorBar
                | ControlKind::NavigationView
                | ControlKind::InfoBar => {
                    if !hovered {
                        n.ctrl_mut().hot_index = -1;
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

    /// Queue the `exited` sink of the surface that held the hover, if it is still
    /// mounted. The exited-sink presence is checked at drain (the closure lives
    /// app-side); here the router only knows a surface it was tracking is being
    /// left.
    fn queue_surface_exit(&mut self) {
        if let Some(old) = self.hovered_surface.take()
            && self.node(old).is_some()
        {
            self.intents.push(record::Intent::SurfaceExit { id: old });
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
        self.queue_surface_exit();
        // Fade out the scrollbar thumb when the pointer leaves the window.
        self.update_hovered_scroll(None);
    }

    // ── Press / release ──────────────────────────────────────────────────────

    /// Left button down. Returns whether the pointer should be captured.
    pub(crate) fn on_pointer_down(&mut self, x: f32, y: f32) -> bool {
        set_last_pointer(x, y);
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
        if let Some((sid, interest, ax, ay)) = self.surface_at(x, y)
            && interest.down
        {
            self.pressed_surface = Some((sid, ay - y));
            self.queue_surface(sid, record::SurfaceIntentKind::Down, ax, ay, true, 0);
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
            // A press starts as a CLICK — a discrete change that may spring. It
            // only becomes a scrub once the pointer actually moves.
            self.scrubbing = false;
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
            // Knobs jump to the angle under the pointer (click-to-position),
            // then scrub on a relative VERTICAL drag from there — precise, and
            // the pointer never has to orbit the dial. The jump lands 1:1 (the
            // node is pressed, so the arc does not ease into it); the drag
            // origin is latched from the post-jump value so the scrub continues
            // from where the click landed. A press on the centre hub does not
            // jump, so grabbing the middle never throws the setting.
            if self.node(id).map(|n| n.kind) == Some(ControlKind::Knob) {
                self.fire_bool(id, Event::DragStateChanged, true);
                self.knob_press_to(id, x, y);
                if let Some(n) = self.node(id) {
                    self.knob_drag = Some((id, n.ctrl().value, y));
                }
            }
            self.fire_pointer(id, x, y, record::PointerIntentKind::Pressed);
            true
        } else {
            false
        }
    }

    /// Pointer capture was taken away from us — a system modal dialog, Alt+Tab,
    /// Win+D, a debugger break. No `WM_LBUTTONUP` will ever follow, so this is
    /// the only chance to tear the gesture down.
    ///
    /// A stolen capture is a **cancel, not a click**. The distinction this draws
    /// through [`on_pointer_up`](Self::on_pointer_up) is:
    ///
    /// * *Release state* — the pressed flag and its ink, `pressed_id`,
    ///   `knob_drag`, `pressed_surface`, `dragging_thumb`, and above all the
    ///   global `scrubbing` flag — is cleared, exactly as a real release clears
    ///   it. Leaving `scrubbing` stuck true is the worst of these: it is global,
    ///   so every slider, knob and meter in the window would snap instead of
    ///   spring, permanently, until the next clean press/release.
    /// * *End-of-gesture notifications* that pair with something already fired
    ///   at press time — `DragStateChanged(false)`, `on_pointer_released`, a viz
    ///   surface's `up` sink — DO still fire. These report that the gesture
    ///   stopped, not what it produced; withholding them would strand the app's
    ///   own drag state in exactly the way this method exists to prevent.
    /// * *Committing the action* — `activate_pointer`, i.e. the toggle, the
    ///   click handler, the selection change, the popup open — does **not** run.
    ///   Nothing is committed by a gesture the user never finished.
    ///
    /// Unlike `on_pointer_up` this clears every kind of press state rather than
    /// returning after the first match: the point is to leave nothing live.
    pub(crate) fn on_pointer_cancel(&mut self) {
        let (x, y) = last_pointer();

        // The gesture is over however it ended: value chrome may spring again.
        self.scrubbing = false;

        // Drop any pending right-press too: its release will never arrive
        // either, and a stale record could otherwise pair with a stray later
        // button-up and report a right-tap the user never made.
        RIGHT_PRESSED.with(|c| c.set(None));

        // A viz pointer-surface drag: the surface hears its release wherever the
        // pointer was (capture semantics). No value is committed — every value
        // was already streamed by the `moved` sink.
        if let Some((sid, dy)) = self.pressed_surface.take() {
            self.queue_surface(sid, record::SurfaceIntentKind::Up, x, y + dy, false, 0);
        }

        // A scrollbar-thumb drag: `scroll_off` is applied live as the thumb
        // moves, so dropping the drag IS the whole cancellation.
        if let Some(sid) = self.dragging_thumb.take() {
            if let Some(n) = self.node_mut(sid) {
                n.thumb_drag = None;
            }
            if self.hovered_scroll != Some(sid) {
                self.set_thumb_shown(sid, false);
            }
        }

        let was_knob_drag = self.knob_drag.take().is_some();

        let Some(id) = self.pressed_id.take() else { return };

        // A text field's press carries no ink and no activation (mirroring the
        // text-field arm of `on_pointer_up`) — dropping the press is enough.
        if self.node(id).is_some_and(|n| n.editor.is_some()) {
            return;
        }

        if let Some(n) = self.node_mut(id) {
            n.pressed = false;
            if parts::converted(n.kind) {
                parts::ink_state_changed(n);
            }
        }

        // Mirror of the press-time `DragStateChanged(true)` for the scrubbing
        // kinds: a state edge the app must see, not a value.
        if was_knob_drag || self.node(id).map(|n| n.kind) == Some(ControlKind::Slider) {
            self.fire_bool(id, Event::DragStateChanged, false);
        }

        // The press is over for anyone tracking it — but deliberately WITHOUT
        // the `activate_pointer` call `on_pointer_up` makes here.
        self.fire_pointer(id, x, y, record::PointerIntentKind::Released);
    }

    /// Left button up.
    pub(crate) fn on_pointer_up(&mut self, x: f32, y: f32) {
        set_last_pointer(x, y);
        // The gesture is over: value chrome may spring again.
        self.scrubbing = false;
        // End a viz pointer-surface drag: the surface always sees the release
        // (capture semantics), wherever the pointer is.
        if let Some((sid, dy)) = self.pressed_surface.take() {
            self.queue_surface(sid, record::SurfaceIntentKind::Up, x, y + dy, false, 0);
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
        self.fire_pointer(id, x, y, record::PointerIntentKind::Released);
        if self.is_over(id, x, y) {
            self.activate_pointer(id, x, y);
        }
    }

    /// Right button down. Returns whether the message was consumed.
    ///
    /// The right button never presses a control: there is no press ink, no
    /// scrub, no focus move, and no pointer capture (a right-drag is not a
    /// gesture this backend has). It only records where the press landed so
    /// [`on_right_pointer_up`](Self::on_right_pointer_up) can require down and
    /// up on the same element, the way `RightTapped` is defined.
    pub(crate) fn on_right_pointer_down(&mut self, x: f32, y: f32) -> bool {
        set_last_pointer(x, y);
        RIGHT_PRESSED.with(|c| c.set(None));

        // An open popup light-dismisses on a right-click anywhere, inside or
        // out — the same as clicking away from it — and swallows the press.
        if self.popup.is_some() {
            self.close_popup();
            return true;
        }

        let target = self.interactive_at(x, y);
        RIGHT_PRESSED.with(|c| c.set(target));
        target.is_some()
    }

    /// Right button up: report a right-tap when the release lands on the same
    /// node the press did. This is what a context menu would hang off.
    pub(crate) fn on_right_pointer_up(&mut self, x: f32, y: f32) {
        set_last_pointer(x, y);
        let Some(id) = RIGHT_PRESSED.with(|c| c.take()) else {
            return;
        };
        if !self.is_over(id, x, y) {
            return;
        }
        self.fire_right_tapped(id);
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
            Some(ControlKind::NavigationView) => {
                if let Some(hit) = self.nav_hit_at(id, x, y) {
                    self.nav_act(id, hit);
                }
            }
            // Only the close button acts. The rest of the band is inert
            // chrome, so a click on the message must NOT dismiss the bar —
            // which is why this resolves the position instead of falling
            // through to the position-free `activate`.
            Some(ControlKind::InfoBar) => {
                if self
                    .node(id)
                    .is_some_and(|n| info_bar::hit_close(n, x - n.rect.x, y - n.rect.y))
                {
                    self.close_info_bar(id);
                }
            }
            _ => self.activate(id),
        }
    }

    /// Dismiss an InfoBar: collapse the band and tell the app.
    ///
    /// The bar's `IsOpen` is an app-controlled prop, so this is the same
    /// two-step a ToggleSwitch flip makes — drive the local state so the
    /// chrome responds to the click immediately, then fire the event the app
    /// converges through. A host that keeps `is_open` true simply re-opens it
    /// on the next reconcile, which is the controlled-prop contract working,
    /// not a glitch.
    ///
    /// The close button of a non-closable bar is not drawn and not hit-tested,
    /// so reaching here with `bar_closable` false is impossible from the
    /// pointer; the guard covers the accessibility path, which addresses the
    /// element by name rather than by position.
    pub(crate) fn close_info_bar(&mut self, id: ControlId) {
        if !self
            .node(id)
            .is_some_and(|n| n.kind == ControlKind::InfoBar && n.extras().bar_closable)
        {
            return;
        }
        if let Some(n) = self.node_mut(id) {
            n.extras_mut().bar_open = false;
            // The pointer is left hovering a button that no longer exists.
            n.ctrl_mut().hot_index = -1;
            n.mark_dirty();
        }
        self.fire_unit(id, Event::Closed);
        // A closed bar leaves layout entirely (`layout::finalize_style`), so
        // the siblings below it have to be re-placed — the same reason a
        // collapsing Expander relays out rather than merely repainting.
        self.relayout_and_paint();
    }

    /// Activate a control with no position dependency (keyboard or button).
    fn activate(&mut self, id: ControlId) {
        let Some(kind) = self.node(id).map(|n| n.kind) else { return };
        match kind {
            ControlKind::ToggleSwitch => {
                let on = !self.node(id).map(|n| n.ctrl().is_on).unwrap_or(false);
                if let Some(n) = self.node_mut(id) {
                    n.ctrl_mut().is_on = on;
                    n.mark_dirty();
                }
                // The knob/track glide runs on the compositor: the repaint's
                // parts sync sees the flipped state and retargets the springs.
                self.repaint();
                self.fire_bool(id, Event::Toggled, on);
            }
            ControlKind::CheckBox | ControlKind::ToggleButton => {
                let on = !self.node(id).map(|n| n.ctrl().is_checked).unwrap_or(false);
                if let Some(n) = self.node_mut(id) {
                    n.ctrl_mut().is_checked = on;
                    n.mark_dirty();
                }
                // The CheckBox reveal fades on the compositor (the repaint's
                // parts sync); a ToggleButton's checked chrome just repaints.
                self.repaint();
                self.fire_bool(id, Event::Checked, on);
            }
            ControlKind::Expander => {
                let ex = !self.node(id).map(|n| n.ctrl().expanded).unwrap_or(false);
                if let Some(n) = self.node_mut(id) {
                    n.ctrl_mut().expanded = ex;
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
                // A Button carrying a MenuFlyout (e.g. "+ Add Processor") or an
                // attached Flyout opens it in the popup overlay, mirroring the
                // native WinUI button-flyout.
                if self.node(id).is_some_and(|n| {
                    !n.ctrl().menu.is_empty() || n.extras().flyout.is_some()
                }) {
                    self.open_popup(id);
                    return;
                }
                // `Click` queues before `on_tapped`, and the intent queue is
                // FIFO — a node carrying both observes them in that order,
                // exactly as the old synchronous dispatch delivered them.
                self.fire_unit(id, Event::Click);
                self.fire_tapped(id);
                // A HyperlinkButton also follows its `NavigateUri` — through the
                // app's installed launcher, or not at all. This sits in
                // `activate`, the ONE path a pointer release, a Space/Enter
                // press and a UIA `Invoke` all converge on, so a screen reader
                // invoking a link launches exactly as a click does; there is no
                // second route to keep in step.
                //
                // Deferred, not called here: we are inside the backend's own
                // `RefCell` borrow, and the launcher is app code that may pump
                // messages (`ShellExecuteW` does) — a synchronous call could
                // re-enter the window procedure and find the backend already
                // borrowed. `post_ui` runs it from the pump with the borrow
                // released, which is also where the contract on
                // `set_uri_launcher` promises it runs.
                if kind == ControlKind::HyperlinkButton
                    && crate::uri_launcher_installed()
                    && let Some(uri) = self
                        .node(id)
                        .map(|n| n.extras().navigate_uri.clone())
                        .filter(|u| !u.is_empty())
                {
                    host::post_ui(self.hwnd, move || {
                        crate::launch_uri(&uri);
                    });
                }
            }
            // Any other node that hit-tested as clickable (a Border/panel made
            // interactive via a Click handler or `on_tapped`, e.g. the nav-rail
            // items) activates the same way a Button does — the press wouldn't
            // have reached here otherwise.
            _ => {
                // Same order contract as the Button arm: Click, then tapped.
                self.fire_unit(id, Event::Click);
                self.fire_tapped(id);
            }
        }
    }

    /// The segment index under window-relative `x`, or `None` for an empty bar.
    fn segment_at(&self, id: ControlId, x: f32) -> Option<i32> {
        let node = self.node(id)?;
        let n = node.ctrl().items.len();
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
            if n.ctrl().selected_index == i || !n.paint.is_enabled {
                return;
            }
            n.ctrl_mut().selected_index = i;
            n.mark_dirty();
            n.ctrl().items.get(i as usize).cloned().unwrap_or_default()
        };
        self.repaint();
        self.fire_string(id, Event::SelectionChanged, label);
    }

    /// The pane geometry for a NavigationView node: the resolved metrics, the
    /// node height they were resolved against, and the item count. `None` for
    /// any other kind.
    ///
    /// Every pane question in this module goes through here so the hit test can
    /// never resolve against different geometry than the paint did — they call
    /// the same `nav::metrics` with the same inputs.
    pub(crate) fn nav_metrics(&self, id: ControlId) -> Option<(nav::Metrics, f32, usize)> {
        let n = self.node(id)?;
        if n.kind != ControlKind::NavigationView {
            return None;
        }
        let has_title = n.nav_text.as_ref().is_some_and(|t| t.title.is_some());
        Some((
            nav::metrics(n.extras(), n.rect.w, has_title),
            n.rect.h,
            n.ctrl().items.len(),
        ))
    }

    /// What a window-relative point lands on inside a nav pane.
    pub(crate) fn nav_hit_at(&self, id: ControlId, x: f32, y: f32) -> Option<nav::Hit> {
        let (m, h, count) = self.nav_metrics(id)?;
        let n = self.node(id)?;
        nav::hit(&m, h, count, x - n.rect.x, y - n.rect.y)
    }

    /// A press landed in the pane: route it to whatever it hit. The pointer and
    /// the accessibility tree share this one entry point, so an invoke from a
    /// screen reader takes exactly the path a click takes.
    pub(crate) fn nav_act(&mut self, id: ControlId, hit: nav::Hit) {
        match hit {
            nav::Hit::Item(i) => self.set_nav_index(id, i),
            nav::Hit::Back => {
                // A disabled back arrow is drawn but inert, mirroring the drawn
                // caption back button — and without a declared handler it is
                // inert too, not merely silent (`Interactivity::back`).
                if self
                    .node(id)
                    .is_some_and(|n| n.extras().back_enabled && n.interactivity.back)
                {
                    self.fire_unit(id, Event::BackRequested);
                }
            }
            nav::Hit::Toggle => self.toggle_nav_pane(id),
            nav::Hit::Settings => self.select_nav_settings(id),
        }
    }

    /// Flip the pane open/closed. This is the hamburger's whole behaviour: the
    /// seam carries no event for it (the NavigationView widget exposes only
    /// `on_selection_changed` and `on_back_requested`), and WinUI's own toggle
    /// likewise just drives `IsPaneOpen`. The pane's new width is derived
    /// geometry, so the flip has to re-derive and re-lay-out — the content pane
    /// beside it must resize with it.
    fn toggle_nav_pane(&mut self, id: ControlId) {
        let Some(n) = self.node_mut(id) else { return };
        let open = !n.extras().pane_open;
        n.extras_mut().pane_open = open;
        layout::apply_nav_metrics(n);
        self.relayout_and_paint();
    }

    /// The settings row: reported as a selection carrying the settings tag.
    /// The row is a selectable page like any menu item — the selection moves to
    /// its sentinel slot ([`nav::SETTINGS_INDEX`]), the tile/bar sprites glide
    /// to the foot of the pane, and `ISelectionProvider` reports the settings
    /// element as the selection.
    fn select_nav_settings(&mut self, id: ControlId) {
        let already = {
            let Some(n) = self.node_mut(id) else { return };
            if n.ctrl().selected_index == nav::SETTINGS_INDEX {
                true
            } else {
                n.ctrl_mut().selected_index = nav::SETTINGS_INDEX;
                n.mark_dirty();
                false
            }
        };
        if already {
            return;
        }
        self.repaint();
        self.fire_string(id, Event::SelectionChanged, nav::SETTINGS_TAG.to_string());
    }

    /// Select NavigationView item `i` (the by-index core `select_nav` and UIA
    /// `SelectionItem::Select` both route through).
    fn set_nav_index(&mut self, id: ControlId, i: i32) {
        let tag = {
            let Some(nd) = self.node_mut(id) else { return };
            if nd.ctrl().selected_index == i {
                return;
            }
            nd.ctrl_mut().selected_index = i;
            nd.mark_dirty();
            nd.ctrl().tags.get(i as usize).cloned().unwrap_or_default()
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
            if n.ctrl().selected_index == i {
                return;
            }
            n.ctrl_mut().selected_index = i;
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
            let mut v = n.ctrl().min + frac as f64 * (n.ctrl().max - n.ctrl().min);
            if let Some(step) = n.ctrl().step
                && step > 0.0
            {
                v = (v / step).round() * step;
                v = v.clamp(n.ctrl().min, n.ctrl().max);
                let span = n.ctrl().max - n.ctrl().min;
                frac = if span.abs() < f64::EPSILON { 0.0 } else { ((v - n.ctrl().min) / span) as f32 };
            }
            // Crossing the fill origin flips the two-tone fill color — a
            // discrete edge, handled by one dirty repaint whose parts sync
            // rebinds the fill's atlas source. Scrub motion stays pure
            // property snaps.
            let old = n.ctrl().value;
            let recolor = n
                .ctrl()
                .fill_origin
                .is_some_and(|o| (old <= o) != (v <= o));
            n.ctrl_mut().value = v;
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
        self.fire_value_changed(id, value);
    }

    /// Click-to-position: jump the knob to the angle under `(x, y)` and report
    /// it. A press on the centre readout hub changes nothing (the relative drag
    /// still starts from there). The node is `pressed` here, so the paint pass
    /// moves the arc 1:1 — a click puts it exactly where you clicked.
    fn knob_press_to(&mut self, id: ControlId, x: f32, y: f32) {
        let value = {
            let Some(n) = self.node_mut(id) else { return };
            let Some(raw) = knob::value_at_point(n, x, y) else { return };
            let mut v = raw.clamp(n.ctrl().min, n.ctrl().max);
            if let Some(step) = n.ctrl().step
                && step > 0.0
            {
                v = ((v / step).round() * step).clamp(n.ctrl().min, n.ctrl().max);
            }
            if (v - n.ctrl().value).abs() < f64::EPSILON {
                return;
            }
            n.ctrl_mut().value = v;
            n.mark_dirty();
            v
        };
        self.repaint();
        self.fire_value_changed(id, value);
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
            let span = n.ctrl().max - n.ctrl().min;
            if span == 0.0 {
                return;
            }
            let dy = (y0 - y) as f64; // up (decreasing y) increases
            let mut v = (origin + (dy / KNOB_DRAG_RANGE as f64) * span).clamp(n.ctrl().min, n.ctrl().max);
            if let Some(step) = n.ctrl().step
                && step > 0.0
            {
                v = (v / step).round() * step;
                v = v.clamp(n.ctrl().min, n.ctrl().max);
            }
            n.ctrl_mut().value = v;
            n.mark_dirty();
            v
        };
        self.repaint();
        self.fire_value_changed(id, value);
    }

    /// Advance a knob value by `detents` mouse-wheel detents (5% of the domain
    /// each), clamped/quantized, and report it.
    fn knob_wheel(&mut self, id: ControlId, detents: f64) {
        /// Fraction of the domain one wheel detent advances.
        const KNOB_WHEEL_FRAC: f64 = 0.05;
        let value = {
            let Some(n) = self.node_mut(id) else { return };
            let span = n.ctrl().max - n.ctrl().min;
            if span == 0.0 {
                return;
            }
            let mut v = (n.ctrl().value + detents * KNOB_WHEEL_FRAC * span).clamp(n.ctrl().min, n.ctrl().max);
            if let Some(step) = n.ctrl().step
                && step > 0.0
            {
                v = (v / step).round() * step;
                v = v.clamp(n.ctrl().min, n.ctrl().max);
            }
            n.ctrl_mut().value = v;
            n.mark_dirty();
            v
        };
        self.repaint();
        self.fire_value_changed(id, value);
    }

    // ── Popup ────────────────────────────────────────────────────────────────

    pub(crate) fn open_popup(&mut self, owner: ControlId) {
        let Some(node) = self.node(owner) else { return };
        let combo = node.kind == ControlKind::ComboBox;
        let rect = CanvasRect::from_xywh(node.rect.x, node.rect.y, node.rect.w, node.rect.h);
        let rows: Vec<MenuRow> = if combo {
            node.ctrl()
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
            node.ctrl().menu.clone()
        };
        // Menu rows win over an attached flyout: a control carrying both is
        // asking for a menu, and the flyout is the fallback content.
        let body = if rows.is_empty() {
            match node.extras().flyout.as_deref() {
                Some(def) if !def.text.is_empty() => PopupBody::Text(def.text.clone()),
                _ => return,
            }
        } else {
            PopupBody::Menu(rows)
        };
        let placement = node.extras().flyout_placement;
        let selected = node.ctrl().selected_index;
        if let Ok(p) = Popup::open(
            &self.comp,
            owner,
            body,
            rect,
            self.dip_size,
            combo,
            selected,
            false,
            placement,
        ) {
            self.close_popup();
            self.popup = Some(p);
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
                    n.ctrl()
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
        // A suggestion list always drops below its field, whatever placement
        // the node happens to carry — it is a continuation of the text being
        // typed, not an attached flyout.
        if let Ok(p) = Popup::open(
            &self.comp,
            owner,
            PopupBody::Menu(rows),
            rect,
            self.dip_size,
            false,
            -1,
            true,
            crate::FlyoutPlacementMode::Bottom.0,
        ) {
            self.close_popup();
            self.popup = Some(p);
        }
    }

    /// Commit a chosen suggestion to its AutoSuggestBox: set the field text to
    /// the suggestion, fire `TextChanged` then `SuggestionChosen` (the buffer
    /// changed, and both are how the app's controlled value and the §7.2
    /// delivered revision stay current — mirroring WinUI, where choosing a
    /// suggestion raises TextChanged with reason SuggestionChosen), and
    /// dismiss the dropdown (focus stays).
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
                e.text_rev += 1;
            }
            n.mark_dirty();
        }
        self.repaint();
        self.fire_editor_text(owner, Event::TextChanged, text.clone());
        self.fire_editor_text(owner, Event::SuggestionChosen, text);
    }

    /// Close the open popup with a compositor-side dismiss fade. The overlay
    /// visual is parked as a [`Ghost`] and released when the scoped batch
    /// wrapping its fade completes — no app frames, no timer.
    pub(crate) fn close_popup(&mut self) {
        if let Some(p) = self.popup.take() {
            // A popup that came from an attached flyout reports its dismissal,
            // whatever dismissed it — light-dismiss, Escape, or a selection.
            // This is the ONE close path, so `on_closed` cannot be missed by a
            // route that forgot to fire it.
            if self
                .node(p.owner)
                .and_then(|n| n.extras().flyout.as_deref())
                .is_some_and(|f| f.notifies_closed)
            {
                self.intents
                    .push(record::Intent::FlyoutClosed { id: p.owner });
            }
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
                n.ctrl_mut().selected_index = idx as i32;
                n.mark_dirty();
            }
            self.fire_i32(owner, Event::SelectionChanged, idx as i32);
        } else if let Some(t) = tag {
            self.fire_string(owner, Event::ItemClicked, t);
        }
    }

    // ── Wheel ────────────────────────────────────────────────────────────────

    /// Vertical mouse wheel at (x, y) DIPs, `delta` in WHEEL_DELTA (120) units,
    /// positive away from the user. The scroll glide plays on the compositor —
    /// nothing here needs the timer.
    pub(crate) fn on_wheel(&mut self, x: f32, y: f32, delta: i32) {
        // A viz pointer surface that subscribed the wheel (EQ Q-adjust) consumes
        // it; surfaces without a wheel sink fall through to scrolling.
        if let Some((sid, interest, ax, ay)) = self.surface_at(x, y)
            && interest.wheel
        {
            self.queue_surface(sid, record::SurfaceIntentKind::Wheel, ax, ay, false, delta);
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
            let max = self.node(id).map(|n| (n.ctrl().content_h - n.rect.h).max(0.0)).unwrap_or(0.0);
            if let Some(n) = self.node_mut(id) {
                let target = layout::snap((n.scroll_off + step).clamp(0.0, max), scale);
                n.scroll_off = target;
                n.scroll_glide(target);
                let g = scroll::thumb_geom(n.rect.h, n.ctrl().content_h, target);
                let tx = n.rect.w - scroll::THUMB_W - scroll::THUMB_MARGIN;
                n.thumb_glide(tx, g.thumb_y);
            }
            // Reveal the thumb while scrolling (it conceals when the pointer
            // leaves the container).
            self.update_hovered_scroll(Some(id));
        }
    }

    /// Horizontal wheel — a tilt-wheel click or a touchpad sideways pan — at
    /// (x, y) DIPs, `delta` in WHEEL_DELTA (120) units.
    ///
    /// **Sign:** `WM_MOUSEHWHEEL` does not share `WM_MOUSEWHEEL`'s convention.
    /// A positive vertical delta means *away from the user*; a positive
    /// horizontal delta means *to the right*. `delta` is forwarded raw, with
    /// [`WheelAxis::Horizontal`] attached, so a sink reads it with the
    /// right-is-positive convention its users expect rather than an invented
    /// mapping onto "forward".
    ///
    /// Only one link of the vertical chain has a horizontal analogue:
    ///
    /// * **Viz surface wheel sink** — receives it, tagged `Horizontal`. The
    ///   surface is the one consumer that can meaningfully distinguish the
    ///   axes, so it decides (an EQ that adjusts Q on the wheel reads
    ///   [`PointerEventInfo::wheel_delta_on`] and stays inert under a tilt).
    /// * **Focused NumberBox** — deliberately nothing. Stepping a numeric value
    ///   is a vertical-wheel gesture; a sideways pan across a form would drift
    ///   every field it crossed.
    /// * **Knob** — deliberately nothing, for the same reason. A knob's domain
    ///   has one axis and the vertical wheel already owns it; an audio control
    ///   must not move because the user panned sideways.
    /// * **Nearest scroll ancestor** — deliberately nothing. `Node::scroll_off`
    ///   is a single *vertical* scalar and the carrier/thumb machinery
    ///   (`scroll::thumb_geom`, `scroll_for_thumb_y`, the content-carrier
    ///   spring) is vertical-only, so there is nothing to move on this axis.
    ///   Falling through to the vertical path would scroll the wrong way, which
    ///   is worse than not scrolling: a container that only scrolls vertically
    ///   must ignore a horizontal tilt. Real horizontal content scrolling needs
    ///   a second scroll scalar plus horizontal thumb geometry and carrier
    ///   springs — not plumbing, and out of scope here.
    ///
    /// So this returns without touching layout unless a surface takes it, and
    /// never falls through to [`on_wheel`](Self::on_wheel).
    pub(crate) fn on_wheel_h(&mut self, x: f32, y: f32, delta: i32) {
        if let Some((sid, interest, ax, ay)) = self.surface_at(x, y)
            && interest.wheel
        {
            self.queue_surface_wheel(
                sid,
                record::SurfaceIntentKind::Wheel,
                ax,
                ay,
                false,
                delta,
                WheelAxis::Horizontal,
            );
        }
    }

    // ── Keyboard ─────────────────────────────────────────────────────────────

    /// A key was released. Ends the held/auto-repeat state started by the
    /// matching [`on_key`](Self::on_key); nothing else keys off a key-up.
    /// Returns `false` — no key-up is ever consumed — so a `WM_SYSKEYUP` falls
    /// through to `DefWindowProc` (Alt-tap-to-menu, F10 release; §7.3).
    pub(crate) fn on_key_up(&mut self, vk: u32) -> bool {
        key_release(vk);
        false
    }

    /// A key was pressed, with the full modifier set held. Returns whether the
    /// backend **consumed** the key — the signal the WndProc needs for the
    /// sys-key `return-0`-vs-`DefWindowProc` decision (§7.3): an unconsumed
    /// sys-key falls through to the system so Alt+F4 / F10 / Alt+Space work.
    pub(crate) fn on_key(&mut self, vk: u32, mods: crate::VirtualKeyModifiers) -> bool {
        let ctrl = mods.contains(crate::VirtualKeyModifiers::Control);
        let shift = mods.contains(crate::VirtualKeyModifiers::Shift);
        let alt = mods.contains(crate::VirtualKeyModifiers::Menu);

        // The keyboard repeats a held key at the system rate. Record the press
        // and find out whether this is a fresh one or the repeat.
        let repeat = key_press(vk);

        // A held Space/Enter must not re-activate on every repeat: that fires a
        // Button's click handler dozens of times a second, and flickers a
        // ToggleSwitch / CheckBox on and off. A RepeatButton is the one kind
        // whose whole purpose is to repeat, so it opts back in. Text editing,
        // arrow-key nudges and Tab all repeat normally and fall through.
        if repeat
            && matches!(vk, VK_SPACE | VK_RETURN)
            && self.focused_editable().is_none()
            && self.popup.is_none()
            && self
                .focused_id
                .and_then(|id| self.node(id).map(|n| n.kind))
                != Some(ControlKind::RepeatButton)
        {
            // Swallowed (a held activation key), so the sys-key path treats it
            // as consumed.
            return true;
        }

        // §7.3 fixed conflict policy: a modifier-chorded binding or F-key wins
        // over a focused editor, *except* the editor's own Ctrl+A/C/X/V and its
        // unmodified printable/editing keys. Match accelerators before the
        // editor unless the editor claims this key; a match consumes it and
        // never also reaches the editor / traversal below.
        let editor_claims =
            self.focused_editable().is_some() && editor_claims_key(vk, ctrl, alt);
        if !editor_claims
            && let Some((id, index)) = self.match_accelerator(vk, mods)
        {
            self.fire_accelerator(id, index);
            return true;
        }

        // A focused text editor consumes editing keys before the generic ring.
        if let Some(id) = self.focused_editable()
            && self.editor_key(id, vk, shift, ctrl).is_some()
        {
            return true;
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
            return true;
        }

        // An Alt-chord that reached here matched no accelerator and is not
        // editor input, so it is a system chord (Alt+Space, Alt+F4, F10-style
        // menu keys). The focus ring must NOT consume it — leave it unconsumed
        // so the sys-key path falls it through to `DefWindowProc` (§7.3).
        if alt {
            return false;
        }

        match vk {
            VK_TAB => {
                self.move_focus(if shift { -1 } else { 1 });
                true
            }
            VK_SPACE | VK_RETURN => {
                if let Some(id) = self.focused_id {
                    self.activate(id);
                }
                true
            }
            VK_LEFT | VK_UP => {
                self.focus_arrow(-1);
                true
            }
            VK_RIGHT | VK_DOWN => {
                self.focus_arrow(1);
                true
            }
            // Not a key the backend routes: leave it unconsumed so a sys-key
            // reaches DefWindowProc (Alt+F4, F10, Alt+Space).
            _ => false,
        }
    }

    /// Match a keydown against the front-resident accelerator table (§7.3):
    /// the first node whose declared list holds this exact `(key, mods)` chord,
    /// with its index into that list so the app half can address the right
    /// callback. Tab always traverses, so it is never an accelerator. The
    /// modifier set must match exactly, mirroring WinUI's accelerator dispatch.
    fn match_accelerator(
        &self,
        vk: u32,
        mods: crate::VirtualKeyModifiers,
    ) -> Option<(ControlId, usize)> {
        if vk == VK_TAB {
            return None;
        }
        let key = crate::VirtualKey(vk as i32);
        self.keybindings.iter().find_map(|(id, list)| {
            list.iter()
                .position(|(k, m)| *k == key && *m == mods)
                .map(|i| (*id, i))
        })
    }

    /// Queue a matched accelerator to fire app-side. Accelerators are app
    /// commands, not control-state changes, so there is no UIA notification —
    /// only the [`record::Intent::Accelerator`] the recorder resolves against
    /// its `accels` map by index.
    fn fire_accelerator(&mut self, id: ControlId, index: usize) {
        self.intents
            .push(record::Intent::Accelerator { id, index });
    }

    // ── Text editor ────────────────────────────────────────────────────────

    /// The focused node, if it is an editable text field. Also what "the
    /// document" means to the TSF text store (`tsf::doc`).
    pub(crate) fn focused_editable(&self) -> Option<ControlId> {
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
        if !focused {
            // Keys released while another window has focus never send us a
            // `WM_KEYUP`, so the held set would keep them down forever and the
            // next genuine press would be mistaken for an auto-repeat.
            keys_clear();
        }
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

    pub(crate) fn with_editor<R>(
        &mut self,
        id: ControlId,
        f: impl FnOnce(&mut editor::Editor) -> R,
    ) -> Option<R> {
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
                        self.fire_editor_text(id, Event::QuerySubmitted, t);
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
                    self.fire_editor_text(id, Event::QuerySubmitted, t);
                }
            }
            VK_UP if kind == ControlKind::NumberBox => self.number_step(id, 1.0, false),
            VK_DOWN if kind == ControlKind::NumberBox => self.number_step(id, -1.0, false),
            VK_PRIOR if kind == ControlKind::NumberBox => self.number_step(id, 1.0, true),
            VK_NEXT if kind == ControlKind::NumberBox => self.number_step(id, -1.0, true),
            // Escape reverts the in-progress edit to the last committed value
            // (§7.3), matching WinUI — focus is retained and no `ValueChanged`
            // fires (the committed value never changed, only the discarded
            // text did). Other editors keep Escape as a consumed no-op.
            VK_ESCAPE if kind == ControlKind::NumberBox => self.revert_number(id),
            _ => {} // consume; printable input arrives via WM_CHAR
        }
        Some(())
    }

    /// Escape in a `NumberBox`: discard the in-progress text edit and restore
    /// the last committed value into the buffer, keeping focus and firing no
    /// `ValueChanged` (§7.3). The revert reformats `ctrl().value` — the pre-edit
    /// value, since a NumberBox commits only on Enter/blur.
    fn revert_number(&mut self, id: ControlId) {
        if let Some(n) = self.node_mut(id) {
            revert_number_text(n);
        }
        self.repaint();
    }

    /// Whether a text field currently has keyboard focus — the pump's gate on
    /// offering keys to a TIP (see `tsf::bridge::filter_key`).
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
    pub(crate) fn editor_caret_moved(&mut self, id: ControlId) {
        if let Some(n) = self.node_mut(id) {
            if let Some(e) = &mut n.editor {
                e.caret_moved = true;
            }
            n.mark_dirty();
        }
        self.repaint();
    }

    /// Text changed: reset blink, repaint, and fire the per-kind change event
    /// (NumberBox fires only on commit). Every user-originated buffer edit
    /// funnels through here (keystroke, backspace, paste, IME commit, UIA
    /// SetValue), so this is where the editor's §7.2 text revision bumps —
    /// the fired intent carries the new revision out to the app.
    pub(crate) fn editor_after_edit(&mut self, id: ControlId) {
        let (kind, text) = match self.node_mut(id) {
            Some(n) => {
                let kind = n.kind;
                if let Some(e) = &mut n.editor {
                    e.caret_moved = true;
                    e.seeded = true;
                    e.text_rev += 1;
                }
                let text = n.editor.as_ref().map(|e| e.text()).unwrap_or_default();
                n.mark_dirty();
                (kind, text)
            }
            None => return,
        };
        self.repaint();
        match kind {
            ControlKind::TextBox => self.fire_editor_text(id, Event::TextChanged, text),
            ControlKind::AutoSuggestBox => {
                self.fire_editor_text(id, Event::TextChanged, text);
                // Reflect the edit in the suggestion dropdown from whatever rows the
                // node currently carries; the app's filtered list (set on the next
                // render via `Prop::Items`) refreshes it again in place.
                self.refresh_suggest(id);
            }
            ControlKind::PasswordBox => self.fire_editor_text(id, Event::PasswordChanged, text),
            _ => {}
        }
    }

    /// Commit a NumberBox: parse (with inline arithmetic) → clamp → round →
    /// format → write back → fire `ValueChanged`.
    fn commit_number(&mut self, id: ControlId) {
        let (text, fallback) = match self.node(id) {
            Some(n) => (
                n.editor.as_ref().map(|e| e.text()).unwrap_or_default(),
                n.ctrl().value,
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
                let cur = editor::eval_numeric(&text).unwrap_or(n.ctrl().value);
                let step = n.ctrl().step.unwrap_or(1.0);
                let inc = if large {
                    n.ctrl().large_change.unwrap_or(step * 10.0)
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
            Some(n) => (n.ctrl().min, n.ctrl().max, n.ctrl().precision),
            None => return,
        };
        let (v, s) = editor::commit_format(value, min, max, precision);
        if let Some(n) = self.node_mut(id) {
            n.ctrl_mut().value = v;
            if let Some(e) = &mut n.editor {
                e.set_text(&s);
                e.seeded = true;
                e.caret_moved = true;
            }
            n.mark_dirty();
        }
        self.repaint();
        self.fire_value_changed(id, v);
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
                    let step = n.ctrl().step.unwrap_or((n.ctrl().max - n.ctrl().min) / 20.0);
                    let v = (n.ctrl().value + dir as f64 * step).clamp(n.ctrl().min, n.ctrl().max);
                    n.ctrl_mut().value = v;
                    n.mark_dirty();
                    v
                };
                // The fill/thumb (slider) or arc/needle (knob) glide runs on the
                // compositor via the repaint's sync — a keyboard nudge needs no tick.
                self.repaint();
                self.fire_value_changed(id, value);
            }
            Some(ControlKind::SelectorBar) => {
                let (cur, n) = self
                    .node(id)
                    .map(|nd| (nd.ctrl().selected_index, nd.ctrl().items.len() as i32))
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
            let v = v.clamp(n.ctrl().min, n.ctrl().max);
            n.ctrl_mut().value = v;
            n.mark_dirty();
            v
        };
        // Compositor glide via the repaint's parts sync (see `focus_arrow`).
        self.repaint();
        self.fire_value_changed(id, value);
    }

    /// `SelectionItem::Select` (and `Invoke`) on synthetic item `i` of a
    /// SelectorBar / ComboBox / NavigationView, routed through the existing
    /// per-kind selection path.
    ///
    /// A nav pane's chrome arrives here too — `Invoke` on any synthetic item
    /// lands on this one entry point — and is handed to [`Self::nav_act`], the
    /// same function a pointer press calls. That is deliberate: an accessibility
    /// client invoking the hamburger must toggle the pane by exactly the path a
    /// click toggles it, not by a parallel implementation that can drift.
    pub(crate) fn uia_select_item(&mut self, id: ControlId, i: i32) {
        match self.node(id).map(|n| n.kind) {
            // The bar's one synthetic child is its close button, and invoking
            // it takes exactly the path a click on it takes.
            Some(ControlKind::InfoBar) if i == uia::INFOBAR_CLOSE_ITEM => {
                self.close_info_bar(id)
            }
            Some(ControlKind::SelectorBar) => self.set_segment(id, i),
            Some(ControlKind::NavigationView) => match uia::nav_chrome_of(i) {
                Some(hit) => self.nav_act(id, hit),
                None => self.set_nav_index(id, i),
            },
            Some(ControlKind::ComboBox) => self.set_combo_index(id, i),
            _ => {}
        }
    }

    /// `ExpandCollapse::Expand` / `Collapse`. Expander toggles through `activate`;
    /// the dropdown kinds open/close their popup.
    pub(crate) fn uia_set_expanded(&mut self, id: ControlId, want: bool) {
        match self.node(id).map(|n| n.kind) {
            Some(ControlKind::Expander) => {
                let cur = self.node(id).map(|n| n.ctrl().expanded).unwrap_or(false);
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

    // The `fire_*` dispatchers are the app-notification choke points: every
    // state change (pointer, keyboard, or UIA-initiated) flows through exactly
    // one of them. Each notifies UI Automation synchronously — screen readers
    // hear the change even when the app attached no handler — then queues a
    // typed, plain-data intent instead of invoking a closure: the backend no
    // longer holds any app handler. The recorder resolves the queue against
    // its app-side handler map after this input dispatch returns
    // (`record::RecordingBackend::drain_intents`), preserving fire order —
    // notably `Click` before `on_tapped` within one activation.
    pub(super) fn fire_unit(&mut self, id: ControlId, event: Event) {
        self.intents.push(record::Intent::Event {
            id,
            event,
            payload: record::IntentPayload::Unit,
        });
    }
    fn fire_bool(&mut self, id: ControlId, event: Event, v: bool) {
        self.uia_notify_bool(id, event, v);
        self.intents.push(record::Intent::Event {
            id,
            event,
            payload: record::IntentPayload::Bool(v),
        });
    }
    fn fire_string(&mut self, id: ControlId, event: Event, v: String) {
        self.uia_notify_string(id, event, &v);
        self.intents.push(record::Intent::Event {
            id,
            event,
            payload: record::IntentPayload::Str(v),
        });
    }
    fn fire_i32(&mut self, id: ControlId, event: Event, v: i32) {
        self.uia_notify_i32(id, event, v);
        self.intents.push(record::Intent::Event {
            id,
            event,
            payload: record::IntentPayload::I32(v),
        });
    }

    /// An editor-owned string event (`TextChanged` / `PasswordChanged` /
    /// `QuerySubmitted` / `SuggestionChosen`) — carries the editor's buffer
    /// revision so the app's programmatic write can come back stamped against
    /// it ([`Cmd::SetText`]) and a stale one be dropped — the §7.2 revision
    /// protocol, text half. The *bump* happens at the edit site
    /// ([`editor_after_edit`](Self::editor_after_edit) /
    /// [`choose_suggestion`](Self::choose_suggestion)); a commit-boundary
    /// event that leaves the buffer untouched (`QuerySubmitted`) fires with
    /// the current revision, which is what lets the app's response to it — a
    /// clear-search, a canonicalization — apply without a force lane.
    fn fire_editor_text(&mut self, id: ControlId, event: Event, v: String) {
        self.uia_notify_string(id, event, &v);
        let rev = self
            .node(id)
            .and_then(|n| n.editor.as_ref())
            .map(|e| e.text_rev)
            .unwrap_or(0);
        self.intents.push(record::Intent::EditorText {
            id,
            event,
            text: v,
            rev,
        });
    }

    /// `Event::ValueChanged` — the one event that carries a revision. Every
    /// input-originated value write bumps the node's `value_rev` here, and the
    /// intent carries it out so the app's echo can come back stamped against
    /// it (`Cmd::SetValue`) — the §7.2 revision protocol for control values.
    fn fire_value_changed(&mut self, id: ControlId, v: f64) {
        self.uia_notify_f64(id, Event::ValueChanged, v);
        let Some(node) = self.node_mut(id) else { return };
        node.value_rev += 1;
        let rev = node.value_rev;
        self.intents.push(record::Intent::ValueChanged { id, value: v, rev });
    }

    /// Queue a positional pointer callback, when the app declared one
    /// (`Node::pointer` carries presence only — the closure lives app-side).
    fn fire_pointer(&mut self, id: ControlId, x: f32, y: f32, kind: record::PointerIntentKind) {
        let Some(node) = self.node(id) else { return };
        let declared = match kind {
            record::PointerIntentKind::Pressed => node.pointer.pressed,
            record::PointerIntentKind::Released => node.pointer.released,
            record::PointerIntentKind::Moved => node.pointer.moved,
        };
        if !declared {
            return;
        }
        let info = PointerEventInfo {
            x: (x - node.rect.x) as f64,
            y: (y - node.rect.y) as f64,
            is_left_button_pressed: node.pressed,
            ..PointerEventInfo::default()
        };
        self.intents.push(record::Intent::Pointer { id, kind, info });
    }

    fn fire_tapped(&mut self, id: ControlId) {
        if self.node(id).is_some_and(|n| n.pointer.tapped) {
            self.intents.push(record::Intent::Tapped { id });
        }
    }

    fn fire_right_tapped(&mut self, id: ControlId) {
        if self.node(id).is_some_and(|n| n.pointer.right_tapped) {
            self.intents.push(record::Intent::RightTapped { id });
        }
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
