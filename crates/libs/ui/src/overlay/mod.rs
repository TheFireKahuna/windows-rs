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

use crate::build::{Host, Mount, View, mount_at};
use crate::gesture::Recognised;
use crate::input::{FocusRing, FocusScope, KeyKind, Move, Report, ScopeId};
use crate::signal::Owner;
use crate::widget::{Intent, What};
use crate::{VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP};
use windows_numerics::Vector2;
use windows_scene::{ControlId, Exit, GroupId, HitTable, SceneEvent};

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
    dismiss: DismissPolicy,
    /// Played as the subtree is destroyed.
    exit: Exit,
    opened: Opened,
}

impl Spec {
    /// Returns a flyout anchored under `invoker`: light dismiss, `Esc`, focus restored to
    /// `invoker` on close.
    #[must_use]
    pub const fn flyout(invoker: ControlId) -> Self {
        Self {
            kind: Kind::Flyout,
            anchor: Anchor::below(invoker),
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

/// One open overlay.
///
/// Private, along with its fields. The `tip` child module is the only other reader.
struct Open {
    generation: u32,
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
}

impl Open {
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
    dwell: tip::Dwell,
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
    /// A kind that takes focus also contributes a full-window blocker entry and pushes a
    /// focus scope named by that blocker, recording the current focus as the scope's restore
    /// target. `body` runs outside every host borrow, so it may build elements, read signals
    /// and create effects that run immediately.
    pub fn open(
        &mut self,
        spec: Spec,
        focus: &mut FocusRing,
        body: impl FnOnce() -> View,
    ) -> OverlayId {
        // The depth this one opens at, which is also its placement row. Both stacks are
        // pushed and truncated together, so one position indexes either.
        let at = self.open.len() as u32;
        // The blocker, the slot root and the placement row are minted under one host borrow.
        let (blocker, root, at_scope) = Host::with(|host| {
            let blocker = spec.kind.takes_focus().map(|_| host.mint_blocker());
            let root = host.open_overlay_slot(blocker);
            host.open_overlay_placement(
                at,
                crate::build::Placement {
                    root,
                    anchor: spec.anchor,
                    at: Vector2 { x: 0.0, y: 0.0 },
                },
            );
            (blocker, root, host.root_scope)
        });

        // Mapped over the blocker rather than asking `takes_focus` again: a focus scope is
        // named by its own first entry in the hit array, and that entry is the blocker, so
        // deriving the scope from the blocker is what guarantees every scope has one.
        let scope = blocker.map(|from| {
            focus.push_scope(FocusScope {
                trap: spec.kind.takes_focus() == Some(true),
                // Captured at open, so closing puts focus back where the user left it.
                restore_to: focus.current(),
                from,
            })
        });

        let invoker = match spec.anchor.to {
            AnchorTo::Control(control) => Some(control),
            _ => None,
        };

        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.open.push(Open {
            generation,
            kind: spec.kind,
            dismiss: spec.dismiss,
            root,
            blocker,
            scope,
            invoker,
            opened: spec.opened,
            owner: None,
            mount: None,
        });

        // Outside every host borrow above: `body` is application code that builds elements,
        // reads signals, and runs any `Effect` it creates immediately.
        let (owner, mount) = Owner::scope(|| mount_at(body(), root, None, at_scope));
        let open = &mut self.open[at as usize];
        open.owner = Some(owner);
        open.mount = Some(mount);

        OverlayId {
            depth: at,
            generation,
        }
    }

    /// Closes `overlay` and everything opened above it.
    ///
    /// An id whose generation does not match the one at that depth closes nothing, so a
    /// close queued behind the close of the overlay above it is a miss rather than closing
    /// whatever has since taken the depth.
    pub fn close(&mut self, overlay: OverlayId, focus: &mut FocusRing) {
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
    pub fn close_top(&mut self, focus: &mut FocusRing) {
        if !self.open.is_empty() {
            self.truncate(self.open.len() - 1, focus);
        }
    }

    /// Closes every overlay. Called when the window loses focus, and by a screen as it
    /// unmounts, since a drop alone cannot release the focus scopes.
    pub fn close_all(&mut self, focus: &mut FocusRing) {
        self.truncate(0, focus);
    }

    /// Drops every overlay at or above `at`, innermost first.
    ///
    /// Innermost first, which is the order the focus ring and the hit array both assume: a
    /// submenu is gone before the menu that anchored it.
    fn truncate(&mut self, at: usize, focus: &mut FocusRing) {
        if at >= self.open.len() {
            return;
        }
        // Every close path arrives here, so the pending delay is cancelled once. A delay
        // outliving its menu would hold the frame clock awake for its full duration and then
        // open a submenu against a row that has gone.
        self.cancel_dwell();
        let mut restore = None;
        while self.open.len() > at {
            let Some(open) = self.open.pop() else { break };
            if let Some(scope) = open.scope {
                // Overwritten as the walk goes outward, so the outermost close is the one
                // whose restore target survives: that is the invoker focus came from.
                restore = focus.pop_scope(scope);
            }
            // The depth just vacated, which is this overlay's own id and its placement row.
            let depth = self.open.len() as u32;
            self.dwell.closed(OverlayId {
                depth,
                generation: open.generation,
            });
            // Two ordinary drops: the mount destroys the subtree with its exit, the owner
            // disposes every signal the body created.
            drop(open.mount);
            drop(open.owner);
            Host::with(|host| {
                host.close_overlay_slot(open.root, open.blocker);
                host.release_overlays_from(depth);
            });
        }
        if let Some(restore) = restore {
            _ = focus.focus(Some(restore));
        }
    }

    /// Applies the keyboard vocabulary an open overlay owns. Runs before the front table
    /// consumes the tick.
    ///
    /// Appends the [`Report::FocusChanged`] a keystroke produced to `reports`, so the focus
    /// ring moves in the same pass a pointer's move would, and appends an [`Intent`] for an
    /// invoke to `intents`, so `Enter` on a menu item reaches the handler a tap reaches.
    pub fn keys(
        &mut self,
        reports: &mut Vec<Report>,
        hits: &HitTable,
        focus: &mut FocusRing,
        intents: &mut Vec<Intent>,
    ) {
        if self.open.is_empty() {
            return;
        }
        // By index: the loop appends to `reports`, and a report this call produced must not
        // then be read back as input.
        for at in 0..reports.len() {
            let Report::Key { target, event } = reports[at] else {
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
                self.key(target, event, hits, focus, reports, intents);
            }
        }
    }

    /// Applies a tick's reports and intents to the stack. Runs after the front table has
    /// consumed them, so the press that opens an overlay here has already lit its button and
    /// no intent is the cause of a visual.
    pub fn service(
        &mut self,
        reports: &[Report],
        intents: &[Intent],
        hits: &HitTable,
        focus: &mut FocusRing,
    ) {
        for report in reports {
            self.report(report, hits, focus);
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

    fn report(&mut self, report: &Report, hits: &HitTable, focus: &mut FocusRing) {
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
            Report::HoverChanged { to, .. } => self.hovered(to, hits, focus),
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
    /// sees them. What is left is the menu vocabulary, and every arm resolves through the
    /// focus ring: `Down` is `Tab`, `Up` is `Shift-Tab`, and type-ahead walks the same
    /// candidates in the same order, so there is no item cursor to keep in step with the
    /// ring.
    fn key(
        &mut self,
        target: Option<ControlId>,
        event: crate::input::KeyEvent,
        hits: &HitTable,
        focus: &mut FocusRing,
        reports: &mut Vec<Report>,
        intents: &mut Vec<Intent>,
    ) {
        let moved = match i32::from(event.key) {
            VK_DOWN => focus.step(hits, true),
            VK_UP => focus.step(hits, false),
            VK_HOME => focus.step_to_end(hits, false),
            VK_END => focus.step_to_end(hits, true),
            // One level up, which is what `Esc` does through the router.
            VK_LEFT => {
                self.close_top(focus);
                return;
            }
            // Invoke through the ordinary tap path, so a row carrying a flyout opens its
            // submenu the same way a pointer tap on that row does.
            VK_RETURN | VK_RIGHT => {
                if let Some(target) = target {
                    intents.push(Intent {
                        target,
                        what: What::Tapped,
                    });
                }
                return;
            }
            // Type-ahead off the virtual key, which is what the router delivers: the
            // unshifted latin and digit ranges, not a general text path.
            key => {
                let Some(letter) = type_ahead(key) else {
                    return;
                };
                focus.step_to(hits, |id| {
                    Host::with(|host| menu::answers(host.name_of(id), letter))
                })
            }
        };
        if let Move::To { from, to } = moved {
            // Into the same list the front table is about to read, so the focus ring lands
            // on the new item in this pass.
            reports.push(Report::FocusChanged { from, to: Some(to) });
        }
    }

    /// Opens the flyout `target` declared, or closes the one it already has open, so a
    /// picker's own button shuts it.
    fn tapped(&mut self, target: ControlId, focus: &mut FocusRing) {
        if let Some(overlay) = self.opened_by(target) {
            self.close(overlay, focus);
            return;
        }
        self.open_flyout(target, Spec::flyout(target), focus);
    }

    /// Opens `target`'s declared flyout with `spec`, doing nothing where it declared none.
    fn open_flyout(&mut self, target: ControlId, spec: Spec, focus: &mut FocusRing) {
        // Taken out of the host borrow before it runs: building the body is application
        // code.
        let Some(body) = Host::with(|host| host.flyout_of(target)) else {
            return;
        };
        _ = self.open(spec, focus, || body());
    }

    /// Applies scene events to the stack. Only [`SceneEvent::DelayElapsed`] is acted on.
    pub fn scene(&mut self, events: &[SceneEvent], focus: &mut FocusRing) {
        for event in events {
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
    /// A focus scope is not released here, because it lives on the caller's [`FocusRing`]
    /// and a destructor cannot reach one. [`close_all`](Self::close_all) is the full
    /// teardown and is what an unmounting screen calls. A scope left behind names a hit
    /// entry that has just gone, and [`FocusRing`] resolves a scope it cannot find to
    /// nothing, so `Tab` goes inert rather than walking the whole window.
    fn drop(&mut self) {
        // A pending delay outlives the stack: its id stays claimed and its `Tick` holds the
        // frame clock awake. Nothing else releases either.
        self.cancel_dwell();
        while let Some(open) = self.open.pop() {
            drop(open.mount);
            drop(open.owner);
            let depth = self.open.len() as u32;
            _ = Host::try_with(|host| {
                host.close_overlay_slot(open.root, open.blocker);
                host.release_overlays_from(depth);
            });
        }
    }
}
