//! Flyouts, popups, menus and tooltips.
//!
//! An overlay is content positioned against an anchor rather than by its parent's layout,
//! drawn above the rest of the window, with a defined way to close. The three kinds in
//! [`Kind`] differ only in their dismiss policy and their focus behaviour.
//!
//! # What an overlay is built from
//!
//! An overlay is a detached root in `windows-scene`, placed by an offset the solve reads.
//! "Press outside dismisses" is a blocker entry in the one hit array, resolved by that
//! array's own back-to-front scan. `Tab` and `Esc` are the router's focus scope. A hover-open
//! delay is a deadline compared on the frame clock. This module contributes a placement rule,
//! a lifetime, and the state machine that decides when a tooltip is showing.
//!
//! # Every overlay lives inside the window
//!
//! There is one HWND and it is composition-hosted, so an overlay is a subtree of the same
//! visual tree and cannot extend past the client box. One that would not fit is flipped, then
//! slid inward, then clamped ([`place`]).
//!
//! # Lifetime
//!
//! Opening mints a slot root and an [`Owner`]; closing drops the `Owner`, which disposes
//! every `Cell`, `Memo` and `Effect` inside it, and drops the [`Mount`], which destroys the
//! subtree with its exit transition. An overlay is never cached and hidden, because a hidden
//! overlay leaves visuals DWM still walks every frame.

mod anchor;
mod menu;
#[cfg(test)]
mod tests;
mod tip;

pub use anchor::{Align, Anchor, AnchorTo, Fit, Side, place};
pub use menu::{MenuItem, menu};
pub use tip::{SUBMENU_DELAY_MS, TIP_DELAY_MS, TIP_EXIT_MS};

use crate::build::{Host, Mount, Ui};
use crate::gesture::Recognised;
use crate::input::{KeyKind, Report, ScopeId};
use crate::seam::FocusOp;
use crate::signal::Owner;
use crate::widget::{Intent, What};
use crate::{VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP};
use windows_numerics::Vector2;
use windows_scene::{ControlId, Exit, GroupId, HitFlags, SceneEvent};

/// Returns the character `key` types ahead on, or `None` where it is not a type-ahead key.
///
/// The latin and digit ranges are literals because Windows defines `VK_A`..`VK_Z` and
/// `VK_0`..`VK_9` as the ASCII values themselves in a header comment rather than in a macro,
/// so no generated metadata constant names them.
const fn type_ahead(key: i32) -> Option<char> {
    match key {
        0x30..=0x39 | 0x41..=0x5A => Some(key as u8 as char),
        _ => None,
    }
}

/// Selects an overlay's dismiss policy and focus behaviour.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Anchored to a control. Light dismiss, `Esc`, focus loss; takes focus and restores it
    /// on close, and `Tab` past the end lets go. Menus, pickers, popovers.
    Flyout,
    /// Anchored to the window. `Esc` and an explicit close only, and it **traps** focus.
    /// Confirm dialogs, modals, the narrow-window drawer.
    Popup,
    /// Anchored to a control, and never the target of anything: no hit entry, no focus
    /// order position, no scope. Hover descriptions.
    Tooltip,
}

impl Kind {
    /// Returns the dismiss policy this kind implies, which a caller may replace with
    /// [`Spec::dismiss()`].
    #[must_use]
    pub const fn dismiss(self) -> DismissPolicy {
        match self {
            Self::Flyout => DismissPolicy {
                light: true,
                escape: true,
                focus_loss: true,
            },
            // A modal is not light-dismissed. It still contributes a blocker, so a press
            // outside it reaches nothing; that blocker's press just does nothing.
            Self::Popup => DismissPolicy {
                light: false,
                escape: true,
                focus_loss: false,
            },
            // A tooltip contributes no hit entry, so nothing in the array can dismiss it.
            // Its exits are this module's dwell machine: any press, any leave, `Esc`, or
            // focus moving.
            Self::Tooltip => DismissPolicy {
                light: false,
                escape: true,
                focus_loss: true,
            },
        }
    }

