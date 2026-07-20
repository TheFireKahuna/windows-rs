//! Off-tree composition surfaces: Direct2D content drawn into a
//! `CompositionDrawingSurface` and shown through a `SpriteVisual` parented under an
//! arbitrary `ContainerVisual` in the system (`Windows.UI.Composition`) tree — the
//! self-hosted DirectComposition backend's compositor.
//!
//! A child-visual composition surface lives in the *composition* tree: redrawing it
//! never invalidates layout or the render walk, and it consumes **no swap chain**.
//! One [`CompositionSurfaceFactory`] (backed by a single Direct2D device, of which
//! there is naturally one compositor per UI thread) mints many
//! [`surfaces`](CompositionSurfaceFactory::create_under), so an arbitrary number of
//! live surfaces costs one graphics device and zero swap chains.
//!
//! With a multi-threaded Direct2D device the returned [`CompositionDrawSurface`]
//! is `Send`, so a worker thread can draw the content while the UI thread runs.
//! One rule governs that concurrency: a `CompositionGraphicsDevice` admits only
//! **one outstanding [`begin_draw`](CompositionDrawSurface::begin_draw)** across
//! all of its surfaces — a second concurrent `BeginDraw` on any surface of the
//! same graphics device *fails* (`0x80131509`), it does not block. So surfaces
//! drawn by different threads must come from different factories (one factory =
//! one graphics device); within one factory the owner's own sequential
//! bracketing of `begin_draw`/`end_draw` is the serialization, and no extra
//! cross-thread lock is needed. Crucially, none is *held across* the surface's
//! `EndDraw` either: an `ID2D1Multithread` lock held there would invert against the
//! UI thread's compositor commit (which takes the composition lock, then the D2D
//! lock) and deadlock. The visual tree itself (create / size / attach / detach)
//! stays on the UI thread, where the compositor lives.

use super::*;
use crate::system_bindings as sys;

/// Mints composition drawing surfaces backed by one Direct2D device, each shown as
/// a child visual under a system `ContainerVisual`. Build once — ideally app-wide,
/// since there is a single compositor per UI thread — and reuse it for every
/// surface; `N` live surfaces then share this one graphics device and cost zero
/// swap chains.
///
/// Pass a **multi-threaded** Direct2D device to [`from_compositor`](Self::from_compositor)
/// to draw the surfaces off the UI thread (the returned [`CompositionDrawSurface`]
/// is then `Send`). Build it on the UI thread.
pub struct CompositionSurfaceFactory {
    compositor: sys::Compositor,
    graphics: sys::CompositionGraphicsDevice,
}

impl CompositionSurfaceFactory {
    /// Create the factory from a system `Windows.UI.Composition.Compositor` (the
    /// self-hosted DirectComposition backend's compositor). Pair with
    /// [`create_under`](Self::create_under) to host surfaces under a backend node's
    /// `ContainerVisual`. `d2d_device` is an `ID2D1Device` (multi-threaded to draw
    /// off the UI thread).
    pub fn from_compositor(
        compositor: &sys::Compositor,
        d2d_device: &impl Interface,
    ) -> Result<Self> {
        let interop: sys::ICompositorInterop = compositor.cast()?;
        let device: windows_core::IUnknown = d2d_device.cast()?;
        let graphics: sys::CompositionGraphicsDevice =
            unsafe { interop.CreateGraphicsDevice(&device)? }.cast()?;
        Ok(Self {
            compositor: compositor.clone(),
            graphics,
        })
    }

    /// Build a factory from a backend **node's container visual** — the
    /// `IInspectable` returned by
    /// [`ElementHandle::native`](crate::ElementHandle::native) (or
    /// `get_native_element`). The system `Compositor` is derived from the visual, so
    /// the whole live-viz hand-off is driven from that one public handle. Pair with
    /// [`create_under_node`](Self::create_under_node). `d2d_device` is an
    /// `ID2D1Device` (multi-threaded to draw off the UI thread).
    pub fn from_node(
        container: &windows_core::IInspectable,
        d2d_device: &impl Interface,
    ) -> Result<Self> {
        let cv: sys::ContainerVisual = container.cast()?;
        let compositor = cv.cast::<sys::ICompositionObject>()?.Compositor()?;
        Self::from_compositor(&compositor, d2d_device)
    }

    /// Host a live surface under a backend node's container visual, given the node's
    /// `IInspectable` (from [`ElementHandle::native`](crate::ElementHandle::native)).
    /// The `IInspectable`-typed analogue of [`create_under`](Self::create_under).
    pub fn create_under_node(
        &self,
        container: &windows_core::IInspectable,
        pixel_size: (i32, i32),
        dip_size: (f32, f32),
        opaque: bool,
    ) -> Result<(CompositionChildVisual, CompositionDrawSurface)> {
        let cv: sys::ContainerVisual = container.cast()?;
        self.create_under(&cv, pixel_size, dip_size, opaque)
    }

