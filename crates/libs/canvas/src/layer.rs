use super::*;
use std::cell::Cell;

/// An off-screen layer renderer on its **own** Direct2D device context.
///
/// Rendering a cached layer through a live surface's context means retargeting that
/// context (`with_target`), and Direct2D has to resolve the batched work against the
/// current target before it can switch. On a **presented composition surface** that
/// mid-bracket resolve publishes whatever has been drawn so far as a complete frame,
/// which the compositor is then free to sample — a half-drawn frame on screen.
///
/// This owns a second context on the same device, so a layer — including effects that
/// need a pass of their own, such as a Gaussian glow — renders without ever touching
/// the live surface's `BeginDraw`/`EndDraw` bracket. Direct2D resources are **device**
/// scoped rather than context scoped, so the bitmap this hands back composites
/// straight onto the live surface.
pub struct LayerRenderer {
    context: ID2D1DeviceContext,
    /// Device-loss flag for the owned `BeginDraw`/`EndDraw` bracket below.
    device_lost: Cell<bool>,
}

impl LayerRenderer {
    /// Create a renderer on a second context of `device`.
    ///
    /// Direct2D calls are serialized by a multi-threaded factory, so this needs no
    /// external lock (that guard exists for direct D3D/DXGI calls, which this makes
    /// none of).
    pub fn new(device: &GpuDevice) -> Result<Self> {
        let context = unsafe {
            device
                .d2d_device()
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
        };
        Ok(Self {
            context,
            device_lost: Cell::new(false),
        })
    }

    /// Render `f` into a fresh `size_px` (physical pixels) bitmap and return it.
    ///
    /// `scale` is the DIP→pixel factor: `f` draws in DIPs exactly as it would on the
    /// live surface, and the result holds already-scaled pixels — which is what the
    /// 1:1 blit that composites it expects. `linear` selects FP16 scRGB (matching a
    /// composition surface) over 8-bit sRGB. The bitmap is stamped at 96 DPI so one
    /// unit is one pixel and the scale lives purely in the transform, mirroring how
    /// the live surface is set up.
    pub fn render(
        &self,
        size_px: (u32, u32),
        scale: f32,
        linear: bool,
        f: impl FnOnce(&DrawingSession<'_>),
    ) -> Result<Bitmap> {
        unsafe {
            let format = if linear {
                DXGI_FORMAT_R16G16B16A16_FLOAT
            } else {
                DXGI_FORMAT_B8G8R8A8_UNORM
            };
            let properties = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };
            let size = D2D_SIZE_U {
                width: size_px.0.max(1),
                height: size_px.1.max(1),
            };
            let bitmap = self.context.CreateBitmap(size, None, 0, &properties)?;

            self.context.SetDpi(96.0, 96.0);
            self.context.SetTarget(&bitmap);
            // Scoped so the session's `Drop` ends the draw (and reports device loss)
            // before the target is released below.
            {
                let session = DrawingSession::new(&self.context, &self.device_lost)?
                    .encode_srgb_target(!linear);
                session.set_transform(&Matrix3x2 {
                    m11: scale,
                    m12: 0.0,
                    m21: 0.0,
                    m22: scale,
                    m31: 0.0,
                    m32: 0.0,
                });
                session.clear(ColorF::TRANSPARENT);
                f(&session);
            }
            self.context.SetTarget(None::<&ID2D1Image>);

            if self.device_lost.get() {
                self.device_lost.set(false);
                return Err(Error::from(DXGI_ERROR_DEVICE_REMOVED));
            }
            Ok(Bitmap(bitmap))
        }
    }
}
