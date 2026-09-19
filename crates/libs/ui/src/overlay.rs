//! Flyouts, popups, menus and tooltips.
//!
//! An overlay is content positioned against an anchor rather than by its parent's layout,
//! drawn above the rest of the window, with a defined way to close. The three kinds in
//! [`Kind`] differ only in their dismiss policy and their focus behaviour, so they are one
//! placement and one row of [`KINDS`].
//!
//! # What an overlay is built from
//!
//! An overlay is a detached root in `windows-scene`, placed by an offset the solve reads.
//! "Press outside dismisses" is a blocker entry in the one hit array, resolved by that array's
//! own back-to-front scan. `Tab` and `Esc` are the router's focus scope. A hover-open delay is
//! a scoped compositor batch. This module contributes a placement rule, a lifetime, and the
//! state machine that decides when a tooltip is showing.
//!
//! # Every overlay lives inside the window
//!
//! There is one HWND and it is composition-hosted, so an overlay is a subtree of the same
//! visual tree and cannot extend past the client box. One that would not fit is flipped, then
//! slid inward, then clamped ([`place`]).
//!
//! # Lifetime
//!
//! Opening mints a slot root and an [`Owner`]; closing drops the `Owner`, which disposes every
//! `Cell`, `Memo` and `Effect` inside it, and drops the [`Mount`], which destroys the subtree
//! with its exit transition. An overlay is never cached and hidden, because a hidden overlay
//! leaves visuals DWM still walks every frame.

use crate::build::tree;
use crate::build::{Entrance, Host, Mount, Placement, Ui};
use crate::gesture::Recognised;
use crate::input::{KeyKind, Report, ScopeId};
use crate::layout::{Len, Rect};
use crate::seam::FocusOp;
use crate::signal::Owner;
use crate::widget::{Intent, TextSource, What};
use crate::{VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP};
use std::rc::Rc;
use windows_numerics::Vector2;
use windows_scene::{ControlId, DelayId, Easing, Exit, HitFlags, NodeId, Prop, SceneEvent};

// ── the three kinds, as one table ────────────────────────────────────────────────

/// Selects an overlay's dismiss policy and focus behaviour.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Anchored to a control. Light dismiss, `Esc`, focus loss; takes focus and restores it on
    /// close, and `Tab` past the end lets go. Menus, pickers, popovers.
    Flyout,
    /// Anchored to the window. `Esc` and an explicit close only, and it **traps** focus.
    /// Confirm dialogs, modals, drawers and sheets.
    Popup,
    /// Anchored to a control, and never the target of anything: no hit entry, no focus order
    /// position, no scope. Hover descriptions.
    Tooltip,
}

/// Declares which events close an overlay.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct DismissPolicy {
    /// A press outside dismisses. The router consumes that press from this flag, so the press
    /// that closes an overlay never also invokes what it landed on.
    pub light: bool,
    /// `Esc` dismisses, as does `Tab` off the end of a scope that does not trap.
    pub escape: bool,
    /// Every contact being taken away dismisses: a lost capture, or the window losing focus.
    pub focus_loss: bool,
}

/// What each kind implies, indexed by the kind.
///
/// `Some(trap)` marks a kind that takes focus, `trap` being whether `Tab` may not leave it.
/// The same answer decides whether the kind contributes a blocker: an overlay that takes focus
/// also takes the pointer, so no press lands on content the keyboard cannot reach. A light
/// overlay's blocker dismisses it and a modal's does nothing, and both exist because a focus
/// scope is named by its own first entry in the hit array, which is that blocker.
const KINDS: [(DismissPolicy, Option<bool>, Exit); 3] = [
    (
        DismissPolicy {
            light: true,
            escape: true,
            focus_loss: true,
        },
        Some(false),
        Exit::Fade { ms: 90 },
    ),
    // A modal is not light-dismissed. It still contributes a blocker, so a press outside it
    // reaches nothing; that blocker's press just does nothing.
    (
        DismissPolicy {
            light: false,
            escape: true,
            focus_loss: false,
        },
        Some(true),
        Exit::Fade { ms: 120 },
    ),
    // A tooltip contributes no hit entry, so nothing in the array can dismiss it. Its exits are
    // this module's dwell machine: any press, any leave, `Esc`, or focus moving.
    (
        DismissPolicy {
            light: false,
            escape: true,
            focus_loss: true,
        },
        None,
        Exit::Fade { ms: TIP_EXIT_MS },
    ),
];

impl Kind {
    /// Returns the dismiss policy this kind implies, which a caller may replace with
    /// [`Spec::dismiss`].
    #[must_use]
    pub const fn dismiss(self) -> DismissPolicy {
        KINDS[self as usize].0
    }

    /// Returns `Some(trap)` where the kind takes focus and `None` where it takes none.
    #[must_use]
    pub const fn takes_focus(self) -> Option<bool> {
        KINDS[self as usize].1
    }
}

// ── where an overlay lands: flip, then slide, then clamp ─────────────────────────
//
// Anchor resolution runs after the solve, because it needs both the anchor's rect and the
// overlay's own measured size. It produces one offset, and that offset is an input to the next
// solve rather than a transform applied over one, so a detached subtree's rects stay absolute
// in window space.
//
// Placement never changes the overlay's layout. Flipping moves the resolved offset and nothing
// else, and an overlay's size does not depend on where it landed. That is what makes the
// placement pass terminate: the second flush computes the same offset from the same size and
// stops.

/// What an overlay is positioned against.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum AnchorTo {
    /// A control's rect, read from the one hit array. A menu under its button.
    Control(ControlId),
    /// A raw pointer position: the point a press was at, not wherever the pointer has since
    /// moved to.
    Point(Vector2),
    /// The window itself, less whatever insets the spec named. What a modal centres against.
    Window,
}

/// Which side of the anchor the overlay sits on.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Side {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
    /// No side: centred on both axes. [`Align`] runs along a chosen side, so centring on both
    /// needs its own variant.
    Center,
}

/// Whether a side stacks on y, and which end of the anchor it seats against.
///
/// `1` is past the far edge, `-1` before the near one, `0` centred on both axes. Indexed by
/// [`Side`], and the opposite of a side is its row with the sign flipped.
const SEATING: [(bool, f32); 5] = [
    (true, 1.0),
    (true, -1.0),
    (false, -1.0),
    (false, 1.0),
    (true, 0.0),
];

impl Side {
    /// Returns the side to try when this one does not fit.
    ///
    /// A centred overlay has nothing to flip to and is clamped instead.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Top => Self::Bottom,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Center => Self::Center,
        }
    }

    /// Returns whether the overlay stacks vertically against its anchor, so the cross axis it
    /// slides along is x.
    #[must_use]
    pub const fn is_vertical(self) -> bool {
        SEATING[self as usize].0
    }
}

/// Where along the chosen side the overlay lines up.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Align {
    /// Leading edges together — a menu's left edge under its button's left edge.
    #[default]
    Start,
    /// Midpoints together.
    Center,
    /// Trailing edges together.
    End,
}

/// How an overlay survives not fitting.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Fit {
    /// Try `side`; if it does not fit, try its opposite; then slide along the cross axis; then
    /// clamp to the window. Flipping runs before sliding, so an overlay clears its anchor
    /// rather than sliding across it.
    #[default]
    Flip,
    /// Stay on the named side, and only pull back inside the window.
    Clamp,
}

/// A complete placement rule.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Anchor {
    /// What the overlay is positioned against.
    pub to: AnchorTo,
    /// Which side of the anchor it seats on.
    pub side: Side,
    /// Where along that side it lines up.
    pub align: Align,
    /// DIPs, applied after side and align. The gap between a menu and its button.
    pub offset: Vector2,
    /// How it survives not fitting.
    pub fit: Fit,
}

impl Anchor {
    /// Returns a rule seating the overlay under `to`, leading edges aligned.
    #[must_use]
    pub const fn below(to: ControlId) -> Self {
        Self {
            to: AnchorTo::Control(to),
            side: Side::Bottom,
            align: Align::Start,
            offset: Vector2 { x: 0.0, y: 0.0 },
            fit: Fit::Flip,
        }
    }

    /// Returns a rule seating the overlay against `point`, as a context menu is placed.
    #[must_use]
    pub const fn at(point: Vector2) -> Self {
        Self {
            to: AnchorTo::Point(point),
            ..Self::below(ControlId::NONE)
        }
    }

    /// Returns a rule centring the overlay in the window.
    #[must_use]
    pub const fn centered() -> Self {
        Self {
            to: AnchorTo::Window,
            side: Side::Center,
            align: Align::Center,
            fit: Fit::Clamp,
            ..Self::below(ControlId::NONE)
        }
    }

    /// Returns a rule docking the overlay inside the window against `side`, as a drawer or a
    /// sheet is placed.
    #[must_use]
    pub const fn window(side: Side) -> Self {
        Self {
            to: AnchorTo::Window,
            side,
            align: Align::Center,
            fit: Fit::Clamp,
            ..Self::below(ControlId::NONE)
        }
    }

    /// Returns this rule seating the overlay on `side` of its anchor.
    #[must_use]
    pub const fn side(self, side: Side) -> Self {
        Self { side, ..self }
    }

    /// Returns this rule lining the overlay up by `align` along the chosen side.
    #[must_use]
    pub const fn align(self, align: Align) -> Self {
        Self { align, ..self }
    }

    /// Returns this rule with a gap of `x` by `y` DIPs between the overlay and its anchor.
    ///
    /// A raw length rather than a palette metric, because it is measured against the anchor's
    /// own box. [`place`] reverses it when the overlay flips.
    #[must_use]
    pub const fn gap(self, x: f32, y: f32) -> Self {
        Self {
            offset: Vector2 { x, y },
            ..self
        }
    }
}

/// Returns the absolute window-DIP origin for an overlay of `size` placed against the rect
/// `against` inside a client box of `window`.
///
/// `against` is the anchor's own rect, which for [`AnchorTo::Window`] is the client box less
/// the spec's own insets, so centring a modal and docking a drawer to an edge run through this
/// one rule. Pure.
#[must_use]
pub fn place(size: Vector2, against: Rect, anchor: Anchor, window: Vector2) -> Vector2 {
    // An overlay sits beside a control and *within* the window, so the same side resolves to
    // opposite offsets in the two: seating a modal beside the window box would put it one
    // window-height off screen.
    let inside = matches!(anchor.to, AnchorTo::Window);
    let mut at = seat(size, against, anchor.side, anchor, inside);
    if anchor.fit == Fit::Flip && overhangs(at, size, anchor.side, window) {
        let other = anchor.side.opposite();
        let flipped = seat(size, against, other, anchor, inside);
        if !overhangs(flipped, size, other, window) {
            at = flipped;
        }
    }
    // Sliding along the cross axis and clamping to the window are the same pull-back on each
    // axis: the leading edge wins where the overlay is larger than the window, so a menu taller
    // than the client box keeps its first items on screen.
    Vector2 {
        x: at.x.min(window.x - size.x).max(0.0),
        y: at.y.min(window.y - size.y).max(0.0),
    }
}

