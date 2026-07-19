//! The retained node arena: one [`Node`] per live [`ControlId`]. Each node owns
//! a composition `ContainerVisual` (parented to mirror the logical tree), its
//! Taffy layout inputs, an optional painted-chrome [`NodeSurface`], children,
//! and the small interaction state (hover/press flags). All motion plays on
//! the system compositor — the backend keeps no CPU-stepped animation state.
//!
//! The arena is the single source of truth for layout and paint. The composition
//! tree is kept in lock-step incrementally: structural edits mark a parent's
//! child order dirty (re-synced once per layout pass), layout writes each node's
//! offset/size/opacity/clip onto its container, and paint redraws a node's
//! surface only when its own content or size changed.

use super::bootstrap::NodeSurface;
use super::editor::Editor;
use super::*;
use crate::backend::{ControlKind, Event};
use crate::style::{
    AccessibilityModifiers, AnimationConfig, ImplicitTransitions, LayoutAnimationConfig,
    PointerHandlers,
};
use crate::system_bindings::{
    ContainerVisual, ICompositionObject, ICompositionObject2, IVisual, ImplicitAnimationCollection,
    InsetClip, Visual,
};
use crate::Color;
use crate::LineEndpoints;
use crate::{
    FlyoutDef, FlyoutPlacementMode, NavigationViewPaneDisplayMode, PasswordRevealMode,
    ScrollBarVisibility,
};
use windows_canvas_core::{ColorF, TextLayout};
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

/// Convert a reactor [`Color`] to a [`ColorF`] for D2D, applying the app's output
/// colour transform ([`color_out`](super::color_out)) on the way. Both currencies
/// are linear scRGB and the node-chrome surfaces are FP16 scRGB (linear), so the
/// transform — an app's tonemap to the current display — runs here in genuine
/// linear light, hue-safe, on every painted colour. Identity when no app opted in.
pub(crate) fn linear(c: Color) -> ColorF {
    let [r, g, b, a] = super::color_out::apply([c.r, c.g, c.b, c.a]);
    ColorF::new(r, g, b, a)
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

/// Which events a control has declared a handler for, for the questions input
/// answers without dispatching: whether a node is hit-testable at all, and
/// whether the caption's back button is live.
///
/// Only events read as a *guard* need a flag. The rest are dispatch-only —
/// their presence changes nothing about how input routes.
#[derive(Copy, Clone, Default)]
pub(crate) struct Interactivity {
    /// A `Click` handler makes an otherwise inert node (a `Border`, a panel)
    /// hit-testable — see [`Node::is_clickable`], which also gates
    /// `WM_NCHITTEST` through `wants_client_at`.
    pub click: bool,
    /// A `BackRequested` handler enables the navigation and caption back
    /// buttons; without one they are inert rather than merely silent.
    pub back: bool,
}

/// Which per-element pointer callbacks the app has declared, by presence.
///
/// The closures themselves live app-side in the recorder's handler map
/// (`record.rs`) and are invoked from queued intents; the retained tree keeps
/// only these bits, which are what input consults synchronously — a node with
/// `tapped`/`pressed` is hit-testable ([`Node::is_clickable`]), and a pressed
/// node with `moved` captures the pointer for the drag's duration.
///
/// Only the five callbacks this backend actually fires have a bit;
/// `on_pointer_entered`/`exited`/`wheel` are WinUI-backend surface and never
/// dispatch here.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub(crate) struct PointerInterest {
    pub tapped: bool,
    pub right_tapped: bool,
    pub pressed: bool,
    pub released: bool,
    pub moved: bool,
}

impl PointerInterest {
    /// The declaration for a set of pointer handlers — presence only.
    pub fn of(h: &PointerHandlers) -> Self {
        Self {
            tapped: h.on_tapped.is_some(),
            right_tapped: h.on_right_tapped.is_some(),
            pressed: h.on_pointer_pressed.is_some(),
            released: h.on_pointer_released.is_some(),
            moved: h.on_pointer_moved.is_some(),
        }
    }
}

/// The painted content of a node, separate from layout. All optional — a bare
/// `StackPanel`/`Grid`/`Canvas` paints nothing itself.
#[derive(Default, Debug)]
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
#[derive(Clone, Default, Debug)]
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

/// The chrome-heavy control state only a handful of kinds ever carry: the
/// caption strip's titles, the NavigationView pane and its embedded search box,
/// the scrollbar visibility policy, a button's flyout/icon, the ToggleSwitch
/// on/off labels, and the editor / repeat-button policy flags.
///
/// Boxed inside [`Ctrl`] and allocated on first write, for the same reason
/// `Ctrl` is boxed inside [`Node`]: a `Ctrl` already exists on every Slider,
/// Knob, ToggleSwitch and ComboBox, and none of those carry ANY of this. Inline
/// it would be ~250 bytes of `String`/`Vec` headers on each of them; behind the
/// box it is one pointer, and a real tree holds at most one TitleBar and one
/// NavigationView.
///
/// Every field here is currently **written and reset but not yet drawn** — see
/// [`Status::Stored`](super::Status::Stored). The state lands so the drawing
/// side can be written against a node that already holds the right value.
#[derive(Clone, Debug)]
pub(crate) struct Extras {
    // ── TitleBar / InfoBar ───────────────────────────────────────────────
    /// Title text (`Prop::Title`) — the caption band's, or the InfoBar's.
    ///
    /// One field for both because a node is exactly one `ControlKind` for its
    /// whole life, so the two readings can never be live at once; splitting it
    /// would only widen every `Extras` that carries neither.
    pub title: String,
    /// Caption subtitle, drawn after the title in a dimmer style.
    pub subtitle: String,
    /// Tall (double-height) caption band.
    pub tall: bool,
    /// TitleBar back button: shown / clickable.
    pub back_button_visible: bool,
    pub back_button_enabled: bool,
    /// TitleBar + NavigationView: the pane (hamburger) toggle is shown.
    pub pane_toggle_visible: bool,

    // ── NavigationView ───────────────────────────────────────────────────
    /// The nav pane's own back arrow is enabled (`Prop::IsBackEnabled` — a
    /// different seam prop from the TitleBar's `IsBackButtonEnabled`).
    pub back_enabled: bool,
    /// The settings row is present at the foot of the pane.
    pub settings_visible: bool,
    /// The pane is expanded rather than collapsed to the icon rail.
    pub pane_open: bool,
    /// Header text drawn above the menu items.
    pub pane_title: String,
    /// `NavigationViewPaneDisplayMode` as delivered (WinRT discriminant).
    pub pane_display_mode: i32,
    /// Expanded pane width (DIP).
    pub open_pane_length: f64,
    /// The pane hosts an embedded search box.
    pub search_box: bool,
    /// Suggestions offered by that search box.
    pub suggest_items: Vec<String>,
    pub suggest_placeholder: String,

