//! Reading back where the solve put a container's keyed children.
//!
//! [`Probe`](super::Probe) answers for one node, in window space, and is named at the point
//! the node is declared. A set of rows whose boxes a *fourth* node draws against needs
//! neither: the drawing node is not their parent, the set of rows changes while the screen
//! is up, and every read would otherwise subtract the drawing node's own origin. An anchor
//! set answers all three — one table, keyed by the application's own identity, in the origin
//! container's space.
//!
//! The table is one signal. A key appearing, moving or leaving changes it, so a consumer
//! subscribed to the set is woken by a row that did not exist when it last ran, and needs no
//! generation counter of its own. A key whose node has been unmounted leaves the table at
//! the next publication: ids carry a generation, so a recycled slot never answers for the
//! key that named the node before it.

use crate::role::Scope;
use crate::signal::Cell;
use windows_numerics::Vector2;
use super::Rect;

/// One keyed child box, in the origin container's own space.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Anchored {
    /// The application identity the child attached under.
    pub key: u64,
    /// The child's box, relative to the origin container's top-left corner.
    pub rect: Rect,
}

/// Where the solve put every keyed child of one container.
///
/// Read through [`Anchors::with`]. Ordered by attachment, not by key: a consumer that wants
/// one box asks for it by key, and one that draws the whole set walks it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    /// The origin container's own solved size.
    pub size: Vector2,
    /// The origin container's enclosing scope, absent until the first publication.
    scope: Option<Scope>,
    pub(crate) boxes: Vec<Anchored>,
}

impl Table {
    /// Returns the origin container's own solved size.
    #[must_use]
    pub fn size(&self) -> Vector2 {
        self.size
    }

    /// Returns the origin container's enclosing scope.
    ///
    /// # Panics
    ///
    /// If the set has never been published. A reader is woken by a publication and the
    /// publication writes the origin first, so every reader has one.
    #[must_use]
    pub fn scope(&self) -> Scope {
        self.scope
            .expect("an anchor set is read through the publication that wrote its origin")
    }

    /// Returns the origin's scope, or `None` before the set's first publication.
    #[must_use]
    pub(crate) fn published(&self) -> Option<Scope> {
        self.scope
    }

    /// Records the origin container's own box and scope.
    pub(crate) fn set_origin(&mut self, size: Vector2, scope: Scope) {
        self.size = size;
        self.scope = Some(scope);
    }

    /// Attaches one child's box, already rebased onto the origin.
    pub(crate) fn push(&mut self, key: u64, rect: Rect) {
        self.boxes.push(Anchored { key, rect });
    }

    /// Drops every attachment and keeps the buffer.
    pub(crate) fn clear(&mut self) {
        self.boxes.clear();
    }

    /// Returns the box `key` attached under, or `None` where nothing is attached under it.
    #[must_use]
    pub fn get(&self, key: u64) -> Option<Rect> {
        self.boxes
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.rect)
    }

    /// Returns every attached box, in attachment order.
    pub fn iter(&self) -> impl Iterator<Item = Anchored> + '_ {
        self.boxes.iter().copied()
    }

    /// Returns how many keys the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    /// Returns whether no key is attached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }
}

/// A handle to one container's keyed child boxes.
///
/// `Copy`, minted in the enclosing owner and disposed with it, like a
/// [`Probe`](super::Probe). One node declares itself the origin with
/// [`Element::anchors_origin`](crate::build::Element::anchors_origin); any node below it
/// attaches with [`Element::anchored`](crate::build::Element::anchored).
///
/// ```no_run
/// # use windows_ui::layout::{anchors, stack};
/// # use windows_ui::widget::caption;
/// # fn f(ui: &mut windows_ui::build::Ui<'_>) {
/// let rows = anchors();
/// stack(ui, |ui| {
///     caption(ui, "a row").anchored(rows, 7);
/// })
/// .anchors_origin(rows);
/// # }
/// ```
#[derive(Copy, Clone, Debug)]
pub struct Anchors(Cell<Table>);

/// Returns a fresh anchor set, reading an empty table until its origin has been solved.
#[must_use]
pub fn anchors() -> Anchors {
    Anchors(Cell::new(Table::default()))
}

/// Two sets are the same set where they are the same cell, whatever each last published.
impl PartialEq for Anchors {
    fn eq(&self, other: &Self) -> bool {
        self.0.id() == other.0.id()
    }
}

impl Anchors {
    /// Calls `f` with the published table, registering a dependency for the reading effect
    /// or memo.
    pub fn with<R>(self, f: impl FnOnce(&Table) -> R) -> R {
        self.0.with(f)
    }

    /// Returns the cell behind the set, which the host publishes into during the flush.
    pub(crate) const fn cell(self) -> Cell<Table> {
        self.0
    }
}
