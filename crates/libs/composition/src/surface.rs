//! The Direct2D bridge surface (feature `system`).
//!
//! Direct2D drawing-surface interop for system composition. Lifted composition has no
//! Direct2D-surface interop.
//!
//! Three kinds of content can end up on a visual through a [`CompositionSurfaceBrush`],
//! and they differ in kind rather than in degree: a [`CompositionDrawingSurface`] holds
//! pixels something drew, a [`CompositionVisualSurface`] holds an already-composed
//! subtree, and a bare [`CompositionSurface`] is a buffer the app presents itself and the
//! compositor only samples. The [`Surface`] trait is what lets one brush take any of them.

// Every `unsafe` block below is a call to a Win32 interop method, which the generated
// bindings declare `unsafe` because COM cannot express the contract in a signature. What
// discharges it here is uniform and worth stating once instead of eight times: the interface
// pointer is owned by the wrapper and cannot be null or dangling, the out-parameters are
// stack locals that outlive their call, and none of these methods retains a borrow. Nothing
// in this module asks the *caller* for an obligation except
// [`Compositor::create_surface_for_handle`], which is `unsafe` for that reason and says so.

use super::*;

/// Direct2D-backed composition device that allocates drawing surfaces.
#[derive(Clone)]
pub struct CompositionGraphicsDevice(pub(crate) bindings::CompositionGraphicsDevice);

impl CompositionGraphicsDevice {
    /// Creates a drawing surface `width`x`height` pixels in size, using a
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

    /// Creates a drawing surface `width`x`height` DIPs in size with a chosen pixel format
    /// and alpha mode.
    ///
    /// Callers rendering in a wide-gamut or high-dynamic-range space allocate
    /// [`PixelFormat::Rgba16Float`] here, and those drawing fully opaque content pair it
    /// with [`AlphaMode::Ignore`] so the compositor does not blend an alpha channel the
    /// content never writes.
    ///
    /// Not every format is supported by every device, and the error is how that is
    /// discovered: a caller probing for [`PixelFormat::A8UNorm`] support creates a surface
    /// and treats `Err` as "unsupported, fall back". So this returns a [`Result`] rather
    /// than panicking the way the crate's infallible setters do.
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
    /// takes a DIP size the device converts by the current scale, so it cannot express
    /// "exactly N pixels wide" — the conversion rounds. A cache that keys rasterized
    /// content by its pixel extent needs the surface to match that key exactly, which is
    /// what this allocates.
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

    /// Creates a **sparsely allocated** surface `width`x`height` pixels in size.
    ///
    /// A virtual surface holds no storage when it is created: the engine materializes a
    /// tile only when a [`begin_draw`](CompositionVirtualDrawingSurface::begin_draw) names
    /// a region inside it. So the declared size is a coordinate space to allocate within
    /// rather than an allocation, and a caller can declare one far larger than it expects
    /// to fill.
    ///
    /// That is what makes a virtual surface the backing for a raster atlas. A cache minting
    /// a surface per entry pays a composition object, a texture and the engine's
    /// per-surface bookkeeping for every entry it has ever drawn; packing those rasters
    /// into regions of one virtual surface costs one of each, and the memory still tracks
    /// what was actually drawn.
    ///
    /// The declared size is capped at 2^24 total pixels by the platform.
    pub fn create_virtual_drawing_surface(
        &self,
        width: i32,
        height: i32,
        format: PixelFormat,
        alpha: AlphaMode,
    ) -> Result<CompositionVirtualDrawingSurface> {
        let device: bindings::ICompositionGraphicsDevice2 = self.0.cast()?;
        let surface = device.CreateVirtualDrawingSurface(
            bindings::SizeInt32 { width, height },
            format.into(),
            alpha.into(),
        )?;
        CompositionVirtualDrawingSurface::new(surface)
    }
}

/// The pixel format a [`CompositionDrawingSurface`] stores its content in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8 bits per channel, blue-green-red-alpha. The ordinary format for
    /// standard-dynamic-range content.
    Bgra8UNorm,
    /// 16-bit float per channel, red-green-blue-alpha. Carries values outside `0.0..=1.0`,
    /// so it holds wide-gamut or above-paper-white content an 8-bit format would clip.
    Rgba16Float,
    /// A single 8-bit alpha channel and no colour. Used for coverage — the mask half of a
    /// [`CompositionMaskBrush`](crate::CompositionMaskBrush) — at a quarter of the memory a
    /// colour surface would take.
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
    /// Each colour channel is already multiplied by alpha.
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

