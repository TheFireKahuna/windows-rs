//! Drags that mean two things.
//!
//! Reordering the processor chain and rescoping a processor to different channels are the
//! same physical gesture on the same object, separated by axis: vertical is order,
//! horizontal is scope. A drag whose meaning depends on its direction needs **one** policy,
//! or every consumer invents its own and they disagree.
//!
//! Five rules, and they hold for every two-axis drag in the application:
//!
//! 1. **Nothing is decided before the threshold.** Below it the gesture has no axis and no
//!    meaning, so a nudge while clicking is a click.
//! 2. **The first axis past the threshold owns the drag for its whole duration.** The lock
//!    never revisits itself, because a drag that changes meaning mid-flight is a drag the
//!    user cannot aim.
//! 3. **The locked axis is named on screen**, beside the pointer, for as long as the lock
//!    holds — which is what makes the axis decision legible instead of something the user
//!    infers from the result. This module reports the axis; the overlay layer anchors the
//!    label.
//! 4. **Commit on release is the default, and cancel aborts.** A canceled contact restores
//!    the pre-drag value — for a reorder that means the row returns to its original index,
//!    not to wherever it was hovering.
//! 5. **Displacement is a compositor animation, not a per-frame write.** The dragged row
//!    follows the contact — one control, on the frame clock — and the rows it displaces move
//!    by a retargeted chrome spring started when the insertion index changes. That belongs
//!    to the widget; what belongs here is knowing *when* the index changed.

use super::decl::{AxisLock, Commit, DragAxes, DragDecl};
use windows_scene::Point;

/// Which axis a drag locked to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// Where a drag has got to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DragPhase {
    /// Below the threshold. No axis, no meaning — and still a click if it ends here.
    Undecided,
    /// Locked to one axis for the rest of the contact.
    Locked(Axis),
    /// Both axes live. Only ever reached with [`AxisLock::None`].
    Free,
}

/// What one sample did to a drag.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DragUpdate {
    pub phase: DragPhase,
    /// Displacement from the contact's origin, in DIPs, **projected onto the locked axis**.
    /// A locked drag reports zero on the axis it does not own, so a consumer cannot
    /// accidentally act on the other one.
    pub delta: Point,
    /// Whether this sample is the one that decided the axis. The moment the label appears.
    pub decided: bool,
}

/// One contact's drag, from down to release.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Drag {
    decl: DragDecl,
    origin: Point,
    phase: DragPhase,
}

impl Drag {
    /// A drag beginning at `origin`, in client DIPs.
    #[must_use]
    pub const fn new(decl: DragDecl, origin: Point) -> Self {
        Self {
            decl,
            origin,
            phase: DragPhase::Undecided,
        }
    }

    /// Where the drag has got to.
    #[must_use]
    pub const fn phase(&self) -> DragPhase {
        self.phase
    }

    /// The axis the drag locked to, if it has locked.
    #[must_use]
    pub const fn axis(&self) -> Option<Axis> {
        match self.phase {
            DragPhase::Locked(axis) => Some(axis),
            _ => None,
        }
    }

    /// When the value takes effect.
    #[must_use]
    pub const fn commit(&self) -> Commit {
        self.decl.commit
    }

    /// Whether the contact never passed the threshold, and is therefore still a click.
    #[must_use]
    pub const fn is_click(&self) -> bool {
        matches!(self.phase, DragPhase::Undecided)
    }

    /// Advances the drag with one sample, in client DIPs.
    pub fn update(&mut self, at: Point) -> DragUpdate {
        let raw = Point {
            x: at.x - self.origin.x,
            y: at.y - self.origin.y,
        };
        let mut decided = false;

        if self.phase == DragPhase::Undecided {
            // Per axis, and only on the axes this declaration admits: a vertical-only drag
            // must not be decided by horizontal travel it will never act on.
            let (dx, dy) = (self.admits(Axis::Horizontal), self.admits(Axis::Vertical));
            let past_x = dx && raw.x.abs() >= self.decl.threshold;
            let past_y = dy && raw.y.abs() >= self.decl.threshold;
            if past_x || past_y {
                decided = true;
                self.phase = match self.decl.lock {
                    AxisLock::None => DragPhase::Free,
                    // The larger displacement wins where one sample crosses on both axes at
                    // once — which is what "first past" means when the frame clock delivered
                    // the crossing as a single batch rather than as two samples.
                    AxisLock::FirstPast if past_x && past_y => {
                        DragPhase::Locked(if raw.x.abs() >= raw.y.abs() {
                            Axis::Horizontal
                        } else {
                            Axis::Vertical
                        })
                    }
                    AxisLock::FirstPast if past_x => DragPhase::Locked(Axis::Horizontal),
                    AxisLock::FirstPast => DragPhase::Locked(Axis::Vertical),
                };
            }
        }

        DragUpdate {
            phase: self.phase,
            delta: match self.phase {
                DragPhase::Undecided => Point { x: 0.0, y: 0.0 },
                DragPhase::Locked(Axis::Horizontal) => Point { x: raw.x, y: 0.0 },
                DragPhase::Locked(Axis::Vertical) => Point { x: 0.0, y: raw.y },
                DragPhase::Free => raw,
            },
            decided,
        }
    }

