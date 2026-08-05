//! The drawing bracket, the target binding, and everything drawn inside one.
//!
//! [`Pass`] is the `BeginDraw`/`EndDraw` bracket: one per device at a time, spanning
//! however many targets a present pass touches. [`Draw`] is one target bound inside that
//! bracket, and [`Pass::draw`] takes `&mut self`, so the previous binding drops before the
//! next one exists. One bracket per pass and one target bound at a time are both enforced
//! by the borrow checker.

use super::*;
use core::cell::Cell;
use core::mem::ManuallyDrop;

/// The Direct2D drawing bracket. Opens on construction, closes on [`end`](Pass::end) or
/// on drop.
///
/// One bracket spans the whole pass. Direct2D charges a fixed cost per pair — a DXGI
/// `ReclaimResources`, an `OfferResources`, and a delayed Direct3D device-context-state
/// swap — that does not depend on what was drawn between them, so the cost scales with the
/// number of brackets.
pub struct Pass<'g> {
    gpu: &'g Gpu,
}

/// A pass that latched an error, and the tag that names where.
///
/// Direct2D records commands and does the work at `EndDraw`, and a failed call latches an
/// error on the context that discards every later draw in the same bracket. The bracket
/// spans every target in the pass, so the tag is what separates a draw that failed from a
/// draw killed by an earlier failure.
#[derive(Copy, Clone, Debug)]
pub struct PassError {
    /// The `HRESULT` `EndDraw` reported.
    pub hr: windows_core::HRESULT,
    /// Whatever [`Pass::tag`] last set before the failing call.
    pub tag: u64,
    /// What `hr` means for recovery.
    pub loss: Loss,
}

impl<'g> Pass<'g> {
    pub(crate) fn new(gpu: &'g Gpu) -> Self {
        Self { gpu }
    }

    /// Binds `target` and returns the drawing surface for it, in DIPs at the target's own
    /// DPI, with its origin at `(0, 0)`.
    ///
    /// `SetTarget` is legal at any time, including while the context is drawing, so a
    /// cached intermediate is rendered by retargeting the open bracket rather than by a
    /// second context.
    ///
    /// The context DPI comes from the target rather than from an argument, so the DPI the
    /// content is drawn at and the DPI the bitmap was built for cannot disagree — and it is
    /// restated at every bind rather than once at construction, because each target in a
    /// pass may be at a different scale.
    pub fn draw(&mut self, target: &Target) -> Draw<'_> {
        let ctx = self.gpu.ctx();
        let dpi = target.dpi();
        unsafe {
            ctx.SetTarget(&target.bitmap);
            ctx.SetDpi(dpi, dpi);
        }
        Draw {
            ctx,
            opacity: target.opacity,
            dpi,
            layers: Cell::new(0),
            unbind: true,
        }
    }

    /// Labels what draws next, so a latched error names its target rather than the batch.
    ///
    /// Called once per retarget: a tag per primitive costs a call per primitive to answer a
    /// question asked once per pass.
    pub fn tag(&self, tag: u64) {
        unsafe { self.gpu.ctx().SetTags(tag, 0) };
    }

    /// Closes the bracket and reports what it latched.
    ///
    /// Dropping a `Pass` closes it too and discards the result, which keeps a panic from
    /// leaving `BeginDraw` outstanding.
    ///
    /// # Errors
    ///
    /// The first failure `EndDraw` latched, with the tag active at the failing call.
    pub fn end(self) -> core::result::Result<(), PassError> {
        self.close()
    }

    /// Closes the bracket if it is open. Idempotent through the device's own open flag, so
    /// there is no second bit of state to keep in step.
    fn close(&self) -> core::result::Result<(), PassError> {
        if !self.gpu.drawing().replace(false) {
            return Ok(());
        }
        let (mut tag, mut _unused) = (0u64, 0u64);
        let hr = unsafe { self.gpu.ctx().EndDraw(Some(&mut tag), Some(&mut _unused)) };
        if hr.is_ok() {
            Ok(())
        } else {
            Err(PassError {
                hr,
                tag,
                loss: classify(hr),
            })
        }
    }
}

