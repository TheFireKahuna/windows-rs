use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows_reactor::{ImageSource, UpdateRect, UpdatesRegistration, VirtualSurfaceImageSource};

/// A virtualized surface for content larger than the screen (or larger than the
/// GPU's maximum texture size) — maps, large document canvases, infinite-scroll
/// boards — drawn with canvas's [`DrawingSession`]. The bridge over the reactor
/// [`VirtualSurfaceImageSource`](windows_reactor::VirtualSurfaceImageSource):
/// the framework keeps only the visible regions resident and asks you, through
/// the [`on_draw`](Self::on_draw) handler, to (re)draw regions as they scroll
/// into view, so you only ever paint what is visible.
///
/// Like [`SurfaceImage`](crate::SurfaceImage), the API works in **DIPs** and the
/// surface is allocated at `dip × dpi/96` pixels. Draw on the UI thread.
///
/// Cloning shares one underlying surface and update registration; the
/// registration (and the surface↔callback cycle it holds) is released when the
/// last clone drops.
#[derive(Clone)]
pub struct VirtualSurfaceImage {
    inner: Rc<Inner>,
}

struct Inner {
    source: VirtualSurfaceImageSource,
    device: GpuDevice,
    dpi: Cell<f32>,
    content_width: Cell<f32>,
    content_height: Cell<f32>,
    device_lost: Cell<bool>,
    // Holds the RAII update registration; dropping `Inner` drops this, which
    // unregisters and breaks the surface↔callback reference cycle.
    registration: RefCell<Option<UpdatesRegistration>>,
}

impl VirtualSurfaceImage {
    /// Create a `width × height` (DIP) virtual surface backed by `device` and
    /// rendered at `dpi` (96 = 100%). Call [`on_draw`](Self::on_draw) to supply
    /// the draw handler.
    pub fn new(device: &GpuDevice, width: f32, height: f32, dpi: f32) -> Result<Self> {
        let scale = dpi / 96.0;
        let pixel_width = ((width * scale).round() as i32).max(1);
        let pixel_height = ((height * scale).round() as i32).max(1);
        let source = VirtualSurfaceImageSource::new(pixel_width, pixel_height)?;
        source.set_device(device.d2d_device())?;
        Ok(Self {
            inner: Rc::new(Inner {
                source,
                device: device.clone(),
                dpi: Cell::new(dpi),
                content_width: Cell::new(width),
                content_height: Cell::new(height),
                device_lost: Cell::new(false),
                registration: RefCell::new(None),
            }),
        })
    }

    /// Set the handler the framework calls to (re)draw regions as they become
    /// visible or are invalidated. Each call draws one dirty region: the closure
    /// receives a [`DrawContext`] whose [`update_rect`](DrawContext::update_rect)
    /// is the region to paint (in DIPs, surface-local), and whose `width`/`height`
    /// are the full content extent. Replacing the handler replaces the previous
    /// registration.
    pub fn on_draw(&self, f: impl Fn(&DrawContext) + 'static) -> Result<()> {
        // The callback holds only a weak handle back to `Inner`, so it never
        // keeps the surface alive — otherwise `Inner -> registration -> callback
        // -> Inner` would be an unbreakable Rc cycle.
        let weak = Rc::downgrade(&self.inner);
        let f = Rc::new(f);
        // Drop any previous registration *before* installing the new one.
        // Otherwise the assignment would drop the old registration after the new
        // is registered, and the old `UpdatesRegistration::drop` would clear the
        // just-installed callback (`RegisterForUpdatesNeeded(null)`).
        *self.inner.registration.borrow_mut() = None;
        let registration =
            self.inner
                .source
                .register_for_updates(move |rects: &[UpdateRect]| {
                    let Some(inner) = weak.upgrade() else {
                        return;
                    };
                    for rect in rects {
                        let _ = inner.draw_rect(*rect, f.as_ref());
                    }
                })?;
        *self.inner.registration.borrow_mut() = Some(registration);
        Ok(())
    }

