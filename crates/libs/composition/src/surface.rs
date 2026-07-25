//! The Direct2D bridge surface (feature `system`).
//!
//! These types let an app draw Direct2D content into a composition surface and
//! paint a visual with it. The graphics-device and drawing-surface interop exists
//! only on the system stack, so the whole bridge is system-only; lifted
//! composition has no Direct2D-surface interop.
//!
//! The [`begin_draw`](CompositionDrawingSurface::begin_draw) /
//! [`end_draw`](CompositionDrawingSurface::end_draw) seam is what
//! [`windows-canvas`](https://docs.rs/windows-canvas)'s `composition` feature
//! draws through; most callers use that bridge rather than these methods
//! directly.

use super::*;

/// A Direct2D-backed composition device that allocates drawing surfaces.
///
/// Create one from a [`Compositor`](crate::Compositor) and the app's Direct2D (or
/// DXGI) rendering device with
/// [`Compositor::create_graphics_device`](crate::Compositor::create_graphics_device),
/// then allocate [`CompositionDrawingSurface`]s to draw into.
#[derive(Clone)]
pub struct CompositionGraphicsDevice(pub(crate) bindings::CompositionGraphicsDevice);

impl CompositionGraphicsDevice {
    /// Creates a drawing surface `width`×`height` pixels in size, using a
    /// premultiplied BGRA pixel format.
    pub fn create_drawing_surface(
        &self,
        width: f32,
        height: f32,
    ) -> Result<CompositionDrawingSurface> {
        let surface = self.0.CreateDrawingSurface(
            bindings::Size { width, height },
            bindings::DirectXPixelFormat::B8G8R8A8UIntNormalized,
            bindings::DirectXAlphaMode::Premultiplied,
        )?;
        CompositionDrawingSurface::new(surface)
    }

    /// Creates a drawing surface `width`×`height` DIPs in size with a chosen
    /// pixel format and alpha mode.
    ///
    /// Callers that render in a wide-gamut or high-dynamic-range space allocate
    /// [`PixelFormat::Rgba16Float`] here, and those drawing fully opaque content
    /// pair it with [`AlphaMode::Ignore`] so the compositor does not blend an
    /// alpha channel the content never writes.
    ///
    /// Not every format is supported by every device, and the error is how that
    /// is discovered: a caller probing for, say, [`PixelFormat::A8UNorm`] support
    /// creates a surface and treats `Err` as "unsupported, fall back". So this
    /// returns a [`Result`] rather than panicking the way the crate's infallible
    /// setters do.
    pub fn create_drawing_surface_with_format(
        &self,
        width: f32,
        height: f32,
        format: PixelFormat,
        alpha: AlphaMode,
    ) -> Result<CompositionDrawingSurface> {
        let surface = self.0.CreateDrawingSurface(
            bindings::Size { width, height },
            format.into(),
            alpha.into(),
        )?;
        CompositionDrawingSurface::new(surface)
    }

    /// Creates a drawing surface sized in whole pixels rather than DIPs.
    ///
    /// [`create_drawing_surface_with_format`](Self::create_drawing_surface_with_format)
    /// takes a DIP size that the device converts by the current scale, so it
    /// cannot express "exactly N pixels wide" — the conversion rounds. A cache
    /// that keys rasterized content by its pixel extent needs the surface to
    /// match that key exactly, which is what this allocates.
    ///
    /// As with the DIP-sized sibling, an unsupported format surfaces as `Err`.
    pub fn create_drawing_surface_with_pixel_size(
        &self,
        width: i32,
        height: i32,
        format: PixelFormat,
        alpha: AlphaMode,
    ) -> Result<CompositionDrawingSurface> {
        let device: bindings::ICompositionGraphicsDevice2 = self.0.cast()?;
        let surface = device.CreateDrawingSurface2(
            bindings::SizeInt32 { width, height },
            format.into(),
            alpha.into(),
        )?;
        CompositionDrawingSurface::new(surface)
    }
}

