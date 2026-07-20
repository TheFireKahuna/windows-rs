//! Servicing for [`crate::surface`] requests on the DirectComposition backend.
//!
//! The backend owns the factory and every hosted sprite's
//! [`CompositionChildVisual`] — the visual tree belongs to the thread that owns
//! the compositor — and hands requesters only the `Send` drawing side.

use crate::backend::ControlId;
use crate::surface::{drain, Op, Request, SurfaceDevice, SurfaceToken};
use crate::widgets::{CompositionChildVisual, CompositionSurfaceFactory};

/// Surfaces the backend is hosting, and the factories it mints them with.
#[derive(Default)]
pub(crate) struct SurfaceHost {
    /// Live surfaces by token. The [`CompositionChildVisual`] is the parenting —
    /// dropping it detaches the sprite — and `ControlId` is its host, so the
    /// surfaces of a destroyed control go with it.
    live: Vec<(SurfaceToken, ControlId, CompositionChildVisual)>,
    /// One factory per **distinct Direct2D device** — keyed by the device's COM
    /// identity, with the caller's device-loss generation alongside. A
    /// `CompositionGraphicsDevice` admits only one outstanding `BeginDraw`
    /// across all its surfaces, so the factory boundary must follow the
    /// draw-serialization boundary, and that is the device (see
    /// [`SurfaceDevice`]): the UI-thread painters and the viz worker each bring
    /// their own device and so can never fail each other's `BeginDraw`. Keying
    /// by the caller's bare generation number instead conflated the two — both
    /// sides count from 1 — and parked painter surfaces on the worker's device,
    /// which is exactly the collision this map exists to make unrepresentable.
    /// An entry whose device was lost is replaced in place when its allocation
    /// is reused (same identity, new generation) and is otherwise a few idle
    /// COM handles; entries are never scanned per frame, so the map costs
    /// nothing at rest.
    factories: Vec<(usize, u64, CompositionSurfaceFactory)>,
}

impl SurfaceHost {
    /// Drop every surface hosted under `id`. Called when the control is
    /// destroyed: its visual is going away, so the sprites parented under it
    /// must go first.
    pub(crate) fn forget(&mut self, id: ControlId) {
        self.live.retain(|(_, host, _)| *host != id);
    }

    fn release(&mut self, token: SurfaceToken) {
        self.live.retain(|(t, _, _)| *t != token);
    }

    /// Resize the child visual hosting `token` to `dip` DIPs, in place. A no-op if
    /// the surface is gone (released between the resize request and this drain) — the
    /// drawing side's backing resize is likewise harmless in that case.
    fn resize(&mut self, token: SurfaceToken, dip: (f32, f32)) {
        if let Some((_, _, visual)) = self.live.iter().find(|(t, _, _)| *t == token) {
            visual.set_dip_size(dip);
        }
    }

    fn factory_for(
        &mut self,
        compositor: &crate::system_bindings::Compositor,
        device: &SurfaceDevice,
    ) -> Option<&CompositionSurfaceFactory> {
        let key = device.identity();
        let found = self.factories.iter().position(|(k, _, _)| *k == key);
        let hit = found.is_some_and(|i| self.factories[i].1 == device.generation);
        let at = if hit {
            found.expect("hit implies found")
        } else {
            let built =
                CompositionSurfaceFactory::from_compositor(compositor, &device.device).ok()?;
            match found {
                // Same allocation, new generation: the device was replaced —
                // rebuild this entry in place.
                Some(i) => {
                    self.factories[i] = (key, device.generation, built);
                    i
                }
                None => {
                    self.factories.push((key, device.generation, built));
                    self.factories.len() - 1
                }
            }
        };
        Some(&self.factories[at].2)
    }
}

impl super::DCompBackend {
    /// Drain the surface queue: create what was asked for, release what was
    /// dropped.
    ///
    /// Runs after the command buffer is replayed, so a control requested in the
    /// same frame it mounts already exists in the arena by the time its surface
    /// is hosted.
    pub(crate) fn service_surface_ops(&mut self) {
        for op in drain() {
            match op {
                Op::Release(token) => self.surfaces.release(token),
                Op::Create(req) => self.host_surface(*req),
                Op::Resize(token, dip) => self.surfaces.resize(token, dip),
            }
        }
    }

    fn host_surface(&mut self, req: Request) {
        // The host must still exist: a control can be destroyed between the
        // request and this drain. Its `PendingSurface` simply never fills, which
        // the requester already handles — it is the same state as "not yet
        // serviced".
        let Some(container) = self.arena.get(req.host).map(|n| n.container.clone()) else {
            return;
        };
        let compositor = self.comp.compositor().clone();
        let Some(factory) = self.surfaces.factory_for(&compositor, &req.device) else {
            return;
        };
        let Ok((visual, draw)) = factory.create_under(&container, req.pixel, req.dip, req.opaque)
        else {
            return;
        };
        self.surfaces.live.push((req.token, req.host, visual));
        if let Ok(mut slot) = req.slot.lock() {
            *slot = Some(draw);
        }
        (req.ready)();
    }
}
