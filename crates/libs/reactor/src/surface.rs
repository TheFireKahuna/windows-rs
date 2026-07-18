//! Backend-hosted composition surfaces, addressed by [`ControlId`].
//!
//! The request API lives here rather than beside the DirectComposition backend
//! so a crate can offer surface hosting without choosing the app's backend.
//! Only the servicing is backend-specific; with no backend to drain the queue a
//! request simply never completes, which callers already treat as "not yet".
//!
//! A viz host wants Direct2D content parented under one of its controls. The
//! visual tree that content hangs from belongs to the thread that owns the
//! compositor, so the host cannot build it: creating, parenting and detaching a
//! child visual is all COM work on that thread. What the host actually needs
//! back is only the *drawing* side, and [`CompositionDrawSurface`] is `Send`
//! precisely so it can be drawn from elsewhere.
//!
//! So the two halves are split along that line. A host [`request_surface`]s one
//! by id and immediately gets a [`PendingSurface`]; the backend services the
//! request on its own thread, keeps the [`CompositionChildVisual`], and fills
//! the pending handle with the drawing side. Nothing blocks, and no thread-affine
//! object ever reaches the requester — which is what lets the requester live on
//! another thread.
//!
//! Requests are queued rather than executed inline because the requester is
//! typically mid-render, where the backend is already borrowed. They are drained
//! once per frame, after the reconcile's command buffer is replayed and before
//! layout reads the arena.

use std::sync::{Arc, Mutex};

use crate::backend::ControlId;
use crate::widgets::CompositionDrawSurface;

/// Identifies one hosted surface, so it can be released without naming the
/// visual the backend holds for it.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct SurfaceToken(u64);

/// The Direct2D device a hosted surface draws through.
///
/// The backend builds one `CompositionSurfaceFactory` per device and reuses it
/// for every surface, so `generation` identifies the caller's device: when it
/// changes — a device loss, a replacement — the factory is rebuilt.
///
/// # Safety contract
/// The device must be multi-threaded (`GpuDevice::new_multi_threaded`), because
/// the requester goes on to draw through the resulting surface from its own
/// thread while the backend's thread keeps compositing.
pub struct SurfaceDevice {
    pub(crate) device: windows_core::IUnknown,
    pub(crate) generation: u64,
}

// SAFETY: the wrapper exists only to carry the device to the thread that owns
// the compositor, where it is used once to create a `CompositionGraphicsDevice`.
// Direct2D serializes calls on a multi-threaded device internally, which the
// caller asserts by construction. `Sync` is deliberately not claimed — nothing
// hands out a shared reference to the device.
unsafe impl Send for SurfaceDevice {}

impl SurfaceDevice {
    /// Wrap a multi-threaded Direct2D device (an `ID2D1Device`) for the backend,
    /// tagged with a caller-chosen `generation` that changes whenever the device
    /// is replaced.
    pub fn new(device: &impl windows_core::Interface, generation: u64) -> windows_core::Result<Self> {
        Ok(Self {
            device: device.cast()?,
            generation,
        })
    }
}

/// A surface the backend has been asked to host. [`take`](Self::take) yields the
/// drawing side once the request has been serviced; dropping this releases the
/// surface and unparents its visual.
pub struct PendingSurface {
    pub(crate) slot: Arc<Mutex<Option<CompositionDrawSurface>>>,
    token: SurfaceToken,
}

impl PendingSurface {
    /// Take the drawing side, or `None` while the request is still outstanding.
    /// Yields it exactly once.
    pub fn take(&self) -> Option<CompositionDrawSurface> {
        self.slot.lock().ok()?.take()
    }
}

impl Drop for PendingSurface {
    fn drop(&mut self) {
        push(Op::Release(self.token));
    }
}

pub(crate) struct Request {
    pub(crate) token: SurfaceToken,
    pub(crate) host: ControlId,
    pub(crate) pixel: (i32, i32),
    pub(crate) dip: (f32, f32),
    pub(crate) opaque: bool,
    pub(crate) device: SurfaceDevice,
    pub(crate) slot: Arc<Mutex<Option<CompositionDrawSurface>>>,
    /// Called once the slot is filled, so the requester can come back for it —
    /// typically by bumping a state that re-runs the effect that asked.
    pub(crate) ready: Box<dyn Fn() + Send>,
}

pub(crate) enum Op {
    Create(Box<Request>),
    Release(SurfaceToken),
}

/// Pending work for the backend. A plain `Mutex` rather than a thread-local:
/// requests originate wherever the requester runs, which is not necessarily the
/// thread that services them.
static OPS: Mutex<Vec<Op>> = Mutex::new(Vec::new());
static NEXT_TOKEN: Mutex<u64> = Mutex::new(0);

fn push(op: Op) {
    if let Ok(mut ops) = OPS.lock() {
        ops.push(op);
    }
}

/// Ask the backend to host a composition surface under control `host`.
///
/// Returns immediately. The surface is created when the backend next services
/// its queue (once per frame); `ready` fires at that point and
/// [`PendingSurface::take`] then yields the drawing side.
///
/// `pixel` is the backing size in physical pixels and `dip` the size it is
/// presented at. The pixel size is fixed for the surface's lifetime — to resize,
/// drop the [`PendingSurface`] and request another.
pub fn request_surface(
    host: ControlId,
    device: SurfaceDevice,
    pixel: (i32, i32),
    dip: (f32, f32),
    opaque: bool,
    ready: impl Fn() + Send + 'static,
) -> PendingSurface {
    let token = {
        let mut next = NEXT_TOKEN.lock().expect("surface token counter");
        *next += 1;
        SurfaceToken(*next)
    };
    let slot = Arc::new(Mutex::new(None));
    push(Op::Create(Box::new(Request {
        token,
        host,
        pixel,
        dip,
        opaque,
        device,
        slot: Arc::clone(&slot),
        ready: Box::new(ready),
    })));
    PendingSurface { slot, token }
}


/// Take the outstanding operations for a backend to service.
pub(crate) fn drain() -> Vec<Op> {
    match OPS.lock() {
        Ok(mut q) if !q.is_empty() => std::mem::take(&mut *q),
        _ => Vec::new(),
    }
}