/// The pixel format a [`CompositionDrawingSurface`] stores its content in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8 bits per channel, blue-green-red-alpha. The ordinary format for
    /// standard-dynamic-range content.
    Bgra8UNorm,
    /// 16-bit float per channel, red-green-blue-alpha. Carries values outside
    /// `0.0..=1.0`, so it can hold a wide-gamut or high-dynamic-range image that
    /// an 8-bit format would clip.
    Rgba16Float,
    /// A single 8-bit alpha channel and no color. Used for coverage — the mask
    /// half of a [`CompositionMaskBrush`](crate::CompositionMaskBrush) — at a
    /// quarter of the memory a color surface would take.
    A8UNorm,
}

impl From<PixelFormat> for bindings::DirectXPixelFormat {
    fn from(format: PixelFormat) -> Self {
        match format {
            PixelFormat::Bgra8UNorm => Self::B8G8R8A8UIntNormalized,
            PixelFormat::Rgba16Float => Self::R16G16B16A16Float,
            PixelFormat::A8UNorm => Self::A8UIntNormalized,
        }
    }
}

/// How the compositor interprets a surface's alpha channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaMode {
    /// Each color channel is already multiplied by alpha.
    Premultiplied,
    /// The alpha channel is disregarded and the surface treated as opaque.
    Ignore,
}

impl From<AlphaMode> for bindings::DirectXAlphaMode {
    fn from(alpha: AlphaMode) -> Self {
        match alpha {
            AlphaMode::Premultiplied => Self::Premultiplied,
            AlphaMode::Ignore => Self::Ignore,
        }
    }
}

/// A composition surface that Direct2D content is drawn into and painted onto a
/// visual through a [`CompositionSurfaceBrush`].
///
/// Allocate one from a [`CompositionGraphicsDevice`]. Draw into it with the
/// canvas bridge, which brackets each redraw between
/// [`begin_draw`](Self::begin_draw) and [`end_draw`](Self::end_draw).
#[derive(Clone)]
pub struct CompositionDrawingSurface {
    surface: bindings::CompositionDrawingSurface,
    interop: bindings::ICompositionDrawingSurfaceInterop,
}

impl CompositionDrawingSurface {
    /// The one funnel every allocation reaches, whichever of the device's three
    /// factories the caller used — so the census counts a surface once.
    fn new(surface: bindings::CompositionDrawingSurface) -> Result<Self> {
        let interop = surface.cast()?;
        bump_count(Count::DrawingSurface);
        Ok(Self { surface, interop })
    }

    /// Begins drawing into the surface, returning the drawing target `T`
    /// (typically `ID2D1DeviceContext`) and the `(x, y)` pixel offset within the
    /// backing atlas at which to draw. Apply the offset as a translation on the
    /// target before issuing draw calls, and pair every call with
    /// [`end_draw`](Self::end_draw).
    ///
    /// This is the interop seam the canvas bridge draws through; most callers use
    /// that bridge instead of calling this directly.
    pub fn begin_draw<T: Interface>(&self) -> Result<(T, (i32, i32))> {
        bump_count(Count::SurfaceDraw);
        let mut offset = bindings::POINT::default();
        let object = unsafe { self.interop.BeginDraw::<T>(None, &mut offset)? };
        Ok((object, (offset.x, offset.y)))
    }

    /// Finishes drawing begun with [`begin_draw`](Self::begin_draw) and presents
    /// the surface contents.
    pub fn end_draw(&self) -> Result<()> {
        unsafe { self.interop.EndDraw().ok() }
    }

    /// Resizes the surface to `width`×`height` pixels.
    pub fn resize(&self, width: i32, height: i32) -> Result<()> {
        unsafe {
            self.interop
                .Resize(bindings::SIZE {
                    cx: width,
                    cy: height,
                })
                .ok()
        }
    }

    /// Returns a handle that can draw into this surface from another thread.
    ///
    /// See [`CompositionDrawHandle`] for what may cross the thread boundary and
    /// what must stay behind.
    pub fn draw_handle(&self) -> CompositionDrawHandle {
        CompositionDrawHandle(self.interop.clone())
    }
}

impl Sealed for CompositionDrawingSurface {}

impl Surface for CompositionDrawingSurface {
    fn as_surface(&self) -> CompositionSurface {
        CompositionSurface(self.surface.cast().unwrap())
    }
}

