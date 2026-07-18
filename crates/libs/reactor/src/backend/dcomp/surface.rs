//! Servicing for [`crate::surface`] requests on the DirectComposition backend.
//!
//! The backend owns the factory and every hosted sprite's
//! [`CompositionChildVisual`] — the visual tree belongs to the thread that owns
//! the compositor — and hands requesters only the `Send` drawing side.

use crate::backend::ControlId;
use crate::surface::{drain, Op, Request, SurfaceDevice, SurfaceToken};
use crate::widgets::{CompositionChildVisual, CompositionSurfaceFactory};

/// Surfaces the backend is hosting, and the factory it mints them with.
#[derive(Default)]
pub(crate) struct SurfaceHost {
    /// Live surfaces by token. The [`CompositionChildVisual`] is the parenting —
    /// dropping it detaches the sprite — and `ControlId` is its host, so the
    /// surfaces of a destroyed control go with it.
    live: Vec<(SurfaceToken, ControlId, CompositionChildVisual)>,
    /// One factory per device; rebuilt when the requester's device generation
    /// changes.
    factory: Option<(u64, CompositionSurfaceFactory)>,
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

    fn factory_for(
        &mut self,
        compositor: &crate::system_bindings::Compositor,
        device: &SurfaceDevice,
    ) -> Option<&CompositionSurfaceFactory> {
        if self.factory.as_ref().map(|(g, _)| *g) != Some(device.generation) {
            let built =
                CompositionSurfaceFactory::from_compositor(compositor, &device.device).ok()?;
            self.factory = Some((device.generation, built));
        }
        self.factory.as_ref().map(|(_, f)| f)
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
