use super::*;
use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

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

/// The drawable backing of a [`SurfacePainter`]. On the WinUI backend it is a XAML
/// [`SurfaceImage`] (a `SurfaceImageSource` shown through an `Image`); on the
/// self-hosted DirectComposition backend it is a child-visual composition surface
/// parented under the host's `ContainerVisual`. The per-frame draw closures are
/// identical across both — only the surface origin differs — so the painter routes
/// every repaint through [`draw_region`](Self::draw_region) without caring which it
/// holds.
pub(crate) enum PaintSurface {
    Image(SurfaceImage),
    Comp(CompSurface),
}

/// A DirectComposition child-visual composition surface, drawn through a
/// [`CompositionDrawTarget`]. FP16 (`R16G16B16A16Float`) so the viz is HDR — see
/// [`CompositionSurfaceFactory::create_under_node`].
pub(crate) struct CompSurface {
    target: CompositionDrawTarget,
    // Keeps the sprite parented under the host element's `ContainerVisual`; dropping
    // it removes the sprite (so replacing the surface on resize detaches the old one).
    _visual: CompositionChildVisual,
    device: GpuDevice,
    width: f32,
    height: f32,
    dpi: f32,
}

impl PaintSurface {
    /// Build the surface for the current backend. Tries the DirectComposition path
    /// first — a child-visual composition surface under `host`'s `ContainerVisual`
    /// — and falls back to a XAML [`SurfaceImage`] when the host is not a system
    /// `ContainerVisual` (the WinUI backend) or no host is available yet.
    pub(crate) fn build(
        host: Option<&ElementHandle>,
        device: &GpuDevice,
        width: f32,
        height: f32,
        dpi: f32,
        opaque: bool,
    ) -> Result<Self> {
        // dcomp backend: host a composition child-visual surface under the node.
        // `from_node` only succeeds when `host.native()` is a system
        // `ContainerVisual`, so this cleanly no-ops on WinUI.
        if let Some(host) = host
            && let Ok(factory) = CompositionSurfaceFactory::from_node(host.native(), device.d2d_device())
        {
            let scale = dpi / 96.0;
            let pw = ((width * scale).round() as i32).max(1);
            let ph = ((height * scale).round() as i32).max(1);
            let (visual, draw) =
                factory.create_under_node(host.native(), (pw, ph), (width, height), opaque)?;
            return Ok(Self::Comp(CompSurface {
                target: CompositionDrawTarget::new(draw),
                _visual: visual,
                device: device.clone(),
                width,
                height,
                dpi,
            }));
        }

        // WinUI backend: a XAML `SurfaceImageSource`.
        let img = if opaque {
            SurfaceImage::new_opaque(device, width, height, dpi)?
        } else {
            SurfaceImage::new(device, width, height, dpi)?
        };
        Ok(Self::Image(img))
    }

    fn width(&self) -> f32 {
        match self {
            Self::Image(i) => i.width(),
            Self::Comp(c) => c.width,
        }
    }

    fn height(&self) -> f32 {
        match self {
            Self::Image(i) => i.height(),
            Self::Comp(c) => c.height,
        }
    }

    /// The reactor `SurfaceImageSource` to display in the host `Image`, or `None`
    /// for a composition surface (whose content is shown by its own sprite visual,
    /// not a XAML `ImageSource`).
    pub(crate) fn image_source(&self) -> Option<SurfaceImageSource> {
        match self {
            Self::Image(i) => Some(i.surface()),
            Self::Comp(_) => None,
        }
    }

    /// Bracket a draw, handing the closure a [`DrawContext`]. `region` is a
    /// surface-local DIP dirty rect (honoured on the `SurfaceImage` path; the
    /// composition path always redraws the whole surface). `changed` is forwarded
    /// to [`DrawContext::device_changed`].
    pub(crate) fn draw_region(
        &self,
        region: Option<Rect>,
        changed: bool,
        f: impl FnOnce(&DrawContext),
    ) -> Result<()> {
        match self {
            Self::Image(img) => img.draw_region(region, changed, f),
            Self::Comp(c) => {
                c.target
                    .draw_context(&c.device, c.width, c.height, c.dpi, changed, f)
            }
        }
    }
}

pub(crate) struct PainterInner {
    // The drawable surface; `None` until first sized, and while a lost device is
    // being recreated. Swapped by the reconciler-driven effect in `surface_painter`.
    pub(crate) surface: RefCell<Option<PaintSurface>>,
    // Latest user draw callback (refreshed every render so imperative redraws use
    // current state).
    pub(crate) draw: RefCell<Rc<dyn Fn(&DrawContext)>>,
    // Current image source and size setter (refreshed every render), read by
    // `element()` to build the `Image` and track its layout size.
    pub(crate) source: RefCell<Option<SurfaceImageSource>>,
    pub(crate) set_size: RefCell<Option<SetState<(u32, u32)>>>,
    size_revoker: RefCell<Option<EventRevoker>>,
    // Optional user mount hook, run once with the hosting `Image`'s
    // `ElementHandle` (e.g. to open a capture-capable `PointerSurface`). A `Cell`
    // like `stepper`: it is taken out and run, and the hook may re-enter the
    // painter, so it must lend no borrow.
    mounted: Cell<Option<Box<dyn Fn(ElementHandle)>>>,
    // Asks the reconciler to recreate the device + surface after device loss.
    pub(crate) request_rebuild: RefCell<Box<dyn Fn()>>,
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
    pub(crate) ready: Cell<bool>,
    // The device backing the current surface, and the DPI it renders at — exposed
    // through `device()` / `dpi()`, the control's `ICanvasResourceCreatorWithDpi`
    // surface. Refreshed by the reconciler-driven effect.
    pub(crate) device: RefCell<Option<GpuDevice>>,
    pub(crate) dpi: Cell<f32>,
    // The host `Border`'s `ElementHandle`, captured on mount. On the dcomp backend
    // its `native()` is the `ContainerVisual` the composition surface parents under;
    // the build effect reads it to choose (and host) the composition variant.
    pub(crate) host: RefCell<Option<ElementHandle>>,
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
    pub(crate) inner: Rc<PainterInner>,
}

impl SurfacePainter {
    pub(crate) fn new() -> Self {
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
                host: RefCell::new(None),
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
                // Capture the host for the build effect: on dcomp its `native()` is
                // the `ContainerVisual` the composition surface parents under. Stored
                // before the size subscription below, so it is always present by the
                // time a non-zero size triggers the effect.
                *inner.host.borrow_mut() = Some(handle.clone());
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