/// The drawing half of a [`CompositionDrawingSurface`], detached so it can be
/// moved to another thread.
///
/// A renderer that produces content continuously — a visualization redrawn every
/// frame — must not do that work on the thread that composites, or every raster
/// delays a commit. The split that makes this possible is that *drawing* into a
/// surface and *owning* the surface are separable: this handle carries only the
/// former, while the surface itself, the brush painting with it, and the visual
/// showing it stay on the owning thread and remain that thread's to mutate.
///
/// The handle mirrors the owning surface's [`begin_draw`](Self::begin_draw),
/// [`end_draw`](Self::end_draw) and [`resize`](Self::resize). Drawing must still
/// be serialized: bracket each redraw between `begin_draw` and `end_draw`, and do
/// not let a resize overlap a bracket.
#[derive(Clone)]
pub struct CompositionDrawHandle(bindings::ICompositionDrawingSurfaceInterop);

// SAFETY: the interop interface is a second face on the same
// `CompositionDrawingSurface` WinRT object, and that object is agile — it
// aggregates the free-threaded marshaler, which is why the generated binding for
// the class itself is declared `Send`/`Sync`. Agility is a property of the
// object, not of the interface: an `ICompositionDrawingSurfaceInterop` pointer
// obtained by `QueryInterface` on an agile object needs no marshalling and may be
// called from any apartment. Only the interface pointer moves here; nothing that
// is thread-affine does. In particular no `Compositor`, `Visual` or brush is
// reachable from this type — it holds exactly one interface pointer and exposes
// no accessor back to the surface — so moving it cannot smuggle the compositor's
// object graph off the owning thread. The Direct2D interface `begin_draw`
// returns is a device context created for the caller, not a composition object,
// so nothing escapes through the return value either.
//
// This is `Send` and deliberately not `Sync`: the underlying interface is
// internally synchronized for lifetime purposes, but `BeginDraw`/`EndDraw` are a
// stateful bracket on the surface, and two threads sharing one handle could
// interleave them. Requiring the handle to be moved, not shared, keeps that
// bracket owned by a single thread at a time.
unsafe impl Send for CompositionDrawHandle {}

impl CompositionDrawHandle {
    /// Begins drawing into the surface, returning the drawing target `T`
    /// (typically `ID2D1DeviceContext`) and the `(x, y)` pixel offset within the
    /// backing atlas at which to draw.
    ///
    /// Mirrors [`CompositionDrawingSurface::begin_draw`]; pair every call with
    /// [`end_draw`](Self::end_draw).
    pub fn begin_draw<T: Interface>(&self) -> Result<(T, (i32, i32))> {
        bump_count(Count::SurfaceDraw);
        let mut offset = bindings::POINT::default();
        let object = unsafe { self.0.BeginDraw::<T>(None, &mut offset)? };
        Ok((object, (offset.x, offset.y)))
    }

    /// Finishes drawing begun with [`begin_draw`](Self::begin_draw) and presents
    /// the surface contents.
    pub fn end_draw(&self) -> Result<()> {
        unsafe { self.0.EndDraw().ok() }
    }

    /// Resizes the surface to `width`×`height` pixels.
    pub fn resize(&self, width: i32, height: i32) -> Result<()> {
        unsafe {
            self.0
                .Resize(bindings::SIZE {
                    cx: width,
                    cy: height,
                })
                .ok()
        }
    }
}

/// The base type shared by every composition surface — one a
/// [`CompositionSurfaceBrush`] can paint with.
///
/// A [`Surface`] can be turned into one via [`Surface::as_surface`].
#[derive(Clone)]
pub struct CompositionSurface(pub(crate) bindings::ICompositionSurface);

/// Content a [`CompositionSurfaceBrush`] can paint a visual with: either a
/// [`CompositionDrawingSurface`] holding rasterized content, or a
/// [`CompositionVisualSurface`] capturing a live visual subtree.
///
/// This trait is sealed: only the surface types in this crate implement it.
pub trait Surface: Sealed {
    /// Returns this surface as the shared [`CompositionSurface`] base type.
    fn as_surface(&self) -> CompositionSurface;
}

