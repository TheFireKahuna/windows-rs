//! Self-hosted DirectComposition + Direct2D 1.3 backend.
//!
//! Implements the reactor [`Backend`] trait by rendering — not instantiating
//! WinUI elements. Each control is a retained [`Node`](node::Node) owning a
//! system-compositor `ContainerVisual`, and the composition tree mirrors the
//! logical tree. The reconciler's `create`/`set_prop`/`*_child` calls mutate the
//! arena, its Taffy layout inputs, and the composition child collections; after
//! each reconcile the host lays the tree out with Taffy, pushes per-node
//! offset/size/opacity/clip onto the containers (no repaint — the compositor
//! handles movement), and repaints only the surfaces of nodes whose own content
//! or size changed. Input is hit-tested against the layout output; ALL control
//! motion (hover/press ink, toggles, scroll glides, progress loops) plays on
//! the system compositor via retained chrome parts (see [`parts`]).
//!
//! Declarative animations (`.animate`, `.transition`, `with_*_transition`,
//! `with_layout_animation`) are compositor-evaluated (see [`animate`]): the
//! backend starts a DWM-side animation and returns to the blocking pump — no
//! ticks, no repaints, zero CPU while motion plays. Exit transitions detach
//! the dying subtree as a top-level ghost visual, released when the scoped
//! batch wrapping its fade reports completion — the compositor's own signal,
//! no timer. Gated behind the `dcomp-backend` feature.

use crate::backend::ControlId;

mod animate;
mod bootstrap;
mod caption;
mod color_out;
mod controls;
mod dispatch;
mod display_change;
mod editor;
mod host;
pub(crate) mod info_badge;
pub(crate) mod info_bar;
pub(crate) mod input;
mod knob;
pub(crate) mod layout;
pub(crate) mod nav;
pub(crate) mod node;
mod pacer;
mod paint;
mod parts;
mod pointer;
mod popup;
pub(crate) mod record;
mod scroll;
mod shape;
mod size;
mod surface;
pub(crate) mod theme;
pub use theme::{set_host_tokens, HostTokens};
pub(crate) mod tsf;
mod uia;
pub(crate) mod visibility;

pub use color_out::set_output_color_transform;
pub use host::DCompHost;
pub use display_change::set_display_change_callback;
pub use visibility::set_window_visibility_callback;
pub(crate) use pointer::{declare, register_element_pointer, PointerSinks};
pub(crate) use size::register_element_size;

use bootstrap::Compositing;
use node::{Arena, Ctrl, Extras, MenuRow, Node};
use paint::PaintCache;

use crate::backend::{Backend, ControlKind, Event, EventHandler, Prop, PropValue};
use crate::style::{
    AccessibilityModifiers, AnimationConfig, GridLength, ImplicitTransitions,
    LayoutAnimationConfig, PointerHandlers,
};
use crate::system_bindings::{CompositionBatchTypes, CompositionScopedBatch, Visual};
use windows_core::Interface as _;

/// A dying visual's exit presentation, kept alive while the compositor plays
/// it. For a destroyed subtree `shown` is normally a FLATTENED
/// [`animate::snapshot_sprite`] of it (whose brush chain keeps the detached
/// source alive), or the live container itself when visual-surface
/// snapshotting failed; for a dismissed popup it is the overlay container
/// playing its close fade. Released by [`DCompBackend::release_ghost`] when
/// the scoped batch wrapping its exit animation reports `Completed` — the
/// compositor's own signal, no timer and no estimated deadline.
struct Ghost {
    id: u64,
    shown: Visual,
    /// Keeps the batch `Completed` subscription alive until release.
    _completed: windows_core::EventRevoker,
}

/// The DirectComposition backend. Owns the node arena, the window's composition
/// infrastructure, and the per-device paint cache.
pub struct DCompBackend {
    arena: Arena,
    comp: Compositing,
    cache: PaintCache,
    /// Shared rasterized chrome-part sources (see [`parts::Atlas`]).
    atlas: parts::Atlas,
    root: Option<ControlId>,
    /// The node whose container is currently attached under the compositor root.
    attached_root: Option<ControlId>,
    /// Viewport in DIPs and the window DPI (96 = 100%).
    dip_size: (f32, f32),
    dpi: f32,
    /// The clickable node currently under the pointer (hover) / pressed.
    hovered_id: Option<ControlId>,
    pressed_id: Option<ControlId>,
    /// The scroll container currently under the pointer (drives thumb fade-in).
    hovered_scroll: Option<ControlId>,
    /// The scroll container whose thumb is being dragged, if any.
    dragging_thumb: Option<ControlId>,
    /// An active knob drag: `(id, value at press, pointer y at press)`. A knob
    /// scrubs on a RELATIVE vertical drag (up = increase) rather than the
    /// slider's absolute positional map, so the gesture origin is latched here.
    knob_drag: Option<(ControlId, f64, f32)>,
    /// Whether a pointer drag is actively streaming value updates — set on the
    /// first MOVE of a slider/knob press, cleared on release.
    ///
    /// This is the difference between a discrete change and a continuous
    /// gesture, and every value chrome keys its motion on it. A natural-motion
    /// spring restarted on each update never leaves rest, so a stream of
    /// updates must move chrome 1:1; only a discrete change (a click, a preset,
    /// the wheel, an external set) may spring. It is global, not per-node,
    /// because a drag on ONE control streams updates to its followers too — the
    /// dial trailing the slider, the output meter trailing both.
    scrubbing: bool,
    /// The node holding keyboard focus (drives the focus ring + Space/Enter).
    focused_id: Option<ControlId>,
    /// A registered viz pointer surface (knob/slider/EQ canvas) being dragged:
    /// its node and the ancestor scroll offset captured at press time (added to
    /// raw move/up coords so element-relative positions stay correct inside a
    /// scrolled chain). Set on down over the surface, cleared on up — implicit
    /// capture for the drag's duration. The sinks are addressed by id at drain,
    /// so the backend holds no closure here.
    pressed_surface: Option<(ControlId, f32)>,
    /// The viz pointer surface currently under the hover, so leaving it can fire
    /// its `exited` sink (there is no per-node exit event otherwise).
    hovered_surface: Option<ControlId>,
    /// The live popup overlay (Select/menu dropdown), if one is open.
    popup: Option<popup::Popup>,
    /// Detached visuals (destroyed subtrees, dismissed popups) playing their
    /// exit fade on the compositor; released by [`Self::release_ghost`] when
    /// their scoped batch completes.
    ghosts: Vec<Ghost>,
    /// Monotonic id source for [`Ghost`]s (keys the batch-completed callback).
    next_ghost: u64,
    /// Composition surfaces hosted under controls on behalf of viz hosts
    /// (see [`surface`]).
    surfaces: surface::SurfaceHost,
    /// Live `TitleBar` node ids in mount order — the caption-geometry cache.
    ///
    /// `WM_NCHITTEST` asks for [`Self::caption_rect`] on every non-client mouse
    /// move, so finding the caption may not scan the arena (an unordered map).
    /// Maintained structurally instead: every arena insert flows through
    /// `create` / `create_with_id` and every removal through `destroy`, all of
    /// which know the kind, so this list is exact by construction and can never
    /// name a node that is no longer in the arena. Ids are never reused, so
    /// even a leaked entry could not alias a later node — but none can leak.
    ///
    /// A `Vec` rather than an `Option` because a tree with two TitleBars is
    /// structurally reachable (nothing in the seam forbids it) and a remount
    /// legitimately mounts the replacement *before* destroying the original —
    /// which an `Option` would resolve by clearing the cache on the destroy,
    /// leaving the live TitleBar unfindable. Order gives a defined policy:
    /// **the first-mounted TitleBar owns the caption**; a second is laid out
    /// and painted as an ordinary node but contributes no non-client region.
    /// (The scan this replaced picked an arbitrary one — map order.)
    titlebars: Vec<ControlId>,
    /// The host window handle (as `isize`) — used for clipboard ownership.
    hwnd: isize,
    /// App notifications queued by the input paths (`fire_*` in [`input`]),
    /// in fire order, drained by the recorder after each input dispatch
    /// ([`record::RecordingBackend::drain_intents`]). The backend never
    /// invokes an app closure: this queue is the seam that keeps handlers on
    /// the app side.
    intents: Vec<record::Intent>,
    /// The §7.3 accelerator table: per node, the declared `(key, mods)` chords
    /// input matches a keydown against. Only the chords live here — the
    /// `on_invoked` callbacks stay in the recorder's app-side `accels` map,
    /// addressed by the matched index via [`record::Intent::Accelerator`]. Fed
    /// by [`record::Cmd::SetKeybindings`] replay; entries die with the node in
    /// [`Backend::destroy`], and ids are never reused so a stale chord could not
    /// re-address a later node.
    keybindings: rustc_hash::FxHashMap<ControlId, Vec<(crate::VirtualKey, crate::VirtualKeyModifiers)>>,
}

impl DCompBackend {
    pub(crate) fn new(comp: Compositing, dip_size: (f32, f32), dpi: f32, hwnd: isize) -> Self {
        Self {
            arena: Arena::default(),
            comp,
            cache: PaintCache::default(),
            atlas: parts::Atlas::default(),
            root: None,
            attached_root: None,
            dip_size,
            dpi,
            hovered_id: None,
            pressed_id: None,
            hovered_scroll: None,
            dragging_thumb: None,
            knob_drag: None,
            scrubbing: false,
            focused_id: None,
            pressed_surface: None,
            hovered_surface: None,
            popup: None,
            ghosts: Vec::new(),
            next_ghost: 0,
            surfaces: surface::SurfaceHost::default(),
            titlebars: Vec::new(),
            hwnd,
            intents: Vec::new(),
            keybindings: rustc_hash::FxHashMap::default(),
        }
    }

    fn scale(&self) -> f32 {
        self.dpi / 96.0
    }

    /// Note the latest reconciled root and (re)attach its container under the
    /// compositor root if it changed.
    pub(crate) fn set_root(&mut self, root: Option<ControlId>) {
        if root != self.attached_root {
            if let Some(old) = self.attached_root.take()
                && let Some(n) = self.arena.get(old)
            {
                self.comp.detach_root(&n.container);
            }
            if let Some(new) = root
                && let Some(n) = self.arena.get(new)
            {
                let _ = self.comp.attach_root(&n.container);
                self.attached_root = Some(new);
            }
        }
        self.root = root;
    }

    /// Full layout + surface paint. Run after each reconcile and on resize.
    pub(crate) fn relayout_and_paint(&mut self) {
        // The tree just settled — audit the caption cache while the structural
        // edits that could have drifted it are still fresh. `cfg!` rather than
        // `#[cfg]` so the audit type-checks in every configuration; `if false`
        // costs a release build nothing.
        if cfg!(debug_assertions) {
            self.audit_titlebars();
        }
        if let Some(root) = self.root {
            let (w, h) = self.dip_size;
            let scale = self.scale();
            layout::compute(&mut self.arena, root, w, h, scale);
            self.repaint();
        }
    }

    /// Repaint dirty node surfaces (no relayout).
    pub(crate) fn repaint(&mut self) {
        if let Some(root) = self.root {
            let scale = self.scale();
            if paint::paint(
                &self.comp,
                &mut self.cache,
                &mut self.atlas,
                &mut self.arena,
                root,
                scale,
                self.scrubbing,
            )
            .is_err()
            {
                // Device loss: drop cached resources; next paint rebuilds them
                // (parts re-bind to freshly rasterized sources by epoch).
                self.cache.invalidate();
                self.atlas.clear();
            }
        }
    }

    /// React to a window resize (physical pixels). Re-folds DPI into the root
    /// scale and the DIP viewport, then relays out and repaints.
    pub(crate) fn resize(&mut self, pixel_w: i32, pixel_h: i32, dpi: u32) {
        if dpi > 0 && dpi as f32 != self.dpi {
            // Pixel scale changed: every atlas source is the wrong resolution.
            self.atlas.clear();
        }
        if dpi > 0 {
            self.dpi = dpi as f32;
        }
        self.comp
            .set_scale_and_pixels(pixel_w.max(1), pixel_h.max(1), self.dpi);
        self.dip_size = self.comp.dip_size();
        self.relayout_and_paint();
    }

    /// Re-resolve the theme background (called on `WM_SETTINGCHANGE`). Token
    /// resolution for node colors is the GUI's job; here we only own the window
    /// backdrop, which we flip with the system light/dark setting.
    pub(crate) fn apply_theme(&mut self, dark: bool) {
        self.comp.set_background(host::window_backdrop(dark));
    }

    fn node(&self, id: ControlId) -> Option<&Node> {
        self.arena.get(id)
    }
    fn node_mut(&mut self, id: ControlId) -> Option<&mut Node> {
        self.arena.get_mut(id)
    }

    /// Point `child`'s parent link at `parent` (it just entered that child list).
    fn link_parent(&mut self, child: ControlId, parent: ControlId) {
        if let Some(c) = self.node_mut(child) {
            c.parent = Some(parent);
        }
        uia::note_tree_change();
    }

    /// Clear `child`'s parent link, but only if it still names `parent`.
    ///
    /// The guard is what keeps the link exact through a reparent: the buffer
    /// legitimately replays "attach to B" before "detach from A", and an
    /// unconditional clear there would erase the newer, correct link.
    fn unlink_parent(&mut self, child: ControlId, parent: ControlId) {
        if let Some(c) = self.node_mut(child)
            && c.parent == Some(parent)
        {
            c.parent = None;
        }
        uia::note_tree_change();
    }

