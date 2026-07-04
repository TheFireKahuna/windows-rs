use super::*;

/// Safe wrapper over `ID2D1DeviceContext`.
pub struct DrawingSession<'a> {
    context: &'a ID2D1DeviceContext,
    device_lost_flag: &'a Cell<bool>,
    // Whether this session owns the Direct2D `BeginDraw`/`EndDraw` bracket. A
    // swap-chain session does (it brackets the frame itself); a session adopted
    // over a `SurfaceImageSource` does not, because that surface's native
    // `BeginDraw`/`EndDraw` already opens and closes the draw — issuing a nested
    // Direct2D `BeginDraw` there is `D2DERR_WRONG_STATE`.
    owns_bracket: bool,
    // Whether the draw target is an 8-bit sRGB surface. Every color entering a
    // session is *linear* scRGB; when this is set, each color the session forwards
    // to Direct2D — solid brushes (including later recolors), gradient stops, the
    // clear color, effect tints — is linear→sRGB encoded (+ clamped) on the way out,
    // so a linear value lands correctly on a UNORM sRGB surface. A linear FP16 scRGB
    // surface leaves this false and passes colors through raw (its native encoding).
    encode_srgb: bool,
}

impl<'a> DrawingSession<'a> {
    pub(crate) fn new(
        context: &'a ID2D1DeviceContext,
        device_lost_flag: &'a Cell<bool>,
    ) -> Result<Self> {
        unsafe { context.BeginDraw() };
        Ok(Self {
            context,
            device_lost_flag,
            owns_bracket: true,
            encode_srgb: false,
        })
    }

    /// Adopt a context that is *already* in a draw (its `BeginDraw`/`EndDraw`
    /// bracket is owned elsewhere — e.g. a `SurfaceImageSource`'s native
    /// `BeginDraw`). This session issues no `BeginDraw` and no `EndDraw`; the
    /// owner is responsible for ending the draw and for observing device-loss
    /// from that call. Used by the reactor surface bridges and by the
    /// self-hosted DirectComposition backend.
    pub fn new_borrowed(
        context: &'a ID2D1DeviceContext,
        device_lost_flag: &'a Cell<bool>,
    ) -> Self {
        Self {
            context,
            device_lost_flag,
            owns_bracket: false,
            encode_srgb: false,
        }
    }

    /// Mark this session's target as an 8-bit sRGB surface (or not).
    ///
    /// Every color entering a session is linear scRGB. On an 8-bit sRGB target the
    /// linear value must be gamma-*encoded* (+ clamped) before it is written — set
    /// this and the session encodes every color it forwards to Direct2D (solid
    /// brushes and their recolors, gradient stops, clears, effect tints). Leave it
    /// off (the default) for a linear FP16 scRGB surface, which stores linear values
    /// raw. Chain it on construction:
    /// `DrawingSession::new_borrowed(ctx, flag).encode_srgb_target(surface_is_8bit)`.
    pub fn encode_srgb_target(mut self, on: bool) -> Self {
        self.encode_srgb = on;
        self
    }

    /// Prepare a linear color for this session's target: linear→sRGB encoded on an
    /// 8-bit sRGB surface, passed through raw on a linear FP16 one. (The display
    /// SDR-white adjustment is NOT applied here — it lives compositor-side as a
    /// per-visual effect, so cached surface content rescales without a repaint;
    /// see the reactor dcomp backend's white-level module.)
    fn resolve(&self, color: ColorF) -> ColorF {
        if self.encode_srgb {
            color.to_srgb()
        } else {
            color
        }
    }

    /// Clears the entire session to the given color.
    pub fn clear(&self, color: ColorF) {
        let c: D2D_COLOR_F = self.resolve(color).into();
        unsafe { self.context.Clear(Some(&c)) };
    }

    /// Draws a straight line between two points.
    pub fn draw_line(&self, p0: Vector2, p1: Vector2, brush: &impl Paint, width: f32) {
        unsafe {
            self.context
                .DrawLine(p0, p1, brush.as_raw_brush(), width, None);
        }
    }

    /// Draws a straight line using the given stroke style.
    pub fn draw_line_styled(
        &self,
        p0: Vector2,
        p1: Vector2,
        brush: &impl Paint,
        width: f32,
        style: &StrokeStyle,
    ) {
        unsafe {
            self.context
                .DrawLine(p0, p1, brush.as_raw_brush(), width, &style.0);
        }
    }

