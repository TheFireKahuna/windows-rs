use super::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};
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

/// Timing for one animation frame, passed to a [`SurfacePainter::animate`] step.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FrameTiming {
    /// Time since the previous animation frame (zero on the first).
    pub delta: Duration,
    /// Time since the animation began.
    pub total: Duration,
}

/// What an animation step wants to happen next. Returned from the
/// [`SurfacePainter::animate`] closure each frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Step {
    /// Keep animating and redraw the whole surface this frame.
    Redraw,
    /// Keep animating and redraw just this region (DIPs, surface-local).
    RedrawRect(Rect),
    /// Keep animating but don't redraw this frame.
    Skip,
    /// The animation has settled; stop ticking.
    Done,
}

/// The region accumulated by `invalidate*` between frames, coalesced into a
/// single repaint by the pump.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Dirty {
    Clean,
    Whole,
    Rect(Rect),
}

impl Dirty {
    fn union_whole(&mut self) {
        *self = Self::Whole;
    }

    fn union_rect(&mut self, rect: Rect) {
        *self = match *self {
            Self::Clean => Self::Rect(rect),
            Self::Rect(existing) => Self::Rect(union_rects(existing, rect)),
            Self::Whole => Self::Whole,
        };
    }

    fn is_clean(self) -> bool {
        matches!(self, Self::Clean)
    }
}

/// Smallest rectangle containing both inputs.
fn union_rects(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.min(b.left),
        a.top.min(b.top),
        a.right.max(b.right),
        a.bottom.max(b.bottom),
    )
}

type Stepper = Box<dyn FnMut(FrameTiming) -> Step>;

struct PainterInner {
    // The drawable surface; `None` until first sized, and while a lost device is
    // being recreated. Swapped by the reconciler-driven effect in `surface_painter`.
    surface: RefCell<Option<SurfaceImage>>,
    // Latest user draw callback (refreshed every render so imperative redraws use
    // current state).
    draw: RefCell<Rc<dyn Fn(&DrawContext)>>,
    // Current image source and size setter (refreshed every render), read by
    // `element()` to build the `Image` and track its layout size.
    source: RefCell<Option<SurfaceImageSource>>,
    set_size: RefCell<Option<SetState<(u32, u32)>>>,
    size_revoker: RefCell<Option<EventRevoker>>,
    // Optional user mount hook, run once with the hosting `Image`'s
    // `ElementHandle` (e.g. to open a capture-capable `PointerSurface`). A `Cell`
    // like `stepper`: it is taken out and run, and the hook may re-enter the
    // painter, so it must lend no borrow.
    mounted: Cell<Option<Box<dyn Fn(ElementHandle)>>>,
    // Asks the reconciler to recreate the device + surface after device loss.
    request_rebuild: RefCell<Box<dyn Fn()>>,
    dirty: Cell<Dirty>,
    // A `Cell`, not a `RefCell`, on purpose: the pump takes the stepper out, runs
    // it (user code that may re-enter the painter via `invalidate`/`animate`/
    // `hold`), then puts it back. `Cell` lends no reference, so that take-run-
    // replace cycle cannot double-borrow — the failure mode a `RefCell` here
    // invites when its guard outlives the statement.
    stepper: Cell<Option<Stepper>>,
    // The `CompositionTarget.Rendering` subscription; `Some` exactly while the
    // pump has work (a dirty region, a running animation, or an active hold).
    rendering: RefCell<Option<Rendering>>,
    holds: Cell<u32>,
    last_tick: Cell<Option<Instant>>,
    start: Cell<Option<Instant>>,
    // Whether the control has a live surface and (if a resource builder was set)
    // successfully created its resources — Win2D's `CanvasControl.ReadyToDraw`.
    ready: Cell<bool>,
    // The device backing the current surface, and the DPI it renders at — exposed
    // through `device()` / `dpi()`, the control's `ICanvasResourceCreatorWithDpi`
    // surface. Refreshed by the reconciler-driven effect.
    device: RefCell<Option<GpuDevice>>,
    dpi: Cell<f32>,
}

