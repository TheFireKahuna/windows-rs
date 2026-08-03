//! Flyouts, popups, menus and tooltips.
//!
//! An overlay is content **positioned against an anchor rather than by its parent's
//! layout**, sitting above everything, with a defined way to go away. Three kinds cover
//! every case in an application, and they differ only in their dismiss policy and their
//! focus behaviour — which is why [`Kind`] is an enum and not three types.
//!
//! # Nothing here is a new mechanism
//!
//! That is the whole design, and it is what keeps this module small. An overlay is a
//! detached root in `windows-scene`, placed by an offset the solve gathers from. "Press
//! outside dismisses" is a blocker entry in the *one* hit array, which the array's own
//! back-to-front scan resolves for free — no capture, no z-index, no case in the router.
//! `Tab` and `Esc` are the router's focus scope. A hover-open delay is a deadline read on the
//! frame clock, because there is no fourth clock to ask. This module contributes a placement
//! rule, a
//! lifetime, and the state machine that decides when a tooltip is showing. It contributes no
//! parallel path to anything.
//!
//! # Every overlay lives inside the window
//!
//! There is one HWND and it is composition-hosted, so an overlay is a subtree of the same
//! visual tree and cannot extend past the client box. One that would not fit is flipped,
//! then slid inward, then clamped ([`place`]). A second HWND would need its own compositor
//! target, its own DPI handling and its own input plumbing, for content that is always
//! within a few hundred DIPs of its anchor.
//!
//! # Lifetime
//!
//! Opening mints a slot root and an [`Owner`]; closing drops the `Owner`, which disposes
//! every `Cell`, `Memo` and `Effect` inside it, and drops the [`Mount`], which destroys the
//! subtree with its exit transition. **There is no cached-and-hidden overlay**: a hidden
//! overlay is visuals DWM still walks, and visual count is the idle frontier.

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

/// The character a virtual key types-ahead on, where it is one of them.
///
/// The latin and digit keys are stated as literals because they have no metadata constants:
/// Windows defines `VK_A`..`VK_Z` and `VK_0`..`VK_9` as the ASCII values themselves, in a
/// header comment rather than in a macro, so there is nothing for the generator to have
/// emitted. Recognising the key and naming its character are one decision, so they are one
/// function and the narrowing is exact by construction.
const fn type_ahead(key: i32) -> Option<char> {
    match key {
        0x30..=0x39 | 0x41..=0x5A => Some(key as u8 as char),
        _ => None,
    }
}

/// Which of the three an overlay is.
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
    /// The dismiss policy this kind implies. A caller may override it; most do not.
    #[must_use]
    pub const fn dismiss(self) -> DismissPolicy {
        match self {
            Self::Flyout => DismissPolicy {
                light: true,
                escape: true,
                focus_loss: true,
            },
            // A modal a stray click could close is not a modal. It still contributes a
            // blocker — see `takes_focus` — the blocker just does nothing.
            Self::Popup => DismissPolicy {
                light: false,
                escape: true,
                focus_loss: false,
            },
            // A tooltip is dismissed by this layer's own state machine — any press, any
            // leave, `Esc`, focus moving — and never by an entry in the array, because it
            // has none to be dismissed through.
            Self::Tooltip => DismissPolicy {
                light: false,
                escape: true,
                focus_loss: true,
            },
        }
    }

    /// Whether it takes focus, and if so whether `Tab` may leave it.
    ///
    /// This is also what decides whether it contributes a blocker, and the two are the same
    /// question: **an overlay that takes focus must also take the pointer**, or a press
    /// lands on content the keyboard cannot reach. A light overlay's blocker dismisses it
    /// and a modal's does nothing, but both exist — and a scope needs a first entry to be
    /// named by, which is exactly what a blocker is.
    #[must_use]
    pub const fn takes_focus(self) -> Option<bool> {
        match self {
            Self::Flyout => Some(false),
            Self::Popup => Some(true),
            Self::Tooltip => None,
        }
    }
}

/// What closes an overlay.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DismissPolicy {
    /// A press outside dismisses. **The press is consumed** — the router does that, from
    /// the flag — because dismiss-and-act would make an accidental menu open cost an
    /// unintended edit.
    pub light: bool,
    pub escape: bool,
    /// Every contact being taken away dismisses: a lost capture, or the window losing
    /// focus.
    pub focus_loss: bool,
}

