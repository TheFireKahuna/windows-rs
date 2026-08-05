//! One presented rectangle: a presentation surface, the buffers it rotates through, and
//! the composition surface handle that binds it into a visual tree.
//!
//! A region draws and binds; its group presents. Every region binds its slot-*k* buffer, and
//! one present then shows them together.

use super::*;
use core::cell::Cell;

/// Carries a region's box: what layout solved, and the display it was solved for.
///
/// One value rather than a size and a DPI that can be set apart: they change together, since
/// a move to another display changes both, and a buffer allocated for one scale and drawn at
/// another renders soft with nothing to report it. Every field of
/// [`FrameCtx`](crate::FrameCtx) is derived from this, so the numbers a renderer reads cannot
/// disagree with each other.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Extent {
    /// Width in DIPs.
    pub w: f32,
    /// Height in DIPs.
    pub h: f32,
    /// Dots per inch of the display the box was solved for.
    pub dpi: f32,
}

impl Extent {
    /// Creates a box `w` by `h` DIPs, solved for a display at `dpi`.
    #[must_use]
    pub fn new(w: f32, h: f32, dpi: f32) -> Self {
        Self { w, h, dpi }
    }

    /// Returns the DIP-to-pixel factor.
    #[must_use]
    pub fn scale(self) -> f32 {
        self.dpi / 96.0
    }

    /// Returns the buffer allocation in pixels. Never zero in either axis: a zero-sized
    /// texture is refused, and a region is legitimately laid out at zero before its first
    /// solve.
    #[must_use]
    pub fn px(self) -> (u32, u32) {
        let s = self.scale();
        let to_px = |dip: f32| (dip * s).round().max(1.0) as u32;
        (to_px(self.w), to_px(self.h))
    }
}

/// Identifies a region, with a number the application chooses.
///
/// One number serves three consumers: the region's key in the present thread's registry, the
/// Direct2D tag that names the region whose draw latched an error, and the presentation
/// surface's tag that attributes a statistics record. Pass the id the caller already holds
/// for the sink this region paints, so there is no second identity space to keep in step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionKey(pub u64);

/// States what a region is, at the point it is mounted.
#[derive(Copy, Clone, Debug)]
pub struct RegionSpec {
    /// The region's identity, in the three places it is used.
    pub key: RegionKey,
    /// The present queue this region asks for.
    pub queue: Queue,
    /// The solved box the buffers are allocated for.
    pub extent: Extent,
}

/// An owned composition surface handle, closed with the region that made it.
struct SurfaceHandle(HANDLE);

impl Drop for SurfaceHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `SurfaceHandle` is the sole owner of the handle it wraps and is
            // constructed only from a freshly minted one, so this closes it exactly once.
            unsafe { _ = CloseHandle(self.0) };
        }
    }
}

/// Holds one rotating buffer: the texture, its registration with the manager, the Direct2D
/// view of it, and the event that says the compositor is finished with it.
struct Buffer {
    /// Keeps the allocation alive for as long as the region can present it. Nothing reads it
    /// after registration except `target`.
    _texture: ID3D11Texture2D,
    buffer: IPresentationBuffer,
    target: Target,
    available: HANDLE,
}

/// How long [`PresentationRegion::acquire`] waits for a buffer before giving up on the
/// frame.
///
/// Not `INFINITE`: this runs on a thread that must stay responsive to shutdown, and a
/// wait that can never return would wedge it. A pass that reaches this has a stalled
/// present queue, which is a skipped frame rather than a fault.
const ACQUIRE_TIMEOUT_MS: u32 = 1000;

/// Draws into one presented rectangle and binds the buffer its group will show.
///
/// Thread-affine, and tied for its whole life to the [`PresentationDevice`] and
/// [`PresentationGroup`] it was built from.
pub struct PresentationRegion {
    group: PresentationGroup,
    gpu: Gpu,
    d3d: ID3D11Device,
    surface: IPresentationSurface,
    handle: SurfaceHandle,
    buffers: Vec<Buffer>,
    /// Buffers handed out, ever, and buffers bound, ever. The rotation is arithmetic:
    /// `acquired % n` is the buffer this frame gets and `submitted % n` is the one the
    /// next bind names, so several frames may be in flight inside one pass and they bind
    /// in the order they were drawn — with no queue, no allocation and nothing to keep in
    /// step with the pool.
    acquired: Cell<u64>,
    submitted: Cell<u64>,
    extent: Extent,
    opacity: Opacity,
    displayable: bool,
    pool: u32,
}