/// Returns where the overlay sits before anything is done about fit.
fn seat(size: Vector2, against: Rect, side: Side, anchor: Anchor, inside: bool) -> Vector2 {
    let (vertical, dir) = SEATING[side as usize];
    let axis = |on_y: bool| {
        let pick = |v: Vector2| if on_y { v.y } else { v.x };
        let (near, extent) = if on_y {
            (against.y0, against.y1 - against.y0)
        } else {
            (against.x0, against.x1 - against.x0)
        };
        let (own, gap) = (pick(size), pick(anchor.offset));
        if on_y == vertical && dir != 0.0 {
            // The gap is measured from the anchor, so it reverses with the side: a menu four
            // DIPs below its button is four DIPs above it when flipped.
            match (inside, dir > 0.0) {
                (true, true) => near + extent - own - gap,
                (true, false) => near + gap,
                (false, true) => near + extent + gap,
                (false, false) => near - own - gap,
            }
        } else {
            let align = if dir == 0.0 {
                Align::Center
            } else {
                anchor.align
            };
            match align {
                Align::Start => near,
                Align::Center => near + (extent - own) * 0.5,
                Align::End => near + extent - own,
            }
        }
    };
    Vector2 {
        x: axis(false),
        y: axis(true),
    }
}

/// Returns whether the overlay runs off the window on the side it was seated against.
///
/// Tests the main axis only. Sliding fixes the cross axis, so a cross-axis overhang is not a
/// reason to flip a menu above the button it was merely too wide for.
fn overhangs(at: Vector2, size: Vector2, side: Side, window: Vector2) -> bool {
    let (vertical, dir) = SEATING[side as usize];
    if dir == 0.0 {
        return false;
    }
    let (near, own, limit) = if vertical {
        (at.y, size.y, window.y)
    } else {
        (at.x, size.x, window.x)
    };
    near < 0.0 || near + own > limit
}

// ── what an overlay is opened with ───────────────────────────────────────────────

/// A popup moves as one compositor group, by a multiple of its measured size.
#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) struct Slide {
    pub by: Vector2,
    pub ms: u32,
    pub easing: Easing,
}

impl Slide {
    /// Returns where the entry starts, from the placed local offset and the measured size.
    #[must_use]
    pub fn from(self, local: Vector2, size: Vector2) -> Vector2 {
        Vector2 {
            x: local.x + self.by.x * size.x,
            y: local.y + self.by.y * size.y,
        }
    }
}

/// Describes how an overlay opens: its kind, anchor, dismiss policy and exit transition.
///
/// The fields are private and reachable only through the constructors. A [`Kind::Tooltip`] spec
/// can be built only inside this module, because a tooltip's lifetime belongs to the dwell
/// machine and one opened from outside would have nothing to close it; `dwelled` records what
/// happened rather than being a setting, so no caller can claim a hover opened an overlay that
/// no hover produced.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Spec {
    kind: Kind,
    anchor: Anchor,
    viewport: Option<[Len; 4]>,
    slide: Option<Slide>,
    dismiss: DismissPolicy,
    /// Played as the subtree is destroyed.
    exit: Exit,
    dwelled: bool,
}

impl Spec {
    /// Returns a flyout anchored under `invoker`: light dismiss, `Esc`, focus restored to
    /// `invoker` on close.
    #[must_use]
    pub const fn flyout(invoker: ControlId) -> Self {
        Self::of(Kind::Flyout, Anchor::below(invoker))
    }

    /// Returns a modal centred in the window, trapping focus and refusing light dismiss.
    #[must_use]
    pub const fn popup() -> Self {
        Self::of(Kind::Popup, Anchor::centered())
    }

    const fn of(kind: Kind, anchor: Anchor) -> Self {
        Self {
            kind,
            anchor,
            viewport: None,
            slide: None,
            dismiss: kind.dismiss(),
            exit: KINDS[kind as usize].2,
            dwelled: false,
        }
    }

    /// Returns the kind this spec opens.
    #[must_use]
    pub const fn kind(self) -> Kind {
        self.kind
    }

    /// Returns this spec placed by `anchor` instead of the kind's default placement.
    #[must_use]
    pub const fn anchor(self, anchor: Anchor) -> Self {
        Self { anchor, ..self }
    }

    /// Returns this spec constrained to the window minus `[left, top, right, bottom]` insets.
    ///
    /// The viewport sizes from window input before layout; placement never feeds its size.
    #[must_use]
    pub const fn viewport(self, insets: [Len; 4]) -> Self {
        Self {
            viewport: Some(insets),
            ..self
        }
    }

    /// Returns this spec sliding the whole popup from `by` times its size, and back there on
    /// dismissal.
    ///
    /// Input stays on its blocker until the compositor reports entry complete.
    #[must_use]
    pub const fn slide(self, by: Vector2, ms: u32, easing: Easing) -> Self {
        Self {
            slide: Some(Slide { by, ms, easing }),
            exit: Exit::Slide { by, ms, easing },
            ..self
        }
    }

    /// Returns this spec with `dismiss` replacing the kind's implied policy.
    #[must_use]
    pub const fn dismiss(self, dismiss: DismissPolicy) -> Self {
        Self { dismiss, ..self }
    }

    /// Returns this spec with `exit` as the transition played while the subtree is destroyed.
    #[must_use]
    pub const fn exit(self, exit: Exit) -> Self {
        Self { exit, ..self }
    }

    /// Returns the control this spec restores focus to, and whose second invoke closes it.
    #[must_use]
    pub const fn invoker(self) -> Option<ControlId> {
        match self.anchor.to {
            AnchorTo::Control(control) => Some(control),
            _ => None,
        }
    }
}

/// Event-rate changes from popup declarations. The retained node id rejects stale opens.
pub(crate) enum Request {
    Show {
        key: NodeId,
        spec: Spec,
        body: Rc<dyn Fn(&mut Ui<'_>)>,
        closed: Rc<dyn Fn()>,
    },
    Close(NodeId),
}

impl Request {
    pub(crate) fn key(&self) -> NodeId {
        match self {
            Self::Show { key, .. } | Self::Close(key) => *key,
        }
    }
}

// ── the stack ────────────────────────────────────────────────────────────────────

/// Identifies one open overlay by its depth in the stack and the generation occupying it.
///
/// The generation makes a stale close a miss: a close queued behind the close of the overlay
/// above it finds a different generation and does nothing, rather than closing whatever now
/// sits at that depth.
///
/// A depth is meaningful only while everything above it is still open, because the stack is
/// truncated from a depth rather than having one entry removed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct OverlayId {
    depth: u16,
    generation: u32,
}

/// One open overlay. Private, along with its fields.
struct Open {
    /// Names this overlay, and names the focus scope it pushed where it pushed one. One number
    /// for both, so a scope id and an overlay id can never disagree about which overlay is
    /// meant.
    generation: u32,
    kind: Kind,
    dismiss: DismissPolicy,
    root: NodeId,
    /// The full-window entry it contributes ahead of its own subtree, and the control its focus
    /// scope is named by. `None` only for a tooltip.
    blocker: Option<ControlId>,
    /// The control that opened it, where one did. A second tap on that control closes the
    /// overlay, and a tooltip's hover target is compared against it.
    invoker: Option<ControlId>,
    /// Whether a hover opened it rather than an invoke, which is what decides that the pointer
    /// leaving closes it. A hover-opened submenu and a clicked flyout are the same [`Kind`], so
    /// the kind cannot carry this.
    dwelled: bool,
    /// The window insets its spec named, kept because a resize re-resolves them against the new
    /// client box rather than scaling what they last came to.
    insets: Option<[Len; 4]>,
    /// The declaration that opened it and the callback that reports it closed, for an overlay a
    /// signal declared rather than a gesture.
    binding: Option<(NodeId, Rc<dyn Fn()>)>,
    /// The item the last type-ahead in this overlay landed on, which the next one cycles from,
    /// so repeated presses of one letter walk the items beginning with it.
    typed: Option<ControlId>,
    /// Dropped on close, which disposes every signal the body created.
    owner: Owner,
    /// Dropped on close, which unmounts the subtree and destroys it with its exit.
    mount: Mount,
}

impl Open {
    /// Returns the declaration key this overlay is held open by, where a signal declared it.
    fn binding_key(&self) -> Option<NodeId> {
        self.binding.as_ref().map(|(key, _)| *key)
    }

    /// Returns whether the pointer leaving this overlay closes it, which is a submenu and only
    /// a submenu.
    ///
    /// A description is hover-opened too and is excluded by the blocker test: leaving one
    /// describable control for another swaps its content rather than closing it.
    fn is_submenu(&self) -> bool {
        self.dwelled && self.blocker.is_some()
    }
}

/// The overlay stack.
///
/// Overlays nest: a submenu sits above its menu and cannot outlive it, and a tooltip is always
/// topmost because any press dismisses it before anything else opens. Closing one closes
/// everything above it, which is the whole nesting policy for the slot roots, the focus scopes
/// and the placement rows alike.
#[derive(Default)]
pub struct Overlays {
    open: Vec<Open>,
    /// Names every overlay this stack has opened, and every focus scope it has pushed. The ring
    /// mints none: scope names are the input half's, and an op carrying its own name is applied
    /// without a reply.
    minted: u32,
    dwell: Dwell,
    /// The depth an invoked choice asked to truncate to, held until the application has run the
    /// handler that choice named. See [`Overlays::after_dispatch`].
    after: Option<usize>,
    /// The client box the last sync saw, so a declaration that flipped because the window
    /// resized is told apart from one the user dismissed.
    window: Option<Vector2>,
}

