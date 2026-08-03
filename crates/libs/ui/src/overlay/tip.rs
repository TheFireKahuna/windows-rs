//! Dwell: what a pointer resting on one target opens, after how long.
//!
//! Two things in the system want "after N milliseconds of hover" — a tooltip's show and a
//! submenu's hover-open — and they are **one state machine here rather than two**, because
//! they are the same question asked of different targets: the pointer stopped somewhere, and
//! something is owed. Two would each need a rule for what happens when a row is both
//! describable and expandable, and would answer it differently.
//!
//! The timing is a monotonic deadline compared on the frame clock, never a timer: there is no
//! fourth clock, nothing fires, and a pending delay is a frame-clock requester for its own
//! bounded, user-initiated duration — which is the whole of what one costs.
//!
//! # Three rules that are easy to get backwards
//!
//! **Re-hovering a different target while a tooltip is open swaps its content without
//! re-delaying.** That is what makes scanning a toolbar tolerable: the delay exists to stop a
//! description appearing under a pointer merely passing through, and once the user has
//! demonstrably stopped to read one, the next is not passing through either.
//!
//! **A submenu closes when the pointer reaches a sibling row, and a clicked flyout does
//! not close when the pointer goes anywhere.** Both are right, and the only thing separating
//! them is *how the overlay was opened* — which is why an overlay records that rather than
//! inferring it from the kind.
//!
//! **A tooltip has no hit entry.** Not "one that ignores input" — none at all, so it cannot
//! be hovered, cannot take focus, and cannot appear in a focus order. Its body is a
//! `flyout()` surface holding one text run, and neither declares a target, so the absence is
//! structural rather than a flag someone has to keep setting.

use super::{Anchor, Kind, OverlayId, Overlays, Spec};
use crate::build::{Host, View};
use crate::input::FocusRing;
use windows_scene::{ControlId, DelayId, Exit, HitFlags, HitTable};

/// How long a pointer must rest on a target before its description appears.
///
/// Tuned by feel rather than derived, and deliberately on the slow side: a hover-open that is
/// too eager is worse than one that is too slow, because the eager one covers what the user
/// was reaching for.
pub const TIP_DELAY_MS: u32 = 500;

/// How long a submenu waits. Shorter than a tooltip's, because the pointer is already inside
/// a menu the user opened deliberately and is travelling along a list of rows rather than
/// crossing unrelated chrome.
pub const SUBMENU_DELAY_MS: u32 = 250;

/// How long a description's fade out takes. Short, because it is leaving something the user
/// has stopped looking at.
pub const TIP_EXIT_MS: u32 = 90;

/// What a dwell on a target will open when its delay elapses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Opens {
    Tip,
    Submenu,
}

/// Where a tick's crossings left the pointer, held until the batch is over.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
enum Settled {
    /// No crossing arrived, so nothing is owed a decision.
    #[default]
    Unmoved,
    /// The batch ended on this target, or on nothing at all.
    At(Option<ControlId>),
}

/// The dwell's whole state, which is three optionals.
#[derive(Default)]
pub(super) struct Dwell {
    /// The target the pointer is resting on, what it is owed, and the delay counting down.
    pending: Option<(ControlId, Opens, DelayId)>,
    /// The description on screen, and which target it describes.
    ///
    /// Tracked separately from the stack because a tooltip survives the pointer moving to a
    /// *different* describable target — it swaps rather than closing — where a submenu does
    /// not. An open submenu needs no field: it is an ordinary overlay, found by its invoker.
    tip: Option<(ControlId, OverlayId)>,
    /// Where this tick's crossings ended, resolved once by [`Overlays::settle`].
    settled: Settled,
}

impl Dwell {
    /// Forgets the description, because the stack has already dropped it.
    ///
    /// Called from the close path rather than calling into it, so a tooltip closed *as part
    /// of* something above it closing does not try to close itself a second time.
    pub(super) fn closed(&mut self, overlay: OverlayId) {
        if self.tip.is_some_and(|(_, open)| open == overlay) {
            self.tip = None;
        }
    }

    /// Whether a description is on screen. What lets `Esc` dismiss the description alone.
    pub(super) const fn showing(&self) -> bool {
        self.tip.is_some()
    }
}