impl PresentationRegion {
    /// Creates the region, its surface handle and its buffer pool.
    ///
    /// `opacity` is [`Frame::opaque`](crate::Frame::opaque)'s answer and decides three
    /// things together: the surface's alpha mode, the Direct2D target's alpha mode, and
    /// whether the buffers are requested displayable. One value sets all three, so the
    /// contradictory combination is unrepresentable — a buffer with a meaningful alpha
    /// channel cannot be handed to a hardware plane, so a displayable allocation under
    /// translucent content pays the constraint and returns nothing.
    pub(crate) fn new(
        device: &PresentationDevice,
        group: PresentationGroup,
        extent: Extent,
        opacity: Opacity,
        key: RegionKey,
        pool: u32,
    ) -> Result<Self> {
        // SAFETY: no security attributes; the out-parameter is a stack local, and
        // ownership of the handle transfers on success.
        let handle = unsafe {
            let mut handle: HANDLE = core::ptr::null_mut();
            DCompositionCreateSurfaceHandle(
                COMPOSITIONOBJECT_ALL_ACCESS as u32,
                core::ptr::null(),
                &mut handle,
            )
            .ok()?;
            SurfaceHandle(handle)
        };
        // SAFETY: the manager is live and owned by `group`; `handle.0` is the handle just
        // minted, and the surface does not take ownership of it.
        let surface = unsafe { group.manager().CreatePresentationSurface(handle.0)? };
        // SAFETY: `surface` is live and owned here.
        unsafe {
            // The compositor's canonical composition space, which is the space `Scrgb`
            // already is, so a buffer in it blends with no conversion. An 8-bit surface is
            // the only alternative, and it is treated as sRGB, colour-managed on terms this
            // crate does not set, and cannot hold a value above white or outside Rec.709.
            surface
                .SetColorSpace(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709)
                .ok()?;
            surface.SetAlphaMode(dxgi_alpha(opacity)).ok()?;
            // Set unconditionally rather than only when statistics are on: the system cannot
            // report on an untagged surface, so enabling the statistic kinds alone yields
            // nothing, and a region tagged only under statistics would present differently
            // from one that is not.
            surface.SetTag(key.0 as usize);
        }

        let mut region = Self {
            group,
            gpu: device.gpu().clone(),
            d3d: device.d3d().clone(),
            surface,
            handle,
            buffers: Vec::new(),
            acquired: Cell::new(0),
            submitted: Cell::new(0),
            extent,
            opacity,
            displayable: opacity == Opacity::Opaque && device.can_flip(),
            pool,
        };
        region.allocate()?;
        region.state_content_layout()?;
        Ok(region)
    }

    /// Returns the composition surface handle to bind into a visual tree, as
    /// `Scene::set_region` takes it.
    ///
    /// The only value on this type that may cross a thread boundary. The region keeps
    /// ownership and closes the handle on drop, so a binding must be released before the
    /// region is.
    #[must_use]
    pub fn surface_handle(&self) -> *mut core::ffi::c_void {
        self.handle.0
    }

    /// Returns the buffer allocation, in pixels.
    #[must_use]
    pub fn size_px(&self) -> (u32, u32) {
        self.extent.px()
    }

    /// Returns the box this region is allocated and drawn for.
    #[must_use]
    pub fn extent(&self) -> Extent {
        self.extent
    }

    /// Reports whether this region's buffers were allocated eligible for a display plane.
    ///
    /// A displayable allocation is a request and a driver may refuse it, in which case the
    /// region still presents and is composed instead. Reported here so a caller can tell the
    /// two apart without inferring it from statistics.
    #[must_use]
    pub fn is_displayable(&self) -> bool {
        self.displayable
    }

    /// Reports whether the group this region belongs to has failed. Regions do not fail
    /// individually: the manager is shared, so its loss is every member's.
    #[must_use]
    pub fn is_lost(&self) -> bool {
        self.group.is_lost()
    }