/// A `SurfaceImageSource`-backed drawing surface you repaint **imperatively** and
/// can drive with a self-stopping per-frame animation — the reactor analogue of
/// Win2D's `CanvasControl`.
///
/// Unlike [`surface_image`], which redraws only when reactor state changes, a
/// painter lets a control repaint on demand without re-running its render
/// function or touching the reconciler: [`invalidate`](Self::invalidate) marks it
/// dirty and a single coalesced repaint runs on the next compositor frame, and
/// [`animate`](Self::animate) drives a frame loop that stops itself when settled.
/// When nothing is dirty or animating it holds no frame subscription and does no
/// work, so idle controls cost nothing.
///
/// Use it for interactive, custom-drawn controls — knobs, sliders, meters — that
/// track a value during a drag and play a brief settling animation afterward,
/// the kind of thing Win2D would put on a `CanvasImageSource` rather than a swap
/// chain (lightweight, transformable by XAML, fine to have many on a page).
///
/// Get the element to render with [`element`](Self::element). The handle is cheap
/// to [`Clone`] into event handlers and effects.
///
/// ```ignore
/// let spring = cx.use_ref(Spring::new(props.value));
/// let painter = surface_painter(cx).draw({
///     let spring = spring.clone();
///     move |ctx| draw_knob(ctx, spring.borrow().position())
/// });
/// // on release, settle to the target:
/// painter.animate({
///     let spring = spring.clone();
///     move |t| {
///         let mut s = spring.borrow_mut();
///         s.step(t.delta);
///         if s.settled() { Step::Done } else { Step::Redraw }
///     }
/// });
/// // ...
/// painter.element()
/// ```
#[derive(Clone)]
pub struct SurfacePainter {
    inner: Rc<PainterInner>,
}

impl SurfacePainter {
    fn new() -> Self {
        Self {
            inner: Rc::new(PainterInner {
                surface: RefCell::new(None),
                draw: RefCell::new(Rc::new(|_| {})),
                source: RefCell::new(None),
                set_size: RefCell::new(None),
                size_revoker: RefCell::new(None),
                mounted: Cell::new(None),
                request_rebuild: RefCell::new(Box::new(|| {})),
                dirty: Cell::new(Dirty::Clean),
                stepper: Cell::new(None),
                rendering: RefCell::new(None),
                holds: Cell::new(0),
                last_tick: Cell::new(None),
                start: Cell::new(None),
                ready: Cell::new(false),
                device: RefCell::new(None),
                dpi: Cell::new(96.0),
            }),
        }
    }

    /// The element to return from your render function. A faithful port of
    /// Win2D's `CanvasControl` structure: an inner `Image` (`Stretch::Fill`) that
    /// *displays* the surface, hosted in a layout-driven `Border` whose own
    /// `SizeChanged` drives the surface size.
    ///
    /// The split matters. `Image` is content-sized — it adopts its `Source`'s
    /// natural size, so a not-yet-created (null) source has no width to report and
    /// could never bootstrap a fill. The `Border` is layout-driven: it fills the
    /// space its parent offers regardless of its child, so it reports the real
    /// available size and the surface is created to match. This is exactly why
    /// Win2D measures its `UserControl` host rather than the inner `Image`. Layout
    /// size therefore tracks on the host; the user mount hook (pointer capture)
    /// runs on the inner `Image`, where the drawn surface — and the pointer — live.
    pub fn element(&self) -> Element {
        let source = self.inner.source.borrow().clone();

        // Inner Image: displays the surface and runs the user mount hook. Pointer
        // capture belongs here, over the drawn surface. `Cell::take` lends no
        // borrow, so the hook may freely re-enter the painter.
        let mount_weak = Rc::downgrade(&self.inner);
        let inner = Image::new(source.into())
            .stretch(Stretch::Fill)
            .on_mounted(move |handle| {
                if let Some(inner) = mount_weak.upgrade()
                    && let Some(mounted) = inner.mounted.take()
                {
                    mounted(handle);
                }
            });

        // Layout-driven host: its `SizeChanged` is the available size, which is
        // what the surface is sized to.
        let size_weak = Rc::downgrade(&self.inner);
        Border::new(inner)
            .on_mounted(move |handle| {
                let Some(inner) = size_weak.upgrade() else {
                    return;
                };
                if let Some(set_size) = inner.set_size.borrow().clone()
                    && let Ok(rev) = handle.on_size_changed(move |w, h| {
                        set_size.call((w.round().max(0.0) as u32, h.round().max(0.0) as u32));
                    })
                {
                    *inner.size_revoker.borrow_mut() = Some(rev);
                }
            })
            .into()
    }