impl Overlays {
    /// A hover crossing. **Every** crossing arrives here, in the order the pointer made them,
    /// because the layer below publishes the whole coalesced batch rather than sampling it —
    /// so a target crossed and left between two samples is a real enter and a real leave, and
    /// this sees both.
    ///
    /// **Closing is decided here; revealing is not.** A real leave is what closes a
    /// hover-opened submenu, and that is the reason the batch is published unsampled. Nothing
    /// is *opened* from a crossing, because the intermediate targets of one sweep are targets
    /// the pointer passed through — see [`settle`](Self::settle), which is where the one that
    /// matters is answered.
    pub(super) fn hovered(
        &mut self,
        to: Option<ControlId>,
        hits: &HitTable,
        focus: &mut FocusRing,
    ) {
        // Recorded first and unconditionally: the batch's *last* crossing is the one owed an
        // answer, and an excursion away and back must not leave the excursion as the answer.
        self.dwell.settled = Settled::At(to);
        // Still on the target already being waited for, or already described. Nothing has
        // moved for either half, and closing work keyed on a target the pointer has not
        // actually left would be closing it for having stayed.
        if self
            .dwell
            .pending
            .is_some_and(|(target, ..)| Some(target) == to)
            || self.dwell.tip.is_some_and(|(target, _)| Some(target) == to)
        {
            return;
        }
        self.close_stale_submenus(to, hits, focus);
    }

    /// Resolves one tick's crossings into **at most one** reveal.
    ///
    /// Run once per service, after every crossing has been seen. A pointer sweeping across a
    /// strip of described controls crosses each of them, and answering per crossing means
    /// arming and tearing down a delay for every one — or worse, while a description is
    /// already up, mounting a tooltip and destroying it again for each, every intermediate
    /// one leaving a ghost that holds the frame clock for its exit. None of them was ever on
    /// screen for a frame. Only where the pointer came to rest is owed anything.
    ///
    /// The batch is still read whole, and that is the point: the closing half needs every
    /// crossing, and the revealing half needs the last.
    pub(super) fn settle(&mut self, focus: &mut FocusRing) {
        let Settled::At(to) = core::mem::take(&mut self.dwell.settled) else {
            return;
        };
        // Still on what is already being waited for, or already described. Re-armed, a
        // description would never appear while the pointer jitters inside one control — and
        // because this is asked of the batch's end rather than of each crossing, a dwell now
        // also survives leaving a control and returning within one tick.
        if self
            .dwell
            .pending
            .is_some_and(|(target, ..)| Some(target) == to)
            || self.dwell.tip.is_some_and(|(target, _)| Some(target) == to)
        {
            return;
        }
        self.cancel_dwell();

        let Some(target) = to else {
            // Off every target at all.
            self.hide_tip(focus);
            return;
        };

        // A submenu first: inside an open menu, an expandable row is expandable whether or
        // not it also carries a description, and opening the list is the more specific
        // answer. This is the case two separate state machines would each answer alone.
        if self.expands(target) {
            let delay = Host::with(|host| host.model().delay(SUBMENU_DELAY_MS));
            self.dwell.pending = Some((target, Opens::Submenu, delay));
            return;
        }

        let Some(text) = Host::with(|host| host.tip_of(target)) else {
            self.hide_tip(focus);
            return;
        };

        if self.dwell.tip.is_some() {
            // Swap content without re-delaying: the old closes and the new opens in the same
            // tick, so the user sees the text change rather than a gap.
            self.hide_tip(focus);
            self.show_tip(target, &text, focus);
            return;
        }

        let delay = Host::with(|host| host.model().delay(TIP_DELAY_MS));
        self.dwell.pending = Some((target, Opens::Tip, delay));
    }

    /// Whether hovering `target` should open a nested list: it declares a flyout, something
    /// that takes focus is already open, and it has not opened one already.
    fn expands(&self, target: ControlId) -> bool {
        self.open
            .last()
            .is_some_and(|open| open.kind.takes_focus().is_some())
            && self.opened_by(target).is_none()
            && Host::with(|host| host.flyout_of(target)).is_some()
    }

    /// Closes any hover-opened overlay the pointer has left.
    ///
    /// Containment is resolved through the **hit array** rather than through a control-to-
    /// overlay table stamped at mount: the array already orders every overlay's entries after
    /// its own blocker, so "is this control inside that overlay" is a comparison of two
    /// positions in the one authority — and a table would be a second one, wrong for any row
    /// a keyed list realized after its overlay opened.
    ///
    /// Gated on a dwell-opened overlay actually being open, which is almost never, so the
    /// scan is off the hover path in every case but the one that needs it.
    fn close_stale_submenus(
        &mut self,
        to: Option<ControlId>,
        hits: &HitTable,
        focus: &mut FocusRing,
    ) {
        // A hover-opened overlay that takes focus — which is to say a submenu, and only a
        // submenu. A description is hover-opened too and is deliberately **not** one of
        // these: leaving one describable control for another swaps its content rather than
        // closing it, and its whole lifetime is `settle`'s and `hide_tip`'s. Closing it from
        // here instead is what made the swap re-delay, so scanning a strip of described
        // controls waited the full delay again at every one.
        let is_submenu = |open: &super::Open| open.by_dwell() && open.takes_focus();
        if !self.open.iter().any(is_submenu) {
            return;
        }
        let entries = hits.entries();
        let at = to.and_then(|target| entries.iter().position(|entry| entry.id == target));
        // The depth the pointer is now inside: a blocker precedes its own overlay's entries,
        // so counting the blockers ahead of the target *is* how deep the target sits.
        let inside = at.map_or(0, |at| {
            entries[..at]
                .iter()
                .filter(|entry| entry.flags.contains(HitFlags::BLOCKER))
                .count()
        });
        // The first hover-opened overlay **above** that depth, and everything above it.
        //
        // Searched from `inside` rather than from the bottom of the stack, because the
        // pointer moving back one level must close only what is above that level: with a
        // menu, its submenu and its sub-submenu all open, returning to a row of the submenu
        // leaves the submenu and takes the one above it. Searching from the bottom finds the
        // submenu itself, decides it is not above the pointer, and closes nothing.
        if let Some(cut) = (inside..self.open.len()).find(|&at| is_submenu(&self.open[at])) {
            self.truncate(cut, focus);
        }
    }

