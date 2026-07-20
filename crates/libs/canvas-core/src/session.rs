use super::*;

/// Safe wrapper over `ID2D1DeviceContext`.
///
/// A session drives either a swap chain (owning the `BeginDraw`/`EndDraw`
/// bracket) or a borrowed context that is already in a drawing state, such as the
/// one handed back by a `SurfaceImageSource`. In the borrowed case an `offset`
/// translation shifts every draw into the shared atlas surface; it composes with
/// caller transforms so `set_transform`/`with_transform` behave as if the surface
/// origin were `(0, 0)`.
pub struct DrawingSession<'a> {
    context: &'a ID2D1DeviceContext,
    mode: Mode<'a>,
    // Whether the draw target is an 8-bit sRGB surface. Every color entering a
    // session is *linear* scRGB; when this is set, each color the session forwards
    // to Direct2D — solid brushes (including later recolors), gradient stops, the
    // clear color, effect tints — is linear→sRGB encoded (+ clamped) on the way out,
    // so a linear value lands correctly on a UNORM sRGB surface. A linear FP16 scRGB
    // surface leaves this false and passes colors through raw (its native encoding).
    encode_srgb: bool,
}

/// How a [`DrawingSession`] relates to its `ID2D1DeviceContext`.
enum Mode<'a> {
    /// The session owns the `BeginDraw`/`EndDraw` bracket (swap-chain path) and
    /// flags device loss reported by `EndDraw`.
    Owned { device_lost_flag: &'a Cell<bool> },
    /// The context is already in a drawing state and is bracketed by its owner
    /// (e.g. a `SurfaceImageSource`), so the session neither begins nor ends
    /// drawing — a nested Direct2D `BeginDraw` there is `D2DERR_WRONG_STATE`.
    /// `offset` is a pure translation mapping surface-local coordinates onto the
    /// shared-atlas position.
    Borrowed { offset: Matrix3x2 },
}

impl<'a> DrawingSession<'a> {
    pub(crate) fn new(
        context: &'a ID2D1DeviceContext,
        device_lost_flag: &'a Cell<bool>,
    ) -> Result<Self> {
        unsafe { context.BeginDraw() };
        Ok(Self {
            context,
            mode: Mode::Owned { device_lost_flag },
            encode_srgb: false,
        })
    }

