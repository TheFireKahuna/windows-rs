use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use windows_reactor::*;

/// How [`DrawContext::convert_dips_to_pixels`] rounds a fractional pixel result.
/// Mirrors Win2D's `CanvasDpiRounding`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DpiRounding {
    /// Round down to the nearest whole pixel.
    Floor,
    /// Round to the nearest whole pixel.
    Round,
    /// Round up to the nearest whole pixel.
    Ceiling,
}

/// Per-frame draw context.
pub struct DrawContext<'a> {
    session: DrawingSession<'a>,
    device: &'a GpuDevice,
    /// Width of the drawing surface, in device-independent pixels.
    pub width: f32,
    /// Height of the drawing surface, in device-independent pixels.
    pub height: f32,
    dpi: f32,
    changed: bool,
    update: Rect,
}

impl<'a> DrawContext<'a> {
    /// Builds a context over a session whose bracket is owned elsewhere (a
    /// [`SurfaceImage`]/[`VirtualSurfaceImage`] update). `update` is the region
    /// being (re)drawn, in DIPs, in surface-local coordinates.
    pub(crate) fn new(
        session: DrawingSession<'a>,
        device: &'a GpuDevice,
        width: f32,
        height: f32,
        dpi: f32,
        changed: bool,
        update: Rect,
    ) -> Self {
        Self {
            session,
            device,
            width,
            height,
            dpi,
            changed,
            update,
        }
    }

    /// Returns the GPU device backing this context.
    pub fn device(&self) -> &GpuDevice {
        self.device
    }

    /// The dots-per-inch the surface renders at (96 = 100%). Mirrors
    /// `CanvasControl.Dpi`.
    pub fn dpi(&self) -> f32 {
        self.dpi
    }

    /// Converts a length in device-independent pixels (DIPs) to physical pixels
    /// at this context's DPI, rounded per `rounding`. Mirrors
    /// `CanvasControl.ConvertDipsToPixels`.
    pub fn convert_dips_to_pixels(&self, dips: f32, rounding: DpiRounding) -> f32 {
        let px = dips * self.dpi / 96.0;
        match rounding {
            DpiRounding::Floor => px.floor(),
            DpiRounding::Round => px.round(),
            DpiRounding::Ceiling => px.ceil(),
        }
    }

    /// Converts a length in physical pixels to device-independent pixels (DIPs)
    /// at this context's DPI. Mirrors `CanvasControl.ConvertPixelsToDips`.
    pub fn convert_pixels_to_dips(&self, pixels: f32) -> f32 {
        pixels * 96.0 / self.dpi
    }

    /// Returns `true` on the first frame after device loss or resize.
    pub fn device_changed(&self) -> bool {
        self.changed
    }

    /// The region being (re)drawn this call, in DIPs (surface-local
    /// coordinates). For a full redraw this is the whole surface; for a
    /// dirty-rect update it is just the changed rectangle. Drawing outside it has
    /// no effect, so callers can clip work to it for performance.
    pub fn update_rect(&self) -> Rect {
        self.update
    }

    /// Clears the surface to the given color.
    pub fn clear(&self, color: ColorF) {
        self.session.clear(color);
    }
}