    /// A delay met its deadline. Anything but the one being waited for is somebody else's.
    pub(super) fn dwell_elapsed(&mut self, delay: DelayId, focus: &mut FocusRing) {
        let Some((target, opens, pending)) = self.dwell.pending else {
            return;
        };
        if pending != delay {
            return;
        }
        self.dwell.pending = None;
        Host::with(|host| host.model().delay_elapsed(delay));
        match opens {
            Opens::Submenu => {
                // Anchored to the row's trailing edge, which is where a nested list belongs
                // and where flip/slide/clamp will move it from if there is no room.
                let spec = Spec::flyout(target)
                    .anchor(Anchor::below(target).side(super::Side::Right))
                    .exit(Exit::Fade { ms: TIP_EXIT_MS })
                    .dwelled();
                self.open_flyout(target, spec, focus);
            }
            Opens::Tip => {
                // Unmounted while we waited: nothing to describe, and nothing to clean up
                // beyond the id just released.
                if let Some(text) = Host::with(|host| host.tip_of(target)) {
                    self.show_tip(target, &text, focus);
                }
            }
        }
    }

    /// Hides whatever is described and cancels whatever is pending.
    ///
    /// The single exit: a press, a leave, `Esc`, focus moving and a capture loss all mean
    /// this, and routing them through one function is what stops a description surviving one
    /// of the five because a branch was missed.
    pub(super) fn hide_tip(&mut self, focus: &mut FocusRing) {
        // Including a reveal this tick's crossings had not performed yet. A press arriving in
        // the same batch as the hover that reached the button means the pointer is being used
        // rather than rested on, and a description owed a moment ago is not owed now.
        self.dwell.settled = Settled::Unmoved;
        self.cancel_dwell();
        if let Some((_, overlay)) = self.dwell.tip.take() {
            self.close(overlay, focus);
        }
    }

    /// Cancels a pending delay, releasing its id and the frame clock it was holding.
    pub(super) fn cancel_dwell(&mut self) {
        if let Some((.., delay)) = self.dwell.pending.take() {
            Host::with(|host| host.model().cancel_delay(delay));
        }
    }

    fn show_tip(
        &mut self,
        target: ControlId,
        text: &crate::widget::TextSource,
        focus: &mut FocusRing,
    ) {
        // The one place a tooltip spec is built, which is why `Spec` has no public
        // constructor for one: a description's lifetime is this machine's, and an overlay of
        // this kind opened from outside would have nothing left to close it.
        let spec = Spec {
            kind: Kind::Tooltip,
            // Below the control it describes, centred on it, clear of the pointer. It flips
            // above near the bottom edge like anything else.
            anchor: Anchor::below(target)
                .align(super::Align::Center)
                .gap(0.0, TIP_GAP_DIPS),
            dismiss: Kind::Tooltip.dismiss(),
            exit: Exit::Fade { ms: TIP_EXIT_MS },
            opened: super::Opened::Dwelled,
        };
        // Read **once**, here, as the description opens.
        //
        // It is on screen for a second or two against a control that is not changing
        // underneath it, so binding it would install an `Effect` and a channel per tooltip
        // for a value nothing rebinds. This is the one place in the widget layer where a
        // `TextSource` is resolved eagerly, and the reason is that its lifetime is shorter
        // than the thing it describes.
        let text = text.read(str::to_owned);
        let overlay = self.open(spec, focus, || tip_body(text));
        self.dwell.tip = Some((target, overlay));
    }
}

/// The gap between a control and its description, in DIPs.
///
/// A *position* rather than a design metric — it is measured against the control's own box,
/// which the palette does not own — and the same reason [`Anchor::gap`] takes raw DIPs.
const TIP_GAP_DIPS: f32 = 4.0;

/// A flyout surface holding one line, and **nothing else**.
///
/// No hit entry is declared by either element, so a tooltip is structurally incapable of
/// being a target. That is what "never hit-testable" has to mean here: a flag that said so
/// would still cost an entry in the array every pointer sample is resolved against.
fn tip_body(text: String) -> View {
    crate::widget::flyout().stack(crate::widget::text(text))
}
