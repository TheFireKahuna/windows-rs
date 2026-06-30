//! The retained node arena: one [`Node`] per live [`ControlId`], holding its
//! Taffy layout inputs, its painted content, children, handlers, and the small
//! amount of interaction state (hover/press springs) the spine animates.
//!
//! The arena is the single source of truth. Taffy trees are rebuilt from it each
//! layout pass (the spine's trees are small and this keeps node identity simple),
//! and the composition surface is repainted from it each change.

use super::*;
use crate::backend::{ControlKind, Event, EventHandler};
use crate::style::{AccessibilityModifiers, PointerHandlers};
use crate::LineEndpoints;
use crate::Color;
use windows_canvas_core::{ColorF, TextLayout};

/// sRGB 8-bit channel -> linear. The single ingestion decode: every reactor
/// [`Color`] is authored in sRGB and lands in the FP16 scRGB surface linearly.
fn s2l(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert a reactor (sRGB) [`Color`] to a linear scRGB [`ColorF`] for D2D.
pub(crate) fn linear(c: Color) -> ColorF {
    ColorF::new(s2l(c.r), s2l(c.g), s2l(c.b), c.a as f32 / 255.0)
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
    /// Button accent/subtle/etc. variant (0 = default).
    pub style_variant: i32,
    pub is_enabled: bool,
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
    /// Transient: the Taffy node this maps to in the current layout pass.
    pub taffy_id: Option<taffy::NodeId>,
    pub rect: LaidRect,
    pub hover: Spring,
    pub press: Spring,
    pub hovered: bool,
    pub pressed: bool,
}

impl Node {
    pub fn new(kind: ControlKind) -> Self {
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
            taffy_id: None,
            rect: LaidRect::default(),
            hover: Spring::new(0.0),
            press: Spring::new(0.0),
            hovered: false,
            pressed: false,
        }
    }

    pub fn handler(&self, event: Event) -> Option<&EventHandler> {
        self.handlers.iter().find(|(e, _)| *e == event).map(|(_, h)| h)
    }

    /// True for nodes that respond to a click (fire chrome + invoke).
    pub fn is_clickable(&self) -> bool {
        self.kind == ControlKind::Button
            || self.handler(Event::Click).is_some()
            || self
                .pointer
                .as_ref()
                .is_some_and(|p| p.on_tapped.is_some() || p.on_pointer_pressed.is_some())
    }
}

fn default_font_size(kind: ControlKind) -> f32 {
    match kind {
        ControlKind::Button => 14.0,
        _ => 14.0,
    }
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
            s.display = Display::Flex;
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
        _ => {
            s.display = Display::Flex;
        }
    }
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

    pub fn remove(&mut self, id: ControlId) -> Option<Node> {
        let idx = (id.get() - 1) as usize;
        let taken = self.slots.get_mut(idx).and_then(|s| s.take());
        if taken.is_some() {
            self.free.push(idx as u32);
        }
        taken
    }
}
