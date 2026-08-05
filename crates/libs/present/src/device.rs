//! The present thread's device, and the drawing bracket every region on it shares.
//!
//! One device per present thread, thread-affine for its whole life, owning the `Gpu` whose
//! single Direct2D context every region retargets. A pass opens one `BeginDraw`/`EndDraw`
//! pair covering every region and every slot it draws: the pair carries a fixed cost — a
//! DXGI `ReclaimResources`, an `OfferResources`, and a delayed Direct3D
//! device-context-state swap — that is independent of what was drawn, so it is paid once
//! per pass rather than once per region.

use super::*;

/// Owns a Direct3D 11 device, a Direct2D device and context over it, and the presentation
/// factory bound to the same Direct3D device.
///
/// Thread-affine. Everything created from it — groups, regions, buffers, and every brush
/// and geometry a [`Frame`](crate::Frame) builds — belongs to the thread that built this.
pub struct PresentationDevice {
    gpu: Gpu,
    /// This crate's own projection of the same Direct3D device `gpu` holds, cast once
    /// rather than per buffer allocation. COM identity belongs to the object rather than to
    /// the interface, so it is the same device by construction.
    d3d: ID3D11Device,
    factory: IPresentationFactory,
    /// Whether the system supports buffers eligible for a display plane. A strictly higher
    /// bar than plain presentation support, and the only reason a region's displayable
    /// request is refused before the driver sees it.
    flip: bool,
}

impl PresentationDevice {
    /// Builds the Direct3D device, the Direct2D device over it, and the presentation
    /// factory bound to both.
    ///
    /// # Errors
    ///
    /// Fails when the devices or the factory cannot be created, and when the system
    /// reports no presentation support. `IPresentationManager` requires Windows 11 and
    /// WDDM 2.0, and Windows 11 mandates WDDM 2.0, so on this stack's floor presentation
    /// support is unconditional and a refusal means a machine this application cannot
    /// render on at all. There is no fallback presentation model.
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

    /// Returns the device every region on this thread draws with.
    ///
    /// Build every brush, geometry, stroke style and cached layer from it: a resource made
    /// from another `Gpu` does not bind here, and the failure is content that never appears
    /// rather than an error.
    #[must_use]
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Returns whether this system can allocate buffers eligible for a display plane.
    ///
    /// A region whose displayable request is refused still presents, and the difference is
    /// visible only in the present statistics.
    #[must_use]
    pub fn can_flip(&self) -> bool {
        self.flip
    }

    /// Opens the single drawing bracket a pass draws inside.
    ///
    /// # Errors
    ///
    /// Fails when a bracket is already open on this device.
    pub fn pass(&self) -> Result<Pass<'_>> {
        self.gpu.pass()
    }

    /// Creates a presentation manager and the group of regions that present through it.
    ///
    /// `statistics` enables the manager's per-present statistics queue. It also forces the
    /// VSync interrupt on for every present in the group: a statistic describes a present
    /// the CPU was woken for, so a group that defers the interrupt reports a queue that has
    /// already moved on.
    ///
    /// # Errors
    ///
    /// Fails when the factory cannot create the manager or its events.
    pub fn create_group(&self, statistics: bool) -> Result<PresentationGroup> {
        PresentationGroup::new(&self.factory, statistics)
    }

    pub(crate) fn d3d(&self) -> &ID3D11Device {
        &self.d3d
    }
}

/// Witnesses that the pass's Direct2D bracket closed.
///
/// [`PresentationRegion::submit`](crate::PresentationRegion::submit) requires one and
/// [`Flushed::end`] is its only constructor, so a buffer cannot be bound for presentation
/// while its pixels are unflushed and every region of every slot has necessarily drawn
/// before any of them binds.
///
/// A bind interleaved with the drawing ends the bracket early and costs every later frame a
/// bracket of its own: 79.82 µs a call against 10.97.
pub struct Flushed(());

impl Flushed {
    /// Closes `pass` and returns the witness.
    ///
    /// # Errors
    ///
    /// Returns the `PassError` carrying the tag of the region whose draw latched the
    /// error. Direct2D defers: a failed call latches an error on the context and silently
    /// discards every later draw in the same bracket. The bracket spans every region of
    /// every slot in the pass, so the tag is what separates the region that failed from the
    /// ones its failure discarded.
    pub fn end(pass: Pass<'_>) -> core::result::Result<Self, PassError> {
        pass.end().map(|()| Self(()))
    }
}
