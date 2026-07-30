//! The bracket, the target binding, and everything drawn inside one.
//!
//! Two types carry the whole of it, and the split is the point. [`Pass`] is the
//! `BeginDraw`/`EndDraw` bracket — exactly one per device at a time, spanning however
//! many targets a present pass touches. [`Draw`] is *one target bound inside that
//! bracket*, and because [`Pass::draw`] takes `&mut self` the previous binding must be
//! dropped before the next one exists. So "one bracket per pass" and "one target bound at
//! a time" are both properties of the borrow checker rather than of a convention.

use super::*;
use core::cell::Cell;
use core::mem::ManuallyDrop;

/// The Direct2D drawing bracket. Opens on construction, closes on [`end`](Pass::end) or
/// on drop.
///
/// One bracket spans the whole pass because Direct2D charges a fixed cost per pair that
/// has nothing to do with what was drawn between them — a DXGI `ReclaimResources`, an
/// `OfferResources`, and a delayed Direct3D device-context-state swap. It scales with the
/// number of brackets, which on a panel of regions means it scales with region count.
pub struct Pass<'g> {
    gpu: &'g Gpu,
}

/// A pass that latched an error, and the tag that names where.
///
/// Direct2D defers: it records commands and does the work at `EndDraw`, and a failed call
/// latches an error on the context that silently discards every later draw in the same
/// bracket. Since the bracket spans every target in the pass, a draw that vanished is as
/// likely to have been killed by an earlier one — so the tag is not a nicety, it is the
/// only way to tell those two apart.
#[derive(Copy, Clone, Debug)]
pub struct PassError {
    pub hr: windows_core::HRESULT,
    /// Whatever [`Pass::tag`] last set before the failing call.
    pub tag: u64,
    pub loss: Loss,
}

impl<'g> Pass<'g> {
    pub(crate) fn new(gpu: &'g Gpu) -> Self {
        Self { gpu }
    }

    /// Binds `target` and returns the drawing surface for it, in DIPs at the target's own
    /// DPI, with its origin at `(0, 0)`.
    ///
    /// Retargeting inside an open bracket is what a cached intermediate costs instead of a
    /// second context: `SetTarget` is documented as legal at any time, including while the
    /// context is drawing.
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
    /// Meant to be called once per retarget — a tag per primitive would cost a call per
    /// primitive to answer a question asked once per pass.
    pub fn tag(&self, tag: u64) {
        unsafe { self.gpu.ctx().SetTags(tag, 0) };
    }

    /// Closes the bracket and reports what it latched.
    ///
    /// Dropping a `Pass` closes it too, discarding the result — that path exists so a
    /// panic cannot leave `BeginDraw` outstanding, not as a way to skip the error.
    pub fn end(self) -> core::result::Result<(), PassError> {
        self.close()
    }

    /// Idempotent: the device's own open flag is what makes it so, which is why there is
    /// no second bit of state to keep in step.
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
/// Its contents are **undefined on entry**: a renderer either clears or covers its whole
/// box. Nothing here is `Send` and nothing here is a pacer — a `Draw` draws, and the
/// decision about when belongs to whoever opened the pass.
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

    /// The DPI this target is bound at.
    #[must_use]
    pub fn dpi(&self) -> f32 {
        self.dpi
    }