    // ── Scroll containers ────────────────────────────────────────────────
    /// `ScrollBarVisibility` per axis as delivered (WinRT discriminant).
    pub h_scrollbar: i32,
    pub v_scrollbar: i32,

    // ── Button ───────────────────────────────────────────────────────────
    /// Attached flyout. A `Str` value arrives as [`FlyoutDef::text`], so both
    /// shapes the seam sends land in the same field with nothing dropped.
    pub flyout: Option<Box<FlyoutDef>>,
    /// `FlyoutPlacementMode` as delivered (WinRT discriminant).
    pub flyout_placement: i32,
    /// Leading icon glyph codepoint (a `Symbol`'s value), 0 = none — the same
    /// encoding [`Ctrl::icons`] and [`MenuRow::icon`] already use.
    pub icon: u32,

    // ── HyperlinkButton ──────────────────────────────────────────────────
    /// Target URI (empty = none).
    pub navigate_uri: String,

    // ── ToggleSwitch ─────────────────────────────────────────────────────
    /// Labels drawn beside the track per state (empty = none).
    pub on_content: String,
    pub off_content: String,

    // ── Editors / text ───────────────────────────────────────────────────
    /// ComboBox: the closed box is a text field, not just a display.
    pub is_editable: bool,
    /// TextBox: Enter inserts a newline instead of committing.
    pub accepts_return: bool,
    /// `PasswordRevealMode` as delivered (WinRT discriminant).
    pub password_reveal_mode: i32,
    /// PasswordBox: the reveal ("eye") button is offered.
    pub password_reveal_button: bool,
    /// TextBlock: its text can be selected with the pointer.
    pub text_selectable: bool,

    // ── RepeatButton ─────────────────────────────────────────────────────
    /// Milliseconds before the first repeat, then between repeats.
    pub repeat_delay: i32,
    pub repeat_interval: i32,

    // ── InfoBar ──────────────────────────────────────────────────────────
    /// The body text drawn after the title (`Prop::Message`).
    pub message: String,
    /// `InfoBarSeverity` as delivered (WinRT discriminant) — resolved through
    /// [`info_bar::Severity::of`](super::info_bar::Severity::of), which treats
    /// an unrecognised value as informational rather than dropping the bar.
    pub severity: i32,
    /// The bar is shown. A closed bar collapses out of layout entirely
    /// (`Display::None`, applied in `layout::finalize_style`) rather than
    /// merely painting nothing, so it reclaims its space like WinUI's.
    ///
    /// Named apart from the pane's `pane_open` because they are different
    /// controls' states that would otherwise both want `open`.
    pub bar_open: bool,
    /// The bar offers its built-in close button (`Prop::IsClosable`).
    pub bar_closable: bool,
}

impl Extras {
    /// The state every node starts in, as a `const` so it can also back
    /// [`EMPTY_EXTRAS`] — the value a node with no allocated `Extras` reads as.
    /// Same single-definition discipline as [`Ctrl::DEFAULT`], and for the same
    /// reason: the absent and the untouched read must be indistinguishable.
    ///
    /// The non-empty entries are the ones a node can actually be observed at,
    /// because their widget binding is CONDITIONAL — it disappears (arriving
    /// here as `PropValue::Unset`) exactly when the widget field holds its own
    /// default. Each therefore mirrors that widget default rather than a zero:
    /// `NavigationView::default()` has `is_back_button_visible: true`,
    /// `is_pane_toggle_button_visible: true`, `is_settings_visible: true`,
    /// `is_pane_open: true` and `open_pane_length: 320.0`;
    /// `PasswordBox::default()` has `is_password_reveal_button_enabled: true`;
    /// `RepeatButton::default()` has `delay: 500, interval: 33`;
    /// `InfoBar::default()` has `is_closable: true` (and `is_open: false` —
    /// `InfoBar::new` is what opens one). The enum fields take the WinRT
    /// enumerator the unset state means, by name.
    pub const DEFAULT: Extras = Extras {
        title: String::new(),
        subtitle: String::new(),
        tall: false,
        back_button_visible: true,
        back_button_enabled: false,
        pane_toggle_visible: true,
        back_enabled: false,
        settings_visible: true,
        pane_open: true,
        pane_title: String::new(),
        pane_display_mode: NavigationViewPaneDisplayMode::Auto.0,
        open_pane_length: 320.0,
        search_box: false,
        suggest_items: Vec::new(),
        suggest_placeholder: String::new(),
        h_scrollbar: ScrollBarVisibility::Auto.0,
        v_scrollbar: ScrollBarVisibility::Auto.0,
        flyout: None,
        flyout_placement: FlyoutPlacementMode::Top.0,
        icon: 0,
        navigate_uri: String::new(),
        on_content: String::new(),
        off_content: String::new(),
        is_editable: false,
        accepts_return: false,
        password_reveal_mode: PasswordRevealMode::Peek.0,
        password_reveal_button: true,
        text_selectable: false,
        repeat_delay: 500,
        repeat_interval: 33,
        message: String::new(),
        severity: 0,
        bar_open: false,
        bar_closable: true,
    };
}

impl Default for Extras {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The lazily-boxed [`Extras`] inside a [`Ctrl`].
///
/// A newtype purely so `Debug` can tell the truth about the invariant: an
/// absent `Extras` READS as [`Extras::DEFAULT`], so it must also PRINT as it.
/// Derived on the `Option` it would print `None` where a materialised-but-
/// untouched one prints its fields, making two states that are equivalent by
/// construction look different to anything that compares node state.
#[derive(Clone, Default)]
pub(crate) struct LazyExtras(Option<Box<Extras>>);

impl std::fmt::Debug for LazyExtras {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

impl LazyExtras {
    /// The effective value — the box's contents, or the shared default.
    fn get(&self) -> &Extras {
        self.0.as_deref().unwrap_or(EMPTY_EXTRAS)
    }

    fn get_mut(&mut self) -> &mut Extras {
        self.0.get_or_insert_with(|| Box::new(Extras::DEFAULT))
    }

