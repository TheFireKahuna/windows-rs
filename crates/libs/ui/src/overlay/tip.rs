//! Dwell: what a pointer resting on one target opens, after how long.
//!
//! A tooltip's show and a submenu's hover-open are one state machine, because both ask what a
//! pointer that has stopped on a target is owed. A row that is both describable and expandable
//! is answered once, and it opens the submenu.
//!
//! The timing is a monotonic deadline compared on the frame clock rather than a timer: nothing
//! fires, and a pending delay holds the frame clock awake only for its own bounded,
//! user-initiated duration.
//!
//! # Three rules
//!
//! Re-hovering a different target while a tooltip is open swaps its content without
//! re-delaying. The delay stops a description appearing under a pointer merely passing
//! through, and a pointer that has already come to rest is not passing through.
//!
//! A submenu closes when the pointer reaches a sibling row, and a clicked flyout does not
//! close when the pointer goes anywhere. Only how the overlay was opened separates them, which
//! is why an overlay records that rather than inferring it from the kind.
//!
//! A tooltip has no hit entry at all, so it cannot be hovered, cannot take focus, and cannot
//! appear in a focus order. Its body is a `flyout()` surface holding one text run, and neither
//! element declares a target, so the absence is structural rather than a flag to keep setting.

use super::{Anchor, Kind, OverlayId, Overlays, Spec};
use crate::build::{Host, View};
use crate::input::FocusRing;
use windows_scene::{ControlId, DelayId, Exit, HitFlags, HitTable};

/// How long a pointer must rest on a target before its description appears, in milliseconds.
///
/// Tuned by feel rather than derived.
pub const TIP_DELAY_MS: u32 = 500;

/// How long a pointer must rest on an expandable row before its submenu opens, in
/// milliseconds. Shorter than [`TIP_DELAY_MS`], because the pointer is already travelling
/// along the rows of a menu it opened rather than crossing unrelated chrome.
pub const SUBMENU_DELAY_MS: u32 = 250;

/// How long a description's fade out takes, in milliseconds.
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

/// The dwell machine's state: a pending delay, the description on screen, and where this
/// tick's crossings left the pointer.
#[derive(Default)]
pub(super) struct Dwell {
    /// The target the pointer is resting on, what it is owed, and the delay counting down.
    pending: Option<(ControlId, Opens, DelayId)>,
    /// The description on screen, and which target it describes.
    ///
    /// Tracked separately from the stack because a tooltip survives the pointer moving to a
    /// different describable target, swapping rather than closing, where a submenu does not.
    /// An open submenu needs no field: it is an ordinary overlay, found by its invoker.
    tip: Option<(ControlId, OverlayId)>,
    /// Where this tick's crossings ended, resolved once by [`Overlays::settle`].
    settled: Settled,
}

impl Dwell {
    /// Forgets the description `overlay`, which the stack has already dropped.
    ///
    /// Called from the close path rather than into it, so a tooltip closed as part of
    /// something above it does not then close itself a second time.
    pub(super) fn closed(&mut self, overlay: OverlayId) {
        if self.tip.is_some_and(|(_, open)| open == overlay) {
            self.tip = None;
        }
    }

    /// Returns whether a description is on screen, which is what lets `Esc` dismiss it alone.
    pub(super) const fn showing(&self) -> bool {
        self.tip.is_some()
    }
}