impl<'a> std::ops::Deref for DrawContext<'a> {
    type Target = DrawingSession<'a>;
    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

struct RenderState {
    device: GpuDevice,
    chain: SwapChain,
    panel: SwapChainPanelHandle,
    scale: f32,
    _rendering: Rendering,
    _scale_revoker: Option<EventRevoker>,
}

/// DIP length to physical pixels for surface sizing, guarding against a zero
/// (a swap chain must be at least 1x1).
fn surface_pixels(dip: f32, scale: f32) -> u32 {
    ((dip * scale) as u32).max(1)
}

/// Debug-build warning for the one collapse the layout-driven host can't solve:
/// the host fills the axes its parent *constrains*, but an axis the parent leaves
/// open (a vertical `StackPanel`'s height, an `Auto` grid track, a `ScrollViewer`)
/// has nothing to size to while the inner `Image` is empty, so it collapses to
/// zero (e.g. `400x0`) and nothing is drawn. This is the same requirement Win2D's
/// `CanvasControl` has — give it a size on the open axis. `0x0` is the normal
/// pre-layout state and is not reported. Compiles to nothing in release.
fn warn_if_collapsed(_what: &str, _w: u32, _h: u32) {
    #[cfg(debug_assertions)]
    if (_w == 0) != (_h == 0) {
        eprintln!(
            "windows-canvas: {_what} laid out at {_w}x{_h}: the host fills the axes \
             its parent constrains, but an axis the parent leaves open (a vertical \
             StackPanel's height, an Auto grid track, a ScrollViewer) collapses to \
             zero. Set `.height(..)`/`.min_height(..)` (or `.width(..)`/`.min_width(..)`) \
             on the returned element, or place it in a *-sized grid track. (Win2D's \
             CanvasControl needs the same in a StackPanel.)"
        );
    }
}

impl RenderState {
    fn rebuild(&mut self, pixel_width: u32, pixel_height: u32) -> bool {
        let Ok(device) = GpuDevice::new_or_warp() else {
            return false;
        };
        let Ok(mut chain) = device.create_swap_chain(pixel_width, pixel_height) else {
            return false;
        };
        let dpi = 96.0 * self.scale;
        chain.set_dpi(dpi, dpi);
        chain.set_composition_scale(self.scale, self.scale);
        let _ = self.panel.set_swap_chain(chain.raw_swap_chain());
        self.device = device;
        self.chain = chain;
        true
    }
}

/// Create an animated canvas that calls `draw` every frame.
///
/// Handles device creation, swap chain management, resize, and device-lost
/// recovery automatically.
///
/// ```ignore
/// animated_canvas(|ctx| {
///     ctx.clear(ColorF::CORNFLOWER_BLUE);
///     ctx.fill_ellipse(&ellipse, &brush);
/// })
/// ```
pub fn animated_canvas(draw: impl Fn(&DrawContext<'_>) + 'static) -> SwapChainPanel {
    let state: Rc<RefCell<Option<RenderState>>> = Rc::new(RefCell::new(None));
    let size: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((0.0, 0.0)));
    let scale: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));
    let changed: Rc<Cell<bool>> = Rc::new(Cell::new(true));
    let draw = Rc::new(draw);

    let ready_state = state.clone();
    let ready_size = size.clone();
    let ready_scale = scale.clone();
    let ready_changed = changed.clone();
    swap_chain_panel()
        .on_mounted(move |panel| {
            let s = panel.composition_scale().map_or(1.0, |(x, _)| x);
            ready_scale.set(s);

            let (w, h) = ready_size.get();
            let pw = surface_pixels(w, s);
            let ph = surface_pixels(h, s);

            let Ok(device) = GpuDevice::new_or_warp() else {
                return;
            };
            let Ok(mut chain) = device.create_swap_chain(pw, ph) else {
                return;
            };
            let dpi = 96.0 * s;
            chain.set_dpi(dpi, dpi);
            chain.set_composition_scale(s, s);
            let _ = panel.set_swap_chain(chain.raw_swap_chain());

            // Listen for scale changes.
            let sc_size = ready_size.clone();
            let sc_scale = ready_scale.clone();
            let sc_state = ready_state.clone();
            let sc_gen = ready_changed.clone();
            let scale_revoker = panel
                .on_composition_scale_changed(move |new_s, _| {
                    sc_scale.set(new_s);
                    let (w, h) = sc_size.get();
                    let pw = surface_pixels(w, new_s);
                    let ph = surface_pixels(h, new_s);
                    let mut borrow = sc_state.borrow_mut();
                    if let Some(rs) = borrow.as_mut() {
                        rs.scale = new_s;
                        let _ = rs.chain.resize(pw, ph);
                        let dpi = 96.0 * new_s;
                        rs.chain.set_dpi(dpi, dpi);
                        rs.chain.set_composition_scale(new_s, new_s);
                        sc_gen.set(true);
                    }
                })
                .ok();

            let render_state = ready_state.clone();
            let render_size = ready_size.clone();
            let render_draw = draw.clone();
            let render_changed = ready_changed.clone();
            let Ok(rendering) = on_rendering(move || {
                let mut borrow = render_state.borrow_mut();
                if let Some(rs) = borrow.as_mut() {
                    let (w, h) = render_size.get();
                    if w <= 0.0 || h <= 0.0 {
                        return;
                    }
                    let Ok(session) = rs.chain.begin_draw() else {
                        return;
                    };
                    let ctx = DrawContext {
                        session,
                        device: &rs.device,
                        width: w,
                        height: h,
                        dpi: 96.0 * rs.scale,
                        changed: render_changed.replace(false),
                        update: Rect::from_xywh(0.0, 0.0, w, h),
                    };
                    render_draw(&ctx);
                    drop(ctx);

                    match rs.chain.present() {
                        Ok(true) => {}
                        Ok(false) => {
                            let pw = surface_pixels(w, rs.scale);
                            let ph = surface_pixels(h, rs.scale);
                            if rs.rebuild(pw, ph) {
                                render_changed.set(true);
                            }
                        }
                        Err(_) => {}
                    }
                }
            }) else {
                return;
            };

            *ready_state.borrow_mut() = Some(RenderState {
                device,
                chain,
                panel,
                scale: s,
                _rendering: rendering,
                _scale_revoker: scale_revoker,
            });
        })
        .on_resize(move |w, h| {
            size.set((w as f32, h as f32));
            let s = scale.get();
            let pw = surface_pixels(w as f32, s);
            let ph = surface_pixels(h as f32, s);
            let mut borrow = state.borrow_mut();
            if let Some(rs) = borrow.as_mut() {
                let _ = rs.chain.resize(pw, ph);
                changed.set(true);
            }
        })
}

