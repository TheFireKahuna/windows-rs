//! The retained node arena: one [`Node`] per live [`ControlId`]. Each node owns
//! a composition `ContainerVisual` (parented to mirror the logical tree), its
//! Taffy layout inputs, an optional painted-chrome [`NodeSurface`], children,
//! handlers, and the small interaction state (hover/press springs) the spine
//! animates.
//!
//! The arena is the single source of truth for layout and paint. The composition
//! tree is kept in lock-step incrementally: structural edits mark a parent's
//! child order dirty (re-synced once per layout pass), layout writes each node's
//! offset/size/opacity/clip onto its container, and paint redraws a node's
//! surface only when its own content or size changed.

use super::bootstrap::NodeSurface;
use super::editor::Editor;
use super::*;
use crate::backend::{ControlKind, Event, EventHandler};
use crate::style::{AccessibilityModifiers, PointerHandlers};
use crate::system_bindings::{ContainerVisual, IVisual, InsetClip};
use crate::Color;
use crate::LineEndpoints;
use windows_canvas_core::{ColorF, TextLayout};
use windows_core::Interface;

/// Convert a reactor [`Color`] to a [`ColorF`] for D2D. Both are linear scRGB now,
/// so this is a straight passthrough — the node-chrome surfaces are FP16 scRGB
/// (linear) and consume the value raw (no gamma transform). The name is kept for its
/// many call sites; an 8-bit target would re-encode at its own boundary instead.
pub(crate) fn linear(c: Color) -> ColorF {
    ColorF::new(c.r, c.g, c.b, c.a)
}

/// Linearly interpolate two scRGB colors.
pub(crate) fn lerp_color(a: ColorF, b: ColorF, t: f32) -> ColorF {
    let l = |x: f32, y: f32| x + (y - x) * t;
    ColorF::new(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a))
}

/// Semi-implicit-Euler spring (snappy tuning), mirroring the project's
/// `spring.rs`. Stepped only while a node is animating; settles to exact target.
#[derive(Clone, Copy)]
pub(crate) struct Spring {
    pub x: f32,
    pub v: f32,
    pub target: f32,
}

impl Spring {
    pub fn new(x: f32) -> Self {
        Self { x, v: 0.0, target: x }
    }

    /// Advance by `dt` seconds; returns `true` once settled.
    pub fn step(&mut self, dt: f32) -> bool {
        let dt = dt.min(0.05);
        let (k, c) = (520.0_f32, 40.0_f32);
        let a = -k * (self.x - self.target) - c * self.v;
        self.v += a * dt;
        self.x += self.v * dt;
        if (self.x - self.target).abs() < 1e-3 && self.v.abs() < 1e-3 {
            self.x = self.target;
            self.v = 0.0;
            true
        } else {
            false
        }
    }
}

/// An absolute laid-out rectangle, in DIPs (top-left origin, window-relative).
/// Used for hit-testing; composition offsets are stored relatively per node.
#[derive(Clone, Copy, Default)]
pub(crate) struct LaidRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl LaidRect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

/// The painted content of a node, separate from layout. All optional — a bare
/// `StackPanel`/`Grid`/`Canvas` paints nothing itself.
#[derive(Default)]
pub(crate) struct Paint {
    pub background: Option<Color>,
    pub corner_radius: f32,
    pub border_brush: Option<Color>,
    pub border_thickness: f32,
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_thickness: f32,
    pub foreground: Option<Color>,
    pub line: LineEndpoints,
    /// Text content (TextBlock text or Button label).
    pub text: String,
    pub font_size: f32,
    pub font_weight: u16,
    pub font_family: Option<String>,
    pub wrap: bool,
    /// Button accent/subtle/etc. variant (0 = default).
    pub style_variant: i32,
    pub is_enabled: bool,
}

/// A single command row in a popup menu / dropdown list.
#[derive(Clone, Default)]
pub(crate) struct MenuRow {
    pub text: String,
    pub tag: String,
    /// Icon glyph codepoint (a `Symbol`'s integer value), 0 = none.
    pub icon: u32,
    pub shortcut: String,
    pub enabled: bool,
    pub danger: bool,
    pub separator: bool,
}