/// Composition surface that Direct2D content is drawn into.
#[derive(Clone)]
pub struct CompositionDrawingSurface {
    surface: bindings::CompositionDrawingSurface,
    interop: bindings::ICompositionDrawingSurfaceInterop,
}

impl CompositionDrawingSurface {
    fn new(surface: bindings::CompositionDrawingSurface) -> Result<Self> {
        let interop = surface.cast()?;
        Ok(Self { surface, interop })
    }

    /// Begins drawing, returning the target and backing-atlas pixel offset.
    ///
    /// Apply the offset as a translation on the target before issuing draw calls, and pair
    /// every call with [`end_draw`](Self::end_draw).
    ///
    /// **`end_draw` is the publish.** A composition drawing surface has no separate
    /// present, so a surface must never be retargeted inside an open bracket: doing so
    /// shows a half-drawn surface. The rule is a property of what publishes and does *not*
    /// extend to a presentation buffer, which publishes on submit and is retargeted
    /// mid-bracket by design.
    pub fn begin_draw<T: Interface>(&self) -> Result<(T, (i32, i32))> {
        let mut offset = bindings::POINT::default();
        let object = unsafe { self.interop.BeginDraw::<T>(None, &mut offset)? };
        Ok((object, (offset.x, offset.y)))
    }

    /// Finishes drawing begun with [`begin_draw`](Self::begin_draw) and presents
    /// the surface contents.
    pub fn end_draw(&self) -> Result<()> {
        unsafe { self.interop.EndDraw().ok() }
    }

    /// Resizes the surface to `width`x`height` pixels.
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
    /// See [`CompositionDrawHandle`] for what may cross the thread boundary and what must
    /// stay behind.
    pub fn draw_handle(&self) -> CompositionDrawHandle {
        CompositionDrawHandle(self.interop.clone())
    }

    /// The surface as the `ICompositionSurface` a surface brush paints with.
    pub(crate) fn as_surface(&self) -> bindings::ICompositionSurface {
        self.surface.cast().unwrap()
    }
}

impl Sealed for CompositionDrawingSurface {}

impl Surface for CompositionDrawingSurface {
    fn as_surface(&self) -> CompositionSurface {
        CompositionSurface(Self::as_surface(self))
    }
}

/// A drawing surface whose storage is allocated per drawn region rather than up front —
/// the backing for a raster atlas.
///
/// Draw into a region with [`begin_draw`](Self::begin_draw), which both reserves that
/// region's storage and returns where in the engine's own tile to put the pixels.
#[derive(Clone)]
pub struct CompositionVirtualDrawingSurface {
    surface: bindings::CompositionVirtualDrawingSurface,
    interop: bindings::ICompositionDrawingSurfaceInterop,
}

impl CompositionVirtualDrawingSurface {
    fn new(surface: bindings::CompositionVirtualDrawingSurface) -> Result<Self> {
        let interop = surface.cast()?;
        Ok(Self { surface, interop })
    }