impl Overlays {
    /// Returns an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns how many overlays are open.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.open.len()
    }

    /// Returns whether no overlay is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// Returns the overlay `invoker` already has open, or `None`.
    ///
    /// Tooltips are skipped, so a description showing over a control does not answer as that
    /// control's flyout.
    #[must_use]
    pub fn opened_by(&self, invoker: ControlId) -> Option<OverlayId> {
        self.open
            .iter()
            .position(|open| open.kind != Kind::Tooltip && open.invoker == Some(invoker))
            .map(|depth| self.id_at(depth))
    }

    /// Returns the id of the overlay at `depth`.
    fn id_at(&self, depth: usize) -> OverlayId {
        OverlayId {
            depth: depth as u16,
            generation: self.open[depth].generation,
        }
    }

    /// Opens an overlay, building `body` under a fresh detached root, and returns its id.
    ///
    /// A kind that takes focus also contributes a full-window blocker entry and emits a
    /// [`FocusOp::Push`] named by that blocker, with the invoker as the scope's restore target.
    /// `body` writes through a borrowed creation context; UI effects wait until that
    /// transaction releases the host.
    pub fn open(
        &mut self,
        focus: &mut Vec<FocusOp>,
        spec: Spec,
        body: impl Fn(&mut Ui<'_>) + 'static,
    ) -> OverlayId {
        let depth = self.open.len();
        self.minted = self.minted.wrapping_add(1);
        let generation = self.minted;
        let invoker = spec.invoker();
        // The blocker, the slot root and the placement row are minted under one host borrow.
        let (blocker, root, scope) = Host::with(|host| {
            let blocker = spec.kind.takes_focus().map(|_| host.mint_blocker());
            let scope = host.intern(host.root_scope());
            (blocker, host.overlay_root(scope).0, scope)
        });
        // Mapped over the blocker rather than asking `takes_focus` again: a focus scope is named
        // by its own first entry in the hit array, and that entry is the blocker, so deriving
        // the scope from the blocker is what guarantees every scope has one.
        if blocker.is_some() {
            focus.push(FocusOp::Push {
                id: ScopeId(generation),
                trap: spec.kind.takes_focus() == Some(true),
                from: blocker,
                // An overlay anchored to a point or to the window names no restore target, and
                // the half holding the ring fills in the focus this one interrupted.
                restore_to: invoker,
            });
        }
        // The detached owner and the retained records share this creation transaction, which
        // runs outside the host borrow because building the body is application code.
        let (owner, mut mount) =
            Owner::scope(|| Ui::mount_interned(root, None, scope, ControlId::NONE, body));
        mount.set_exit(spec.exit);
        Host::with(|host| {
            let node = mount.node();
            let window = host.window_extent();
            if spec.slide.is_some() {
                // The entrance holds input on this subtree for as long as the slide plays, so a
                // press cannot land on a surface that is still arriving.
                host.suspend_input(node, true);
            }
            host.overlays.push(Placement {
                root,
                blocker,
                anchor: spec.anchor,
                viewport: host.overlay_viewport(spec.viewport),
                at: Vector2 { x: 0.0, y: 0.0 },
                entry: spec.slide.map(|slide| Entrance::new(node, slide, window)),
            });
        });
        self.open.push(Open {
            generation,
            kind: spec.kind,
            dismiss: spec.dismiss,
            root,
            blocker,
            invoker,
            dwelled: spec.dwelled,
            insets: spec.viewport,
            binding: None,
            typed: None,
            owner,
            mount,
        });
        self.id_at(depth)
    }

    /// Applies the popup declarations changed by the preceding signal flush, and returns whether
    /// anything is open.
    pub(crate) fn sync(&mut self, focus: &mut Vec<FocusOp>) -> bool {
        let window = Host::window_size().peek();
        // A drawer that flipped because the window crossed its threshold keeps the application's
        // resting intent: it neither slides in nor plays its exit, because nothing was dismissed.
        let resized = self.window.replace(window).is_some_and(|was| was != window);
        if resized {
            // The insets are stated against the window, so the box every open overlay is laid
            // out inside moves with it.
            let insets: Vec<Option<[Len; 4]>> = self.open.iter().map(|open| open.insets).collect();
            Host::with(|host| {
                for (at, insets) in insets.into_iter().enumerate() {
                    // Resolved before the write, because the row being written is reached
                    // through the same host the resolution reads.
                    let box_ = host.overlay_viewport(insets);
                    host.overlays[at].viewport = box_;
                }
            });
        }
        let requests = Host::with(|host| core::mem::take(&mut host.popups));
        for request in requests {
            let key = request.key();
            let held = self
                .open
                .iter()
                .position(|open| open.binding_key() == Some(key));
            match request {
                Request::Close(_) => {
                    let Some(depth) = held else { continue };
                    // A condition change rather than a dismissal preserves the application's
                    // resting intent, including a drawer closed by a window resize.
                    self.open[depth].binding = None;
                    if resized {
                        self.open[depth].mount.set_exit(Exit::None);
                    }
                    self.truncate(depth, focus);
                }
                // The retained node id rejects a stale open: a declaration whose node the patch
                // has since rebuilt names a key nothing is holding.
                Request::Show {
                    mut spec,
                    body,
                    closed,
                    ..
                } if held.is_none() && Host::with(|host| host.tree.is_live(key)) => {
                    if resized {
                        spec.slide = None;
                    }
                    let id = self.open(focus, spec, move |ui| body(ui));
                    self.open[usize::from(id.depth)].binding = Some((key, closed));
                }
                Request::Show { .. } => {}
            }
        }
        // A declaration that unmounted takes its overlay with it: nothing will ask for the
        // close, and the cell the closed callback would write may be gone with the page.
        let dead = Host::with(|host| {
            self.open
                .iter()
                .position(|open| open.binding_key().is_some_and(|key| !host.tree.is_live(key)))
        });
        if let Some(depth) = dead {
            for open in &mut self.open[depth..] {
                open.binding = None;
                open.mount.set_exit(Exit::None);
            }
            self.truncate(depth, focus);
        }
        !self.open.is_empty()
    }

    /// Closes `overlay` and everything opened above it.
    ///
    /// An id whose generation does not match the one at that depth closes nothing, so a close
    /// queued behind the close of the overlay above it is a miss rather than closing whatever
    /// has since taken the depth.
    pub fn close(&mut self, overlay: OverlayId, focus: &mut Vec<FocusOp>) {
        let depth = usize::from(overlay.depth);
        if self
            .open
            .get(depth)
            .is_some_and(|open| open.generation == overlay.generation)
        {
            self.truncate(depth, focus);
        }
    }

    /// Closes the topmost overlay, which is what `Esc` and a light-dismiss press do.
    pub fn close_top(&mut self, focus: &mut Vec<FocusOp>) {
        if !self.open.is_empty() {
            self.truncate(self.open.len() - 1, focus);
        }
    }

    /// Dismisses what `Esc` means here: the description alone where one is on screen, and the
    /// topmost overlay otherwise.
    pub(crate) fn escape(&mut self, focus: &mut Vec<FocusOp>) {
        // A description is innermost, so one `Esc` takes it alone. The router raises `Escape`
        // rather than a key wherever a scope is open, so the next one reaches the menu.
        if self.dwell.showing.is_some() {
            self.hide(focus);
            return;
        }
        self.cancel();
        if self.open.last().is_some_and(|open| open.dismiss.escape) {
            self.close_top(focus);
        }
    }

    /// Drops every overlay at or above `at`, innermost first.
    ///
    /// Innermost first, which is the order the focus ring and the hit array both assume: a
    /// submenu is gone before the menu that anchored it.
    fn truncate(&mut self, at: usize, focus: &mut Vec<FocusOp>) {
        if at >= self.open.len() {
            return;
        }
        // Every close path arrives here, so the pending delay is cancelled once. A delay
        // outliving its menu would hold a compositor batch open for its full duration and then
        // open a submenu against a row that has gone.
        self.cancel();
        while self.open.len() > at {
            let depth = self.open.len() - 1;
            // Forgotten from the close path rather than into it, so a description closed as part
            // of something above it does not then close itself a second time.
            if self
                .dwell
                .showing
                .is_some_and(|(id, _)| usize::from(id.depth) == depth)
            {
                self.dwell.showing = None;
            }
            let mut open = self.open.pop().expect("the stack is longer than `at`");
            if open.blocker.is_some() {
                // Innermost first, so the outermost pop is the last one applied and its restore
                // target is where focus ends: the invoker it was opened from.
                focus.push(FocusOp::Pop(ScopeId(open.generation)));
            }
            _ = Host::try_with(|host| {
                // A subtree whose input the host already suspended is on its way out for another
                // reason, and playing an exit over that shows the same content leaving twice.
                if host.input_suspended(open.mount.node()) {
                    open.mount.set_exit(Exit::None);
                }
                open.mount.retire(host);
                if let Some(blocker) = open.blocker {
                    host.release_control(blocker);
                }
                host.unplace(open.root);
            });
            let binding = open.binding.take();
            drop(open);
            if let Some((_, closed)) = binding {
                closed();
            }
        }
    }

    /// Applies the keyboard vocabulary an open overlay owns. Runs before the front table
    /// consumes the tick.
    ///
    /// Appends the focus edit a keystroke implied to `focus`, and an [`Intent`] for an invoke to
    /// `intents`, so `Enter` on a menu item reaches the handler a tap reaches.
    pub(crate) fn keys(
        &mut self,
        reports: &[Report],
        focus: &mut Vec<FocusOp>,
        intents: &mut Vec<Intent>,
    ) {
        if self.open.is_empty() {
            return;
        }
        for report in reports {
            let Report::Key { target, event } = *report else {
                continue;
            };
            if event.kind != KeyKind::Down {
                continue;
            }
            // The router raises `Report::Escape` only where a focus scope is open, and a tooltip
            // pushes none, so a description's `Esc` is read here from the raw key.
            if i32::from(event.key) == VK_ESCAPE {
                self.hide(focus);
                continue;
            }
            // The menu vocabulary applies only while a focus-taking overlay is topmost;
            // otherwise the arrow keys belong to whatever has focus in the window's content.
            if self.open.last().is_some_and(|open| open.blocker.is_some()) {
                self.key(target, i32::from(event.key), focus, intents);
            }
        }
    }

    /// Handles one keystroke while a focus-taking overlay is topmost.
    ///
    /// `Tab` and `Esc` never reach here, because the router takes both before any control sees
    /// them. What is left is the menu vocabulary: `Down` is `Tab`, `Up` is `Shift-Tab`, and
    /// `Home` and `End` are the ends of the scope. Each is emitted as the focus edit it means
    /// rather than performed, because the ring belongs to the half that routes input.
    fn key(
        &mut self,
        target: Option<ControlId>,
        vk: i32,
        focus: &mut Vec<FocusOp>,
        intents: &mut Vec<Intent>,
    ) {
        const MOVES: [(i32, FocusOp); 4] = [
            (VK_DOWN, FocusOp::Step { forward: true }),
            (VK_UP, FocusOp::Step { forward: false }),
            (VK_HOME, FocusOp::End { last: false }),
            (VK_END, FocusOp::End { last: true }),
        ];
        if let Some((_, op)) = MOVES.iter().find(|(key, _)| *key == vk) {
            focus.push(*op);
        } else if vk == VK_LEFT {
            // One level up, which is what `Esc` does through the router.
            self.close_top(focus);
        } else if vk == VK_RIGHT || vk == VK_RETURN {
            // Invoked through the ordinary tap path, so a row carrying a flyout opens its
            // submenu the same way a pointer tap on that row does.
            if let Some(target) = target {
                intents.push(Intent::invoke_focused(target));
            }
        } else if let Some(letter) = type_ahead(vk) {
            self.type_ahead(letter, focus);
        }
    }

    /// Focuses the next item of the topmost overlay whose accessible name begins with `letter`,
    /// and emits nothing where none does.
    ///
    /// The candidates are that overlay's own interactive controls in tree order, which is the
    /// order [`FocusOp::Step`] walks. The cycle runs from the item this overlay last typed onto
    /// rather than from the focused control, because the focused control is held by the half
    /// that routes input and cannot be read here.
    fn type_ahead(&mut self, letter: char, focus: &mut Vec<FocusOp>) {
        let Some(open) = self.open.last_mut() else {
            return;
        };
        let mut items = Vec::new();
        let found = Host::with(|host| {
            host.interactive_under(open.root, &mut items);
            let from = open
                .typed
                .and_then(|held| items.iter().position(|&item| item == held))
                .map_or(0, |at| at + 1);
            items
                .iter()
                .cycle()
                .skip(from)
                .take(items.len())
                .copied()
                .find(|&item| answers(host.name_of(item), letter))
        });
        if let Some(control) = found {
            open.typed = Some(control);
            focus.push(FocusOp::Focus(Some(control)));
        }
    }

    /// Applies a tick's reports and intents to the stack.
    ///
    /// Runs after the front table has consumed them, so the press that opens an overlay here has
    /// already lit its button and no intent is the cause of a visual.
    pub(crate) fn settle(
        &mut self,
        reports: &[Report],
        intents: &[Intent],
        focus: &mut Vec<FocusOp>,
    ) {
        for report in reports {
            match *report {
                // A press on a blocker, already consumed by the router. The array puts a blocker
                // directly under the overlay it belongs to, so this closes that overlay and
                // everything above it. A modal's blocker does nothing, which is what stops a
                // press outside a dialog reaching the content behind it.
                Report::Dismiss { blocker, .. } => {
                    self.hide(focus);
                    if let Some(depth) = self
                        .open
                        .iter()
                        .position(|open| open.blocker == Some(blocker))
                        && self.open[depth].dismiss.light
                    {
                        self.truncate(depth, focus);
                    }
                }
                // `Esc`, or `Tab` off the end of a scope that does not trap. The router raises
                // this only where a scope is open, and a tooltip pushes none, so a description's
                // own `Esc` arrives as a key instead and is read in [`Overlays::keys`].
                Report::Escape { .. } => self.escape(focus),
                // Every contact taken away, which is what the window losing focus produces.
                Report::CaptureLost => {
                    self.hide(focus);
                    if let Some(depth) = self.open.iter().position(|open| open.dismiss.focus_loss) {
                        self.truncate(depth, focus);
                    }
                }
                // Any press at all hides a description, whether or not it was over one, and so
                // does focus moving off the control being described.
                Report::Pressed { .. } | Report::FocusChanged { .. } => self.hide(focus),
                Report::HoverChanged { to, .. } => self.crossed(to, focus),
                // A right tap opens the target's flyout at the press point rather than under the
                // control, so a context menu opens where the pointer was when it was pressed.
                Report::Gesture {
                    target,
                    event: Recognised::RightTapped { at },
                    ..
                } => {
                    let anchor = Anchor::at(Vector2 { x: at.x, y: at.y });
                    self.open_flyout(target, Spec::flyout(target).anchor(anchor), focus);
                }
                _ => {}
            }
        }
        for intent in intents {
            if intent.what == What::Tapped {
                self.invoke(intent.target, focus);
            }
        }
        // Every crossing and press in the batch has been seen, so at most one target is still
        // owed a reveal.
        self.dwell_settle(focus);
    }

    /// Opens the flyout `target` declared, or closes the one it already has open, so a picker's
    /// own button shuts it.
    ///
    /// A control **inside** an open flyout that declares no flyout of its own is a terminal
    /// choice — a menu option — so invoking it closes the flyout it was chosen from, and every
    /// submenu above it. A press *outside* cannot arrive here: a [`Kind::Flyout`] contributes a
    /// blocker, and a press on that blocker is consumed as a dismiss rather than a tap. A
    /// [`Kind::Popup`] is left alone, because a button in a dialog is not a choice **from** the
    /// dialog and closing it would dismiss the dialog on its first control.
    fn invoke(&mut self, target: ControlId, focus: &mut Vec<FocusOp>) {
        if let Some(overlay) = self.opened_by(target) {
            self.close(overlay, focus);
            return;
        }
        if Host::with(|host| host.flyout_of(target)).is_some() {
            self.open_flyout(target, Spec::flyout(target), focus);
            return;
        }
        // Recorded rather than performed: the flyout's body **owns** the handler this intent
        // names, and the overlay pass runs before the application is dispatched to, so closing
        // here would dispose the control the intent points at and the choice would be discarded
        // as a stale id.
        let depth = self.open.iter().position(|open| open.kind == Kind::Flyout);
        self.after = self.after.min(depth).or(depth);
    }

    /// Opens `target`'s declared flyout with `spec`, doing nothing where it declared none.
    fn open_flyout(&mut self, target: ControlId, spec: Spec, focus: &mut Vec<FocusOp>) {
        // Taken out of the host borrow before it runs: building the body is application code.
        let Some(body) = Host::with(|host| host.flyout_of(target)) else {
            return;
        };
        _ = self.open(focus, spec, move |ui| body(ui));
    }

    /// Closes what an invoked choice asked to close, once the application has acted on it.
    ///
    /// Called after `Host::dispatch`, which is the one point at which a menu option's handler
    /// has run and its overlay is no longer owed to anything.
    pub(crate) fn after_dispatch(&mut self, focus: &mut Vec<FocusOp>) {
        if let Some(depth) = self.after.take() {
            self.truncate(depth, focus);
        }
    }

    /// Applies scene events to the stack: an elapsed dwell, and an entrance the compositor has
    /// finished playing.
    ///
    /// The entrance is the only animation this stack binds on an overlay root's offset, so the
    /// channel the report names is what identifies it.
    pub(crate) fn scene(&mut self, events: &[SceneEvent], focus: &mut Vec<FocusOp>) {
        for event in events {
            match *event {
                SceneEvent::DelayElapsed(delay) => self.elapsed(delay, focus),
                SceneEvent::AnimationCompleted { node, prop: Prop::Offset } => {
                    _ = Host::try_with(|host| host.complete_overlay_entry(node));
                }
                _ => {}
            }
        }
    }

    /// Returns the depth of the overlay `control` sits inside, by the slot root its node hangs
    /// under.
    ///
    /// Resolved through the tree rather than a control-to-overlay table stamped at mount, which
    /// would be stale for any row a keyed list realized after its overlay opened.
    fn slot_depth(&self, control: ControlId) -> Option<usize> {
        let root = Host::with(|host| host.slot_of(control))?;
        self.open.iter().position(|open| open.root == root)
    }

    /// Releases everything the stack owns by itself: the slot roots, the placement rows, the
    /// subtrees and the signals under them.
    ///
    /// Does not panic, because this can run while the thread is tearing its locals down, like
    /// [`Mount`]'s own drop.
    ///
    /// A focus scope is not released here: a scope closes by an op emitted into the tick's focus
    /// buffer, and a destructor has no buffer to emit into. A scope left behind names a hit
    /// entry that has just gone, and a scope whose entry is absent bounds navigation to nothing,
    /// so `Tab` goes inert rather than walking the whole window.
    pub(crate) fn retire(&mut self, host: &mut Host) {
        if let Some((.., delay)) = self.dwell.pending.take() {
            host.cancel_delay(delay);
        }
        self.dwell.showing = None;
        self.dwell.settled = None;
        for open in self.open.drain(..).rev() {
            let Open { mut mount, blocker, root, owner, .. } = open;
            mount.retire(host);
            if let Some(blocker) = blocker {
                host.release_control(blocker);
            }
            host.unplace(root);
            // Dropped after the borrow: the owner disposes every signal the body created, and
            // a payload of one may hold a mount, whose drop reaches for the host again.
            host.retired.push(crate::build::binding::Retired::new(owner));
            host.retired.push(crate::build::binding::Retired::new(mount));
        }
    }
}