    /// Reallocates at a new box, keeping the surface handle, so a resize neither rebinds the
    /// visual tree nor disturbs any other region in the group.
    ///
    /// Tearing a region down and rebuilding it instead drops frames, reallocates buffers,
    /// and re-issues a handle the front thread has already bound.
    ///
    /// # Errors
    ///
    /// Fails when the new buffer pool cannot be allocated or the surface rejects the
    /// restated source rect.
    pub fn resize(&mut self, extent: Extent) -> Result<()> {
        if extent == self.extent {
            return Ok(());
        }
        self.extent = extent;
        // Dropping the old buffers only tells the manager they will not be presented again.
        // It keeps each alive until the present displaying it retires, so the screen never
        // shows a freed buffer during the swap.
        self.buffers.clear();
        // Both counters index the discarded pool, and anything drawn into it is not this
        // region's to bind.
        self.acquired.set(0);
        self.submitted.set(0);
        self.allocate()?;
        self.state_content_layout()
    }

    /// Takes the next buffer of the rotation, blocking until the compositor is finished with
    /// it.
    ///
    /// This wait is what paces the producer to the present queue, the way a waitable swap
    /// chain does, and it is the only pacing mechanism here. Inside a batch it does not
    /// block: the pool is `depth + slack`, so a pass never asks for a buffer the queue has
    /// not had time to retire, and widening the pool measured monotonically worse.
    ///
    /// The buffer is chosen by counting rather than by searching for a free one: with one
    /// present per buffer handed out, buffer *i* was last bound `pool` presents ago and has
    /// been superseded every time since. The wait stays because correctness does not rest on
    /// that arithmetic — a stalled queue blocks here rather than handing out a buffer the
    /// display is still reading.
    ///
    /// Returns `Ok(None)` to skip this frame, when the group is lost or the wait expired.
    /// Bind the returned target with [`Pass::draw`] and draw; the target's contents are
    /// undefined on entry.
    pub fn acquire(&self) -> Result<Option<&Target>> {
        if self.buffers.is_empty() || self.is_lost() {
            return Ok(None);
        }
        let index = (self.acquired.get() % u64::from(self.pool)) as usize;
        if !self.wait_for(index) {
            return Ok(None);
        }
        self.acquired.set(self.acquired.get() + 1);
        Ok(Some(&self.buffers[index].target))
    }

    /// Binds the oldest drawn-but-unbound buffer, to be shown by the group's next present.
    /// Does not present.
    ///
    /// Requiring [`Flushed`] is the contract: a buffer cannot be bound while its pixels are
    /// unflushed, and since the only way to obtain one is to have closed the pass, every
    /// region of every slot has necessarily drawn by the time any of them binds.
    ///
    /// Returns `Ok(false)` when there was nothing to bind — no matching
    /// [`acquire`](Self::acquire), or a lost group. A region that does not bind keeps showing
    /// whatever it last presented, so an idle region costs nothing on a frame its neighbours
    /// update.
    ///
    /// # Errors
    ///
    /// Fails when the surface rejects the buffer.
    pub fn submit(&self, _flushed: &Flushed) -> Result<bool> {
        if self.submitted.get() == self.acquired.get() || self.is_lost() {
            return Ok(false);
        }
        let index = (self.submitted.get() % u64::from(self.pool)) as usize;
        self.submitted.set(self.submitted.get() + 1);
        // SAFETY: `surface` and the buffer are live and owned by this region.
        unsafe { self.surface.SetBuffer(&self.buffers[index].buffer).ok()? };
        self.group.note_bound();
        Ok(true)
    }

    /// Blocks until the compositor has finished with buffer `index`. Returns `false` to skip
    /// the frame.
    ///
    /// The group's lost event is waited on alongside it and placed first, because
    /// `WaitForMultipleObjects` resolves ties by index: a group that dies while its buffer
    /// happens to be free must report the death rather than hand out a buffer.
    fn wait_for(&self, index: usize) -> bool {
        let handles = [self.group.lost_event(), self.buffers[index].available];
        // SAFETY: both handles are live kernel objects owned by the group and by this
        // region; the array is a stack local of the stated length.
        let result = unsafe {
            WaitForMultipleObjects(
                handles.len() as u32,
                handles.as_ptr(),
                false.into(),
                ACQUIRE_TIMEOUT_MS,
            )
        };
        result == (WAIT_OBJECT_0 as u32) + 1
    }

