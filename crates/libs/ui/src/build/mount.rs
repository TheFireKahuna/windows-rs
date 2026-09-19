//! Retained subtree lifetime and scene resource publication.
//!
//! A `Mount` owns one declaration transaction's root list; dropping it retires those roots and
//! everything reachable from them, callbacks last.

use super::host::Host;
use super::theme::{PaintMask, PaintSource, Part};
use super::tree;
use crate::layout::{Len, THUMB_W};
use crate::role::{Role, Scope, Text};
use windows_scene::{Exit, GeomId, GroupId, NodeId, PathVerb, RampId, Spread, SpriteId};

pub fn root_scope() -> Scope {
    Host::with(|host| host.root_scope())
}

pub fn set_geometry(id: GeomId, verbs: &[PathVerb]) {
    Host::with(|h| h.set_geometry(id, verbs));
}

/// One stop of a ramp, stated in roles so a theme change re-resolves it.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Stop {
    /// Where the stop sits along the ramp, `0..=1`.
    pub at: f32,
    /// The chromatic role this stop paints.
    pub role: Role,
    /// How much of that role, as an alpha in `0..=1`.
    pub strength: f32,
}

pub fn set_ramp(id: RampId, stops: &[Stop], spread: Spread) {
    Host::with(|h| h.set_ramp(id, stops, spread));
}

/// One declaration transaction's roots, retired together.
///
/// Logical creation membership rather than layout ancestry: a keyed result may own several
/// siblings and an overlay a detached root, so a mount names node ids and holds no second
/// identity table.
#[must_use = "dropping a mount unmounts its subtree immediately"]
pub(crate) struct Mount {
    roots: Vec<NodeId>,
    exit: Exit,
    /// The window root survives its content: ending a transaction retires what it built and
    /// leaves the root for the next one.
    keep_root: bool,
}

impl Mount {
    pub(crate) fn new(roots: Vec<NodeId>) -> Self {
        Self { roots, exit: Exit::None, keep_root: false }
    }

    /// One transaction over roots that outlive it, whose children are what retires.
    pub(crate) fn rooted(roots: Vec<NodeId>) -> Self {
        Self { roots, exit: Exit::None, keep_root: true }
    }

    pub(crate) fn set_exit(&mut self, exit: Exit) {
        self.exit = exit;
    }

    /// Returns the node this subtree is rooted at.
    pub fn node(&self) -> NodeId {
        self.roots.first().copied().unwrap_or(NodeId::NONE)
    }

    /// Returns the last root, which is what the next sibling is ordered against.
    pub(crate) fn last(&self) -> Option<NodeId> {
        self.roots.last().copied()
    }

    /// Places these roots under `parent` above `after`, one at a time, chaining so each is
    /// ordered against the one before it.
    ///
    /// A visual collection offers insert-at-bottom, insert-above and remove and no
    /// insert-at-index, so a keyed reorder is a sequence of these and nothing else.
    pub(crate) fn place(&self, host: &mut Host, parent: NodeId, after: Option<NodeId>) {
        let mut after = after;
        for &root in &self.roots {
            host.place(root, GroupId(parent), after);
            after = Some(root);
        }
    }

    /// Retires every root and everything reachable from them.
    ///
    /// One `Op::Drop` per root: a subtree removal is one op and a partial destroy is not
    /// expressible, so the scene releases the descendants' resources on its own side.
    pub(crate) fn retire(&mut self, host: &mut Host) {
        if self.keep_root {
            let children: Vec<NodeId> = self
                .roots
                .iter()
                .flat_map(|&root| host.tree.children(root).collect::<Vec<_>>())
                .collect();
            for &child in &children {
                host.destroy(child, self.exit);
            }
            host.retire_tree(&children);
            return;
        }
        for &root in &self.roots {
            host.destroy(root, self.exit);
        }
        host.retire_tree(&self.roots);
        host.give_roots(core::mem::take(&mut self.roots));
    }
}

impl Drop for Mount {
    fn drop(&mut self) {
        // Non-panicking: a drop during teardown can run after the host has gone, and a panic
        // in a drop takes the process with it.
        Host::try_with(|h| self.retire(h));
    }
}

/// Mints a scroll container's rail and thumb under its viewport.
///
/// The scrollbar lives in the viewport rather than in the content, so it does not scroll with
/// what it reports on, and above the content, because child order is paint order and the order
/// the hit array is scanned in. Below it, the bar paints under whatever the list draws and a
/// grab resolves to the row behind it.
///
/// The rail is static geometry and carries the hit target; the thumb is moved by the
/// compositor and carries none. A hit entry on the thumb would name a rect the solve fixed and
/// the tracker then moved away from.
pub(crate) fn mount_scroll_chrome(host: &mut Host, viewport: NodeId) -> (NodeId, SpriteId) {
    let rail = host.group(GroupId(viewport), None);
    host.tree.author(rail.0, |l| *l = crate::layout::RAIL);
    let thumb = host.visual(rail, None);
    // An ordinary part, so the one resolver that repaints every other sprite on a theme change
    // repaints this one too. `Ink` and not a derived part: the thumb hangs on no surface, and
    // the scroll row is what releases it.
    host.declare_part(
        thumb.0,
        Part::Ink,
        PaintSource::Role(Role::Text(Text::Secondary)),
        PaintMask::Box { radius: Len::dip(THUMB_W * 0.5) },
        THUMB_ALPHA,
    );
    // Hidden from the mount rather than shown and faded out: a surface whose content fits
    // never overflows, and a thumb visible for one frame to say so is a flash on every screen
    // that opens.
    host.tree.set_flag(thumb.0, tree::HIDDEN, true);
    (rail.0, thumb)
}

/// How much of its role a resting thumb paints.
pub(crate) const THUMB_ALPHA: f32 = 0.55;