    /// DIP-to-pixel factor.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.dpi / 96.0
    }

    // ── pixel snapping ────────────────────────────────────────────────────────────
    //
    // Direct2D snaps nothing except text baselines, and only in the text APIs this crate
    // does not use. For everything else the documentation is explicit that "physical device
    // pixels might end up at fractional DIP coordinates, which is one of the reasons why
    // Direct2D uses a floating-point coordinate space" — fractional coordinates are the
    // expected condition, not a fault, and aligning to the grid is the caller's arithmetic.
    //
    // So the arithmetic lives here, once. `D2D1_ANTIALIAS_MODE_ALIASED` is emphatically not
    // a substitute: it makes an edge hard, not aligned, so an unsnapped aliased hairline is
    // a crisp line in the wrong pixel.

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
    /// A stroke is centred on the path, so where its centre has to sit depends on the parity
    /// of its physical width: an odd number of pixels covers whole pixels when centred on a
    /// pixel *centre*, an even number when centred on a *boundary*. Getting that backwards
    /// is what turns a one-pixel rule into two half-covered rows — the single most common
    /// high-DPI artefact, and invisible at 96 DPI where the two happen to coincide for the
    /// widths a design actually uses.
    ///
    /// The **width is deliberately not snapped**. A 1-DIP rule is 1.5 physical pixels at
    /// 1.5×, and no placement makes that crisp on both edges; rounding it to 1 or 2 would
    /// change the design's line weight by a third, and design metrics stay DIP-constant. A
    /// feathered 1.5-pixel rule in the right place is the correct rendering.
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

    /// A line is stroked and never filled, so it is its own call rather than a [`Shape`]
    /// arm that would be meaningless in [`fill`](Self::fill).
    pub fn line(&self, from: Vector2, to: Vector2, brush: &impl Brush, k: Stroke<'_>) {
        let (w, style) = k.parts();
        unsafe {
            self.ctx.DrawLine(from, to, brush.brush().raw(), w, style);
        }
    }

    /// Blits `src`. Its source rectangle is in the **source's** DIPs, which are the same
    /// DIPs as this target's because every target carries the display's DPI.
    ///
    /// **The destination is snapped for you.** Unlike a fill or a stroke, a fractional
    /// destination here has no legitimate use: the source was rasterized at device
    /// resolution, so landing it off the grid resamples finished pixels and softens a cached
    /// glyph run for nothing. The requirement has no exceptions and this method knows the
    /// scale, so leaving it to the caller would only mean discovering later which call site
    /// forgot.
    ///
    /// This is `DrawBitmap` and not `DrawImage`, and the difference is not stylistic:
    /// `DrawImage` accepts any `ID2D1Image` and so runs Direct2D's image command graph to
    /// discover what it was handed, measured at 2473 samples against 108 for six blits a
    /// frame. There is no general image draw here.
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
    /// Sets aliased mode for the duration and restores what it found. That is not a
    /// preference: with per-primitive antialiasing on, `DrawSpriteBatch` does not draw,
    /// and because it returns `void` the failure surfaces only as an error latched on the
    /// context — which then discards the rest of the pass. Direct2D's reference does not
    /// state the rule; Microsoft's own Direct2D wrapper performs exactly this swap around
    /// every batch it submits.
    pub fn sprites(&self, batch: &SpriteBatch, src: &Target, interp: Interp) {
        let count = batch.len() as u32;
        if count == 0 {
            return;
        }
        // Splitting an oversized batch would need a `Flush` between the halves, and a
        // `Flush` with a layer outstanding puts the target into an error state. The field
        // this stack draws is well inside the limit, so the ceiling is asserted rather
        // than worked around.
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
    /// This and not a layer, for a rectangle: Direct2D's debug layer emits a dedicated
    /// performance message when a layer is pushed with a null opacity mask, 1.0 opacity
    /// and an axis-aligned rectangular mask, because a clip achieves the same result more
    /// cheaply.
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
    /// See [`Layer`] for which of the three, and for the cases where something cheaper is
    /// both correct and better.
    pub fn layer(&self, l: Layer<'_>) -> Layered<'_> {
        let mut params = l.params(self.opacity);
        unsafe {
            if l.replacing {
                self.ctx.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
            }
            self.ctx.PushLayer(&params, None);
            // The parameters carry counted references the struct will not release, so the
            // clones taken to fill them are dropped now that `PushLayer` has taken its
            // own.
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
/// It is the right answer for a narrower set of cases than it looks, because the
/// alternatives are cheaper wherever they apply:
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
/// What is left, and what this is for: a fade over **live** content whose members
/// **overlap**, an **arbitrary** geometric mask, and an opacity mask over a **group**. The
/// mask cases are also where it wins on quality rather than just generality — content
/// keeps its per-primitive antialiasing and the mask edge is antialiased separately, where
/// masking through `FillOpacityMask` requires aliased mode for everything under it.
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

    /// Group opacity: the case per-primitive alpha gets wrong, because members that
    /// overlap each other show through one another as the group fades.
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

    #[must_use]
    pub const fn with_opacity(self, a: f32) -> Self {
        Self { opacity: a, ..self }
    }

    /// Antialias the mask edge to whole pixels. Worth it where the content's own edges
    /// meet the mask edge: two antialiased edges at one boundary multiply their coverage
    /// and leave a faint seam.
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
    /// composite over existing pixels — which is why it is stated rather than inferred.
    #[must_use]
    pub const fn replacing(self) -> Self {
        Self {
            replacing: true,
            ..self
        }
    }

    fn params(&self, target: Opacity) -> D2D1_LAYER_PARAMETERS1 {
        // Skipping the clear to transparent black is documented as usually faster, and is
        // not the default. Avoiding a write to the alpha channel is legal only when the
        // target surface ignores alpha in the first place — so it is read off the target
        // rather than offered as a choice.
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
/// Free-standing so the arithmetic is testable without a device — see the tests below.
fn snap_to(dip: f32, scale: f32) -> f32 {
    (dip * scale).round() / scale
}

/// Positions a box so a stroke of `width` DIPs along its edges covers whole pixels.
///
/// The parity is the whole content of this function, and it is the part that is easy to
/// invert: a stroke is centred on the path, so an **odd** number of physical pixels covers
/// whole pixels only when its centre sits on a pixel *centre*, and an **even** number only
/// when its centre sits on a *boundary*.
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