    /// Register a hook run once when the hosting `Image` is mounted, handed its
    /// [`ElementHandle`](windows_reactor::ElementHandle). Use it to open a
    /// capture-capable [`PointerSurface`](windows_reactor::PointerSurface) so a
    /// knob/slider drag keeps tracking past the element bounds, wiring its pointer
    /// events to [`hold`](Self::hold) / [`invalidate`](Self::invalidate) /
    /// [`animate`](Self::animate). Layout-size tracking is handled internally
    /// either way. Call it before [`element`](Self::element); the most recent hook
    /// set before mount is the one that runs.
    pub fn on_mounted(&self, f: impl Fn(ElementHandle) + 'static) {
        self.inner.mounted.set(Some(Box::new(f)));
    }

    /// The current surface size in DIPs (`(0.0, 0.0)` before it is first sized).
    /// Useful in pointer handlers to map a position to a value. Mirrors
    /// `CanvasControl.Size`.
    pub fn size(&self) -> (f32, f32) {
        self.inner
            .surface
            .borrow()
            .as_ref()
            .map_or((0.0, 0.0), |s| (s.width(), s.height()))
    }

    /// Whether the control has a live surface and (if a resource builder was set)
    /// its resources were created successfully — so a [`draw`](SurfacePainterBuilder::draw)
    /// will actually paint. Mirrors `CanvasControl.ReadyToDraw`. `false` before the
    /// first layout, while a lost device is being recreated, or after a resource
    /// build failed.
    pub fn ready_to_draw(&self) -> bool {
        self.inner.ready.get()
    }

    /// The [`GpuDevice`] backing the current surface, or `None` before the control
    /// is first sized (or while a lost device is being recreated). Mirrors
    /// `CanvasControl.Device`.
    pub fn device(&self) -> Option<GpuDevice> {
        self.inner.device.borrow().clone()
    }

    /// The DPI the surface renders at (96 = 100%). Mirrors `CanvasControl.Dpi`.
    pub fn dpi(&self) -> f32 {
        self.inner.dpi.get()
    }

    /// Request a coalesced full redraw on the next frame (like Win2D's
    /// `Invalidate()`). Multiple calls before the next frame collapse into one
    /// repaint.
    pub fn invalidate(&self) {
        let mut dirty = self.inner.dirty.get();
        dirty.union_whole();
        self.inner.dirty.set(dirty);
        self.ensure_pump();
    }

    /// Request a coalesced redraw of just `rect` (DIPs, surface-local) on the next
    /// frame; repeated calls union into one region. Cheaper than a full redraw for
    /// a small change on a large surface.
    pub fn invalidate_rect(&self, rect: Rect) {
        let mut dirty = self.inner.dirty.get();
        dirty.union_rect(rect);
        self.inner.dirty.set(dirty);
        self.ensure_pump();
    }