/// An overlay's identity: where it sits in the stack, and which occupant of that depth it is.
///
/// Generational, so a close arriving after the overlay went — a submenu's own, queued behind
/// the menu's — finds nothing rather than closing whatever now sits at that depth.
///
/// **Not a slot id**, and it does not borrow the packing of one: overlays genuinely nest, so
/// the stack is a `Vec` truncated from a depth and an index into it is only meaningful while
/// everything above is still open. Naming the two halves is what says so.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct OverlayId {
    depth: u32,
    generation: u32,
}

/// What opened an overlay, which is what decides whether the pointer leaving closes it.
///
/// Not derivable from the kind: a hover-opened submenu and a clicked flyout are the same
/// kind, opened the same way, and the right answer to "the pointer moved away" is opposite in
/// each. Private, and set only by the dwell machine — so a caller cannot mint a hover-opened
/// overlay by hand and get the submenu behaviour on something no hover produced.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Opened {
    Invoked,
    Dwelled,
}

/// How an overlay is opened.
///
/// The fields are private and the constructors are the only doors, because two of them are
/// not free choices: [`Kind::Tooltip`] has its lifetime owned by this module's dwell machine
/// and would be unclosable if a caller could ask for one, and whether a hover opened it is a
/// record of what happened rather than a setting.
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
    /// A flyout under a control: light dismiss, `Esc`, focus restored to the invoker.
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

    /// A modal against the window, trapping focus.
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

    /// Which of the three it is.
    #[must_use]
    pub const fn kind(self) -> Kind {
        self.kind
    }

    #[must_use]
    pub const fn anchor(self, anchor: Anchor) -> Self {
        Self { anchor, ..self }
    }

    #[must_use]
    pub const fn dismiss(self, dismiss: DismissPolicy) -> Self {
        Self { dismiss, ..self }
    }

    #[must_use]
    pub const fn exit(self, exit: Exit) -> Self {
        Self { exit, ..self }
    }

    /// Records that a hover opened this rather than an invoke. Not public: the dwell machine
    /// is the only thing that can honestly say so.
    const fn dwelled(self) -> Self {
        Self {
            opened: Opened::Dwelled,
            ..self
        }
    }
}

/// One open overlay.
///
/// Private, and the fields with it: `tip` is a child module and sees them, and nothing else
/// in the crate has business reading an overlay's internals.
struct Open {
    generation: u32,
    kind: Kind,
    dismiss: DismissPolicy,
    root: GroupId,
    /// The full-window entry it contributes ahead of its own subtree, and the control its
    /// focus scope is named by. `None` only for a tooltip.
    blocker: Option<ControlId>,
    scope: Option<ScopeId>,
    /// The control that opened it, where one did. What a second tap on the same button
    /// toggles against, and what a tooltip's hover is compared to.
    invoker: Option<ControlId>,
    /// What opened it. See [`Opened`] — the whole submenu-versus-flyout distinction.
    opened: Opened,
    /// Dropped on close, which disposes every signal the body created.
    owner: Option<Owner>,
    /// Dropped on close, which unmounts the subtree and destroys it with its exit.
    mount: Option<Mount>,
}

impl Open {
    /// Whether a hover opened it rather than an invoke.
    const fn by_dwell(&self) -> bool {
        matches!(self.opened, Opened::Dwelled)
    }

    /// Whether it took focus, which is also whether it contributed a blocker.
    const fn takes_focus(&self) -> bool {
        self.kind.takes_focus().is_some()
    }
}

/// The overlay stack.
///
/// **A stack and not a set**, because overlays genuinely nest: a submenu sits above its menu
/// and cannot outlive it, and a tooltip is always topmost because any press dismisses it
/// before anything else can open. So closing one takes everything above it, and that single
/// rule is the whole nesting policy — for the slot roots, the focus scopes and the placement
/// rows alike, none of which needs its own.
#[derive(Default)]
pub struct Overlays {
    open: Vec<Open>,
    generation: u32,
    dwell: tip::Dwell,
}

impl Overlays {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many are open. Zero is the common case, and the one the idle path cares about.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.open.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// The overlay a control already has open, if any. What makes a second tap on a picker
    /// close it rather than open a second one.
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

