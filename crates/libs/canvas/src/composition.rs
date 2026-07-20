//! Drawing into an off-XAML-tree composition surface.
//!
//! [`windows_reactor::CompositionSurfaceFactory`] mints a `CompositionDrawSurface`
//! (the drawing half of a child-visual composition surface). This wraps one as a
//! [`CompositionDrawTarget`] and brackets each frame: it adopts the Direct2D
//! context handed back by the surface's native `BeginDraw` as a [`DrawingSession`]
//! and pre-applies the transform that maps DIP drawing onto the surface's pixels —
//! so callers draw exactly as they would a [`SwapChain`](crate::SwapChain) frame,
//! but the result is presented through the compositor (no XAML render walk, no
//! swap chain consumed).
//!
//! When the factory's device is multi-threaded the wrapped surface is `Send`, so a
//! `CompositionDrawTarget` can live on a worker thread and draw while the UI thread
//! runs — its `BeginDraw`/`EndDraw` serialize against the device through the
//! Direct2D factory lock the surface carries.

use super::*;
use std::cell::Cell;
use windows_reactor::CompositionDrawSurface;

/// Draws frames into a composition child-visual surface. Construct from a
/// [`CompositionDrawSurface`] obtained via
/// [`windows_reactor::CompositionSurfaceFactory::create`], then call
/// [`draw`](Self::draw) per frame. Call [`resize`](Self::resize) to grow the backing
/// in place — no need to recreate the surface or this target.
pub struct CompositionDrawTarget {
    surface: CompositionDrawSurface,
    // Set by an adopted [`DrawingSession`] when a draw call reports device loss, so
    // [`draw`](Self::draw) can surface it to the caller to trigger a rebuild.
    device_lost: Cell<bool>,
}

impl CompositionDrawTarget {
    /// Wrap a surface's drawing handle. Inherits the surface's `Send`-ness, so on a
    /// multi-threaded device this can be moved to a worker render thread.
    pub fn new(surface: CompositionDrawSurface) -> Self {
        Self { surface, device_lost: Cell::new(false) }
    }

    /// Draw one frame. Brackets the surface's native `BeginDraw`/`EndDraw`, adopts
    /// the returned Direct2D context as a [`DrawingSession`], and sets its transform
    /// so coordinates in DIPs land at `dip * scale + atlas_offset` pixels — pass the
    /// surface's rasterization `scale` (pixel size / DIP size). `f` issues the draw
    /// calls in DIP space.
    ///
    /// Returns whatever `f` returns. On device loss the returned `Err` satisfies
    /// [`is_device_lost`] — drop and recreate the surface against a fresh device.
    pub fn draw<R>(&self, scale: f32, f: impl FnOnce(&DrawingSession<'_>) -> R) -> Result<R> {
        let (context, (offset_x, offset_y)) = self.surface.begin_draw::<ID2D1DeviceContext>()?;
        self.device_lost.set(false);
        // Colors arrive linear scRGB: a linear FP16 surface writes them raw; an 8-bit
        // sRGB surface linear→sRGB encodes them at the boundary. The surface knows which.
        // pixel = dip * scale + atlas offset. The surface hands back an offset into a
        // shared atlas texture, so every frame must translate by it (it is not always
        // zero). Carrying it on the session rather than in the transform means a
        // caller's own `set_transform` composes with it instead of dropping it.
        let session = DrawingSession::from_borrowed_context(
            &context,
            Matrix3x2::translation(offset_x as f32, offset_y as f32),
        )
        .encode_srgb_target(!self.surface.is_linear());
        // The uniform scale keeps strokes/text crisp at the display DPI.
        session.set_transform(&Matrix3x2 {
            m11: scale,
            m12: 0.0,
            m21: 0.0,
            m22: scale,
            m31: 0.0,
            m32: 0.0,
        });
        let out = f(&session);
        drop(session);
        self.surface.end_draw()?;
        Ok(out)
    }

    /// Draw one frame, handing the closure a full [`DrawContext`] (DIP space, sized
    /// `width`×`height` at `dpi`) instead of the bare [`DrawingSession`]. Brackets
    /// the surface's native `BeginDraw`/`EndDraw`, adopts the returned Direct2D
    /// context, sets the dip→pixel transform (uniform `dpi/96` scale plus the atlas
    /// offset), and selects grayscale text antialiasing (mandatory on the
    /// premultiplied/transparent composition surface). `changed` is forwarded to
    /// [`DrawContext::device_changed`].
    ///
    /// This is the composition-surface analogue of [`SurfaceImage::draw_region`],
    /// so a [`SurfacePainter`](crate::SurfacePainter) routes its imperative repaint
    /// through the same draw closures whether it is hosted on a XAML
    /// `SurfaceImageSource` or a DirectComposition child-visual surface. The whole
    /// surface is always redrawn (composition surfaces present the full surface).
    pub(crate) fn draw_context(
        &self,
        device: &GpuDevice,
        width: f32,
        height: f32,
        dpi: f32,
        changed: bool,
        f: impl FnOnce(&DrawContext),
    ) -> Result<()> {
        let scale = dpi / 96.0;
        self.device_lost.set(false);
        let (context, (offset_x, offset_y)) = self
            .surface
            .begin_draw::<ID2D1DeviceContext>()
            .inspect_err(|e| {
                if is_device_lost(e.code()) {
                    self.device_lost.set(true);
                }
            })?;
        {
            // Colors arrive linear scRGB: a linear FP16 surface writes them raw; an
            // 8-bit sRGB surface linear→sRGB encodes them. The surface knows which.
            // The atlas offset rides on the session, so a caller's own
            // `set_transform` composes with it instead of dropping it.
            let session = DrawingSession::from_borrowed_context(
                &context,
                Matrix3x2::translation(offset_x as f32, offset_y as f32),
            )
            .encode_srgb_target(!self.surface.is_linear());
            // Grayscale text AA: ClearType's subpixel coverage is invalid on a
            // premultiplied/transparent surface (it would fringe).
            session.set_grayscale_text_antialiasing();
            // pixel = dip * scale + atlas offset; uniform scale keeps strokes/text
            // crisp at the display DPI without a `SetDpi` round-trip.
            session.set_transform(&Matrix3x2 {
                m11: scale,
                m12: 0.0,
                m21: 0.0,
                m22: scale,
                m31: 0.0,
                m32: 0.0,
            });
            let ctx = DrawContext::new(
                session,
                device,
                width,
                height,
                dpi,
                changed,
                Rect::from_xywh(0.0, 0.0, width, height),
            );
            f(&ctx);
        }
        let result = self.surface.end_draw();
        if let Err(e) = &result
            && is_device_lost(e.code())
        {
            self.device_lost.set(true);
        }
        result
    }

    /// Resize the backing surface in place to `pixel` physical pixels — forwards to
    /// [`CompositionDrawSurface::resize`]. The next [`draw`](Self::draw) fills the new
    /// extent; the presented sprite is resized separately on the hosting thread.
    pub fn resize(&self, pixel: (i32, i32)) -> Result<()> {
        self.surface.resize(pixel)
    }
}