    /// Mark a region (DIPs, surface-local) as needing a redraw. The handler is
    /// called for the visible part of it.
    pub fn invalidate(&self, rect: Rect) -> Result<()> {
        self.inner.source.invalidate(self.inner.to_pixels(rect))
    }

    /// Mark the whole content extent as needing a redraw.
    pub fn invalidate_all(&self) -> Result<()> {
        let rect = Rect::from_xywh(
            0.0,
            0.0,
            self.inner.content_width.get(),
            self.inner.content_height.get(),
        );
        self.invalidate(rect)
    }

    /// Resize the content extent (DIPs). Content outside the new bounds is
    /// discarded.
    pub fn resize(&self, width: f32, height: f32) -> Result<()> {
        let scale = self.inner.dpi.get() / 96.0;
        self.inner.content_width.set(width);
        self.inner.content_height.set(height);
        self.inner.source.resize(
            ((width * scale).round() as i32).max(1),
            ((height * scale).round() as i32).max(1),
        )
    }

    /// The currently visible region of the content, in DIPs.
    pub fn visible_bounds(&self) -> Result<Rect> {
        let scale = self.inner.dpi.get() / 96.0;
        let b = self.inner.source.visible_bounds()?;
        Ok(Rect::from_xywh(
            b.x as f32 / scale,
            b.y as f32 / scale,
            b.width as f32 / scale,
            b.height as f32 / scale,
        ))
    }

    /// Whether the last draw lost the device.
    pub fn is_device_lost(&self) -> bool {
        self.inner.device_lost.get()
    }

    /// The underlying reactor source, for handing to a reactor `Image`.
    pub fn surface(&self) -> VirtualSurfaceImageSource {
        self.inner.source.clone()
    }

    /// The reactor `ImageSource` for this surface, ready for `Image::new`.
    pub fn image_source(&self) -> ImageSource {
        ImageSource::Virtual(self.inner.source.clone())
    }
}

impl Inner {
    fn to_pixels(&self, rect: Rect) -> UpdateRect {
        // Round the edges (not the size) so an edge-aligned rect stays within the
        // surface's pixel bounds rather than overshooting by a pixel.
        let scale = self.dpi.get() / 96.0;
        let x = (rect.left * scale).round() as i32;
        let y = (rect.top * scale).round() as i32;
        let right = (rect.right * scale).round() as i32;
        let bottom = (rect.bottom * scale).round() as i32;
        UpdateRect {
            x,
            y,
            width: (right - x).max(1),
            height: (bottom - y).max(1),
        }
    }

    fn draw_rect(&self, ur: UpdateRect, f: &dyn Fn(&DrawContext)) -> Result<()> {
        let dpi = self.dpi.get();
        let scale = dpi / 96.0;

        self.device_lost.set(false);
        let (context, (offset_x, offset_y)) = self
            .source
            .begin_draw::<ID2D1DeviceContext>(ur)
            .inspect_err(|e| {
                if is_device_lost(e.code()) {
                    self.device_lost.set(true);
                }
            })?;

        // Same atlas-offset mapping as `SurfaceImage::draw`, but the dirty rect
        // arrives in pixels here: t = (offset − rect.origin) / scale.
        unsafe { context.SetDpi(dpi, dpi) };
        let tx = (offset_x - ur.x) as f32 / scale;
        let ty = (offset_y - ur.y) as f32 / scale;
        let update = Rect::from_xywh(
            ur.x as f32 / scale,
            ur.y as f32 / scale,
            ur.width as f32 / scale,
            ur.height as f32 / scale,
        );
        {
            let session = DrawingSession::new_borrowed(&context, &self.device_lost);
            session.set_transform(&Matrix3x2::translation(tx, ty));
            let ctx = DrawContext::new(
                session,
                &self.device,
                self.content_width.get(),
                self.content_height.get(),
                false,
                update,
            );
            f(&ctx);
        }

        let result = self.source.end_draw();
        if let Err(e) = &result
            && is_device_lost(e.code())
        {
            self.device_lost.set(true);
        }
        result
    }
}