    /// Attach (or clear) one of a `TitleBar`'s caption slot children.
    ///
    /// `footer == false` is the centered `Content` slot: it spans both caption
    /// columns and centers across the full strip width. `footer == true` is the
    /// trailing `RightHeader` slot: it lands in the right auto-sized column, hard
    /// against the trailing edge. The mounted subtree becomes a real composition
    /// child of the TitleBar node (laid out by Taffy like any other child); the
    /// previously tracked slot child, if any, is detached first. `slot == None`
    /// clears the slot.
    fn set_title_slot(&mut self, id: ControlId, slot: Option<ControlId>, footer: bool) {
        use taffy::prelude::*;
        // Swap the tracked child out of (and the new one into) the TitleBar's
        // composition children, marking the child order for re-sync.
        let mut detached = None;
        if let Some(tb) = self.node_mut(id) {
            let prev = if footer {
                tb.title_footer.take()
            } else {
                tb.title_content.take()
            };
            if let Some(prev) = prev {
                tb.children.retain(|c| *c != prev);
                tb.children_dirty = true;
                detached = Some(prev);
            }
            if let Some(new) = slot {
                tb.children.push(new);
                tb.children_dirty = true;
                if footer {
                    tb.title_footer = Some(new);
                } else {
                    tb.title_content = Some(new);
                }
            }
        }
        // Keep the parent links the exact inverse of the edited child list.
        if let Some(prev) = detached {
            self.unlink_parent(prev, id);
        }
        if let Some(new) = slot {
            self.link_parent(new, id);
        }
        // Place the freshly attached slot inside the caption grid. Alignment is
        // driven through the child's `h_align`/`v_align` so the per-layout
        // `resolve_align` pass keeps `justify_self`/`align_self` in agreement.
        if let Some(new) = slot
            && let Some(child) = self.node_mut(new)
        {
            child.style.grid_row.start = line(1);
            if footer {
                // Trailing auto column, vertically centered; its own horizontal
                // alignment is irrelevant in a track sized to its content.
                child.style.grid_column.start = line(2);
                child.style.grid_column.end = span(1);
                child.v_align = 1;
            } else {
                // Span both columns and stretch across the full caption width:
                // the app's content row owns its own spread (brand hard-left,
                // device centered — the mockup layout). A child with an
                // explicit alignment still wins via `resolve_align`.
                child.style.grid_column.start = line(1);
                child.style.grid_column.end = span(2);
                child.h_align = 3;
                child.v_align = 1;
            }
        }
    }

    /// The TitleBar node that owns the caption (the custom caption band), if
    /// the tree has one. O(1) — see [`Self::titlebars`].
    fn titlebar_id(&self) -> Option<ControlId> {
        self.titlebars.first().copied()
    }

    /// Debug-only: prove [`Self::titlebars`] still names exactly the live
    /// TitleBar nodes. The list is maintained structurally at create/destroy,
    /// so drift here would mean a mint or teardown path that bypassed them —
    /// and a cached id naming a destroyed node would hand `WM_NCHITTEST` a
    /// stale caption region for the rest of the window's life, which is worse
    /// than the arena scan this cache replaced.
    fn audit_titlebars(&self) {
        // Arena order is unspecified, so compare membership, not sequence.
        let live = self
            .arena
            .iter()
            .filter(|(_, n)| n.kind == ControlKind::TitleBar)
            .count();
        debug_assert_eq!(
            live,
            self.titlebars.len(),
            "TitleBar cache drifted: {live} live, {} cached",
            self.titlebars.len()
        );
        debug_assert!(
            self.titlebars.iter().all(|id| self
                .arena
                .get(*id)
                .is_some_and(|n| n.kind == ControlKind::TitleBar)),
            "TitleBar cache names a node that is gone or is no longer a TitleBar"
        );
    }

    /// The caption band's layout box in window DIPs (`(x, y, w, h)`), if a
    /// TitleBar is mounted — the host's non-client hit-test region.
    pub(crate) fn caption_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let n = self.arena.get(self.titlebar_id()?)?;
        Some((n.rect.x, n.rect.y, n.rect.w, n.rect.h))
    }

    /// The drawn back button's box in window DIPs, if the mounted TitleBar
    /// shows one. `None` when there is no TitleBar or its back button is
    /// hidden — the host then never reports `HTSYSMENU` for that band.
    pub(crate) fn back_button_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let n = self.arena.get(self.titlebar_id()?)?;
        let r = caption::back_rect(
            n.extras(),
            windows_canvas_core::Rect::from_xywh(n.rect.x, n.rect.y, n.rect.w, n.rect.h),
        )?;
        Some((r.left, r.top, r.width(), r.height()))
    }

    /// Whether the back button is currently clickable — drawn AND enabled. A
    /// visible-but-disabled button still paints (greyed) but must not take the
    /// hit, so the band under it stays draggable.
    pub(crate) fn back_button_active(&self) -> bool {
        self.titlebar_id()
            .and_then(|id| self.arena.get(id))
            .is_some_and(|n| n.extras().back_button_visible && n.extras().back_button_enabled)
    }

    /// Queue the mounted TitleBar's `BackRequested` for the app, if declared.
    pub(crate) fn raise_back_requested(&mut self) {
        if let Some(id) = self
            .titlebar_id()
            .filter(|id| self.arena.get(*id).is_some_and(|n| n.interactivity.back))
        {
            self.fire_unit(id, Event::BackRequested);
        }
    }

    /// Whether the point sits over content that must stay client (an
    /// interactive control or a registered viz pointer surface) — keeps the
    /// caption drag region from swallowing the titlebar's own controls.
    pub(crate) fn wants_client_at(&self, x: f32, y: f32) -> bool {
        self.interactive_at(x, y).is_some() || self.surface_at(x, y).is_some()
    }

    /// Repaint the caption band (hover / maximized state changed).
    pub(crate) fn repaint_caption(&mut self) {
        if let Some(id) = self.titlebar_id() {
            if let Some(n) = self.arena.get_mut(id) {
                n.mark_dirty();
            }
            self.repaint();
        }
    }

    /// Mark every node's surface for repaint (e.g. on a theme change), then
    /// repaint. Layout is unchanged, so no relayout is needed.
    pub(crate) fn mark_all_dirty_and_repaint(&mut self) {
        // The output colour map may have changed (display / theme edge): the
        // atlas sources carry baked mapped colours, so re-rasterize them too.
        self.atlas.clear();
        for slot in self.arena.iter_mut() {
            slot.mark_dirty();
        }
        self.repaint();
    }

    /// Detach a to-be-destroyed node's container as a top-level "ghost" and
    /// play its exit transition on it, compositor-side. The whole visual
    /// subtree rides along (child visuals are COM-referenced by the
    /// container), frozen at its last painted content. Skipped when the node
    /// has no visible exit effect, was never laid out, or is not currently
    /// reachable from the compositor root (e.g. it already sits inside an
    /// ancestor's ghost — one fade is enough).
    fn spawn_exit_ghost(&mut self, id: ControlId) {
        let Some(node) = self.arena.get(id) else { return };
        let Some(cfg) = node.exit else { return };
        if !cfg.is_visible_effect() || node.last_off.is_none() {
            return;
        }

        // Scoped batch around the exit animation: its `Completed` event is the
        // release signal. Without one there is no way to know when the fade
        // ends, so skip the effect rather than leak a top-level visual.
        let Ok(batch) = self
            .comp
            .compositor()
            .CreateScopedBatch(CompositionBatchTypes::Animation)
        else {
            animate::warn(format_args!("ghost {id}: scoped batch failed — exit skipped"));
            return;
        };

        // Walk to the compositor root accumulating the absolute (root-space)
        // offset — exact even inside scrolled containers, whose child offsets
        // include the scroll translation.
        let root_ident = self.comp.root_identity();
        let (mut ax, mut ay) = (0.0f32, 0.0f32);
        let mut cur = node.vis.clone();
        let mut reached_root = false;
        loop {
            if let Ok(o) = cur.Offset() {
                ax += o.x;
                ay += o.y;
            }
            let Ok(parent) = cur.Parent() else { break };
            let ident = parent
                .cast::<windows_core::IUnknown>()
                .map(|u| u.as_raw())
                .unwrap_or(core::ptr::null_mut());
            if ident == root_ident {
                reached_root = true;
                break;
            }
            let Ok(pv) = parent.cast::<crate::system_bindings::IVisual>() else { break };
            cur = pv;
        }
        if !reached_root {
            animate::warn(format_args!("ghost {id}: parent walk did not reach root — skipped"));
            return;
        }

        let container = node.container.clone();
        let vis = node.vis.clone();
        let (w, h) = (node.rect.w, node.rect.h);
        let center = (w > 0.0 && h > 0.0).then(|| (w / 2.0, h / 2.0));

        // Detach the subtree from its parent. A visual can hold only one
        // parent, and the old parent's child re-sync (already marked dirty by
        // `remove_child`) no longer knows this visual either way.
        let Ok(source) = container.cast::<Visual>() else { return };
        if let Ok(parent) = vis.Parent()
            && let Ok(children) = parent.Children()
        {
            let _ = children.Remove(&source);
        }

        // Prefer a FLATTENED ghost: one visual-surface sprite re-compositing
        // the detached subtree, so the exit fade is a single layer and
        // overlapping translucent children cannot bleed through mid-fade. The
        // sprite's brush chain keeps the source subtree alive. Fall back to
        // showing the live container when snapshotting fails.
        let shown = match animate::snapshot_sprite(self.comp.compositor(), &source, w, h) {
            Ok(sprite) => sprite.cast::<Visual>().unwrap_or(source),
            Err(e) => {
                animate::warn(format_args!("ghost {id}: snapshot failed ({e:?}) — live fallback"));
                source
            }
        };

        // Present top-level at the same on-screen position.
        if self.comp.attach_root_visual(&shown).is_err() {
            animate::warn(format_args!("ghost {id}: root attach failed — dropped"));
            return;
        }
        if let Ok(v) = shown.cast::<crate::system_bindings::IVisual>() {
            let _ = v.SetOffset(windows_numerics::Vector3::new(ax, ay, 0.0));
        }

        animate::start(self.comp.compositor(), &shown, &cfg, center);

        self.park_ghost(shown, batch);
    }

    /// Park `shown` as a [`Ghost`] until `batch` — which must wrap the exit
    /// animations just started on it — completes. `End` seals the batch; the
    /// compositor raises `Completed` (on this thread, via the dispatcher queue)
    /// the moment the last enclosed animation finishes, and the handler
    /// releases the ghost. A batch that cannot deliver that signal releases the
    /// visual immediately instead of leaking it.
    fn park_ghost(&mut self, shown: Visual, batch: CompositionScopedBatch) {
        let id = self.next_ghost;
        self.next_ghost += 1;
        let hwnd = self.hwnd;
        let revoker = batch
            .Completed(move |_, _| {
                // Fires from the message pump, outside any backend borrow. The
                // re-entrant case (`None`) can only happen if composition events
                // ever dispatch mid-reconcile — defer through the pump then.
                if host::with_backend(|b| b.release_ghost(id)).is_none() {
                    host::post_ui(hwnd, move || {
                        let _ = host::with_backend(|b| b.release_ghost(id));
                    });
                }
            })
            .ok()
            .filter(|_| batch.End().is_ok());
        match revoker {
            Some(revoker) => self.ghosts.push(Ghost { id, shown, _completed: revoker }),
            None => {
                // No completion signal will ever come — drop the exit
                // presentation now rather than leak the visual.
                animate::warn(format_args!("ghost batch subscribe/end failed — released early"));
                self.comp.remove_root_visual(&shown);
            }
        }
    }

    /// Release one ghost: its scoped batch completed, the exit animation is
    /// done compositor-side. Dropping the ghost also revokes the subscription.
    pub(crate) fn release_ghost(&mut self, id: u64) {
        if let Some(i) = self.ghosts.iter().position(|g| g.id == id) {
            let g = self.ghosts.swap_remove(i);
            self.comp.remove_root_visual(&g.shown);
        }
    }
}

impl DCompBackend {
    /// Build a control's retained node and its composition visuals.
    ///
    /// Shared by both minting paths: [`Backend::create`], which assigns the id
    /// from the arena, and [`CreateWithId::create_with_id`], which takes one
    /// minted by the command-buffer seam.
    fn build_node(&mut self, kind: ControlKind) -> Node {
        let container = self
            .comp
            .new_container()
            .expect("compositor container allocation");
        let mut node = Node::new(kind, container);
        // Scroll/overflow containers clip their children to their own bounds;
        // a ProgressBar clips its indeterminate sweep at the track edges.
        if matches!(
            kind,
            ControlKind::ScrollViewer | ControlKind::ScrollView | ControlKind::ProgressBar
        ) && let Ok(clip) = self.comp.new_inset_clip()
        {
            use windows_core::Interface;
            if let Ok(c) = clip.cast::<crate::system_bindings::CompositionClip>() {
                let _ = node.vis.SetClip(&c);
            }
            node.clip = Some(clip);
        }
        // Scroll containers get a content CARRIER visual their children parent
        // into: scrolling animates this one visual's Offset on the compositor
        // (see `Node::scroll_glide`), so a wheel glide never ticks the app.
        if node.is_scroll() {
            node.scroll_content = self.comp.new_container().ok();
        }
        node
    }