    /// Begins drawing into the region `(x, y, width, height)`, returning the drawing target
    /// and the `(x, y)` pixel offset at which to draw it.
    ///
    /// The returned offset is where the region landed inside whichever tile the engine
    /// allocated for it, and bears **no relation to the requested origin** — so a caller
    /// translates its drawing by the offset and never by its own coordinates. Drawing
    /// outside the requested region is undefined, and the region's initial contents are
    /// undefined too: every pixel in it must be written, so clear it first if the content
    /// does not cover it.
    pub fn begin_draw<T: Interface>(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(T, (i32, i32))> {
        let rect = bindings::RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
        let mut offset = bindings::POINT::default();
        let object = unsafe { self.interop.BeginDraw::<T>(Some(&rect), &mut offset)? };
        Ok((object, (offset.x, offset.y)))
    }

    /// Finishes drawing begun with [`begin_draw`](Self::begin_draw) and presents the
    /// region's contents.
    pub fn end_draw(&self) -> Result<()> {
        unsafe { self.interop.EndDraw().ok() }
    }
}

impl Sealed for CompositionVirtualDrawingSurface {}

impl Surface for CompositionVirtualDrawingSurface {
    fn as_surface(&self) -> CompositionSurface {
        CompositionSurface(self.surface.cast().unwrap())
    }
}

/// The drawing half of a [`CompositionDrawingSurface`], detached so it can be moved to
/// another thread.
///
/// A renderer that produces content continuously must not do that work on the thread that
/// composites, or every raster delays a commit. The split that makes this possible is that
/// *drawing* into a surface and *owning* it are separable: this handle carries only the
/// former, while the surface, the brush painting with it and the visual showing it stay on
/// the owning thread and remain that thread's to mutate.
///
/// Drawing must still be serialized: bracket each redraw between
/// [`begin_draw`](Self::begin_draw) and [`end_draw`](Self::end_draw), and do not let a
/// resize overlap a bracket.
#[derive(Clone)]
pub struct CompositionDrawHandle(bindings::ICompositionDrawingSurfaceInterop);

// SAFETY: the interop interface is a second face on the same `CompositionDrawingSurface`
// WinRT object, and that object is agile — it aggregates the free-threaded marshaler, which
// is why the generated binding for the class itself is declared `Send`/`Sync`. Agility is a
// property of the object, not of the interface: an `ICompositionDrawingSurfaceInterop`
// pointer obtained by `QueryInterface` on an agile object needs no marshalling and may be
// called from any apartment. Only the interface pointer moves here; nothing thread-affine
// does. In particular no `Compositor`, `Visual` or brush is reachable from this type — it
// holds exactly one interface pointer and exposes no accessor back to the surface — so
// moving it cannot smuggle the compositor's object graph off the owning thread. The Direct2D
// interface `begin_draw` returns is a device context created for the caller, not a
// composition object, so nothing escapes through the return value either.
//
// `Send` and deliberately not `Sync`: the underlying interface is internally synchronized
// for lifetime purposes, but `BeginDraw`/`EndDraw` are a stateful bracket on the surface,
// and two threads sharing one handle could interleave them. Requiring the handle to be moved
// rather than shared keeps that bracket owned by one thread at a time.
unsafe impl Send for CompositionDrawHandle {}

impl CompositionDrawHandle {
    /// Begins drawing into the surface, returning the drawing target and the `(x, y)` pixel
    /// offset within the backing atlas at which to draw.
    pub fn begin_draw<T: Interface>(&self) -> Result<(T, (i32, i32))> {
        let mut offset = bindings::POINT::default();
        let object = unsafe { self.0.BeginDraw::<T>(None, &mut offset)? };
        Ok((object, (offset.x, offset.y)))
    }

    /// Finishes drawing begun with [`begin_draw`](Self::begin_draw) and presents the
    /// surface contents.
    pub fn end_draw(&self) -> Result<()> {
        unsafe { self.0.EndDraw().ok() }
    }

    /// Resizes the surface to `width`x`height` pixels.
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

/// The base type shared by every composition surface — one a [`CompositionSurfaceBrush`]
/// can paint with.
#[derive(Clone)]
pub struct CompositionSurface(pub(crate) bindings::ICompositionSurface);

impl Sealed for CompositionSurface {}

/// The base type is itself paintable, which is what lets content the compositor did not
/// allocate — a composition swapchain adopted through
/// [`Compositor::create_surface_for_handle`](crate::Compositor::create_surface_for_handle)
/// — be used exactly like a surface it did.
impl Surface for CompositionSurface {
    fn as_surface(&self) -> CompositionSurface {
        self.clone()
    }
}

/// Content a [`CompositionSurfaceBrush`] can paint a visual with: a
/// [`CompositionDrawingSurface`] holding rasterized content, a
/// [`CompositionVisualSurface`] capturing a live visual subtree, or a bare
/// [`CompositionSurface`] adopted from a handle.
///
/// This trait is sealed: only the surface types in this crate implement it.
pub trait Surface: Sealed {
    /// Returns this surface as the shared [`CompositionSurface`] base type.
    fn as_surface(&self) -> CompositionSurface;
}

/// A surface whose content is a live visual subtree rather than pixels drawn into it.
///
/// It captures the visual set by [`set_source_visual`](Self::set_source_visual) — and its
/// descendants — as they are currently composed, so painting a brush with it re-uses an
/// already-composed subtree instead of re-rasterizing its content. The captured region is
/// the rectangle at [`set_source_offset`](Self::set_source_offset) of
/// [`set_source_size`](Self::set_source_size), in the source visual's own coordinate space.
///
/// **It captures content, not the source visual's own transform.** Scaling the source
/// visual does not scale what lands in the surface; a DIP scale has to live on the geometry
/// inside it.
#[derive(Clone)]
pub struct CompositionVisualSurface(pub(crate) bindings::CompositionVisualSurface);

impl CompositionVisualSurface {
    /// Sets the visual whose subtree this surface captures.
    pub fn set_source_visual(&self, visual: &Visual) {
        self.0.SetSourceVisual(&visual.0).unwrap();
    }

    /// Sets the top-left corner of the captured region, in the source visual's coordinate
    /// space.
    pub fn set_source_offset(&self, offset: Vector2) {
        self.0.SetSourceOffset(offset).unwrap();
    }