impl Drop for Overlays {
    /// Releases what the stack owns, on a thread that may already be tearing its locals down.
    fn drop(&mut self) {
        _ = Host::try_with(|host| self.retire(host));
    }
}

// ── dwell: what a pointer resting on one target opens, after how long ────────────
//
// A tooltip's show and a submenu's hover-open are one state machine, because both ask what a
// pointer that has stopped on a target is owed. A row that is both describable and expandable is
// answered once, and it opens the submenu.
//
// A submenu closes when the pointer reaches a sibling row, and a clicked flyout does not close
// when the pointer goes anywhere. Only how the overlay was opened separates them, which is why
// an overlay records that rather than inferring it from the kind.

/// How long a pointer must rest on a target before its description appears, in milliseconds.
///
/// Tuned by feel rather than derived.
pub const TIP_DELAY_MS: u32 = 500;
/// How long a pointer must rest on an expandable row before its submenu opens, in milliseconds.
///
/// Shorter than [`TIP_DELAY_MS`], because the pointer is already travelling along the rows of a
/// menu it opened rather than crossing unrelated chrome.
pub const SUBMENU_DELAY_MS: u32 = 250;
/// How long a description's fade out takes, in milliseconds.
pub const TIP_EXIT_MS: u32 = 90;
/// The gap between a control and its description, in DIPs.
///
/// A raw length rather than a palette metric, because it is measured against the control's own
/// box, which is also why [`Anchor::gap`] takes DIPs.
const TIP_GAP: f32 = 6.0;

/// What a dwell on a target will open when its delay elapses.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Owed {
    Tip,
    Submenu,
}

/// The dwell machine's state: a pending delay, the description on screen, and where this tick's
/// crossings left the pointer.
#[derive(Default)]
struct Dwell {
    /// The target the pointer is resting on, what it is owed, and the delay counting down.
    pending: Option<(ControlId, Owed, DelayId)>,
    /// The description on screen, and which target it describes.
    ///
    /// Tracked separately from the stack because a tooltip survives the pointer moving to a
    /// different describable target, swapping rather than closing, where a submenu does not. An
    /// open submenu needs no field: it is an ordinary overlay, found by its invoker.
    showing: Option<(OverlayId, ControlId)>,
    /// Where this tick's crossings ended. The outer `None` is "no crossing arrived, so nothing
    /// is owed a decision"; the inner one is "the batch ended on nothing at all".
    settled: Option<Option<ControlId>>,
}

