use super::*;

/// Shared GPU device.
///
/// `Clone` is a COM `AddRef` of the same underlying D3D11/D2D device, so a clone
/// refers to the *same* device. Combined with [`new_multi_threaded`](Self::new_multi_threaded),
/// this lets one device be driven from more than one thread — e.g. a UI thread
/// drawing `SurfaceImageSource`s and a render thread presenting a composition
/// swap chain — with Direct2D serializing access to the shared resources.
#[derive(Clone)]
pub struct GpuDevice {
    d3d_device: ID3D11Device,
    d2d_factory: ID2D1Factory1,
    d2d_device: ID2D1Device,
    dxgi_factory: IDXGIFactory2,
    dwrite_factory: IDWriteFactory,
    // `Some` only for a multi-threaded factory. When `None`, [`GpuDevice::lock`]
    // is a true no-op, so single-threaded devices pay nothing.
    multithread: Option<ID2D1Multithread>,
}

/// RAII guard over the Direct2D factory lock ([`ID2D1Multithread`]).
///
/// A multi-threaded Direct2D factory automatically serializes calls to the
/// Direct2D resources created from it, but that protection does **not** extend to
/// direct Direct3D/DXGI calls (`Present`, `ResizeBuffers`, `GetBuffer`, swap-chain
/// creation, …). The crate brackets those internally with this guard so they
/// can't race another thread's Direct2D work on the shared Direct3D device:
/// `Enter` on construction, the paired `Leave` on `Drop` (released even on an
/// early `?` return or a panic). For a single-threaded device it holds no lock and
/// does nothing.
#[must_use = "the lock is released as soon as the guard is dropped"]
pub(crate) struct D2dLock<'a>(Option<&'a ID2D1Multithread>);

impl<'a> D2dLock<'a> {
    pub(crate) fn enter(multithread: Option<&'a ID2D1Multithread>) -> Self {
        if let Some(multithread) = multithread {
            unsafe { multithread.Enter() }
        }
        Self(multithread)
    }
}

impl Drop for D2dLock<'_> {
    fn drop(&mut self) {
        if let Some(multithread) = self.0 {
            unsafe { multithread.Leave() }
        }
    }
}

impl GpuDevice {
    /// Creates a new hardware-accelerated GPU device with a single-threaded
    /// Direct2D factory (the default — use it when the device is only ever
    /// touched from one thread).
    pub fn new() -> Result<Self> {
        unsafe { Self::create(false, false) }
    }

    /// Creates a hardware-accelerated GPU device with a **multi-threaded**
    /// Direct2D factory, so the one device can be driven from more than one
    /// thread. Direct2D serializes calls to the resources created from the
    /// factory, and every [`SwapChain`] made from this device also serializes its
    /// own direct DXGI calls (`Present`, `ResizeBuffers`, …) against that work —
    /// so sharing the device across threads, each rendering through its own
    /// `SwapChain`, is safe.
    ///
    /// Direct Direct3D/DXGI/`SurfaceImageSource` calls you make yourself, outside
    /// this crate, are **not** covered and must be serialized by you.
    pub fn new_multi_threaded() -> Result<Self> {
        unsafe { Self::create(false, true) }
    }

    /// Create a software (WARP) device for testing or headless rendering.
    pub fn new_warp() -> Result<Self> {
        unsafe { Self::create(true, false) }
    }

    /// Creates a hardware device, falling back to a software (WARP) device when
    /// no GPU is available (headless sessions, VMs, or RDP). Use this for render
    /// loops that must produce output on any machine.
    pub fn new_or_warp() -> Result<Self> {
        Self::new().or_else(|_| Self::new_warp())
    }

    /// Create a software (WARP) device with a **multi-threaded** Direct2D factory:
    /// the GPU-independence of [`new_warp`](Self::new_warp) combined with the
    /// cross-thread sharing of [`new_multi_threaded`](Self::new_multi_threaded).
    /// Useful for headless multi-threaded rendering and for testing the
    /// multi-threaded path without a GPU.
    pub fn new_warp_multi_threaded() -> Result<Self> {
        unsafe { Self::create(true, true) }
    }

