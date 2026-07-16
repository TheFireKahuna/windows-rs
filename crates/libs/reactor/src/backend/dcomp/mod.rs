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
//! or size changed. Input is hit-tested against the layout output; button
//! hover/press animate via a self-stopping timer (see [`host`]). Gated behind the
//! `dcomp-backend` feature.

use crate::backend::ControlId;

mod bootstrap;
mod caption;
mod color_out;
mod controls;
mod dispatch;
mod display_change;
mod editor;
mod host;
mod input;
mod layout;
mod node;
mod paint;
mod pointer;
mod popup;
mod scroll;
mod size;
mod theme;
pub use theme::{set_host_tokens, HostTokens};
mod uia;
pub(crate) mod visibility;

pub use color_out::set_output_color_transform;
pub use dispatch::Win32Dispatcher;
pub use host::DCompHost;
pub use display_change::set_display_change_callback;
pub use visibility::set_window_visibility_callback;
pub(crate) use pointer::{register_element_pointer, PointerSinks};
pub(crate) use size::register_element_size;

use bootstrap::Compositing;
use node::{Arena, MenuRow, Node};
use paint::PaintCache;
use rustc_hash::FxHashSet;

use crate::backend::{Backend, ControlKind, Event, EventHandler, Prop, PropValue};
use crate::style::{AccessibilityModifiers, GridLength, PointerHandlers};

/// The DirectComposition backend. Owns the node arena, the window's composition
/// infrastructure, and the per-device paint cache.
pub struct DCompBackend {
    arena: Arena,
    comp: Compositing,
    cache: PaintCache,
    root: Option<ControlId>,
    /// The node whose container is currently attached under the compositor root.
    attached_root: Option<ControlId>,
    /// Viewport in DIPs and the window DPI (96 = 100%).
    dip_size: (f32, f32),
    dpi: f32,
    /// Nodes with a spring still in flight (driven by the self-stopping tick).
    animating: FxHashSet<ControlId>,
    /// The clickable node currently under the pointer (hover) / pressed.
    hovered_id: Option<ControlId>,
    pressed_id: Option<ControlId>,
    /// The scroll container currently under the pointer (drives thumb fade-in).
    hovered_scroll: Option<ControlId>,
    /// The scroll container whose thumb is being dragged, if any.
    dragging_thumb: Option<ControlId>,
    /// The node holding keyboard focus (drives the focus ring + Space/Enter).
    focused_id: Option<ControlId>,
    /// A registered viz pointer surface (knob/slider/EQ canvas) being dragged:
    /// its node, sinks, and the ancestor scroll offset captured at press time
    /// (added to raw move/up coords so element-relative positions stay correct
    /// inside a scrolled chain). Set on down over the surface, cleared on up —
    /// implicit capture for the drag's duration.
    pressed_surface: Option<(ControlId, std::rc::Rc<PointerSinks>, f32)>,
    /// The live popup overlay (Select/menu dropdown), if one is open.
    popup: Option<popup::Popup>,
    /// Whether the open popup's reveal animation has settled.
    popup_settled: bool,
    /// The host window handle (as `isize`) — used for clipboard ownership.
    hwnd: isize,
}