    /// Register a freshly inserted node in the kind-keyed id caches. Called
    /// from both minting paths, so [`Self::titlebars`] tracks the arena exactly.
    fn note_inserted(&mut self, id: ControlId, kind: ControlKind) {
        if kind == ControlKind::TitleBar && !self.titlebars.contains(&id) {
            if !self.titlebars.is_empty() {
                animate::warn(format_args!(
                    "TitleBar {id}: a second TitleBar is mounted — the first \
                     ({:?}) keeps the caption; this one paints as a plain node",
                    self.titlebars[0]
                ));
            }
            self.titlebars.push(id);
        }
    }
}

impl Backend for DCompBackend {
    fn create(&mut self, id: ControlId, kind: ControlKind) {
        let node = self.build_node(kind);
        self.arena.insert_with_id(id, node);
        self.note_inserted(id, kind);
    }

    fn set_prop(&mut self, id: ControlId, prop: Prop, value: &PropValue) {
        // Any prop write can move a value the UIA property snapshot caches
        // (Name, IsEnabled, ToggleState, …), so retire the snapshots first.
        uia::note_state_change();
        // An InfoBar that is about to OPEN must announce itself (it appeared
        // without the user asking, so nothing else would mention it). Sampled
        // before the write and compared after, because the opening EDGE is what
        // announces — a bar already on screen must not interrupt the user again
        // every time an unrelated prop of it changes.
        let was_open = self
            .node(id)
            .filter(|n| n.kind == ControlKind::InfoBar)
            .map(|n| n.extras().bar_open);
        let Some(node) = self.node_mut(id) else { return };
        // A focused AutoSuggestBox whose filtered list just changed refreshes
        // its open dropdown in place (deferred until the node borrow ends).
        if apply_prop(node, prop, value) {
            self.refresh_suggest(id);
        }
        if was_open == Some(false)
            && self.node(id).is_some_and(|n| n.extras().bar_open)
        {
            self.uia_announce_live_region(id);
        }
    }

    fn append_child(&mut self, parent: ControlId, child: ControlId) {
        if let Some(p) = self.node_mut(parent) {
            p.children.push(child);
            p.children_dirty = true;
        }
        self.link_parent(child, parent);
    }

    fn insert_child(&mut self, parent: ControlId, index: usize, child: ControlId) {
        if let Some(p) = self.node_mut(parent) {
            let i = index.min(p.children.len());
            p.children.insert(i, child);
            p.children_dirty = true;
        }
        self.link_parent(child, parent);
    }

    fn remove_child(&mut self, parent: ControlId, index: usize) {
        let mut gone = None;
        if let Some(p) = self.node_mut(parent)
            && index < p.children.len()
        {
            gone = Some(p.children.remove(index));
            p.children_dirty = true;
        }
        if let Some(c) = gone {
            self.unlink_parent(c, parent);
        }
    }

    fn replace_child(&mut self, parent: ControlId, index: usize, new: ControlId) {
        let mut gone = None;
        if let Some(p) = self.node_mut(parent)
            && index < p.children.len()
        {
            gone = Some(std::mem::replace(&mut p.children[index], new));
            p.children_dirty = true;
        }
        if let Some(c) = gone {
            self.unlink_parent(c, parent);
            self.link_parent(new, parent);
        }
    }

    /// A pure reorder inside one child list — every moved node keeps the same
    /// parent, so no parent link changes.
    fn move_child(&mut self, parent: ControlId, from: usize, to: usize) {
        if let Some(p) = self.node_mut(parent)
            && from < p.children.len()
            && to < p.children.len()
        {
            let c = p.children.remove(from);
            p.children.insert(to, c);
            p.children_dirty = true;
        }
    }

    fn destroy(&mut self, id: ControlId) {
        // Registrations are id-keyed, and ids are never reused, so a stale entry
        // could not be re-addressed — but dropping them here keeps the registries
        // bounded when a subscriber leaks its `Subscription`.
        size::forget(id);
        pointer::forget(id);
        self.surfaces.forget(id);
        uia::forget(self.hwnd, id);
        // Accelerator chords are scoped to the node's lifetime — drop them so a
        // keydown can never match a chord for a node that no longer exists.
        self.keybindings.remove(&id);
        // Drop the caption cache entry in lock-step with the arena entry: a
        // cached id naming a destroyed node would hand the host a stale (or
        // absent) non-client region for the rest of the window's life.
        self.titlebars.retain(|t| *t != id);
        // This node's parent link dies with the entry, but its children would be
        // left naming a parent that is no longer in the arena. Cut those links so
        // the inverse of `children` stays exact for every node that survives.
        if let Some(kids) = self.arena.get(id).map(|n| n.children.clone()) {
            for k in kids {
                self.unlink_parent(k, id);
            }
        }
        if self.attached_root == Some(id) {
            if let Some(n) = self.arena.get(id) {
                self.comp.detach_root(&n.container);
            }
            self.attached_root = None;
        }
        // An exit transition detaches the container as a compositor-side ghost
        // before the arena entry (and with it the last live Rust ref outside
        // the ghost list) goes away.
        self.spawn_exit_ghost(id);
        self.arena.remove(id);
    }

    /// This backend never stores or invokes an [`EventHandler`]: the closure
    /// lives app-side in the recorder's handler map and is invoked from queued
    /// intents. The trait entry point exists for a directly-driven backend and
    /// keeps only the declaration, exactly as replay does via
    /// [`record::FrontBackend::declare_event`].
    fn attach_event(&mut self, id: ControlId, event: Event, handler: EventHandler) {
        let _ = handler;
        self.declare_event(id, event);
    }

    fn detach_event(&mut self, id: ControlId, event: Event) {
        if let Some(node) = self.node_mut(id) {
            node.note_interactivity(event, false);
        }
    }

    /// Same declaration-only treatment as `attach_event`: the closures stay
    /// app-side; the node keeps their presence bits.
    fn set_pointer_handlers(&mut self, id: ControlId, handlers: Option<&PointerHandlers>) {
        self.set_pointer_interest(
            id,
            handlers.map(node::PointerInterest::of).unwrap_or_default(),
        );
    }

    fn set_accessibility(&mut self, id: ControlId, accessibility: &AccessibilityModifiers) {
        if let Some(node) = self.node_mut(id) {
            node.accessibility = Some(accessibility.clone());
        }
    }

    /// A `TitleBar`'s centered `Content` slot (WinUI `TitleBar.Content`). Other
    /// element-header kinds (e.g. Expander) draw their header from props here, so
    /// only TitleBar consumes an element header.
    fn set_header_element(&mut self, id: ControlId, header_id: Option<ControlId>) {
        if self.node(id).map(|n| n.kind) == Some(ControlKind::TitleBar) {
            self.set_title_slot(id, header_id, false);
        }
    }

    /// A `TitleBar`'s trailing `RightHeader`/footer slot (WinUI
    /// `TitleBar.RightHeader`) — where the Simple/Pro mode toggle lives.
    fn set_pane_element(&mut self, id: ControlId, pane_id: Option<ControlId>) {
        if self.node(id).map(|n| n.kind) == Some(ControlKind::TitleBar) {
            self.set_title_slot(id, pane_id, true);
        }
    }


    // ── Compositor animations (DWM-evaluated; no app ticks, no repaints) ──

    fn set_implicit_transitions(
        &mut self,
        id: ControlId,
        transitions: Option<ImplicitTransitions>,
    ) {
        // Disjoint field borrows: the arena node and the compositor handle.
        let Some(node) = self.arena.get_mut(id) else { return };
        node.transitions = transitions;
        if transitions.is_some_and(|t| t.scale.is_some()) {
            animate::note_scale_intent(node);
        }
        let coll = animate::build_implicit(
            self.comp.compositor(),
            node.transitions.as_ref(),
            node.layout_anim.as_ref(),
        )
        .ok()
        .flatten();
        node.set_implicit(coll);
    }

    fn set_layout_animation(&mut self, id: ControlId, config: Option<LayoutAnimationConfig>) {
        let Some(node) = self.arena.get_mut(id) else { return };
        node.layout_anim = config;
        // Damping/period are baked into the cached spring; rebuild on change.
        node.spring_anim = None;
        let coll = animate::build_implicit(
            self.comp.compositor(),
            node.transitions.as_ref(),
            node.layout_anim.as_ref(),
        )
        .ok()
        .flatten();
        node.set_implicit(coll);
    }

    fn run_property_animation(&mut self, id: ControlId, config: Option<AnimationConfig>) {
        let Some(cfg) = config else { return };
        if !cfg.is_visible_effect() {
            return;
        }
        let Some(node) = self.arena.get_mut(id) else { return };
        if animate::wants_center(&cfg) {
            animate::note_scale_intent(node);
        }
        let center = (node.rect.w > 0.0 && node.rect.h > 0.0)
            .then(|| (node.rect.w / 2.0, node.rect.h / 2.0));
        if let Ok(v) = node.container.cast::<Visual>() {
            animate::start(self.comp.compositor(), &v, &cfg, center);
        }
    }

    fn set_exit_transition(&mut self, id: ControlId, config: Option<AnimationConfig>) {
        let Some(node) = self.arena.get_mut(id) else { return };
        node.exit = config;
        // Maintain the centre pivot from now on so a scale-out exit pivots
        // correctly even though it starts after the node stops laying out.
        if config.as_ref().is_some_and(animate::wants_center) {
            animate::note_scale_intent(node);
        }
    }
}

// ── Intent seam (see `record`) ──────────────────────────────────────────────
impl DCompBackend {
    /// Note that `event` has an app-side handler. The declaration is all this
    /// backend keeps — see [`Backend::attach_event`] above.
    pub(crate) fn declare_event(&mut self, id: ControlId, event: Event) {
        if let Some(node) = self.node_mut(id) {
            node.note_interactivity(event, true);
        }
    }

    /// Note which of the app's pointer callbacks exist for `id` — the bits
    /// input consults synchronously (hit-testability, pointer capture).
    pub(crate) fn set_pointer_interest(&mut self, id: ControlId, interest: node::PointerInterest) {
        if let Some(node) = self.node_mut(id) {
            node.pointer = interest;
        }
    }

    /// Revision-gated `Prop::Value` write (§7.2 applied to control values).
    ///
    /// `based_on` is the input revision the app had been consulted about when
    /// it recorded this write. A node whose revision has moved past it means
    /// the user drove the value after the app last heard about it — the write
    /// is a stale echo and applying it would snap the chrome backwards, so it
    /// is dropped; the app converges through the newer `ValueChanged` intent
    /// already queued or delivered. The gesture-time half of the same gate
    /// (`node.pressed`) lives in [`apply_prop`].
    pub(crate) fn set_value_stamped(&mut self, id: ControlId, value: f64, based_on: u64) {
        if self.node(id).is_some_and(|n| !n.accepts_value_echo(based_on)) {
            return;
        }
        Backend::set_prop(self, id, Prop::Value, &PropValue::F64(value));
    }

    /// Revision-gated editor-text write — the §7.2 arrival rules, text half.
    ///
    /// In order: while an IME composition is active **nothing** applies (the
    /// composition guard — every platform that let a programmatic write land
    /// mid-composition shipped broken CJK input; the app converges through
    /// the `TextChanged` the commit fires). An echo-identical write is a
    /// strict no-op — it never moves the caret. A write stamped older than
    /// the buffer's revision is a stale echo of text the user has already
    /// superseded and is dropped — the app converges through the newer
    /// intent already queued. A fresh write applies with caret
    /// position-mapping ([`editor::Editor::apply_program_text`]), never
    /// collapse-to-end.
    pub(crate) fn set_text_stamped(&mut self, id: ControlId, text: &str, based_on: u64) {
        let Some(node) = self.node_mut(id) else { return };
        if !apply_text_stamped(node, text, based_on) {
            // Not an editor (a stamped write can only reach one through the
            // recorder's kind gate, so this is a direct caller): fall through
            // to the plain prop path.
            Backend::set_prop(self, id, Prop::Value, &PropValue::Str(text.into()));
        }
    }

    /// Record the node's declared accelerator chords (§7.3). An empty list
    /// clears the entry; input matches keydowns against this table
    /// ([`input`]'s `match_accelerator`) and fires the app callback through
    /// [`record::Intent::Accelerator`], keeping the closure app-side.
    pub(crate) fn set_keybindings(
        &mut self,
        id: ControlId,
        keys: Vec<(crate::VirtualKey, crate::VirtualKeyModifiers)>,
    ) {
        if keys.is_empty() {
            self.keybindings.remove(&id);
        } else {
            self.keybindings.insert(id, keys);
        }
    }

}

impl record::FrontBackend for DCompBackend {
    fn declare_event(&mut self, id: ControlId, event: Event) {
        Self::declare_event(self, id, event);
    }

    fn set_pointer_interest(&mut self, id: ControlId, interest: node::PointerInterest) {
        Self::set_pointer_interest(self, id, interest);
    }

    fn set_value_stamped(&mut self, id: ControlId, value: f64, based_on: u64) {
        Self::set_value_stamped(self, id, value, based_on);
    }

    fn set_text_stamped(&mut self, id: ControlId, text: &str, based_on: u64) {
        Self::set_text_stamped(self, id, text, based_on);
    }

    fn set_keybindings(
        &mut self,
        id: ControlId,
        keys: Vec<(crate::VirtualKey, crate::VirtualKeyModifiers)>,
    ) {
        Self::set_keybindings(self, id, keys);
    }

    /// Hand the queued intents to the host, in fire order, for the recorder's
    /// app-side resolution.
    fn take_intents(&mut self) -> Vec<record::Intent> {
        std::mem::take(&mut self.intents)
    }
}

