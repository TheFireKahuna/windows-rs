//! Reading back where the solve put something.
//!
//! Layout runs one way — a declaration goes down, a box comes back — and almost everything
//! is written so that the box never has to be read: a container places its children, a
//! fraction is finished against a travel the host multiplies out, a wrapping run is measured
//! rather than asked about. That is the right default and it stays the default.
//!
//! What it cannot express is a second piece of geometry that has to **agree** with the first
//! and is not inside it. A graph gutter beside a list of rows is the case: each wire meets
//! its row at that row's resolved centre, the rows are independently sized, and the gutter is
//! one column of continuous geometry rather than a cell per row — so there is no container
//! that could place both.
//!
//! # A probe is a signal, and the value is one frame old
//!
//! Writing a cell from inside the flush is safe — it marks the graph and asks for a frame,
//! and it calls no application code — but whatever reads it runs on the **next** tick. That
//! is not a limitation to work around; it is the only honest answer. Consuming a solve and
//! producing declarations from it in the same solve is a fixed point, and a layer that
//! pretended otherwise would either iterate to convergence or ship the frame where the two
//! halves disagree.
//!
//! So a probe reports where a node **was** put, and a consumer draws against that. For the
//! gutter this is exactly right: the wires follow the rows by one frame during a resize
//! drag, and land together the moment it stops.

use crate::role::WidthClass;
use crate::signal::Cell;
use windows_numerics::Vector2;
use windows_scene::{NodeId, Rect, Solved};

/// Where the solve put a node.
///
/// The four facts a consumer can act on, and no more: this is deliberately not [`Solved`],
/// which also carries the content size and the clipping flag — both of them internals of how
/// a scroll container is published, and neither meaningful to an application.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Placed {
    /// Absolute, window-relative, pixel-snapped, and **unscrolled** — where layout put the
    /// node, before any tracker offset. The space the hit array is scanned in, so a point
    /// compared against this is comparing like with like.
    pub rect: Rect,
    pub size: Vector2,
    /// Offset relative to the parent group, which is what the node's own visual carries.
    pub local: Vector2,
    /// The class the enclosing responsive container resolved here. Carried because a
    /// consumer drawing against this geometry resolves its own metrics, and resolving them
    /// at a different class than the thing it is drawing beside is how two halves of one row
    /// come out at two densities.
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
/// `Copy` and eight bytes, because it **is** a [`Cell`] — so it reads like every other
/// signal, an [`Effect`](crate::signal::Effect) over it re-runs when the node moves, and a
/// [`Memo`](crate::signal::Memo) derived from it cuts off when it does not. There is no
/// second notification mechanism, and nothing to unsubscribe.
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

/// A fresh probe, reporting the origin until the node it is attached to is solved.
///
/// The default is a zero box rather than an `Option`, because "not yet solved" and "solved
/// at zero" are the same instruction to a consumer — draw nothing there — and an `Option`
/// would put a match at every read to say so twice.
#[must_use]
pub fn probe() -> Probe {
    Probe(Cell::new(Placed::default()))
}

impl Probe {
    /// Where the node was put, registering a dependency.
    #[must_use]
    pub fn get(self) -> Placed {
        self.0.get()
    }

    /// The same, without copying it out.
    pub fn with<R>(self, f: impl FnOnce(&Placed) -> R) -> R {
        self.0.with(f)
    }

    /// The cell behind it, for the host's publish.
    pub(crate) const fn cell(self) -> Cell<Placed> {
        self.0
    }
}

/// One probed node.
///
/// **No copy of the last published box.** [`Cell::set`] compares before it propagates, so
/// the cell is already the authority on whether this node moved, and a second copy beside it
/// could only ever be the same answer or a wrong one. What it would buy is skipping a `Vec`
/// index per probed node per flush, on a table with a handful of rows.
pub(crate) struct ProbeRow {
    pub node: NodeId,
    pub cell: Cell<Placed>,
}