    /// Returns `Some(trap)` where the kind takes focus, `trap` being whether `Tab` may not
    /// leave it, and `None` where the kind takes no focus.
    ///
    /// The same answer decides whether the kind contributes a blocker: an overlay that takes
    /// focus also takes the pointer, so no press lands on content the keyboard cannot reach.
    /// A light overlay's blocker dismisses it and a modal's does nothing, and both exist
    /// because a focus scope is named by its own first entry in the hit array, which is that
    /// blocker.
    #[must_use]
    pub const fn takes_focus(self) -> Option<bool> {
        match self {
            Self::Flyout => Some(false),
            Self::Popup => Some(true),
            Self::Tooltip => None,
        }
    }
}

/// Declares which events close an overlay.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DismissPolicy {
    /// A press outside dismisses. The router consumes that press from this flag, so the
    /// press that closes an overlay never also invokes what it landed on.
    pub light: bool,
    /// `Esc` dismisses, as does `Tab` off the end of a scope that does not trap.
    pub escape: bool,
    /// Every contact being taken away dismisses: a lost capture, or the window losing
    /// focus.
    pub focus_loss: bool,
}

/// Identifies one open overlay by its depth in the stack and the generation occupying it.
///
/// The generation makes a stale close a miss: a close queued behind the close of the overlay
/// above it finds a different generation and does nothing, rather than closing whatever now
/// sits at that depth.
///
/// A depth is meaningful only while everything above it is still open, because the stack is
/// truncated from a depth rather than having one entry removed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct OverlayId {
    depth: u32,
    generation: u32,
}

/// Records what opened an overlay, which decides whether the pointer leaving closes it.
///
/// A hover-opened submenu and a clicked flyout are the same [`Kind`], so the kind cannot
/// carry this. [`Opened::Dwelled`] is set only by the dwell machine, so no caller can claim a
/// hover opened an overlay that no hover produced.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Opened {
    /// A tap, a keyboard invoke, or a direct [`Overlays::open()`] call.
    Invoked,
    /// The pointer rested on the invoker until its delay elapsed.
    Dwelled,
}

/// Describes how an overlay opens: its kind, anchor, dismiss policy and exit transition.
///
/// The fields are private and reachable only through the constructors. A [`Kind::Tooltip`]
/// spec can be built only inside this module, because a tooltip's lifetime belongs to the
/// dwell machine and one opened from outside would have nothing to close it; `opened` records
/// what happened rather than being a setting.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Spec {
    kind: Kind,
    anchor: Anchor,
    viewport: Option<[crate::layout::Len; 4]>,
    dismiss: DismissPolicy,
    /// Played as the subtree is destroyed.
    exit: Exit,
    slide: Option<Slide>,
    opened: Opened,
}

/// A popup moves as one compositor group, by a multiple of its measured size.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Slide {
    pub by: Vector2,
    pub ms: u32,
    pub easing: windows_scene::Easing,
}

impl Spec {
    /// Returns a flyout anchored under `invoker`: light dismiss, `Esc`, focus restored to
    /// `invoker` on close.
    #[must_use]
    pub const fn flyout(invoker: ControlId) -> Self {
        Self {
            kind: Kind::Flyout,
            anchor: Anchor::below(invoker),
            viewport: None,
            slide: None,
            dismiss: Kind::Flyout.dismiss(),
            exit: Exit::Fade { ms: 90 },
            opened: Opened::Invoked,
        }
    }