impl Drop for Pass<'_> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// One target bound inside a [`Pass`], in DIPs.
///
/// The target's contents are **undefined on entry**: a renderer either clears or covers its
/// whole box. A `Draw` is not `Send`, and it decides nothing about when to draw; that
/// belongs to whoever opened the pass.
pub struct Draw<'p> {
    ctx: &'p ID2D1DeviceContext6,
    opacity: Opacity,
    dpi: f32,
    layers: Cell<u32>,
    unbind: bool,
}

impl<'p> Draw<'p> {
    /// Wraps a context whose target is already bound by something else — a composition
    /// drawing surface's `BeginDraw`, which hands back a context created for that call.
    #[cfg(feature = "composition")]
    pub(crate) fn borrowed(ctx: &'p ID2D1DeviceContext6, opacity: Opacity, dpi: f32) -> Self {
        Self {
            ctx,
            opacity,
            dpi,
            layers: Cell::new(0),
            unbind: false,
        }
    }

    /// Returns the DPI this target is bound at.
    #[must_use]
    pub fn dpi(&self) -> f32 {
        self.dpi
    }

    /// Returns the DIP-to-pixel factor.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.dpi / 96.0
    }

    // ── pixel snapping ────────────────────────────────────────────────────────────
    //
    // Direct2D snaps nothing except text baselines, and only in the text APIs this crate
    // does not use. Everywhere else a physical pixel may land at a fractional DIP
    // coordinate, which is the expected condition of a floating-point coordinate space, and
    // aligning to the pixel grid is the caller's arithmetic. That arithmetic lives here.
    //
    // `D2D1_ANTIALIAS_MODE_ALIASED` is not a substitute: it makes an edge hard, not
    // aligned, so an unsnapped aliased hairline is a crisp line in the wrong pixel.

    /// Rounds a DIP coordinate to the nearest whole physical pixel.
    #[must_use]
    pub fn snap(&self, dip: f32) -> f32 {
        snap_to(dip, self.scale())
    }

    /// Rounds every edge of a box to whole physical pixels.
    ///
    /// For **fills and blits**. A stroke centred on a snapped edge straddles the boundary,
    /// which for an odd physical width covers two pixels at half coverage each — use
    /// [`snap_stroke`](Self::snap_stroke) for that.
    #[must_use]
    pub fn snap_rect(&self, r: Rect) -> Rect {
        Rect::new(
            self.snap(r.left),
            self.snap(r.top),
            self.snap(r.right),
            self.snap(r.bottom),
        )
    }

    /// Positions a box so a stroke along its edges lands on whole pixels.
    ///
    /// A stroke is centred on the path, so where its centre sits depends on the parity of
    /// its physical width: an odd number of pixels covers whole pixels when centred on a
    /// pixel *centre*, an even number when centred on a *boundary*. Centring an odd width on
    /// a boundary turns a one-pixel rule into two half-covered rows; at 96 DPI the two
    /// placements coincide for the widths a design uses, so that error is invisible there.
    ///
    /// The **width is not snapped**. A 1-DIP rule is 1.5 physical pixels at 1.5×, and no
    /// placement makes that crisp on both edges; rounding it to 1 or 2 changes the line
    /// weight by a third, and design metrics stay DIP-constant. The box moves; the width
    /// does not.
    #[must_use]
    pub fn snap_stroke(&self, r: Rect, k: Stroke<'_>) -> Rect {
        snap_stroke_to(r, k.width, self.scale())
    }

    // ── primitives ────────────────────────────────────────────────────────────────

    /// Fills the whole target — or, inside a [`clip`](Self::clip), the clipped area.
    ///
    /// Direct2D reads a colour as straight alpha whatever the target's alpha mode, and
    /// premultiplies on the way in; a target that ignores alpha replaces this one with
    /// fully opaque.
    pub fn clear(&self, color: Scrgb) {
        unsafe { self.ctx.Clear(Some(d2d_color(&color))) };
    }

    /// Fills `shape` with `brush`.
    pub fn fill<'s>(&self, shape: impl Into<Shape<'s>>, brush: &impl Brush) {
        let brush = brush.brush().raw();
        unsafe {
            match shape.into() {
                Shape::Rect(r) => self.ctx.FillRectangle(r.d2d(), brush),
                Shape::Round(r) => self.ctx.FillRoundedRectangle(r.d2d(), brush),
                Shape::Ellipse(e) => self.ctx.FillEllipse(e.d2d(), brush),
                Shape::Path(p) => self.ctx.FillGeometry(p.raw(), brush, None),
            }
        }
    }

    /// Strokes the outline of `shape` with `brush`, `k` wide.
    pub fn stroke<'s>(&self, shape: impl Into<Shape<'s>>, brush: &impl Brush, k: Stroke<'_>) {
        let brush = brush.brush().raw();
        let (w, style) = k.parts();
        unsafe {
            match shape.into() {
                Shape::Rect(r) => self.ctx.DrawRectangle(r.d2d(), brush, w, style),
                Shape::Round(r) => self.ctx.DrawRoundedRectangle(r.d2d(), brush, w, style),
                Shape::Ellipse(e) => self.ctx.DrawEllipse(e.d2d(), brush, w, style),
                Shape::Path(p) => self.ctx.DrawGeometry(p.raw(), brush, w, style),
            }
        }
    }

    /// Strokes a line from `from` to `to`.
    ///
    /// A line cannot be filled, so it is its own call rather than a [`Shape`] arm that
    /// would be meaningless in [`fill`](Self::fill).
    pub fn line(&self, from: Vector2, to: Vector2, brush: &impl Brush, k: Stroke<'_>) {
        let (w, style) = k.parts();
        unsafe {
            self.ctx.DrawLine(from, to, brush.brush().raw(), w, style);
        }
    }

    /// Blits `src`. Its source rectangle is in the **source's** DIPs, which are the same
    /// DIPs as this target's because every target carries the display's DPI.
    ///
    /// **The destination is snapped here**, unlike a fill or a stroke: the source was
    /// rasterized at device resolution, so landing it off the pixel grid resamples finished
    /// pixels and softens a cached glyph run.
    ///
    /// This is `DrawBitmap` and not `DrawImage`. `DrawImage` accepts any `ID2D1Image` and
    /// runs Direct2D's image command graph to discover what it was handed, at several times
    /// the cost per call. There is no general image draw here.
    pub fn blit(&self, src: &Target, dest: Rect, src_rect: Option<Rect>, interp: Interp) {
        let dest = self.snap_rect(dest);
        unsafe {
            self.ctx.DrawBitmap(
                &src.bitmap,
                Some(dest.d2d()),
                1.0,
                interp.image(),
                src_rect.as_ref().map(Rect::d2d),
                None,
            );
        }
    }

    /// Draws a whole sprite batch, sampling `src`.
    ///
    /// Sets aliased mode for the duration and restores what it found: with per-primitive
    /// antialiasing on, `DrawSpriteBatch` does not draw, and because it returns `void` the
    /// failure surfaces only as an error latched on the context, which then discards the
    /// rest of the pass.
    pub fn sprites(&self, batch: &SpriteBatch, src: &Target, interp: Interp) {
        let count = batch.len() as u32;
        if count == 0 {
            return;
        }
        // Splitting an oversized batch takes a `Flush` between the halves, and a `Flush`
        // with a layer outstanding puts the target into an error state. Nothing splits, so
        // the ceiling is asserted.
        debug_assert!(
            count <= SpriteBatch::CEILING || self.layers.get() == 0,
            "a batch over {} sprites needs a Flush to split, which is illegal inside a layer",
            SpriteBatch::CEILING
        );
        let _aliased = self.aliased();
        unsafe {
            self.ctx.DrawSpriteBatch(
                batch.raw(),
                0,
                count,
                &src.bitmap,
                interp.bitmap(),
                D2D1_SPRITE_OPTIONS_NONE,
            );
        }
    }

    /// Draws one run of positioned glyphs, with `origin` on the baseline.
    ///
    /// This is `DrawGlyphRun` and there is nothing above it: drawing a text *layout* would
    /// put DirectWrite's shaper on whatever path issued the call. A run reaching here was
    /// shaped once, elsewhere.
    ///
    /// **Measuring is `NATURAL`**, so glyph advances keep ideal metrics that do not depend
    /// on the display resolution and horizontal positions are subpixel. Only the baseline
    /// is snapped, and snapping it is the caller's: `DrawGlyphRun` takes no options
    /// parameter, so the free baseline snapping the text APIs perform is not available
    /// here and glyphs land wherever `origin` puts them.
    ///
    /// How the coverage is rasterized comes from the context, which
    /// [`text_params`](Self::text_params) states rather than inheriting the system's.
    pub fn glyphs(&self, origin: Vector2, run: &GlyphRun<'_>, brush: &impl Brush) {
        debug_assert!(
            run.glyphs.len() == run.advances.len() && run.glyphs.len() == run.offsets.len(),
            "a glyph run's indices, advances and offsets are three views of one sequence"
        );
        let count = run
            .glyphs
            .len()
            .min(run.advances.len())
            .min(run.offsets.len());
        if count == 0 {
            return;
        }
        let Ok(face) = run.face.cast::<IDWriteFontFace>() else {
            debug_assert!(false, "a glyph run's face must be an IDWriteFontFace");
            return;
        };
        // `DWRITE_GLYPH_RUN` holds the face in a `ManuallyDrop`, so the reference the cast
        // took is held for the length of the call and dropped after it.
        let dwrite = DWRITE_GLYPH_RUN {
            fontFace: ManuallyDrop::new(Some(face)),
            fontEmSize: run.em,
            glyphCount: count as u32,
            glyphIndices: run.glyphs.as_ptr(),
            glyphAdvances: run.advances.as_ptr(),
            glyphOffsets: run.offsets.as_ptr().cast::<DWRITE_GLYPH_OFFSET>(),
            isSideways: false.into(),
            bidiLevel: run.bidi,
        };
        unsafe {
            self.ctx.DrawGlyphRun(
                origin,
                &raw const dwrite,
                None,
                brush.brush().raw(),
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        drop(ManuallyDrop::into_inner(dwrite.fontFace));
    }

    /// Draws a realization: tessellated once, rasterized per frame.
    pub fn realization(&self, r: &Realization, brush: &impl Brush) {
        debug_assert!(
            (r.scale() - self.scale()).abs() < f32::EPSILON,
            "a realization drawn at a scale it was not built for shows its flattening as facets"
        );
        unsafe {
            self.ctx
                .DrawGeometryRealization(r.raw(), brush.brush().raw());
        }
    }

    // ── state, as guards ──────────────────────────────────────────────────────────

    /// Clips to an axis-aligned rectangle until the guard drops.
    ///
    /// A clip and not a layer, for a rectangle: Direct2D's debug layer emits a performance
    /// message when a layer is pushed with a null opacity mask, 1.0 opacity and an
    /// axis-aligned rectangular mask, because a clip reaches the same result for less.
    pub fn clip(&self, r: Rect) -> Clipped<'_> {
        unsafe {
            self.ctx
                .PushAxisAlignedClip(r.d2d(), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
        Clipped(self)
    }

    /// Applies a group operation — a geometric mask, an opacity mask, a group opacity —
    /// to everything drawn until the guard drops.
    ///
    /// [`Layer`] names which of the three, and the cheaper alternatives where they apply.
    pub fn layer(&self, l: Layer<'_>) -> Layered<'_> {
        let mut params = l.params(self.opacity);
        unsafe {
            if l.replacing {
                self.ctx.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
            }
            self.ctx.PushLayer(&params, None);
            // The parameter struct never releases the counted references it carries, so the
            // clones taken to fill it are dropped now that `PushLayer` holds its own.
            ManuallyDrop::drop(&mut params.geometricMask);
            ManuallyDrop::drop(&mut params.opacityBrush);
        }
        self.layers.set(self.layers.get() + 1);
        Layered {
            draw: self,
            restore_blend: l.replacing,
        }
    }

    /// Replaces the transform until the guard drops.
    pub fn transform(&self, m: Matrix3x2) -> Transformed<'_> {
        let mut previous = Matrix3x2::default();
        unsafe {
            self.ctx.GetTransform(&mut previous);
            self.ctx.SetTransform(&m);
        }
        Transformed(self, previous)
    }

    /// Turns antialiasing off until the guard drops. A guard and not a setter, so a mode
    /// cannot leak into the next target bound in the same bracket.
    pub fn aliased(&self) -> Aliased<'_> {
        let previous = unsafe { self.ctx.GetAntialiasMode() };
        unsafe { self.ctx.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED) };
        Aliased(self, previous)
    }

    /// Blends additively until the guard drops — a glow over what is already there,
    /// without an intermediate.
    pub fn additive(&self) -> Additive<'_> {
        unsafe { self.ctx.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_ADD) };
        Additive(self)
    }

    /// States how glyph coverage is rasterized, until the guard drops.
    ///
    /// `params` must be a DirectWrite `IDWriteRenderingParams`, built by whoever owns the
    /// font stack; this crate names the interface only so one can be handed over. Inherited
    /// parameters are the *system's* — a user's ClearType tuning, a display's gamma — and
    /// coverage carrying those comes out systematically thin or fat.
    ///
    /// A guard rather than a setter, for the same reason as [`aliased`](Self::aliased): one
    /// bracket spans every target in a pass, and a mode set on the context outlives the
    /// target it was meant for.
    pub fn text_params(&self, params: &impl Interface) -> TextParams<'_> {
        // A context that was never given parameters carries none, and the getter reports
        // that as a failed call rather than as `Ok(None)`. Restoring `None` is correct both
        // when there were none and when the getter could not answer.
        let previous = unsafe { self.ctx.GetTextRenderingParams().ok() };
        if let Ok(params) = params.cast::<IDWriteRenderingParams>() {
            unsafe { self.ctx.SetTextRenderingParams(&params) };
        } else {
            debug_assert!(false, "text params must be an IDWriteRenderingParams");
        }
        TextParams(self, previous)
    }
}

