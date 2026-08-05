//! Reading back where the solve put a node.
//!
//! Layout runs one way: a declaration goes down and a box comes back, and a container that
//! places its own children never reads the box. A probe covers what that cannot express — a
//! second piece of geometry that has to agree with the first and is not inside it. A graph
//! gutter beside a list of independently sized rows is the case: each wire meets its row at
//! that row's resolved centre, and no container places both.
//!
//! # The value is one frame old
//!
//! The host writes a probe's cell during the flush; whatever reads it runs on the next
//! tick. Producing declarations from a solve inside that same solve is a fixed point, so a
//! probe reports where a node **was** put and a consumer draws against that. During a
//! resize drag the consumer trails the probed node by one frame and lands with it when the
//! drag stops.

use crate::role::WidthClass;
use crate::signal::Cell;
use windows_numerics::Vector2;
use windows_scene::{NodeId, Rect, Solved};

/// Where the solve put a node.
///
/// The four fields a consumer acts on. Narrower than [`Solved`], which also carries the
/// content size and the clipping flag a scroll container is published with.
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
    /// The width class the enclosing responsive container resolved at this node. A consumer
    /// drawing against this geometry resolves its own metrics at the same class, so both
    /// halves of one row come out at one density.
    pub class: WidthClass,
}

impl From<Solved> for Placed {
    fn from(solved: Solved) -> Self {
        Self {
            rect: solved.rect,
            size: solved.size,
            local: solved.local,
            class: solved.class,
        }
    }
}

/// A handle to where the solve put a node.
///
/// A [`Cell`], so it reads like any other signal: an [`Effect`](crate::signal::Effect) over
/// it re-runs when the node moves, and a [`Memo`](crate::signal::Memo) derived from it cuts
/// off when it does not. `Copy`, and there is nothing to unsubscribe.
///
/// Minted inside the enclosing owner, so it is disposed with the component that made it.
///
/// ```no_run
/// # use windows_ui::layout::{probe, stack};
/// # use windows_ui::widget::{caption, shown};
/// # fn f() -> windows_ui::build::View {
/// let row = probe();
/// stack((
///     caption("a row").probed(row),
///     // Reads where the row landed, one tick later.
///     caption(shown(move || row.get().rect.y0)),
/// ))
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

/// One probed node, as the flush needs it.
///
/// Holds no copy of the last published box: [`Cell::set`] compares before it propagates, so
/// the cell is the only record of whether this node moved.
pub(crate) struct ProbeRow {
    pub node: NodeId,
    pub cell: Cell<Placed>,
}