/// Apply one reconciler prop write to a node, and report whether the caller
/// must refresh an open suggestion dropdown afterwards (it needs the backend,
/// which this deliberately does not take — everything else a prop write does is
/// a mutation of the node itself).
///
/// Split out of [`Backend::set_prop`] so the whole prop vocabulary can be
/// driven against a real [`Node`] without a window: the set and the reset of a
/// prop are one contract, and a test that can only exercise half of it cannot
/// see the two disagree.
pub(crate) fn apply_prop(node: &mut Node, prop: Prop, value: &PropValue) -> bool {
    use taffy::prelude::*;
    let mut refresh_suggest = false;
    match (prop, value) {
        // ── Prop removal — a conditional prop diffed away reverts to its
        // default (e.g. a Segmented pill losing its active accent fill) ──
        (_, PropValue::Unset) => reset_prop(node, prop),
        // ── Paint props (mark the node's surface dirty) ──────────────
        (Prop::Background, PropValue::Color(c)) => {
            node.paint.background = Some(*c);
            node.mark_dirty();
        }
        (Prop::Foreground, PropValue::Color(c)) => {
            node.paint.foreground = Some(*c);
            node.mark_dirty();
        }
        (Prop::BorderBrush, PropValue::Color(c)) => {
            node.paint.border_brush = Some(*c);
            node.mark_dirty();
        }
        (Prop::BorderThickness, PropValue::Thickness(t)) => {
            node.paint.border_thickness = t.left as f32;
            // Border thickness also insets content in layout.
            node.style.border = Rect {
                left: length(t.left as f32),
                right: length(t.right as f32),
                top: length(t.top as f32),
                bottom: length(t.bottom as f32),
            };
            node.mark_dirty();
        }
        (Prop::CornerRadius, PropValue::F64(v)) => {
            node.paint.corner_radius = *v as f32;
            node.mark_dirty();
        }
        (Prop::Fill, PropValue::Color(c)) => {
            node.paint.fill = Some(*c);
            node.mark_dirty();
        }
        (Prop::Stroke, PropValue::Color(c)) => {
            node.paint.stroke = Some(*c);
            node.mark_dirty();
        }
        (Prop::StrokeThickness, PropValue::F64(v)) => {
            node.paint.stroke_thickness = *v as f32;
            node.mark_dirty();
        }
        (Prop::LineEndpoints, PropValue::LineEndpoints(l)) => {
            node.paint.line = *l;
            node.mark_dirty();
        }
        (Prop::StyleVariant, PropValue::I32(v)) => {
            node.paint.style_variant = *v;
            node.mark_dirty();
        }
        (Prop::IsEnabled, PropValue::Bool(b)) => {
            node.paint.is_enabled = *b;
            node.mark_dirty();
        }

        // An Expander's title arrives as `Prop::Header`; it paints as the
        // node's label like every other text-bearing control.
        (Prop::Content | Prop::Text | Prop::Header, PropValue::Str(s)) => {
            // For an editable kind (AutoSuggestBox carries its text via
            // `Prop::Text`), write the editor buffer instead of the label.
            if node.editor.is_some() {
                direct_editor_text(node, s);
            } else {
                node.paint.text = s.clone();
                node.text_dirty = true;
            }
            node.mark_dirty();
        }
        // TextBox / PasswordBox carry their text via `Prop::Value(Str)`.
        (Prop::Value, PropValue::Str(s)) if node.editor.is_some() => {
            direct_editor_text(node, s);
            node.mark_dirty();
        }
        (Prop::Precision, PropValue::I32(v)) => {
            node.ctrl_mut().precision = Some(*v);
            // Reformat the seeded value to the new precision (the `Value`
            // prop usually arrives before `Precision`). Never while focused
            // — the user owns the buffer mid-edit.
            if node.kind == ControlKind::NumberBox && !node.focused {
                let value = node.ctrl().value;
                if let Some(ed) = &mut node.editor {
                    ed.seeded = false;
                }
                seed_number_text(node, value);
                node.mark_dirty();
            }
        }
        (Prop::LargeChange, PropValue::F64(v)) => node.ctrl_mut().large_change = Some(*v),
        (Prop::HorizontalContentAlignment, PropValue::I32(v)) => {
            node.ctrl_mut().content_align = *v;
            if let Some(ed) = &mut node.editor {
                ed.layout_dirty = true;
            }
            node.mark_dirty();
        }
        (Prop::FontSize, PropValue::F64(v)) => {
            node.paint.font_size = *v as f32;
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::FontWeight, PropValue::U16(w)) => {
            node.paint.font_weight = *w;
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::FontFamily, PropValue::Str(s)) => {
            node.paint.font_family = Some(s.clone());
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::TextWrapping | Prop::TextWrappingWrap, _) => {
            // WinRT TextWrapping: NoWrap = 1, Wrap = 2, WrapWholeWords = 3 — and
            // 0 for a widget that never set one, since the generated TextBlock
            // bindings push this prop unconditionally and the field's Rust default
            // is `TextWrapping(0)`. Only a real Wrap value wraps: 0 is "unset",
            // which must mean NoWrap to match XAML's own TextBlock default.
            // Reading it as `!= 1` instead marked virtually every text node in the
            // tree as wrapping — inert only for as long as the DWrite box stayed
            // unconstrained (see `layout::build_text_layout`), and silently
            // wrapping every label the moment it did not.
            let wrap = match value {
                PropValue::I32(v) => *v > 1,
                PropValue::Bool(b) => *b,
                _ => true,
            };
            node.paint.wrap = wrap;
            node.text_dirty = true;
            node.mark_dirty();
        }

        // ── Visual prop applied straight onto the container ──────────
        (Prop::Opacity, PropValue::F64(v)) => {
            let _ = node.vis.SetOpacity((*v as f32).clamp(0.0, 1.0));
        }

        // ── Layout props (Taffy inputs; relayout runs each reconcile) ─
        (Prop::Padding, PropValue::Thickness(t)) => {
            node.style.padding = Rect {
                left: length(t.left as f32),
                right: length(t.right as f32),
                top: length(t.top as f32),
                bottom: length(t.bottom as f32),
            };
        }
        (Prop::Margin, PropValue::Thickness(t)) => {
            node.style.margin = Rect {
                left: length(t.left as f32),
                right: length(t.right as f32),
                top: length(t.top as f32),
                bottom: length(t.bottom as f32),
            };
        }
        (Prop::Width, PropValue::F64(v)) => node.style.size.width = length(*v as f32),
        (Prop::Height, PropValue::F64(v)) => node.style.size.height = length(*v as f32),
        (Prop::MinWidth, PropValue::F64(v)) => node.style.min_size.width = length(*v as f32),
        (Prop::MinHeight, PropValue::F64(v)) => node.style.min_size.height = length(*v as f32),
        (Prop::MaxWidth, PropValue::F64(v)) => node.style.max_size.width = length(*v as f32),
        (Prop::MaxHeight, PropValue::F64(v)) => node.style.max_size.height = length(*v as f32),

        (Prop::HorizontalAlignment, PropValue::I32(v)) => node.h_align = *v,
        (Prop::VerticalAlignment, PropValue::I32(v)) => node.v_align = *v,

        (Prop::Orientation, PropValue::I32(v)) => {
            // WinRT Orientation: Vertical = 0, Horizontal = 1.
            node.style.flex_direction = if *v == 1 {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            };
            apply_stack_gap(node);
        }
        (Prop::Spacing, PropValue::F64(v)) => {
            node.spacing = *v as f32;
            apply_stack_gap(node);
        }
        (Prop::ColumnSpacing, PropValue::F64(v)) => node.style.gap.width = length(*v as f32),
        (Prop::RowSpacing, PropValue::F64(v)) => node.style.gap.height = length(*v as f32),
        (Prop::GridRows, PropValue::GridLengths(g)) => node.grid_rows = clone_lengths(g),
        (Prop::GridColumns, PropValue::GridLengths(g)) => node.grid_cols = clone_lengths(g),

        (Prop::AttachedGridRow, PropValue::I32(v)) => {
            node.style.grid_row.start = line((*v + 1) as i16);
        }
        (Prop::AttachedGridColumn, PropValue::I32(v)) => {
            node.style.grid_column.start = line((*v + 1) as i16);
        }
        (Prop::AttachedGridRowSpan, PropValue::I32(v)) => {
            node.style.grid_row.end = span((*v).max(1) as u16);
        }
        (Prop::AttachedGridColumnSpan, PropValue::I32(v)) => {
            node.style.grid_column.end = span((*v).max(1) as u16);
        }
        (Prop::AttachedCanvasLeft, PropValue::F64(v)) => {
            node.style.position = Position::Absolute;
            node.style.inset.left = length(*v as f32);
        }
        (Prop::AttachedCanvasTop, PropValue::F64(v)) => {
            node.style.position = Position::Absolute;
            node.style.inset.top = length(*v as f32);
        }
        (Prop::AttachedCanvasZIndex, PropValue::I32(v)) => {
            node.z_index = *v;
            node.z_dirty = true;
        }

        // ── Control state (stateful drawn controls) ──────────────────
        // The part-converted kinds (toggle / slider / segmented / nav) need
        // no spring tick here: marking dirty routes them through the paint
        // pass, whose parts sync glides the change on the compositor.
        (Prop::IsOn, PropValue::Bool(v)) => {
            node.ctrl_mut().is_on = *v;
            node.mark_dirty();
        }
        (Prop::IsChecked, PropValue::Bool(v)) => {
            // The CheckBox reveal fades via its chrome parts on repaint; a
            // ToggleButton's checked state is plain painted chrome.
            node.ctrl_mut().is_checked = *v;
            node.mark_dirty();
        }
        (Prop::Value, PropValue::F64(v)) if node.pressed => {
            // While the user is driving this control, input owns its value.
            // A write arriving now is the app echoing a value the gesture has
            // already moved past, and applying it would drag the chrome
            // backwards under the user's finger — the same reason the editor
            // ignores `Prop::Value` while its buffer is focused and seeded.
            //
            // This is the gesture-time half of the gate. The post-release
            // window is closed by the revision half (`Node::value_rev` +
            // `DCompBackend::set_value_stamped`): every recorded value write
            // arrives through `Cmd::SetValue` stamped with the revision the
            // app was based on, and a stale one is dropped before reaching
            // this match at all.
            let _ = v;
        }
        (Prop::Value, PropValue::F64(v)) => {
            node.ctrl_mut().value = *v;
            // NumberBox: reflect the programmatic value as formatted text
            // (unless the user is mid-edit — the editor owns the buffer
            // while focused).
            if node.kind == ControlKind::NumberBox {
                seed_number_text(node, *v);
            }
            node.mark_dirty();
        }
        (Prop::Minimum, PropValue::F64(v)) => {
            node.ctrl_mut().min = *v;
            node.mark_dirty();
        }
        (Prop::Maximum, PropValue::F64(v)) => {
            node.ctrl_mut().max = *v;
            node.mark_dirty();
        }
        (Prop::Step, PropValue::F64(v)) => node.ctrl_mut().step = Some(*v),
        (Prop::FillOrigin, PropValue::F64(v)) => {
            node.ctrl_mut().fill_origin = Some(*v);
            node.mark_dirty();
        }
        (Prop::FillColor, PropValue::Color(c)) => {
            node.ctrl_mut().fill_color = Some(*c);
            node.mark_dirty();
        }
        (Prop::FillColorAlt, PropValue::Color(c)) => {
            node.ctrl_mut().fill_color_alt = Some(*c);
            node.mark_dirty();
        }
        (Prop::Marker, PropValue::F64(v)) => {
            node.ctrl_mut().marker = Some(*v);
            node.mark_dirty();
        }
        (Prop::MarkerColor, PropValue::Color(c)) => {
            node.ctrl_mut().marker_color = Some(*c);
            node.mark_dirty();
        }
        (Prop::GradientStops, PropValue::GradientStops(stops)) => {
            node.ctrl_mut().stops = stops.clone();
            node.mark_dirty();
        }
        (Prop::StartAngle, PropValue::F64(v)) => {
            node.ctrl_mut().start_angle = *v as f32;
            node.mark_dirty();
        }
        (Prop::EndAngle, PropValue::F64(v)) => {
            node.ctrl_mut().end_angle = *v as f32;
            node.mark_dirty();
        }
        (Prop::Ticks, PropValue::F64List(list)) => {
            node.ctrl_mut().ticks = list.clone();
            node.mark_dirty();
        }
        (Prop::TickLabels, PropValue::ValueLabels(list)) => {
            node.ctrl_mut().tick_labels = list.clone();
            node.mark_dirty();
        }
        (Prop::MajorEvery, PropValue::F64(v)) => {
            node.ctrl_mut().major_every = Some(*v);
            node.mark_dirty();
        }
        (Prop::Accent, PropValue::Color(c)) => {
            node.ctrl_mut().accent = Some(*c);
            node.mark_dirty();
        }
        (Prop::Unit, PropValue::Str(s)) => {
            node.ctrl_mut().unit = s.clone();
            node.mark_dirty();
        }
        (Prop::SubText, PropValue::Str(s)) => {
            node.ctrl_mut().sub_text = s.clone();
            node.mark_dirty();
        }
        // An InfoBadge's count. The `Value` prop is `F64` everywhere else (it
        // is the range controls' slot), so without this arm the badge's write
        // fell through to the terminal `_` and was dropped — as a *reported*
        // defect, since `Value` classifies as consumed.
        (Prop::Value, PropValue::I32(v)) => {
            node.ctrl_mut().badge_value = Some(*v);
            // The count is the drawn label; a new one has to be re-laid-out
            // before the badge can measure to it.
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::Message, PropValue::Str(s)) => {
            node.extras_mut().message = s.clone();
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::Severity, PropValue::I32(v)) => {
            node.extras_mut().severity = *v;
            node.mark_dirty();
        }
        (Prop::IsOpen, PropValue::Bool(v)) => {
            node.extras_mut().bar_open = *v;
            // The flip changes whether the band occupies layout at all
            // (`layout::finalize_style`), which the reconcile's own relayout
            // picks up — the style comparison sees the new `Display`.
            node.mark_dirty();
        }
        (Prop::IsClosable, PropValue::Bool(v)) => {
            node.extras_mut().bar_closable = *v;
            // The close button's column is part of the text budget, so losing
            // or gaining it re-wraps the paragraph.
            node.measure_dirty = true;
            node.mark_dirty();
        }
        (Prop::IsIndeterminate, PropValue::Bool(v)) => {
            node.ctrl_mut().indeterminate = *v;
            node.mark_dirty();
        }
        (Prop::IsActive, PropValue::Bool(v)) => {
            node.ctrl_mut().is_active = *v;
            node.mark_dirty();
        }
        (Prop::IsExpanded, PropValue::Bool(v)) => {
            node.ctrl_mut().expanded = *v;
            node.mark_dirty();
        }
        (Prop::SelectedIndex, PropValue::I32(v)) => {
            node.ctrl_mut().selected_index = *v;
            node.mark_dirty();
        }
        (Prop::SelectedTag, PropValue::Str(s)) => {
            node.ctrl_mut().selected_tag = Some(s.clone());
            sync_selected_tag(node);
            node.mark_dirty();
        }
        (Prop::PlaceholderText, PropValue::Str(s)) => {
            node.ctrl_mut().placeholder = s.clone();
            node.mark_dirty();
        }
        (Prop::Items, PropValue::StrList(list)) => {
            node.ctrl_mut().items = list.clone();
            node.mark_dirty();
            // A focused AutoSuggestBox whose filtered list just changed refreshes
            // its open dropdown in place — reported back to the caller, which
            // holds the backend this needs and no longer holds the node borrow.
            refresh_suggest = node.kind == ControlKind::AutoSuggestBox;
        }
        (Prop::Items, PropValue::SelectorBarItems(items)) => {
            node.ctrl_mut().items = items.iter().map(|i| i.text.clone()).collect();
            if node.ctrl().selected_index < 0 && !node.ctrl().items.is_empty() {
                node.ctrl_mut().selected_index = 0;
            }
            // Labels feed the per-item width measure — rebuild it.
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::MenuItems, PropValue::NavMenuItems(items)) => {
            let ctrl = node.ctrl_mut();
            ctrl.items.clear();
            ctrl.tags.clear();
            ctrl.icons.clear();
            for it in items {
                if it.is_header {
                    continue;
                }
                ctrl.items.push(it.content.clone());
                ctrl.tags
                    .push(it.tag.clone().unwrap_or_else(|| it.content.clone()));
                ctrl.icons.push(it.icon.map(|s| s.0 as u32).unwrap_or(0));
            }
            sync_selected_tag(node);
            node.mark_dirty();
        }
        (Prop::MenuFlyoutItems, PropValue::MenuFlyoutItems(items)) => {
            node.ctrl_mut().menu = items.iter().map(menu_row).collect();
            node.mark_dirty();
        }

        // ── Chrome state: stored, not yet drawn ──────────────────────
        // Everything below lands in `Extras` and nothing reads it yet. It is
        // stored anyway because the alternative is what this arm block
        // replaced: a silent drop at the consumer, where the value is gone by
        // the time anyone writes the drawing. The state is now exact and the
        // gap is purely the paint — see [`Status::Stored`].
        (Prop::Title, PropValue::Str(s)) => {
            node.extras_mut().title = s.clone();
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::Subtitle, PropValue::Str(s)) => {
            node.extras_mut().subtitle = s.clone();
            node.text_dirty = true;
            node.mark_dirty();
        }
        // These three change the band's drawn geometry (its height, and the
        // leading inset its back button occupies), so each re-derives the
        // caption metrics rather than waiting on a text rebuild it does not
        // need — `tall` and back-button visibility do not touch the titles.
        (Prop::Tall, PropValue::Bool(v)) => {
            node.extras_mut().tall = *v;
            layout::apply_caption_metrics(node);
        }
        (Prop::IsBackButtonVisible, PropValue::Bool(v)) => {
            node.extras_mut().back_button_visible = *v;
            layout::apply_caption_metrics(node);
        }
        (Prop::IsBackButtonEnabled, PropValue::Bool(v)) => {
            node.extras_mut().back_button_enabled = *v;
            node.mark_dirty();
        }
        // The nav pane's toggle and back arrow occupy its chrome row, and
        // whether that row exists at all shifts every item below it — so these
        // move geometry, not just pixels.
        (Prop::IsPaneToggleButtonVisible, PropValue::Bool(v)) => {
            node.extras_mut().pane_toggle_visible = *v;
            node.mark_dirty();
        }
        (Prop::IsBackEnabled, PropValue::Bool(v)) => {
            node.extras_mut().back_enabled = *v;
            node.mark_dirty();
        }
        (Prop::IsSettingsVisible, PropValue::Bool(v)) => {
            node.extras_mut().settings_visible = *v;
            node.mark_dirty();
        }
        // The three that change the pane's WIDTH re-derive the layout inset the
        // content child sits behind — the same call the layout pass makes, so a
        // set and an unset cannot fall out of step (see `apply_nav_metrics`).
        (Prop::IsPaneOpen, PropValue::Bool(v)) => {
            node.extras_mut().pane_open = *v;
            layout::apply_nav_metrics(node);
        }
        (Prop::PaneTitle, PropValue::Str(s)) => {
            node.extras_mut().pane_title = s.clone();
            // A header row appears/disappears with the string, so this
            // re-measures as well as repaints; the text pass re-derives the
            // metrics once it knows whether there is a title to lay out.
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::PaneDisplayMode, PropValue::I32(v)) => {
            node.extras_mut().pane_display_mode = *v;
            layout::apply_nav_metrics(node);
        }
        (Prop::OpenPaneLength, PropValue::F64(v)) => {
            node.extras_mut().open_pane_length = *v;
            layout::apply_nav_metrics(node);
        }
        (Prop::AutoSuggestBox, PropValue::Bool(v)) => {
            node.extras_mut().search_box = *v;
            node.mark_dirty();
        }
        (Prop::AutoSuggestItems, PropValue::StrList(list)) => {
            node.extras_mut().suggest_items = list.clone();
            node.mark_dirty();
        }
        (Prop::AutoSuggestPlaceholder, PropValue::Str(s)) => {
            node.extras_mut().suggest_placeholder = s.clone();
            node.mark_dirty();
        }
        (Prop::HorizontalScrollBarVisibility, PropValue::I32(v)) => {
            node.extras_mut().h_scrollbar = *v;
            node.mark_dirty();
        }
        (Prop::VerticalScrollBarVisibility, PropValue::I32(v)) => {
            node.extras_mut().v_scrollbar = *v;
            node.mark_dirty();
        }
        // A plain-text flyout is a `FlyoutDef` with only its text set — the
        // seam's own constructor for that case, so the two shapes converge
        // with nothing dropped and no second field to keep in step.
        (Prop::FlyoutContent, PropValue::Str(s)) => {
            node.extras_mut().flyout = Some(Box::new(crate::FlyoutDef::text(s.clone())));
            node.mark_dirty();
        }
        (Prop::FlyoutContent, PropValue::FlyoutDef(def)) => {
            node.extras_mut().flyout = Some(Box::new(def.clone()));
            node.mark_dirty();
        }
        (Prop::FlyoutPlacement, PropValue::I32(v)) => {
            node.extras_mut().flyout_placement = *v;
            node.mark_dirty();
        }
        // A `Symbol`'s codepoint, 0 = none — the encoding `Ctrl::icons` and
        // `MenuRow::icon` already carry glyph icons in.
        (Prop::Icon, PropValue::I32(v)) => {
            node.extras_mut().icon = *v as u32;
            // The icon widens the button, and gaining/losing one is also what
            // decides whether a label-less button has a layout at all — so
            // this has to re-measure, not just repaint.
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::NavigateUri, PropValue::Str(s)) => {
            node.extras_mut().navigate_uri = s.clone();
        }
        (Prop::OnContent, PropValue::Str(s)) => {
            node.extras_mut().on_content = s.clone();
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::OffContent, PropValue::Str(s)) => {
            node.extras_mut().off_content = s.clone();
            node.text_dirty = true;
            node.mark_dirty();
        }
        (Prop::IsEditable, PropValue::Bool(v)) => {
            node.extras_mut().is_editable = *v;
            node.mark_dirty();
        }
        (Prop::AcceptsReturn, PropValue::Bool(v)) => {
            node.extras_mut().accepts_return = *v;
        }
        (Prop::PasswordRevealMode, PropValue::I32(v)) => {
            node.extras_mut().password_reveal_mode = *v;
            node.mark_dirty();
        }
        (Prop::IsPasswordRevealButtonEnabled, PropValue::Bool(v)) => {
            node.extras_mut().password_reveal_button = *v;
            node.mark_dirty();
        }
        (Prop::IsTextSelectionEnabled, PropValue::Bool(v)) => {
            node.extras_mut().text_selectable = *v;
        }
        (Prop::Delay, PropValue::I32(v)) => node.extras_mut().repeat_delay = *v,
        (Prop::Interval, PropValue::I32(v)) => node.extras_mut().repeat_interval = *v,

        // Every pair this backend does not consume lands here and is
        // dropped. In a debug build say so — but only when the drop is a
        // defect (see [`unhandled`]); in release this compiles to nothing.
        _ => unhandled::note(node.kind, prop, value),
    }
    refresh_suggest
}

