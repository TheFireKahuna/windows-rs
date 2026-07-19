use super::*;

/// A handle to an OS event-like object that [`SwapChain::begin_draw`] blocks on
/// to pace a render thread to the present queue.
///
/// Normally this is a swap chain's own *frame-latency waitable object* (created
/// via [`GpuDevice::create_waitable_swap_chain`] and returned by
/// [`SwapChain::frame_latency_waitable`]), but you can substitute your own
/// waitable handle — a manual-reset event, a timer, … — with
/// [`SwapChain::set_wait_object`]. Wrap a raw OS handle with `WaitObject(handle)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitObject(pub *mut core::ffi::c_void);

// The wrapped value is an OS kernel handle, which is valid and safe to wait on
// from any thread, so a `SwapChain` carrying one can still move to a render
// thread. (The raw-pointer field is otherwise `!Send`.)
unsafe impl Send for WaitObject {}
unsafe impl Sync for WaitObject {}

/// Manages a DXGI swap chain.
pub struct SwapChain {
    swap_chain: IDXGISwapChain1,
    d2d_context: ID2D1DeviceContext,
    device_lost_flag: Cell<bool>,
    // `Some` when the backing device is multi-threaded; lets this swap chain
    // serialize its direct DXGI calls (`Present`, `ResizeBuffers`, …) against
    // Direct2D's work on the shared Direct3D device. A clone is held so the swap
    // chain can be moved to a render thread without borrowing the device.
    multithread: Option<ID2D1Multithread>,
    // The chain's own frame-latency waitable object, present only when it was
    // created waitable. Returned by `frame_latency_waitable`; never changes after
    // creation. Its presence also means the `FRAME_LATENCY_WAITABLE_OBJECT` flag
    // must be replayed on `ResizeBuffers`.
    frame_latency: Option<WaitObject>,
    // What `begin_draw` blocks on before producing a frame. Defaults to
    // `frame_latency`; a consumer can substitute their own object or clear it
    // (`None`) to pace the frame themselves.
    wait: Option<WaitObject>,
    width: u32,
    height: u32,
    dpi_x: f32,
    dpi_y: f32,
}

