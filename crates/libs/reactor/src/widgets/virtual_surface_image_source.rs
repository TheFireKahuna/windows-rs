use super::*;
use crate::bindings::{
    IVirtualSurfaceImageSourceNative, IVirtualSurfaceUpdatesCallbackNative,
    IVirtualSurfaceUpdatesCallbackNative_Impl,
};
use windows_core::implement_decl;

/// A rectangular region, in physical pixels, of a
/// [`VirtualSurfaceImageSource`]. Used both for the regions the framework asks
/// you to redraw and for [`invalidate`](VirtualSurfaceImageSource::invalidate).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct UpdateRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl UpdateRect {
    fn from_abi(r: bindings::RECT) -> Self {
        Self {
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
        }
    }

    fn to_abi(self) -> bindings::RECT {
        bindings::RECT {
            left: self.x,
            top: self.y,
            right: self.x + self.width,
            bottom: self.y + self.height,
        }
    }
}

/// RAII guard over the Direct2D factory lock wrapping a single native DXGI
/// interop call (`Enter` on construction, `Leave` on `Drop`). A no-op for a
/// single-threaded device. Mirrors the guard in the fixed `SurfaceImageSource`,
/// kept local so this module stays self-contained.
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

/// A `VirtualSurfaceImageSource` you draw into with Direct2D, then display by
/// handing it to [`Image::new`](crate::Image::new). Unlike a fixed
/// [`SurfaceImageSource`], the surface can be larger than the screen — or larger
/// than the GPU's maximum texture size — because the framework virtualizes it:
/// only the regions that are actually visible are kept resident, and it asks you
/// (through [`register_for_updates`](Self::register_for_updates)) to redraw
/// regions as they scroll into view. Use it for maps, large document canvases,
/// and other pannable/zoomable content.
///
/// Drawing each region uses the same [`begin_draw`](Self::begin_draw) /
/// [`end_draw`](Self::end_draw) bracket as [`SurfaceImageSource`], constrained to
/// the region's rectangle for performance.
#[derive(Clone, Debug)]
pub struct VirtualSurfaceImageSource {
    // Cast to `ImageSource` and applied as the native `Image.Source`.
    source: bindings::VirtualSurfaceImageSource,
    // Drawing interface (the docs require SetDevice/BeginDraw/EndDraw to go
    // through `ISurfaceImageSourceNativeWithD2D` for both the fixed and the
    // virtual surface).
    draw: bindings::ISurfaceImageSourceNativeWithD2D,
    // Virtualization interface: update regions, invalidation, resize, callback.
    native: IVirtualSurfaceImageSourceNative,
    // See `SurfaceImageSource`: `Some` only for a multi-threaded factory.
    multithread: core::cell::RefCell<Option<bindings::ID2D1Multithread>>,
}

// Identity is the underlying native source; the draw/virtualization interfaces
// are casts of it and any registration is internal state, so equality compares
// the source alone.
impl PartialEq for VirtualSurfaceImageSource {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl VirtualSurfaceImageSource {
    /// Create a `VirtualSurfaceImageSource` of the given initial pixel size. The
    /// size can be changed later with [`resize`](Self::resize).
    pub fn new(initial_width: i32, initial_height: i32) -> Result<Self> {
        let source = bindings::VirtualSurfaceImageSource::CreateInstanceWithDimensions(
            initial_width,
            initial_height,
        )?;
        let draw = source.cast()?;
        let native = source.cast()?;
        Ok(Self {
            source,
            draw,
            native,
            multithread: core::cell::RefCell::new(None),
        })
    }