impl Overlays {
    /// Records one hover crossing and closes any hover-opened overlay the pointer has left.
    ///
    /// Every crossing arrives here in the order the pointer made them, because the layer below
    /// publishes the whole coalesced batch rather than sampling it: a target crossed and left
    /// between two samples is a real enter and a real leave, and both are seen.
    ///
    /// Nothing is opened from a crossing. The intermediate targets of one sweep are targets the
    /// pointer passed through, so the reveal is decided once in [`Overlays::dwell_settle`].
    fn crossed(&mut self, to: Option<ControlId>, focus: &mut Vec<FocusOp>) {
        // Recorded first and unconditionally, so the batch's last crossing is the one left as the
        // answer even when the pointer went away and came back inside the batch.
        self.dwell.settled = Some(to);
        // Returns before the walk unless a hover-opened submenu is open, so the hover path pays
        // for it only when one is.
        if !self.open.iter().any(Open::is_submenu) {
            return;
        }
        // The depth the pointer is now inside: an overlay's own slot root holds its subtree, so
        // the position of that root in the stack, plus one, is how deep the target sits.
        let inside = to
            .and_then(|target| self.slot_depth(target))
            .map_or(0, |depth| depth + 1);
        // Searched from `inside` rather than from the bottom of the stack, so the pointer moving
        // back one level closes only what is above that level: with a menu, its submenu and its
        // sub-submenu open, returning to a row of the submenu takes the sub-submenu alone.
        if let Some(cut) = (inside..self.open.len()).find(|&at| self.open[at].is_submenu()) {
            self.truncate(cut, focus);
        }
    }

    /// Resolves one tick's crossings into at most one reveal.
    ///
    /// Runs once per settle, after every crossing has been seen. Only the target the pointer
    /// came to rest on is owed a delay or a description: answering a sweep across a strip of
    /// described controls per crossing would arm and tear down a delay for each, or mount and
    /// destroy a tooltip that was never on screen for a frame.
    fn dwell_settle(&mut self, focus: &mut Vec<FocusOp>) {
        let Some(settled) = self.dwell.settled.take() else {
            return;
        };
        let Some(target) = settled else {
            return self.hide(focus);
        };
        // A row that is both describable and expandable is answered once, and it opens the
        // submenu.
        if self.expands(target) {
            return self.arm(target, Owed::Submenu);
        }
        match (
            Host::with(|host| host.tip_of(target)).is_some(),
            self.dwell.showing,
        ) {
            // Re-hovering a different target while a tooltip is open swaps its content without
            // re-delaying, so scanning a toolbar shows each tip on arrival.
            (true, Some((_, shown))) if shown != target => self.show(target, focus),
            (true, None) => self.arm(target, Owed::Tip),
            (false, _) => self.hide(focus),
            (true, Some(_)) => {}
        }
    }

    /// Returns whether hovering `target` opens a nested list: an overlay that takes focus is
    /// already open, `target` has not opened one, and `target` declares a flyout.
    fn expands(&self, target: ControlId) -> bool {
        self.open.last().is_some_and(|open| open.blocker.is_some())
            && self.opened_by(target).is_none()
            && Host::with(|host| host.flyout_of(target)).is_some()
    }

    /// Arms a delay on `target`, replacing whatever was pending.
    ///
    /// A timed reveal is a scoped compositor batch and never a timer: one property set carries
    /// one delayed keyframe and its completion is the signal, because two delays sharing a key
    /// would be two animations on one property and starting the second would end the first. A
    /// pending delay asks for no frames at all.
    fn arm(&mut self, target: ControlId, owed: Owed) {
        // Still on the target already being waited for, so the wait is not restarted: a
        // description would otherwise never appear while the pointer jitters inside one control.
        if self
            .dwell
            .pending
            .is_some_and(|(held, what, ..)| (held, what) == (target, owed))
        {
            return;
        }
        self.cancel();
        let ms = if owed == Owed::Submenu {
            SUBMENU_DELAY_MS
        } else {
            TIP_DELAY_MS
        };
        let delay = Host::with(|host| host.delay(ms));
        self.dwell.pending = Some((target, owed, delay));
    }

    /// Cancels a pending delay, releasing its id and the completion it was subscribed to.
    ///
    /// Cancelling is dropping the record, which unsubscribes, so a cancelled delay is silent.
    fn cancel(&mut self) {
        if let Some((.., delay)) = self.dwell.pending.take() {
            _ = Host::try_with(|host| host.cancel_delay(delay));
        }
    }

    /// Opens whatever the pending dwell was waiting for.
    ///
    /// A `delay` that is not the pending one belongs to another requester and is ignored.
    fn elapsed(&mut self, delay: DelayId, focus: &mut Vec<FocusOp>) {
        let Some((target, owed, pending)) = self.dwell.pending else {
            return;
        };
        if pending != delay {
            return;
        }
        self.dwell.pending = None;
        Host::with(|host| host.delay_elapsed(delay));
        match owed {
            // The target may have unmounted while the delay ran: nothing to describe then, and
            // nothing to release beyond the delay id above.
            Owed::Tip => self.show(target, focus),
            Owed::Submenu => {
                // Anchored to the row's trailing edge, from which flip, slide and clamp move it
                // where there is no room.
                let mut spec = Spec::flyout(target).anchor(Anchor::below(target).side(Side::Right));
                spec.dwelled = true;
                self.open_flyout(target, spec, focus);
            }
        }
    }

    /// Opens a description of `target` carrying the text it declared, seated on the side it
    /// asked for, and closes whichever description was showing.
    ///
    /// A tooltip has no hit entry at all, so it cannot be hovered, cannot take focus, and cannot
    /// appear in a focus order. Its body is a `flyout()` surface holding one text run, and
    /// neither element declares a target, so the absence is structural rather than a flag to
    /// keep setting.
    fn show(&mut self, target: ControlId, focus: &mut Vec<FocusOp>) {
        let Some((text, side)) = Host::with(|host| host.tip_of(target)) else {
            return;
        };
        self.hide(focus);
        // On the axis the side is on, so the gap separates the two boxes rather than sliding the
        // description along the control's own edge. `place` reverses it on a flip, so one number
        // serves both directions.
        let (gx, gy) = if side.is_vertical() {
            (0.0, TIP_GAP)
        } else {
            (TIP_GAP, 0.0)
        };
        let mut spec = Spec::of(
            Kind::Tooltip,
            Anchor::below(target)
                .side(side)
                .align(Align::Center)
                .gap(gx, gy),
        );
        spec.dwelled = true;
        // Resolved once, here, rather than bound: a description is on screen for a second or two
        // against a control that is not changing underneath it, so no `Effect` or channel is
        // installed for it. The one eager resolve in the widget layer.
        let mut resolved = String::new();
        text.append(&mut resolved);
        let id = self.open(focus, spec, move |ui| {
            // Neither element declares a hit entry, so a description contributes nothing to the
            // array every pointer sample is resolved against and cannot be a target.
            crate::widget::flyout(ui).stack(|ui| {
                crate::widget::text(ui, resolved.clone());
            });
        });
        self.dwell.showing = Some((id, target));
    }

    /// Closes the description on screen and cancels any pending dwell.
    ///
    /// The single exit for a description: a press, a leave, `Esc`, focus moving and a capture
    /// loss all route here.
    fn hide(&mut self, focus: &mut Vec<FocusOp>) {
        // Clears a reveal this tick's crossings had not performed yet. A press arriving in the
        // same batch as the hover that reached the control means the pointer is being used
        // rather than rested on.
        self.dwell.settled = None;
        self.cancel();
        if let Some((id, _)) = self.dwell.showing.take() {
            self.close(id, focus);
        }
    }
}

// ── what the stack asks of the host ──────────────────────────────────────────────
//
// Each of these reads one thing off the arena or the control table on this module's behalf.
// They sit here rather than in `build/host.rs` because every one of them is an overlay
// question: which overlay a control is inside, what a row is described as, what a menu's items
// are in tree order.

impl Host {
    /// Returns the body `control` declared as its flyout, or `None` where it declared none.
    ///
    /// Cloned out, because building the body is application code and must not run under this
    /// borrow.
    pub(crate) fn flyout_of(&self, control: ControlId) -> Option<Rc<dyn Fn(&mut Ui<'_>)>> {
        let row = self.control(control)?;
        self.handlers.get(row.handlers)?.flyout.clone()
    }

    /// Returns the description `control` declared and the side it opens on.
    pub(crate) fn tip_of(&self, control: ControlId) -> Option<(Rc<TextSource>, Side)> {
        let row = self.control(control)?;
        self.handlers.get(row.handlers)?.tip.clone()
    }

    /// Returns `control`'s accessible name, which is what type-ahead matches against.
    pub(crate) fn name_of(&self, control: ControlId) -> Option<&str> {
        self.control(control)?.name.as_deref()
    }

    /// Returns whether `node`'s subtree is taking no input.
    pub(crate) fn input_suspended(&self, node: NodeId) -> bool {
        self.tree.is_live(node) && self.tree.c.flags[node.index()] & tree::SUSPENDED != 0
    }

    /// Returns the detached root `control`'s node hangs under, or `None` for window content.
    ///
    /// A parent walk rather than a control-to-overlay table stamped at mount, which would be
    /// stale for any row a keyed list realized after its overlay opened.
    pub(crate) fn slot_of(&self, control: ControlId) -> Option<NodeId> {
        let mut at = self.control(control)?.node;
        while self.tree.is_live(at) {
            let parent = self.tree.parent(at);
            if parent == NodeId::NONE {
                return (at != self.root()).then_some(at);
            }
            at = parent;
        }
        None
    }

    /// Appends every interactive control under `root` to `out`, in tree order.
    ///
    /// Tree order is the order the hit array is built in, which is the order a focus step walks,
    /// so type-ahead and arrow navigation cannot disagree about what comes next.
    pub(crate) fn interactive_under(&self, root: NodeId, out: &mut Vec<ControlId>) {
        let flags = self.tree.c.flags[root.index()];
        if flags & (tree::HIDDEN | tree::SUSPENDED) != 0 {
            return;
        }
        let decl = HitFlags::from_bits(tree::unpack_decl(flags));
        if flags & tree::HIT != 0
            && decl.contains(HitFlags::INTERACTIVE)
            && !decl.contains(HitFlags::BLOCKER)
        {
            let control = self.tree.c.control[root.index()];
            if control != ControlId::NONE {
                out.push(control);
            }
        }
        for child in self.tree.children(root) {
            self.interactive_under(child, out);
        }
    }

    /// Drops the placement row `root` heads and destroys the root, whose subtree its mount has
    /// already retired. An exit plays on a ghost the scene holds outside the tree, so the root
    /// goes at once.
    fn unplace(&mut self, root: NodeId) {
        self.overlays.retain(|placement| placement.root != root);
        self.destroy(root, Exit::None);
        self.retire_tree(&[root]);
    }
}

