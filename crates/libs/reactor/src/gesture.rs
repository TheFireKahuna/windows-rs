//! Gestures — the one way pointer input is allowed to drive a visual.
//!
//! ## The rule this module exists to enforce
//!
//! An [`Intent`](crate::backend::dcomp::record::Intent) is a **post-visual
//! notification**: by the time one is queued, the pixels it describes have
//! already moved. A slider snaps its thumb inside the input router and *then*
//! tells the app its value changed; the app hop is decoupled from the visual, so
//! a busy app thread cannot stall the drag.
//!
//! Viz surfaces used to break that rule. Their sinks were app-thread closures,
//! so a pointer move produced no pixels at all until the intent had crossed the
//! seam and the app had run — which put reconcile load directly in the path of
//! the one gesture the split exists to make fast. The rule was stated in prose,
//! nothing checked it, and it was violated for a year without anyone noticing.
//!
//! A gesture makes the violation unrepresentable rather than merely discouraged:
//!
//! * The handler is **`Send`**, so a closure capturing app-thread hook state
//!   fails to compile at the registration site. That is the whole enforcement
//!   mechanism — the compiler is the auditor.
//! * The handler receives **no backend handle** — only the transition. It cannot
//!   reach the retained tree, take a second borrow, or re-enter the router. The
//!   same discipline the TSF bridge keeps by convention, kept here by signature.
//! * Its only channel to the app is an [`ActionSlot`], which is *coalescing*.
//!   There is no way to ask the app to render something per-move, because there
//!   is no per-move message.
//!
//! A gesture is therefore always able to run on whichever thread owns input, and
//! whether it publishes anything visible is up to what it captures — typically a
//! shared draw slot its renderer reads. The reactor never sees that slot: the
//! handler captures it, and the drawing side is entirely the app's business.
//!
//! ## What the app still gets
//!
//! Everything a gesture wants to *persist* — a committed value, a document edit
//! — travels as an ordinary action. The gesture posts the newest one to an
//! [`ActionSlot`] and returns [`GestureOutcome::Notify`]; the reactor wakes the
//! app once per burst, and the app drains the latest. Fifty moves between two
//! app turns collapse to one action and one wake, so the notification path gets
//! *cheaper* under load rather than more expensive.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::style::PointerEventInfo;

/// One pointer transition delivered to a gesture.
///
/// [`Exit`](Self::Exit) carries no sample: it fires when the hover leaves the
/// element, which has no meaningful position on the element itself.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GestureEvent {
    /// A button went down on the element. Starts a captured drag.
    Down(PointerEventInfo),
    /// The pointer moved — over the element while hovering, or anywhere at all
    /// while a drag started here holds capture.
    Move(PointerEventInfo),
    /// The captured drag ended. Always delivered, wherever the release landed.
    Up(PointerEventInfo),
    /// The wheel turned over the element. Read the delta through
    /// [`PointerEventInfo::wheel_delta_on`] to ignore the axis you did not mean.
    Wheel(PointerEventInfo),
    /// The hover left the element. Hover-only — a captured drag suppresses hover
    /// routing, so no exit arrives mid-drag.
    Exit,
}

impl GestureEvent {
    /// The pointer sample, or `None` for [`Exit`](Self::Exit).
    #[must_use]
    pub fn info(&self) -> Option<&PointerEventInfo> {
        match self {
            Self::Down(i) | Self::Move(i) | Self::Up(i) | Self::Wheel(i) => Some(i),
            Self::Exit => None,
        }
    }
}

/// What a gesture reports after handling a transition.
///
/// Deliberately not a `bool`: the two cases mean quite different things at a
/// call site, and the one that costs a thread wake should have to be named.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[must_use = "a gesture's outcome decides whether the app is woken"]
pub enum GestureOutcome {
    /// Handled entirely here. Whatever this transition changed visually has
    /// already been published; the app has nothing to learn from it.
    Handled,
    /// An action was posted to an [`ActionSlot`] and the app should drain it.
    /// Return this only via [`ActionSlot::post`], which suppresses the wake when
    /// one is already in flight.
    Notify,
}

/// Which transitions a gesture wants routed to it.
///
/// Declared once, at registration, with every field known — so a surface is
/// never half-registered. (The sink API this replaces filled one cell per
/// builder call and redeclared after each, which meant a surface could be routed
/// to while still incompletely wired.)
///
/// The router reads these bits to decide routing, not just delivery: a gesture
/// with no [`down`](Self::down) leaves the element **click-transparent**, so a
/// press falls through to whatever lies beneath it — a button layered over a
/// plot keeps working.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GestureInterest {
    pub down: bool,
    pub moved: bool,
    pub up: bool,
    pub wheel: bool,
    pub exited: bool,
}

impl GestureInterest {
    /// A draggable surface: every transition, including the wheel and the
    /// hover-exit that ends a highlight. The EQ-plot shape.
    #[must_use]
    pub const fn drag() -> Self {
        Self { down: true, moved: true, up: true, wheel: true, exited: true }
    }

    /// A hover-only surface: moves and the exit that ends them, and **nothing
    /// else** — so it stays click-transparent. The shape for a display that
    /// lights up under the pointer without being a drag target.
    #[must_use]
    pub const fn hover() -> Self {
        Self { down: false, moved: true, up: false, wheel: false, exited: true }
    }