    /// Opens one, building `body` inside a fresh scope under a fresh detached root.
    pub fn open(
        &mut self,
        spec: Spec,
        focus: &mut FocusRing,
        body: impl FnOnce() -> View,
    ) -> OverlayId {
        // The depth this one opens at, which is also its placement row's. The two stacks are
        // pushed and truncated together, so the position is the only index either needs.
        let at = self.open.len() as u32;
        // One borrow, not three. An overlay that takes focus must also take the pointer, so
        // the blocker is minted here with the rest rather than beside them.
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

        // Over the **blocker** and not over `takes_focus` a second time: a scope is named by
        // its own first entry in the array, and that entry *is* the blocker. Deriving it
        // from the same value that produced the blocker is what makes the two facts one
        // fact — asking twice would let a future kind grow a scope with nothing to name it,
        // and the only symptom would be a `Tab` that silently walks the whole window.
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

        // Outside every borrow above, and it has to be: `body` is application code — it
        // builds elements, reads signals, and an `Effect` it creates runs immediately.
        let (owner, mount) = Owner::scope(|| mount_at(body(), root, None, at_scope));
        let open = &mut self.open[at as usize];
        open.owner = Some(owner);
        open.mount = Some(mount);

        OverlayId {
            depth: at,
            generation,
        }
    }

    /// Closes one, and everything opened above it.
    ///
    /// A stale id is a **miss**: a submenu's own close, queued behind its menu's, finds a
    /// different generation and does nothing rather than closing whatever is now at that
    /// depth.
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

    /// Closes the topmost, which is what `Esc` and a light-dismiss press mean.
    pub fn close_top(&mut self, focus: &mut FocusRing) {
        if !self.open.is_empty() {
            self.truncate(self.open.len() - 1, focus);
        }
    }

    /// Closes everything. What the window losing focus means, and what an unmounting screen
    /// owes the disposal walk.
    pub fn close_all(&mut self, focus: &mut FocusRing) {
        self.truncate(0, focus);
    }