    /// Draws the outline of a rectangle.
    pub fn draw_rect(&self, rect: &Rect, brush: &impl Paint, width: f32) {
        unsafe {
            self.context
                .DrawRectangle(&rect.to_abi(), brush.as_raw_brush(), width, None);
        }
    }

    /// Draws the outline of a rectangle using the given stroke style.
    pub fn draw_rect_styled(
        &self,
        rect: &Rect,
        brush: &impl Paint,
        width: f32,
        style: &StrokeStyle,
    ) {
        unsafe {
            self.context
                .DrawRectangle(&rect.to_abi(), brush.as_raw_brush(), width, &style.0);
        }
    }

    /// Fills a rectangle.
    pub fn fill_rect(&self, rect: &Rect, brush: &impl Paint) {
        unsafe {
            self.context
                .FillRectangle(&rect.to_abi(), brush.as_raw_brush());
        }
    }

    /// Draws the outline of a rounded rectangle.
    pub fn draw_rounded_rect(&self, rect: &RoundedRect, brush: &impl Paint, width: f32) {
        unsafe {
            self.context
                .DrawRoundedRectangle(&rect.to_abi(), brush.as_raw_brush(), width, None);
        }
    }

    /// Draws the outline of a rounded rectangle using the given stroke style.
    pub fn draw_rounded_rect_styled(
        &self,
        rect: &RoundedRect,
        brush: &impl Paint,
        width: f32,
        style: &StrokeStyle,
    ) {
        unsafe {
            self.context.DrawRoundedRectangle(
                &rect.to_abi(),
                brush.as_raw_brush(),
                width,
                &style.0,
            );
        }
    }

    /// Fills a rounded rectangle.
    pub fn fill_rounded_rect(&self, rect: &RoundedRect, brush: &impl Paint) {
        unsafe {
            self.context
                .FillRoundedRectangle(&rect.to_abi(), brush.as_raw_brush());
        }
    }

    /// Draws the outline of an ellipse.
    pub fn draw_ellipse(&self, ellipse: &Ellipse, brush: &impl Paint, width: f32) {
        unsafe {
            self.context
                .DrawEllipse(&ellipse.to_abi(), brush.as_raw_brush(), width, None);
        }
    }

    /// Draws the outline of an ellipse using the given stroke style.
    pub fn draw_ellipse_styled(
        &self,
        ellipse: &Ellipse,
        brush: &impl Paint,
        width: f32,
        style: &StrokeStyle,
    ) {
        unsafe {
            self.context
                .DrawEllipse(&ellipse.to_abi(), brush.as_raw_brush(), width, &style.0);
        }
    }

    /// Fills an ellipse.
    pub fn fill_ellipse(&self, ellipse: &Ellipse, brush: &impl Paint) {
        unsafe {
            self.context
                .FillEllipse(&ellipse.to_abi(), brush.as_raw_brush());
        }
    }

    /// Creates a solid color brush. The brush inherits this session's target color
    /// space, so a later [`set_color`](Brush::set_color) on a linear target converts
    /// too (the recolorable-brush path that immediate-mode draws reuse every frame).
    pub fn create_solid_brush(&self, color: ColorF) -> Result<Brush> {
        let c: D2D_COLOR_F = self.resolve(color).into();
        unsafe {
            self.context
                .CreateSolidColorBrush(&c, None)
                .map(|b| Brush::new(b, self.encode_srgb))
        }
    }

    /// Resolve a stop list for this session's target and pick the interpolation
    /// gamma. Stops enter linear. On a linear (FP16) target the colors pass through
    /// raw and interpolation runs in linear space (`GAMMA_1_0`) to match them; on an
    /// 8-bit sRGB target the colors are linear→sRGB encoded and interpolation stays
    /// in the perceptual `GAMMA_2_2` space (matching the encoded endpoints).
    fn resolve_stops(&self, stops: &[GradientStop]) -> (Vec<D2D1_GRADIENT_STOP>, D2D1_GAMMA) {
        let abi: Vec<D2D1_GRADIENT_STOP> = stops
            .iter()
            .map(|s| D2D1_GRADIENT_STOP {
                position: s.position,
                color: self.resolve(s.color).into(),
            })
            .collect();
        let gamma = if self.encode_srgb {
            D2D1_GAMMA_2_2
        } else {
            D2D1_GAMMA_1_0
        };
        (abi, gamma)
    }