/// Create an `Image` element backed by a [`SurfaceImage`] that is drawn with
/// `draw` and kept correctly sized and DPI-crisp automatically.
///
/// Call this from a render function: it tracks the host's layout size (via
/// `on_size_changed`) and the window DPI (via [`RenderCx::use_dpi`]), and
/// rebuilds and redraws the surface whenever either changes or the device is
/// lost. The surface is allocated at the host's pixel size and stretched to
/// fill it, so it stays sharp at any scale.
///
/// The returned element is a layout-driven `Border` host wrapping the `Image`
/// (the reactor port of how Win2D's `CanvasControl` hosts its surface in a
/// `UserControl`), so it **fills the width** of a star `Grid` cell or a stretched
/// parent natively. Only an axis the parent leaves *open* needs a hint: in a
/// vertical `StackPanel` / `Auto` row give it a [`height`](windows_reactor::ElementExt::height)
/// (or [`min_height`](windows_reactor::ElementExt::min_height)) — the same
/// requirement `CanvasControl` has in a `StackPanel`.
///
/// `draw` is called once per (re)build — suited to static or event-driven
/// content (the `SurfaceImageSource` model). For per-frame animation use
/// [`animated_canvas`]; for content larger than the screen use
/// [`virtual_surface_image`].
pub fn surface_image(cx: &mut RenderCx, draw: impl Fn(&DrawContext) + 'static) -> Element {
    let dpi = cx.use_dpi() as f32;
    let (size, set_size) = cx.use_state::<(u32, u32)>((0, 0));
    let (generation, set_generation) = cx.use_state::<u32>(0);
    let (source, set_source) = cx.use_state::<Option<SurfaceImageSource>>(None);
    let device_ref = cx.use_ref::<Option<GpuDevice>>(None);
    let revoker = cx.use_ref::<Option<EventRevoker>>(None);
    let draw = Rc::new(draw);
    let (w, h) = size;

    cx.use_effect((dpi.to_bits(), size, generation), move || {
        if w == 0 || h == 0 {
            warn_if_collapsed("surface_image", w, h);
            set_source.call(None);
            return;
        }
        // Reuse the device across resizes; only recreate it after a loss.
        let device = {
            let mut slot = device_ref.borrow_mut();
            if slot.is_none() {
                *slot = GpuDevice::new_or_warp().ok();
            }
            slot.clone()
        };
        let Some(device) = device else {
            set_source.call(None);
            return;
        };
        let Ok(image) = SurfaceImage::new(&device, w as f32, h as f32, dpi) else {
            set_source.call(None);
            return;
        };
        match image.draw_all(|ctx| draw(ctx)) {
            Ok(()) => set_source.call(Some(image.surface())),
            Err(e) if is_device_lost(e.code()) => {
                // Drop the lost device and retry with a fresh one.
                *device_ref.borrow_mut() = None;
                set_generation.call(generation.wrapping_add(1));
            }
            Err(_) => set_source.call(None),
        }
    });

    // Faithful port of Win2D's `CanvasControl`: an inner `Image` (Stretch::Fill)
    // that only *displays* the surface, hosted in a layout-driven `Border` whose
    // own `SizeChanged` drives the surface size. `Image` is content-sized — it
    // adopts its `Source`'s natural size, so a null/not-yet-created source has no
    // width to report and could never bootstrap. The `Border` is layout-driven:
    // it fills the space its parent offers regardless of its child, so it reports
    // the real available size and the surface is created to match. (Win2D
    // measures its `UserControl` host for the same reason, not the inner `Image`.)
    let inner = Image::new(source.into()).stretch(Stretch::Fill);
    Border::new(inner)
        .on_mounted(move |handle| {
            let set_size = set_size.clone();
            if let Ok(rev) = handle.on_size_changed(move |w, h| {
                set_size.call((w.round().max(0.0) as u32, h.round().max(0.0) as u32));
            }) {
                revoker.set(Some(rev));
            }
        })
        .into()
}

