//! Off-XAML-tree composition surfaces: Direct2D content drawn into a
//! `CompositionDrawingSurface` and shown through a `SpriteVisual` set as a XAML
//! element's child visual (`ElementCompositionPreview::SetElementChildVisual`).
//!
//! Unlike a [`SurfaceImageSource`](super::SurfaceImageSource) — a XAML
//! `ImageSource` whose updates go through the XAML render walk — a child-visual
//! composition surface lives in the *composition* tree: redrawing it never
//! invalidates XAML layout or render, and it consumes **no swap chain**. One
//! [`CompositionSurfaceFactory`] (backed by a single Direct2D device, of which
//! there is naturally one compositor per UI thread) mints many
//! [`surfaces`](CompositionSurfaceFactory::create), so an arbitrary number of
//! live surfaces costs one graphics device and zero swap chains.
//!
//! With a multi-threaded Direct2D device the returned [`CompositionDrawSurface`]
//! is `Send`, so a worker thread can draw the content while the UI thread runs.
//! [`begin_draw`](CompositionDrawSurface::begin_draw) /
//! [`end_draw`](CompositionDrawSurface::end_draw) are synchronized by
//! DirectComposition internally (its own `ContextSession` lock), and the shared
//! device is multi-threaded (Direct2D serializes each call), so no extra
//! cross-thread lock is needed. Crucially, none is *held across* the surface's
//! `EndDraw` either: an `ID2D1Multithread` lock held there would invert against the
//! UI thread's compositor commit (which takes the composition lock, then the D2D
//! lock) and deadlock. The visual tree itself (create / size / attach / detach)
//! stays on the UI thread, where the compositor lives.

use super::*;

/// Mints composition drawing surfaces backed by one Direct2D device, each shown as
/// a child visual of a XAML element. Build once — ideally app-wide, since there is
/// a single compositor per UI thread — and reuse it for every surface; `N` live
/// surfaces then share this one graphics device and cost zero swap chains.
///
/// Pass a **multi-threaded** Direct2D device to [`new`](Self::new) to draw the
/// surfaces off the UI thread (the returned [`CompositionDrawSurface`] is then
/// `Send`). Build it on the UI thread.
pub struct CompositionSurfaceFactory {
    backing: Backing,
}

/// Which compositor namespace a factory is bound to. The lifted
/// `Microsoft.UI.Composition` path hosts surfaces as XAML element child visuals
/// (the WinUI backend); the system `Windows.UI.Composition` path hosts them as
/// child visuals under an arbitrary `ContainerVisual` (the self-hosted
/// DirectComposition backend). Both mint surfaces from one Direct2D device.
enum Backing {
    Lifted {
        compositor: bindings::Compositor,
        graphics: bindings::CompositionGraphicsDevice,
    },
    System {
        compositor: system_bindings::Compositor,
        graphics: system_bindings::CompositionGraphicsDevice,
    },
}

impl CompositionSurfaceFactory {
    /// Create the factory on the UI thread. `element` supplies the per-thread
    /// (lifted) compositor (any live element's `Visual` carries it); `d2d_device`
    /// is an `ID2D1Device` — pass a multi-threaded one (its factory created with
    /// `D2D1_FACTORY_TYPE_MULTI_THREADED`) to draw the surfaces off the UI thread.
    pub fn new(element: &ElementHandle, d2d_device: &impl Interface) -> Result<Self> {
        let ui: bindings::UIElement = element.0.cast()?;
        let visual = bindings::ElementCompositionPreview::GetElementVisual(&ui)?;
        let compositor = visual.cast::<bindings::ICompositionObject>()?.Compositor()?;
        // The lifted ICompositorInterop carries only CreateGraphicsDevice.
        let interop: bindings::ICompositorInterop = compositor.cast()?;
        let mut graphics_raw = core::ptr::null_mut();
        let graphics = unsafe {
            interop.CreateGraphicsDevice(d2d_device.as_raw(), &mut graphics_raw)?;
            bindings::CompositionGraphicsDevice::from_raw(graphics_raw)
        };
        Ok(Self {
            backing: Backing::Lifted { compositor, graphics },
        })
    }

