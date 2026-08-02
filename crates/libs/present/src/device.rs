//! The present thread's device, and the bracket every region on it draws inside.
//!
//! One device per present thread, thread-affine for its whole life, owning the `Gpu`
//! whose single Direct2D context every region retargets. There is no second context and
//! no second bracket: Direct2D charges a fixed cost per `BeginDraw`/`EndDraw` pair that
//! has nothing to do with what was drawn — a DXGI `ReclaimResources`, an
//! `OfferResources`, and a delayed Direct3D device-context-state swap — so a bracket per
//! region scales with region count rather than with drawing, measured at about a fifth of
//! the process.

use super::*;

/// A Direct3D 11 device, a Direct2D device and context over it, and the presentation
/// factory bound to the same Direct3D device.
///
/// Thread-affine. Everything created from it — groups, regions, buffers, and every brush
/// and geometry a [`Frame`](crate::Frame) builds — belongs to the thread that built this.
pub struct PresentationDevice {
    gpu: Gpu,
    /// This crate's own projection of the same Direct3D device `gpu` holds, cast once
    /// rather than per buffer allocation. COM identity belongs to the object, not to the
    /// interface, so it is the same device by construction.
    d3d: ID3D11Device,
    factory: IPresentationFactory,
    /// Whether the system supports buffers eligible for a display plane. A strictly
    /// higher bar than plain presentation support, and the only reason a region's
    /// displayable request is refused before the driver ever sees it.
    flip: bool,
}

impl PresentationDevice {
    /// Builds the device, or fails.
    ///
    /// There is no `Option` and no retained fallback. `IPresentationManager` requires
    /// Windows 11 and WDDM 2.0, Windows 11 mandates WDDM 2.0, and this stack's floor is
    /// Windows 11 — so on that floor presentation support is unconditional and a system
    /// reporting otherwise is a system this application cannot render on at all. The
    /// prior stack returned `Ok(None)` here and carried two more presentation models to
    /// fall back to; both are deleted, so the honest report is an error.
    pub fn new() -> Result<Self> {
        let gpu = Gpu::for_presentation()?;
        let d3d: ID3D11Device = gpu.d3d().cast()?;
        // SAFETY: `d3d` is a live interface pointer owned by `gpu`, and the out-parameter
        // is a stack local that outlives the call.
        let factory: IPresentationFactory = unsafe {
            let mut out = core::ptr::null_mut();
            CreatePresentationFactory(d3d.as_raw(), &IPresentationFactory::IID, &mut out).ok()?;
            IPresentationFactory::from_raw(out)
        };
        // SAFETY: `factory` is live for the rest of this function.
        if unsafe { factory.IsPresentationSupported() } == 0 {
            return Err(windows_core::Error::from_hresult(E_FAIL));
        }
        // SAFETY: as above.
        let flip = unsafe { factory.IsPresentationSupportedWithIndependentFlip() } != 0;
        Ok(Self {
            gpu,
            d3d,
            factory,
            flip,
        })
    }

    /// The device every region on this thread draws with.
    ///
    /// Build every brush, geometry, stroke style and cached layer from it: a resource
    /// made from another `Gpu` does not bind here, and the failure is content that never
    /// appears rather than an error.
    #[must_use]
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Whether this system can allocate buffers eligible for a display plane.
    ///
    /// Reported rather than assumed: a region that asked for displayable buffers and did
    /// not get them still presents, and the difference is visible only in the statistics.
    #[must_use]
    pub fn can_flip(&self) -> bool {
        self.flip
    }

    /// Opens the one drawing bracket for this pass.
    ///
    /// `Err` if one is already open, which is the device's own flag rather than a second
    /// piece of state to keep in step.
    pub fn pass(&self) -> Result<Pass<'_>> {
        self.gpu.pass()
    }

    /// A presentation manager, and the regions that will present through it.
    ///
    /// `statistics` belongs to the group because the manager is what reports them, and it
    /// forces the VSync interrupt on for every present in the group — a statistic
    /// describes a present the CPU was woken for, so a group that defers the interrupt
    /// reports a queue that has already moved on.
    pub fn create_group(&self, statistics: bool) -> Result<PresentationGroup> {
        PresentationGroup::new(&self.factory, statistics)
    }

    pub(crate) fn d3d(&self) -> &ID3D11Device {
        &self.d3d
    }
}

/// Proof that the pass's Direct2D bracket closed.
///
/// [`PresentationRegion::submit`](crate::PresentationRegion::submit) needs one, and this
/// is its only constructor — so a buffer cannot be bound for presentation while its
/// pixels are still unflushed, and "nothing binds until everything has drawn" is a thing
/// the borrow checker knows rather than a rule in a comment.
///
/// That ordering is the entire saving in the batched pass: the bracket's cost is fixed
/// and paid once per pair, so a bind interleaved with the drawing ends the batch early
/// and every later frame pays for a bracket of its own — **79.82 µs a call against
/// 10.97**.
pub struct Flushed(());

impl Flushed {
    /// Closes `pass` and returns the proof, or the tag that names the region whose draw
    /// latched the error.
    ///
    /// Direct2D defers: a failed call latches an error on the context that silently
    /// discards every later draw in the same bracket, and since the bracket spans every
    /// region of every slot in the pass, a draw that vanished is as likely to have been
    /// killed by an earlier region's as to be wrong itself. The tag is what tells those
    /// apart, which is why the pass tags per retarget.
    pub fn end(pass: Pass<'_>) -> core::result::Result<Self, PassError> {
        pass.end().map(|()| Self(()))
    }
}
