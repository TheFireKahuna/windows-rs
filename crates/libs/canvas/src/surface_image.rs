use super::*;
use std::cell::Cell;
use windows_reactor::{ImageSource, SurfaceImageSource};

/// A fixed-size `SurfaceImageSource` you draw into with canvas's
/// [`DrawingSession`] — brushes, geometry, text, effects — then display in a
/// reactor `Image`. The bridge over the raw reactor
/// [`SurfaceImageSource`](windows_reactor::SurfaceImageSource): it attaches your
/// [`GpuDevice`], maps the surface's pixel atlas to logical (DIP) drawing
/// coordinates at the configured DPI, and reports device loss the canvas way.
///
/// The API works in **DIPs**; the surface is allocated at `dip × dpi/96` pixels
/// so it stays crisp at high DPI. The size and DPI are fixed for the lifetime of
/// the surface — create a new one (or use [`surface_image`](crate::surface_image)
/// for automatic recreation) when either changes.
///
/// Draw on the UI thread. A [`GpuDevice::new_multi_threaded`] device may be
/// shared with a render thread (e.g. an [`animated_canvas`](crate::animated_canvas)
/// swap chain): the reactor surface serializes its DXGI interop against that
/// thread's work, so the two can't race.
pub struct SurfaceImage {
    source: SurfaceImageSource,
    device: GpuDevice,
    width: f32,
    height: f32,
    dpi: f32,
    device_lost: Cell<bool>,
}

impl SurfaceImage {
    /// Create a `width × height` (DIP) surface with alpha, backed by `device` and
    /// rendered at `dpi` (96 = 100%).
    pub fn new(device: &GpuDevice, width: f32, height: f32, dpi: f32) -> Result<Self> {
        Self::create(device, width, height, dpi, false)
    }

    /// Create an **opaque** surface (no alpha channel — cheaper to composite when
    /// the content fully covers its bounds; you must paint every pixel).
    pub fn new_opaque(device: &GpuDevice, width: f32, height: f32, dpi: f32) -> Result<Self> {
        Self::create(device, width, height, dpi, true)
    }

    fn create(device: &GpuDevice, width: f32, height: f32, dpi: f32, opaque: bool) -> Result<Self> {
        let scale = dpi / 96.0;
        let pixel_width = ((width * scale).round() as i32).max(1);
        let pixel_height = ((height * scale).round() as i32).max(1);
        let source = if opaque {
            SurfaceImageSource::new_opaque(pixel_width, pixel_height)?
        } else {
            SurfaceImageSource::new(pixel_width, pixel_height)?
        };
        source.set_device(device.d2d_device())?;
        Ok(Self {
            source,
            device: device.clone(),
            width,
            height,
            dpi,
            device_lost: Cell::new(false),
        })
    }

    /// Width of the surface, in DIPs.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Height of the surface, in DIPs.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// The DPI the surface renders at (96 = 100%).
    pub fn dpi(&self) -> f32 {
        self.dpi
    }

    /// Whether the last [`draw`](Self::draw) lost the device. Recreate the device
    /// and this surface to recover.
    pub fn is_device_lost(&self) -> bool {
        self.device_lost.get()
    }

    /// Redraw the whole surface.
    pub fn draw_all(&self, f: impl FnOnce(&DrawContext)) -> Result<()> {
        self.draw_region(None, false, f)
    }

    /// Redraw a single dirty rectangle (in DIPs, surface-local). Only those
    /// pixels are presented, so pass the tightest rectangle that covers your
    /// change. The closure draws in surface-local DIP coordinates; use
    /// [`DrawContext::update_rect`] to clip work to the dirty region.
    pub fn draw(&self, rect: Rect, f: impl FnOnce(&DrawContext)) -> Result<()> {
        self.draw_region(Some(rect), false, f)
    }