    const fn admits(&self, axis: Axis) -> bool {
        matches!(
            (self.decl.axes, axis),
            (DragAxes::Both, _)
                | (DragAxes::Vertical, Axis::Vertical)
                | (DragAxes::Horizontal, Axis::Horizontal)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn drag() -> Drag {
        Drag::new(DragDecl::reorder(), at(100.0, 100.0))
    }

    #[test]
    fn nothing_is_decided_before_the_threshold_so_a_nudge_is_a_click() {
        let mut drag = drag();
        let update = drag.update(at(104.0, 103.0));
        assert_eq!(update.phase, DragPhase::Undecided);
        assert_eq!(
            update.delta,
            at(0.0, 0.0),
            "an undecided drag reports no travel"
        );
        assert!(drag.is_click());
    }

    #[test]
    fn the_first_axis_past_the_threshold_owns_the_drag_for_its_whole_duration() {
        let mut drag = drag();
        assert!(drag.update(at(100.0, 110.0)).decided);
        assert_eq!(drag.axis(), Some(Axis::Vertical));

        // A long horizontal excursion afterwards must not re-decide it, and must not leak
        // into the delta either.
        let update = drag.update(at(400.0, 130.0));
        assert_eq!(update.phase, DragPhase::Locked(Axis::Vertical));
        assert_eq!(update.delta, at(0.0, 30.0));
        assert!(!update.decided, "the lock revisited itself");
    }

    #[test]
    fn a_batch_that_crosses_both_axes_at_once_locks_to_the_larger() {
        let mut drag = drag();
        let update = drag.update(at(120.0, 108.0));
        assert_eq!(update.phase, DragPhase::Locked(Axis::Horizontal));
        assert_eq!(update.delta, at(20.0, 0.0));
    }

    #[test]
    fn a_single_axis_declaration_is_never_decided_by_the_other_one() {
        let mut drag = Drag::new(
            DragDecl {
                axes: DragAxes::Vertical,
                ..DragDecl::default()
            },
            at(0.0, 0.0),
        );
        assert_eq!(drag.update(at(500.0, 0.0)).phase, DragPhase::Undecided);
        assert_eq!(
            drag.update(at(500.0, 8.0)).phase,
            DragPhase::Locked(Axis::Vertical)
        );
    }

    #[test]
    fn an_unlocked_drag_reports_both_axes() {
        let mut drag = Drag::new(
            DragDecl {
                lock: AxisLock::None,
                ..DragDecl::default()
            },
            at(0.0, 0.0),
        );
        let update = drag.update(at(20.0, 9.0));
        assert_eq!(update.phase, DragPhase::Free);
        assert_eq!(update.delta, at(20.0, 9.0));
    }

    #[test]
    fn folding_the_batch_locks_to_the_axis_that_actually_crossed_first() {
        // The property the batch exists for. This path goes out horizontally and comes back
        // while descending, so the *newest* sample is level and below the origin — a drag
        // decided from it alone locks vertical. The crossing happened on the way out and it
        // was horizontal, and only walking every sample can see that.
        //
        // Which axis a drag owns is a **threshold crossing**: an event on the path, not a
        // state at an instant. Point-sampling an event aliases it, and here the aliasing
        // does not merely lose the gesture — it reports the opposite one.
        let path = [at(112.0, 102.0), at(108.0, 104.0), at(100.0, 112.0)];

        let mut folded = drag();
        for point in path {
            folded.update(point);
        }
        assert_eq!(folded.axis(), Some(Axis::Horizontal));

        let mut sampled = drag();
        sampled.update(path[2]);
        assert_eq!(
            sampled.axis(),
            Some(Axis::Vertical),
            "the newest sample alone reports the wrong axis, which is what folding prevents"
        );
    }

    #[test]
    fn commit_on_release_is_the_default() {
        assert_eq!(drag().commit(), Commit::OnRelease);
    }
}