impl Overlays {
    /// Records one hover crossing and closes any hover-opened overlay the pointer has left.
    ///
    /// Every crossing arrives here in the order the pointer made them, because the layer
    /// below publishes the whole coalesced batch rather than sampling it: a target crossed
    /// and left between two samples is a real enter and a real leave, and both are seen.
    ///
    /// Nothing is opened from a crossing. The intermediate targets of one sweep are targets
    /// the pointer passed through, so the reveal is decided once in [`settle`](Self::settle).
    pub(super) fn hovered(
        &mut self,
        to: Option<ControlId>,
        hits: &HitTable,
        focus: &mut FocusRing,
    ) {
        // Recorded first and unconditionally, so the batch's last crossing is the one left
        // as the answer even when the pointer went away and came back inside the batch.
        self.dwell.settled = Settled::At(to);
        // Still on the target already being waited for, or already described: the pointer
        // has left nothing, so nothing is closed.
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

    /// Resolves one tick's crossings into at most one reveal.
    ///
    /// Runs once per service, after every crossing has been seen. Only the target the
    /// pointer came to rest on is owed a delay or a description: answering a sweep across a
    /// strip of described controls per crossing would arm and tear down a delay for each,
    /// or mount and destroy a tooltip that was never on screen for a frame.
    pub(super) fn settle(&mut self, focus: &mut FocusRing) {
        let Settled::At(to) = core::mem::take(&mut self.dwell.settled) else {
            return;
        };
        // Still on what is already being waited for, or already described, so the wait is
        // not restarted: a description would otherwise never appear while the pointer
        // jitters inside one control. Asking this of the batch's end rather than of each
        // crossing also lets a dwell survive leaving a control and returning within one tick.
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
            // Off every target.
            self.hide_tip(focus);
            return;
        };

        // A submenu first: inside an open menu, an expandable row opens its list whether or
        // not it also carries a description.
        if self.expands(target) {
            let delay = Host::with(|host| host.model().delay(SUBMENU_DELAY_MS));
            self.dwell.pending = Some((target, Opens::Submenu, delay));
            return;
        }

        let Some((text, side)) = Host::with(|host| host.tip_of(target)) else {
            self.hide_tip(focus);
            return;
        };

        if self.dwell.tip.is_some() {
            // Swap content without re-delaying: the old closes and the new opens in the same
            // tick, so the user sees the text change rather than a gap.
            self.hide_tip(focus);
            self.show_tip(target, &text, side, focus);
            return;
        }

        let delay = Host::with(|host| host.model().delay(TIP_DELAY_MS));
        self.dwell.pending = Some((target, Opens::Tip, delay));
    }

    /// Returns whether hovering `target` opens a nested list: an overlay that takes focus is
    /// already open, `target` has not opened one, and `target` declares a flyout.
    fn expands(&self, target: ControlId) -> bool {
        self.open
            .last()
            .is_some_and(|open| open.kind.takes_focus().is_some())
            && self.opened_by(target).is_none()
            && Host::with(|host| host.flyout_of(target)).is_some()
    }

    /// Closes any hover-opened overlay the pointer has left.
    ///
    /// Containment is resolved through the hit array, which orders every overlay's entries
    /// after that overlay's own blocker, so counting the blockers ahead of a target gives the
    /// depth the target sits at. A control-to-overlay table stamped at mount would be stale
    /// for any row a keyed list realized after its overlay opened.
    ///
    /// Returns before the scan unless a dwell-opened overlay is open, so the hover path pays
    /// for it only when one is.
    fn close_stale_submenus(
        &mut self,
        to: Option<ControlId>,
        hits: &HitTable,
        focus: &mut FocusRing,
    ) {
        // A hover-opened overlay that takes focus, which is a submenu and only a submenu. A
        // description is hover-opened too and is excluded by the focus test: leaving one
        // describable control for another swaps its content rather than closing it, and a
        // description's lifetime belongs to `settle` and `hide_tip`.
        let is_submenu = |open: &super::Open| open.by_dwell() && open.takes_focus();
        if !self.open.iter().any(is_submenu) {
            return;
        }
        let entries = hits.entries();
        let at = to.and_then(|target| entries.iter().position(|entry| entry.id == target));
        // The depth the pointer is now inside: a blocker precedes its own overlay's entries,
        // so the count of blockers ahead of the target is how deep the target sits.
        let inside = at.map_or(0, |at| {
            entries[..at]
                .iter()
                .filter(|entry| entry.flags.contains(HitFlags::BLOCKER))
                .count()
        });
        // The first hover-opened overlay above that depth, and everything above it.
        //
        // Searched from `inside` rather than from the bottom of the stack, so the pointer
        // moving back one level closes only what is above that level: with a menu, its
        // submenu and its sub-submenu open, returning to a row of the submenu takes the
        // sub-submenu alone. A search from the bottom finds the submenu itself, decides it
        // is not above the pointer, and closes nothing.
        if let Some(cut) = (inside..self.open.len()).find(|&at| is_submenu(&self.open[at])) {
            self.truncate(cut, focus);
        }
    }