impl Drop for Draw<'_> {
    fn drop(&mut self) {
        debug_assert_eq!(
            self.layers.get(),
            0,
            "an unpopped layer poisons the target through to EndDraw"
        );
        if self.unbind {
            // Release the target's hold before the buffer is presented or freed. Drawing
            // calls after this would error, and nothing draws after this.
            unsafe { self.ctx.SetTarget(None) };
        }
    }
}

/// A group operation over everything drawn while it is pushed: Direct2D's only such
/// operator, where everything else it offers is per-primitive.
///
/// The alternatives below are cheaper wherever they apply:
///
/// | Want | Use |
/// |---|---|
/// | a rectangular clip | [`Draw::clip`] |
/// | opacity on one primitive | fold it into the brush colour |
/// | a fade over primitives that do not overlap | scale each primitive's alpha — exact, since source-over composition of disjoint coverage is |
/// | a crossfade between two *static* states | cache each state and blit both at complementary alpha |
/// | a rounded-rect clip inside an *opaque* target | overdraw the four corners in the surface colour |
/// | an intermediate that outlives the frame | [`Gpu::offscreen`] |
///
/// A layer covers what is left: a fade over **live** content whose members **overlap**, an
/// **arbitrary** geometric mask, and an opacity mask over a **group**. In the mask cases the
/// content keeps its per-primitive antialiasing and the mask edge is antialiased separately,
/// where masking through `FillOpacityMask` requires aliased mode for everything under it.
#[derive(Copy, Clone)]
pub struct Layer<'a> {
    mask: Option<&'a Path>,
    mask_brush: Option<&'a BrushRef>,
    opacity: f32,
    bounds: Option<Rect>,
    mask_aa: D2D1_ANTIALIAS_MODE,
    replacing: bool,
}