    /// Create an FP16 surface `pixel_size` pixels large, presented at `dip_size`
    /// DIPs, and parent its sprite **at the top** of `parent`'s child collection.
    ///
    /// Returns a [`CompositionChildVisual`] (drop removes the sprite from `parent`)
    /// and a [`CompositionDrawSurface`] that draws the content (move it to a worker
    /// thread when the factory's device is multi-threaded). The surface resizes in
    /// place: call [`CompositionDrawSurface::resize`] on the drawing side to grow the
    /// backing and [`CompositionChildVisual::set_dip_size`] on the hosting side to
    /// grow the presented sprite — no need to drop and recreate.
    pub fn create_under(
        &self,
        parent: &sys::ContainerVisual,
        pixel_size: (i32, i32),
        dip_size: (f32, f32),
        opaque: bool,
    ) -> Result<(CompositionChildVisual, CompositionDrawSurface)> {
        let graphics2: sys::ICompositionGraphicsDevice2 = self.graphics.cast()?;
        let alpha = if opaque {
            sys::DirectXAlphaMode::Ignore
        } else {
            sys::DirectXAlphaMode::Premultiplied
        };
        // FP16 scRGB (`R16G16B16A16Float`), matching the backend's HDR composition
        // pipeline (the node-chrome surfaces and the whole-window FP16 path) so a
        // meter/accent can author values past 1.0 and pop. The system compositor
        // presents this surface as scRGB-*linear*; the viz draw closures author
        // colours in sRGB, so the surface is flagged linear below and the draw
        // session (see `CompositionDrawTarget`) gamma-decodes every colour onto it —
        // a near-black #1c1c1c backdrop lands near-black, not the mid-grey that writing
        // sRGB values raw onto a linear surface would produce.
        let surface = graphics2.CreateDrawingSurface2(
            sys::SizeInt32 { width: pixel_size.0.max(1), height: pixel_size.1.max(1) },
            sys::DirectXPixelFormat::R16G16B16A16Float,
            alpha,
        )?;

        let brush = self
            .compositor
            .CreateSurfaceBrushWithSurface(&surface.cast::<sys::ICompositionSurface>()?)?;
        brush.SetStretch(sys::CompositionStretch::Fill)?;
        // Live-viz surfaces are painted by the app through its own draw-time colour
        // map (`DrawKit`), so the surface brush is used raw — no compositor effect.
        let content = brush.cast::<sys::CompositionBrush>()?;
        let sprite = self.compositor.CreateSpriteVisual()?;
        sprite
            .cast::<sys::IVisual>()?
            .SetSize(windows_numerics::Vector2 { x: dip_size.0, y: dip_size.1 })?;
        sprite.SetBrush(&content)?;

        let visual: sys::Visual = sprite.cast()?;
        parent.Children()?.InsertAtTop(&visual)?;

        let draw = CompositionDrawSurface {
            interop: surface.cast::<sys::ICompositionDrawingSurfaceInterop>()?,
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
/// sprite** from the parent. Not `Send` — the visual tree belongs to the UI thread.
pub struct CompositionChildVisual {
    parent: sys::ContainerVisual,
    // Held so the visual (and its brush + surface) outlives this handle even if
    // the draw side drops first; also the thing we detach on drop.
    visual: sys::Visual,
}

impl CompositionChildVisual {
    /// Resize the presented sprite to `dip` DIPs, in place. Paired with
    /// [`CompositionDrawSurface::resize`] (which grows the backing on the drawing
    /// side): the brush stretches its surface to the sprite, so both must move for a
    /// crisp result, but neither requires re-parenting or recreating the visual.
    /// Runs on the UI thread, which owns the visual tree.
    pub fn set_dip_size(&self, dip: (f32, f32)) {
        if let Ok(v) = self.visual.cast::<sys::IVisual>() {
            let _ = v.SetSize(windows_numerics::Vector2 { x: dip.0, y: dip.1 });
        }
    }
}

impl Drop for CompositionChildVisual {
    fn drop(&mut self) {
        if let Ok(children) = self.parent.Children() {
            let _ = children.Remove(&self.visual);
        }
    }
}

/// The drawing side of a composition surface: brackets each frame between
/// [`begin_draw`](Self::begin_draw) and [`end_draw`](Self::end_draw). `Send` when
/// the factory's device is multi-threaded, so it can be moved to a worker thread;
/// the factory lock then serializes its DXGI interop against the device.
pub struct CompositionDrawSurface {
    interop: sys::ICompositionDrawingSurfaceInterop,
}

impl CompositionDrawSurface {
    /// Whether the backing surface stores linear scRGB. Always true here — the
    /// system-composited viz surfaces are FP16 `R16G16B16A16Float`. A draw path uses
    /// this to enable sRGB→linear color conversion so sRGB-authored content renders
    /// correctly on the FP16 surface.
    pub fn is_linear(&self) -> bool {
        true
    }
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
        unsafe {
            // Null update rect = redraw the whole surface (we repaint every frame).
            let mut offset = sys::POINT::default();
            let object: T = self.interop.BeginDraw::<T>(None, &mut offset)?;
            Ok((object, (offset.x, offset.y)))
        }
    }

    /// Finish drawing and commit the surface contents to the compositor.
    pub fn end_draw(&self) -> Result<()> {
        unsafe { self.interop.EndDraw().ok() }
    }

    /// Resize the backing surface in place to `pixel` physical pixels, keeping the
    /// same visual, brush and atlas — no re-parenting and no fresh
    /// [`create_under`](CompositionSurfaceFactory::create_under). The next
    /// [`begin_draw`](Self::begin_draw) fills the new extent. Pair with
    /// [`CompositionChildVisual::set_dip_size`] so the presented sprite matches.
    /// `ICompositionDrawingSurfaceInterop::Resize` is internally synchronized, so
    /// this is safe from the drawing thread between frames.
    pub fn resize(&self, pixel: (i32, i32)) -> Result<()> {
        unsafe {
            self.interop
                .Resize(sys::SIZE { cx: pixel.0.max(1), cy: pixel.1.max(1) })
                .ok()
        }
    }
}
