use super::device::{acquire_device, reset_device};
use super::painter::PaintSurface;
use super::*;
use std::cell::Cell;
use std::rc::Rc;

/// Why a [`create_resources`](SurfacePainterBuilder::create_resources) callback is
/// being (re)invoked. Mirrors Win2D's `CanvasCreateResourcesReason`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CreateReason {
    /// The control's first resource creation.
    FirstTime,
    /// The GPU device was recreated (e.g. after device loss); any resources held
    /// against the old device are gone and must be rebuilt.
    NewDevice,
    /// The DPI changed; rebuild any DPI-dependent resources.
    DpiChanged,
}

/// The resource-creation context handed to a
/// [`create_resources`](SurfacePainterBuilder::create_resources) callback — Win2D's
/// `ICanvasResourceCreatorWithDpi`. Derefs to the [`DrawingSession`] so you can
/// build brushes, gradients, and bitmaps; also exposes the
/// [`device`](Self::device) and [`dpi`](Self::dpi). Don't issue drawing commands
/// here — that is what [`draw`](SurfacePainterBuilder::draw) is for.
pub struct ResourceCx<'a> {
    ctx: &'a DrawContext<'a>,
}

impl<'a> ResourceCx<'a> {
    /// The GPU device the resources will live on.
    pub fn device(&self) -> &GpuDevice {
        self.ctx.device()
    }

    /// The DPI the surface renders at (96 = 100%).
    pub fn dpi(&self) -> f32 {
        self.ctx.dpi()
    }

    /// Converts DIPs to physical pixels at the current DPI.
    pub fn convert_dips_to_pixels(&self, dips: f32, rounding: DpiRounding) -> f32 {
        self.ctx.convert_dips_to_pixels(dips, rounding)
    }

    /// Converts physical pixels to DIPs at the current DPI.
    pub fn convert_pixels_to_dips(&self, pixels: f32) -> f32 {
        self.ctx.convert_pixels_to_dips(pixels)
    }
}

impl<'a> std::ops::Deref for ResourceCx<'a> {
    type Target = DrawingSession<'a>;
    fn deref(&self) -> &Self::Target {
        &self.ctx.session
    }
}

// Accumulated configuration for a `surface_painter`, built up by the builder and
// consumed by `build_painter`.
struct PainterConfig {
    clear_color: Option<ColorF>,
    device: DeviceSource,
    force_software: bool,
    opaque: bool,
}

/// Builder for a [`SurfacePainter`] — the reactor's faithful port of Win2D's
/// `CanvasControl`: immediate-mode 2D drawing onto a `CanvasImageSource`, with a
/// two-phase resource model, an auto-clear color, on-demand invalidation, and
/// automatic device / DPI / size / device-loss handling.
///
/// Start with [`surface_painter`], chain the configuration you need, then finish
/// with [`draw`](Self::draw) (no per-control resources) or
/// [`create_resources`](Self::create_resources) followed by
/// [`draw`](ResourcePainterBuilder::draw) (the two-phase model).
///
/// ```ignore
/// let painter = surface_painter(cx)
///     .clear_color(ColorF::CORNFLOWER_BLUE)
///     .create_resources(|rc, _reason| {
///         Ok(rc.create_solid_brush(ColorF::RED)?)
///     })
///     .draw(|ctx, red| {
///         ctx.draw_text("Hello, world!", &format, &rect, red);
///     });
/// painter.element() // place it in the render tree
/// ```
///
/// The hosting [`element`](SurfacePainter::element) is a layout-driven `Border`
/// wrapping the surface `Image` — the reactor port of how `CanvasControl` hosts
/// its surface in a `UserControl` and measures *that*, not the inner `Image`. So
/// it **fills the width** of a star `Grid` cell or a stretched parent natively,
/// no size hint needed. Only an axis the parent leaves *open* still needs one: in
/// a vertical `StackPanel` / `Auto` row the height has nothing to size to while
/// the surface is empty, so give it a
/// [`height`](windows_reactor::ElementExt::height) /
/// [`min_height`](windows_reactor::ElementExt::min_height) — the same requirement
/// `CanvasControl` has in a `StackPanel`.
pub struct SurfacePainterBuilder<'a> {
    cx: &'a mut RenderCx,
    config: PainterConfig,
}