    /// Associate the Direct2D device used for drawing. See
    /// [`SurfaceImageSource::set_device`](crate::SurfaceImageSource::set_device).
    pub fn set_device(&self, device: &impl Interface) -> Result<()> {
        unsafe { self.draw.SetDevice(device.as_raw())? };
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

    /// Begin drawing into one region of the surface, returning the drawing target
    /// `T` and the `(x, y)` pixel offset within the atlas to translate by. Pass
    /// the region (typically one of the rects from
    /// [`register_for_updates`](Self::register_for_updates)); only those pixels
    /// are presented, so keep the rectangle tight.
    pub fn begin_draw<T: Interface>(&self, rect: UpdateRect) -> Result<(T, (i32, i32))> {
        let update_rect = rect.to_abi();
        let mut offset = bindings::POINT::default();
        let mut object = core::ptr::null_mut();
        unsafe {
            let _lock = self.lock();
            self.draw
                .BeginDraw(&update_rect, &T::IID, &mut object, &mut offset)?;
            Ok((T::from_raw(object), (offset.x, offset.y)))
        }
    }

    /// Finish drawing a region and present it. Must be called on the UI thread.
    pub fn end_draw(&self) -> Result<()> {
        unsafe {
            let _lock = self.lock();
            self.draw.EndDraw()
        }
    }

    /// Suspend drawing, allowing GPU resources to be reclaimed.
    pub fn suspend_draw(&self) -> Result<()> {
        unsafe {
            let _lock = self.lock();
            self.draw.SuspendDraw()
        }
    }

    /// Resume drawing after a [`suspend_draw`](Self::suspend_draw).
    pub fn resume_draw(&self) -> Result<()> {
        unsafe {
            let _lock = self.lock();
            self.draw.ResumeDraw()
        }
    }

    /// Mark a region as needing to be redrawn. The framework will call your
    /// registered update handler for the part of it that is visible.
    pub fn invalidate(&self, rect: UpdateRect) -> Result<()> {
        let _lock = self.lock();
        unsafe { self.native.Invalidate(rect.to_abi()) }
    }

    /// Resize the virtual surface (in pixels). Existing content outside the new
    /// bounds is discarded.
    pub fn resize(&self, width: i32, height: i32) -> Result<()> {
        // `Resize` can release tile allocations on the shared Direct3D device, so
        // bracket it against the same factory lock the draw calls use.
        let _lock = self.lock();
        unsafe { self.native.Resize(width, height) }
    }

    /// Returns the currently visible region of the surface, in pixels.
    pub fn visible_bounds(&self) -> Result<UpdateRect> {
        Ok(UpdateRect::from_abi(unsafe { self.native.GetVisibleBounds()? }))
    }

    /// Register a handler the framework calls whenever regions of the surface
    /// need to be (re)drawn — when content scrolls into view or after
    /// [`invalidate`](Self::invalidate). The handler is given the list of dirty
    /// rectangles; draw each with [`begin_draw`](Self::begin_draw) /
    /// [`end_draw`](Self::end_draw).
    ///
    /// Drawing must not happen while the window is hidden or inactive, or the
    /// native calls fail; the framework only requests updates for visible
    /// content, so simply honoring the requested rects satisfies this.
    ///
    /// Returns an RAII [`UpdatesRegistration`]: keep it alive for as long as you
    /// want updates, and drop it to unregister. Dropping it is also what breaks
    /// the reference cycle between the surface and its callback, so don't leak
    /// it.
    pub fn register_for_updates(
        &self,
        on_updates: impl Fn(&[UpdateRect]) + 'static,
    ) -> Result<UpdatesRegistration> {
        let callback: IVirtualSurfaceUpdatesCallbackNative = UpdatesCallback {
            native: self.native.clone(),
            on_updates: Box::new(on_updates),
            raw: core::cell::RefCell::new(Vec::new()),
            rects: core::cell::RefCell::new(Vec::new()),
        }
        .into();
        unsafe { self.native.RegisterForUpdatesNeeded(callback.as_raw())? };
        Ok(UpdatesRegistration {
            native: self.native.clone(),
            _callback: callback,
        })
    }

    /// Cast the underlying source to the `ImageSource` the backend assigns to
    /// `Image.Source`.
    #[cfg(feature = "winui-backend")]
    pub(crate) fn image_source(&self) -> Result<bindings::ImageSource> {
        self.source.cast()
    }
}

/// RAII handle for an update registration made by
/// [`VirtualSurfaceImageSource::register_for_updates`]. Dropping it unregisters
/// the callback (and releases the surface↔callback reference cycle).
#[must_use = "dropping this immediately unregisters the update callback"]
pub struct UpdatesRegistration {
    native: IVirtualSurfaceImageSourceNative,
    // Kept alive so the framework's stored callback pointer stays valid until we
    // unregister in `Drop`.
    _callback: IVirtualSurfaceUpdatesCallbackNative,
}

impl Drop for UpdatesRegistration {
    fn drop(&mut self) {
        // Clear the registration before `_callback` is released, both to stop
        // further callbacks and to break the surface -> callback -> surface
        // strong-reference cycle.
        unsafe {
            let _ = self.native.RegisterForUpdatesNeeded(core::ptr::null_mut());
        }
    }
}

// The COM object the framework calls back into. Holds the native surface (to
// query the dirty rectangles) plus the user's handler. The strong ref to the
// surface forms a cycle that `UpdatesRegistration::drop` breaks.
struct UpdatesCallback {
    native: IVirtualSurfaceImageSourceNative,
    on_updates: Box<dyn Fn(&[UpdateRect])>,
    // Scratch buffers reused across callbacks so `UpdatesNeeded` allocates
    // nothing in steady state; `cap_scratch` reclaims the excess after a one-off
    // large batch so capacity stays bounded. Only ever touched on the UI thread,
    // where the framework delivers the callback.
    raw: core::cell::RefCell<Vec<bindings::RECT>>,
    rects: core::cell::RefCell<Vec<UpdateRect>>,
}

implement_decl! {
    impl UpdatesCallback as UpdatesCallback_Impl: [IVirtualSurfaceUpdatesCallbackNative]
}

/// Headroom (in elements) retained on a reused scratch buffer above its last
/// use, so the common small-batch case never reallocates.
const SCRATCH_HEADROOM: usize = 32;

/// Cap a reused scratch buffer's retained capacity so a one-off large update
/// batch (e.g. panning a huge surface) doesn't pin memory for the registration's
/// lifetime. Reclaims only when the capacity is more than double the working set
/// plus headroom — and only down to that headroom — so steady state runs
/// realloc-free (the test fails) and a spike costs a single reclaim afterwards.
fn cap_scratch<T>(buf: &mut Vec<T>) {
    let target = buf.len() + SCRATCH_HEADROOM;
    if buf.capacity() > target * 2 {
        buf.shrink_to(target);
    }
}

impl IVirtualSurfaceUpdatesCallbackNative_Impl for UpdatesCallback_Impl {
    fn UpdatesNeeded(&self) -> Result<()> {
        let count = unsafe { self.native.GetUpdateRectCount()? } as usize;
        if count == 0 {
            return Ok(());
        }
        // Fill the reusable buffers (resize/extend reuse their capacity).
        let mut rects = self.rects.borrow_mut();
        rects.clear();
        {
            let mut raw = self.raw.borrow_mut();
            raw.resize(count, bindings::RECT::default());
            unsafe { self.native.GetUpdateRects(raw.as_mut_ptr(), count as u32)? };
            rects.extend(raw.iter().map(|r| UpdateRect::from_abi(*r)));
            cap_scratch(&mut raw);
        }
        (self.on_updates)(&rects);
        cap_scratch(&mut rects);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SCRATCH_HEADROOM, UpdateRect, bindings, cap_scratch};