    /// Opens whatever the pending dwell was waiting for. A `delay` that is not the pending
    /// one belongs to another requester and is ignored.
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
                // Anchored to the row's trailing edge, from which flip, slide and clamp move
                // it where there is no room.
                let spec = Spec::flyout(target)
                    .anchor(Anchor::below(target).side(super::Side::Right))
                    .exit(Exit::Fade { ms: TIP_EXIT_MS })
                    .dwelled();
                self.open_flyout(target, spec, focus);
            }
            Opens::Tip => {
                // The target may have unmounted while the delay ran: nothing to describe
                // then, and nothing to release beyond the delay id above.
                if let Some((text, side)) = Host::with(|host| host.tip_of(target)) {
                    self.show_tip(target, &text, side, focus);
                }
            }
        }
    }

    /// Closes the description on screen and cancels any pending dwell.
    ///
    /// The single exit for a description: a press, a leave, `Esc`, focus moving and a
    /// capture loss all route here.
    pub(super) fn hide_tip(&mut self, focus: &mut FocusRing) {
        // Clears a reveal this tick's crossings had not performed yet. A press arriving in
        // the same batch as the hover that reached the control means the pointer is being
        // used rather than rested on.
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

    /// Opens a description of `target` carrying `text`, seated on `side` of the control.
    fn show_tip(
        &mut self,
        target: ControlId,
        text: &crate::widget::TextSource,
        side: super::Side,
        focus: &mut FocusRing,
    ) {
        // On the axis the side is on, so the gap separates the two boxes rather than sliding
        // the description along the control's own edge. `place` reverses it on a flip, so
        // one number serves both directions.
        let gap = if side.is_vertical() {
            (0.0, TIP_GAP_DIPS)
        } else {
            (TIP_GAP_DIPS, 0.0)
        };
        // The only place a tooltip `Spec` is built. A description's lifetime belongs to this
        // machine, so `Spec` exposes no public constructor for the kind and one opened from
        // outside would have nothing to close it.
        let spec = Spec {
            kind: Kind::Tooltip,
            // Centred on the control it describes, clear of the pointer, on the side the
            // author named, and flipped to the opposite one near a window edge.
            anchor: Anchor::below(target)
                .side(side)
                .align(super::Align::Center)
                .gap(gap.0, gap.1),
            dismiss: Kind::Tooltip.dismiss(),
            exit: Exit::Fade { ms: TIP_EXIT_MS },
            opened: super::Opened::Dwelled,
        };
        // The `TextSource` is resolved once, here, as the description opens, rather than
        // bound: a description is on screen for a second or two against a control that is
        // not changing underneath it, so no `Effect` or channel is installed for it. This is
        // the one eager resolve in the widget layer.
        let mut owned = String::new();
        text.append(&mut owned);
        let text = owned;
        let overlay = self.open(spec, focus, || tip_body(text));
        self.dwell.tip = Some((target, overlay));
    }
}

/// The gap between a control and its description, in DIPs.
///
/// A raw length rather than a palette metric, because it is measured against the control's
/// own box, which is also why [`Anchor::gap`] takes DIPs.
const TIP_GAP_DIPS: f32 = 4.0;

/// Returns a flyout surface holding one line of text and nothing else.
///
/// Neither element declares a hit entry, so a tooltip contributes nothing to the array every
/// pointer sample is resolved against and cannot be a target.
fn tip_body(text: String) -> View {
    crate::widget::flyout().stack(crate::widget::text(text))
}
