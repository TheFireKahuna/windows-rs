//! Owner-scoped local geometry over the existing layout publication and signal graph.

use super::Host;
use crate::role::Scope;
use crate::signal::Effect;
use windows_numerics::Vector2;

pub(super) type Draw = Box<dyn FnMut(Vector2, Scope)>;
pub(super) struct Row {
    pub local: Option<(Vector2, Scope)>,
    pub effect: Option<Effect>,
    pub pivot: Option<Vector2>,
}

pub(super) fn install(host: &mut Host, node: windows_scene::NodeId, mut draw: Draw) {
    let runtime = host.identity;
    let effect = Effect::geometry(move || {
        let local = Host::with(|host| {
            (host.identity == runtime)
                .then(|| host.geometry_jobs.get(node).and_then(|row| row.local))
                .flatten()
        });
        if let Some((size, scope)) = local {
            draw(size, scope);
        }
    });
    host.own_binding(node, None, effect);
    if let Some(row) = host.geometry_jobs.get_mut(node) {
        if let Some(previous) = row.effect.replace(effect).and_then(Effect::retire) {
            host.retired.push(super::binding::Retired::Effect(previous));
        }
    } else {
        host.geometry_jobs.place(
            node,
            Row {
                local: None,
                effect: Some(effect),
                pivot: None,
            },
        );
    }
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

pub(super) struct RampLease(pub Lease<windows_scene::Ramp>);
impl Drop for RampLease {
    fn drop(&mut self) {
        Host::try_with(|host| {
            if host.identity == self.0.1
                && let Some((mut stops, _)) = host.ramps.take(self.0.0)
            {
                stops.clear();
                host.ramp_pool.push(stops);
            }
        });
    }
}
