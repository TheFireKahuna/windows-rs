//! Owner-scoped local geometry over the existing layout publication and signal graph.

use super::{Host, geometry, set_geometry};
use crate::layout::Probe;
use crate::role::Scope;
use crate::signal::{Effect, Memo};
use windows_numerics::Vector2;
use windows_scene::{GeomId, PathVerb};

/// Authors one retained path directly. Bounds, scratch storage and disposal are implicit.
/// Use `local_geometries` when several independently painted paths share one computation.
pub fn path_with(
    capacity: usize,
    fill: impl FnMut(&mut Vec<PathVerb>, Vector2, Scope) + 'static,
) -> super::El<super::Path> {
    let bounds = crate::layout::probe();
    crate::widget::path(local_geometry(bounds, capacity, fill)).probed(bounds)
}

pub(super) struct Lease<T>(pub windows_scene::Id<T>, pub std::rc::Rc<()>);

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        Host::try_with(|host| {
            if std::rc::Rc::ptr_eq(&host.identity, &self.1) {
                host.model().release(self.0);
            }
        });
    }
}

/// Retains a local-DIP path and reusable verb buffer until the current owner is disposed.
/// The callback runs on local size/scope or tracked input changes, never on position alone.
/// It may only emit geometry; it must not change layout, structure or application state.
/// `capacity` bounds the intended scratch usage; input-dependent growth is event-rate work.
#[must_use]
pub fn local_geometry(
    bounds: Probe,
    capacity: usize,
    mut fill: impl FnMut(&mut Vec<PathVerb>, Vector2, Scope) + 'static,
) -> GeomId {
    local_geometries(bounds, [capacity], move |[verbs], size, scope| {
        fill(verbs, size, scope);
    })[0]
}

/// Emits related paths in one tracked computation. Each path has its own retained buffer.
#[must_use]
pub fn local_geometries<const N: usize>(
    bounds: Probe,
    capacities: [usize; N],
    mut fill: impl FnMut(&mut [Vec<PathVerb>; N], Vector2, Scope) + 'static,
) -> [GeomId; N] {
    let ids = capacities.map(|_| geometry(&[]));
    let local = Memo::new(move || {
        let placed = bounds.get();
        placed.scope.map(|scope| (placed.size, scope))
    });
    let mut paths = capacities.map(Vec::with_capacity);
    Effect::new(move || {
        let Some((size, scope)) = local.get() else {
            return;
        };
        paths.iter_mut().for_each(Vec::clear);
        fill(&mut paths, size, scope);
        for (id, verbs) in ids.into_iter().zip(&paths) {
            set_geometry(id, verbs);
        }
    });
    ids
}