/// A surface whose content is a live visual subtree, rather than pixels drawn
/// into it.
///
/// It captures the visual set by [`set_source_visual`](Self::set_source_visual) —
/// and that visual's descendants — as they are currently composed, so painting a
/// brush with it re-uses an already-composed subtree instead of re-rasterizing
/// its content. The captured region is the rectangle at
/// [`set_source_offset`](Self::set_source_offset) of
/// [`set_source_size`](Self::set_source_size), in the source visual's own
/// coordinate space.
///
/// Create one with
/// [`Compositor::create_visual_surface`](crate::Compositor::create_visual_surface).
#[derive(Clone)]
pub struct CompositionVisualSurface(pub(crate) bindings::CompositionVisualSurface);

impl CompositionVisualSurface {
    /// Sets the visual whose subtree this surface captures.
    pub fn set_source_visual(&self, visual: &Visual) {
        bump_count(Count::PropertyWrite);
        self.0.SetSourceVisual(&visual.0).unwrap();
    }

    /// Sets the top-left corner of the captured region, in the source visual's
    /// coordinate space.
    pub fn set_source_offset(&self, offset: Vector2) {
        bump_count(Count::PropertyWrite);
        self.0.SetSourceOffset(offset).unwrap();
    }

    /// Sets the size of the captured region, in the source visual's coordinate
    /// space.
    pub fn set_source_size(&self, size: Vector2) {
        bump_count(Count::PropertyWrite);
        self.0.SetSourceSize(size).unwrap();
    }
}

impl Sealed for CompositionVisualSurface {}

impl Surface for CompositionVisualSurface {
    fn as_surface(&self) -> CompositionSurface {
        CompositionSurface(self.0.cast().unwrap())
    }
}

impl Compositor {
    /// Creates a surface that captures a live visual subtree.
    pub fn create_visual_surface(&self) -> CompositionVisualSurface {
        bump_count(Count::VisualSurface);
        let compositor: bindings::ICompositorWithVisualSurface = self.0.cast().unwrap();
        CompositionVisualSurface(compositor.CreateVisualSurface().unwrap())
    }
}

/// A brush that paints a visual with the contents of a [`Surface`].
///
/// Create one with
/// [`Compositor::create_surface_brush`](crate::Compositor::create_surface_brush).
#[derive(Clone)]
pub struct CompositionSurfaceBrush(pub(crate) bindings::CompositionSurfaceBrush);

impl CompositionSurfaceBrush {
    /// Sets how the surface is fitted into the area the brush paints.
    ///
    /// The composition default is [`Stretch::Uniform`], which letterboxes the
    /// surface whenever its aspect ratio differs from the painted area's. A
    /// caller that has already sized the surface to the area it paints — an
    /// atlas, a glyph run, a gradient ramp — wants [`Stretch::Fill`], so the
    /// surface maps onto the area one-to-one.
    pub fn set_stretch(&self, stretch: Stretch) {
        self.0.SetStretch(stretch.into()).unwrap();
    }
}

impl Sealed for CompositionSurfaceBrush {}

impl Brush for CompositionSurfaceBrush {
    fn as_brush(&self) -> CompositionBrush {
        CompositionBrush(self.0.cast().unwrap())
    }
}

impl PartialEq for CompositionSurfaceBrush {
    fn eq(&self, other: &Self) -> bool {
        canonical(&self.0) == canonical(&other.0)
    }
}

impl Eq for CompositionSurfaceBrush {}

/// How a [`CompositionSurfaceBrush`] fits its surface into the area it paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stretch {
    /// Do not scale the surface; paint it at its natural size.
    None,
    /// Scale each axis independently to fill the area, ignoring aspect ratio.
    Fill,
    /// Scale uniformly until the surface fits inside the area, letterboxing the
    /// remainder. This is composition's default.
    Uniform,
    /// Scale uniformly until the surface covers the area, cropping the overflow.
    UniformToFill,
}

impl From<Stretch> for bindings::CompositionStretch {
    fn from(stretch: Stretch) -> Self {
        match stretch {
            Stretch::None => Self::None,
            Stretch::Fill => Self::Fill,
            Stretch::Uniform => Self::Uniform,
            Stretch::UniformToFill => Self::UniformToFill,
        }
    }
}