impl<'a> SurfacePainterBuilder<'a> {
    /// The color the surface is cleared to before each [`draw`](Self::draw). Mirrors
    /// `CanvasControl.ClearColor` (default fully transparent). Pass `None` to skip
    /// the auto-clear and paint every pixel yourself.
    pub fn clear_color(mut self, color: impl Into<Option<ColorF>>) -> Self {
        self.config.clear_color = color.into();
        self
    }

    /// Which device to draw with (default [`DeviceSource::Shared`]).
    pub fn device(mut self, device: DeviceSource) -> Self {
        self.config.device = device;
        self
    }

    /// Force a software (WARP) renderer. Mirrors `CanvasControl.ForceSoftwareRenderer`.
    /// Only affects [`Shared`](DeviceSource::Shared)/[`Owned`](DeviceSource::Owned)
    /// devices, not a [`Custom`](DeviceSource::Custom) one.
    pub fn force_software(mut self, on: bool) -> Self {
        self.config.force_software = on;
        self
    }

    /// Allocate the surface without an alpha channel (cheaper to composite when the
    /// content fully covers its bounds; you must paint every pixel).
    pub fn opaque(mut self, on: bool) -> Self {
        self.config.opaque = on;
        self
    }

    /// Add a two-phase resource builder (Win2D's `CreateResources` event). It runs
    /// once before the first [`draw`](ResourcePainterBuilder::draw), and again
    /// whenever the device is recreated or the DPI changes (see [`CreateReason`]);
    /// the resources it returns are handed to every `draw` by reference, so they are
    /// built once and reused across frames instead of rebuilt each frame.
    pub fn create_resources<R: 'static>(
        self,
        create: impl Fn(&ResourceCx, CreateReason) -> Result<R> + 'static,
    ) -> ResourcePainterBuilder<'a, R> {
        ResourcePainterBuilder {
            cx: self.cx,
            config: self.config,
            create: Box::new(create),
        }
    }

    /// Finish the control with an immediate-mode draw callback that needs no
    /// per-control resources. Runs on the first frame and on every
    /// [`invalidate`](SurfacePainter::invalidate) / [`animate`](SurfacePainter::animate)
    /// frame thereafter.
    pub fn draw(self, draw: impl Fn(&DrawContext) + 'static) -> SurfacePainter {
        build_painter(
            self.cx,
            self.config,
            |_, _| Ok(()),
            move |ctx, _: &()| draw(ctx),
        )
    }
}

/// Builder stage after [`create_resources`](SurfacePainterBuilder::create_resources):
/// finish it with [`draw`](Self::draw), whose callback receives the created
/// resources by reference.
pub struct ResourcePainterBuilder<'a, R> {
    cx: &'a mut RenderCx,
    config: PainterConfig,
    create: Box<dyn Fn(&ResourceCx, CreateReason) -> Result<R>>,
}

impl<'a, R: 'static> ResourcePainterBuilder<'a, R> {
    /// See [`SurfacePainterBuilder::clear_color`].
    pub fn clear_color(mut self, color: impl Into<Option<ColorF>>) -> Self {
        self.config.clear_color = color.into();
        self
    }

    /// See [`SurfacePainterBuilder::device`].
    pub fn device(mut self, device: DeviceSource) -> Self {
        self.config.device = device;
        self
    }

    /// See [`SurfacePainterBuilder::force_software`].
    pub fn force_software(mut self, on: bool) -> Self {
        self.config.force_software = on;
        self
    }

    /// See [`SurfacePainterBuilder::opaque`].
    pub fn opaque(mut self, on: bool) -> Self {
        self.config.opaque = on;
        self
    }

    /// Finish the control. `draw` receives the [`DrawContext`] and the resources
    /// built by [`create_resources`](SurfacePainterBuilder::create_resources),
    /// already cleared to the [`clear_color`](Self::clear_color).
    pub fn draw(self, draw: impl Fn(&DrawContext, &R) + 'static) -> SurfacePainter {
        build_painter(self.cx, self.config, self.create, draw)
    }
}

