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
//! precisely so it can be drawn from elsewhere — it holds nothing but the
//! composition crate's detached drawing handle, which is where that `Send`
//! (and its soundness argument) comes from.
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

use windows_canvas::GpuDevice;

use crate::backend::ControlId;
use crate::widgets::CompositionDrawSurface;

/// The backend's own Direct2D/Direct3D device, published when the compositor is
/// bootstrapped so a surface host can draw through it instead of standing up a
/// second one.
///
/// A `GpuDevice` clone is a COM `AddRef` of the *same* device, and the backend
/// builds its device with a multi-threaded Direct2D factory — the constructor
/// whose purpose is one device driven from several threads, with Direct2D
/// serializing them. A host that draws its surface on a worker thread therefore
/// wants this device, not one of its own: a second `D3D11CreateDevice` brings a
/// whole second display-driver worker pool and its heaps with it.
///
/// `generation` changes whenever the backend publishes a different device, so a
/// requester can tell a replacement from the device it was already using.
struct BackendDevice {
    device: GpuDevice,
    generation: u64,
}

// SAFETY: the device is built by `GpuDevice::new_multi_threaded`, i.e. with a
// multi-threaded Direct2D factory, which serializes access across threads —
// exactly the contract [`SurfaceDevice`] documents. Handing a clone (a COM
// `AddRef`, itself thread-safe) to another thread is the designed use.
unsafe impl Send for BackendDevice {}

static BACKEND_DEVICE: Mutex<Option<BackendDevice>> = Mutex::new(None);
static NEXT_DEVICE_GEN: Mutex<u64> = Mutex::new(0);

/// Publish the backend's device. Called once as the compositor is created; a
/// second call (a replacement device) supersedes the first under a new
/// generation.
pub(crate) fn publish_backend_device(device: &GpuDevice) {
    let generation = {
        let mut next = NEXT_DEVICE_GEN.lock().expect("backend device generation");
        *next += 1;
        *next
    };
    if let Ok(mut slot) = BACKEND_DEVICE.lock() {
        *slot = Some(BackendDevice {
            device: device.clone(),
            generation,
        });
    }
}

/// The backend's Direct2D/Direct3D device and its generation, or `None` before
/// the compositor exists.
///
/// Callable from any thread. The device it returns is `!Send` once unwrapped —
/// see [`SurfaceDevice`] for the contract a second thread drawing through it
/// takes on: the device tolerates concurrent use, and all draws through *one*
/// `CompositionGraphicsDevice` are serialized by whoever owns it.
pub fn backend_gpu_device() -> Option<(GpuDevice, u64)> {
    let slot = BACKEND_DEVICE.lock().ok()?;
    let d = slot.as_ref()?;
    Some((d.device.clone(), d.generation))
}

/// Identifies one hosted surface, so it can be released without naming the
/// visual the backend holds for it.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct SurfaceToken(u64);

/// The Direct2D device a hosted surface draws through.
///
/// The device is more than a resource here: it names the surface's
/// **draw-serialization domain**. A `CompositionGraphicsDevice` allows only one
/// outstanding `BeginDraw` across all of its surfaces — a second concurrent
/// `BeginDraw` *fails* (`0x80131509`), it does not block. So the backend keeps
/// one `CompositionSurfaceFactory` (one `CompositionGraphicsDevice`) per
/// *distinct* Direct2D device, keyed by the device's COM identity: surfaces
/// requested with the same device share a factory and its atlas, and surfaces
/// drawn by different threads — which by the contract below arrive with
/// different devices — can never collide in `BeginDraw`, with no lock and no
/// retry. `generation` is a caller-chosen device-loss stamp: when it changes
/// for the same device identity (a replacement reusing the allocation), the
/// factory is rebuilt.
///
/// # Safety contract
/// The requester draws through the resulting surface from its own thread while
/// the backend's thread keeps compositing, so the device must tolerate that
/// (e.g. `GpuDevice::new_multi_threaded`, or a single-threaded device used only
/// from the one thread that owns it). Additionally, all draws through **one**
/// device must be serialized by its owner (one owning thread, or an external
/// order) — the per-device `CompositionGraphicsDevice` turns that existing
/// Direct2D discipline into BeginDraw safety.
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
    /// Wrap a Direct2D device (an `ID2D1Device`) for the backend, tagged with a
    /// caller-chosen `generation` that changes whenever the device is replaced.
    /// See the type docs for the serialization-domain contract the device carries.
    pub fn new(device: &impl windows_core::Interface, generation: u64) -> windows_core::Result<Self> {
        Ok(Self {
            device: device.cast()?,
            generation,
        })
    }

    /// The device's COM identity — the canonical `IUnknown` pointer (`cast` in
    /// [`new`](Self::new) is a `QueryInterface` for `IUnknown`, which COM
    /// requires to be identity-stable). Two wrappers around the same device
    /// yield the same key; distinct devices never share one while both are
    /// alive. Pointer reuse after a device is destroyed is disambiguated by
    /// `generation`.
    pub(crate) fn identity(&self) -> usize {
        use windows_core::Interface;
        self.device.as_raw() as usize
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

    /// Resize the presented sprite to `dip` DIPs, in place. Queues a resize of the
    /// child visual the backend holds for this surface (applied on the backend's
    /// thread next drain) — the backing pixels are resized separately by the drawing
    /// side (`CompositionDrawSurface::resize`). A no-op if the surface was never
    /// serviced. This is the resize path that replaces dropping and re-requesting:
    /// the visual is never unparented, so the surface never blanks.
    pub fn resize_visual(&self, dip: (f32, f32)) {
        push(Op::Resize(self.token, dip));
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
    /// Resize a hosted surface's child visual to the given DIP size, in place.
    Resize(SurfaceToken, (f32, f32)),
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
/// presented at. To resize later, use [`PendingSurface::resize_visual`] together
/// with [`CompositionDrawSurface::resize`] on the drawing side — the surface
/// resizes in place, so there is no need to drop it and request another.
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