impl<'a> Layer<'a> {
    const EMPTY: Self = Self {
        mask: None,
        mask_brush: None,
        opacity: 1.0,
        bounds: None,
        mask_aa: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        replacing: false,
    };

    /// Group opacity. Per-primitive alpha differs here: members that overlap each other
    /// show through one another as the group fades.
    #[must_use]
    pub const fn opacity(a: f32) -> Self {
        Self {
            opacity: a,
            ..Self::EMPTY
        }
    }

    /// A geometric mask. The path is in the target's own coordinate space.
    #[must_use]
    pub const fn mask(p: &'a Path) -> Self {
        Self {
            mask: Some(p),
            ..Self::EMPTY
        }
    }

    /// An opacity mask: the brush's alpha multiplies the group's, per pixel. A linear ramp
    /// here is an edge fade over a whole group.
    #[must_use]
    pub fn mask_brush(b: &'a impl Brush) -> Self {
        Self {
            mask_brush: Some(b.brush()),
            ..Self::EMPTY
        }
    }

    /// Sets the group opacity.
    #[must_use]
    pub const fn with_opacity(self, a: f32) -> Self {
        Self { opacity: a, ..self }
    }

    /// Aligns the mask edge to whole pixels instead of antialiasing it. Worth it where the
    /// content's own edges meet the mask edge: two antialiased edges at one boundary
    /// multiply their coverage and leave a faint seam.
    #[must_use]
    pub const fn aliased_mask(self) -> Self {
        Self {
            mask_aa: D2D1_ANTIALIAS_MODE_ALIASED,
            ..self
        }
    }