impl DCompBackend {
    pub(crate) fn new(comp: Compositing, dip_size: (f32, f32), dpi: f32, hwnd: isize) -> Self {
        Self {
            arena: Arena::default(),
            comp,
            cache: PaintCache::default(),
            root: None,
            attached_root: None,
            dip_size,
            dpi,
            animating: FxHashSet::default(),
            hovered_id: None,
            pressed_id: None,
            hovered_scroll: None,
            dragging_thumb: None,
            focused_id: None,
            pressed_surface: None,
            popup: None,
            popup_settled: true,
            hwnd,
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
        if let Some(root) = self.root {
            let (w, h) = self.dip_size;
            let scale = self.scale();
            layout::compute(&mut self.arena, root, w, h, scale);
            self.repaint();
        }
    }

    /// Repaint dirty node surfaces (no relayout) — also used by the animation
    /// tick, which marks animating nodes dirty.
    pub(crate) fn repaint(&mut self) {
        if let Some(root) = self.root {
            let scale = self.scale();
            if paint::paint(&self.comp, &mut self.cache, &mut self.arena, root, scale).is_err() {
                // Device loss: drop cached resources; next paint rebuilds them.
                self.cache.invalidate();
            }
        }
    }

    /// React to a window resize (physical pixels). Re-folds DPI into the root
    /// scale and the DIP viewport, then relays out and repaints.
    pub(crate) fn resize(&mut self, pixel_w: i32, pixel_h: i32, dpi: u32) {
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
        if let Some(tb) = self.node_mut(id) {
            let prev = if footer {
                tb.title_footer.take()
            } else {
                tb.title_content.take()
            };
            if let Some(prev) = prev {
                tb.children.retain(|c| *c != prev);
                tb.children_dirty = true;
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

    /// The TitleBar node (the custom caption band), if the tree has one.
    fn titlebar_id(&self) -> Option<ControlId> {
        self.arena
            .iter()
            .find(|(_, n)| n.kind == ControlKind::TitleBar)
            .map(|(id, _)| id)
    }

    /// The caption band's layout box in window DIPs (`(x, y, w, h)`), if a
    /// TitleBar is mounted — the host's non-client hit-test region.
    pub(crate) fn caption_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let n = self.arena.get(self.titlebar_id()?)?;
        Some((n.rect.x, n.rect.y, n.rect.w, n.rect.h))
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
        for slot in self.arena.iter_mut() {
            slot.mark_dirty();
        }
        self.repaint();
    }
}

impl Backend for DCompBackend {
    fn create(&mut self, kind: ControlKind) -> ControlId {
        let container = self
            .comp
            .new_container()
            .expect("compositor container allocation");
        let mut node = Node::new(kind, container);
        // Scroll/overflow containers clip their children to their own bounds.
        if matches!(kind, ControlKind::ScrollViewer | ControlKind::ScrollView)
            && let Ok(clip) = self.comp.new_inset_clip()
        {
            use windows_core::Interface;
            if let Ok(c) = clip.cast::<crate::system_bindings::CompositionClip>() {
                let _ = node.vis.SetClip(&c);
            }
            node.clip = Some(clip);
        }
        self.arena.insert(node)
    }

    fn set_prop(&mut self, id: ControlId, prop: Prop, value: &PropValue) {
        use taffy::prelude::*;
        // Set inside an arm to slide the control's primary spring to a new
        // target after the node borrow ends (programmatic state change).
        let mut start_anim = false;
        let mut refresh_suggest = false;
        {
            let Some(node) = self.node_mut(id) else { return };
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

            (Prop::Content | Prop::Text, PropValue::Str(s)) => {
                // For an editable kind (AutoSuggestBox carries its text via
                // `Prop::Text`), seed the editor buffer instead of the label.
                if node.editor.is_some() {
                    seed_editor_text(node, s);
                } else {
                    node.paint.text = s.clone();
                    node.text_dirty = true;
                }
                node.mark_dirty();
            }
            // TextBox / PasswordBox carry their text via `Prop::Value(Str)`.
            (Prop::Value, PropValue::Str(s)) if node.editor.is_some() => {
                seed_editor_text(node, s);
                node.mark_dirty();
            }
            (Prop::Precision, PropValue::I32(v)) => {
                node.ctrl.precision = Some(*v);
                // Reformat the seeded value to the new precision (the `Value`
                // prop usually arrives before `Precision`). Never while focused
                // — the user owns the buffer mid-edit.
                if node.kind == ControlKind::NumberBox && !node.focused {
                    let value = node.ctrl.value;
                    if let Some(ed) = &mut node.editor {
                        ed.seeded = false;
                    }
                    seed_number_text(node, value);
                    node.mark_dirty();
                }
            }
            (Prop::LargeChange, PropValue::F64(v)) => node.ctrl.large_change = Some(*v),
            (Prop::HorizontalContentAlignment, PropValue::I32(v)) => {
                node.ctrl.content_align = *v;
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
            (Prop::IsOn, PropValue::Bool(v)) => {
                node.ctrl.is_on = *v;
                node.anim.target = if *v { 1.0 } else { 0.0 };
                start_anim = (node.anim.x - node.anim.target).abs() > 1e-3;
                node.mark_dirty();
            }
            (Prop::IsChecked, PropValue::Bool(v)) => {
                node.ctrl.is_checked = *v;
                node.anim.target = if *v { 1.0 } else { 0.0 };
                start_anim = (node.anim.x - node.anim.target).abs() > 1e-3;
                node.mark_dirty();
            }
            (Prop::Value, PropValue::F64(v)) => {
                node.ctrl.value = *v;
                node.anim.target = ctrl_value_frac(node) as f32;
                start_anim = node.kind == ControlKind::Slider
                    && (node.anim.x - node.anim.target).abs() > 1e-3;
                // NumberBox: reflect the programmatic value as formatted text
                // (unless the user is mid-edit — the editor owns the buffer
                // while focused).
                if node.kind == ControlKind::NumberBox {
                    seed_number_text(node, *v);
                }
                node.mark_dirty();
            }
            (Prop::Minimum, PropValue::F64(v)) => {
                node.ctrl.min = *v;
                node.anim.x = ctrl_value_frac(node) as f32;
                node.anim.target = node.anim.x;
                node.mark_dirty();
            }
            (Prop::Maximum, PropValue::F64(v)) => {
                node.ctrl.max = *v;
                node.anim.x = ctrl_value_frac(node) as f32;
                node.anim.target = node.anim.x;
                node.mark_dirty();
            }
            (Prop::Step, PropValue::F64(v)) => node.ctrl.step = Some(*v),
            (Prop::IsIndeterminate, PropValue::Bool(v)) => {
                node.ctrl.indeterminate = *v;
                node.mark_dirty();
            }
            (Prop::IsActive, PropValue::Bool(v)) => {
                node.ctrl.is_active = *v;
                node.mark_dirty();
            }
            (Prop::IsExpanded, PropValue::Bool(v)) => {
                node.ctrl.expanded = *v;
                node.anim.target = if *v { 1.0 } else { 0.0 };
                start_anim = (node.anim.x - node.anim.target).abs() > 1e-3;
                node.mark_dirty();
            }
            (Prop::SelectedIndex, PropValue::I32(v)) => {
                node.ctrl.selected_index = *v;
                node.anim.target = (*v).max(0) as f32;
                start_anim = node.kind == ControlKind::SelectorBar
                    && (node.anim.x - node.anim.target).abs() > 1e-3;
                node.mark_dirty();
            }
            (Prop::SelectedTag, PropValue::Str(s)) => {
                node.ctrl.selected_tag = Some(s.clone());
                sync_selected_tag(node);
                node.mark_dirty();
            }
            (Prop::PlaceholderText, PropValue::Str(s)) => {
                node.ctrl.placeholder = s.clone();
                node.mark_dirty();
            }
            (Prop::Items, PropValue::StrList(list)) => {
                node.ctrl.items = list.clone();
                node.mark_dirty();
                // A focused AutoSuggestBox whose filtered list just changed refreshes
                // its open dropdown in place (deferred until the node borrow ends).
                refresh_suggest = node.kind == ControlKind::AutoSuggestBox;
            }
            (Prop::Items, PropValue::SelectorBarItems(items)) => {
                node.ctrl.items = items.iter().map(|i| i.text.clone()).collect();
                if node.ctrl.selected_index < 0 && !node.ctrl.items.is_empty() {
                    node.ctrl.selected_index = 0;
                    node.anim.x = 0.0;
                    node.anim.target = 0.0;
                }
                node.mark_dirty();
            }
            (Prop::MenuItems, PropValue::NavMenuItems(items)) => {
                node.ctrl.items.clear();
                node.ctrl.tags.clear();
                node.ctrl.icons.clear();
                for it in items {
                    if it.is_header {
                        continue;
                    }
                    node.ctrl.items.push(it.content.clone());
                    node.ctrl
                        .tags
                        .push(it.tag.clone().unwrap_or_else(|| it.content.clone()));
                    node.ctrl.icons.push(it.icon.map(|s| s.0 as u32).unwrap_or(0));
                }
                sync_selected_tag(node);
                node.mark_dirty();
            }
            (Prop::MenuFlyoutItems, PropValue::MenuFlyoutItems(items)) => {
                node.ctrl.menu = items.iter().map(menu_row).collect();
                node.mark_dirty();
            }
            _ => {}
            }
        }
        if start_anim {
            self.animating.insert(id);
        }
        if refresh_suggest {
            self.refresh_suggest(id);
        }
    }

    fn append_child(&mut self, parent: ControlId, child: ControlId) {
        if let Some(p) = self.node_mut(parent) {
            p.children.push(child);
            p.children_dirty = true;
        }
    }

    fn insert_child(&mut self, parent: ControlId, index: usize, child: ControlId) {
        if let Some(p) = self.node_mut(parent) {
            let i = index.min(p.children.len());
            p.children.insert(i, child);
            p.children_dirty = true;
        }
    }

    fn remove_child(&mut self, parent: ControlId, index: usize) {
        if let Some(p) = self.node_mut(parent)
            && index < p.children.len()
        {
            p.children.remove(index);
            p.children_dirty = true;
        }
    }

    fn replace_child(&mut self, parent: ControlId, index: usize, new: ControlId) {
        if let Some(p) = self.node_mut(parent)
            && index < p.children.len()
        {
            p.children[index] = new;
            p.children_dirty = true;
        }
    }

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
        self.animating.remove(&id);
        if self.attached_root == Some(id) {
            if let Some(n) = self.arena.get(id) {
                self.comp.detach_root(&n.container);
            }
            self.attached_root = None;
        }
        self.arena.remove(id);
    }

    fn attach_event(&mut self, id: ControlId, event: Event, handler: EventHandler) {
        if let Some(node) = self.node_mut(id) {
            node.handlers.retain(|(e, _)| *e != event);
            node.handlers.push((event, handler));
        }
    }

    fn detach_event(&mut self, id: ControlId, event: Event) {
        if let Some(node) = self.node_mut(id) {
            node.handlers.retain(|(e, _)| *e != event);
        }
    }

    fn set_pointer_handlers(&mut self, id: ControlId, handlers: Option<&PointerHandlers>) {
        if let Some(node) = self.node_mut(id) {
            node.pointer = handlers.cloned();
        }
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

    fn get_native_element(&self, id: ControlId) -> Option<windows_core::IInspectable> {
        use windows_core::Interface;
        // The node's container visual — what a viz host element (SurfacePainter /
        // composition-surface) attaches its child visual under via `on_mounted`.
        self.node(id).and_then(|n| n.container.cast().ok())
    }
}

/// Revert a prop to its default when the reconciler diffs it away
/// (`PropValue::Unset`). Covers the props elements set conditionally; anything
/// else keeps its last value (matching WinUI's ClearValue granularity).
fn reset_prop(node: &mut Node, prop: Prop) {
    use taffy::prelude::*;
    match prop {
        Prop::Background => {
            node.paint.background = None;
            node.mark_dirty();
        }
        Prop::Foreground => {
            node.paint.foreground = None;
            node.mark_dirty();
        }
        Prop::BorderBrush => {
            node.paint.border_brush = None;
            node.mark_dirty();
        }
        Prop::BorderThickness => {
            node.paint.border_thickness = 0.0;
            node.style.border = Rect::zero();
            node.mark_dirty();
        }
        Prop::CornerRadius => {
            node.paint.corner_radius = 0.0;
            node.mark_dirty();
        }
        Prop::Fill => {
            node.paint.fill = None;
            node.mark_dirty();
        }
        Prop::Stroke => {
            node.paint.stroke = None;
            node.mark_dirty();
        }
        Prop::StrokeThickness => {
            node.paint.stroke_thickness = 0.0;
            node.mark_dirty();
        }
        Prop::Opacity => {
            let _ = node.vis.SetOpacity(1.0);
        }
        Prop::Padding => node.style.padding = Rect::zero(),
        Prop::Margin => node.style.margin = Rect::zero(),
        Prop::Width => node.style.size.width = auto(),
        Prop::Height => node.style.size.height = auto(),
        Prop::MinWidth => node.style.min_size.width = auto(),
        Prop::MinHeight => node.style.min_size.height = auto(),
        Prop::MaxWidth => node.style.max_size.width = auto(),
        Prop::MaxHeight => node.style.max_size.height = auto(),
        Prop::HorizontalAlignment => node.h_align = -1,
        Prop::VerticalAlignment => node.v_align = -1,
        _ => {}
    }
}

fn clone_lengths(g: &[GridLength]) -> Vec<GridLength> {
    g.to_vec()
}

/// Seed an editor's buffer from a programmatic string prop. Skipped while the
/// field is focused so the user's in-progress edit is never clobbered.
fn seed_editor_text(node: &mut Node, s: &str) {
    let focused = node.focused;
    if let Some(ed) = &mut node.editor
        && (!focused || !ed.seeded)
    {
        ed.set_text(s);
        ed.seeded = true;
    }
}

/// Seed a NumberBox editor from a programmatic numeric value, formatted to the
/// configured precision. Skipped while focused (the user owns the buffer).
fn seed_number_text(node: &mut Node, v: f64) {
    let focused = node.focused;
    let precision = node.ctrl.precision;
    if let Some(ed) = &mut node.editor
        && (!focused || !ed.seeded)
    {
        let digits = precision.unwrap_or(2).clamp(0, 12) as usize;
        ed.set_text(&format!("{v:.digits$}"));
        ed.seeded = true;
    }
}

/// The control's value as a 0..1 fraction of its `[min, max]` range.
pub(crate) fn ctrl_value_frac(node: &Node) -> f64 {
    let span = node.ctrl.max - node.ctrl.min;
    if span.abs() < f64::EPSILON {
        0.0
    } else {
        ((node.ctrl.value - node.ctrl.min) / span).clamp(0.0, 1.0)
    }
}

/// Resolve a pending `selected_tag` against the loaded `tags` into a
/// `selected_index` (NavigationView), snapping the indicator spring.
fn sync_selected_tag(node: &mut Node) {
    if let Some(tag) = &node.ctrl.selected_tag
        && let Some(i) = node.ctrl.tags.iter().position(|t| t == tag)
    {
        node.ctrl.selected_index = i as i32;
        node.anim.x = i as f32;
        node.anim.target = i as f32;
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