    /// Drive a per-frame animation. `step` is called once per compositor frame
    /// with the frame timing and returns what to do next; it stops when it returns
    /// [`Step::Done`]. The frame subscription is created on demand and dropped when
    /// the animation settles, so an idle painter costs nothing. Calling `animate`
    /// again replaces any running animation.
    ///
    /// Keep animation state in your own `use_ref` and read it from the `draw`
    /// callback; the step closure only reports progress, so it never needs (and
    /// must not capture) the painter — that keeps it allocation-light and
    /// cycle-free.
    pub fn animate(&self, step: impl FnMut(FrameTiming) -> Step + 'static) {
        self.inner.stepper.set(Some(Box::new(step)));
        self.inner.last_tick.set(None);
        self.inner.start.set(None);
        self.ensure_pump();
    }

    /// Stop any running animation. Pending invalidations still repaint.
    pub fn stop(&self) {
        self.inner.stepper.set(None);
    }

    /// Keep the frame pump warm for a sustained interaction (a drag): hold this
    /// for the gesture and call [`invalidate`](Self::invalidate) /
    /// [`invalidate_rect`](Self::invalidate_rect) per pointer move without the
    /// per-frame subscribe/unsubscribe churn. Drop it on release (then hand off to
    /// [`animate`](Self::animate)).
    pub fn hold(&self) -> PumpHold {
        self.inner.holds.set(self.inner.holds.get() + 1);
        self.ensure_pump();
        PumpHold {
            inner: Rc::downgrade(&self.inner),
        }
    }

    /// Subscribe to per-frame ticks if not already subscribed. The handler holds
    /// only a weak reference back, so the painter is freed (and the subscription
    /// dropped) as soon as the owning component unmounts.
    fn ensure_pump(&self) {
        if self.inner.rendering.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(&self.inner);
        if let Ok(rendering) = on_rendering(move || {
            if let Some(inner) = weak.upgrade() {
                Self { inner }.pump();
            }
        }) {
            *self.inner.rendering.borrow_mut() = Some(rendering);
        }
    }

    /// One compositor frame: advance the animation, then repaint the coalesced
    /// dirty region, then unsubscribe if there is no more work.
    fn pump(&self) {
        // 1. Advance the animation. `Cell::take` removes the stepper outright and
        //    lends no reference, so the step closure is free to re-enter the
        //    painter (`invalidate`/`animate`/`hold`) without any borrow to collide
        //    with — then `rearm` puts a still-running step back.
        if let Some(mut step) = self.inner.stepper.take() {
            let now = Instant::now();
            let start = self.inner.start.get().unwrap_or(now);
            let last = self.inner.last_tick.get().unwrap_or(now);
            self.inner.start.set(Some(start));
            self.inner.last_tick.set(Some(now));
            let timing = FrameTiming {
                delta: now.saturating_duration_since(last),
                total: now.saturating_duration_since(start),
            };
            match step(timing) {
                Step::Redraw => {
                    self.invalidate_region(Dirty::Whole);
                    self.rearm(step);
                }
                Step::RedrawRect(rect) => {
                    self.invalidate_region(Dirty::Rect(rect));
                    self.rearm(step);
                }
                Step::Skip => self.rearm(step),
                Step::Done => {
                    self.inner.last_tick.set(None);
                    self.inner.start.set(None);
                }
            }
        }

        // 2. Coalesced repaint.
        let region = self.inner.dirty.replace(Dirty::Clean);
        if !region.is_clean() {
            self.repaint(region);
        }

        // 3. Drop the subscription when idle. Safe to do from inside the handler:
        //    the event source keeps its own reference to this delegate for the
        //    duration of the call, so revoking here doesn't free the running
        //    closure until after it returns.
        if self.is_idle() {
            *self.inner.rendering.borrow_mut() = None;
        }
    }

    fn invalidate_region(&self, region: Dirty) {
        let mut dirty = self.inner.dirty.get();
        match region {
            Dirty::Whole => dirty.union_whole(),
            Dirty::Rect(rect) => dirty.union_rect(rect),
            Dirty::Clean => {}
        }
        self.inner.dirty.set(dirty);
    }