    /// The allocated value, or `None` — the reset path's view.
    fn get_opt_mut(&mut self) -> Option<&mut Extras> {
        self.0.as_deref_mut()
    }

    #[cfg(feature = "test")]
    fn allocated(&self) -> bool {
        self.0.is_some()
    }
}

/// The [`Extras`] a node that has never had any written reads as.
///
/// An inline `const` block rather than a `static` like [`EMPTY_CTRL`]: a
/// `static` must be `Sync`, and [`FlyoutDef`] carries the app's `Rc` callbacks
/// and element tree, which are not. The block still yields one `&'static` the
/// whole process shares, and every heap field in [`Extras::DEFAULT`] is an
/// empty `Vec`/`String`/`None`, so it owns no allocation.
pub(crate) const EMPTY_EXTRAS: &Extras = &Extras::DEFAULT;

/// Control-specific state, distinct from generic layout/paint. Populated by
/// `set_prop` for the stateful drawn controls (toggle, slider, segmented, …).
#[derive(Clone, Debug)]
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
    /// SelectorBar: the segment currently under the pointer (-1 = none) — the
    /// hovered segment gets its own ink wash and brighter label.
    pub hot_index: i32,
    /// SelectorBar: measured label width per item (DIPs, parallel to `items`),
    /// written by the text-rebuild pass — segments size to their own label.
    pub seg_label_w: Vec<f32>,
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
    /// Slider: fill origin in value units (`None` = fill from `min`). An
    /// origin strictly inside the range fills bidirectionally out from it and
    /// paints a neutral tick notch on the track.
    pub fill_origin: Option<f64>,
    /// Slider: fill color at or below the origin (`None` = theme accent).
    /// Authored linear scRGB, display-mapped at the draw choke.
    pub fill_color: Option<Color>,
    /// Slider: fill color above the origin (`None` = same as `fill_color`).
    pub fill_color_alt: Option<Color>,
    /// Meter: reference marker hairline position in value units.
    pub marker: Option<f64>,
    /// Meter: marker hairline color (`None` = a neutral tick).
    pub marker_color: Option<Color>,
    /// Meter fill / Knob arc: gradient stops `(position 0..1, authored color)`.
    pub stops: Vec<(f64, Color)>,
    /// Knob: sweep start / end angle (radians, canvas convention: 0 = east,
    /// clockwise on a y-down surface).
    pub start_angle: f32,
    pub end_angle: f32,
    /// Knob: tick-mark positions (value units).
    pub ticks: Vec<f64>,
    /// Knob: `(value, label)` scale labels (labels formatted by the app).
    pub tick_labels: Vec<(f64, String)>,
    /// Knob: ticks whose value is an exact multiple draw longer/brighter.
    pub major_every: Option<f64>,
    /// Knob: per-value accent color for the value arc / needle glow (`None` =
    /// theme accent). Authored linear scRGB, display-mapped at the draw choke.
    pub accent: Option<Color>,
    /// Knob: small unit string under the center readout (e.g. `"dB"`).
    pub unit: String,
    /// Knob: optional sub-line under the unit (e.g. a linear multiplier).
    pub sub_text: String,
    /// InfoBadge: the count it carries, or `None` for the bare status dot.
    /// An `Option` rather than a sentinel because `0` is a legitimate count
    /// and must not be indistinguishable from "no value" — the widget's two
    /// constructors (`InfoBadge::dot` / `::numeric`) are exactly this choice.
    pub badge_value: Option<i32>,
    /// The chrome-heavy state of the caption / nav-pane / flyout / editor-policy
    /// props, allocated on the first write of any of them. Absent on every node
    /// that carries none — which is nearly every node that carries a `Ctrl` at
    /// all. Read it through [`Node::extras`], write it through
    /// [`Node::extras_mut`]; like `Ctrl` itself the field is deliberately not
    /// reachable directly, so an absent `Extras` cannot be mistaken for a
    /// present one.
    extras: LazyExtras,
}

impl Ctrl {
    /// The state every control starts in, as a `const` so it can also back
    /// [`EMPTY_CTRL`] — the value a node with no allocated [`Ctrl`] reads as.
    ///
    /// This constant is the SINGLE definition: [`Default`] returns it and the
    /// absent-read path returns a reference to it. A node that has never been
    /// written therefore reads exactly what an eagerly-constructed `Ctrl` would
    /// have held, and the two cannot drift apart because there is only one.
    pub const DEFAULT: Ctrl = Ctrl {
        is_on: false,
        is_checked: false,
        value: 0.0,
        min: 0.0,
        max: 100.0,
        step: None,
        indeterminate: false,
        is_active: true,
        selected_index: -1,
        hot_index: -1,
        seg_label_w: Vec::new(),
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
        fill_origin: None,
        fill_color: None,
        fill_color_alt: None,
        marker: None,
        marker_color: None,
        stops: Vec::new(),
        start_angle: 0.0,
        end_angle: 0.0,
        ticks: Vec::new(),
        tick_labels: Vec::new(),
        major_every: None,
        accent: None,
        unit: String::new(),
        sub_text: String::new(),
        badge_value: None,
        extras: LazyExtras(None),
    };

    /// This node's [`Extras`] for reading — [`EMPTY_EXTRAS`] when none has been
    /// allocated, which IS [`Extras::DEFAULT`], so absent and untouched read
    /// identically.
    ///
    /// Unused until the caption / nav-pane / flyout chrome is actually drawn —
    /// that is what "stored, not yet drawn" means, and the read path is here
    /// so the drawing lands as a paint change and nothing else.
    #[allow(dead_code)]
    pub fn extras(&self) -> &Extras {
        self.extras.get()
    }

    /// This node's [`Extras`] for writing, allocated on first use.
    pub fn extras_mut(&mut self) -> &mut Extras {
        self.extras.get_mut()
    }

    /// The allocated [`Extras`], or `None` — the reset path's view, which must
    /// never materialise one (see [`Node::extras_reset`]).
    fn extras_opt_mut(&mut self) -> Option<&mut Extras> {
        self.extras.get_opt_mut()
    }