    /// States which part of the bound buffer is shown, and how it maps into the visual it is
    /// content for.
    ///
    /// A presentation surface's source rect and transform are zero-initialized, and a zero
    /// source rect samples nothing: the surface binds, the brush paints, and the result is an
    /// empty box with every call having returned success. The whole buffer and an identity
    /// transform are therefore stated at creation and restated on every resize.
    fn state_content_layout(&self) -> Result<()> {
        let (w, h) = self.extent.px();
        let source = RECT {
            left: 0,
            top: 0,
            right: w as i32,
            bottom: h as i32,
        };
        let mut identity = PresentationTransform {
            M11: 1.0,
            M12: 0.0,
            M21: 0.0,
            M22: 1.0,
            M31: 0.0,
            M32: 0.0,
        };
        // SAFETY: `surface` is live; both parameters are stack locals that outlive the
        // calls, and neither is retained.
        unsafe {
            self.surface.SetSourceRect(&source).ok()?;
            self.surface.SetTransform(&mut identity).ok()?;
        }
        Ok(())
    }

    /// Allocates the rotation: `pool` textures, each registered with the manager and adopted
    /// as a Direct2D target.
    fn allocate(&mut self) -> Result<()> {
        let (w, h) = self.extent.px();
        let pool = self.pool as usize;
        self.buffers.reserve(pool);
        let plain = D3D11_RESOURCE_MISC_SHARED | D3D11_RESOURCE_MISC_SHARED_NTHANDLE;
        let mut misc = if self.displayable {
            plain | D3D11_RESOURCE_MISC_SHARED_DISPLAYABLE
        } else {
            plain
        };
        for index in 0..pool {
            let texture = match self.texture(w, h, misc) {
                Ok(t) => t,
                // A displayable allocation is more constrained and a refusal arrives as a
                // plain allocation failure. Downgraded once for the whole region, so the
                // pool never mixes displayable and plain buffers, and only on the first
                // buffer, past which the region is committed to what it has.
                Err(_) if index == 0 && misc != plain => {
                    misc = plain;
                    self.displayable = false;
                    self.texture(w, h, plain)?
                }
                Err(e) => return Err(e),
            };
            // SAFETY: the manager is live; `texture` outlives the registration it returns.
            let buffer = unsafe { self.group.manager().AddBufferFromResource(&texture)? };
            // SAFETY: as above; the out-parameter is a stack local.
            let available = unsafe { buffer.GetAvailableEvent()? };
            // The texture is this crate's Direct3D projection and `adopt` takes a DXGI one.
            // Both are faces of a single COM object, so the cast inside `adopt` is a
            // `QueryInterface` rather than a conversion.
            let target = self.gpu.adopt(&texture, self.extent.dpi, self.opacity)?;
            self.buffers.push(Buffer {
                _texture: texture,
                buffer,
                target,
                available,
            });
        }
        Ok(())
    }

    /// Creates one FP16 render-target texture, shared and NT-handle-shareable, with `misc`
    /// deciding whether it is also requested displayable.
    fn texture(&self, w: u32, h: u32, misc: D3D11_RESOURCE_MISC_FLAG) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET) as u32,
            CPUAccessFlags: 0,
            MiscFlags: misc as u32,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        // SAFETY: `d3d` is live and owned here; the descriptor and the out-parameter are
        // stack locals that outlive the call.
        unsafe {
            self.d3d
                .CreateTexture2D(&desc, None, Some(&mut texture))
                .ok()?;
        }
        texture.ok_or_else(|| windows_core::Error::from_hresult(E_FAIL))
    }
}

/// Maps opacity to the surface's alpha mode, from the same value the Direct2D target's mode
/// comes from.
fn dxgi_alpha(opacity: Opacity) -> DXGI_ALPHA_MODE {
    match opacity {
        Opacity::Translucent => DXGI_ALPHA_MODE_PREMULTIPLIED,
        Opacity::Opaque => DXGI_ALPHA_MODE_IGNORE,
    }
}

#[cfg(test)]
mod tests {
    use super::Extent;

    /// The allocation follows the scale and never reaches zero: a region is legitimately
    /// laid out at zero before its first solve, and a zero-sized texture is refused.
    #[test]
    fn extents_allocate() {
        assert_eq!(Extent::new(100.0, 40.0, 96.0).px(), (100, 40));
        assert_eq!(Extent::new(100.0, 40.0, 144.0).px(), (150, 60));
        assert_eq!(Extent::new(100.0, 40.0, 120.0).px(), (125, 50));
        assert_eq!(Extent::new(0.0, 0.0, 96.0).px(), (1, 1));
        assert_eq!(Extent::new(100.0, 40.0, 192.0).scale(), 2.0);
    }
}