    /// Put a still-running step back after a frame, unless the step re-armed a new
    /// animation via [`animate`](Self::animate) during the call — in which case the
    /// new one wins and the old is dropped.
    fn rearm(&self, step: Stepper) {
        if let Some(rearmed) = self.inner.stepper.replace(Some(step)) {
            self.inner.stepper.set(Some(rearmed));
        }
    }

    /// Whether the pump has no more work — no animation, no pending repaint, no
    /// active hold — so its frame subscription can be dropped. Peeks the stepper
    /// by taking it and immediately restoring it (a `Cell` lends no reference to
    /// test in place), which is safe because the pump is single-threaded.
    fn is_idle(&self) -> bool {
        let stepper = self.inner.stepper.take();
        let no_animation = stepper.is_none();
        self.inner.stepper.set(stepper);
        no_animation && self.inner.dirty.get().is_clean() && self.inner.holds.get() == 0
    }

    fn repaint(&self, region: Dirty) {
        let region = match region {
            Dirty::Whole => None,
            Dirty::Rect(rect) => Some(rect),
            Dirty::Clean => return,
        };
        let draw = self.inner.draw.borrow().clone();
        let result = {
            let surface = self.inner.surface.borrow();
            // No surface yet (initial / mid device-loss): the effect repaints once
            // it is (re)created.
            let Some(surface) = surface.as_ref() else {
                return;
            };
            surface.draw_region(region, false, |ctx| draw(ctx))
        };
        if let Err(e) = result
            && is_device_lost(e.code())
        {
            // The borrow spans this call, which is sound only because the rebuild
            // closure is internal — it bumps a reconciler generation (a deferred
            // re-render) and never re-enters the painter, so it cannot re-borrow
            // `request_rebuild`. User callbacks that *can* re-enter (the animation
            // stepper, the mount hook) are `Cell`s instead, never borrowed here.
            (self.inner.request_rebuild.borrow())();
        }
    }
}

/// Keeps a [`SurfacePainter`]'s frame pump subscribed for the duration of a
/// gesture. Drop it to release the hold (see [`SurfacePainter::hold`]).
#[must_use = "the pump hold ends as soon as this guard is dropped"]
pub struct PumpHold {
    inner: Weak<PainterInner>,
}

impl Drop for PumpHold {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.holds.set(inner.holds.get().saturating_sub(1));
        }
    }
}

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

/// Which GPU device a [`surface_painter`] draws with. Mirrors Win2D's
/// `UseSharedDevice` / `ForceSoftwareRenderer` / `CustomDevice` knobs.
#[derive(Clone, Default)]
pub enum DeviceSource {
    /// Use a process-thread-wide shared device (Win2D's `UseSharedDevice = true`,
    /// the default). One device backs every canvas on the UI thread — far less GPU
    /// memory and driver overhead than a device per control. When it is lost it is
    /// recreated once and every dependent control gets a `NewDevice`
    /// [`create_resources`](SurfacePainterBuilder::create_resources).
    #[default]
    Shared,
    /// Create a fresh device used only by this control (`UseSharedDevice = false`).
    Owned,
    /// Draw with a device the caller already owns — share an existing
    /// [`GpuDevice`] (e.g. one also driving an [`animated_canvas`] swap chain).
    /// The caller owns its lifetime; if it is lost, recovery is the caller's
    /// responsibility (re-render with a fresh device).
    Custom(GpuDevice),
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

// A process-thread-wide shared `GpuDevice` (Win2D's shared device), kept separate
// for hardware and forced-software so `ForceSoftwareRenderer` controls never share
// a device with hardware ones. Each slot caches the device with a monotonic
// generation; resetting clears it so the next acquire makes a fresh one with a new
// generation, which dependent controls observe as a `NewDevice`.
thread_local! {
    static SHARED_HW: RefCell<Option<(GpuDevice, u64)>> = const { RefCell::new(None) };
    static SHARED_SW: RefCell<Option<(GpuDevice, u64)>> = const { RefCell::new(None) };
    static NEXT_DEVICE_GEN: Cell<u64> = const { Cell::new(1) };
}

fn next_device_gen() -> u64 {
    NEXT_DEVICE_GEN.with(|g| {
        let v = g.get();
        g.set(v.wrapping_add(1));
        v
    })
}

fn make_device(force_software: bool) -> Result<GpuDevice> {
    if force_software {
        GpuDevice::new_warp()
    } else {
        GpuDevice::new_or_warp()
    }
}

/// Get (creating if needed) the shared device for the given renderer, with its
/// generation.
fn shared_device(force_software: bool) -> Option<(GpuDevice, u64)> {
    let get = |slot: &RefCell<Option<(GpuDevice, u64)>>| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let device = make_device(force_software).ok()?;
            *slot = Some((device, next_device_gen()));
        }
        slot.clone()
    };
    if force_software {
        SHARED_SW.with(get)
    } else {
        SHARED_HW.with(get)
    }
}