    /// Whether the [`Extras`] box has actually been allocated (test seam only).
    #[cfg(feature = "test")]
    fn extras_allocated(&self) -> bool {
        self.extras.allocated()
    }
}

impl Default for Ctrl {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The [`Ctrl`] a node that has never had control state written reads as.
///
/// An inline `const` rather than a `static` for the reason given on
/// [`EMPTY_EXTRAS`]: `Ctrl` reaches a [`FlyoutDef`] through its `Extras`, and
/// that is not `Sync`. Every heap field in [`Ctrl::DEFAULT`] is an empty
/// `Vec`/`String`/`None`, so this owns no allocation either way, and no caller
/// depends on it having one address — it is only ever read through.
pub(crate) const EMPTY_CTRL: &Ctrl = &Ctrl::DEFAULT;

/// One live control.
pub(crate) struct Node {
    pub kind: ControlKind,
    /// Taffy layout inputs, mutated in place by `set_prop`.
    pub style: taffy::Style,
    pub children: Vec<ControlId>,
    /// The node currently listing this one in its `children`, or `None` for the
    /// root and for a node that is momentarily unparented (the reconciler
    /// legitimately produces such states mid-diff — see `RecordingBackend::flush`).
    ///
    /// Maintained as the exact inverse of `children` by every mutator in
    /// [`mod`](super) that changes WHICH list a node is in — `append_child`,
    /// `insert_child`, `remove_child`, `replace_child`, `set_title_slot`, and
    /// `destroy`. (`move_child` only reorders one list, so every node keeps its
    /// parent.) It exists so the UIA provider can answer parent/ancestor queries
    /// in O(depth) instead of a DFS from the root per call; a client `FindAll`
    /// walk was O(n²) without it. Ids are never reused (see [`Arena`]), so a
    /// stored id can never alias a different node — a parent that has since been
    /// destroyed simply fails the arena lookup.
    pub parent: Option<ControlId>,
    pub paint: Paint,
    /// StackPanel spacing (DIPs); applied to the Taffy gap on the main axis.
    pub spacing: f32,
    /// Grid track templates (applied at layout time for a `Grid`).
    pub grid_rows: Vec<GridLength>,
    pub grid_cols: Vec<GridLength>,
    /// Cached DWrite layout for text-bearing nodes; rebuilt on text/font change.
    pub text_layout: Option<TextLayout>,
    pub text_dirty: bool,
    /// What the app has declared this control responds to.
    ///
    /// A declaration, not a closure: the `EventHandler`s themselves live
    /// app-side in the recorder's handler map and are invoked from queued
    /// intents (`record.rs`). Hit-testing and `WM_NCHITTEST` only need to know
    /// that one exists, and once replay crosses to the front thread these
    /// flags are all the input side keeps.
    pub interactivity: Interactivity,
    /// Which per-element pointer callbacks exist — same declaration-not-closure
    /// split as `interactivity`; see [`PointerInterest`].
    pub pointer: PointerInterest,
    /// Monotonic revision of `ctrl.value`, bumped on every **input-originated**
    /// write (drag, wheel, keyboard nudge, NumberBox commit, UIA SetValue).
    /// Rides out on `ValueChanged` intents; the app's echo comes back stamped
    /// with the revision it was based on, and a stale echo is dropped instead
    /// of dragging the chrome backwards after the gesture has moved on — the
    /// §7.2 revision protocol, extended from text to control values.
    pub value_rev: u64,
    pub accessibility: Option<AccessibilityModifiers>,

    // ── Composition ──────────────────────────────────────────────────────
    /// This node's container visual (always present); mirrors the logical tree.
    pub container: ContainerVisual,
    /// Cached `IVisual` view of `container` for frequent offset/size/opacity ops.
    pub vis: IVisual,
    /// Painted-chrome surface — created lazily for nodes that draw something.
    pub surf: Option<NodeSurface>,
    /// Retained chrome parts (indicator pill / toggle knob / slider fill /
    /// hover ink) for the converted control kinds — compositor sprites whose
    /// motion runs DWM-side. Created lazily by the parts sync; `None` for
    /// every other node. See [`parts`](super::parts).
    pub parts: Option<Box<parts::Parts>>,
    /// Editors only: the caret sprite (topmost child, above the painted text)
    /// whose blink is a compositor-side square-wave opacity animation. Created
    /// lazily on first focused paint; see [`parts::sync_caret`].
    pub caret: Option<parts::Caret>,
    /// Knob only: the value-arc shape + needle (retained compositor vector
    /// chrome grown by a `TrimEnd` spring). Created lazily on first paint; see
    /// [`knob::KnobParts`](super::knob::KnobParts).
    pub knob: Option<Box<super::knob::KnobParts>>,
    /// ScrollViewer only: the auto-hiding overlay scrollbar thumb sprite (a top
    /// child of the container, above the scrolled content), created lazily.
    pub scroll_thumb: Option<NodeSurface>,
    /// ScrollViewer only: the content **carrier** visual all scrolled children
    /// parent into (created with the node). Scrolling animates this ONE
    /// visual's Offset with a compositor spring — no per-frame tick, no
    /// per-child writes while the glide plays.
    pub scroll_content: Option<ContainerVisual>,
    /// Cached retargetable compositor spring driving `scroll_content`'s Offset
    /// (built on first glide; a wheel retarget is `SetFinalValue` + start).
    pub scroll_spring: Option<crate::system_bindings::SpringVector3NaturalMotionAnimation>,
    /// Cached spring for the thumb sprite's Offset (same tuning, so the thumb
    /// tracks the content glide proportionally).
    pub thumb_spring: Option<crate::system_bindings::SpringVector3NaturalMotionAnimation>,
    /// Last scroll offset written/targeted on the carrier (gates no-op writes;
    /// `None` until first placement).
    pub last_scroll: Option<f32>,
    /// Whether the thumb is currently revealed (hover/drag/scroll-in-flight).
    /// The show/hide fade itself plays on the system compositor
    /// ([`animate::fade_thumb`](super::animate::fade_thumb)) — this is only the
    /// edge detector that triggers it, never a per-frame value.
    pub thumb_shown: bool,
    /// Thumb height (DIP) the thumb surface was last drawn at (redraw on change).
    pub thumb_drawn_h: f32,
    /// While dragging the thumb: the pointer-to-thumb-top offset captured at press.
    pub thumb_drag: Option<f32>,
    /// Bounds clip (ScrollViewer/overflow); tracks the container's own size.
    pub clip: Option<InsetClip>,