    /// Restricts rasterization. Defaults to the mask's own transformed bounds when a mask
    /// is set, and to the target otherwise.
    #[must_use]
    pub const fn bounds(self, r: Rect) -> Self {
        Self {
            bounds: Some(r),
            ..self
        }
    }

    /// Composites the group back by replacing rather than blending, so the group's **own
    /// alpha** reaches the target instead of alpha-compositing over what was already
    /// there.
    ///
    /// Required when masking content that itself carries transparency into a
    /// [`Translucent`](Opacity::Translucent) target, and wrong when the layer is meant to
    /// composite over existing pixels, so the caller states it.
    #[must_use]
    pub const fn replacing(self) -> Self {
        Self {
            replacing: true,
            ..self
        }
    }

    fn params(&self, target: Opacity) -> D2D1_LAYER_PARAMETERS1 {
        // Skipping the clear to transparent black is usually faster and is not the default.
        // Leaving the alpha channel unwritten is legal only where the target surface ignores
        // alpha, so the flag is read off the target rather than offered as a choice.
        let mut options = D2D1_LAYER_OPTIONS1_INITIALIZE_FROM_BACKGROUND;
        if target == Opacity::Opaque {
            options |= D2D1_LAYER_OPTIONS1_IGNORE_ALPHA;
        }
        D2D1_LAYER_PARAMETERS1 {
            // An infinite rectangle is Direct2D's own default, and with a mask set it
            // resolves to the mask's transformed bounds — so the intermediate is sized to
            // the group rather than to the target.
            contentBounds: self.bounds.map_or(INFINITE, |r| D2D_RECT_F {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
            }),
            geometricMask: ManuallyDrop::new(self.mask.map(Path::owned)),
            maskAntialiasMode: self.mask_aa,
            maskTransform: Matrix3x2::identity(),
            opacity: self.opacity,
            opacityBrush: ManuallyDrop::new(self.mask_brush.map(BrushRef::owned)),
            layerOptions: options,
        }
    }
}