/// Control-specific state, distinct from generic layout/paint. Populated by
/// `set_prop` for the stateful drawn controls (toggle, slider, segmented, …).
#[derive(Clone)]
pub(crate) struct Ctrl {
    pub is_on: bool,
    pub is_checked: bool,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: Option<f64>,
    pub indeterminate: bool,
    pub is_active: bool,
    pub selected_index: i32,
    /// Display labels (SelectorBar segments / ComboBox items / nav item names).
    pub items: Vec<String>,
    /// Per-item tag (NavigationView) — parallel to `items` when present.
    pub tags: Vec<String>,
    /// Per-item icon glyph codepoint (NavigationView), 0 = none.
    pub icons: Vec<u32>,
    pub expanded: bool,
    /// Selected tag requested via `Prop::SelectedTag` before items arrived.
    pub selected_tag: Option<String>,
    /// Menu rows for DropDownButton / SplitButton / MenuFlyout popups.
    pub menu: Vec<MenuRow>,
    pub placeholder: String,
    /// ScrollViewer: total content height in DIPs (computed at layout).
    pub content_h: f32,
    /// NumberBox: fraction digits shown / rounded to on commit.
    pub precision: Option<i32>,
    /// NumberBox: PageUp/PageDown increment (`LargeChange`).
    pub large_change: Option<f64>,
    /// Text field content alignment (WinRT `HorizontalAlignment`; -1 = unset).
    pub content_align: i32,
}

impl Default for Ctrl {
    fn default() -> Self {
        Self {
            is_on: false,
            is_checked: false,
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: None,
            indeterminate: false,
            is_active: true,
            selected_index: -1,
            items: Vec::new(),
            tags: Vec::new(),
            icons: Vec::new(),
            expanded: false,
            selected_tag: None,
            menu: Vec::new(),
            placeholder: String::new(),
            content_h: 0.0,
            precision: None,
            large_change: None,
            content_align: -1,
        }
    }
}

/// One live control.
pub(crate) struct Node {
    pub kind: ControlKind,
    /// Taffy layout inputs, mutated in place by `set_prop`.
    pub style: taffy::Style,
    pub children: Vec<ControlId>,
    pub paint: Paint,
    /// StackPanel spacing (DIPs); applied to the Taffy gap on the main axis.
    pub spacing: f32,
    /// Grid track templates (applied at layout time for a `Grid`).
    pub grid_rows: Vec<GridLength>,
    pub grid_cols: Vec<GridLength>,
    /// Cached DWrite layout for text-bearing nodes; rebuilt on text/font change.
    pub text_layout: Option<TextLayout>,
    pub text_dirty: bool,
    pub handlers: Vec<(Event, EventHandler)>,
    pub pointer: Option<PointerHandlers>,
    pub accessibility: Option<AccessibilityModifiers>,

    // ── Composition ──────────────────────────────────────────────────────
    /// This node's container visual (always present); mirrors the logical tree.
    pub container: ContainerVisual,
    /// Cached `IVisual` view of `container` for frequent offset/size/opacity ops.
    pub vis: IVisual,
    /// Painted-chrome surface — created lazily for nodes that draw something.
    pub surf: Option<NodeSurface>,
    /// ScrollViewer only: the auto-hiding overlay scrollbar thumb sprite (a top
    /// child of the container, above the scrolled content), created lazily.
    pub scroll_thumb: Option<NodeSurface>,
    /// Thumb opacity (0 hidden … 1 shown); sprung in on hover/scroll, out at rest.
    pub thumb_fade: Spring,
    /// Thumb height (DIP) the thumb surface was last drawn at (redraw on change).
    pub thumb_drawn_h: f32,
    /// While dragging the thumb: the pointer-to-thumb-top offset captured at press.
    pub thumb_drag: Option<f32>,
    /// Bounds clip (ScrollViewer/overflow); tracks the container's own size.
    pub clip: Option<InsetClip>,
    /// WinRT alignment requests (-1 = unset; 0..3 mirror WinRT enums).
    pub h_align: i32,
    pub v_align: i32,
    /// Canvas Z-order (composition child order is resynced by it).
    pub z_index: i32,
    /// This node's Z-order changed; its parent must re-sync child order.
    pub z_dirty: bool,
    /// The composition child order under this node needs re-syncing.
    pub children_dirty: bool,
    /// This node's surface needs a repaint (content/size/state changed).
    pub dirty: bool,

