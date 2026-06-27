use super::*;

/// A `SurfaceImageSource` you create and draw into with Direct2D, then display
/// by handing it to [`Image::new`](crate::Image::new). Create it on the UI
/// thread (for example inside your render function).
///
/// Drawing must happen on the UI thread. Call [`set_device`](Self::set_device)
/// once with your Direct2D device, then bracket each frame between
/// [`begin_draw`](Self::begin_draw) and [`end_draw`](Self::end_draw). The same
/// source can be drawn into before or after it is attached to an `Image`.
#[derive(Clone, Debug)]
pub struct SurfaceImageSource {
    // Cast to `ImageSource` and applied as the native `Image.Source`.
    pub source: bindings::SurfaceImageSource,
    native: bindings::ISurfaceImageSourceNativeWithD2D,
    // `Some` only when [`set_device`](Self::set_device) is given a device backed
    // by a *multi-threaded* Direct2D factory. It lets the DXGI-touching native
    // calls (`BeginDraw`/`EndDraw`/`SuspendDraw`/`ResumeDraw`) serialize against
    // Direct2D's work on the shared Direct3D device, so a UI-thread frame can't
    // race a background render thread's `Present`. Derived from the device the
    // caller already passes, so it stays entirely transparent — no public API.
    multithread: core::cell::RefCell<Option<bindings::ID2D1Multithread>>,
}

// Identity is the underlying native source; the derived factory lock is internal
// state, so it is deliberately excluded from equality.
impl PartialEq for SurfaceImageSource {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.native == other.native
    }
}

/// RAII guard over the Direct2D factory lock ([`bindings::ID2D1Multithread`])
/// wrapping a single native `SurfaceImageSource` DXGI-interop call: `Enter` on
/// construction, the paired `Leave` on `Drop` (released even on an early `?`
/// return or a panic). Holds an owned clone of the lock so it never borrows the
/// source's `RefCell` across the call. A no-op for a single-threaded device.
struct D2dLock(Option<bindings::ID2D1Multithread>);

impl D2dLock {
    fn enter(multithread: Option<bindings::ID2D1Multithread>) -> Self {
        if let Some(multithread) = &multithread {
            unsafe { multithread.Enter() };
        }
        Self(multithread)
    }
}

impl Drop for D2dLock {
    fn drop(&mut self) {
        if let Some(multithread) = &self.0 {
            unsafe { multithread.Leave() };
        }
    }
}

impl SurfaceImageSource {
    /// Create a `SurfaceImageSource` of the given pixel size. The size is fixed
    /// for the lifetime of the source; create a new one to resize.
    pub fn new(pixel_width: i32, pixel_height: i32) -> Result<Self> {
        let source = bindings::SurfaceImageSource::CreateInstanceWithDimensions(
            pixel_width,
            pixel_height,
        )?;
        let native = source.cast()?;
        Ok(Self {
            source,
            native,
            multithread: core::cell::RefCell::new(None),
        })
    }

    /// Associate the Direct2D device used for drawing. Pass an `ID2D1Device`
    /// (or `IDXGIDevice`). Must be called before [`begin_draw`](Self::begin_draw).
    ///
    /// If the device is backed by a multi-threaded Direct2D factory, this also
    /// captures the factory lock so each draw call serializes its DXGI interop
    /// against other threads sharing the device. This is transparent: a
    /// single-threaded device captures nothing and pays no cost.
    pub fn set_device(&self, device: &impl Interface) -> Result<()> {
        unsafe { self.native.SetDevice(device.as_raw()).ok()? };
        // Walk device -> factory -> ID2D1Multithread, keeping it only when the
        // factory is actually multi-threaded (so single-threaded stays a no-op).
        let lock = device
            .cast::<bindings::ID2D1Resource>()
            .ok()
            .and_then(|resource| unsafe { resource.GetFactory() }.ok())
            .and_then(|factory| factory.cast::<bindings::ID2D1Multithread>().ok())
            .filter(|multithread| unsafe { multithread.GetMultithreadProtected() }.as_bool());
        *self.multithread.borrow_mut() = lock;
        Ok(())
    }

    /// Acquires the Direct2D factory lock for one native DXGI-interop call. A
    /// no-op when the backing device is single-threaded (or no device is set).
    fn lock(&self) -> D2dLock {
        D2dLock::enter(self.multithread.borrow().clone())
    }

    /// Begin drawing into the surface, returning the drawing target `T`
    /// (typically `ID2D1DeviceContext`) and the `(x, y)` pixel offset within the
    /// underlying atlas at which to draw. The update region is given in pixels
    /// as `(x, y, width, height)`; apply the returned offset as a translation on
    /// the drawing target before issuing draw calls.
    pub fn begin_draw<T: Interface>(
        &self,
        update_x: i32,
        update_y: i32,
        update_width: i32,
        update_height: i32,
    ) -> Result<(T, (i32, i32))> {
        let update_rect = bindings::RECT {
            left: update_x,
            top: update_y,
            right: update_x + update_width,
            bottom: update_y + update_height,
        };
        let mut offset = bindings::POINT::default();
        let mut object = core::ptr::null_mut();
        unsafe {
            let _lock = self.lock();
            self.native
                .BeginDraw(&update_rect, &T::IID, &mut object, &mut offset)
                .ok()?;
            Ok((T::from_raw(object), (offset.x, offset.y)))
        }
    }

    /// Finish drawing and present the surface contents.
    pub fn end_draw(&self) -> Result<()> {
        unsafe {
            let _lock = self.lock();
            self.native.EndDraw().ok()
        }
    }

    /// Suspend drawing, allowing GPU resources to be reclaimed.
    pub fn suspend_draw(&self) -> Result<()> {
        unsafe {
            let _lock = self.lock();
            self.native.SuspendDraw().ok()
        }
    }

    /// Resume drawing after a [`suspend_draw`](Self::suspend_draw).
    pub fn resume_draw(&self) -> Result<()> {
        unsafe {
            let _lock = self.lock();
            self.native.ResumeDraw().ok()
        }
    }

    /// Cast the underlying source to the `ImageSource` the backend assigns to
    /// `Image.Source`.
    pub fn image_source(&self) -> Result<bindings::ImageSource> {
        self.source.cast()
    }
}