/// Direct2D's infinite rectangle, which is what its own helper produces for a layer whose
/// content bounds are unrestricted.
const INFINITE: D2D_RECT_F = D2D_RECT_F {
    left: -f32::MAX,
    top: -f32::MAX,
    right: f32::MAX,
    bottom: f32::MAX,
};

/// One run of positioned glyphs from one face, as plain data plus the face itself.
///
/// Shaping produces this and drawing consumes it. Everything but the face is plain data —
/// glyph indices, advances and offsets — so a run crosses between the two carrying nothing
/// thread-affine and no pixels. The face arrives as an `IUnknown` because this crate names
/// DirectWrite in no public signature and only hands the pointer back.
pub struct GlyphRun<'a> {
    /// The `IDWriteFontFace` every glyph index in this run is an index into.
    pub face: &'a windows_core::IUnknown,
    /// Em size in DIPs.
    pub em: f32,
    /// Glyph indices into `face`.
    pub glyphs: &'a [u16],
    /// Advance per glyph, in DIPs. Same length as `glyphs`.
    pub advances: &'a [f32],
    /// Displacement per glyph, in DIPs: `[along the baseline, up from it]`. Same length
    /// as `glyphs`.
    pub offsets: &'a [[f32; 2]],
    /// Bidi embedding level; odd means the run advances leftward from `origin`. Shaping
    /// resolves it, and a run drawn at level 0 that was shaped at an odd one renders its
    /// glyphs in the wrong direction rather than failing.
    pub bidi: u32,
}