/// Begin building a [`SurfacePainter`] — the faithful reactor port of Win2D's
/// `CanvasControl`. See [`SurfacePainterBuilder`].
pub fn surface_painter(cx: &mut RenderCx) -> SurfacePainterBuilder<'_> {
    SurfacePainterBuilder {
        cx,
        config: PainterConfig {
            clear_color: Some(ColorF::TRANSPARENT),
            device: DeviceSource::Shared,
            force_software: false,
            opaque: false,
        },
    }
}

/// Wire up the hooks and the reconciler-driven effect that drive a painter. Generic
/// over the resource type `R`, which never appears on [`SurfacePainter`] itself —
/// the per-frame draw and the resource slot are captured here and type-erased.
fn build_painter<R: 'static>(
    cx: &mut RenderCx,
    config: PainterConfig,
    create: impl Fn(&ResourceCx, CreateReason) -> Result<R> + 'static,
    draw: impl Fn(&DrawContext, &R) + 'static,
) -> SurfacePainter {
    let dpi = cx.use_dpi() as f32;
    let (size, set_size) = cx.use_state::<(u32, u32)>((0, 0));
    let (generation, set_generation) = cx.use_async_state::<u32>(0);
    let (source, set_source) = cx.use_state::<Option<SurfaceImageSource>>(None);
    let owned_device = cx.use_ref::<Option<GpuDevice>>(None);
    let owned_gen = cx.use_ref::<u64>(0);
    let resources = cx.use_ref::<Option<R>>(None);
    // (device generation, dpi bits) the current resources were built against — used
    // to decide whether (and why) to rebuild them.
    let last_built = cx.use_ref::<Option<(u64, u32)>>(None);
    let painter_ref = cx.use_ref::<Option<SurfacePainter>>(None);

    // One persistent handle for the component's lifetime. A single `borrow_mut`
    // creates-on-first-render and clones out; taking the clone from `borrow()` and
    // inserting under `borrow_mut()` in the same statement would alias the cell
    // (the `borrow()` temporary outlives the statement) and panic on first render.
    let painter = painter_ref
        .borrow_mut()
        .get_or_insert_with(SurfacePainter::new)
        .clone();

    let create = Rc::new(create);
    let draw = Rc::new(draw);
    let clear = config.clear_color;

    // The type-erased per-frame draw used by imperative repaints: clear, then draw
    // with the current resources (skipping if a build is pending). Refreshed every
    // render so redraws use the latest callbacks and state.
    *painter.inner.draw.borrow_mut() = Rc::new({
        let resources = resources.clone();
        let draw = draw.clone();
        move |ctx: &DrawContext| {
            if let Some(c) = clear {
                ctx.clear(c);
            }
            if let Some(r) = resources.borrow().as_ref() {
                draw(ctx, r);
            }
        }
    });

    *painter.inner.source.borrow_mut() = source;
    *painter.inner.set_size.borrow_mut() = Some(set_size);
    *painter.inner.request_rebuild.borrow_mut() = Box::new({
        let device_source = config.device.clone();
        let owned_device = owned_device.clone();
        let resources = resources.clone();
        let set_generation = set_generation.clone();
        let force_software = config.force_software;
        move || {
            // A repaint hit a lost device: drop it (so a fresh one is acquired),
            // discard the now-dead resources, and bump the generation to re-run the
            // effect. `last_built` is kept so the rebuild is seen as a `NewDevice`.
            reset_device(&device_source, force_software, &owned_device);
            *resources.borrow_mut() = None;
            set_generation.call(generation.wrapping_add(1));
        }
    });

    let (w, h) = size;
    let device_source = config.device.clone();
    let force_software = config.force_software;
    let opaque = config.opaque;
    cx.use_effect((dpi.to_bits(), size, generation), {
        // The effect is the last user of these handles, so it takes them by move;
        // `painter` is still needed below, so it alone is cloned.
        let painter = painter.clone();
        move || {
            if w == 0 || h == 0 {
                warn_if_collapsed("surface_painter", w, h);
                *painter.inner.surface.borrow_mut() = None;
                *painter.inner.device.borrow_mut() = None;
                painter.inner.ready.set(false);
                set_source.call(None);
                return;
            }

            // Acquire the device per policy; the generation changes when it is
            // recreated, which distinguishes a `NewDevice` rebuild from a resize.
            let Some((device, device_gen)) =
                acquire_device(&device_source, force_software, &owned_device, &owned_gen)
            else {
                *painter.inner.surface.borrow_mut() = None;
                *painter.inner.device.borrow_mut() = None;
                painter.inner.ready.set(false);
                set_source.call(None);
                return;
            };
            painter.inner.dpi.set(dpi);
            *painter.inner.device.borrow_mut() = Some(device.clone());

            // (Re)build the surface at the element's pixel size. The variant is
            // chosen at runtime from the captured host: a DirectComposition
            // child-visual surface on the dcomp backend, else a XAML
            // `SurfaceImageSource`. A device-lost failure here triggers the same
            // recovery as a lost repaint.
            let host = painter.inner.host.borrow().clone();
            let built = {
                let set_generation = set_generation.clone();
                PaintSurface::build(
                    host.as_ref(),
                    &painter.inner.pending_surface,
                    &device,
                    device_gen,
                    w as f32,
                    h as f32,
                    dpi,
                    opaque,
                    move || set_generation.call(generation.wrapping_add(1)),
                )
            };
            let surface = match built {
                // The backend is hosting the surface; `ready` re-runs this effect
                // once it exists. Same standing-by state as a lost device.
                Ok(None) => {
                    *painter.inner.surface.borrow_mut() = None;
                    painter.inner.ready.set(false);
                    return;
                }
                Ok(Some(surface)) => surface,
                Err(e) => {
                    if is_device_lost(e.code()) {
                        reset_device(&device_source, force_software, &owned_device);
                        *resources.borrow_mut() = None;
                        set_generation.call(generation.wrapping_add(1));
                    }
                    *painter.inner.surface.borrow_mut() = None;
                    painter.inner.ready.set(false);
                    set_source.call(None);
                    return;
                }
            };

            // Decide whether the resources must be (re)built and why.
            let dpi_bits = dpi.to_bits();
            let reason = match *last_built.borrow() {
                None => Some(CreateReason::FirstTime),
                Some((_, d)) if d != dpi_bits => Some(CreateReason::DpiChanged),
                Some((g, _)) if g != device_gen => Some(CreateReason::NewDevice),
                Some(_) => None,
            };
            let need = reason.is_some() || resources.borrow().is_none();

            // First paint: build resources if needed (inside a live session so
            // brush/bitmap creation is valid), then clear and draw — all in one
            // native `BeginDraw`/`EndDraw`.
            let lost = Cell::new(false);
            let failed = Cell::new(false);
            let painted = surface.draw_region(None, true, |ctx| {
                if need {
                    let rcx = ResourceCx { ctx };
                    match create(&rcx, reason.unwrap_or(CreateReason::FirstTime)) {
                        Ok(r) => {
                            *resources.borrow_mut() = Some(r);
                            *last_built.borrow_mut() = Some((device_gen, dpi_bits));
                        }
                        Err(e) if is_device_lost(e.code()) => {
                            lost.set(true);
                            return;
                        }
                        Err(_) => {
                            failed.set(true);
                            return;
                        }
                    }
                }
                if let Some(c) = clear {
                    ctx.clear(c);
                }
                if let Some(r) = resources.borrow().as_ref() {
                    draw(ctx, r);
                }
            });

            let device_lost =
                lost.get() || painted.as_ref().is_err_and(|e| is_device_lost(e.code()));
            if device_lost {
                reset_device(&device_source, force_software, &owned_device);
                *resources.borrow_mut() = None;
                *painter.inner.surface.borrow_mut() = None;
                painter.inner.ready.set(false);
                set_source.call(None);
                set_generation.call(generation.wrapping_add(1));
                return;
            }

            // Ready iff resources built and the frame painted. On a non-fatal
            // resource failure we still attach the cleared surface so layout is
            // stable, but stay not-ready so `draw` is skipped.
            painter.inner.ready.set(!failed.get() && painted.is_ok());
            // The `Image` source on the WinUI path; `None` on dcomp, where the
            // composition sprite shows the content (the host `Image` stays empty).
            let new_source = surface.image_source();
            *painter.inner.surface.borrow_mut() = Some(surface);
            set_source.call(new_source);
        }
    });

    painter
}