    // ── Compositor animations (all DWM-evaluated — zero app ticks) ───────
    /// Declared implicit property transitions (opacity/scale/rotation/
    /// translation), kept so the merged collection can be rebuilt.
    pub transitions: Option<ImplicitTransitions>,
    /// Declared layout (offset/size) glide, merged into the same collection.
    pub layout_anim: Option<LayoutAnimationConfig>,
    /// The merged implicit-animation collection built from the two above.
    pub implicit: Option<ImplicitAnimationCollection>,
    /// Whether `implicit` is attached to the container. Attachment is deferred
    /// to the first layout write so initial placement never plays as a fly-in
    /// from the visual's zeroed defaults.
    pub implicit_attached: bool,
    /// Exit transition played on a detached "ghost" of this container when the
    /// node is destroyed (see `DCompBackend::destroy`).
    pub exit: Option<AnimationConfig>,
    /// Cached explicit-start spring for offset glides (built once per config;
    /// cleared when `layout_anim` changes so damping/period rebuild).
    pub spring_anim: Option<crate::system_bindings::SpringVector3NaturalMotionAnimation>,
    /// Keep `CenterPoint` at `size/2` — set once any animation touches scale,
    /// so scale effects pivot around the node centre at every size.
    pub wants_center: bool,
    /// Last offset/size pushed to the visual. Gates the COM writes so an
    /// unchanged layout pass costs nothing and never re-triggers an implicit
    /// animation.
    pub last_off: Option<(f32, f32)>,
    pub last_size: Option<(f32, f32)>,
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

    /// The Taffy node this maps to, PERSISTENT across layout passes, stamped
    /// with the generation of the [`LayoutTree`](super::layout::LayoutTree)
    /// that minted it. Taffy indexes its slotmap unchecked, so an id from a
    /// tree that no longer exists must never be dereferenced — the stamp makes
    /// that unrepresentable rather than merely unlikely: a mismatch reads as
    /// "no Taffy node yet" and the node is re-created.
    pub taffy_id: Option<(u32, taffy::NodeId)>,
    /// Something Taffy cannot see (a rebuilt DirectWrite layout) invalidated
    /// this node's cached intrinsic measurement. Taffy keys its measure cache
    /// on constraints alone, so without this a relabelled text node keeps its
    /// old size forever. Cleared by the layout tree sync that marks it dirty.
    pub measure_dirty: bool,
    /// The visuals the composition child-order sync last parented under this
    /// node, with the collection each went into — see
    /// [`layout::sync`](super::layout). A re-stack detaches exactly this set
    /// and nothing else, so a sprite parented in elsewhere (a Knob's arc and
    /// needle, an editor's caret) is never torn out from under its owner.
    pub stacked: Vec<(layout::Slot, Visual)>,
    pub rect: LaidRect,
    pub hovered: bool,
    pub pressed: bool,

    // ── Control library state ────────────────────────────────────────────
    /// Stateful drawn-control data (toggle/slider/segmented/select/nav/…),
    /// allocated on the first write and absent until then.
    ///
    /// [`Ctrl`] is the largest per-node payload after the Taffy style, yet a
    /// real tree is mostly `TextBlock`/`Border`/`Grid`/`StackPanel` — kinds that
    /// never hold any of it. Boxing it keeps that majority out of the arena's
    /// working set entirely. Read it through [`Node::ctrl`] and write it through
    /// [`Node::ctrl_mut`]; the field is deliberately not accessed directly so an
    /// absent `Ctrl` cannot be mistaken for a present one.
    ctrl: Option<Box<Ctrl>>,
    /// ScrollViewer only: the LOGICAL scroll offset (DIPs of content above the
    /// viewport). Hit-testing and thumb geometry read this; the VISUAL glide
    /// toward it plays on the compositor (`scroll_glide`), so during a wheel
    /// glide it already holds the destination.
    pub scroll_off: f32,
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
    /// TitleBar only: the band's own cached title/subtitle layouts. Boxed and
    /// lazy for the same reason [`Extras`] is — a tree holds at most one or two
    /// TitleBars, and every other node would carry the dead weight. Rebuilt by
    /// the layout pass on `text_dirty`; `None` when the band has no titles.
    pub caption_text: Option<Box<caption::CaptionText>>,

    // ── NavigationView pane ────────────────────────────────────────
    /// NavigationView only: the pane's cached header / item / settings label
    /// layouts. Boxed and lazy for the reason [`Extras`] is — a tree holds at
    /// most one or two nav shells, and every other node would carry the dead
    /// weight. Rebuilt by the layout pass on `text_dirty`; `None` when the pane
    /// has no text at all (a glyph-only rail).
    pub nav_text: Option<Box<nav::NavPaneText>>,

    // ── InfoBar ──────────────────────────────────────────────────────────
    /// InfoBar only: the band's cached title + message paragraph. Boxed and
    /// lazy for the reason [`Extras`] is. Rebuilt by the layout pass on
    /// `text_dirty`; `None` when the bar carries no text at all.
    pub bar_text: Option<Box<info_bar::InfoBarText>>,
}

impl Node {
    pub fn new(kind: ControlKind, container: ContainerVisual) -> Self {
        let vis: IVisual = container.cast().expect("ContainerVisual is an IVisual");
        let paint = birth_paint(kind);
        let focusable = is_focusable_kind(kind);
        Self {
            kind,
            style: default_style(kind),
            children: Vec::new(),
            parent: None,
            paint,
            spacing: 0.0,
            grid_rows: Vec::new(),
            grid_cols: Vec::new(),
            text_layout: None,
            text_dirty: true,
            interactivity: Interactivity::default(),
            pointer: PointerInterest::default(),
            value_rev: 0,
            accessibility: None,
            container,
            vis,
            surf: None,
            parts: None,
            caret: None,
            knob: None,
            scroll_thumb: None,
            scroll_content: None,
            scroll_spring: None,
            thumb_spring: None,
            last_scroll: None,
            thumb_shown: false,
            thumb_drawn_h: 0.0,
            thumb_drag: None,
            clip: None,
            transitions: None,
            layout_anim: None,
            implicit: None,
            implicit_attached: false,
            exit: None,
            spring_anim: None,
            wants_center: false,
            last_off: None,
            last_size: None,
            h_align: ALIGN_UNSET,
            v_align: ALIGN_UNSET,
            z_index: 0,
            z_dirty: false,
            children_dirty: false,
            dirty: true,
            taffy_id: None,
            measure_dirty: true,
            stacked: Vec::new(),
            rect: LaidRect::default(),
            hovered: false,
            pressed: false,
            ctrl: None,
            scroll_off: 0.0,
            focusable,
            focused: false,
            editor: is_text_editable(kind).then(|| Editor::new(kind)),
            title_content: None,
            title_footer: None,
            caption_text: None,
            nav_text: None,
            bar_text: None,
        }
    }

