//! The composition bridge (feature `composition`).
//!
//! Two seams, and both have to live on this side of the boundary: a composition drawing
//! surface's `begin_draw` is generic over the interface to hand back, and only this crate
//! can name a Direct2D device context.
//!
//! # This is the one place a second context legitimately exists
//!
//! On the presented path a private context is a second `BeginDraw` and a Direct3D
//! device-context-state swap, measured at +748 samples against rendering through the
//! pass's own. Here it is not a choice: `BeginDraw` on a drawing surface hands back a
//! context created *for that call*. The asymmetry is the platform's, and it exists because
//! on a drawing surface `EndDraw` **is** the publish — so the bracket has to be the
//! surface's, and a live surface must never be retargeted inside one.
//!
//! Resources do not care. Direct2D shares anything created from a device context with any
//! other context on the same device, so brushes, paths and realizations built on the
//! [`Gpu`] work here unchanged.

use super::*;
use windows_composition::{
    AlphaMode, CompositionDrawHandle, CompositionDrawingSurface, CompositionGraphicsDevice,
    Compositor, PixelFormat,
};

impl Gpu {
    /// A composition graphics device backed by this one.
    ///
    /// One `Gpu` per compositor. When the compositor realizes a composition path it asks
    /// the geometry source for geometry belonging to a factory of its own choosing, and
    /// neither side of that callback can check the match — so a path from a second `Gpu`
    /// shows up as content that never appears rather than as an error.
    pub fn graphics_device(&self, compositor: &Compositor) -> Result<CompositionGraphicsDevice> {
        compositor.create_graphics_device(self.d2d())
    }
}

/// Allocates the surfaces retained content is rasterized into.
///
/// The only allocator, which is the point: **no colour reaches the compositor at eight
/// bits**, so it never gets a surface it would treat as sRGB and colour-manage on terms
/// this pipeline does not control.
///
/// That rule is about colour, and a mask has none. [`mask`](Self::mask) allocates one alpha
/// channel — an eighth of [`color`](Self::color)'s memory for the same coverage. Two
/// methods and not one with a flag, because only one of them can fail for a reason the
/// caller has to handle.
pub trait SceneSurface: Sealed {
    /// A scene-linear colour surface `px` **whole pixels** in size.
    ///
    /// Pixels and not DIPs because a cache keys its content by pixel extent, and the
    /// DIP-sized allocator converts by the current scale and rounds — so it cannot express
    /// "exactly N wide", which is what the key means.
    fn color(&self, px: (i32, i32), opacity: Opacity) -> Result<CompositionDrawingSurface>;

    /// A coverage surface `px` **whole pixels** in size: one alpha channel and no colour.
    ///
    /// Always premultiplied and always translucent — an opaque mask is not a mask.
    ///
    /// `Err` where the device has no `A8`, which is how support is discovered: the format
    /// is not universal and there is no query for it. A caller falls back to
    /// [`color`](Self::color), which is correct and eight times the memory, and should
    /// remember the answer rather than ask per surface.
    fn mask(&self, px: (i32, i32)) -> Result<CompositionDrawingSurface>;
}

impl Sealed for CompositionGraphicsDevice {}

impl SceneSurface for CompositionGraphicsDevice {
    fn color(&self, px: (i32, i32), opacity: Opacity) -> Result<CompositionDrawingSurface> {
        let alpha = match opacity {
            Opacity::Translucent => AlphaMode::Premultiplied,
            Opacity::Opaque => AlphaMode::Ignore,
        };
        self.create_drawing_surface_with_pixel_size(px.0, px.1, PixelFormat::Rgba16Float, alpha)
    }

    fn mask(&self, px: (i32, i32)) -> Result<CompositionDrawingSurface> {
        self.create_drawing_surface_with_pixel_size(
            px.0,
            px.1,
            PixelFormat::A8UNorm,
            AlphaMode::Premultiplied,
        )
    }
}