    /// Transient: the Taffy node this maps to in the current layout pass.
    pub taffy_id: Option<taffy::NodeId>,
    pub rect: LaidRect,
    pub hover: Spring,
    pub press: Spring,
    pub hovered: bool,
    pub pressed: bool,

    // ── Control library state ────────────────────────────────────────────
    /// Stateful drawn-control data (toggle/slider/segmented/select/nav/…).
    pub ctrl: Ctrl,
    /// The control's primary animated quantity: toggle-knob / segmented-pill /
    /// nav-indicator position, slider-thumb fraction, or scroll offset.
    pub anim: Spring,
    /// Continuous phase for indeterminate progress (advanced by the frame tick).
    pub phase: f32,
    /// This node accepts keyboard focus (Tab) + Space/Enter activation.
    pub focusable: bool,
    /// This node currently holds keyboard focus (draws the focus ring).
    pub focused: bool,

    /// Text-editor state for the editable text kinds (NumberBox / TextBox /
    /// PasswordBox / AutoSuggestBox); `None` for every other kind.
    pub editor: Option<Editor>,

    // ── TitleBar caption slots ───────────────────────────────────────────
    /// TitleBar only: the mounted `Content` (centered) slot child, if any.
    /// Tracked so a slot swap/clear can detach the right child from `children`.
    pub title_content: Option<ControlId>,
    /// TitleBar only: the mounted `RightHeader`/footer (trailing) slot child.
    pub title_footer: Option<ControlId>,
}

impl Node {
    pub fn new(kind: ControlKind, container: ContainerVisual) -> Self {
        let vis: IVisual = container.cast().expect("ContainerVisual is an IVisual");
        let mut paint = Paint {
            font_size: default_font_size(kind),
            font_weight: 400,
            is_enabled: true,
            ..Paint::default()
        };
        // Buttons draw their own chrome (the WinUI default style is gone here).
        if kind == ControlKind::Button {
            paint.corner_radius = 6.0;
        }
        let focusable = is_focusable_kind(kind);
        Self {
            kind,
            style: default_style(kind),
            children: Vec::new(),
            paint,
            spacing: 0.0,
            grid_rows: Vec::new(),
            grid_cols: Vec::new(),
            text_layout: None,
            text_dirty: true,
            handlers: Vec::new(),
            pointer: None,
            accessibility: None,
            container,
            vis,
            surf: None,
            scroll_thumb: None,
            thumb_fade: Spring::new(0.0),
            thumb_drawn_h: 0.0,
            thumb_drag: None,
            clip: None,
            h_align: -1,
            v_align: -1,
            z_index: 0,
            z_dirty: false,
            children_dirty: false,
            dirty: true,
            taffy_id: None,
            rect: LaidRect::default(),
            hover: Spring::new(0.0),
            press: Spring::new(0.0),
            hovered: false,
            pressed: false,
            ctrl: Ctrl::default(),
            anim: Spring::new(0.0),
            phase: 0.0,
            focusable,
            focused: false,
            editor: is_text_editable(kind).then(|| Editor::new(kind)),
            title_content: None,
            title_footer: None,
        }
    }

    pub fn handler(&self, event: Event) -> Option<&EventHandler> {
        self.handlers.iter().find(|(e, _)| *e == event).map(|(_, h)| h)
    }

    /// True for nodes that respond to a press (hover/press ink + activate).
    pub fn is_clickable(&self) -> bool {
        is_interactive_kind(self.kind)
            || is_text_editable(self.kind)
            || self.handler(Event::Click).is_some()
            || self
                .pointer
                .as_ref()
                .is_some_and(|p| p.on_tapped.is_some() || p.on_pointer_pressed.is_some())
    }

    /// Whether this node draws any chrome (and therefore needs a surface).
    pub fn has_chrome(&self) -> bool {
        match self.kind {
            ControlKind::Button => true,
            ControlKind::TextBlock => !self.paint.text.is_empty(),
            ControlKind::Line => self.paint.stroke.is_some(),
            ControlKind::Ellipse | ControlKind::Rectangle => {
                self.paint.fill.is_some()
                    || self.paint.background.is_some()
                    || (self.paint.stroke.is_some() && self.paint.stroke_thickness > 0.0)
                    || (self.paint.border_brush.is_some() && self.paint.border_thickness > 0.0)
            }
            // Every drawn control owns a surface (it always paints its chrome).
            _ if draws_own_chrome(self.kind) => true,
            _ => {
                self.paint.background.is_some()
                    || (self.paint.border_brush.is_some() && self.paint.border_thickness > 0.0)
                    || self.focused
            }
        }
    }

