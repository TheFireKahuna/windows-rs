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
mod dispatch;
mod host;
mod input;
mod layout;
mod node;
mod paint;

pub use dispatch::Win32Dispatcher;
pub use host::DCompHost;

use bootstrap::Compositing;
use node::{Arena, Node};
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
}

impl DCompBackend {
    pub(crate) fn new(comp: Compositing, dip_size: (f32, f32), dpi: f32) -> Self {
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
            layout::compute(&mut self.arena, root, w, h);
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
        let Some(node) = self.node_mut(id) else { return };
        match (prop, value) {
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
                node.paint.text = s.clone();
                node.text_dirty = true;
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
                // WinRT TextWrapping: NoWrap = 1, Wrap = 2, WrapWholeWords = 3.
                let wrap = match value {
                    PropValue::I32(v) => *v != 1,
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
            _ => {}
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

    fn get_native_element(&self, id: ControlId) -> Option<windows_core::IInspectable> {
        use windows_core::Interface;
        // The node's container visual — what a viz host element (SurfacePainter /
        // composition-surface) attaches its child visual under via `on_mounted`.
        self.node(id).and_then(|n| n.container.cast().ok())
    }
}

fn clone_lengths(g: &[GridLength]) -> Vec<GridLength> {
    g.to_vec()
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