    /// This node's control state for reading. A node that has never had any
    /// written reads [`EMPTY_CTRL`] — which IS [`Ctrl::DEFAULT`], the same value
    /// an eagerly-allocated `Ctrl` was constructed with, so an absent `Ctrl` and
    /// an untouched one are indistinguishable to every reader.
    pub fn ctrl(&self) -> &Ctrl {
        self.ctrl.as_deref().unwrap_or(EMPTY_CTRL)
    }

    /// This node's control state for writing, allocated on first use.
    pub fn ctrl_mut(&mut self) -> &mut Ctrl {
        self.ctrl.get_or_insert_with(|| Box::new(Ctrl::DEFAULT))
    }

    /// Restore part of the control state to its birth value — but only if a
    /// [`Ctrl`] was ever allocated.
    ///
    /// A reset writes the value a never-written node holds, and a node with no
    /// `Ctrl` already READS exactly that ([`EMPTY_CTRL`]). Going through
    /// [`Self::ctrl_mut`] here would allocate several hundred bytes to store a
    /// value that is already in effect — undoing the lazy box for any node the
    /// reconciler happens to diff a prop away from. The repaint is marked only
    /// when there was something to change.
    pub fn ctrl_reset(&mut self, f: impl FnOnce(&mut Ctrl)) {
        if let Some(c) = self.ctrl.as_deref_mut() {
            f(c);
            self.dirty = true;
        }
    }

    /// This node's [`Extras`] for reading (see [`Ctrl::extras`]) — the entry
    /// point the drawing side will use. Dead until then, by construction.
    #[allow(dead_code)]
    pub fn extras(&self) -> &Extras {
        self.ctrl().extras()
    }

    /// This node's [`Extras`] for writing, allocating both boxes on first use.
    pub fn extras_mut(&mut self) -> &mut Extras {
        self.ctrl_mut().extras_mut()
    }

    /// [`Self::ctrl_reset`] for the [`Extras`] tier — same rule, same reason:
    /// an absent `Extras` already reads its birth value, so a reset that
    /// materialises one has only spent memory.
    pub fn extras_reset(&mut self, f: impl FnOnce(&mut Extras)) {
        if let Some(x) = self.ctrl.as_deref_mut().and_then(Ctrl::extras_opt_mut) {
            f(x);
            self.dirty = true;
        }
    }

    /// The [`Paint`] this node was born with — what a prop reset must restore.
    /// Built by the same function [`Node::new`] builds it with, so the two
    /// cannot disagree about a per-kind default (a Button's 6 DIP corner
    /// radius, a compact kind's smaller font).
    pub fn birth_paint(&self) -> Paint {
        birth_paint(self.kind)
    }

    /// The Taffy style this node was born with — likewise the reset target for
    /// every layout prop. Not all-zero: a Button is born with padding and a
    /// minimum height, an editor and a TitleBar with a minimum height, a
    /// NavigationView with the icon rail's left padding.
    pub fn birth_style(&self) -> taffy::Style {
        default_style(self.kind)
    }

    /// Whether the boxed [`Ctrl`] has actually been allocated. Only the test
    /// seam asks — the backend proper never distinguishes the two states.
    #[cfg(feature = "test")]
    pub fn ctrl_allocated(&self) -> bool {
        self.ctrl.is_some()
    }

    /// Whether the [`Extras`] tier inside the boxed [`Ctrl`] has been
    /// allocated. Only the test seam asks — see [`Self::ctrl_allocated`].
    #[cfg(feature = "test")]
    pub fn extras_allocated(&self) -> bool {
        self.ctrl.as_deref().is_some_and(Ctrl::extras_allocated)
    }

    /// The control state and the retained chrome parts as two disjoint borrows.
    ///
    /// Two field accesses can be split by the borrow checker; a method call
    /// borrowing the whole `Node` cannot. A parts sync that reads `ctrl` while
    /// mutating `parts` goes through here instead of cloning either.
    pub fn ctrl_and_parts(&mut self) -> (&Ctrl, Option<&mut parts::Parts>) {
        (
            self.ctrl.as_deref().unwrap_or(EMPTY_CTRL),
            self.parts.as_deref_mut(),
        )
    }

    /// Record that `event` gained or lost a handler.
    pub fn note_interactivity(&mut self, event: Event, attached: bool) {
        match event {
            Event::Click => self.interactivity.click = attached,
            Event::BackRequested => self.interactivity.back = attached,
            // Every other event is dispatch-only: nothing consults its presence
            // to decide whether the control participates in input.
            _ => {}
        }
    }

    /// Whether an app value write stamped `based_on` may still apply — the
    /// revision half of the §7.2 gate for control values. `false` means the
    /// user drove the value after the app last heard about it
    /// (`ValueChanged` intents deliver `value_rev`), so the write is a stale
    /// echo: applying it would snap the chrome backwards, and the app
    /// converges through the newer intent instead.
    pub fn accepts_value_echo(&self, based_on: u64) -> bool {
        based_on >= self.value_rev
    }