    /// Sets the size of the captured region, in the source visual's coordinate space.
    pub fn set_source_size(&self, size: Vector2) {
        self.0.SetSourceSize(size).unwrap();
    }
}

impl Sealed for CompositionVisualSurface {}

impl Surface for CompositionVisualSurface {
    fn as_surface(&self) -> CompositionSurface {
        CompositionSurface(self.0.cast().unwrap())
    }
}

/// How a [`CompositionSurfaceBrush`] fits its surface into the area it paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stretch {
    /// Do not scale the surface; paint it at its natural size.
    None,
    /// Scale each axis independently to fill the area, ignoring aspect ratio.
    Fill,
    /// Scale uniformly until the surface fits inside the area, letterboxing the remainder.
    /// This is composition's default.
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

/// Brush that paints a visual with a [`Surface`].
#[derive(Clone)]
pub struct CompositionSurfaceBrush(pub(crate) bindings::CompositionSurfaceBrush);

impl CompositionSurfaceBrush {
    /// Sets how the surface is fitted into the area the brush paints.
    ///
    /// The composition default is [`Stretch::Uniform`], which letterboxes the surface
    /// whenever its aspect ratio differs from the painted area's. A caller that has already
    /// sized the surface to the area it paints — an atlas, a glyph run, a gradient ramp —
    /// wants [`Stretch::Fill`], so the surface maps onto the area one-to-one.
    pub fn set_stretch(&self, stretch: Stretch) {
        self.0.SetStretch(stretch.into()).unwrap();
    }

    /// Sets where the surface sits within the painted area when it does not fill it, as a
    /// fraction of the leftover space on each axis.
    ///
    /// Composition's default is `0.5` on both — centred — which is rarely what a caller
    /// wants and is easy to mistake for a placement bug. Anchoring at `(0.0, 0.0)` puts the
    /// surface's top-left on the painted area's, which is the frame
    /// [`set_source_transform`](Self::set_source_transform) measures its offsets from.
    pub fn set_alignment_ratio(&self, horizontal: f32, vertical: f32) {
        self.0.SetHorizontalAlignmentRatio(horizontal).unwrap();
        self.0.SetVerticalAlignmentRatio(vertical).unwrap();
    }

    /// Positions and scales the surface within the area this brush paints, so a sprite can
    /// show one *region* of a larger surface.
    ///
    /// Composition applies the stretch and alignment first and this transform second, in
    /// the painted sprite's own coordinate space. So to show the region at `origin` pixels
    /// of an atlas at one surface pixel per physical pixel, pair [`Stretch::None`] and a
    /// `(0.0, 0.0)` [alignment ratio](Self::set_alignment_ratio) with a `scale` of one over
    /// the DIP→pixel factor and an `offset` of the region's origin negated and carried
    /// through that same scale.
    pub fn set_source_transform(&self, offset: Vector2, scale: Vector2) {
        let brush: bindings::ICompositionSurfaceBrush2 = self.0.cast().unwrap();
        brush.SetScale(scale).unwrap();
        brush.SetOffset(offset).unwrap();
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

impl Compositor {
    /// Creates a surface that captures a live visual subtree.
    pub fn create_visual_surface(&self) -> CompositionVisualSurface {
        let compositor: bindings::ICompositorWithVisualSurface = self.0.cast().unwrap();
        CompositionVisualSurface(compositor.CreateVisualSurface().unwrap())
    }

    /// Adopts a composition surface **handle** as content this compositor can paint with —
    /// the seam for buffers the app presents itself rather than draws into a surface the
    /// compositor allocated.
    ///
    /// The handle is the one `DCompositionCreateSurfaceHandle` mints and a presentation
    /// manager presents into. Whatever is presented on it appears wherever the returned
    /// surface is painted, with no involvement from this thread per frame: the producer
    /// presents, and the compositor samples.
    ///
    /// # Safety
    ///
    /// `handle` must be a live composition surface handle. The compositor does not take
    /// ownership — the caller keeps it valid for as long as the returned surface, or any
    /// brush painted with it, is in the visual tree.
    ///
    /// The producer that minted it is normally its owner and closes it when it is dropped,
    /// so binding one outlives nothing on its own: if the surface must survive the producer,
    /// **duplicate the handle** and adopt the duplicate. Two owners of one handle is a
    /// double close, which is why this takes a borrow and not a wrapper that frees.
    pub unsafe fn create_surface_for_handle(
        &self,
        handle: *mut core::ffi::c_void,
    ) -> Result<CompositionSurface> {
        let interop: bindings::ICompositorInterop = self.0.cast()?;
        let surface = unsafe { interop.CreateCompositionSurfaceForHandle(handle)? };
        Ok(CompositionSurface(surface))
    }
}
