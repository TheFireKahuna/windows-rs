//! Reading back where the solve put a node.
//!
//! Layout runs one way: a declaration goes down and a box comes back, and a container that
//! places its own children never reads the box. A probe covers what that cannot express — a
//! second piece of geometry that has to agree with the first and is not inside it. A graph
//! gutter beside a list of independently sized rows is the case: each wire meets its row at
//! that row's resolved centre, and no container places both.
//!
//! # Which phase reads it decides which solve it reports
//!
//! The host publishes every probe during the flush, after the last solve and before the
//! signal graph's geometry phase runs. A geometry job therefore reads **this** batch's
//! settled box and publishes path verbs in the same scene patch:
//! [`Ui::local_geometry`](crate::build::Ui::local_geometry),
//! [`Ui::local_geometries`](crate::build::Ui::local_geometries) and
//! [`Ui::anchored_geometries`](crate::build::Ui::anchored_geometries) are that phase's entry
//! points, and they are the ones to reach for when a second piece of geometry has to agree
//! with a box in the frame it was solved in.
//!
//! An ordinary [`Effect`](crate::signal::Effect) or [`Memo`](crate::signal::Memo) is on the
//! update phase, which ran before the publication, so it reports where the node **was** put
//! and is woken for the next flush. Producing *declarations* from a solve inside that same
//! solve is a fixed point, which is why the update phase cannot be moved: during a resize
//! drag an update-phase consumer trails the probed node by one frame and lands with it when
//! the drag stops.
//!
//! [`Anchors`](super::Anchors) covers the keyed case: many child boxes, in one container's
//! own space, under the application's own identities.

use super::Rect;
use crate::signal::Cell;
use windows_numerics::Vector2;

/// Where the solve put a node.
///
/// The four fields a consumer acts on, which is narrower than the node's own solved row.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Placed {
    /// Absolute, window-relative, pixel-snapped and **unscrolled**: where layout put the
    /// node, before any tracker offset. The hit array is scanned in this same space, so a
    /// reported point compares directly against it.
    pub rect: Rect,
    /// The node's solved size.
    pub size: Vector2,
    /// Offset relative to the parent group, which is what the node's own visual carries.
    pub local: Vector2,
    /// The enclosing scope at the width class the solve resolved this node under, published
    /// after the first solve. A consumer drawing against this geometry resolves its own
    /// metrics through it, so both halves of one row come out at one density.
    pub scope: Option<crate::role::Scope>,
}

/// A handle to where the solve put a node.
///
/// A [`Cell`], so it reads like any other signal: an [`Effect`](crate::signal::Effect) over
/// it re-runs when the node moves, and a [`Memo`](crate::signal::Memo) derived from it cuts
/// off when it does not. `Copy`, and there is nothing to unsubscribe. The reader's phase
/// decides which solve it reports.
///
/// Minted inside the enclosing owner, so it is disposed with the component that made it.
///
/// ```no_run
/// # use windows_ui::layout::{probe, stack};
/// # use windows_ui::widget::{caption, shown};
/// # fn f(ui: &mut windows_ui::build::Ui<'_>) {
/// let row = probe();
/// stack(ui, |ui| {
///     caption(ui, "a row").probed(row);
///     // An update-phase read, so it reports the previous solve.
///     caption(ui, shown(move || row.get().rect.y0));
/// });
/// # }
/// ```
#[derive(Copy, Clone, Debug)]
pub struct Probe(Cell<Placed>);

/// Returns a fresh probe, reading a zero box until the node it is attached to is solved.
///
/// A zero box rather than an `Option`: an unsolved node and one solved at the origin with
/// no extent are the same instruction to a consumer, so a read needs no match.
#[must_use]
pub fn probe() -> Probe {
    Probe(Cell::new(Placed::default()))
}

/// Two probes are the same probe where they are the same cell, whatever each last read.
impl PartialEq for Probe {
    fn eq(&self, other: &Self) -> bool {
        self.0.id() == other.0.id()
    }
}

impl Probe {
    /// Returns where the node was put, registering a dependency for the reading effect or
    /// memo.
    #[must_use]
    pub fn get(self) -> Placed {
        self.0.get()
    }

    /// Calls `f` with the placement in place, without copying it out.
    pub fn with<R>(self, f: impl FnOnce(&Placed) -> R) -> R {
        self.0.with(f)
    }

    /// Returns the cell behind the probe, which the host writes during the flush.
    pub(crate) const fn cell(self) -> Cell<Placed> {
        self.0
    }
}