    /// Whether any transition is wanted. A gesture interested in nothing is not
    /// routed at all.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.down || self.moved || self.up || self.wheel || self.exited
    }
}

/// The typed, coalescing channel from a gesture to the app.
///
/// A gesture [`post`](Self::post)s the newest action; the app [`take`](Self::take)s
/// it. Only the newest survives — an action is a *statement of where the gesture
/// now is*, not an event in a stream, so superseding one loses nothing. That is
/// what makes the path cheapen under load: a fast drag between two app turns
/// costs one action and one wake instead of one of each per move.
///
/// ## The wake handshake
///
/// `pending` is the empty→full edge detector. [`post`](Self::post) returns
/// [`GestureOutcome::Notify`] only on that edge, so a burst of moves queues one
/// intent rather than one per move.
///
/// [`take`](Self::take) clears the flag **before** reading the slot, and the
/// order is load-bearing. Clearing after would lose an action written in the
/// window between the read and the clear: the writer would see `pending` still
/// set, suppress its wake, and the action would sit in the slot with nothing
/// scheduled to collect it. Clearing first can only cost a spurious wake, which
/// is harmless.
///
/// ## Why a `Mutex` here is fine
///
/// The writer is the input thread, which must never block. It doesn't
/// meaningfully: this lock is held for a move and an `Option` write, and the
/// only other party takes it just as briefly. That is quite different from a
/// draw slot, where a renderer holds the lock for a whole snapshot and a plain
/// `Mutex` really would stall the pump.
pub struct ActionSlot<A> {
    slot: Mutex<Option<A>>,
    pending: AtomicBool,
}

impl<A> ActionSlot<A> {
    /// An empty slot.
    #[must_use]
    pub fn new() -> Self {
        Self { slot: Mutex::new(None), pending: AtomicBool::new(false) }
    }

    /// Post the newest action, replacing any not yet drained.
    ///
    /// Returns the [`GestureOutcome`] to return from the gesture: `Notify` on
    /// the empty→full edge, `Handled` when a wake is already in flight.
    pub fn post(&self, action: A) -> GestureOutcome {
        if let Ok(mut g) = self.slot.lock() {
            *g = Some(action);
        }
        if self.pending.swap(true, Ordering::AcqRel) {
            GestureOutcome::Handled
        } else {
            GestureOutcome::Notify
        }
    }

    /// Take the newest action, if any. Called by the app on its wake.
    ///
    /// Clears the pending flag first — see the handshake note on the type.
    pub fn take(&self) -> Option<A> {
        self.pending.store(false, Ordering::Release);
        self.slot.lock().ok().and_then(|mut g| g.take())
    }
}

impl<A> Default for ActionSlot<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A> std::fmt::Debug for ActionSlot<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionSlot")
            .field("pending", &self.pending.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first post wakes; further posts before a drain do not.
    #[test]
    fn only_the_first_post_of_a_burst_notifies() {
        let s = ActionSlot::new();
        assert_eq!(s.post(1), GestureOutcome::Notify);
        assert_eq!(s.post(2), GestureOutcome::Handled);
        assert_eq!(s.post(3), GestureOutcome::Handled);
    }

    /// A burst collapses to its newest action — the whole point of coalescing.
    #[test]
    fn take_yields_only_the_newest() {
        let s = ActionSlot::new();
        let _ = s.post(1);
        let _ = s.post(2);
        let _ = s.post(3);
        assert_eq!(s.take(), Some(3));
        assert_eq!(s.take(), None);
    }

    /// After a drain the next post wakes again, or a later burst would never be
    /// collected.
    #[test]
    fn a_drain_rearms_the_wake() {
        let s = ActionSlot::new();
        let _ = s.post(1);
        assert_eq!(s.take(), Some(1));
        assert_eq!(s.post(2), GestureOutcome::Notify);
    }

    /// The clear-before-read order: an action posted between the clear and the
    /// read is still delivered, because the writer sees a cleared flag and
    /// schedules its own wake. Written as the interleaving it defends against.
    #[test]
    fn an_action_racing_the_drain_is_not_lost() {
        let s = ActionSlot::new();
        let _ = s.post(1);

        // `take` clears first...
        s.pending.store(false, Ordering::Release);
        // ...the writer lands here, sees a clear flag, and arms a fresh wake.
        assert_eq!(s.post(2), GestureOutcome::Notify);
        // ...and the drain reads whatever is newest.
        assert_eq!(s.slot.lock().unwrap().take(), Some(2));
    }

    /// `Exit` is the one transition with no position.
    #[test]
    fn exit_carries_no_sample() {
        assert!(GestureEvent::Exit.info().is_none());
        assert!(GestureEvent::Move(PointerEventInfo::default()).info().is_some());
    }

    /// A hover gesture must stay click-transparent, or a button under a plot
    /// stops working.
    #[test]
    fn hover_interest_takes_no_presses() {
        let h = GestureInterest::hover();
        assert!(!h.down && !h.up && !h.wheel);
        assert!(h.moved && h.exited);
        assert!(h.any());
    }
}