    /// Drops every overlay at or above `at`, innermost first.
    ///
    /// Innermost first because that is the order the focus ring and the hit array both
    /// already assume: a submenu is gone before the menu that anchored it.
    fn truncate(&mut self, at: usize, focus: &mut FocusRing) {
        if at >= self.open.len() {
            return;
        }
        // Whatever the pointer was owed is owed by something that is closing. Cancelled
        // here and not at each call site, because `Esc`, `Left`, a light press and the layer
        // above's own `close` all arrive through this one function — and a delay that
        // outlives its menu holds the frame clock awake for its whole duration and then
        // opens a submenu against a row that has gone.
        self.cancel_dwell();
        let mut restore = None;
        while self.open.len() > at {
            let Some(open) = self.open.pop() else { break };
            if let Some(scope) = open.scope {
                // Answers what focus should become. The *outermost* close is the one whose
                // answer survives, because that is the invoker the user actually came from.
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

    /// The keyboard vocabulary an open overlay owns, run **before** the front table
    /// consumes the tick.
    ///
    /// Before, because a keystroke that moves focus has to move the focus ring's pixels in
    /// the same pass a pointer's would: it appends the [`Report::FocusChanged`] it made to
    /// the same list the front table is about to read, so there is one focus-visual path
    /// and not two. It appends an [`Intent`] for an invoke for the same reason — a menu
    /// item pressed with `Enter` reaches the handler a click reaches, through the one
    /// dispatch point.
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
        // By index, because the loop appends to the list it is reading — and a report this
        // call produced is not one it should then interpret.
        for at in 0..reports.len() {
            let Report::Key { target, event } = reports[at] else {
                continue;
            };
            if event.kind != KeyKind::Down {
                continue;
            }
            // `Esc` closing a tooltip is **this** layer's, not the router's. The router
            // raises `Escape` only where a focus scope is open, and a tooltip deliberately
            // pushes none — so without this arm a description could not be dismissed from
            // the keyboard at all, and the only symptom is one that will not go away.
            if i32::from(event.key) == VK_ESCAPE {
                self.hide_tip(focus);
                continue;
            }
            // The rest is the menu vocabulary, and it applies only while a focus-taking
            // overlay is topmost: arrow keys inside the window's own content belong to
            // whatever has focus there.
            if self
                .open
                .last()
                .is_some_and(|open| open.kind.takes_focus().is_some())
            {
                self.key(target, event, hits, focus, reports, intents);
            }
        }
    }

    /// What the tick's reports and intents mean for the stack, run **after** the front
    /// table has consumed them.
    ///
    /// After, because by the time an overlay opens here the press that opened it has
    /// already lit its button. No intent may be the cause of a visual, and that ordering is
    /// what makes it so rather than something to remember.
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
        // Every crossing in the batch has been seen, and every press with them. At most one
        // of them is owed a reveal, and this is the one call that can know which.
        self.settle(focus);
    }

    fn report(&mut self, report: &Report, hits: &HitTable, focus: &mut FocusRing) {
        match *report {
            // A press on a blocker. The array puts a blocker directly under the overlay it
            // belongs to, so this closes that one and everything above it — and the press
            // itself was already consumed by the router.
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
            // `Esc`, or `Tab` off the end of a scope that does not trap. Both mean "close
            // the innermost scope"; one that declines to escape keeps it.
            Report::Escape { .. } => {
                // A description is the innermost thing on screen, and dismissing it is the
                // whole of what this keystroke means. Returning here is what stops one `Esc`
                // taking the tooltip *and* the menu under it: the router raises `Escape`
                // rather than a key wherever a scope is open, so both would otherwise be
                // read from the same press. The next `Esc` reaches the menu.
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
            // A right tap opens the target's flyout **at the pointer** rather than under
            // the control, because a context menu's origin is a discrete decision.
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

    /// One keystroke, while a focus-taking overlay is open.
    ///
    /// `Tab` and `Esc` never reach here — the router takes both before any control sees
    /// them, which is what makes an open overlay closable from the keyboard whether or not
    /// the pointer is near it. What is left is the menu vocabulary, and every arm of it
    /// resolves through the **focus ring**: `Down` is `Tab`, `Up` is `Shift-Tab`, and
    /// type-ahead walks the same candidates in the same order. A menu carrying its own item
    /// cursor beside that ring would be a second order to keep in step with the first,
    /// which is the failure the one hit array exists to prevent.
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
            // One level up. `Esc` means the same thing and is already the router's.
            VK_LEFT => {
                self.close_top(focus);
                return;
            }
            // Invoke, which for a row carrying a flyout is "open the submenu" — and that is
            // the ordinary tap path rather than a second one, so a submenu opened by
            // keyboard and one opened by pointer are the same overlay opened the same way.
            VK_RETURN | VK_RIGHT => {
                if let Some(target) = target {
                    intents.push(Intent {
                        target,
                        what: What::Tapped,
                    });
                }
                return;
            }
            // Type-ahead. Off the virtual key rather than a character message, because that
            // is what the router delivers — so this is the unshifted latin and digit range
            // and deliberately not a general text path. A menu whose items need one is a
            // list, and a list has a filter field.
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
            // Into the same list the front table is about to read, so the ring lands on the
            // new item this pass — the one focus-visual path, reached the one way.
            reports.push(Report::FocusChanged { from, to: Some(to) });
        }
    }

    /// A tap on a control that declared a flyout opens it — or closes the one it already
    /// has, so a picker's own button is what shuts it.
    fn tapped(&mut self, target: ControlId, focus: &mut FocusRing) {
        if let Some(overlay) = self.opened_by(target) {
            self.close(overlay, focus);
            return;
        }
        self.open_flyout(target, Spec::flyout(target), focus);
    }

    /// Opens a control's declared flyout, if it declared one. Private: `tip` is a child
    /// module and reaches it, and nothing outside this one should.
    fn open_flyout(&mut self, target: ControlId, spec: Spec, focus: &mut FocusRing) {
        // Taken out of the borrow before it runs: building the body is application code.
        let Some(body) = Host::with(|host| host.flyout_of(target)) else {
            return;
        };
        _ = self.open(spec, focus, || body());
    }

    /// What the scene reported. One thing reaches here: a delay that met its deadline.
    pub fn scene(&mut self, events: &[SceneEvent], focus: &mut FocusRing) {
        for event in events {
            if let SceneEvent::DelayElapsed { delay } = *event {
                self.dwell_elapsed(delay, focus);
            }
        }
    }
}

impl Drop for Overlays {
    /// Releases everything a stack owns **on its own**: the slot roots, the placement rows,
    /// the subtrees and the signals under them. Non-panicking, for the reason [`Mount`]'s own
    /// drop is: this can run while the thread is tearing its locals down, and a panic in a
    /// drop takes the process with it.
    ///
    /// A focus scope is not one of those things — it lives on the caller's [`FocusRing`], and
    /// a destructor cannot reach one. [`close_all`](Self::close_all) is the full teardown and
    /// is what an unmounting screen should call. What a drop leaves behind is bounded rather
    /// than dangerous: a scope names its own first entry in the hit array, that entry has
    /// just gone, and [`FocusRing`] resolves a scope it cannot find to nothing at all — so a
    /// stranded scope makes `Tab` inert instead of letting it walk the whole window.
    fn drop(&mut self) {
        // A pending delay outlives the stack: its id is still claimed and its `Tick` is
        // still holding the frame clock awake. Nothing else releases either.
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