/// A pair of `f32` per glyph *is* `DWRITE_GLYPH_OFFSET`, so [`GlyphRun::offsets`] is passed
/// straight through: no conversion, no scratch buffer, and no named type either crate would
/// have to own for the other.
const _: () = {
    assert!(size_of::<[f32; 2]>() == size_of::<DWRITE_GLYPH_OFFSET>());
    assert!(align_of::<[f32; 2]>() == align_of::<DWRITE_GLYPH_OFFSET>());
    assert!(core::mem::offset_of!(DWRITE_GLYPH_OFFSET, advanceOffset) == 0);
    assert!(core::mem::offset_of!(DWRITE_GLYPH_OFFSET, ascenderOffset) == size_of::<f32>());
};

/// Undoes a [`Draw::clip`].
pub struct Clipped<'d>(&'d Draw<'d>);

impl Drop for Clipped<'_> {
    fn drop(&mut self) {
        unsafe { self.0.ctx.PopAxisAlignedClip() };
    }
}

/// Composites a [`Draw::layer`] back.
pub struct Layered<'d> {
    draw: &'d Draw<'d>,
    restore_blend: bool,
}

impl Drop for Layered<'_> {
    fn drop(&mut self) {
        unsafe {
            self.draw.ctx.PopLayer();
            if self.restore_blend {
                self.draw
                    .ctx
                    .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
            }
        }
        self.draw.layers.set(self.draw.layers.get() - 1);
    }
}

/// Restores the transform a [`Draw::transform`] replaced.
pub struct Transformed<'d>(&'d Draw<'d>, Matrix3x2);

impl Drop for Transformed<'_> {
    fn drop(&mut self) {
        unsafe { self.0.ctx.SetTransform(&self.1) };
    }
}

/// Restores the antialias mode a [`Draw::aliased`] replaced.
pub struct Aliased<'d>(&'d Draw<'d>, D2D1_ANTIALIAS_MODE);

impl Drop for Aliased<'_> {
    fn drop(&mut self) {
        unsafe { self.0.ctx.SetAntialiasMode(self.1) };
    }
}

