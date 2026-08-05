//! The composition bridge (feature `composition`).
//!
//! This module holds both seams: a composition drawing surface's `begin_draw` is generic
//! over the interface it hands back, and only this crate names a Direct2D device context.
//!
//! # The one context this crate does not own
//!
//! `BeginDraw` on a drawing surface hands back a context created *for that call*, so
//! drawing into composition content runs on a context other than the [`Gpu`]'s. On a
//! drawing surface `EndDraw` is the publish, so the bracket belongs to the surface and a
//! live surface must never be retargeted inside one.
//!
//! Resources are unaffected. Direct2D shares anything created from a device context with
//! any other context on the same device, so brushes, paths and realizations built on the
//! [`Gpu`] work here unchanged.

use super::*;
use windows_composition::{
    AlphaMode, CompositionDrawHandle, CompositionDrawingSurface, CompositionGraphicsDevice,
    Compositor, PixelFormat,
};

impl Gpu {
    /// Creates a composition graphics device backed by this one.
    ///
    /// One `Gpu` per compositor. When the compositor realizes a composition path it asks
    /// the geometry source for geometry belonging to a factory of its own choosing, and
    /// neither side of that callback checks the match, so a path from a second `Gpu`
    /// renders as content that never appears rather than as an error.
    pub fn graphics_device(&self, compositor: &Compositor) -> Result<CompositionGraphicsDevice> {
        compositor.create_graphics_device(self.d2d())
    }
}

/// Allocates the surfaces retained content is rasterized into.
///
/// Every colour surface is FP16, so **no colour reaches the compositor at eight bits** and
/// the compositor never holds a surface it would treat as sRGB and colour-manage.
///
/// A mask carries no colour, so [`mask`](Self::mask) allocates one alpha channel — an
/// eighth of [`color`](Self::color)'s memory for the same coverage. The two are separate
/// methods because only `mask` fails for a reason the caller has to handle.
pub trait SceneSurface: Sealed {
    /// Allocates a scene-linear colour surface `px` **whole pixels** in size, with
    /// `opacity` choosing its alpha mode.
    ///
    /// Pixels and not DIPs: a cache keys its content by pixel extent, and the DIP-sized
    /// allocator converts by the current scale and rounds, so it cannot express an exact
    /// pixel width.
    fn color(&self, px: (i32, i32), opacity: Opacity) -> Result<CompositionDrawingSurface>;

    /// Allocates a coverage surface `px` **whole pixels** in size: one alpha channel and no
    /// colour, always premultiplied and always translucent.
    ///
    /// # Errors
    ///
    /// Fails where the device has no `A8`, which is how support is discovered: the format
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
    /// Runs `f` against the surface at `dpi`, treating its content as `opacity`, then
    /// publishes.
    ///
    /// Returns `Ok(false)` when the device was lost: the caller drops its cache and
    /// rebuilds the surface. Device loss is reported rather than raised because it is an
    /// ordinary event on this path.
    ///
    /// # Errors
    ///
    /// Any failure of `BeginDraw` or the publish that is not device loss.
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

/// Configures the context a surface handed back, offsets into the surface's atlas tile,
/// runs `f`, and publishes. Shared by both surface kinds.
fn paint(
    ctx: &ID2D1DeviceContext,
    offset: (i32, i32),
    dpi: f32,
    opacity: Opacity,
    f: impl FnOnce(&Draw<'_>),
    // `Fn` and not `FnOnce`: the panic guard and the success path can each call it, and
    // exactly one of them does.
    publish: impl Fn() -> Result<()>,
) -> Result<bool> {
    // The interop contract names the base interface, so the context arrives as
    // `ID2D1DeviceContext` and narrowing is a cast on the object the surface created.
    check_dpi(dpi);
    let ctx: ID2D1DeviceContext6 = ctx.cast()?;
    // A context created for this call carries Direct2D's own defaults rather than this
    // device's state, so `device::configure` restates the pipeline settings.
    device::configure(&ctx)?;

    // Content is packed into a shared backing atlas, so the surface hands back the origin of
    // its own tile within it and everything drawn is translated by that. The caller draws at
    // (0, 0) and never learns where it landed.
    //
    // The offset arrives in **pixels** and the transform is applied in DIPs, so it is divided
    // by the scale. Translating by the raw pixel count displaces content by `scale - 1` times
    // the offset: at 1.5× a tile 100 pixels into the atlas lands 50 pixels past its own tile,
    // on top of a neighbour's. At 96 DPI the two numbers are equal and the error is absent.
    //
    // The round trip through DIPs is not exact, because Direct2D multiplies the scale back
    // in. A 4096-pixel atlas at 1.25× returns within about 3×10⁻⁴ of a pixel; displacing
    // content into a neighbouring pixel needs half a pixel, and aliased pixel-centre sampling
    // sits ~0.4999 away from flipping, so the margin is four orders of magnitude.
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