    /// True for nodes that respond to a press (hover/press ink + activate).
    pub fn is_clickable(&self) -> bool {
        is_interactive_kind(self.kind)
            || is_text_editable(self.kind)
            || self.interactivity.click
            || self.pointer.tapped
            || self.pointer.pressed
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

    /// Push the visual's parent-relative offset, skipping the COM write when
    /// unchanged. All offset writers (layout, scroll) route through here: the
    /// gate keeps an unchanged pass free AND keeps implicit Offset animations
    /// from re-triggering on a no-op set. The node's first-ever write also
    /// attaches any pending implicit collection — *after* the position lands,
    /// so mounting never animates from (0, 0).
    ///
    /// A spring layout glide replaces the property set entirely: the move is
    /// handed to an explicit compositor spring targeting the new offset
    /// ([`animate::spring_offset`](super::animate::spring_offset)). Initial
    /// placement always SETS — mounting must never fly in.
    pub fn push_offset(&mut self, x: f32, y: f32) {
        if self.last_off == Some((x, y)) {
            return;
        }
        let first = self.last_off.is_none();
        if !first
            && let Some(l) = self.layout_anim.filter(|l| l.animate_offset && l.use_spring)
            && let Ok(obj) = self.container.cast::<ICompositionObject>()
            && super::animate::spring_offset(
                &obj,
                &mut self.spring_anim,
                x,
                y,
                l.damping_ratio,
                l.period,
            )
            .is_ok()
        {
            self.last_off = Some((x, y));
            return;
        }
        let _ = self.vis.SetOffset(Vector3::new(x, y, 0.0));
        self.last_off = Some((x, y));
        if first {
            self.attach_implicit_if_ready();
        }
    }

    /// Push the visual's size, skipping the COM write when unchanged; keeps
    /// the scale pivot centred while any animation wants one.
    pub fn push_size(&mut self, w: f32, h: f32) {
        if self.last_size == Some((w, h)) {
            return;
        }
        let _ = self.vis.SetSize(Vector2::new(w, h));
        self.last_size = Some((w, h));
        if self.wants_center {
            let _ = self
                .vis
                .SetCenterPoint(Vector3::new(w / 2.0, h / 2.0, 0.0));
        }
    }

    // ── Scroll carrier (compositor-side scrolling) ───────────────────────

    /// Snap the scroll carrier to `offset` (content moved up by `offset` DIPs):
    /// stop any in-flight glide, then a plain property set. Used by layout
    /// (placement, not motion) and 1:1 thumb drags. Gated on the last written
    /// target so an unchanged pass costs nothing — and never interrupts a glide
    /// already heading to the same place.
    pub fn scroll_snap(&mut self, offset: f32) {
        if self.last_scroll == Some(offset) {
            return;
        }
        if let Some(c) = &self.scroll_content {
            if let Ok(o) = c.cast::<ICompositionObject>() {
                let _ = o.StopAnimation("Offset");
            }
            if let Ok(v) = c.cast::<IVisual>() {
                let _ = v.SetOffset(Vector3::new(0.0, -offset, 0.0));
            }
        }
        self.last_scroll = Some(offset);
    }

    /// Spring-glide the scroll carrier to `offset` on the compositor (wheel
    /// scrolling). Retargets smoothly mid-flight; first placement snaps.
    pub fn scroll_glide(&mut self, offset: f32) {
        if self.last_scroll == Some(offset) {
            return;
        }
        if self.last_scroll.is_none() {
            self.scroll_snap(offset);
            return;
        }
        if let Some(c) = &self.scroll_content
            && let Ok(o) = c.cast::<ICompositionObject>()
            && animate::spring_offset(
                &o,
                &mut self.scroll_spring,
                0.0,
                -offset,
                parts::SPRING_DAMPING,
                parts::SPRING_PERIOD,
            )
            .is_ok()
        {
            self.last_scroll = Some(offset);
            return;
        }
        self.scroll_snap(offset);
    }

    /// Spring-glide the overlay thumb sprite to `(x, y)` — same tuning as the
    /// carrier, so the thumb rides the content glide.
    pub fn thumb_glide(&mut self, x: f32, y: f32) {
        if let Some(t) = &self.scroll_thumb
            && let Ok(o) = t.sprite.cast::<ICompositionObject>()
        {
            let _ = animate::spring_offset(
                &o,
                &mut self.thumb_spring,
                x,
                y,
                parts::SPRING_DAMPING,
                parts::SPRING_PERIOD,
            );
        }
    }

    /// Snap the thumb sprite to `(x, y)` (1:1 drag tracking), stopping any
    /// in-flight glide first — a plain set while an animation holds the
    /// property would be ignored.
    pub fn thumb_snap(&mut self, x: f32, y: f32) {
        if let Some(t) = &self.scroll_thumb {
            if let Ok(o) = t.sprite.cast::<ICompositionObject>() {
                let _ = o.StopAnimation("Offset");
            }
            t.set_offset(x, y);
        }
    }

    /// Attach the merged implicit collection once the node has a real laid-out
    /// position. No-op when nothing is pending or the node hasn't laid out.
    pub fn attach_implicit_if_ready(&mut self) {
        if self.implicit_attached || self.last_off.is_none() {
            return;
        }
        if let Some(coll) = &self.implicit
            && let Ok(o2) = self.container.cast::<ICompositionObject2>()
            && o2.SetImplicitAnimations(coll).is_ok()
        {
            self.implicit_attached = true;
        }
    }

    /// Install a freshly merged implicit collection (or clear it). Swaps in
    /// place when the node is already laid out; otherwise attachment defers to
    /// the first layout write (see [`Self::push_offset`]).
    pub fn set_implicit(&mut self, coll: Option<ImplicitAnimationCollection>) {
        self.implicit = coll;
        if self.implicit.is_none() {
            if self.implicit_attached
                && let Ok(o2) = self.container.cast::<ICompositionObject2>()
            {
                let _ = o2.SetImplicitAnimations(None);
            }
            self.implicit_attached = false;
            return;
        }
        self.implicit_attached = false;
        self.attach_implicit_if_ready();
    }
}

/// No alignment requested — the value [`Node::h_align`] / [`Node::v_align`] are
/// born with and the one a reset restores. Not zero: 0 is a real WinRT
/// `HorizontalAlignment::Left`.
pub(crate) const ALIGN_UNSET: i32 = -1;

/// The [`Paint`] a node of `kind` is born with.
///
/// The single definition of those defaults: [`Node::new`] builds the node's
/// paint with it and [`Node::birth_paint`] hands it to the prop-reset path, so
/// "what an unset value means" is one fact stated once. Deliberately not all
/// default: text is laid out at a real size, drawn at a real weight, and
/// controls are enabled — resetting any of those to zero would render a node
/// invisible, hairline-thin, or greyed out.
pub(crate) fn birth_paint(kind: ControlKind) -> Paint {
    Paint {
        font_size: default_font_size(kind),
        font_weight: 400,
        is_enabled: true,
        // Buttons draw their own chrome (the WinUI default style is gone here).
        corner_radius: if kind == ControlKind::Button { 6.0 } else { 0.0 },
        ..Paint::default()
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
            | ControlKind::Knob
            | ControlKind::ComboBox
            | ControlKind::DropDownButton
            | ControlKind::SplitButton
            | ControlKind::NavigationView
            | ControlKind::Expander
            // The band itself is inert, but its drawn close button is not, and
            // a node has to be hit-testable as a whole before any part of it
            // can be. It takes no ink and no focus ring: `parts::converted`
            // excludes it, and it is deliberately absent from the Tab ring
            // (see `is_focusable_kind`).
            | ControlKind::InfoBar
    )
}

/// Kinds that always own a paint surface (they draw chrome unconditionally).
fn draws_own_chrome(kind: ControlKind) -> bool {
    is_interactive_kind(kind)
        || is_text_editable(kind)
        || matches!(
            kind,
            ControlKind::ProgressBar
                | ControlKind::ProgressRing
                | ControlKind::TitleBar
                | ControlKind::Meter
                | ControlKind::InfoBadge
        )
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
///
/// `InfoBar` is deliberately absent even though it is interactive. Focus here
/// is per NODE, so focusing a bar would ring the whole band and put a large
/// informational strip — one that is very often not closable, and then has no
/// action at all — into the Tab order. Its close button is instead reachable
/// the way the caption cluster's is: as an invokable accessibility element
/// (see `uia::INFOBAR_CLOSE_ITEM`).
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
            | ControlKind::Knob
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
///
/// Also the reset target for every layout prop, via [`Node::birth_style`].
pub(crate) fn default_style(kind: ControlKind) -> taffy::Style {
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
            // The pane is drawn on the node's own surface; the content child is
            // inset past it. The inset is derived from `nav` rather than
            // spelled out here, because the layout pass re-derives it whenever
            // the pane state changes and the two must agree exactly — see
            // `nav::pane_width`. A virgin NavigationView resolves to WinUI's own
            // default: `is_pane_open` true at `open_pane_length` 320.
            //
            // Grid, not Flex: the content must FILL the space beside the pane
            // (WinUI's content frame does), and grid items default to
            // align/justify-self Stretch. A flex child has no grow, so content
            // would size to its own width and hug the pane.
            s.display = Display::Grid;
            s.padding.left = length(nav::pane_width(&Extras::DEFAULT, 0.0));
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
        ControlKind::ToggleSwitch => {
            // Drawn control with no children: without an intrinsic size it
            // measures 0×0 in a flex row and vanishes. Matches the painted
            // 40×20 track (`paint_toggle_switch`).
            s.display = Display::Flex;
            s.min_size = Size {
                width: length(40.0),
                height: length(20.0),
            };
        }
        ControlKind::CheckBox => {
            // Intrinsic 18×18 painted box (`paint_check_box`).
            s.display = Display::Flex;
            s.min_size = Size {
                width: length(18.0),
                height: length(18.0),
            };
        }
        ControlKind::Slider => {
            // Drawn control with no children: without an intrinsic height it
            // measures 0 tall in a flex row and vanishes. The thumb diameter,
            // not the (larger) hover halo, so a host can run compact rows —
            // the halo is an unclipped sprite and may overhang a tight box.
            s.display = Display::Flex;
            s.min_size.height = length(theme::SLIDER_THUMB);
        }
        ControlKind::Meter => {
            // Drawn control with no children: intrinsic bar height so a bare
            // meter doesn't measure 0 tall in a flex row.
            s.display = Display::Flex;
            s.min_size.height = length(theme::METER_H);
        }
        ControlKind::Knob => {
            // A square dial with no children — give it an intrinsic box so it
            // doesn't collapse in a flex row; the host normally sizes it larger.
            s.display = Display::Flex;
            s.min_size = Size {
                width: length(theme::KNOB_D),
                height: length(theme::KNOB_D),
            };
        }
        ControlKind::InfoBar => {
            // The whole band — card, icon, paragraph and close button — is
            // drawn on the node's own surface; the control takes no children.
            // Its HEIGHT is a function of its width (the paragraph wraps), so
            // unlike every other drawn control it cannot state one here: the
            // measure callback asks `info_bar::measure` per pass, and this
            // minimum is only the single-line floor that keeps a bar with no
            // text yet from measuring zero.
            s.display = Display::Block;
            s.min_size.height = length(info_bar::MIN_H);
        }
        ControlKind::InfoBadge => {
            // A dot or a small numeric pill, drawn with no children. Both axes
            // need an intrinsic size or it vanishes in a flex row; the numeric
            // form widens past this from its own measure.
            s.display = Display::Flex;
            s.min_size = Size {
                width: length(info_badge::DOT_D),
                height: length(info_badge::DOT_D),
            };
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
            // Caption height and side padding, both reserving the band's own
            // drawn chrome: the trailing min/max/close cluster (the frame is
            // extended; the buttons are ours) and the leading back button.
            //
            // The leading pair is derived from `caption` rather than spelled
            // out here, because the layout pass re-derives it whenever the
            // caption state changes and the two must agree exactly — see
            // `caption::pad_left`.
            s.min_size.height = length(caption::band_height(&Extras::DEFAULT));
            s.padding.left = length(caption::pad_left(&Extras::DEFAULT));
            s.padding.right = length(theme::SPACE_16 + caption::CLUSTER_W);
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















/// Node arena keyed by [`ControlId`] (a `NonZeroU32`).
///
/// Ids arrive already minted, from the reconciler's single monotonic counter,
/// and are NEVER reused. The reconciler tracks nodes by id across an
/// unmount/mount sequence (`children_mirror`, the `new_id != old_id` graft
/// check after a component remount), so a recycled id would alias a destroyed
/// node and silently corrupt the diff — the freshly mounted subtree is never
/// grafted and the destroyed subtree's visuals stay on screen. Map storage
/// (vs. slots) releases a removed node's memory under remount churn.
#[derive(Default)]
pub(crate) struct Arena {
    nodes: rustc_hash::FxHashMap<u32, Node>,
    next: u32,
    /// Persistent layout state — the Taffy tree, kept in lock-step with these
    /// nodes across passes instead of rebuilt per pass. Lives with the arena
    /// because its lifetime is exactly the arena's; see
    /// [`layout::compute`](super::layout::compute), which takes it out for the
    /// duration of a pass (the measure callback needs `&Arena` while Taffy
    /// holds `&mut TaffyTree`) and puts it back at the end.
    pub(crate) layout: Option<layout::LayoutTree>,
}

impl Arena {
    /// Insert at the id the reconciler minted. Ids arrive from one monotonic,
    /// never-reused source, so the arena assigns none of its own.
    pub fn insert_with_id(&mut self, id: ControlId, node: Node) {
        self.nodes.insert(id.get(), node);
    }

    pub fn get(&self, id: ControlId) -> Option<&Node> {
        self.nodes.get(&id.get())
    }

    pub fn get_mut(&mut self, id: ControlId) -> Option<&mut Node> {
        self.nodes.get_mut(&id.get())
    }

    /// Iterate every live node mutably (order unspecified).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.nodes.values_mut()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ControlId, &Node)> {
        self.nodes.iter().map(|(k, n)| (ControlId::new(*k), n))
    }

    pub fn remove(&mut self, id: ControlId) -> Option<Node> {
        self.nodes.remove(&id.get())
    }
}