/// Drop the cached shared device so the next [`shared_device`] makes a fresh one.
fn reset_shared_device(force_software: bool) {
    if force_software {
        SHARED_SW.with(|s| *s.borrow_mut() = None);
    } else {
        SHARED_HW.with(|s| *s.borrow_mut() = None);
    }
}

/// Acquire the device for a control per its [`DeviceSource`], returning it with a
/// generation that changes whenever the underlying device is recreated.
fn acquire_device(
    source: &DeviceSource,
    force_software: bool,
    owned: &HookRef<Option<GpuDevice>>,
    owned_gen: &HookRef<u64>,
) -> Option<(GpuDevice, u64)> {
    match source {
        DeviceSource::Custom(device) => Some((device.clone(), 0)),
        DeviceSource::Shared => shared_device(force_software),
        DeviceSource::Owned => {
            let mut slot = owned.borrow_mut();
            if slot.is_none() {
                *slot = make_device(force_software).ok();
                if slot.is_some() {
                    owned_gen.set(next_device_gen());
                }
            }
            slot.clone().map(|d| (d, owned_gen.get_cloned()))
        }
    }
}

/// Drop the control's device after a loss so the next acquire makes a fresh one.
/// A `Custom` device is caller-owned and left untouched.
fn reset_device(source: &DeviceSource, force_software: bool, owned: &HookRef<Option<GpuDevice>>) {
    match source {
        DeviceSource::Custom(_) => {}
        DeviceSource::Shared => reset_shared_device(force_software),
        DeviceSource::Owned => *owned.borrow_mut() = None,
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
    let (generation, set_generation) = cx.use_state::<u32>(0);
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

            // (Re)build the surface at the element's pixel size. A device-lost
            // failure here triggers the same recovery as a lost repaint.
            let built = if opaque {
                SurfaceImage::new_opaque(&device, w as f32, h as f32, dpi)
            } else {
                SurfaceImage::new(&device, w as f32, h as f32, dpi)
            };
            let surface = match built {
                Ok(surface) => surface,
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
            let new_source = surface.surface();
            *painter.inner.surface.borrow_mut() = Some(surface);
            set_source.call(Some(new_source));
        }
    });

    painter
}

#[cfg(test)]
mod painter_tests {
    use super::{Dirty, union_rects};
    use crate::Rect;