    /// Whether the node should consume vertical mouse-wheel input (scrolling).
    pub fn is_scroll(&self) -> bool {
        matches!(self.kind, ControlKind::ScrollViewer | ControlKind::ScrollView)
    }

    /// Mark this node's surface for repaint.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

fn default_font_size(kind: ControlKind) -> f32 {
    match kind {
        ControlKind::ToggleSwitch
        | ControlKind::CheckBox
        | ControlKind::SelectorBar
        | ControlKind::ComboBox
        | ControlKind::DropDownButton
        | ControlKind::SplitButton => theme::FONT_SIZE_SM,
        _ => 14.0,
    }
}

/// Control kinds the backend draws and interacts with directly (a press toggles
/// / selects / activates them and they get hover/press ink + a focus ring).
pub(crate) fn is_interactive_kind(kind: ControlKind) -> bool {
    matches!(
        kind,
        ControlKind::Button
            | ControlKind::ToggleSwitch
            | ControlKind::CheckBox
            | ControlKind::ToggleButton
            | ControlKind::RepeatButton
            | ControlKind::HyperlinkButton
            | ControlKind::SelectorBar
            | ControlKind::Slider
            | ControlKind::ComboBox
            | ControlKind::DropDownButton
            | ControlKind::SplitButton
            | ControlKind::NavigationView
            | ControlKind::Expander
    )
}

/// Kinds that always own a paint surface (they draw chrome unconditionally).
fn draws_own_chrome(kind: ControlKind) -> bool {
    is_interactive_kind(kind)
        || is_text_editable(kind)
        || matches!(kind, ControlKind::ProgressBar | ControlKind::ProgressRing)
}

/// The editable text kinds, each backed by a shared [`Editor`].
pub(crate) fn is_text_editable(kind: ControlKind) -> bool {
    matches!(
        kind,
        ControlKind::NumberBox
            | ControlKind::TextBox
            | ControlKind::PasswordBox
            | ControlKind::AutoSuggestBox
    )
}

/// Kinds that take keyboard focus in the Tab ring.
fn is_focusable_kind(kind: ControlKind) -> bool {
    matches!(
        kind,
        ControlKind::Button
            | ControlKind::ToggleSwitch
            | ControlKind::CheckBox
            | ControlKind::ToggleButton
            | ControlKind::RepeatButton
            | ControlKind::HyperlinkButton
            | ControlKind::SelectorBar
            | ControlKind::Slider
            | ControlKind::ComboBox
            | ControlKind::DropDownButton
            | ControlKind::SplitButton
            | ControlKind::Expander
            | ControlKind::NumberBox
            | ControlKind::TextBox
            | ControlKind::PasswordBox
            | ControlKind::AutoSuggestBox
    )
}

/// Per-kind default Taffy style (display mode + the small intrinsic defaults a
/// drawn control needs, e.g. button padding so its label isn't cramped).
fn default_style(kind: ControlKind) -> taffy::Style {
    use taffy::prelude::*;
    let mut s = Style::default();
    match kind {
        ControlKind::StackPanel => {
            s.display = Display::Flex;
            s.flex_direction = FlexDirection::Column;
        }
        ControlKind::Grid => {
            s.display = Display::Grid;
        }
        ControlKind::Canvas => {
            // Children position themselves absolutely via Canvas.Left/Top.
            s.display = Display::Block;
        }
        ControlKind::Border => {
            // A Border is a WinUI ContentControl: it sizes to its single child +
            // padding, and stretches that child to fill its content box (the child's
            // own alignment/size still win). A one-cell Grid is the faithful Taffy
            // model — a Flex row leaves the child at its content width (no main-axis
            // stretch), which collapses a full-bleed Border to its content. Grid
            // items default to align/justify-self Stretch, so the child fills.
            s.display = Display::Grid;
        }
        ControlKind::ScrollViewer | ControlKind::ScrollView => {
            // Visual clipping is done by a composition InsetClip on the node's
            // container (see `Backend::create`); layout treats it as a block.
            s.display = Display::Block;
        }
        ControlKind::NavigationView => {
            // The icon rail is drawn on the node's own surface; the single
            // content child is inset to the right of it.
            s.display = Display::Flex;
            s.padding.left = length(theme::NAV_RAIL_W);
        }
        ControlKind::Expander => {
            // A header band is drawn on the node's surface; content sits below.
            s.display = Display::Flex;
            s.flex_direction = FlexDirection::Column;
            s.padding.top = length(theme::ROW_H + theme::SPACE_8 + theme::SPACE_4);
        }
        ControlKind::Button => {
            s.display = Display::Flex;
            s.align_items = Some(AlignItems::CENTER);
            s.justify_content = Some(JustifyContent::CENTER);
            s.padding = Rect {
                left: length(12.0),
                right: length(12.0),
                top: length(6.0),
                bottom: length(6.0),
            };
            s.min_size = Size {
                width: length(0.0),
                height: length(30.0),
            };
        }
        ControlKind::NumberBox
        | ControlKind::TextBox
        | ControlKind::PasswordBox
        | ControlKind::AutoSuggestBox => {
            // The editor draws its own box chrome + caret on its surface.
            s.display = Display::Block;
            s.min_size.height = length(theme::ROW_H);
        }
        ControlKind::TitleBar => {
            // The custom caption strip. Its two slot children (Content and the
            // trailing RightHeader/footer) are attached by `set_header_element` /
            // `set_pane_element`; there are no positional children. A two-track
            // grid `[Star, Auto]` lets Content span the full width and center
            // while the footer pins to the trailing auto-sized column. WinUI's
            // native caption owns min/max/close above the client area, so the
            // whole strip is ours end-to-end.
            s.display = Display::Grid;
            s.grid_template_columns =
                vec![GridTemplateComponent::Single(flex(1.0)), GridTemplateComponent::Single(auto())];
            s.grid_template_rows = vec![GridTemplateComponent::Single(auto())];
            // Standard 48px caption height with the token side padding.
            s.min_size.height = length(theme::ROW_H + theme::SPACE_16);
            s.padding.left = length(theme::SPACE_16);
            s.padding.right = length(theme::SPACE_16);
        }
        _ => {
            s.display = Display::Flex;
        }
    }
    // XAML Grid parity: a child with no explicit `Grid.Row`/`Grid.Column` belongs to
    // cell (0,0) and *overlaps* any sibling that also has none — XAML Grid has no
    // auto-flow. Taffy would otherwise auto-place each unplaced item into the next
    // free cell, stacking same-cell children (e.g. a backdrop + shell that both sit
    // at row 0 / col 0). Default the start lines to the first track; an explicit
    // `grid_row(n)`/`grid_column(n)` overrides it (the reconciler only emits the
    // attached prop for a non-zero value). `grid_*` is inert unless the parent is a
    // Grid, so this is harmless for flex/block/canvas parents.
    s.grid_row.start = line(1);
    s.grid_column.start = line(1);
    s
}

/// Slot-map style arena keyed by [`ControlId`] (a `NonZeroU32`). Index `id-1`.
#[derive(Default)]
pub(crate) struct Arena {
    slots: Vec<Option<Node>>,
    free: Vec<u32>,
    next: u32,
}

impl Arena {
    pub fn insert(&mut self, node: Node) -> ControlId {
        if let Some(idx) = self.free.pop() {
            self.slots[idx as usize] = Some(node);
            ControlId::new(idx + 1)
        } else {
            self.next += 1;
            self.slots.push(Some(node));
            ControlId::new(self.next)
        }
    }

    pub fn get(&self, id: ControlId) -> Option<&Node> {
        self.slots.get((id.get() - 1) as usize).and_then(|s| s.as_ref())
    }

    pub fn get_mut(&mut self, id: ControlId) -> Option<&mut Node> {
        self.slots
            .get_mut((id.get() - 1) as usize)
            .and_then(|s| s.as_mut())
    }

    /// Iterate every live node mutably (order unspecified).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.slots.iter_mut().filter_map(|s| s.as_mut())
    }

    pub fn remove(&mut self, id: ControlId) -> Option<Node> {
        let idx = (id.get() - 1) as usize;
        let taken = self.slots.get_mut(idx).and_then(|s| s.take());
        if taken.is_some() {
            self.free.push(idx as u32);
        }
        taken
    }
}