    /// Adopt a context that is *already* in a draw (its `BeginDraw`/`EndDraw`
    /// bracket is owned elsewhere — e.g. a `SurfaceImageSource`'s native
    /// `BeginDraw`). This session issues no `BeginDraw` and no `EndDraw`; the
    /// owner is responsible for ending the draw and for observing device-loss
    /// from that call. Used by the reactor surface bridges and by the
    /// self-hosted DirectComposition backend.
    ///
    /// `offset` is a pure translation applied beneath every draw (composing with
    /// caller transforms), so you draw from a `(0, 0)` origin even when the target
    /// is a sub-region of a shared atlas surface; pass
    /// `Matrix3x2::translation(0.0, 0.0)` for no offset.
    pub fn from_borrowed_context(context: &'a ID2D1DeviceContext, offset: Matrix3x2) -> Self {
        debug_assert!(
            offset.m11 == 1.0 && offset.m12 == 0.0 && offset.m21 == 0.0 && offset.m22 == 1.0,
            "offset must be a pure translation: get_transform decomposes it by negating m31/m32"
        );
        unsafe { context.SetTransform(&offset) };
        Self {
            context,
            mode: Mode::Borrowed { offset },
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
    /// `DrawingSession::from_borrowed_context(ctx, offset).encode_srgb_target(surface_is_8bit)`.
    pub fn encode_srgb_target(mut self, on: bool) -> Self {
        self.encode_srgb = on;
        self
    }

    /// Whether this session encodes linear→sRGB on the way out, i.e. whether its
    /// target is an 8-bit sRGB surface rather than a linear FP16 one. An off-screen
    /// layer rendered for this target must match, or it composites with the wrong
    /// transfer function.
    pub fn encodes_srgb(&self) -> bool {
        self.encode_srgb
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

    /// Resolve a stop list into ABI stops for this session's target: linear→sRGB
    /// encoded on an 8-bit sRGB target, raw linear on an FP16 one.
    fn resolve_stops(&self, stops: &[GradientStop]) -> Vec<D2D1_GRADIENT_STOP> {
        stops
            .iter()
            .map(|s| D2D1_GRADIENT_STOP {
                position: s.position,
                color: self.resolve(s.color).into(),
            })
            .collect()
    }

    /// Build the stop collection matched to the target.
    ///
    /// Direct2D realizes every gradient brush by baking its stops into a small
    /// lookup texture, and the D2D1.0 `CreateGradientStopCollection` bakes an
    /// **8-bit** texture unconditionally — on a linear FP16 target a subtle
    /// near-black gradient then posterizes into a handful of flat rings *at the
    /// brush*, before compositing, no matter how smooth the authored stops are.
    /// So on a linear target this uses the D2D1.1 overload with a
    /// **`FLOAT16` gradient texture** (scRGB in/out, straight-alpha
    /// interpolation in linear light); the 8-bit sRGB target keeps the classic
    /// gamma-2.2 collection, whose 8-bit texture matches its own surface depth.
    fn create_stop_collection(
        &self,
        abi_stops: &[D2D1_GRADIENT_STOP],
    ) -> Result<ID2D1GradientStopCollection> {
        unsafe {
            if self.encode_srgb {
                ID2D1RenderTarget::CreateGradientStopCollection(
                    &self.context,
                    abi_stops,
                    D2D1_GAMMA_2_2,
                    D2D1_EXTEND_MODE_CLAMP,
                )
            } else {
                self.context
                    .CreateGradientStopCollection(
                        abi_stops,
                        D2D1_COLOR_SPACE_SCRGB,
                        D2D1_COLOR_SPACE_SCRGB,
                        D2D1_BUFFER_PRECISION_16BPC_FLOAT,
                        D2D1_EXTEND_MODE_CLAMP,
                        D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT,
                    )
                    .map(|c| c.cast())
                    .and_then(|c| c)
            }
        }
    }

    /// Stops define colors at positions 0.0–1.0 along the axis from `start` to `end`.
    pub fn create_linear_gradient(
        &self,
        start: Vector2,
        end: Vector2,
        stops: &[GradientStop],
    ) -> Result<LinearGradient> {
        let abi_stops = self.resolve_stops(stops);
        let collection = self.create_stop_collection(&abi_stops)?;
        unsafe {
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
        let abi_stops = self.resolve_stops(stops);
        let collection = self.create_stop_collection(&abi_stops)?;
        unsafe {
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
        let m = match &self.mode {
            Mode::Borrowed { offset } => *transform * *offset,
            Mode::Owned { .. } => *transform,
        };
        unsafe { self.context.SetTransform(&m) };
    }

    /// Returns the current transform.
    pub fn get_transform(&self) -> Matrix3x2 {
        let mut transform = Matrix3x2::default();
        unsafe { self.context.GetTransform(&mut transform) };
        match &self.mode {
            // Undo the atlas offset (a pure translation) so callers see the
            // surface origin as `(0, 0)`.
            Mode::Borrowed { offset } => transform * Matrix3x2::translation(-offset.m31, -offset.m32),
            Mode::Owned { .. } => transform,
        }
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
    ///
    /// Text rendering params (see [`crate::text::text_rendering_params`]) use a
    /// display-referred **~2.2 blend gamma** on both the sRGB and the linear FP16
    /// scRGB targets, grid-fit ENABLED, and the modern NATURAL_SYMMETRIC grayscale
    /// outline mode. A linear (1.0) blend gamma erodes light text over fills, so
    /// re-tuning must A/B on the dcomp backend.
    pub fn set_grayscale_text_antialiasing(&self) {
        unsafe {
            self.context
                .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            if let Some(params) = text_rendering_params(!self.encode_srgb) {
                self.context.SetTextRenderingParams(&params);
            }
        }
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

    /// Upload an FP16 (`R16G16B16A16_FLOAT`) bitmap from RGBA `f32` texels
    /// (`width_px × height_px × 4` values, row-major, premultiplied alpha, linear
    /// extended-range — negatives and `> 1.0` are preserved). The bitmap's DPI is
    /// stamped from the session, so through a [`create_tiling_brush`]
    /// (Self::create_tiling_brush) one texel lands on exactly one physical pixel.
    ///
    /// The signed-texel upload primitive for dither / noise tiles (a zero-mean
    /// tile needs negative values, which only a FLOAT format carries).
    pub fn create_bitmap_fp16(
        &self,
        width_px: u32,
        height_px: u32,
        rgba: &[f32],
    ) -> Result<Bitmap> {
        assert_eq!(
            rgba.len(),
            (width_px * height_px * 4) as usize,
            "texel buffer must be width*height*4 f32s"
        );
        let half: Vec<u16> = rgba.iter().map(|&v| f32_to_f16(v)).collect();
        unsafe {
            let mut dpi_x = 0.0f32;
            let mut dpi_y = 0.0f32;
            self.context.GetDpi(&mut dpi_x, &mut dpi_y);
            let properties = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_R16G16B16A16_FLOAT,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi_x,
                dpiY: dpi_y,
                bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
                ..Default::default()
            };
            self.context
                .CreateBitmap(
                    D2D_SIZE_U {
                        width: width_px,
                        height: height_px,
                    },
                    Some(half.as_ptr().cast()),
                    width_px * 8,
                    &properties,
                )
                .map(Bitmap)
        }
    }

    /// A wrap-extended, nearest-sampled bitmap brush: one
    /// [`fill_rect`](Self::fill_rect) /
    /// [`fill_rect_additive`](Self::fill_rect_additive) repeats `bitmap` across the
    /// filled area, each texel on one physical pixel (the bitmap carries the
    /// session DPI). The tiled-texture primitive the backdrop's blue-noise dither
    /// composite uses.
    pub fn create_tiling_brush(&self, bitmap: &Bitmap) -> Result<TilingBrush> {
        let props = D2D1_BITMAP_BRUSH_PROPERTIES1 {
            extendModeX: D2D1_EXTEND_MODE_WRAP,
            extendModeY: D2D1_EXTEND_MODE_WRAP,
            interpolationMode: D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        };
        unsafe {
            self.context
                .CreateBitmapBrush(&bitmap.0, Some(&props), None)
                .map(TilingBrush)
        }
    }

    /// Fill a rectangle with **additive** blending (`out = dst + src`), restoring
    /// the default source-over blend afterwards. With a zero-mean signed tile
    /// (see [`create_bitmap_fp16`](Self::create_bitmap_fp16)) this is the standard
    /// GPU ordered-dither composite: it perturbs what is already painted without
    /// shifting its mean.
    pub fn fill_rect_additive(&self, rect: &Rect, brush: &impl Paint) {
        unsafe {
            self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_ADD);
            self.context
                .FillRectangle(&rect.to_abi(), brush.as_raw_brush());
            self.context
                .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
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
            )
            .ok()?;
            // The tint composites into the surface, so it follows the same color
            // space: linear on an FP16 target, sRGB passthrough otherwise.
            let color = self.resolve(color);
            let mut rgba = [0u8; 16];
            rgba[0..4].copy_from_slice(&color.r.to_le_bytes());
            rgba[4..8].copy_from_slice(&color.g.to_le_bytes());
            rgba[8..12].copy_from_slice(&color.b.to_le_bytes());
            rgba[12..16].copy_from_slice(&color.a.to_le_bytes());
            effect.SetValue(D2D1_SHADOW_PROP_COLOR as u32, D2D1_PROPERTY_TYPE_VECTOR4, &rgba)
                .ok()?;
            Ok(Effect(effect))
        }
    }

    /// Creates a **color-preserving** Gaussian blur effect from `source`: every
    /// channel is blurred by `blur_standard_deviation` (DIPs) — unlike
    /// [`create_shadow`](Self::create_shadow), which blurs only the alpha channel and
    /// re-tints it with a single color. Draw its output with
    /// [`draw_effect`](Self::draw_effect). Use it for a bloom that must keep a
    /// multi-colored source's own hues (e.g. a gradient-stroked line glowing in its
    /// own colors along its length). `source` must be an effect-readable image (see
    /// [`create_shadow`](Self::create_shadow)).
    pub fn create_blur(&self, source: &Bitmap, blur_standard_deviation: f32) -> Result<Effect> {
        unsafe {
            let effect = self.context.CreateEffect(&CLSID_D2D1GaussianBlur)?;
            effect.SetInput(0, &source.0, true);
            effect.SetValue(
                D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION as u32,
                D2D1_PROPERTY_TYPE_FLOAT,
                &blur_standard_deviation.to_le_bytes(),
            )
            .ok()?;
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
    /// off-screen `D2D1Shadow` mechanism, but translated. Works unchanged on a
    /// borrowed atlas surface: it reads and writes the transform through the
    /// session, which adds and removes the atlas slot itself. Returns `false` if
    /// any off-screen step fails (e.g. under device loss), so callers can fall
    /// back to an approximate shadow.
    pub fn drop_shadow(
        &self,
        renderer: &LayerRenderer,
        size_px: (u32, u32),
        blur_standard_deviation: f32,
        color: ColorF,
        offset: (f32, f32),
        draw_shape: impl FnOnce(&DrawingSession<'_>),
    ) -> bool {
        let live = self.get_transform();
        // Render the silhouette on the caller's OFF-SCREEN renderer, never by
        // retargeting this session. Retargeting mid-`BeginDraw` makes Direct2D
        // resolve the batched work against the current target first, which on a
        // presented surface publishes a half-drawn frame the compositor can sample —
        // a popup whose shadow flickers as it opens. A scratch context also carries
        // no atlas translation, so the silhouette cannot bake in an offset that the
        // composite below would then apply a second time.
        let Ok(shape) = renderer.render(size_px, live.m11, !self.encode_srgb, draw_shape) else {
            return false;
        };
        let Ok(shadow) = self.create_shadow(&shape, blur_standard_deviation, color) else {
            return false;
        };
        // Composite the blurred shadow at the live translation plus the drop offset
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
        let Mode::Owned { device_lost_flag } = self.mode else {
            return;
        };
        unsafe {
            let result = self.context.EndDraw(None, None);
            if is_device_lost(result) {
                device_lost_flag.set(true);
            }
        }
    }
}


/// Convert an `f32` to IEEE 754 binary16 bits, round-to-nearest (ties away from
/// zero) — the upload conversion for [`DrawingSession::create_bitmap_fp16`].
/// Handles signs, subnormals, overflow-to-infinity, and NaN; precision loss at
/// these magnitudes is far below any visible threshold.
fn f32_to_f16(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let man = bits & 0x007f_ffff;
    if exp == 0xff {
        // Infinity / NaN (keep NaN signalling-agnostic with a set payload bit).
        return sign | 0x7c00 | if man != 0 { 0x0200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00; // overflow -> infinity
    }
    if e <= 0 {
        if e < -10 {
            return sign; // underflow -> signed zero
        }
        // Subnormal half: shift the (restored-implicit-bit) mantissa into place.
        let man = man | 0x0080_0000;
        let shift = (14 - e) as u32;
        let half_man = (man >> shift) as u16;
        let round = ((man >> (shift - 1)) & 1) as u16;
        return sign | (half_man + round);
    }
    let half = ((e as u32) << 10) | (man >> 13);
    let round = (man >> 12) & 1;
    sign | (half + round) as u16
}