/// Returns the character `key` types ahead on, or `None` where it is not a type-ahead key.
///
/// The latin and digit ranges are literals because Windows defines `VK_A`..`VK_Z` and
/// `VK_0`..`VK_9` as the ASCII values themselves in a header comment rather than in a macro, so
/// no generated metadata constant names them.
const fn type_ahead(key: i32) -> Option<char> {
    match key {
        0x30..=0x39 | 0x41..=0x5A => Some(key as u8 as char),
        _ => None,
    }
}

/// Returns whether `name` answers a type-ahead of `key`, comparing first characters with case
/// folded. An absent or empty name answers nothing.
///
/// Matching runs against the accessible name rather than a list of items kept beside the menu,
/// so type-ahead walks the same controls arrow navigation does and the two cannot disagree once
/// an item is disabled. A menu row states its label as its name.
fn answers(name: Option<&str>, key: char) -> bool {
    name.and_then(|name| name.chars().next())
        .is_some_and(|first| first.eq_ignore_ascii_case(&key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::tests::fixture;
    use crate::input::{KeyEvent, Mods, PointerFlags, PointerType, Sample};
    use crate::signal::live_nodes;
    use crate::widget::{button, flyout};
    use windows_scene::{Op, Point, SinkPatch};

    // ── placement, which is pure ─────────────────────────────────────────────────

    /// Returns a rect of `w` by `h` at `x`, `y`.
    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
        }
    }

    /// Returns a size of `x` by `y`.
    fn size(x: f32, y: f32) -> Vector2 {
        Vector2 { x, y }
    }

    /// The window every placement case is resolved inside.
    const WINDOW: Vector2 = Vector2 { x: 800.0, y: 600.0 };

    /// A side seats the overlay against the named edge, and the align runs along it.
    #[test]
    fn a_side_seats_the_overlay_and_the_align_runs_along_it() {
        let target = rect(100.0, 200.0, 60.0, 20.0);
        let menu = size(120.0, 90.0);
        let under = Anchor::below(ControlId::NONE);

        assert_eq!(place(menu, target, under, WINDOW), size(100.0, 220.0));
        assert_eq!(
            place(menu, target, under.align(Align::Center), WINDOW),
            size(70.0, 220.0),
            "the midpoints did not meet"
        );
        assert_eq!(
            place(menu, target, under.align(Align::End), WINDOW),
            size(40.0, 220.0),
            "the trailing edges did not meet"
        );
        assert_eq!(
            place(menu, target, under.side(Side::Right).align(Align::Center), WINDOW),
            size(160.0, 165.0),
            "a horizontal side aligns on y"
        );
    }

    /// An overlay that overhangs the side it was seated against takes the opposite one, and the
    /// gap reverses with it.
    #[test]
    fn a_main_axis_overhang_flips_and_reverses_the_gap() {
        let low = rect(100.0, 560.0, 60.0, 20.0);
        let menu = size(120.0, 90.0);
        let under = Anchor::below(ControlId::NONE).gap(0.0, 4.0);
        assert_eq!(
            place(menu, low, under, WINDOW),
            size(100.0, 466.0),
            "the menu did not flip above the button, four DIPs clear of it"
        );
        // Tall enough that neither side fits: the chosen side stands and the clamp answers.
        let tall = size(120.0, 590.0);
        assert_eq!(
            place(tall, low, under, WINDOW).y,
            10.0,
            "a doomed flip moved"
        );
    }

    /// A cross-axis overhang slides rather than flipping, because a menu too wide for its button
    /// is not a reason to put it above the button.
    #[test]
    fn a_cross_axis_overhang_slides_and_never_flips() {
        let right_edge = rect(760.0, 200.0, 30.0, 20.0);
        let menu = size(120.0, 90.0);
        let at = place(menu, right_edge, Anchor::below(ControlId::NONE), WINDOW);
        assert_eq!(at.y, 220.0, "a wide menu flipped above its button");
        assert_eq!(
            at.x, 680.0,
            "the menu was not pulled back inside the window"
        );
    }

    /// The leading edge wins where the overlay is larger than the window, so a menu taller than
    /// the client box keeps its first items on screen.
    #[test]
    fn an_overlay_larger_than_the_window_keeps_its_leading_edge() {
        let anchor = rect(400.0, 300.0, 10.0, 10.0);
        let huge = size(900.0, 700.0);
        assert_eq!(
            place(huge, anchor, Anchor::below(ControlId::NONE), WINDOW),
            size(0.0, 0.0)
        );
    }

    /// A clamped rule stays on the side it was given and is only pulled back inside the window.
    #[test]
    fn a_clamped_rule_never_flips() {
        let low = rect(100.0, 560.0, 60.0, 20.0);
        let menu = size(120.0, 90.0);
        let fixed = Anchor {
            fit: Fit::Clamp,
            ..Anchor::below(ControlId::NONE)
        };
        assert_eq!(
            place(menu, low, fixed, WINDOW),
            size(100.0, 510.0),
            "a clamped rule flipped instead of being pulled inside"
        );
    }

    /// A modal centres and a drawer docks through the one rule, because a window anchor seats
    /// the overlay inside its box rather than beside it.
    #[test]
    fn a_window_anchor_seats_inside_the_box_it_names() {
        let client = rect(0.0, 0.0, 800.0, 600.0);
        let card = size(400.0, 200.0);
        assert_eq!(
            place(card, client, Anchor::centered(), WINDOW),
            size(200.0, 200.0),
            "the modal was not centred on both axes"
        );
        assert_eq!(
            place(card, client, Anchor::window(Side::Right), WINDOW),
            size(400.0, 200.0),
            "the drawer did not dock flush right"
        );
        // The insets the spec named shrink the box, and the dock follows them.
        let inset = rect(0.0, 48.0, 800.0, 524.0);
        let drawer = size(400.0, 524.0);
        assert_eq!(
            place(drawer, inset, Anchor::window(Side::Right), WINDOW),
            size(400.0, 48.0),
            "the drawer ignored the band above it"
        );
    }

    // ── the kind table ───────────────────────────────────────────────────────────

    /// A kind that takes focus takes the pointer with it, and only a tooltip takes neither.
    #[test]
    fn a_kind_that_takes_focus_takes_the_pointer_with_it() {
        assert_eq!(Kind::Flyout.takes_focus(), Some(false));
        assert_eq!(Kind::Popup.takes_focus(), Some(true));
        assert_eq!(Kind::Tooltip.takes_focus(), None);
        assert!(Kind::Flyout.dismiss().light);
        assert!(!Kind::Popup.dismiss().light, "a modal was light-dismissed");
        assert!(Kind::Tooltip.dismiss().focus_loss);
    }

    /// A slide states the entry and the dismissal together, so a popup leaves the way it came.
    #[test]
    fn a_slide_states_the_exit_it_implies() {
        let by = size(0.0, 1.0);
        let spec = Spec::popup().slide(by, 200, Easing::Linear);
        assert_eq!(
            spec.exit,
            Exit::Slide {
                by,
                ms: 200,
                easing: Easing::Linear
            }
        );
        let slide = spec.slide.expect("a slide");
        assert_eq!(slide.ms, 200);
        assert_eq!(
            slide.from(size(10.0, 20.0), size(0.0, 300.0)),
            size(10.0, 320.0),
            "the entry did not start one measured size away"
        );
    }

    /// A flyout restores focus to the control it was opened from; a modal names none.
    #[test]
    fn only_a_control_anchor_names_an_invoker() {
        let control = ControlId::NONE;
        assert_eq!(Spec::flyout(control).invoker(), Some(control));
        assert_eq!(Spec::popup().invoker(), None);
        assert_eq!(
            Spec::flyout(control)
                .anchor(Anchor::at(size(4.0, 4.0)))
                .invoker(),
            None
        );
    }

    /// Type-ahead reads the unshifted latin and digit ranges and nothing else, and matches an
    /// accessible name's first character with case folded.
    #[test]
    fn type_ahead_reads_the_ranges_windows_states_in_a_comment() {
        assert_eq!(type_ahead(0x41), Some('A'));
        assert_eq!(type_ahead(0x39), Some('9'));
        assert_eq!(type_ahead(VK_LEFT), None);
        assert!(answers(Some("alpha"), 'A'));
        assert!(answers(Some("Alpha"), 'a'));
        assert!(!answers(Some("beta"), 'A'));
        assert!(!answers(None, 'A'));
        assert!(!answers(Some(""), 'A'));
    }

    // ── the stack, against a mounted host ────────────────────────────────────────

    /// Collects the focus edits one pass emitted.
    #[derive(Default)]
    struct Ops(Vec<FocusOp>);

    impl Ops {
        /// Returns every scope pushed, with its trap and its restore target.
        fn pushed(&self) -> Vec<(ScopeId, bool, Option<ControlId>)> {
            self.0
                .iter()
                .filter_map(|op| match op {
                    FocusOp::Push {
                        id,
                        trap,
                        restore_to,
                        ..
                    } => Some((*id, *trap, *restore_to)),
                    _ => None,
                })
                .collect()
        }

        /// Returns every scope popped, in the order they were.
        fn popped(&self) -> Vec<ScopeId> {
            self.0
                .iter()
                .filter_map(|op| match op {
                    FocusOp::Pop(id) => Some(*id),
                    _ => None,
                })
                .collect()
        }

        fn clear(&mut self) {
            self.0.clear();
        }
    }

    /// Returns a flyout surface holding two choices.
    fn body(ui: &mut Ui<'_>) {
        flyout(ui).stack(|ui| {
            button(ui, "Alpha").name("Alpha");
            button(ui, "Beta").name("Beta");
        });
    }

    /// Mounts `declare` under the window root and flushes, so its controls exist.
    fn mounted(patch: &mut SinkPatch, declare: impl FnOnce(&mut Ui<'_>)) -> Mount {
        let mount = Ui::mount_at(
            Host::with(|host| host.root()),
            None,
            crate::build::root_scope(),
            ControlId::NONE,
            declare,
        );
        Host::flush(patch);
        patch.clear();
        mount
    }

    /// Returns a mouse contact at the origin, which is every field a press report needs.
    fn sample() -> Sample {
        Sample {
            id: 1,
            ptype: PointerType::Mouse,
            flags: PointerFlags(0),
            at: Point { x: 10.0, y: 10.0 },
            raw: Point { x: 10.0, y: 10.0 },
            contact: (0.0, 0.0),
            pen: None,
            time: 0,
            qpc: 0,
        }
    }

    /// Returns a mounted control that declares `body` as its flyout.
    fn invoker(patch: &mut SinkPatch) -> (Mount, ControlId) {
        let mut id = ControlId::NONE;
        let mount = mounted(patch, |ui| {
            id = button(ui, "Pick").name("Pick").flyout(body).control_id();
        });
        (mount, id)
    }

    /// Returns the interactive controls of the overlay at `depth`, in tree order.
    fn rows_of(overlays: &Overlays, depth: usize) -> Vec<ControlId> {
        let root = overlays.open[depth].root;
        let mut out = Vec::new();
        Host::with(|host| host.interactive_under(root, &mut out));
        out
    }

    /// Returns how many delays this patch started and cancelled, taking the patch.
    fn delay_ops(patch: &mut SinkPatch) -> (usize, usize) {
        let started = patch
            .ops()
            .iter()
            .filter(|op| matches!(op, Op::Delay { ms: Some(_), .. }))
            .count();
        let cancelled = patch
            .ops()
            .iter()
            .filter(|op| matches!(op, Op::Delay { ms: None, .. }))
            .count();
        patch.clear();
        (started, cancelled)
    }

    /// Returns the delay this patch started, taking the patch.
    fn pending_delay(patch: &mut SinkPatch) -> DelayId {
        let id = patch
            .ops()
            .iter()
            .find_map(|op| match op {
                Op::Delay { id, ms: Some(_) } => Some(*id),
                _ => None,
            })
            .expect("a dwell started no delay");
        patch.clear();
        id
    }

    /// Returns a crossing onto `to`.
    fn hover(to: Option<ControlId>) -> Report {
        Report::HoverChanged {
            from: None,
            to,
            at: Point { x: 10.0, y: 10.0 },
            qpc: 0,
        }
    }

    /// Returns a key-down report for `code`, aimed at nothing.
    fn key(code: i32) -> Report {
        Report::Key {
            target: None,
            event: KeyEvent {
                kind: KeyKind::Down,
                key: code as u16,
                repeat: false,
                mods: Mods::default(),
            },
        }
    }

    /// Runs the dwell on `target` to its reveal.
    fn dwell_open(
        overlays: &mut Overlays,
        patch: &mut SinkPatch,
        target: ControlId,
        ops: &mut Ops,
    ) {
        overlays.settle(&[hover(Some(target))], &[], &mut ops.0);
        Host::flush(patch);
        let delay = pending_delay(patch);
        overlays.scene(&[SceneEvent::DelayElapsed(delay)], &mut ops.0);
        Host::flush(patch);
        patch.clear();
    }

    /// Closing an overlay closes everything opened above it, and the scopes pop innermost first.
    #[test]
    fn closing_an_overlay_takes_everything_opened_above_it() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();

        let menu = overlays.open(&mut ops.0, Spec::flyout(anchor), body);
        overlays.open(&mut ops.0, Spec::flyout(ControlId::NONE), body);
        assert_eq!(overlays.depth(), 2);
        let opened: Vec<ScopeId> = ops.pushed().into_iter().map(|(id, ..)| id).collect();
        ops.clear();

        overlays.close(menu, &mut ops.0);
        assert!(overlays.is_empty());
        assert_eq!(
            ops.popped(),
            vec![opened[1], opened[0]],
            "the scopes did not pop innermost first"
        );
    }

    /// An id whose generation has been reused closes nothing.
    #[test]
    fn a_stale_close_is_a_miss() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();

        let first = overlays.open(&mut ops.0, Spec::flyout(anchor), body);
        overlays.close(first, &mut ops.0);
        let second = overlays.open(&mut ops.0, Spec::flyout(anchor), body);
        overlays.close(first, &mut ops.0);
        assert_eq!(
            overlays.depth(),
            1,
            "a stale id closed the overlay under it"
        );
        overlays.close(second, &mut ops.0);
        assert!(overlays.is_empty());
    }

    /// Opening emits one scope, named by its blocker and restoring to the invoker.
    #[test]
    fn opening_emits_one_scope_named_by_its_blocker() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();

        overlays.open(&mut ops.0, Spec::flyout(anchor), body);
        let pushed = ops.pushed();
        assert_eq!(pushed.len(), 1);
        assert!(!pushed[0].1, "a flyout trapped the tab order");
        assert_eq!(pushed[0].2, Some(anchor));
        let FocusOp::Push { from, .. } = ops.0[0] else {
            panic!("the first op was not a scope push")
        };
        assert!(from.is_some(), "the scope named no blocker");
        ops.clear();

        overlays.close_top(&mut ops.0);
        ops.clear();
        overlays.open(&mut ops.0, Spec::popup(), body);
        let pushed = ops.pushed();
        assert!(pushed[0].1, "a modal let the tab order out");
        assert_eq!(pushed[0].2, None, "a window anchor named a restore target");
        overlays.close_top(&mut ops.0);
    }

    /// A description contributes no focus scope, so its own `Esc` arrives as a raw key.
    #[test]
    fn a_description_contributes_no_scope_and_reads_its_own_escape() {
        let mut patch = fixture();
        let mut target = ControlId::NONE;
        let _mount = mounted(&mut patch, |ui| {
            target = button(ui, "Mute")
                .name("Mute")
                .tip("Mute this processor")
                .control_id();
        });

        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        dwell_open(&mut overlays, &mut patch, target, &mut ops);
        assert_eq!(overlays.depth(), 1, "the dwell opened no description");
        assert!(
            ops.pushed().is_empty(),
            "a description pushed a focus scope"
        );

        let mut intents = Vec::new();
        overlays.keys(&[key(VK_ESCAPE)], &mut ops.0, &mut intents);
        assert!(
            overlays.is_empty(),
            "the raw `Esc` did not close the description"
        );
    }

    /// A press on a light overlay's blocker dismisses it, and a modal's does nothing.
    #[test]
    fn a_blocker_press_dismisses_a_light_overlay_and_not_a_modal() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);

        for (spec, closes) in [(Spec::flyout(anchor), true), (Spec::popup(), false)] {
            let mut overlays = Overlays::new();
            let mut ops = Ops::default();
            overlays.open(&mut ops.0, spec, body);
            let FocusOp::Push { from, .. } = ops.0[0] else {
                panic!("the first op was not a scope push")
            };
            overlays.settle(
                &[Report::Dismiss {
                    blocker: from.expect("the scope named a blocker"),
                    scope: None,
                }],
                &[],
                &mut ops.0,
            );
            assert_eq!(overlays.is_empty(), closes, "{:?}", spec.kind());
            overlays.close_top(&mut ops.0);
        }
    }

    /// A second invoke on the control that opened a flyout shuts it.
    #[test]
    fn a_second_invoke_on_the_opener_closes_what_it_opened() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        let tap = [Intent {
            target: anchor,
            what: What::Tapped,
        }];

        overlays.settle(&[], &tap, &mut ops.0);
        assert_eq!(overlays.depth(), 1, "the tap opened no flyout");
        overlays.settle(&[], &tap, &mut ops.0);
        assert!(overlays.is_empty(), "the second tap did not shut it");
    }

    /// Losing every contact truncates from the first overlay whose policy asks for it.
    #[test]
    fn focus_loss_truncates_from_the_first_policy_that_asks() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();

        overlays.open(&mut ops.0, Spec::popup(), body);
        overlays.open(&mut ops.0, Spec::flyout(anchor), body);
        overlays.settle(&[Report::CaptureLost], &[], &mut ops.0);
        assert_eq!(
            overlays.depth(),
            1,
            "the modal, which declines focus loss, went with the flyout above it"
        );
        overlays.close_top(&mut ops.0);
    }

    /// One `Esc` takes the description, and the next reaches the menu under it.
    #[test]
    fn escape_takes_the_description_before_the_menu_under_it() {
        let mut patch = fixture();
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        overlays.open(&mut ops.0, Spec::popup(), |ui| {
            flyout(ui).stack(|ui| {
                button(ui, "Alpha").name("Alpha").tip("Describe Alpha");
            });
        });
        Host::flush(&mut patch);
        patch.clear();
        let row = rows_of(&overlays, 0)[0];

        dwell_open(&mut overlays, &mut patch, row, &mut ops);
        assert_eq!(overlays.depth(), 2, "the dwell opened no description");

        overlays.settle(&[Report::Escape { scope: None }], &[], &mut ops.0);
        assert_eq!(overlays.depth(), 1, "the first `Esc` took the menu too");
        overlays.settle(&[Report::Escape { scope: None }], &[], &mut ops.0);
        assert!(overlays.is_empty(), "the second `Esc` left the menu open");
    }

    /// One batch of crossings arms one delay, a return inside it restarts nothing, and a press
    /// in it reveals nothing.
    #[test]
    fn one_batch_of_crossings_arms_at_most_one_delay() {
        let mut patch = fixture();
        let mut ids = Vec::new();
        let _mount = mounted(&mut patch, |ui| {
            for label in ["a", "b", "c"] {
                ids.push(button(ui, label).name("row").tip("described").control_id());
            }
        });

        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        overlays.settle(
            &[
                hover(Some(ids[0])),
                hover(Some(ids[1])),
                hover(Some(ids[2])),
            ],
            &[],
            &mut ops.0,
        );
        Host::flush(&mut patch);
        assert_eq!(
            delay_ops(&mut patch),
            (1, 0),
            "a sweep armed a delay for every control it crossed"
        );

        overlays.settle(&[hover(Some(ids[1])), hover(Some(ids[2]))], &[], &mut ops.0);
        Host::flush(&mut patch);
        assert_eq!(delay_ops(&mut patch), (0, 0), "the dwell was restarted");

        overlays.settle(
            &[
                hover(Some(ids[0])),
                Report::Pressed {
                    target: ids[0],
                    contact: 1,
                    sample: sample(),
                    buttons: 1,
                },
            ],
            &[],
            &mut ops.0,
        );
        Host::flush(&mut patch);
        assert_eq!(
            delay_ops(&mut patch).0,
            0,
            "a press in the same batch still revealed a description"
        );
    }

    /// Dropping the stack releases a pending delay rather than leaving a batch open.
    #[test]
    fn dropping_the_stack_releases_a_pending_delay() {
        let mut patch = fixture();
        let mut target = ControlId::NONE;
        let _mount = mounted(&mut patch, |ui| {
            target = button(ui, "Mute")
                .name("Mute")
                .tip("described")
                .control_id();
        });

        {
            let mut overlays = Overlays::new();
            let mut ops = Ops::default();
            overlays.settle(&[hover(Some(target))], &[], &mut ops.0);
            Host::flush(&mut patch);
            assert_eq!(delay_ops(&mut patch), (1, 0));
        }
        Host::flush(&mut patch);
        assert_eq!(
            delay_ops(&mut patch),
            (0, 1),
            "the dropped stack kept its delay"
        );
    }

    /// The arrow keys emit the step they mean, and `Left` closes one level.
    #[test]
    fn the_arrow_keys_emit_the_step_they_mean_and_left_closes_a_level() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        let mut intents = Vec::new();

        overlays.open(&mut ops.0, Spec::flyout(anchor), body);
        ops.clear();
        for code in [VK_DOWN, VK_UP, VK_HOME, VK_END] {
            overlays.keys(&[key(code)], &mut ops.0, &mut intents);
        }
        assert_eq!(
            ops.0,
            vec![
                FocusOp::Step { forward: true },
                FocusOp::Step { forward: false },
                FocusOp::End { last: false },
                FocusOp::End { last: true },
            ]
        );
        assert!(intents.is_empty());
        ops.clear();

        overlays.keys(&[key(VK_LEFT)], &mut ops.0, &mut intents);
        assert!(overlays.is_empty(), "`Left` did not close a level");
        assert_eq!(ops.popped().len(), 1);
    }

    /// `Enter` invokes the focused row through the same intent a tap raises.
    #[test]
    fn enter_invokes_the_focused_row_through_the_tap_path() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        let mut intents = Vec::new();

        overlays.open(&mut ops.0, Spec::flyout(anchor), body);
        Host::flush(&mut patch);
        let focused = rows_of(&overlays, 0)[0];
        overlays.keys(
            &[Report::Key {
                target: Some(focused),
                event: KeyEvent {
                    kind: KeyKind::Down,
                    key: VK_RETURN as u16,
                    repeat: false,
                    mods: Mods::default(),
                },
            }],
            &mut ops.0,
            &mut intents,
        );
        assert_eq!(
            intents,
            vec![Intent {
                target: focused,
                what: What::Tapped
            }]
        );
        overlays.close_top(&mut ops.0);
    }

    /// Type-ahead walks the topmost overlay's own controls and cycles through the repeats.
    #[test]
    fn type_ahead_walks_the_items_of_the_topmost_overlay_and_cycles() {
        let mut patch = fixture();
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        let mut intents = Vec::new();

        overlays.open(&mut ops.0, Spec::popup(), |ui| {
            flyout(ui).stack(|ui| {
                button(ui, "Alpha").name("Alpha");
                button(ui, "Almond").name("Almond");
                button(ui, "Beta").name("Beta");
            });
        });
        Host::flush(&mut patch);
        let rows = rows_of(&overlays, 0);
        assert_eq!(rows.len(), 3, "the menu mounted the wrong rows");
        ops.clear();

        overlays.keys(&[key(0x41)], &mut ops.0, &mut intents);
        assert_eq!(ops.0, vec![FocusOp::Focus(Some(rows[0]))]);
        ops.clear();
        overlays.keys(&[key(0x41)], &mut ops.0, &mut intents);
        assert_eq!(
            ops.0,
            vec![FocusOp::Focus(Some(rows[1]))],
            "the cycle did not advance"
        );
        ops.clear();
        overlays.keys(&[key(0x42)], &mut ops.0, &mut intents);
        assert_eq!(ops.0, vec![FocusOp::Focus(Some(rows[2]))]);
        ops.clear();
        overlays.keys(&[key(0x5A)], &mut ops.0, &mut intents);
        assert!(ops.0.is_empty(), "a letter nothing answers moved focus");
        assert!(intents.is_empty());
        overlays.close_top(&mut ops.0);
    }

    /// A hover-opened submenu closes when the pointer reaches a sibling of the menu that opened
    /// it, and a clicked flyout does not close because the pointer moved.
    #[test]
    fn a_hover_open_closes_what_the_pointer_left_and_a_clicked_flyout_does_not() {
        let mut patch = fixture();
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        overlays.open(&mut ops.0, Spec::popup(), |ui| {
            flyout(ui).stack(|ui| {
                button(ui, "Alpha").name("Alpha").flyout(|ui| {
                    flyout(ui).stack(|ui| {
                        button(ui, "Leaf").name("Leaf");
                    });
                });
                button(ui, "Beta").name("Beta");
            });
        });
        Host::flush(&mut patch);
        patch.clear();
        let rows = rows_of(&overlays, 0);

        dwell_open(&mut overlays, &mut patch, rows[0], &mut ops);
        assert_eq!(overlays.depth(), 2, "the hover opened no submenu");
        overlays.settle(&[hover(Some(rows[1]))], &[], &mut ops.0);
        assert_eq!(
            overlays.depth(),
            1,
            "the submenu survived the pointer leaving it"
        );
        overlays.close_top(&mut ops.0);

        let (_mount, anchor) = invoker(&mut patch);
        overlays.open(&mut ops.0, Spec::flyout(anchor), body);
        overlays.settle(&[hover(None)], &[], &mut ops.0);
        assert_eq!(overlays.depth(), 1, "a clicked flyout closed on a hover");
        overlays.close_top(&mut ops.0);
    }

    /// Returning one level from a sub-submenu closes that level alone.
    #[test]
    fn returning_one_level_closes_only_what_is_above_it() {
        let mut patch = fixture();
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        overlays.open(&mut ops.0, Spec::popup(), |ui| {
            flyout(ui).stack(|ui| {
                button(ui, "Top").name("Top").flyout(|ui| {
                    flyout(ui).stack(|ui| {
                        button(ui, "Mid").name("Mid").flyout(|ui| {
                            flyout(ui).stack(|ui| {
                                button(ui, "Leaf").name("Leaf");
                            });
                        });
                        button(ui, "Mid2").name("Mid2");
                    });
                });
            });
        });
        Host::flush(&mut patch);
        patch.clear();

        let top = rows_of(&overlays, 0);
        dwell_open(&mut overlays, &mut patch, top[0], &mut ops);
        assert_eq!(overlays.depth(), 2);
        let mid = rows_of(&overlays, 1);
        dwell_open(&mut overlays, &mut patch, mid[0], &mut ops);
        assert_eq!(overlays.depth(), 3);

        overlays.settle(&[hover(Some(mid[1]))], &[], &mut ops.0);
        assert_eq!(
            overlays.depth(),
            2,
            "returning one level closed the level returned to"
        );
        overlays.close_top(&mut ops.0);
        overlays.close_top(&mut ops.0);
    }

    /// Closing a menu cancels the dwell it started, and the late completion opens nothing.
    #[test]
    fn closing_a_menu_cancels_the_dwell_it_started() {
        let mut patch = fixture();
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        overlays.open(&mut ops.0, Spec::popup(), |ui| {
            flyout(ui).stack(|ui| {
                button(ui, "Alpha").name("Alpha").flyout(|ui| {
                    flyout(ui).stack(|ui| {
                        button(ui, "Leaf").name("Leaf");
                    });
                });
            });
        });
        Host::flush(&mut patch);
        patch.clear();
        let row = rows_of(&overlays, 0)[0];

        overlays.settle(&[hover(Some(row))], &[], &mut ops.0);
        Host::flush(&mut patch);
        let delay = pending_delay(&mut patch);
        overlays.close_top(&mut ops.0);
        Host::flush(&mut patch);
        assert_eq!(
            delay_ops(&mut patch),
            (0, 1),
            "the close left the delay armed"
        );
        overlays.scene(&[SceneEvent::DelayElapsed(delay)], &mut ops.0);
        assert!(
            overlays.is_empty(),
            "a cancelled delay still opened a submenu"
        );
    }

    /// Opening and closing a thousand times leaks no slot root and no signal.
    #[test]
    fn a_thousand_opens_leak_no_slot_root_and_no_signal() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        let mut baseline = 0;

        for round in 0..1_000 {
            let open = overlays.open(&mut ops.0, Spec::flyout(anchor), body);
            Host::flush(&mut patch);
            overlays.close(open, &mut ops.0);
            Host::flush(&mut patch);
            patch.clear();
            ops.clear();
            if round == 0 {
                baseline = live_nodes();
            }
        }
        assert!(overlays.is_empty());
        assert_eq!(live_nodes(), baseline, "a round leaked a signal");
    }

    /// A surface still arriving takes no press: its subtree's input is held from the open,
    /// through the whole slide, and released on the compositor's report that the curve has run.
    #[test]
    fn an_entry_slide_holds_input_until_the_compositor_reports_it_complete() {
        let mut patch = fixture();
        let (_mount, anchor) = invoker(&mut patch);
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();

        let spec = Spec::flyout(anchor).slide(size(0.0, -1.0), 200, Easing::Linear);
        overlays.open(&mut ops.0, spec, body);
        let node = Host::with(|host| {
            host.overlays
                .last()
                .and_then(|placement| placement.entry)
                .expect("an entrance")
                .node
        });
        assert!(
            Host::with(|host| host.input_suspended(node)),
            "the open did not hold input"
        );

        patch.clear();
        Host::flush(&mut patch);
        assert!(
            patch.ops().iter().any(|op| matches!(
                op,
                Op::Bind {
                    id,
                    prop: Prop::Offset,
                    bind: windows_scene::Bind::Animate(windows_scene::Anim::Frames { .. }),
                } if *id == node
            )),
            "the publication did not start the slide"
        );
        assert!(
            Host::with(|host| host.input_suspended(node)),
            "input was released on the flush that started the slide"
        );

        overlays.scene(
            &[SceneEvent::AnimationCompleted { node, prop: Prop::Offset }],
            &mut ops.0,
        );
        assert!(
            !Host::with(|host| host.input_suspended(node)),
            "the completion did not release input"
        );

        overlays.close_top(&mut ops.0);
    }

    /// A declaration that flipped because the window resized keeps the application's resting
    /// intent: it neither slides in nor plays its exit.
    #[test]
    fn a_resize_suppresses_the_entry_and_the_exit() {
        let mut patch = fixture();
        let mut overlays = Overlays::new();
        let mut ops = Ops::default();
        let spec = Spec::popup().slide(size(1.0, 0.0), 200, Easing::Linear);
        let shown = crate::signal::Cell::new(true);

        // The first sync records the window, so the second is what compares against it.
        let _mount = mounted(&mut patch, |ui| {
            button(ui, "Pick").popup_when(shown, spec, || {}, body);
        });
        overlays.sync(&mut ops.0);
        Host::flush(&mut patch);
        patch.clear();

        Host::with(|host| host.set_window(size(500.0, 600.0)));
        shown.set(false);
        crate::signal::flush();
        overlays.sync(&mut ops.0);
        assert!(overlays.is_empty(), "the declaration did not close its popup");
        Host::flush(&mut patch);
        assert!(
            !patch.ops().iter().any(|op| matches!(
                op,
                Op::Drop {
                    exit: Exit::Slide { .. },
                    ..
                }
            )),
            "a resize-driven close was animated as a dismissal"
        );
    }
}
