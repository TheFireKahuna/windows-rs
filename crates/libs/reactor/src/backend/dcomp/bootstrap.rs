//! Compositor + surface bootstrap: a system `Windows.UI.Composition.Compositor`
//! rooted on the bare HWND via `CreateDesktopWindowTarget`, a shared multi-threaded
//! canvas [`GpuDevice`] exposed to the compositor as a `CompositionGraphicsDevice`,
//! and ONE whole-window FP16 (`R16G16B16A16Float`) `CompositionDrawingSurface`
//! shown through a `SpriteVisual`. The system compositor composites scRGB FP16
//! straight to DWM without clamping — whole-window HDR comes free.

use std::cell::Cell;

use crate::system_bindings::{
    CompositionBrush, CompositionDrawingSurface, CompositionGraphicsDevice, CompositionStretch,
    CompositionSurfaceBrush, Compositor, ContainerVisual, DesktopWindowTarget, DirectXAlphaMode,
    DirectXPixelFormat, ICompositionDrawingSurfaceInterop, ICompositionSurface, ICompositionTarget,
    ICompositorDesktopInterop, ICompositorInterop, IVisual, IVisualCollection, Size, SpriteVisual,
    Visual, HWND,
};
use windows_canvas_core::GpuDevice;
use windows_core::Interface;
use windows_numerics::Vector2;

/// The drawable half of the surface — what `paint` needs each frame.
pub(crate) struct Surface {
    pub interop: ICompositionDrawingSurfaceInterop,
    pub device_lost: Cell<bool>,
    // Kept alive so the surface + its brush outlive the interop handle.
    _surface: CompositionDrawingSurface,
    _brush: CompositionSurfaceBrush,
}

/// All composition state for the window. Owns the shared GpuDevice and the single
/// whole-window FP16 surface; recreate the surface on resize.
pub(crate) struct Compositing {
    // Held so the shared D3D/D2D device outlives the graphics device and surfaces.
    #[allow(dead_code)]
    pub gpu: GpuDevice,
    pub surface: Surface,
    pub pixel_size: (i32, i32),
    compositor: Compositor,
    graphics: CompositionGraphicsDevice,
    sprite: SpriteVisual,
    _target: DesktopWindowTarget,
    _root: ContainerVisual,
}

impl Compositing {
    pub fn new(hwnd: HWND, pixel_w: i32, pixel_h: i32) -> windows_core::Result<Self> {
        let gpu = GpuDevice::new_multi_threaded()?;

        let compositor = Compositor::new()?;
        let interop: ICompositorInterop = compositor.cast()?;
        let graphics: CompositionGraphicsDevice =
            unsafe { interop.CreateGraphicsDevice(gpu.d2d_device())? };

        let desktop: ICompositorDesktopInterop = compositor.cast()?;
        let target: DesktopWindowTarget = unsafe { desktop.CreateDesktopWindowTarget(hwnd, false)? };
        let root = compositor.CreateContainerVisual()?;
        target.cast::<ICompositionTarget>()?.SetRoot(&root)?;

        let sprite = compositor.CreateSpriteVisual()?;
        root.Children()?
            .cast::<IVisualCollection>()?
            .InsertAtTop(&sprite.cast::<Visual>()?)?;

        let surface = make_surface(&compositor, &graphics, &sprite, pixel_w, pixel_h)?;

        Ok(Self {
            gpu,
            surface,
            pixel_size: (pixel_w.max(1), pixel_h.max(1)),
            compositor,
            graphics,
            sprite,
            _target: target,
            _root: root,
        })
    }

    /// Recreate the FP16 surface and brush at a new pixel size.
    pub fn resize(&mut self, pixel_w: i32, pixel_h: i32) -> windows_core::Result<()> {
        if (pixel_w.max(1), pixel_h.max(1)) == self.pixel_size {
            return Ok(());
        }
        self.surface = make_surface(
            &self.compositor,
            &self.graphics,
            &self.sprite,
            pixel_w,
            pixel_h,
        )?;
        self.pixel_size = (pixel_w.max(1), pixel_h.max(1));
        Ok(())
    }
}

fn make_surface(
    compositor: &Compositor,
    graphics: &CompositionGraphicsDevice,
    sprite: &SpriteVisual,
    pixel_w: i32,
    pixel_h: i32,
) -> windows_core::Result<Surface> {
    let surface = graphics.CreateDrawingSurface(
        Size {
            width: pixel_w.max(1) as f32,
            height: pixel_h.max(1) as f32,
        },
        DirectXPixelFormat::R16G16B16A16Float,
        DirectXAlphaMode::Premultiplied,
    )?;

    let brush =
        compositor.CreateSurfaceBrushWithSurface(&surface.cast::<ICompositionSurface>()?)?;
    let _ = brush.SetStretch(CompositionStretch::Fill);

    let v: IVisual = sprite.cast()?;
    v.SetSize(Vector2::new(pixel_w.max(1) as f32, pixel_h.max(1) as f32))?;
    sprite.SetBrush(&brush.cast::<CompositionBrush>()?)?;

    let interop: ICompositionDrawingSurfaceInterop = surface.cast()?;
    Ok(Surface {
        interop,
        device_lost: Cell::new(false),
        _surface: surface,
        _brush: brush,
    })
}