    /// Returns a modal centred in the window, trapping focus and refusing light dismiss.
    #[must_use]
    pub const fn popup() -> Self {
        Self {
            kind: Kind::Popup,
            anchor: Anchor::centered(),
            viewport: None,
            slide: None,
            dismiss: Kind::Popup.dismiss(),
            exit: Exit::Fade { ms: 120 },
            opened: Opened::Invoked,
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

    /// Constrains the popup to the window minus `[left, top, right, bottom]` insets.
    /// The viewport sizes from window input before layout; placement never feeds its size.
    #[must_use]
    pub const fn viewport(self, insets: [crate::layout::Len; 4]) -> Self {
        Self {
            viewport: Some(insets),
            ..self
        }
    }

    /// Slides the whole popup from `by` times its size, and back there on dismissal.
    /// Input stays on its blocker until the compositor reports entry complete.
    #[must_use]
    pub const fn slide(self, by: Vector2, ms: u32, easing: windows_scene::Easing) -> Self {
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

    /// Returns this spec marked as hover-opened. Private, because the dwell machine is the
    /// only opener that can record it.
    const fn dwelled(self) -> Self {
        Self {
            opened: Opened::Dwelled,
            ..self
        }
    }
}

/// Event-rate changes from popup declarations. The retained node id rejects stale opens.
pub(crate) enum Request {
    Show {
        key: windows_scene::NodeId,
        spec: Spec,
        body: std::rc::Rc<dyn Fn(&mut Ui<'_>)>,
        closed: std::rc::Rc<dyn Fn()>,
    },
    Close(windows_scene::NodeId),
}

impl Request {
    pub(crate) fn key(&self) -> windows_scene::NodeId {
        match self {
            Self::Show { key, .. } | Self::Close(key) => *key,
        }
    }
}

struct Binding {
    key: windows_scene::NodeId,
    closed: std::rc::Rc<dyn Fn()>,
}

/// One open overlay.
///
/// Private, along with its fields. The `tip` child module is the only other reader.
struct Open {
    generation: u32,
    runtime: u64,
    binding: Option<Binding>,
    kind: Kind,
    dismiss: DismissPolicy,
    root: GroupId,
    /// The full-window entry it contributes ahead of its own subtree, and the control its
    /// focus scope is named by. `None` only for a tooltip.
    blocker: Option<ControlId>,
    scope: Option<ScopeId>,
    /// The control that opened it, where one did. A second tap on that control closes the
    /// overlay, and a tooltip's hover target is compared against it.
    invoker: Option<ControlId>,
    /// What opened it; see [`Opened`].
    opened: Opened,
    /// Dropped on close, which disposes every signal the body created.
    owner: Option<Owner>,
    /// Dropped on close, which unmounts the subtree and destroys it with its exit.
    mount: Option<Mount>,
    /// The item the last type-ahead in this overlay landed on, which the next one cycles
    /// from, so repeated presses of one letter walk the items beginning with it.
    last_typeahead: Option<ControlId>,
}

impl Open {
    fn retire(&mut self, host: &mut Host, depth: u32) {
        if self.runtime != host.identity {
            return;
        }
        if let Some(mount) = &mut self.mount {
            mount.retire(host);
        }
        host.close_overlay_slot(self.root, self.blocker);
        host.release_overlays_from(depth);
        self.runtime = 0;
    }

    /// Returns whether a hover opened it rather than an invoke.
    const fn by_dwell(&self) -> bool {
        matches!(self.opened, Opened::Dwelled)
    }

    /// Returns whether it took focus, which is also whether it contributed a blocker.
    const fn takes_focus(&self) -> bool {
        self.kind.takes_focus().is_some()
    }
}

/// The overlay stack.
///
/// Overlays nest: a submenu sits above its menu and cannot outlive it, and a tooltip is
/// always topmost because any press dismisses it before anything else opens. Closing one
/// closes everything above it, which is the whole nesting policy for the slot roots, the
/// focus scopes and the placement rows alike.
#[derive(Default)]
pub struct Overlays {
    open: Vec<Open>,
    generation: u32,
    /// Names the focus scopes this stack opens. The ring mints none: it is the input half's,
    /// and an op carrying its own scope name is applied without a reply.
    scopes: u32,
    dwell: tip::Dwell,
    /// The depth an invoked choice asked to truncate to, held until the application has run
    /// the handler that choice named. See [`Overlays::after_dispatch`].
    closing: Option<usize>,
    last_window: Option<Vector2>,
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

    /// Returns the overlay `invoker` already has open, or `None`. Tooltips are skipped, so a
    /// description showing over a control does not answer as that control's flyout.
    #[must_use]
    pub fn opened_by(&self, invoker: ControlId) -> Option<OverlayId> {
        self.open
            .iter()
            .enumerate()
            .find(|(_, open)| open.invoker == Some(invoker) && open.kind != Kind::Tooltip)
            .map(|(at, open)| OverlayId {
                depth: at as u32,
                generation: open.generation,
            })
    }

    /// Opens an overlay, building `body` under a fresh detached root, and returns its id.
    ///
    /// A kind that takes focus also contributes a full-window blocker entry and emits a
    /// [`FocusOp::PushScope`] named by that blocker, with the invoker as the scope's restore
    /// target. `body` writes through a borrowed creation context; UI effects wait until
    /// that transaction releases the host.
    pub(crate) fn open(
        &mut self,
        spec: Spec,
        focus: &mut Vec<FocusOp>,
        body: impl FnOnce(&mut Ui<'_>),
    ) -> OverlayId {
        // The depth this one opens at, which is also its placement row. Both stacks are
        // pushed and truncated together, so one position indexes either.
        let at = self.open.len() as u32;
        // The blocker, the slot root and the placement row are minted under one host borrow.
        let (blocker, root, at_scope, runtime) = Host::with(|host| {
            let blocker = spec.kind.takes_focus().map(|_| host.mint_blocker());
            let root = host.open_overlay_slot(blocker);
            host.open_overlay_placement(
                at,
                crate::build::Placement {
                    root,
                    anchor: spec.anchor,
                    viewport: spec.viewport,
                    bounds: None,
                    entry: None,
                    at: Vector2 { x: 0.0, y: 0.0 },
                },
            );
            (blocker, root, host.root_scope, host.identity)
        });

        let invoker = match spec.anchor.to {
            AnchorTo::Control(control) => Some(control),
            _ => None,
        };

        // Mapped over the blocker rather than asking `takes_focus` again: a focus scope is
        // named by its own first entry in the hit array, and that entry is the blocker, so
        // deriving the scope from the blocker is what guarantees every scope has one.
        let scope = blocker.map(|from| {
            self.scopes = self.scopes.wrapping_add(1);
            let scope = ScopeId(self.scopes);
            focus.push(FocusOp::PushScope {
                scope,
                trap: spec.kind.takes_focus() == Some(true),
                from,
                // An overlay anchored to a point or to the window names no restore target,
                // and the half holding the ring fills in the focus this one interrupted.
                restore_to: invoker,
            });
            scope
        });

        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.open.push(Open {
            generation,
            runtime,
            binding: None,
            kind: spec.kind,
            dismiss: spec.dismiss,
            root,
            blocker,
            scope,
            invoker,
            opened: spec.opened,
            owner: None,
            mount: None,
            last_typeahead: None,
        });

        // The detached Owner and retained records share this creation transaction.
        let (owner, mount) = Owner::scope(|| {
            let mut mounted = Ui::mount_at(root, None, at_scope, None, body);
            mounted.set_exit(spec.exit);
            mounted
        });
        if let Some(slide) = spec.slide {
            Host::with(|h| h.open_overlay_entry(at, mount.node(), slide));
        }
        let open = &mut self.open[at as usize];
        open.owner = Some(owner);
        open.mount = Some(mount);

        OverlayId {
            depth: at,
            generation,
        }
    }

    /// Applies the popup declarations changed by the preceding signal flush.
    pub(crate) fn sync(&mut self, focus: &mut Vec<FocusOp>) {
        let window = Host::with(|h| h.model.window());
        let resized = self.last_window.is_some_and(|previous| previous != window);
        self.last_window = Some(window);
        let requests = Host::with(|h| core::mem::take(&mut h.popup_requests));
        for request in requests {
            let key = request.key();
            let existing = self
                .open
                .iter()
                .position(|open| open.binding.as_ref().is_some_and(|b| b.key == key));
            match request {
                Request::Show {
                    mut spec,
                    body,
                    closed,
                    ..
                } => {
                    if resized {
                        spec.slide = None;
                    }
                    if existing.is_none() && Host::with(|h| h.mounts.get(key).is_some()) {
                        let id = self.open(spec, focus, move |ui| body(ui));
                        self.open[id.depth as usize].binding = Some(Binding { key, closed });
                    }
                }
                Request::Close(_) => {
                    if let Some(at) = existing {
                        // A condition change (not a dismissal) preserves the application's
                        // resting intent, including a drawer closed by a window resize.
                        self.open[at].binding = None;
                        if resized && let Some(mount) = self.open[at].mount.as_mut() {
                            mount.set_exit(Exit::None);
                        }
                        self.truncate(at, focus);
                    }
                }
            }
        }
    }

    /// Closes `overlay` and everything opened above it.
    ///
    /// An id whose generation does not match the one at that depth closes nothing, so a
    /// close queued behind the close of the overlay above it is a miss rather than closing
    /// whatever has since taken the depth.
    pub(crate) fn close(&mut self, overlay: OverlayId, focus: &mut Vec<FocusOp>) {
        if self
            .open
            .get(overlay.depth as usize)
            .is_none_or(|open| open.generation != overlay.generation)
        {
            return;
        }
        self.truncate(overlay.depth as usize, focus);
    }

    /// Closes the topmost overlay, which is what `Esc` and a light-dismiss press do.
    pub(crate) fn close_top(&mut self, focus: &mut Vec<FocusOp>) {
        if !self.open.is_empty() {
            self.truncate(self.open.len() - 1, focus);
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
        // outliving its menu would hold the frame clock awake for its full duration and then
        // open a submenu against a row that has gone.
        self.cancel_dwell();
        while self.open.len() > at {
            let Some(mut open) = self.open.pop() else {
                break;
            };
            if let Some(mount) = open.mount.as_mut()
                && Host::with(|h| h.model.input_suspended(mount.node()))
            {
                mount.set_exit(Exit::None);
            }
            if let Some(scope) = open.scope {
                // Innermost first, so the outermost pop is the last one applied and its
                // restore target is where focus ends: the invoker it was opened from.
                focus.push(FocusOp::PopScope(scope));
            }
            // The depth just vacated, which is this overlay's own id and its placement row.
            let depth = self.open.len() as u32;
            self.dwell.closed(OverlayId {
                depth,
                generation: open.generation,
            });
            Host::with(|host| {
                open.retire(host, depth);
            });
            drop(open.mount);
            drop(open.owner);
            if let Some(binding) = open.binding {
                (binding.closed)();
            }
        }
    }

    /// Applies the keyboard vocabulary an open overlay owns. Runs before the front table
    /// consumes the tick.
    ///
    /// Appends the focus edit a keystroke implied to `focus`, and an [`Intent`] for an
    /// invoke to `intents`, so `Enter` on a menu item reaches the handler a tap reaches.
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
            // The router raises `Report::Escape` only where a focus scope is open, and a
            // tooltip pushes none, so a tooltip's `Esc` is read here from the raw key.
            if i32::from(event.key) == VK_ESCAPE {
                self.hide_tip(focus);
                continue;
            }
            // The menu vocabulary applies only while a focus-taking overlay is topmost;
            // otherwise arrow keys belong to whatever has focus in the window's own content.
            if self
                .open
                .last()
                .is_some_and(|open| open.kind.takes_focus().is_some())
            {
                self.key(target, event, focus, intents);
            }
        }
    }

    /// Applies a tick's reports and intents to the stack. Runs after the front table has
    /// consumed them, so the press that opens an overlay here has already lit its button and
    /// no intent is the cause of a visual.
    pub(crate) fn service(
        &mut self,
        reports: &[Report],
        intents: &[Intent],
        focus: &mut Vec<FocusOp>,
    ) {
        for report in reports {
            self.report(report, focus);
        }
        for intent in intents {
            if intent.what == What::Tapped {
                self.tapped(intent.target, focus);
            }
        }
        // Every crossing and press in the batch has been seen, so at most one target is
        // still owed a reveal.
        self.settle(focus);
    }

    fn report(&mut self, report: &Report, focus: &mut Vec<FocusOp>) {
        match *report {
            // A press on a blocker, already consumed by the router. The array puts a blocker
            // directly under the overlay it belongs to, so this closes that overlay and
            // everything above it.
            Report::Dismiss { blocker, .. } => {
                self.hide_tip(focus);
                if let Some(at) = self
                    .open
                    .iter()
                    .position(|open| open.blocker == Some(blocker))
                    && self.open[at].dismiss.light
                {
                    self.truncate(at, focus);
                }
            }
            // `Esc`, or `Tab` off the end of a scope that does not trap. Both close the
            // innermost overlay, unless its policy declines `Esc`.
            Report::Escape { .. } => {
                // A description is innermost, so one `Esc` takes it alone. Returning stops
                // the same press also closing the menu under it, because the router raises
                // `Escape` rather than a key wherever a scope is open; the next `Esc`
                // reaches the menu.
                if self.dwell.showing() {
                    self.hide_tip(focus);
                    return;
                }
                self.cancel_dwell();
                if self.open.last().is_some_and(|open| open.dismiss.escape) {
                    self.close_top(focus);
                }
            }
            // Every contact taken away, which is what the window losing focus produces.
            Report::CaptureLost => {
                self.hide_tip(focus);
                if let Some(at) = self.open.iter().position(|open| open.dismiss.focus_loss) {
                    self.truncate(at, focus);
                }
            }
            // Any press at all hides a tooltip, whether or not it was over one.
            Report::Pressed { .. } | Report::FocusChanged { .. } => self.hide_tip(focus),
            Report::HoverChanged { to, .. } => self.hovered(to, focus),
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

    /// Handles one keystroke while a focus-taking overlay is topmost.
    ///
    /// `Tab` and `Esc` never reach here, because the router takes both before any control
    /// sees them. What is left is the menu vocabulary: `Down` is `Tab`, `Up` is `Shift-Tab`,
    /// and `Home` and `End` are the ends of the scope. Each is emitted as the focus edit it
    /// means rather than performed, because the ring belongs to the half that routes input.
    fn key(
        &mut self,
        target: Option<ControlId>,
        event: crate::input::KeyEvent,
        focus: &mut Vec<FocusOp>,
        intents: &mut Vec<Intent>,
    ) {
        match i32::from(event.key) {
            VK_DOWN => focus.push(FocusOp::Step { forward: true }),
            VK_UP => focus.push(FocusOp::Step { forward: false }),
            VK_HOME => focus.push(FocusOp::StepToEnd { last: false }),
            VK_END => focus.push(FocusOp::StepToEnd { last: true }),
            // One level up, which is what `Esc` does through the router.
            VK_LEFT => self.close_top(focus),
            // Invoke through the ordinary tap path, so a row carrying a flyout opens its
            // submenu the same way a pointer tap on that row does.
            VK_RETURN | VK_RIGHT => {
                if let Some(target) = target {
                    intents.push(Intent {
                        target,
                        what: What::Tapped,
                    });
                }
            }
            // Type-ahead off the virtual key, which is what the router delivers: the
            // unshifted latin and digit ranges, not a general text path.
            key => {
                if let Some(letter) = type_ahead(key) {
                    self.type_ahead(letter, focus);
                }
            }
        }
    }

    /// Focuses the next item of the topmost overlay whose accessible name begins with
    /// `letter`, and emits nothing where none does.
    ///
    /// The candidates are the hit array's entries after that overlay's blocker filtered to
    /// `INTERACTIVE`, which is the order [`FocusOp::Step`] walks. The cycle runs from the item
    /// this overlay last typed onto rather than from the focused control, because the focused
    /// control is held by the half that routes input and cannot be read here.
    fn type_ahead(&mut self, letter: char, focus: &mut Vec<FocusOp>) {
        let Some(open) = self.open.last() else { return };
        let Some(blocker) = open.blocker else { return };
        let from = open.last_typeahead;
        let found = Host::with(|host| {
            let entries = host.model.last_hits();
            // The scope begins at its blocker and every entry of the overlay's own subtree
            // follows it, so the search is bounded to the overlay by that one position.
            let start = entries.iter().position(|entry| entry.id == blocker)?;
            let items = &entries[start + 1..];
            if items.is_empty() {
                return None;
            }
            let at = from.and_then(|id| items.iter().position(|entry| entry.id == id));
            let after = at.map_or(0, |at| at + 1);
            (0..items.len())
                .map(|step| items[(after + step) % items.len()])
                .find(|entry| {
                    entry.flags.contains(HitFlags::INTERACTIVE)
                        && !entry.flags.contains(HitFlags::BLOCKER)
                        && menu::answers(host.name_of(entry.id), letter)
                })
                .map(|entry| entry.id)
        });
        let Some(id) = found else { return };
        if let Some(open) = self.open.last_mut() {
            open.last_typeahead = Some(id);
        }
        focus.push(FocusOp::Focus(Some(id)));
    }

    /// Opens the flyout `target` declared, or closes the one it already has open, so a
    /// picker's own button shuts it.
    ///
    /// A control **inside** an open flyout that declares no flyout of its own is a terminal
    /// choice — a menu option — so invoking it closes the flyout it was chosen from, and every
    /// submenu above it. A press *outside* cannot arrive here: a [`Kind::Flyout`] contributes
    /// a blocker, and a press on that blocker is consumed as a dismiss rather than a tap. A
    /// [`Kind::Popup`] is left alone, because a button in a dialog is not a choice **from**
    /// the dialog and closing it would dismiss the dialog on its first control.
    fn tapped(&mut self, target: ControlId, focus: &mut Vec<FocusOp>) {
        if let Some(overlay) = self.opened_by(target) {
            self.close(overlay, focus);
            return;
        }
        if Host::with(|host| host.flyout_of(target)).is_none() {
            // Recorded, not performed. The flyout's body **owns** the handler this intent
            // names, and the overlay service runs before the application is dispatched to, so
            // closing here would dispose the control the intent points at and the choice would
            // be discarded as a stale id.
            self.closing = self
                .open
                .iter()
                .position(|open| open.kind == Kind::Flyout)
                .or(self.closing);
            return;
        }
        self.open_flyout(target, Spec::flyout(target), focus);
    }

    /// Closes what an invoked choice asked to close, once the application has acted on it.
    ///
    /// Called after `Host::dispatch`, which is the one point at which a menu option's handler
    /// has run and its overlay is no longer owed to anything.
    pub(crate) fn after_dispatch(&mut self, focus: &mut Vec<FocusOp>) {
        if let Some(at) = self.closing.take() {
            self.truncate(at, focus);
        }
    }

    /// Opens `target`'s declared flyout with `spec`, doing nothing where it declared none.
    fn open_flyout(&mut self, target: ControlId, spec: Spec, focus: &mut Vec<FocusOp>) {
        // Taken out of the host borrow before it runs: building the body is application
        // code.
        let Some(body) = Host::with(|host| host.flyout_of(target)) else {
            return;
        };
        _ = self.open(spec, focus, |ui| body(ui));
    }

    /// Applies scene events to the stack. Only [`SceneEvent::DelayElapsed`] is acted on.
    pub(crate) fn scene(&mut self, events: &[SceneEvent], focus: &mut Vec<FocusOp>) {
        for event in events {
            if let SceneEvent::AnimationCompleted {
                node,
                prop: windows_scene::Prop::Offset,
            } = *event
            {
                Host::with(|h| h.complete_overlay_entry(node));
            }
            if let SceneEvent::DelayElapsed { delay } = *event {
                self.dwell_elapsed(delay, focus);
            }
        }
    }
}

impl Drop for Overlays {
    /// Releases everything the stack owns by itself: the slot roots, the placement rows, the
    /// subtrees and the signals under them. Does not panic, because this can run while the
    /// thread is tearing its locals down, like [`Mount`]'s own drop.
    ///
    /// A focus scope is not released here: a scope closes by an op emitted into the tick's
    /// focus buffer, and a destructor has no buffer to emit into. A scope left
    /// behind names a hit entry that has just gone, and a scope whose entry is absent bounds
    /// navigation to nothing, so `Tab` goes inert rather than walking the whole window.
    fn drop(&mut self) {
        _ = Host::try_with(|host| self.retire(host));
    }
}

impl Overlays {
    pub(crate) fn retire(&mut self, host: &mut Host) {
        self.retire_dwell(host);
        for (depth, open) in self.open.iter_mut().enumerate().rev() {
            open.retire(host, depth as u32);
        }
    }
}