    /// Create the factory from a **system** `Windows.UI.Composition.Compositor`
    /// (the self-hosted DirectComposition backend's compositor), skipping the
    /// XAML `ElementCompositionPreview` path. Pair with
    /// [`create_under`](Self::create_under) to host surfaces under a backend
    /// node's `ContainerVisual`. `d2d_device` is an `ID2D1Device` (multi-threaded
    /// to draw off the UI thread).
    pub fn from_compositor(
        compositor: &system_bindings::Compositor,
        d2d_device: &impl Interface,
    ) -> Result<Self> {
        let interop: system_bindings::ICompositorInterop = compositor.cast()?;
        let device: windows_core::IUnknown = d2d_device.cast()?;
        let graphics = unsafe { interop.CreateGraphicsDevice(&device)? };
        Ok(Self {
            backing: Backing::System {
                compositor: compositor.clone(),
                graphics,
            },
        })
    }

    /// Create a surface `pixel_size` pixels large, presented at `dip_size` DIPs, and
    /// attach it as `element`'s child visual. `opaque` skips alpha blending (clear
    /// every pixel each frame); the brush fills the visual, so the pixel buffer
    /// should be the DIP size times the current rasterization scale to stay crisp.
    ///
    /// Returns a UI-thread [`CompositionChildSurface`] (drop detaches the visual)
    /// and a [`CompositionDrawSurface`] that draws the content (move it to a worker
    /// thread when the factory's device is multi-threaded). The pixel size is fixed
    /// for the surface's lifetime — drop both and call `create` again to resize.
    pub fn create(
        &self,
        element: &ElementHandle,
        pixel_size: (i32, i32),
        dip_size: (f32, f32),
        opaque: bool,
    ) -> Result<(CompositionChildSurface, CompositionDrawSurface)> {
        let Backing::Lifted { compositor, graphics } = &self.backing else {
            // `create` hosts the surface as a XAML element child visual; a
            // system-backed factory must use `create_under` instead.
            return Err(Error::empty());
        };
        let ui: bindings::UIElement = element.0.cast()?;
        let graphics: bindings::ICompositionGraphicsDevice2 = graphics.cast()?;
        let alpha = if opaque {
            bindings::DirectXAlphaMode::Ignore
        } else {
            bindings::DirectXAlphaMode::Premultiplied
        };
        let surface = graphics.CreateDrawingSurface2(
            bindings::SizeInt32 { width: pixel_size.0.max(1), height: pixel_size.1.max(1) },
            bindings::DirectXPixelFormat::B8G8R8A8UIntNormalized,
            alpha,
        )?;

        // Sprite visual filled by a surface brush: surface pixels stretch to the
        // visual's DIP size (crisp when pixels == DIPs x rasterization scale).
        let brush =
            compositor.CreateSurfaceBrushWithSurface(&surface.cast::<bindings::ICompositionSurface>()?)?;
        brush.SetStretch(bindings::CompositionStretch::Fill)?;
        let sprite = compositor.CreateSpriteVisual()?;
        sprite
            .cast::<bindings::IVisual>()?
            .SetSize(windows_numerics::Vector2 { x: dip_size.0, y: dip_size.1 })?;
        sprite.SetBrush(&brush.cast::<bindings::CompositionBrush>()?)?;

        let visual: bindings::Visual = sprite.cast()?;
        bindings::ElementCompositionPreview::SetElementChildVisual(&ui, &visual)?;

        let draw = CompositionDrawSurface { interop: surface.cast()? };
        Ok((CompositionChildSurface { element: ui, _visual: visual }, draw))
    }

    /// Create an FP16 surface `pixel_size` pixels large, presented at `dip_size`
    /// DIPs, and parent its sprite **at the top** of `parent`'s child collection
    /// — the system-compositor analogue of [`create`](Self::create), used to host
    /// live viz under a DirectComposition backend node's `ContainerVisual`. The
    /// factory must have been built with [`from_compositor`](Self::from_compositor).
    ///
    /// Returns a [`CompositionChildVisual`] (drop removes the sprite from
    /// `parent`) and a [`CompositionDrawSurface`] that draws the content.
    pub fn create_under(
        &self,
        parent: &system_bindings::ContainerVisual,
        pixel_size: (i32, i32),
        dip_size: (f32, f32),
        opaque: bool,
    ) -> Result<(CompositionChildVisual, CompositionDrawSurface)> {
        use crate::system_bindings as sys;
        let Backing::System { compositor, graphics } = &self.backing else {
            // `create_under` parents under a system ContainerVisual; a lifted
            // (XAML) factory must use `create` instead.
            return Err(Error::empty());
        };
        let graphics2: sys::ICompositionGraphicsDevice2 = graphics.cast()?;
        let alpha = if opaque {
            sys::DirectXAlphaMode::Ignore
        } else {
            sys::DirectXAlphaMode::Premultiplied
        };
        // FP16 scRGB to match the backend's HDR composition pipeline.
        let surface = graphics2.CreateDrawingSurface2(
            sys::SizeInt32 { width: pixel_size.0.max(1), height: pixel_size.1.max(1) },
            sys::DirectXPixelFormat::R16G16B16A16Float,
            alpha,
        )?;

        let brush = compositor
            .CreateSurfaceBrushWithSurface(&surface.cast::<sys::ICompositionSurface>()?)?;
        brush.SetStretch(sys::CompositionStretch::Fill)?;
        let sprite = compositor.CreateSpriteVisual()?;
        sprite
            .cast::<sys::IVisual>()?
            .SetSize(windows_numerics::Vector2 { x: dip_size.0, y: dip_size.1 })?;
        sprite.SetBrush(&brush.cast::<sys::CompositionBrush>()?)?;

        let visual: sys::Visual = sprite.cast()?;
        parent.Children()?.InsertAtTop(&visual)?;

        // The interop IID is namespace-agnostic, so the system surface reuses the
        // same `CompositionDrawSurface` the canvas draw stack already consumes.
        let draw = CompositionDrawSurface {
            interop: surface.cast::<bindings::ICompositionDrawingSurfaceInterop>()?,
        };
        Ok((
            CompositionChildVisual {
                parent: parent.clone(),
                visual,
            },
            draw,
        ))
    }
}