    #[test]
    fn update_rect_roundtrips_through_abi() {
        let r = UpdateRect {
            x: 3,
            y: 7,
            width: 20,
            height: 5,
        };
        let abi = r.to_abi();
        // `UpdateRect` is x/y/width/height; the ABI `RECT` is left/top/right/bottom.
        assert_eq!((abi.left, abi.top, abi.right, abi.bottom), (3, 7, 23, 12));
        assert_eq!(UpdateRect::from_abi(abi), r);
    }

    #[test]
    fn update_rect_from_abi_derives_width_and_height() {
        let abi = bindings::RECT {
            left: 10,
            top: 20,
            right: 35,
            bottom: 26,
        };
        assert_eq!(
            UpdateRect::from_abi(abi),
            UpdateRect {
                x: 10,
                y: 20,
                width: 25,
                height: 6,
            }
        );
    }

    #[test]
    fn cap_scratch_preserves_steady_state_capacity() {
        // A buffer whose capacity is close to its length is left alone — no realloc
        // churn on the common small-batch path.
        let mut v: Vec<u8> = Vec::with_capacity(SCRATCH_HEADROOM + 4);
        v.resize(4, 0);
        let before = v.capacity();
        cap_scratch(&mut v);
        assert_eq!(v.capacity(), before);
    }

    #[test]
    fn cap_scratch_reclaims_after_a_spike() {
        // One-off large batch then back to small: the excess capacity is released,
        // but never below the live length or the retained headroom.
        let mut v: Vec<u8> = Vec::with_capacity(10_000);
        v.resize(4, 0);
        cap_scratch(&mut v);
        assert!(v.capacity() < 10_000, "excess reclaimed");
        assert!(v.capacity() >= v.len(), "never below live length");
        assert!(v.capacity() >= SCRATCH_HEADROOM, "headroom retained");
    }

    #[test]
    fn cap_scratch_never_shrinks_below_length() {
        let mut v: Vec<u8> = Vec::with_capacity(10_000);
        v.resize(500, 0);
        cap_scratch(&mut v);
        assert!(v.capacity() >= 500);
    }
}