/// Restores the rendering parameters a [`Draw::text_params`] replaced.
///
/// The previous value is an `Option` because a context that has never been told carries
/// none, and restoring "none" is what puts it back rather than pinning the default.
pub struct TextParams<'d>(&'d Draw<'d>, Option<IDWriteRenderingParams>);

impl Drop for TextParams<'_> {
    fn drop(&mut self) {
        unsafe { self.0.ctx.SetTextRenderingParams(self.1.as_ref()) };
    }
}

/// Restores source-over blending after a [`Draw::additive`].
pub struct Additive<'d>(&'d Draw<'d>);

impl Drop for Additive<'_> {
    fn drop(&mut self) {
        unsafe {
            self.0
                .ctx
                .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
        }
    }
}

/// Rounds a DIP coordinate to the nearest whole physical pixel.
///
/// Free-standing so the arithmetic is testable without a device.
fn snap_to(dip: f32, scale: f32) -> f32 {
    (dip * scale).round() / scale
}

/// Positions a box so a stroke of `width` DIPs along its edges covers whole pixels.
///
/// The parity decides the placement: a stroke is centred on the path, so an **odd** number
/// of physical pixels covers whole pixels only when its centre sits on a pixel *centre*, and
/// an **even** number only when its centre sits on a *boundary*.
fn snap_stroke_to(r: Rect, width: f32, scale: f32) -> Rect {
    let width_px = (width * scale).round().max(1.0);
    let bias = if (width_px as i32) % 2 == 1 { 0.5 } else { 0.0 };
    let snap = |dip: f32| ((dip * scale).round() + bias) / scale;
    Rect::new(snap(r.left), snap(r.top), snap(r.right), snap(r.bottom))
}

#[cfg(test)]
mod tests {
    use super::{Rect, snap_stroke_to, snap_to};

    #[test]
    fn snapping_lands_on_whole_pixels() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for dip in [0.0, 0.3, 7.4, 7.6, 100.49, 100.51] {
                let px = snap_to(dip, scale) * scale;
                assert!(
                    (px - px.round()).abs() < 1e-4,
                    "{dip} at {scale}x snapped to {px} px"
                );
            }
        }
    }

    #[test]
    fn a_one_dip_rule_straddles_a_pixel_centre() {
        // 1 DIP is 1 physical pixel at 1x and 2 at 2x, so the parity — and therefore where
        // the stroke's centre has to sit — differs between them. At 96 DPI the two rules
        // coincide for even widths, which is why an inverted parity hides on a 1x machine.
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);

        let at_1x = snap_stroke_to(r, 1.0, 1.0);
        assert_eq!(at_1x.left, 10.5, "1 px wide: centre on a pixel centre");

        let at_2x = snap_stroke_to(r, 1.0, 2.0);
        assert_eq!(at_2x.left, 10.0, "2 px wide: centre on a pixel boundary");

        // A 2-DIP rule is 4 px at 2x — still even, still a boundary.
        assert_eq!(snap_stroke_to(r, 2.0, 2.0).left, 10.0);
        // ...and 2 px at 1x, likewise.
        assert_eq!(snap_stroke_to(r, 2.0, 1.0).left, 10.0);
        // A 3-DIP rule is 3 px at 1x: odd again.
        assert_eq!(snap_stroke_to(r, 3.0, 1.0).left, 10.5);
    }

    #[test]
    fn the_width_is_never_snapped() {
        // 1 DIP at 1.5x is 1.5 px and stays 1.5 px: rounding it to 1 or 2 would change the
        // design's line weight by a third, and design metrics stay DIP-constant.
        let r = snap_stroke_to(Rect::new(4.0, 4.0, 8.0, 8.0), 1.0, 1.5);
        assert!(
            (r.width() - 4.0).abs() < 1e-6,
            "the box, not the width, moves"
        );
    }
}