impl SwapChain {
    /// Acquires the Direct2D factory lock for the surrounding DXGI critical
    /// section. A no-op when the device is single-threaded.
    fn lock(&self) -> D2dLock<'_> {
        D2dLock::enter(self.multithread.as_ref())
    }

    pub(crate) fn new(device: &GpuDevice, width: u32, height: u32) -> Result<Self> {
        Self::new_composition(device, width, height, 0)
    }

    /// Creates a composition swap chain with the frame-latency waitable object
    /// flag, capturing the waitable handle that [`begin_draw`](Self::begin_draw)
    /// blocks on. See [`GpuDevice::create_waitable_swap_chain`].
    pub(crate) fn new_waitable(device: &GpuDevice, width: u32, height: u32) -> Result<Self> {
        Self::new_composition(
            device,
            width,
            height,
            DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT as u32,
        )
    }

    fn new_composition(device: &GpuDevice, width: u32, height: u32, flags: u32) -> Result<Self> {
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: flags,
            ..Default::default()
        };

        let swap_chain = unsafe {
            let _lock = device.lock();
            device
                .dxgi_factory()
                .CreateSwapChainForComposition(device.d3d_device(), &desc, None)?
        };

        let mut result = Self::from_swap_chain(device, swap_chain, width, height)?;
        if flags & (DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT as u32) != 0 {
            result.init_frame_latency()?;
        }
        Ok(result)
    }

    /// For a waitable chain: cap the queued frames at one and capture the
    /// frame-latency waitable object that `begin_draw` blocks on. The `cast`,
    /// `SetMaximumFrameLatency`, and `GetFrameLatencyWaitableObject` are direct
    /// DXGI calls, so they run under the factory lock; the later wait on the
    /// returned handle does not (it is an OS wait — see `begin_draw`).
    fn init_frame_latency(&mut self) -> Result<()> {
        let wait = unsafe {
            let _lock = self.lock();
            let sc2 = self.swap_chain.cast::<IDXGISwapChain2>()?;
            sc2.SetMaximumFrameLatency(1).ok()?;
            WaitObject(sc2.GetFrameLatencyWaitableObject())
        };
        self.frame_latency = Some(wait);
        self.wait = Some(wait);
        Ok(())
    }

    pub(crate) fn new_for_hwnd(
        device: &GpuDevice,
        hwnd: *mut core::ffi::c_void,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            ..Default::default()
        };

        let swap_chain = unsafe {
            let _lock = device.lock();
            device.dxgi_factory().CreateSwapChainForHwnd(
                device.d3d_device(),
                hwnd,
                &desc,
                None,
                None,
            )?
        };

        Self::from_swap_chain(device, swap_chain, width, height)
    }

    fn from_swap_chain(
        device: &GpuDevice,
        swap_chain: IDXGISwapChain1,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let d2d_context = unsafe {
            device
                .d2d_device()
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
        };

        let mut result = Self {
            swap_chain,
            d2d_context,
            device_lost_flag: Cell::new(false),
            multithread: device.multithread_handle(),
            frame_latency: None,
            wait: None,
            width,
            height,
            dpi_x: 96.0,
            dpi_y: 96.0,
        };
        result.set_target()?;
        Ok(result)
    }

    /// Resizes the swap chain buffers. A zero width or height is ignored.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        // A waitable chain must keep its creation flag across resizes.
        let flags = if self.frame_latency.is_some() {
            DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT as u32
        } else {
            0
        };
        unsafe {
            let _lock = self.lock();
            self.d2d_context.SetTarget(None);
            self.swap_chain
                .ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, flags)
                .ok()?;
        }
        self.width = width;
        self.height = height;
        self.set_target()
    }

    /// Begins drawing a frame, returning a [`DrawingSession`].
    ///
    /// For a waitable swap chain (see [`GpuDevice::create_waitable_swap_chain`])
    /// this first blocks until the chain can accept a new frame, pacing the
    /// render thread to the present queue. The wait target can be replaced or
    /// disabled with [`set_wait_object`](Self::set_wait_object).
    pub fn begin_draw(&mut self) -> Result<DrawingSession<'_>> {
        // Pace the producer before drawing. Deliberately outside the factory
        // lock: this is an OS wait, and blocking while holding the D2D lock would
        // stall every other thread sharing the device. A 1-second timeout guards
        // against a never-signalled object; we render regardless of the result.
        if let Some(wait) = self.wait {
            unsafe { WaitForSingleObjectEx(wait.0, 1000, false.into()) };
        }
        self.device_lost_flag.set(false);
        // A swap-chain target is 8-bit `B8G8R8A8_UNORM` (sRGB), so its session
        // linear→sRGB encodes every color at the boundary (clears, gradients, …).
        DrawingSession::new(&self.d2d_context, &self.device_lost_flag)
            .map(|s| s.encode_srgb_target(true))
    }

    /// Returns the chain's own frame-latency waitable object, or `None` when it
    /// was not created waitable (via [`GpuDevice::create_waitable_swap_chain`]).
    ///
    /// `begin_draw` already waits on this for you. Read it only to fold it into a
    /// composite wait of your own — in which case clear the built-in wait first
    /// with [`set_wait_object(None)`](Self::set_wait_object).
    pub fn frame_latency_waitable(&self) -> Option<WaitObject> {
        self.frame_latency
    }

    /// Overrides what [`begin_draw`](Self::begin_draw) blocks on before each
    /// frame: `Some(obj)` waits on your own object instead of the chain's
    /// frame-latency object; `None` disables the built-in wait so you can pace
    /// the frame yourself. Defaults to the chain's frame-latency object for a
    /// waitable chain, or to no wait otherwise.
    pub fn set_wait_object(&mut self, wait: Option<WaitObject>) {
        self.wait = wait;
    }

    /// Returns `Ok(true)` on success or `Ok(false)` if the device was lost.
    pub fn present(&self) -> Result<bool> {
        // If EndDraw detected device-lost, don't bother presenting.
        if self.device_lost_flag.get() {
            return Ok(false);
        }
        let result = unsafe {
            let _lock = self.lock();
            self.swap_chain.Present(1, 0).ok()
        };
        if check_device_lost(&result) {
            return Ok(false);
        }
        result.map(|()| true)
    }

    /// Creates a solid color brush. A swap-chain target is 8-bit sRGB, so the brush
    /// linear→sRGB encodes the (linear) color it is given, now and on every recolor.
    pub fn create_solid_brush(&self, color: ColorF) -> Result<Brush> {
        let c: D2D_COLOR_F = color.to_srgb().into();
        unsafe {
            self.d2d_context
                .CreateSolidColorBrush(&c, None)
                .map(|b| Brush::new(b, true))
        }
    }

    /// Loads a bitmap from an image file.
    pub fn load_bitmap(&self, path: impl AsRef<std::path::Path>) -> Result<Bitmap> {
        Bitmap::load_from_file(&self.d2d_context, path.as_ref())
    }

    /// Returns the underlying `IDXGISwapChain1`.
    pub fn raw_swap_chain(&self) -> &IDXGISwapChain1 {
        &self.swap_chain
    }

    /// Returns the width of the swap chain, in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the swap chain, in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Set the DPI so that Direct2D renders at the correct resolution.
    pub fn set_dpi(&mut self, dpi_x: f32, dpi_y: f32) {
        self.dpi_x = dpi_x;
        self.dpi_y = dpi_y;
        unsafe { self.d2d_context.SetDpi(dpi_x, dpi_y) }
        // Recreate the target bitmap with the updated DPI.
        let _ = self.set_target();
    }

    /// Apply an inverse composition scale so that a pixel-sized buffer
    /// is presented at the correct DIP size.
    pub fn set_composition_scale(&self, scale_x: f32, scale_y: f32) {
        // `cast` is a direct DXGI `QueryInterface`, so it belongs inside the lock
        // alongside `SetMatrixTransform`.
        unsafe {
            let _lock = self.lock();
            if let Ok(sc2) = self.swap_chain.cast::<IDXGISwapChain2>() {
                let matrix = DXGI_MATRIX_3X2_F {
                    _11: 1.0 / scale_x,
                    _12: 0.0,
                    _21: 0.0,
                    _22: 1.0 / scale_y,
                    _31: 0.0,
                    _32: 0.0,
                };
                let _ = sc2.SetMatrixTransform(&matrix);
            }
        }
    }

    /// Returns `true` if the device was lost during the last frame.
    pub fn is_device_lost(&self) -> bool {
        self.device_lost_flag.get()
    }

    fn set_target(&mut self) -> Result<()> {
        unsafe {
            let _lock = self.lock();
            let surface: IDXGISurface = self.swap_chain.GetBuffer(0)?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: self.dpi_x,
                dpiY: self.dpi_y,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let bitmap = self
                .d2d_context
                .CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
            self.d2d_context.SetTarget(&bitmap);
            // Ensure context DPI matches after SetTarget (some target types reset it).
            self.d2d_context.SetDpi(self.dpi_x, self.dpi_y);
            Ok(())
        }
    }
}