    unsafe fn create(software: bool, multi_threaded: bool) -> Result<Self> {
        let driver_type = if software {
            D3D_DRIVER_TYPE_WARP
        } else {
            D3D_DRIVER_TYPE_HARDWARE
        };
        let factory_type = if multi_threaded {
            D2D1_FACTORY_TYPE_MULTI_THREADED
        } else {
            D2D1_FACTORY_TYPE_SINGLE_THREADED
        };

        let mut d3d_device: Option<ID3D11Device> = None;
        let feature_levels = [D3D_FEATURE_LEVEL_11_0];
        unsafe {
            D3D11CreateDevice(
                std::ptr::null_mut(),
                driver_type,
                std::ptr::null_mut(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                feature_levels.as_ptr(),
                feature_levels.len() as u32,
                D3D11_SDK_VERSION,
                &mut d3d_device as *mut _ as *mut _,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
            .ok()?;
        }
        let d3d_device = d3d_device.unwrap();

        let mut d2d_factory: Option<ID2D1Factory1> = None;
        unsafe {
            D2D1CreateFactory(
                factory_type,
                &ID2D1Factory1::IID,
                std::ptr::null(),
                &mut d2d_factory as *mut _ as *mut _,
            )
            .ok()?;
        }
        let d2d_factory = d2d_factory.unwrap();

        // For a multi-threaded factory, hold the `ID2D1Multithread` lock so direct
        // Direct3D/DXGI calls can be serialized against Direct2D's own work.
        let multithread = if multi_threaded {
            Some(d2d_factory.cast::<ID2D1Multithread>()?)
        } else {
            None
        };

        // These calls touch the shared Direct3D device through DXGI; guard them.
        let _lock = D2dLock::enter(multithread.as_ref());
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;
        let d2d_device = unsafe { d2d_factory.CreateDevice(&dxgi_device)? };

        let dxgi_adapter: IDXGIAdapter = unsafe { dxgi_device.GetAdapter()? };
        let dxgi_factory: IDXGIFactory2 = unsafe { dxgi_adapter.GetParent()? };
        drop(_lock);

        let dwrite_factory = dwrite_factory()?;

        Ok(Self {
            d3d_device,
            d2d_factory,
            d2d_device,
            dxgi_factory,
            dwrite_factory,
            multithread,
        })
    }

    /// Creates a swap chain for off-screen or composition rendering.
    pub fn create_swap_chain(&self, width: u32, height: u32) -> Result<SwapChain> {
        SwapChain::new(self, width, height)
    }

    /// Creates a composition swap chain whose [`SwapChain::begin_draw`] blocks
    /// until the swap chain can accept a new frame — a *waitable* swap chain with
    /// a maximum frame latency of one.
    ///
    /// This paces a render thread to the present queue, lowering latency. Use it
    /// for the render-thread rendering model; it is **not** for the UI-thread
    /// `CompositionTarget.Rendering` model, where the compositor already paces you
    /// and an extra wait on the UI thread would stall it. The built-in wait can be
    /// substituted or disabled with [`SwapChain::set_wait_object`].
    pub fn create_waitable_swap_chain(&self, width: u32, height: u32) -> Result<SwapChain> {
        SwapChain::new_waitable(self, width, height)
    }

    /// Create an HWND swap chain for standalone windowed rendering.
    ///
    /// # Safety
    ///
    /// `hwnd` must be a valid window handle for the lifetime of the returned `SwapChain`.
    pub unsafe fn create_swap_chain_for_hwnd(
        &self,
        hwnd: *mut core::ffi::c_void,
        width: u32,
        height: u32,
    ) -> Result<SwapChain> {
        SwapChain::new_for_hwnd(self, hwnd, width, height)
    }

    /// Returns the underlying `ID3D11Device`.
    pub fn d3d_device(&self) -> &ID3D11Device {
        &self.d3d_device
    }

    /// Returns the underlying `ID2D1Device`.
    pub fn d2d_device(&self) -> &ID2D1Device {
        &self.d2d_device
    }

    /// Acquires the Direct2D factory lock for the duration of the returned guard.
    ///
    /// Used internally to wrap each direct Direct3D/DXGI call (`Present`,
    /// `ResizeBuffers`, `GetBuffer`, swap-chain creation, …) so it can't race
    /// another thread's Direct2D work on a shared multi-threaded device. Pure
    /// Direct2D calls don't need it — the multi-threaded factory already serializes
    /// those. A no-op for a single-threaded device.
    pub(crate) fn lock(&self) -> D2dLock<'_> {
        D2dLock::enter(self.multithread.as_ref())
    }

    /// Returns a clone of the `ID2D1Multithread` lock when this device was created
    /// multi-threaded, or `None` otherwise. Lets a [`SwapChain`] keep the lock so
    /// it can serialize its own DXGI calls after the borrow of the device ends.
    pub(crate) fn multithread_handle(&self) -> Option<ID2D1Multithread> {
        self.multithread.clone()
    }

    /// Returns the underlying `ID2D1Factory1`.
    pub fn d2d_factory(&self) -> &ID2D1Factory1 {
        &self.d2d_factory
    }

    /// Creates a stroke style from the given builder.
    pub fn create_stroke_style(&self, builder: &StrokeStyleBuilder) -> Result<StrokeStyle> {
        let props = builder.to_abi();
        unsafe {
            self.d2d_factory
                .CreateStrokeStyle(&props, None)
                .map(StrokeStyle)
        }
    }

    /// Returns the underlying `IDXGIFactory2`.
    pub fn dxgi_factory(&self) -> &IDXGIFactory2 {
        &self.dxgi_factory
    }

    /// Returns the underlying `IDWriteFactory`.
    pub fn dwrite_factory(&self) -> &IDWriteFactory {
        &self.dwrite_factory
    }
}
