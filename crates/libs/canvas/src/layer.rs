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
    /// The bitmap the last `render` handed back, kept — with the `(size, format)` it
    /// was made for — so the next call can draw into it again. See `render`.
    bitmap: Cell<Option<(D2D_SIZE_U, DXGI_FORMAT, ID2D1Bitmap1)>>,
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
            bitmap: Cell::new(None),
        })
    }

    /// Render `f` into a `size_px` (physical pixels) bitmap and return it.
    ///
    /// `scale` is the DIP→pixel factor: `f` draws in DIPs exactly as it would on the
    /// live surface, and the result holds already-scaled pixels — which is what the
    /// 1:1 blit that composites it expects. `linear` selects FP16 scRGB (matching a
    /// composition surface) over 8-bit sRGB. The bitmap is stamped at 96 DPI so one
    /// unit is one pixel and the scale lives purely in the transform, mirroring how
    /// the live surface is set up.
    ///
    /// The bitmap handed back may be the very one an earlier call returned, redrawn —
    /// but only once you have dropped that one, so a layer you keep stays untouched.
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
            // A render-target bitmap is a D3D texture, and creating — then destroying
            // — one per call is real driver-side work on a path that runs once per
            // layer per frame. Draw into the last one again when it was made for this
            // same `(size, format)` and nothing else still holds it. A caller that
            // *caches* its layer keeps a reference to the bitmap it was handed, so its
            // pixels can never be overwritten by a later render: that call finds a
            // shared bitmap and creates a new one, exactly as before. The draw below
            // clears the target, so a reused bitmap starts as blank as a fresh one.
            let bitmap = match self.bitmap.take() {
                Some((cached_size, cached_format, bitmap))
                    if (cached_size, cached_format) == (size, format) && is_sole_ref(&bitmap) =>
                {
                    bitmap
                }
                _ => self.context.CreateBitmap(size, None, 0, &properties)?,
            };

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
            // Kept for the next call to draw into again. Nothing is kept from a failed
            // or device-lost render — the `take` above already dropped what there was —
            // so a bitmap belonging to a dead device is never handed back.
            self.bitmap.set(Some((size, format, bitmap.clone())));
            Ok(Bitmap(bitmap))
        }
    }
}

/// `true` if nothing besides `bitmap` itself holds a reference to the underlying COM
/// object, sampled by an `AddRef`/`Release` pair — `Release` returns the count that
/// survives it. Direct2D takes a reference of its own while a bitmap sits in an
/// unflushed batch, so this reads as *shared* whenever there is any doubt.
fn is_sole_ref(bitmap: &ID2D1Bitmap1) -> bool {
    let unknown: &IUnknown = bitmap.into();
    unsafe {
        (Interface::vtable(unknown).AddRef)(Interface::as_raw(unknown));
        (Interface::vtable(unknown).Release)(Interface::as_raw(unknown)) == 1
    }
}