/// The UI-thread side of a system-compositor child-visual surface: keeps the
/// sprite parented under its host `ContainerVisual`. **Dropping it removes the
/// sprite** from the parent. Not `Send` — the visual tree belongs to the UI
/// thread. The system analogue of [`CompositionChildSurface`].
pub struct CompositionChildVisual {
    parent: system_bindings::ContainerVisual,
    // Held so the visual (and its brush + surface) outlives this handle even if
    // the draw side drops first; also the thing we detach on drop.
    visual: system_bindings::Visual,
}

impl Drop for CompositionChildVisual {
    fn drop(&mut self) {
        if let Ok(children) = self.parent.Children() {
            let _ = children.Remove(&self.visual);
        }
    }
}

/// The UI-thread side of a composition surface: keeps the child visual attached to
/// its host element for as long as it lives. **Dropping it detaches the visual**
/// (the element shows its own XAML content again), so store it for the surface's
/// lifetime. Not `Send` — the visual tree belongs to the UI thread.
pub struct CompositionChildSurface {
    element: bindings::UIElement,
    // Held so the visual (and, through it, the brush and surface) outlives this
    // handle even if the draw side is dropped first; also the thing we detach.
    _visual: bindings::Visual,
}

impl Drop for CompositionChildSurface {
    fn drop(&mut self) {
        // Detach our child visual, restoring the element's own composition content.
        let _ = bindings::ElementCompositionPreview::SetElementChildVisual(
            &self.element,
            None::<&bindings::Visual>,
        );
    }
}

/// The drawing side of a composition surface: brackets each frame between
/// [`begin_draw`](Self::begin_draw) and [`end_draw`](Self::end_draw). `Send` when
/// the factory's device is multi-threaded, so it can be moved to a worker thread;
/// the factory lock then serializes its DXGI interop against the device.
pub struct CompositionDrawSurface {
    interop: bindings::ICompositionDrawingSurfaceInterop,
}

// SAFETY: the surface's drawing interop is used from one thread at a time (the worker
// that owns this handle). `ICompositionDrawingSurfaceInterop::BeginDraw`/`EndDraw` are
// internally synchronized by DirectComposition, and the backing Direct2D device is
// multi-threaded (each device call serializes internally), so a worker thread can draw
// while the UI thread runs. The caller passes a multi-threaded device to opt into this.
unsafe impl Send for CompositionDrawSurface {}

impl CompositionDrawSurface {
    /// Begin drawing the whole surface, returning the drawing target `T` (typically
    /// `ID2D1DeviceContext`) and the `(x, y)` pixel offset within the backing atlas
    /// to translate drawing by. Pair with [`end_draw`](Self::end_draw).
    pub fn begin_draw<T: Interface>(&self) -> Result<(T, (i32, i32))> {
        let mut offset = bindings::POINT::default();
        let mut object = core::ptr::null_mut();
        unsafe {
            // Null update rect = redraw the whole surface (we repaint every frame).
            self.interop
                .BeginDraw(core::ptr::null(), &T::IID, &mut object, &mut offset)?;
            Ok((T::from_raw(object), (offset.x, offset.y)))
        }
    }

    /// Finish drawing and commit the surface contents to the compositor.
    pub fn end_draw(&self) -> Result<()> {
        unsafe { self.interop.EndDraw() }
    }
}