/// Create an `Image` element backed by a [`VirtualSurfaceImage`] of the given
/// content size (DIPs), drawn with `draw`.
///
/// The framework virtualizes the surface: `draw` is called for each region as it
/// becomes visible (place the element in a `ScrollViewer` to pan a large canvas).
/// DPI is tracked via [`RenderCx::use_dpi`], and the surface is rebuilt when the
/// DPI or content size changes. The draw handler receives a [`DrawContext`] whose
/// [`update_rect`](DrawContext::update_rect) is the region to paint.
pub fn virtual_surface_image(
    cx: &mut RenderCx,
    content_width: f32,
    content_height: f32,
    draw: impl Fn(&DrawContext) + 'static,
) -> Element {
    let dpi = cx.use_dpi() as f32;
    let (source, set_source) = cx.use_state::<Option<VirtualSurfaceImageSource>>(None);
    // Holds the live `VirtualSurfaceImage` so its update registration stays
    // registered; dropping it (on rebuild or unmount) unregisters.
    let surface_ref = cx.use_ref::<Option<VirtualSurfaceImage>>(None);
    let device_ref = cx.use_ref::<Option<GpuDevice>>(None);
    let draw = Rc::new(draw);

    cx.use_effect(
        (
            dpi.to_bits(),
            content_width.to_bits(),
            content_height.to_bits(),
        ),
        move || {
            let device = {
                let mut slot = device_ref.borrow_mut();
                if slot.is_none() {
                    *slot = GpuDevice::new_or_warp().ok();
                }
                slot.clone()
            };
            let Some(device) = device else {
                set_source.call(None);
                return;
            };
            let Ok(image) = VirtualSurfaceImage::new(&device, content_width, content_height, dpi)
            else {
                set_source.call(None);
                return;
            };
            if image.on_draw(move |ctx| draw(ctx)).is_err() {
                set_source.call(None);
                return;
            }
            set_source.call(Some(image.surface()));
            // Keep the image (and its registration) alive across renders.
            surface_ref.set(Some(image));
        },
    );

    Image::new(source.into()).stretch(Stretch::Fill).into()
}

mod builder;
mod device;
mod painter;

pub use builder::{
    CreateReason, ResourceCx, ResourcePainterBuilder, SurfacePainterBuilder, surface_painter,
};
pub use device::DeviceSource;
pub use painter::{FrameTiming, PumpHold, Step, SurfacePainter};