/// Draws into a composition surface.
pub trait SurfaceDraw: Sealed {
    /// Runs `f` against the surface, then publishes.
    ///
    /// `Ok(false)` means the device was lost and the surface must be rebuilt — reported
    /// rather than raised, because a lost surface is an ordinary event on this path and the
    /// caller's response is to drop its cache, not to fail.
    fn draw(&self, dpi: f32, opacity: Opacity, f: impl FnOnce(&Draw<'_>)) -> Result<bool>;
}

impl Sealed for CompositionDrawingSurface {}

impl SurfaceDraw for CompositionDrawingSurface {
    fn draw(&self, dpi: f32, opacity: Opacity, f: impl FnOnce(&Draw<'_>)) -> Result<bool> {
        match self.begin_draw::<ID2D1DeviceContext>() {
            Ok((ctx, offset)) => paint(&ctx, offset, dpi, opacity, f, || self.end_draw()),
            Err(e) if classify(e.code()) != Loss::None => Ok(false),
            Err(e) => Err(e),
        }
    }
}

impl Sealed for CompositionDrawHandle {}

impl SurfaceDraw for CompositionDrawHandle {
    fn draw(&self, dpi: f32, opacity: Opacity, f: impl FnOnce(&Draw<'_>)) -> Result<bool> {
        match self.begin_draw::<ID2D1DeviceContext>() {
            Ok((ctx, offset)) => paint(&ctx, offset, dpi, opacity, f, || self.end_draw()),
            Err(e) if classify(e.code()) != Loss::None => Ok(false),
            Err(e) => Err(e),
        }
    }
}

/// Shared by both surface kinds: configure, offset, draw, publish.
fn paint(
    ctx: &ID2D1DeviceContext,
    offset: (i32, i32),
    dpi: f32,
    opacity: Opacity,
    f: impl FnOnce(&Draw<'_>),
    // `Fn` and not `FnOnce`, because the panic guard and the success path each need to be
    // able to call it — and only one of them ever will.
    publish: impl Fn() -> Result<()>,
) -> Result<bool> {
    // Every version of the context after the first is a cast on the object the surface
    // created, not a new one. It is requested as the base interface and narrowed because
    // that is what the interop contract names.
    check_dpi(dpi);
    let ctx: ID2D1DeviceContext6 = ctx.cast()?;
    // A fresh context defaults to Direct2D's own state, not this device's, so the pipeline
    // settings are restated from the one function that owns them.
    device::configure(&ctx)?;

    // Content is packed into a shared backing atlas, so the surface hands back the origin of
    // its own tile within it and everything drawn is translated by that. The caller draws at
    // (0, 0) and never learns where it landed.
    //
    // The offset arrives in **pixels** and the transform is applied in DIPs, so it has to be
    // divided by the scale. Translating by the raw pixel count would displace content by
    // `scale - 1` times the offset — at 1.5× a tile 100 pixels into the atlas lands 50 pixels
    // past its own tile, on top of a neighbour's. Nothing about that is visible at 96 DPI,
    // where the two numbers are equal.
    //
    // The round trip through DIPs is not exact: Direct2D multiplies the scale back in, and
    // the worst realistic case — a 4096-pixel atlas at 1.25× — returns within about 3×10⁻⁴ of
    // a pixel. Displacing content into a neighbouring pixel needs half of one, and even
    // aliased pixel-centre sampling sits ~0.4999 away from flipping, so the margin is four
    // orders of magnitude.
    let guard = Publish(&publish);
    let scale = dpi / 96.0;
    unsafe {
        ctx.SetDpi(dpi, dpi);
        ctx.SetTransform(&Matrix3x2::translation(
            offset.0 as f32 / scale,
            offset.1 as f32 / scale,
        ));
    }
    f(&Draw::borrowed(&ctx, opacity, dpi));
    core::mem::forget(guard);

    match publish() {
        Ok(()) => Ok(true),
        Err(e) if classify(e.code()) != Loss::None => Ok(false),
        Err(e) => Err(e),
    }
}

/// Publishes on the panic path only, so a successful `begin_draw` is never left unpaired.
struct Publish<'a, F: Fn() -> Result<()>>(&'a F);

impl<F: Fn() -> Result<()>> Drop for Publish<'_, F> {
    fn drop(&mut self) {
        let _ = (self.0)();
    }
}