    #[test]
    fn dirty_coalesces_and_resets() {
        let mut d = Dirty::Clean;
        assert!(d.is_clean());

        // First rect → that rect.
        d.union_rect(Rect::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(d, Dirty::Rect(Rect::new(1.0, 2.0, 3.0, 4.0)));

        // Second rect → bounding box of both.
        d.union_rect(Rect::new(0.0, 1.0, 5.0, 3.0));
        assert_eq!(d, Dirty::Rect(Rect::new(0.0, 1.0, 5.0, 4.0)));

        // Whole dominates any rect, in either order.
        d.union_whole();
        assert_eq!(d, Dirty::Whole);
        assert!(!d.is_clean());
        d.union_rect(Rect::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(d, Dirty::Whole);

        assert!(Dirty::Clean.is_clean());
    }

    #[test]
    fn whole_then_clean_via_clean_start() {
        let mut d = Dirty::Clean;
        d.union_whole();
        assert_eq!(d, Dirty::Whole);
    }

    #[test]
    fn union_rects_is_the_bounding_box() {
        let a = Rect::new(0.0, 0.0, 2.0, 2.0);
        let b = Rect::new(1.0, 1.0, 3.0, 4.0);
        assert_eq!(union_rects(a, b), Rect::new(0.0, 0.0, 3.0, 4.0));
        // Commutative.
        assert_eq!(union_rects(b, a), Rect::new(0.0, 0.0, 3.0, 4.0));
    }

    // The painter's frame state machine (stepper take/rearm/idle, dirty
    // coalescing, hold counting) is pure Rust and exercised by driving `pump`
    // directly. There is no live reactor in a unit test, so `ensure_pump`'s
    // `on_rendering` subscription simply fails and is skipped — we advance frames
    // by calling `pump` ourselves. The surface is never created (`None`), so
    // `repaint` is a no-op; these tests target the re-entrancy / borrow paths that
    // crashed in the field, not GPU drawing.
    use super::{Step, SurfacePainter};
    use std::cell::Cell as StdCell;
    use std::rc::Rc;

    #[test]
    fn pump_with_redrawing_animation_does_not_double_borrow() {
        let painter = SurfacePainter::new();
        // The exact shape that aborted in production: a step that keeps redrawing,
        // so the old code re-borrowed `stepper` (via `restore_stepper`) while the
        // take-guard was still alive.
        painter.animate(|_t| Step::Redraw);
        for _ in 0..8 {
            painter.pump();
        }
    }

    #[test]
    fn animation_runs_each_frame_and_stops_on_done() {
        let painter = SurfacePainter::new();
        let frames = Rc::new(StdCell::new(0u32));
        let f = frames.clone();
        painter.animate(move |_t| {
            f.set(f.get() + 1);
            if f.get() >= 3 {
                Step::Done
            } else {
                Step::Redraw
            }
        });
        assert!(!painter.is_idle(), "armed animation is not idle");
        for _ in 0..10 {
            painter.pump();
        }
        // Ran exactly 3 frames, then stopped — no calls after Done.
        assert_eq!(frames.get(), 3);
        assert!(painter.is_idle(), "settled animation drops to idle");
    }

    #[test]
    fn step_may_reenter_invalidate_and_hold() {
        let painter = SurfacePainter::new();
        let p = painter.clone();
        let frames = Rc::new(StdCell::new(0u32));
        let f = frames.clone();
        // The step re-enters the painter every frame — the borrow path that
        // crashed. It also takes (and immediately drops) a hold to stress that.
        painter.animate(move |_t| {
            f.set(f.get() + 1);
            p.invalidate();
            let _h = p.hold();
            if f.get() >= 4 {
                Step::Skip
            } else {
                Step::Redraw
            }
        });
        for _ in 0..6 {
            painter.pump();
        }
        // Stepper keeps running (only ever Skip/Redraw), so it ran every frame.
        assert_eq!(frames.get(), 6);
    }

    #[test]
    fn step_may_rearm_a_new_animation() {
        let painter = SurfacePainter::new();
        let p = painter.clone();
        let first = Rc::new(StdCell::new(0u32));
        let second = Rc::new(StdCell::new(0u32));
        let f = first.clone();
        let s = second.clone();
        painter.animate(move |_t| {
            f.set(f.get() + 1);
            // Replace ourselves with a different animation mid-flight.
            let s = s.clone();
            p.animate(move |_t| {
                s.set(s.get() + 1);
                if s.get() >= 2 {
                    Step::Done
                } else {
                    Step::Redraw
                }
            });
            Step::Done
        });
        for _ in 0..10 {
            painter.pump();
        }
        // The original ran once and was replaced; the re-armed one ran to its end.
        assert_eq!(first.get(), 1);
        assert_eq!(second.get(), 2);
        assert!(painter.is_idle());
    }

    #[test]
    fn rapid_animate_replaces_running_animation() {
        let painter = SurfacePainter::new();
        // Replacing a running animation must not leak or panic; the latest wins.
        for _ in 0..100 {
            painter.animate(|_t| Step::Redraw);
        }
        let frames = Rc::new(StdCell::new(0u32));
        let f = frames.clone();
        painter.animate(move |_t| {
            f.set(f.get() + 1);
            if f.get() >= 2 {
                Step::Done
            } else {
                Step::Redraw
            }
        });
        for _ in 0..5 {
            painter.pump();
        }
        assert_eq!(frames.get(), 2);
    }

    #[test]
    fn reentrant_pump_from_within_step_is_safe() {
        let painter = SurfacePainter::new();
        let p = painter.clone();
        let frames = Rc::new(StdCell::new(0u32));
        painter.animate(move |_t| {
            frames.set(frames.get() + 1);
            if frames.get() == 1 {
                // Re-enter the whole pump from inside a step. The inner pump finds
                // no stepper (we are taken out) and must not panic on any cell.
                p.pump();
            }
            if frames.get() >= 3 {
                Step::Done
            } else {
                Step::Skip
            }
        });
        for _ in 0..10 {
            painter.pump();
        }
        assert!(painter.is_idle());
    }

    #[test]
    fn invalidate_marks_dirty_until_pumped() {
        let painter = SurfacePainter::new();
        assert!(painter.is_idle());
        painter.invalidate();
        assert!(!painter.is_idle(), "a pending repaint is not idle");
        painter.pump();
        assert!(
            painter.is_idle(),
            "the coalesced repaint clears the dirty region"
        );

        // The rect path coalesces and clears the same way.
        painter.invalidate_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        painter.invalidate_rect(Rect::new(5.0, 5.0, 30.0, 30.0));
        assert!(!painter.is_idle());
        painter.pump();
        assert!(painter.is_idle());
    }

    #[test]
    fn hold_keeps_pump_busy_until_dropped() {
        let painter = SurfacePainter::new();
        assert!(painter.is_idle());
        let h1 = painter.hold();
        let h2 = painter.hold();
        assert!(!painter.is_idle(), "outstanding holds keep the pump busy");
        // Pumping with only holds (no dirty, no anim) must not drop below zero or
        // panic, and must stay busy.
        painter.pump();
        assert!(!painter.is_idle());
        drop(h1);
        assert!(!painter.is_idle(), "one hold remains");
        drop(h2);
        assert!(painter.is_idle(), "releasing the last hold returns to idle");
    }

    #[test]
    fn stop_clears_a_running_animation() {
        let painter = SurfacePainter::new();
        painter.animate(|_t| Step::Redraw);
        assert!(!painter.is_idle());
        painter.stop();
        assert!(painter.is_idle());
        // Pumping after stop does nothing and stays idle.
        painter.pump();
        assert!(painter.is_idle());
    }

    #[test]
    fn dropped_hold_after_painter_gone_does_not_panic() {
        let painter = SurfacePainter::new();
        let hold = painter.hold();
        // The painter (and its inner) is dropped while a hold outlives it; the
        // hold's `Weak` upgrade fails and the drop is a no-op rather than a crash.
        drop(painter);
        drop(hold);
    }
}
