//! Owner-scoped local geometry over the existing layout publication and signal graph.

use super::{Host, geometry, set_geometry};
use crate::layout::Probe;
use crate::role::Scope;
use crate::signal::{Effect, Memo};
use windows_numerics::Vector2;
use windows_scene::{GeomId, PathVerb};

/// Authors one retained path directly. Bounds, scratch storage and disposal are implicit.
/// Use `paths_with` when several independently painted paths share one local computation.
pub fn path_with(
    capacity: usize,
    mut fill: impl FnMut(&mut Vec<PathVerb>, Vector2, Scope) + 'static,
) -> super::El<super::Path> {
    paths_with(
        [capacity],
        move |[verbs], size, scope| fill(verbs, size, scope),
        |[id]| crate::widget::path(id),
    )
}

pub(super) type Draw = Box<dyn FnMut(Vector2, Scope)>;
pub(super) struct Row {
    pub scope: Scope,
    pub local: Option<(Vector2, Scope)>,
    pub effect: Option<Effect>,
    pub pivot: Option<Vector2>,
}

/// Builds related retained paths in the final local box of the returned view.
/// The draw callback may read signals and fill paths, but cannot change UI or signal state.
/// It runs after layout, only when size, scope or a tracked value changes.
pub fn paths_with<const N: usize, K>(
    capacities: [usize; N],
    mut fill: impl FnMut(&mut [Vec<PathVerb>; N], Vector2, Scope) + 'static,
    view: impl FnOnce([GeomId; N]) -> super::El<K>,
) -> super::El<K> {
    let ids = capacities.map(|_| geometry(&[]));
    let mut paths = capacities.map(Vec::with_capacity);
    let view = view(ids);
    super::arena::Build::with(|build| {
        let slot = &mut build.nodes[view.at as usize];
        assert!(
            slot.geometry_job.is_none(),
            "one geometry job per local box"
        );
        slot.geometry_job = Some(build.geometry_jobs.len() as u32);
        build.geometry_jobs.push(Some(Box::new(move |size, scope| {
            paths.iter_mut().for_each(Vec::clear);
            crate::signal::read_only(|| fill(&mut paths, size, scope));
            for (id, verbs) in ids.into_iter().zip(&paths) {
                set_geometry(id, verbs);
            }
        })));
    });
    view
}

pub(super) fn mount(node: windows_scene::NodeId, scope: Scope, mut draw: Draw) {
    let effect = Effect::geometry(move || {
        let local = Host::with(|host| host.geometry_jobs.get(node).and_then(|row| row.local));
        if let Some((size, scope)) = local {
            draw(size, scope);
        }
    });
    Host::with(|host| {
        if let Some(row) = host.geometry_jobs.get_mut(node) {
            row.effect = Some(effect);
        } else {
            host.geometry_jobs.place(
                node,
                Row {
                    scope,
                    local: None,
                    effect: Some(effect),
                    pivot: None,
                },
            );
        }
    });
}

pub(super) struct Lease<T>(pub windows_scene::Id<T>, pub u64);

impl<T> Drop for Lease<T> {
    fn drop(&mut self) {
        Host::try_with(|host| {
            if host.identity == self.1 {
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
    Effect::geometry(move || {
        let Some((size, scope)) = local.get() else {
            return;
        };
        paths.iter_mut().for_each(Vec::clear);
        crate::signal::read_only(|| fill(&mut paths, size, scope));
        for (id, verbs) in ids.into_iter().zip(&paths) {
            set_geometry(id, verbs);
        }
    });
    ids
}