/// What this backend does with a [`Prop`] — the classification behind both the
/// dropped-prop diagnostic ([`unhandled`]) and the reset table below.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub(crate) enum Status {
    /// Implemented end to end: [`apply_prop`] stores it and something reads
    /// what it stored. Reaching the terminal `_` arm with one of these means
    /// the value arrived in a shape no arm accepts (`Width` as an `I32`, a
    /// `Value(Str)` on a node with no editor) — always a defect, because the
    /// write was addressed to a feature that exists here and was dropped
    /// anyway.
    Consumed,
    /// Stored in node state, correctly and completely — but nothing draws or
    /// acts on it yet. Distinct from [`Self::Consumed`] because the gap is
    /// real (the app sees nothing) and distinct from the old "unimplemented"
    /// because the value is no longer LOST: whoever writes the drawing finds
    /// the state already there and already reset correctly.
    Stored,
    /// Every control kind that can carry this prop is one this backend does
    /// not render. Ignoring it is the design, not a gap — reported never.
    NotApplicable,
}

/// The per-prop contract, stated once: how this backend classifies a [`Prop`],
/// and — for every prop it stores — how an [`Unset`](PropValue::Unset) undoes
/// it.
///
/// The two used to be separate matches, and they disagreed. `set_prop` grew a
/// classifier covering all 165 props while `reset_prop` kept a terminal `_` and
/// covered 24 of the 75 it claimed to consume, so an `Unset` for any of the
/// other 51 was silently ignored and the node kept a stale value — the exact
/// defect that had already been found and fixed once on the `set_prop` side.
/// Nothing structural stopped it recurring, because nothing tied a
/// classification to the existence of a reset.
///
/// This macro is that tie. A prop's status and its reset are ONE entry, so
/// classifying a prop as stored-here without saying how to unstore it is not
/// an omission anyone can make — it is a syntax error. Both generated matches
/// are exhaustive with no `_` arm, so a [`Prop`] added to the seam must be
/// placed in one of the three sections before this compiles at all, and a prop
/// named twice trips `unreachable_patterns`.
///
/// Reset targets are DERIVED, never remembered: [`Node::birth_paint`] and
/// [`Node::birth_style`] are the same functions [`Node::new`] builds a node
/// with, and `Ctrl::DEFAULT` / `Extras::DEFAULT` are the same constants the
/// absent-read path returns. Several of those birth values are deliberately
/// not zero — `max` is 100, `is_active` is true, `selected_index` and
/// `content_align` are -1, a Button is born with a 6 DIP corner radius, 12/6
/// padding and a 30 DIP minimum height — so a reset written from memory as
/// "set it to zero" is a new bug, not a fix. Writing `Ctrl::DEFAULT.max`
/// cannot be that bug.
macro_rules! prop_contract {
    (
        $(#[$cm:meta])*
        consumed { $($($cp:ident)|+ => |$cn:ident| $cb:block)* }
        $(#[$sm:meta])*
        stored { $($($sp:ident)|+ => |$sn:ident| $sb:block)* }
        $(#[$nm:meta])*
        not_applicable { $($($np:ident),+ ;)* }
    ) => {
        /// Classify a prop — see [`prop_contract`].
        #[cfg_attr(not(debug_assertions), allow(dead_code))]
        pub(crate) fn prop_status(prop: Prop) -> Status {
            match prop {
                $($(Prop::$cp)|+ => Status::Consumed,)*
                $($(Prop::$sp)|+ => Status::Stored,)*
                $($(Prop::$np)|+ => Status::NotApplicable,)*
            }
        }

        /// Revert a prop to the value a node that never received it holds,
        /// when the reconciler diffs it away (`PropValue::Unset`).
        ///
        /// Every prop this backend stores has an arm here — see
        /// [`prop_contract`], which generates both this and [`prop_status`]
        /// from one list so the two cannot drift apart again.
        fn reset_prop(node: &mut Node, prop: Prop) {
            #[allow(unused_imports)]
            use taffy::prelude::*;
            match prop {
                $($(Prop::$cp)|+ => { let $cn = &mut *node; $cb })*
                $($(Prop::$sp)|+ => { let $sn = &mut *node; $sb })*
                // Nothing was stored, so there is nothing to restore.
                $($(Prop::$np)|+ => {})*
            }
        }
    };
}

prop_contract! {
    /// Implemented end to end by [`apply_prop`].
    consumed {
        // ── Paint ────────────────────────────────────────────────────────
        Background => |n| { n.paint.background = None; n.mark_dirty(); }
        Foreground => |n| { n.paint.foreground = None; n.mark_dirty(); }
        BorderBrush => |n| { n.paint.border_brush = None; n.mark_dirty(); }
        BorderThickness => |n| {
            n.paint.border_thickness = n.birth_paint().border_thickness;
            n.style.border = n.birth_style().border;
            n.mark_dirty();
        }
        // NOT zero for a Button, which is born with a 6 DIP radius.
        CornerRadius => |n| {
            n.paint.corner_radius = n.birth_paint().corner_radius;
            n.mark_dirty();
        }
        Fill => |n| { n.paint.fill = None; n.mark_dirty(); }
        Stroke => |n| { n.paint.stroke = None; n.mark_dirty(); }
        StrokeThickness => |n| {
            n.paint.stroke_thickness = n.birth_paint().stroke_thickness;
            n.mark_dirty();
        }
        LineEndpoints => |n| { n.paint.line = n.birth_paint().line; n.mark_dirty(); }
        StyleVariant => |n| {
            n.paint.style_variant = n.birth_paint().style_variant;
            n.mark_dirty();
        }
        // Born ENABLED — resetting this to `false` greys the control out.
        IsEnabled => |n| { n.paint.is_enabled = n.birth_paint().is_enabled; n.mark_dirty(); }
        Opacity => |n| { let _ = n.vis.SetOpacity(1.0); }

        // ── Text ─────────────────────────────────────────────────────────
        // Mirrors the set: an editable kind carries its text in the editor
        // buffer, everything else in the paint label.
        Content | Text | Header => |n| {
            if n.editor.is_some() {
                clear_editor_text(n);
            } else {
                n.paint.text = n.birth_paint().text;
            }
            n.text_dirty = true;
            n.mark_dirty();
        }
        // Born at a real size and weight: zeroing either makes the text
        // invisible rather than default.
        FontSize => |n| {
            n.paint.font_size = n.birth_paint().font_size;
            n.text_dirty = true;
            n.mark_dirty();
        }
        FontWeight => |n| {
            n.paint.font_weight = n.birth_paint().font_weight;
            n.text_dirty = true;
            n.mark_dirty();
        }
        FontFamily => |n| { n.paint.font_family = None; n.text_dirty = true; n.mark_dirty(); }
        TextWrapping | TextWrappingWrap => |n| {
            n.paint.wrap = n.birth_paint().wrap;
            n.text_dirty = true;
            n.mark_dirty();
        }

        // ── Layout ───────────────────────────────────────────────────────
        // Not `Rect::zero()`: a Button is born with 12/6 padding, an Expander
        // and a TitleBar with the inset their drawn header occupies, and a
        // NavigationView with the icon rail's width on the left. Zeroing that
        // paints the chrome over the content.
        Padding => |n| { n.style.padding = n.birth_style().padding; }
        Margin => |n| { n.style.margin = n.birth_style().margin; }
        Width => |n| { n.style.size.width = n.birth_style().size.width; }
        Height => |n| { n.style.size.height = n.birth_style().size.height; }
        // Likewise not `auto()`: the intrinsic minimum is the only thing
        // keeping a childless drawn control (ToggleSwitch, Slider, Knob,
        // Meter, CheckBox) from measuring 0 and vanishing in a flex row.
        MinWidth => |n| { n.style.min_size.width = n.birth_style().min_size.width; }
        MinHeight => |n| { n.style.min_size.height = n.birth_style().min_size.height; }
        MaxWidth => |n| { n.style.max_size.width = n.birth_style().max_size.width; }
        MaxHeight => |n| { n.style.max_size.height = n.birth_style().max_size.height; }
        HorizontalAlignment => |n| { n.h_align = node::ALIGN_UNSET; }
        VerticalAlignment => |n| { n.v_align = node::ALIGN_UNSET; }
        Orientation => |n| {
            n.style.flex_direction = n.birth_style().flex_direction;
            apply_stack_gap(n);
        }
        Spacing => |n| { n.spacing = 0.0; apply_stack_gap(n); }
        ColumnSpacing => |n| { n.style.gap.width = n.birth_style().gap.width; }
        RowSpacing => |n| { n.style.gap.height = n.birth_style().gap.height; }
        GridRows => |n| { n.grid_rows.clear(); }
        GridColumns => |n| { n.grid_cols.clear(); }
        // XAML Grid parity: an unplaced child belongs to cell (0, 0), which is
        // Taffy line 1 — not `auto`, which would auto-flow it into the next
        // free cell and un-overlap a deliberately overlapping pair.
        AttachedGridRow => |n| { n.style.grid_row.start = n.birth_style().grid_row.start; }
        AttachedGridColumn => |n| {
            n.style.grid_column.start = n.birth_style().grid_column.start;
        }
        AttachedGridRowSpan => |n| { n.style.grid_row.end = n.birth_style().grid_row.end; }
        AttachedGridColumnSpan => |n| {
            n.style.grid_column.end = n.birth_style().grid_column.end;
        }
        // Setting either inset made the node absolute, so clearing one only
        // returns it to flow once the other is gone too.
        AttachedCanvasLeft => |n| {
            let birth = n.birth_style();
            n.style.inset.left = birth.inset.left;
            if n.style.inset.top == birth.inset.top {
                n.style.position = birth.position;
            }
        }
        AttachedCanvasTop => |n| {
            let birth = n.birth_style();
            n.style.inset.top = birth.inset.top;
            if n.style.inset.left == birth.inset.left {
                n.style.position = birth.position;
            }
        }
        AttachedCanvasZIndex => |n| { n.z_index = 0; n.z_dirty = true; }

        // ── Control state ────────────────────────────────────────────────
        // `Ctrl::DEFAULT` is the same constant an unallocated `Ctrl` reads as,
        // and several of its fields are pointedly not zero.
        IsOn => |n| { n.ctrl_reset(|c| c.is_on = Ctrl::DEFAULT.is_on); }
        IsChecked => |n| { n.ctrl_reset(|c| c.is_checked = Ctrl::DEFAULT.is_checked); }
        // One prop, three carriers: the range controls' number, an editor's
        // buffer, and an InfoBadge's count. All three restore here, because a
        // node is one kind and only its own arm can have stored anything.
        Value => |n| {
            n.ctrl_reset(|c| c.value = Ctrl::DEFAULT.value);
            // An editor's buffer is the value for the text kinds; a node born
            // without one shows an empty, UNSEEDED field.
            clear_editor_text(n);
            // Back to the bare status dot — `InfoBadge::dot()` is the no-value
            // form, which is what an `Unset` on a badge means.
            n.ctrl_reset(|c| c.badge_value = Ctrl::DEFAULT.badge_value);
            n.text_dirty = true;
            n.mark_dirty();
        }
        Minimum => |n| { n.ctrl_reset(|c| c.min = Ctrl::DEFAULT.min); }
        // 100, not 0 — a zero-span range makes every fill and thumb NaN.
        Maximum => |n| { n.ctrl_reset(|c| c.max = Ctrl::DEFAULT.max); }
        Step => |n| { n.ctrl_reset(|c| c.step = Ctrl::DEFAULT.step); }
        LargeChange => |n| { n.ctrl_reset(|c| c.large_change = Ctrl::DEFAULT.large_change); }
        // Mirrors the set arm: the seeded text is re-formatted at the restored
        // precision, and never while focused (the user owns the buffer).
        //
        // Only text that was ACTUALLY seeded from a value prop is re-formatted.
        // The set arm can reformat unconditionally because a `Precision` write
        // implies a value; a reset carries no such implication, and a field
        // that never received one must stay empty rather than acquire a "0.00"
        // from nowhere.
        Precision => |n| {
            n.ctrl_reset(|c| c.precision = Ctrl::DEFAULT.precision);
            if n.kind == ControlKind::NumberBox
                && !n.focused
                && n.editor.as_ref().is_some_and(|ed| ed.seeded)
            {
                let value = n.ctrl().value;
                if let Some(ed) = &mut n.editor {
                    ed.seeded = false;
                }
                seed_number_text(n, value);
                n.mark_dirty();
            }
        }
        // -1 (unset), not 0 — 0 is a real WinRT `Left`.
        HorizontalContentAlignment => |n| {
            n.ctrl_reset(|c| c.content_align = Ctrl::DEFAULT.content_align);
            if let Some(ed) = &mut n.editor {
                ed.layout_dirty = true;
            }
            n.mark_dirty();
        }
        FillOrigin => |n| { n.ctrl_reset(|c| c.fill_origin = Ctrl::DEFAULT.fill_origin); }
        FillColor => |n| { n.ctrl_reset(|c| c.fill_color = Ctrl::DEFAULT.fill_color); }
        FillColorAlt => |n| {
            n.ctrl_reset(|c| c.fill_color_alt = Ctrl::DEFAULT.fill_color_alt);
        }
        Marker => |n| { n.ctrl_reset(|c| c.marker = Ctrl::DEFAULT.marker); }
        MarkerColor => |n| { n.ctrl_reset(|c| c.marker_color = Ctrl::DEFAULT.marker_color); }
        GradientStops => |n| { n.ctrl_reset(|c| c.stops = Ctrl::DEFAULT.stops); }
        StartAngle => |n| { n.ctrl_reset(|c| c.start_angle = Ctrl::DEFAULT.start_angle); }
        EndAngle => |n| { n.ctrl_reset(|c| c.end_angle = Ctrl::DEFAULT.end_angle); }
        Ticks => |n| { n.ctrl_reset(|c| c.ticks = Ctrl::DEFAULT.ticks); }
        TickLabels => |n| { n.ctrl_reset(|c| c.tick_labels = Ctrl::DEFAULT.tick_labels); }
        MajorEvery => |n| { n.ctrl_reset(|c| c.major_every = Ctrl::DEFAULT.major_every); }
        Accent => |n| { n.ctrl_reset(|c| c.accent = Ctrl::DEFAULT.accent); }
        Unit => |n| { n.ctrl_reset(|c| c.unit = Ctrl::DEFAULT.unit); }
        SubText => |n| { n.ctrl_reset(|c| c.sub_text = Ctrl::DEFAULT.sub_text); }
        IsIndeterminate => |n| {
            n.ctrl_reset(|c| c.indeterminate = Ctrl::DEFAULT.indeterminate);
        }
        // Born ACTIVE — a false here dims every meter that loses the prop.
        IsActive => |n| { n.ctrl_reset(|c| c.is_active = Ctrl::DEFAULT.is_active); }
        IsExpanded => |n| { n.ctrl_reset(|c| c.expanded = Ctrl::DEFAULT.expanded); }
        // -1 ("nothing selected"), not 0 ("the first item").
        SelectedIndex => |n| {
            n.ctrl_reset(|c| c.selected_index = Ctrl::DEFAULT.selected_index);
        }
        SelectedTag => |n| { n.ctrl_reset(|c| c.selected_tag = Ctrl::DEFAULT.selected_tag); }
        PlaceholderText => |n| { n.ctrl_reset(|c| c.placeholder = Ctrl::DEFAULT.placeholder); }
        // The measured per-segment label widths are derived from `items`, so
        // they go with them and the text pass re-measures.
        Items => |n| {
            n.ctrl_reset(|c| {
                c.items = Ctrl::DEFAULT.items;
                c.seg_label_w = Ctrl::DEFAULT.seg_label_w;
            });
            n.text_dirty = true;
        }
        MenuItems => |n| {
            n.ctrl_reset(|c| {
                c.items = Ctrl::DEFAULT.items;
                c.tags = Ctrl::DEFAULT.tags;
                c.icons = Ctrl::DEFAULT.icons;
            });
            n.text_dirty = true;
        }
        MenuFlyoutItems => |n| { n.ctrl_reset(|c| c.menu = Ctrl::DEFAULT.menu); }

        // ── Overlay scrollbar policy ─────────────────────────────────────
        // Reverting to `Auto` restores the hover-driven reveal; the next
        // paint of the container re-resolves the policy either way, so no
        // explicit re-derive is needed here.
        VerticalScrollBarVisibility => |n| {
            n.extras_reset(|x| x.v_scrollbar = Extras::DEFAULT.v_scrollbar);
            n.mark_dirty();
        }

        // ── Button leading icon ──────────────────────────────────────────
        // Re-measures as well as repaints: losing the icon narrows the button,
        // and on a label-less one it also removes the only reason it has a
        // text layout at all.
        Icon => |n| {
            n.extras_reset(|x| x.icon = Extras::DEFAULT.icon);
            n.text_dirty = true;
        }

        // ── HyperlinkButton target ───────────────────────────────────────
        // Consumed at activation, not at paint: `input::activate` offers it to
        // the app's [`crate::set_uri_launcher`] hook — the one path a pointer
        // release, a Space/Enter press and a UIA `Invoke` all share. This
        // backend still makes no policy decision about the string; with no
        // launcher installed the link is inert, which is the default. Purely a
        // stored value as far as rendering is concerned, so the reset neither
        // re-measures nor repaints.
        NavigateUri => |n| { n.extras_reset(|x| x.navigate_uri = Extras::DEFAULT.navigate_uri); }

        // ── ToggleSwitch state labels ────────────────────────────────────
        // The switch measures to the WIDER of the two, so dropping either one
        // re-measures — hence `text_dirty` on both.
        OnContent => |n| {
            n.extras_reset(|x| x.on_content = Extras::DEFAULT.on_content);
            n.text_dirty = true;
        }
        OffContent => |n| {
            n.extras_reset(|x| x.off_content = Extras::DEFAULT.off_content);
            n.text_dirty = true;
        }

        // ── TitleBar caption band ────────────────────────────────────────
        // The titles are drawn from cached layouts the text pass owns, so a
        // reset drops the state and re-flags it; the pass then rebuilds (to
        // `None`, here) and re-derives the band's leading inset with it.
        // Shared by the caption band and the InfoBar — one `Extras` field, so
        // one reset (see `Extras::title`).
        Title => |n| { n.extras_reset(|x| x.title = Extras::DEFAULT.title); n.text_dirty = true; }
        Subtitle => |n| {
            n.extras_reset(|x| x.subtitle = Extras::DEFAULT.subtitle);
            n.text_dirty = true;
        }

        // ── InfoBar ──────────────────────────────────────────────────────
        Message => |n| {
            n.extras_reset(|x| x.message = Extras::DEFAULT.message);
            n.text_dirty = true;
        }
        Severity => |n| { n.extras_reset(|x| x.severity = Extras::DEFAULT.severity); }
        // Born CLOSED: `InfoBar::default()` is `is_open: false` — it is
        // `InfoBar::new` that opens one — so an unset bar is a dismissed bar,
        // and dismissed means out of layout, not merely unpainted.
        IsOpen => |n| { n.extras_reset(|x| x.bar_open = Extras::DEFAULT.bar_open); }
        // Born CLOSABLE, matching `InfoBar::default()`. The close button's
        // column comes out of the text budget, so restoring it re-wraps.
        IsClosable => |n| {
            n.extras_reset(|x| x.bar_closable = Extras::DEFAULT.bar_closable);
            n.measure_dirty = true;
        }

        // Band height and back-button inset are derived geometry, so each of
        // these restores the state and immediately re-derives — the same call
        // `apply_prop` makes, so set and unset cannot fall out of step.
        Tall => |n| {
            n.extras_reset(|x| x.tall = Extras::DEFAULT.tall);
            layout::apply_caption_metrics(n);
        }
        IsBackButtonEnabled => |n| {
            n.extras_reset(|x| x.back_button_enabled = Extras::DEFAULT.back_button_enabled);
            n.mark_dirty();
        }
        // Born VISIBLE: a NavigationView only emits this prop to say `false`,
        // so its removal means "show it again".
        //
        // Carried by BOTH drawn bands — it is the caption's back button and the
        // nav pane's alike — so the reset re-derives both geometries. Each is a
        // no-op on the kind it does not belong to.
        IsBackButtonVisible => |n| {
            n.extras_reset(|x| x.back_button_visible = Extras::DEFAULT.back_button_visible);
            layout::apply_caption_metrics(n);
            layout::apply_nav_metrics(n);
        }

        // ── NavigationView pane ──────────────────────────────────────────
        // Pane width, and everything that follows from it, is DERIVED — see
        // `nav::pane_width`, which `birth_style` also builds a virgin
        // NavigationView from. So each of these restores the state and then
        // re-derives with the same call `apply_prop` makes, and a node whose
        // pane state is back at its defaults is indistinguishable from one that
        // never received the prop, style included.
        //
        // Born TOGGLE-VISIBLE and SETTINGS-VISIBLE: like the back button, a
        // NavigationView only emits these to say `false`, so losing the binding
        // means "show it again".
        IsPaneToggleButtonVisible => |n| {
            n.extras_reset(|x| x.pane_toggle_visible = Extras::DEFAULT.pane_toggle_visible);
            layout::apply_nav_metrics(n);
        }
        IsBackEnabled => |n| { n.extras_reset(|x| x.back_enabled = Extras::DEFAULT.back_enabled); }
        IsSettingsVisible => |n| {
            n.extras_reset(|x| x.settings_visible = Extras::DEFAULT.settings_visible);
        }
        // Born OPEN, matching `NavigationView::default()` — so an app that
        // never binds this gets WinUI's own expanded pane, not a rail.
        IsPaneOpen => |n| {
            n.extras_reset(|x| x.pane_open = Extras::DEFAULT.pane_open);
            layout::apply_nav_metrics(n);
        }
        // The header is drawn from a cached layout the text pass owns, so a
        // reset drops the state and re-flags it; the pass then rebuilds (to
        // `None`, here) and re-derives the pane metrics with it.
        PaneTitle => |n| {
            n.extras_reset(|x| x.pane_title = Extras::DEFAULT.pane_title);
            n.text_dirty = true;
        }
        // `Auto`, not `Left`: the unset state is the ADAPTIVE mode, which is
        // what a NavigationView with no `pane_display_mode` binding means.
        PaneDisplayMode => |n| {
            n.extras_reset(|x| x.pane_display_mode = Extras::DEFAULT.pane_display_mode);
            layout::apply_nav_metrics(n);
        }
        // 320, not 0 — a zero-length pane is not a default, it is an invisible
        // control.
        OpenPaneLength => |n| {
            n.extras_reset(|x| x.open_pane_length = Extras::DEFAULT.open_pane_length);
            layout::apply_nav_metrics(n);
        }
    }

    /// Stored in [`Extras`] but not yet drawn — the state is exact, the paint
    /// is the gap. `Extras::DEFAULT` is the same constant an unallocated
    /// `Extras` reads as, and its non-empty entries mirror the widget default
    /// whose absence sends the `Unset` (see the constant for which, and why).
    stored {
        // ── NavigationView's embedded search box ─────────────────────────
        // Deliberately still stored: this one is blocked on the shape of
        // [`Node`], not on the drawing.
        //
        // Every other pane element is a rectangle plus glyphs — the pane draws
        // them and `nav::hit` resolves them, all inside one node. An editor is
        // not. The editing machinery this backend already has is keyed to a
        // NODE, and in three ways that a sub-element of a non-editor node
        // cannot satisfy:
        //
        // * **Geometry.** `Editor` has no box of its own. `caret_index_at`,
        //   `editor_caret_box` and `paint_editor` all take the field's box to
        //   BE `node.rect` (see `editor::editor_content(kind, rect.w)`). An
        //   editor hosted at some sub-rect of a nav pane would map pointer x to
        //   a caret index across the whole pane and hang its caret sprite off
        //   the pane's top-left corner.
        // * **Focus.** Focus is `Option<ControlId>` — one focusable per node —
        //   and `editor_key` routes EVERY editing key to the focused node's
        //   editor before the generic ring sees it. A nav pane with a search
        //   field needs two focus targets in one node (the field, and the row
        //   ring the arrow keys drive), and the Tab order needs to contain
        //   both. Neither is representable.
        // * **Accessibility.** A synthetic item can be an invokable button or a
        //   selectable row; `pattern_supported` gives `IValueProvider` and the
        //   Text pattern to real editor NODES only, and both read
        //   `node.editor`. A search box exposed as an item would answer
        //   `SetValue` for the whole pane.
        //
        // Making this work needs `Node` to carry editor state as a placed
        // sub-element — an editor with its own rect and its own focus identity,
        // with the ~30 `ControlId`-keyed editor call sites taught to address
        // it. That is a seam change, and forcing it from the paint side would
        // mean a second, parallel editor that looks right and mishandles every
        // caret, IME composition and screen reader that meets it.
        //
        // The state below stays exact, so that change lands as plumbing and
        // nothing else. Note the seam is otherwise ready: the NavigationView
        // handle already carries `QuerySubmitted` / `TextChanged` /
        // `SuggestionChosen`, so only the hosting is missing.
        AutoSuggestBox => |n| { n.extras_reset(|x| x.search_box = Extras::DEFAULT.search_box); }
        AutoSuggestItems => |n| {
            n.extras_reset(|x| x.suggest_items = Extras::DEFAULT.suggest_items);
        }
        AutoSuggestPlaceholder => |n| {
            n.extras_reset(|x| x.suggest_placeholder = Extras::DEFAULT.suggest_placeholder);
        }

        // ── Scroll containers ────────────────────────────────────────────
        // Still stored: this backend scrolls and indicates on ONE axis. There
        // is no horizontal overlay thumb for this policy to govern, so
        // consuming it would mean inventing the scrollbar first — see
        // `scroll::Reveal`, and `VerticalScrollBarVisibility` for the axis
        // that is wired.
        HorizontalScrollBarVisibility => |n| {
            n.extras_reset(|x| x.h_scrollbar = Extras::DEFAULT.h_scrollbar);
        }

        // ── Button / HyperlinkButton ─────────────────────────────────────
        FlyoutContent => |n| { n.extras_reset(|x| x.flyout = Extras::DEFAULT.flyout); }
        FlyoutPlacement => |n| {
            n.extras_reset(|x| x.flyout_placement = Extras::DEFAULT.flyout_placement);
        }
        // ── Editors / text ───────────────────────────────────────────────
        IsEditable => |n| { n.extras_reset(|x| x.is_editable = Extras::DEFAULT.is_editable); }
        AcceptsReturn => |n| {
            n.extras_reset(|x| x.accepts_return = Extras::DEFAULT.accepts_return);
        }
        PasswordRevealMode => |n| {
            n.extras_reset(|x| x.password_reveal_mode = Extras::DEFAULT.password_reveal_mode);
        }
        // Born OFFERED, matching `PasswordBox::default()`.
        IsPasswordRevealButtonEnabled => |n| {
            n.extras_reset(|x| x.password_reveal_button = Extras::DEFAULT.password_reveal_button);
        }
        IsTextSelectionEnabled => |n| {
            n.extras_reset(|x| x.text_selectable = Extras::DEFAULT.text_selectable);
        }

        // ── RepeatButton ─────────────────────────────────────────────────
        // Born at the WinUI repeat timing (500 ms then 33 ms), not 0 — a zero
        // interval is an unbounded repeat, not a default.
        Delay => |n| { n.extras_reset(|x| x.repeat_delay = Extras::DEFAULT.repeat_delay); }
        Interval => |n| {
            n.extras_reset(|x| x.repeat_interval = Extras::DEFAULT.repeat_interval);
        }
    }

    /// Never stored, so never reset: every kind that can carry these is one
    /// this backend does not render (or, for the last group, nothing emits
    /// them at all).
    not_applicable {
        // XAML framework machinery with no counterpart in a self-rendering
        // backend: there is no resource dictionary, no style system, and no
        // XAML drag-drop here.
        Style, Resources, AllowDrop;
        // RelativePanel attached props — this backend has no RelativePanel.
        AlignBottomWithPanel, AlignHCenterWithPanel, AlignLeftWithPanel,
        AlignRightWithPanel, AlignTopWithPanel, AlignVCenterWithPanel;
        // TabView / TabViewItem / Pivot.
        CanReorderTabs, IsAddTabButtonVisible, ItemKey, ItemHeader;
        // ColorPicker.
        ColorValue, IsAlphaEnabled, IsColorChannelTextInputVisible,
        IsColorSliderVisible, IsHexInputVisible;
        // Date / time / calendar pickers.
        ClockIdentifier, MinuteIncrement, DayVisible, MonthVisible, YearVisible,
        IsCalendarOpen, IsTodayHighlighted, IsGroupLabelVisible;
        // ContentDialog.
        PrimaryButtonText, SecondaryButtonText, CloseButtonText,
        IsPrimaryButtonEnabled, IsSecondaryButtonEnabled;
        // TeachingTip. `Message`, `Severity`, `IsOpen` and `IsClosable` moved
        // to `consumed` when the InfoBar gained its chrome; the remainder here
        // are carried only by controls this backend does not render.
        ActionButton, ActionButtonText, CloseButton,
        PreferredPlacement, IsLightDismissEnabled;
        // CommandBar / TreeView / SplitView-only / PersonPicture / Image /
        // Viewbox / RatingControl / RadioButton(s). `CompactPaneLength` is
        // SplitView's alone — the NavigationView bindings never emit it, so
        // it is not part of this backend's nav-pane gap.
        PrimaryCommands, SecondaryCommands, CommandBarFlyoutCommands,
        DefaultLabelPosition, Nodes, SelectionMode, DisplayMode, DisplayName,
        Initials, ImageSource, Stretch, MaxRating, Caption, PlaceholderValue,
        IsReadOnly, GroupName, MaxColumns, CompactPaneLength;
        // Seam vocabulary no widget emits.
        Columns, Rows;
    }
}

/// Return an editor to the empty, unseeded buffer a node is born with. Skipped
/// while the field is focused, for the same reason the seed is: the user owns
/// the buffer mid-edit. No-op for a kind that has no editor.
fn clear_editor_text(node: &mut Node) {
    let focused = node.focused;
    if let Some(ed) = &mut node.editor
        && (!focused || !ed.seeded)
    {
        ed.set_text("");
        ed.seeded = false;
    }
}

/// Debug-only diagnostics for the terminal `_` arm of [`Backend::set_prop`].
///
/// The reconciler seam is one shared vocabulary — ~165 [`Prop`]s × ~24
/// [`PropValue`] shapes — and any single backend implements a slice of it, so
/// most pairs legitimately reach that arm. The recorder already refuses a `_`
/// arm in `SendValue::from_prop` precisely so a new value shape cannot vanish
/// from the wire; before this module the same value could still vanish one
/// layer later, at the actual consumer, with no warning and no counter.
///
/// A blanket warning would be useless: the large majority of fallthroughs are
/// props whose only carriers are controls this backend does not render, and
/// those would drown the handful that are real gaps. So each fallthrough is
/// classified first (see [`Status`], whose table lives in
/// [`prop_contract`](super::prop_contract)) and only defects are reported,
/// once each.
///
/// [`prop_status`](super::prop_status) and [`shape`] are **exhaustive matches
/// with no `_` arm** — the same discipline as the recorder, and for the same
/// reason: a `Prop` or `PropValue` variant added to the seam must not be able
/// to slip in already silently dropped. They stay compiled in release (dead,
/// so no codegen) so that check holds in every configuration; only the
/// reporting is cfg'd out.
mod unhandled {
    // In release nothing calls into here; the matches are kept for their
    // compile-time exhaustiveness check alone.
    #![cfg_attr(not(debug_assertions), allow(dead_code))]

    use super::{prop_status, ControlKind, Prop, PropValue, Status};
    use std::cell::RefCell;
    use std::mem::Discriminant;

    thread_local! {
        /// Pairs already reported. A dropped prop repeats on every reconcile,
        /// so without this one gap would scroll the console. The backend is
        /// single-threaded (UI thread), hence a plain thread-local.
        static SEEN: RefCell<rustc_hash::FxHashSet<(ControlKind, Prop, Discriminant<PropValue>)>> =
            RefCell::new(rustc_hash::FxHashSet::default());
        /// Unrendered kinds already reported (one line per kind, not per prop).
        static SEEN_KINDS: RefCell<rustc_hash::FxHashSet<ControlKind>> =
            RefCell::new(rustc_hash::FxHashSet::default());
    }

    /// Report one dropped `(kind, prop, value)` — if it is worth reporting.
    #[cfg(debug_assertions)]
    pub(super) fn note(kind: ControlKind, prop: Prop, value: &PropValue) {
        // Scoped to the reporting path so the release stub leaves no import
        // behind for either to be unused in.
        use super::animate::warn;
        use std::mem::discriminant;

        // A prop dropped on a kind this backend never renders is not a fact
        // about the prop; the one actionable fact is the kind itself, and it is
        // worth saying exactly once rather than once per prop it carries.
        if !renders(kind) {
            if SEEN_KINDS.with(|s| s.borrow_mut().insert(kind)) {
                warn(format_args!(
                    "{kind:?} is not rendered by the DirectComposition backend — \
                     it lays out as a plain container and its own props are inert"
                ));
            }
            return;
        }
        let status = prop_status(prop);
        if matches!(status, Status::NotApplicable) {
            return;
        }
        if !SEEN.with(|s| s.borrow_mut().insert((kind, prop, discriminant(value)))) {
            return;
        }
        match status {
            Status::Consumed => warn(format_args!(
                "set_prop({kind:?}, {prop:?}, {}): DROPPED — this backend \
                 implements {prop:?}, but no arm accepts that value shape",
                shape(value)
            )),
            // The value SHAPE is still the defect — an arm exists, it just
            // did not match. Worth its own wording because the state it
            // would have written is not drawn yet either, so this one goes
            // unnoticed twice over.
            Status::Stored => warn(format_args!(
                "set_prop({kind:?}, {prop:?}, {}): DROPPED — this backend \
                 stores {prop:?} (though nothing draws it yet), but no arm \
                 accepts that value shape",
                shape(value)
            )),
            Status::NotApplicable => {}
        }
    }

    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub(super) fn note(_kind: ControlKind, _prop: Prop, _value: &PropValue) {}

    /// Whether this backend gives `kind` its own behaviour — drawn chrome, or
    /// (for the panels and host surfaces) a real layout/attachment role. `false`
    /// means the node exists only as a laid-out, unpainted container.
    fn renders(kind: ControlKind) -> bool {
        use ControlKind as K;
        match kind {
            // Panels, host surfaces and shapes.
            K::StackPanel
            | K::Grid
            | K::Canvas
            | K::Border
            | K::ScrollViewer
            | K::ScrollView
            | K::SwapChainPanel
            | K::Rectangle
            | K::Ellipse
            | K::Line
            // Drawn controls.
            | K::TextBlock
            | K::Button
            | K::RepeatButton
            | K::HyperlinkButton
            | K::DropDownButton
            | K::SplitButton
            | K::ToggleButton
            | K::CheckBox
            | K::ToggleSwitch
            | K::Slider
            | K::Knob
            | K::Meter
            | K::ProgressBar
            | K::ProgressRing
            | K::Expander
            | K::TextBox
            | K::PasswordBox
            | K::NumberBox
            | K::AutoSuggestBox
            | K::ComboBox
            | K::SelectorBar
            | K::NavigationView
            | K::InfoBar
            | K::InfoBadge
            | K::TitleBar => true,

            // Not rendered: no drawn chrome and no behaviour of their own.
            K::RadioButton
            | K::RadioButtons
            | K::PersonPicture
            | K::Image
            | K::TabView
            | K::TabViewItem
            | K::Pivot
            | K::PivotItem
            | K::BreadcrumbBar
            | K::RichTextBlock
            | K::RichEditBox
            | K::ListView
            | K::GridView
            | K::ListBox
            | K::FlipView
            | K::TreeView
            | K::ContentDialog
            | K::TeachingTip
            | K::Viewbox
            | K::RatingControl
            | K::ColorPicker
            | K::DatePicker
            | K::TimePicker
            | K::CalendarDatePicker
            | K::CalendarView
            | K::SplitView
            | K::MenuBar
            | K::CommandBar
            | K::RelativePanel
            | K::WebView2 => false,
        }
    }

    /// Short name of a value's shape — `Debug` on the value itself would dump
    /// whole item lists into the log.
    fn shape(value: &PropValue) -> &'static str {
        match value {
            PropValue::Str(_) => "Str",
            PropValue::F64(_) => "F64",
            PropValue::U16(_) => "U16",
            PropValue::Bool(_) => "Bool",
            PropValue::I32(_) => "I32",
            PropValue::Thickness(_) => "Thickness",
            PropValue::Color(_) => "Color",
            PropValue::Unset => "Unset",
            PropValue::GridLengths(_) => "GridLengths",
            PropValue::SurfaceImageSource(_) => "SurfaceImageSource",
            PropValue::VirtualSurfaceImageSource(_) => "VirtualSurfaceImageSource",
            PropValue::LineEndpoints(_) => "LineEndpoints",
            PropValue::NavMenuItems(_) => "NavMenuItems",
            PropValue::StrList(_) => "StrList",
            PropValue::MenuBarItems(_) => "MenuBarItems",
            PropValue::MenuFlyoutItems(_) => "MenuFlyoutItems",
            PropValue::FlyoutDef(_) => "FlyoutDef",
            PropValue::TreeViewNodes(_) => "TreeViewNodes",
            PropValue::CommandBarCommands(_) => "CommandBarCommands",
            PropValue::CommandBarFlyoutDef { .. } => "CommandBarFlyoutDef",
            PropValue::SelectorBarItems(_) => "SelectorBarItems",
            PropValue::Resources(_) => "Resources",
            PropValue::GradientStops(_) => "GradientStops",
            PropValue::F64List(_) => "F64List",
            PropValue::ValueLabels(_) => "ValueLabels",
        }
    }
}

fn clone_lengths(g: &[GridLength]) -> Vec<GridLength> {
    g.to_vec()
}

/// The §7.2 arrival rules for a revision-stamped editor-text write, node
/// half — the REAL body [`DCompBackend::set_text_stamped`] applies (and the
/// headless harness drives). Returns `false` when the node has no editor
/// (the write is not editor text and the caller falls back to the plain
/// prop path). In order: composition guard, echo-identical no-op,
/// stale-revision drop, then apply with caret position-mapping.
pub(crate) fn apply_text_stamped(node: &mut Node, text: &str, based_on: u64) -> bool {
    let Some(ed) = &mut node.editor else {
        return false;
    };
    if ed.comp_len > 0 || ed.text_eq(text) || based_on < ed.text_rev {
        return true;
    }
    ed.apply_program_text(text);
    ed.seeded = true;
    ed.caret_moved = true;
    node.mark_dirty();
    true
}

/// Direct (unstamped) programmatic editor text — the arrival path for a plain
/// `Backend::set_prop` string write, which on the shipping pipeline only a
/// direct caller (a test, a headless harness) can produce: the recorder
/// routes every reconciler-originated write through the revision-stamped
/// [`DCompBackend::set_text_stamped`] instead. Applies with the same caret
/// position-mapping and composition guard, but no revision gate — an
/// unstamped caller has no revision to be stale against.
fn direct_editor_text(node: &mut Node, s: &str) {
    if let Some(ed) = &mut node.editor
        && ed.comp_len == 0
        && !ed.text_eq(s)
    {
        ed.apply_program_text(s);
        ed.seeded = true;
        ed.caret_moved = true;
    }
}

/// Seed a NumberBox editor from a programmatic numeric value, formatted to the
/// configured precision. Skipped while focused (the user owns the buffer).
fn seed_number_text(node: &mut Node, v: f64) {
    let focused = node.focused;
    let precision = node.ctrl().precision;
    if let Some(ed) = &mut node.editor
        && (!focused || !ed.seeded)
    {
        let digits = precision.unwrap_or(2).clamp(0, 12) as usize;
        ed.set_text(&format!("{v:.digits$}"));
        ed.seeded = true;
    }
}

/// Revert a `NumberBox`'s in-progress edit to its last committed value (§7.3
/// Escape-revert): reformat `ctrl().value` — the pre-edit value, since a
/// NumberBox commits only on Enter/blur — back into the buffer and select it,
/// exactly as WinUI does. Unlike [`seed_number_text`] this applies even while
/// focused (Escape is the one place the user asks to discard their own edit),
/// and the caller retains focus. No `ValueChanged` fires: only the discarded
/// text changed, never the committed value. A no-op for a node with no editor.
pub(crate) fn revert_number_text(node: &mut Node) {
    let (min, max, precision, value) = {
        let c = node.ctrl();
        (c.min, c.max, c.precision, c.value)
    };
    let (_, s) = editor::commit_format(value, min, max, precision);
    if let Some(ed) = &mut node.editor {
        ed.set_text(&s);
        ed.select_all();
        ed.seeded = true;
        ed.caret_moved = true;
    }
    node.mark_dirty();
}

/// The control's value as a 0..1 fraction of its `[min, max]` range.
pub(crate) fn ctrl_value_frac(node: &Node) -> f64 {
    let span = node.ctrl().max - node.ctrl().min;
    if span.abs() < f64::EPSILON {
        0.0
    } else {
        ((node.ctrl().value - node.ctrl().min) / span).clamp(0.0, 1.0)
    }
}

/// Resolve a pending `selected_tag` against the loaded `tags` into a
/// `selected_index` (NavigationView). The rail indicator is a chrome part —
/// the paint pass glides/snaps it from `selected_index` directly.
///
/// The built-in settings row is selectable but lives in no app item list, so
/// its tag resolves to the sentinel [`nav::SETTINGS_INDEX`] instead of a list
/// position. An app item tagged "settings" wins over the built-in row — the
/// list is the app's own naming, matched first.
fn sync_selected_tag(node: &mut Node) {
    if let Some(tag) = &node.ctrl().selected_tag {
        if let Some(i) = node.ctrl().tags.iter().position(|t| t == tag) {
            node.ctrl_mut().selected_index = i as i32;
        } else if node.kind == ControlKind::NavigationView && tag == nav::SETTINGS_TAG {
            node.ctrl_mut().selected_index = nav::SETTINGS_INDEX;
        }
    }
}

/// Lower a frontend [`crate::MenuItemDef`] to a flat painted [`MenuRow`].
fn menu_row(def: &crate::MenuItemDef) -> MenuRow {
    use crate::MenuItemDef as M;
    match def {
        M::Separator => MenuRow {
            separator: true,
            enabled: false,
            ..MenuRow::default()
        },
        M::SubItem { text, .. } => MenuRow {
            text: text.clone(),
            tag: text.clone(),
            enabled: true,
            ..MenuRow::default()
        },
        M::Item {
            text,
            icon,
            danger,
            enabled,
            shortcut,
        } => MenuRow {
            text: text.clone(),
            tag: text.clone(),
            icon: icon.map(|s| s.0 as u32).unwrap_or(0),
            shortcut: shortcut.clone().unwrap_or_default(),
            enabled: *enabled,
            danger: *danger,
            separator: false,
        },
    }
}

/// Apply a StackPanel's spacing to the correct Taffy gap axis for its direction.
fn apply_stack_gap(node: &mut Node) {
    use taffy::prelude::*;
    let s = node.spacing;
    node.style.gap = match node.style.flex_direction {
        FlexDirection::Row | FlexDirection::RowReverse => Size {
            width: length(s),
            height: length(0.0),
        },
        _ => Size {
            width: length(0.0),
            height: length(s),
        },
    };
}