    /// Core draw. `region` is the dirty rectangle (DIPs, surface-local) or `None`
    /// for the whole surface; `changed` is forwarded to
    /// [`DrawContext::device_changed`] so a caller can rebuild device-specific
    /// resources on the first frame after the surface was (re)created.
    pub(crate) fn draw_region(
        &self,
        region: Option<Rect>,
        changed: bool,
        f: impl FnOnce(&DrawContext),
    ) -> Result<()> {
        let rect = region.unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, self.width, self.height));
        let scale = self.dpi / 96.0;
        let (px, py, pw, ph) = dip_rect_to_pixels(rect, self.width, self.height, self.dpi);

        self.device_lost.set(false);
        let (context, (offset_x, offset_y)) = self
            .source
            .begin_draw::<ID2D1DeviceContext>(px, py, pw, ph)
            .inspect_err(|e| {
                if is_device_lost(e.code()) {
                    self.device_lost.set(true);
                }
            })?;

        // Draw in DIPs (crisp at high DPI), positioned so the dirty rect's origin
        // lands at the atlas offset: pixel = (S + t)·scale must equal
        // offset + (S − rect.origin)·scale, so t = offset/scale − rect.origin.
        unsafe { context.SetDpi(self.dpi, self.dpi) };
        let tx = offset_x as f32 / scale - rect.left;
        let ty = offset_y as f32 / scale - rect.top;
        {
            let session = DrawingSession::new_borrowed(&context, &self.device_lost);
            session.set_transform(&Matrix3x2::translation(tx, ty));
            let ctx = DrawContext::new(
                session,
                &self.device,
                self.width,
                self.height,
                self.dpi,
                changed,
                rect,
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

    /// Suspend drawing, allowing the surface's GPU resources to be reclaimed.
    pub fn suspend(&self) -> Result<()> {
        self.source.suspend_draw()
    }

    /// Resume drawing after [`suspend`](Self::suspend).
    pub fn resume(&self) -> Result<()> {
        self.source.resume_draw()
    }

    /// The underlying reactor source, for handing to a reactor `Image`.
    pub fn surface(&self) -> SurfaceImageSource {
        self.source.clone()
    }

    /// The reactor `ImageSource` for this surface, ready for `Image::new`.
    pub fn image_source(&self) -> ImageSource {
        ImageSource::Surface(self.source.clone())
    }
}

/// Map a DIP dirty rectangle to an in-bounds pixel update rect `(x, y, w, h)`
/// for `SurfaceImageSource::begin_draw`.
///
/// The edges are rounded and the size derived from them (rather than rounding
/// the origin and size independently) and clamped to the surface's allocated
/// pixel bounds, so an edge-aligned rect can never produce a right/bottom edge
/// one pixel past the surface — which would fail the native `BeginDraw` at
/// fractional DPI scales (125%, 150%, …).
fn dip_rect_to_pixels(
    rect: Rect,
    surface_width: f32,
    surface_height: f32,
    dpi: f32,
) -> (i32, i32, i32, i32) {
    let scale = dpi / 96.0;
    let pixel_w = ((surface_width * scale).round() as i32).max(1);
    let pixel_h = ((surface_height * scale).round() as i32).max(1);
    let px = ((rect.left * scale).round() as i32).clamp(0, pixel_w);
    let py = ((rect.top * scale).round() as i32).clamp(0, pixel_h);
    let right = ((rect.right * scale).round() as i32).clamp(px, pixel_w);
    let bottom = ((rect.bottom * scale).round() as i32).clamp(py, pixel_h);
    (px, py, (right - px).max(1), (bottom - py).max(1))
}

#[cfg(test)]
mod tests {
    use super::dip_rect_to_pixels;
    use crate::Rect;

    fn surface_pixels(dip: f32, dpi: f32) -> i32 {
        ((dip * dpi / 96.0).round() as i32).max(1)
    }

    // At fractional DPI an edge-aligned dirty rect must never extend past the
    // surface's allocated pixel bounds (regression for the independent-rounding
    // overshoot that failed the native BeginDraw at 150%).
    #[test]
    fn edge_aligned_rect_stays_in_bounds() {
        // 10x10 DIP surface at 150%: 3..10 horizontally is the case that
        // overshot (round(4.5)=5, round(10.5)=11 -> right 16 > 15).
        for &dpi in &[96.0_f32, 120.0, 144.0, 192.0] {
            let (w, h) = (10.0_f32, 8.0_f32);
            let pixel_w = surface_pixels(w, dpi);
            let pixel_h = surface_pixels(h, dpi);
            for &(l, t, r, b) in &[
                (0.0, 0.0, 10.0, 8.0), // whole surface
                (3.0, 2.0, 10.0, 8.0), // touches right/bottom edge
                (0.0, 0.0, 3.0, 4.0),  // interior
                (9.0, 7.0, 10.0, 8.0), // 1-DIP sliver at the corner
            ] {
                let (px, py, pw, ph) = dip_rect_to_pixels(Rect::new(l, t, r, b), w, h, dpi);
                assert!(px >= 0 && py >= 0, "origin negative at dpi {dpi}");
                assert!(
                    px + pw <= pixel_w && py + ph <= pixel_h,
                    "rect ({l},{t},{r},{b}) at dpi {dpi} overshoots: \
                     ({px}+{pw}, {py}+{ph}) vs bounds ({pixel_w}, {pixel_h})"
                );
                assert!(pw >= 1 && ph >= 1, "degenerate size at dpi {dpi}");
            }
        }
    }

    // The full-surface rect maps to exactly the allocated pixel size.
    #[test]
    fn full_surface_maps_to_exact_pixels() {
        let (w, h, dpi) = (256.0_f32, 128.0_f32, 144.0);
        let (px, py, pw, ph) = dip_rect_to_pixels(Rect::new(0.0, 0.0, w, h), w, h, dpi);
        assert_eq!((px, py), (0, 0));
        assert_eq!((pw, ph), (surface_pixels(w, dpi), surface_pixels(h, dpi)));
    }
}