    /// Stops define colors at positions 0.0–1.0 along the axis from `start` to `end`.
    pub fn create_linear_gradient(
        &self,
        start: Vector2,
        end: Vector2,
        stops: &[GradientStop],
    ) -> Result<LinearGradient> {
        let (abi_stops, gamma) = self.resolve_stops(stops);
        unsafe {
            let collection = self.context.CreateGradientStopCollection(
                &abi_stops,
                gamma,
                D2D1_EXTEND_MODE_CLAMP,
            )?;
            let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: start,
                endPoint: end,
            };
            self.context
                .CreateLinearGradientBrush(&props, None, &collection)
                .map(LinearGradient)
        }
    }

    /// Stops define colors at positions 0.0 (center) to 1.0 (edge).
    pub fn create_radial_gradient(
        &self,
        center: Vector2,
        radius_x: f32,
        radius_y: f32,
        stops: &[GradientStop],
    ) -> Result<RadialGradient> {
        let (abi_stops, gamma) = self.resolve_stops(stops);
        unsafe {
            let collection = self.context.CreateGradientStopCollection(
                &abi_stops,
                gamma,
                D2D1_EXTEND_MODE_CLAMP,
            )?;
            let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                center,
                gradientOriginOffset: Vector2::new(0.0, 0.0),
                radiusX: radius_x,
                radiusY: radius_y,
            };
            self.context
                .CreateRadialGradientBrush(&props, None, &collection)
                .map(RadialGradient)
        }
    }

    /// Draws text within a rectangle using the given format and brush.
    pub fn draw_text(&self, text: &str, format: &TextFormat, rect: &Rect, brush: &impl Paint) {
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.context.DrawText(
                &wide,
                format.raw(),
                &rect.to_abi(),
                brush.as_raw_brush(),
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                0,
            );
        }
    }

    /// Draws a pre-measured [`TextLayout`] with its top-left at `origin` (DIPs).
    ///
    /// Prefer this over [`draw_text`](Self::draw_text) whenever you also need to
    /// measure / hit-test / trim the text — build the [`TextLayout`] once, size
    /// your UI from [`TextLayout::metrics`], then draw it here.
    pub fn draw_text_layout(&self, origin: Vector2, layout: &TextLayout, brush: &impl Paint) {
        unsafe {
            self.context.DrawTextLayout(
                origin,
                layout.raw(),
                brush.as_raw_brush(),
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
    }

    /// Draws the outline of a path.
    pub fn draw_path(&self, path: &Path, brush: &impl Paint, width: f32) {
        unsafe {
            self.context
                .DrawGeometry(path.raw(), brush.as_raw_brush(), width, None);
        }
    }

    /// Draws the outline of a path using the given stroke style.
    pub fn draw_path_styled(
        &self,
        path: &Path,
        brush: &impl Paint,
        width: f32,
        style: &StrokeStyle,
    ) {
        unsafe {
            self.context
                .DrawGeometry(path.raw(), brush.as_raw_brush(), width, &style.0);
        }
    }

    /// Fills a path.
    pub fn fill_path(&self, path: &Path, brush: &impl Paint) {
        unsafe {
            self.context
                .FillGeometry(path.raw(), brush.as_raw_brush(), None);
        }
    }

    /// Draws a bitmap into the destination rectangle with the given opacity.
    pub fn draw_bitmap(&self, bitmap: &Bitmap, dest: &Rect, opacity: f32) {
        unsafe {
            self.context.DrawBitmap(
                &bitmap.0,
                Some(&dest.to_abi()),
                opacity,
                D2D1_INTERPOLATION_MODE_LINEAR,
                None,
                None,
            );
        }
    }

    /// Loads a bitmap from an image file.
    pub fn load_bitmap(&self, path: impl AsRef<std::path::Path>) -> Result<Bitmap> {
        Bitmap::load_from_file(self.context, path.as_ref())
    }

    /// Sets the current transform.
    pub fn set_transform(&self, transform: &Matrix3x2) {
        unsafe { self.context.SetTransform(transform) };
    }

    /// Returns the current transform.
    pub fn get_transform(&self) -> Matrix3x2 {
        let mut transform = Matrix3x2::default();
        unsafe { self.context.GetTransform(&mut transform) };
        transform
    }

    /// Apply a transform for the duration of the closure, then restore the previous one.
    pub fn with_transform(&self, transform: &Matrix3x2, f: impl FnOnce()) {
        let prev = self.get_transform();
        self.set_transform(transform);
        f();
        self.set_transform(&prev);
    }

    /// Push an axis-aligned (aliased) clip rectangle. Subsequent drawing is
    /// confined to `rect` until the matching [`pop_clip`](Self::pop_clip). Used
    /// by the text editor to confine an overflowing single-line run to its box.
    pub fn push_clip(&self, rect: &Rect) {
        // 1 == D2D1_ANTIALIAS_MODE_ALIASED (crisp box edges, no clip-edge AA).
        unsafe { self.context.PushAxisAlignedClip(&rect.to_abi(), 1) };
    }

    /// Pop the clip pushed by [`push_clip`](Self::push_clip).
    pub fn pop_clip(&self) {
        unsafe { self.context.PopAxisAlignedClip() };
    }

    /// Returns the underlying `ID2D1DeviceContext`.
    pub fn raw(&self) -> &ID2D1DeviceContext {
        self.context
    }

    /// Switch text antialiasing to grayscale. Required for correct text on
    /// premultiplied / transparent composition surfaces: ClearType subpixel AA
    /// blends against an assumed-opaque background and is invalid there.
    pub fn set_grayscale_text_antialiasing(&self) {
        unsafe {
            self.context
                .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE)
        };
    }

    /// Creates a bitmap suitable for use as a render target.
    ///
    /// The pixel format follows the session's pipeline: **FP16
    /// (`R16G16B16A16_FLOAT`) on a linear scRGB target**, so an offscreen
    /// intermediate (a glow shape, a cached layer) carries extended-range values
    /// — negatives and `> 1.0` headroom — without an SDR clamp on the way back
    /// to the surface; 8-bit `B8G8R8A8` only when the session is encoding for an
    /// 8-bit sRGB target, where the round-trip is clamped anyway.
    pub fn create_bitmap_target(&self) -> Result<Bitmap> {
        unsafe {
            let mut dpi_x = 0.0f32;
            let mut dpi_y = 0.0f32;
            self.context.GetDpi(&mut dpi_x, &mut dpi_y);
            let pixel_size = self.context.GetPixelSize();

            let format = if self.encode_srgb {
                DXGI_FORMAT_B8G8R8A8_UNORM
            } else {
                DXGI_FORMAT_R16G16B16A16_FLOAT
            };
            let properties = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi_x,
                dpiY: dpi_y,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };

            self.context
                .CreateBitmap(pixel_size, None, 0, &properties)
                .map(Bitmap)
        }
    }

    /// Creates a Gaussian shadow/glow effect from `source`: the source's alpha channel
    /// is blurred by `blur_standard_deviation` (DIPs) and tinted with `color`. Draw its
    /// output with [`draw_effect`](Self::draw_effect) — at the identity transform it
    /// reads as a centered glow; under a translation, a drop shadow. `source` must be an
    /// effect-readable image (e.g. a [`create_bitmap_target`](Self::create_bitmap_target)
    /// bitmap that has been drawn into and is not the current target).
    pub fn create_shadow(
        &self,
        source: &Bitmap,
        blur_standard_deviation: f32,
        color: ColorF,
    ) -> Result<Effect> {
        unsafe {
            let effect = self.context.CreateEffect(&CLSID_D2D1Shadow)?;
            effect.SetInput(0, &source.0, true);
            // Blur is a FLOAT; color is a straight-RGBA VECTOR4 (D2D1_COLOR_F layout).
            effect.SetValue(
                D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION as u32,
                D2D1_PROPERTY_TYPE_FLOAT,
                &blur_standard_deviation.to_le_bytes(),
            )?;
            // The tint composites into the surface, so it follows the same color
            // space: linear on an FP16 target, sRGB passthrough otherwise.
            let color = self.resolve(color);
            let mut rgba = [0u8; 16];
            rgba[0..4].copy_from_slice(&color.r.to_le_bytes());
            rgba[4..8].copy_from_slice(&color.g.to_le_bytes());
            rgba[8..12].copy_from_slice(&color.b.to_le_bytes());
            rgba[12..16].copy_from_slice(&color.a.to_le_bytes());
            effect.SetValue(D2D1_SHADOW_PROP_COLOR as u32, D2D1_PROPERTY_TYPE_VECTOR4, &rgba)?;
            Ok(Effect(effect))
        }
    }

    /// Redirect drawing to a bitmap target for the duration of the closure.
    pub fn with_target(&self, bitmap: &Bitmap, f: impl FnOnce()) {
        unsafe {
            let previous = self.context.GetTarget();
            self.context.SetTarget(&bitmap.0);
            f();
            match previous {
                Ok(prev) => self.context.SetTarget(&prev),
                Err(_) => self.context.SetTarget(None::<&ID2D1Image>),
            }
        }
    }

    /// Draw a bitmap at its natural size at the current transform.
    pub fn draw_image(&self, bitmap: &Bitmap) {
        unsafe {
            self.context.DrawImage(
                &bitmap.0,
                None,
                None,
                D2D1_INTERPOLATION_MODE_LINEAR,
                0, // D2D1_COMPOSITE_MODE_SOURCE_OVER
            );
        }
    }

    /// Paint a soft Gaussian drop shadow beneath an arbitrary shape. `draw_shape`
    /// renders the opaque silhouette into a transparent off-screen bitmap (only its
    /// alpha matters); that alpha is blurred by `blur_standard_deviation` (DIPs),
    /// tinted with `color`, and composited at `offset` DIPs `(dx, dy)` — positive
    /// values push the shadow down/right for a classic drop. The caller paints the
    /// real surface on top afterwards (the shape's own ink is not redrawn here).
    ///
    /// This is the chrome counterpart to the viz `glow` (a centered halo): same
    /// off-screen `D2D1Shadow` mechanism, but translated. It mirrors the glow path's
    /// atlas-offset handling so it is correct on a DirectComposition composition
    /// surface (whose transform carries the atlas slot in `m31`/`m32`). Returns
    /// `false` if any off-screen step fails (e.g. under device loss), so callers can
    /// fall back to an approximate shadow.
    pub fn drop_shadow(
        &self,
        blur_standard_deviation: f32,
        color: ColorF,
        offset: (f32, f32),
        draw_shape: impl FnOnce(),
    ) -> bool {
        let Ok(shape) = self.create_bitmap_target() else {
            return false;
        };
        // Render the silhouette at scale only — strip the atlas translation so it is
        // not baked into the bitmap and then re-applied at composite (double-offset).
        let live = self.get_transform();
        let scale_only = Matrix3x2 { m31: 0.0, m32: 0.0, ..live };
        self.with_target(&shape, || {
            self.with_transform(&scale_only, || {
                self.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
                draw_shape();
            });
        });
        let Ok(shadow) = self.create_shadow(&shape, blur_standard_deviation, color) else {
            return false;
        };
        // Composite the blurred shadow at the live atlas offset plus the drop offset
        // (DIPs → pixels via the surface scale), scale forced to 1 — the shape bitmap
        // already holds scaled pixels, so compositing under the scale transform would
        // scale it twice.
        let scale = live.m11;
        let blit = Matrix3x2 {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: live.m31 + offset.0 * scale,
            m32: live.m32 + offset.1 * scale,
        };
        self.with_transform(&blit, || self.draw_effect(&shadow));
        true
    }

    /// Draws the output of an effect.
    pub fn draw_effect(&self, effect: &Effect) {
        if let Ok(output) = unsafe { effect.0.GetOutput() } {
            unsafe {
                self.context.DrawImage(
                    &output,
                    None,
                    None,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    0, // D2D1_COMPOSITE_MODE_SOURCE_OVER
                );
            }
        }
    }
}

impl Drop for DrawingSession<'_> {
    fn drop(&mut self) {
        // A borrowed session does not own the bracket: the `SurfaceImageSource`
        // that opened the draw is responsible for `EndDraw` and for reporting
        // device-loss from it.
        if !self.owns_bracket {
            return;
        }
        unsafe {
            let result = self.context.EndDraw(None, None);
            if is_device_lost(result) {
                self.device_lost_flag.set(true);
            }
        }
    }
}
