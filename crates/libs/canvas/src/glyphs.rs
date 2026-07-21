//! Glyph-level text: walking a shaped [`TextLayout`] as glyph runs, and drawing
//! an individual run.
//!
//! [`TextLayout`] draws as one opaque call ([`DrawingSession::draw_text_layout`]),
//! which re-shapes and re-rasterizes every frame. A glyph-atlas renderer instead
//! wants the *shaped* output — which font face, which glyph ids, at which
//! advances — so it can raster each glyph once and blit the cached raster
//! thereafter. [`TextLayout::glyph_runs`] yields exactly that, and
//! [`DrawingSession::draw_glyph_run`] draws one back.
//!
//! DirectWrite hands glyph runs to a *callback* ([`IDWriteTextRenderer`]) whose
//! arrays are borrowed for the duration of the call, so nothing it is handed can
//! outlive the callback. [`GlyphRun`] is therefore an owned, plain-data copy.

use super::*;
use std::cell::RefCell;

/// Which of the two baseline-relative rules a [`TextDecoration`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecorationKind {
    /// Below the baseline — what `TextLayout::set_underline` marks, and what an
    /// IME draws under its active composition span.
    Underline,
    /// Through the x-height.
    Strikethrough,
}

/// One underline or strikethrough rule, in DIPs.
///
/// DirectWrite resolves these against the font's own
/// [`underline_position`](FontMetrics::underline_position) /
/// [`strikethrough_position`](FontMetrics::strikethrough_position) and the
/// span's tallest run, so the values here are already the final geometry — a
/// caller places the rect and does no font arithmetic of its own.
///
/// A decoration is NOT a glyph: it has no outline, no coverage, and no atlas
/// entry. Whoever renders it draws a rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextDecoration {
    pub kind: DecorationKind,
    /// Baseline origin of the decorated span, in the same space the sibling
    /// [`GlyphRun::baseline_origin`]s are reported in.
    pub baseline_origin: Vector2,
    /// Length along the reading direction.
    pub width: f32,
    /// Rule thickness.
    pub thickness: f32,
    /// Baseline-relative position of the rule's top edge, positive DOWNWARD —
    /// DirectWrite's sign convention, and the opposite of
    /// [`GlyphOffset::ascender_offset`]'s.
    pub offset: f32,
}

impl TextDecoration {
    /// The rule's rect as `(left, top, width, height)`, in the same space as
    /// [`baseline_origin`](Self::baseline_origin).
    ///
    /// Right-to-left spans report their origin at the span's right edge, so the
    /// left edge is derived rather than assumed.
    pub fn rect(&self, right_to_left: bool) -> (f32, f32, f32, f32) {
        let left = if right_to_left {
            self.baseline_origin.x - self.width
        } else {
            self.baseline_origin.x
        };
        (
            left,
            self.baseline_origin.y + self.offset,
            self.width,
            self.thickness,
        )
    }
}

/// Everything a [`TextLayout::Draw`](TextLayout) walk produces: the shaped glyph
/// runs, and the decorations that are not glyphs.
///
/// The two come from ONE walk because DirectWrite emits them from one walk. A
/// caller that wanted both from [`TextLayout::glyph_runs`] plus a second query
/// would shape the layout twice to learn things it was already told once.
#[derive(Clone, Default)]
pub struct ShapedText {
    pub runs: Vec<GlyphRun>,
    /// Empty for the overwhelming majority of text — only a layout with
    /// [`set_underline`](TextLayout::set_underline) or a strikethrough applied
    /// to some range produces any.
    pub decorations: Vec<TextDecoration>,
}

/// Per-glyph positional adjustment, in DIPs, relative to where the advance
/// alone would place the glyph.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlyphOffset {
    /// Offset along the run's advance direction (positive = further along).
    pub advance_offset: f32,
    /// Offset perpendicular to the baseline (positive = toward the ascender).
    pub ascender_offset: f32,
}

/// Face-wide metrics, in font design units. Divide by
/// [`design_units_per_em`](Self::design_units_per_em) and multiply by the run's
/// em size to get DIPs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontMetrics {
    /// Design units per em — the denominator for every other field here.
    pub design_units_per_em: u16,
    /// Distance from the baseline to the top of the em box.
    pub ascent: u16,
    /// Distance from the baseline to the bottom of the em box.
    pub descent: u16,
    /// Recommended extra leading between lines.
    pub line_gap: i16,
    /// Height of a capital letter.
    pub cap_height: u16,
    /// Height of a lowercase `x`.
    pub x_height: u16,
    /// Baseline-relative position of an underline (negative = below).
    pub underline_position: i16,
    /// Thickness of an underline.
    pub underline_thickness: u16,
    /// Baseline-relative position of a strikethrough.
    pub strikethrough_position: i16,
    /// Thickness of a strikethrough.
    pub strikethrough_thickness: u16,
}

/// Per-glyph design-space metrics — the ink box of one glyph, in font design
/// units. This is what sizes an atlas cell: the raster extent of a glyph is the
/// advance box minus the side bearings, scaled by `em_size / design_units_per_em`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphMetrics {
    /// Horizontal space between the origin and the left edge of the ink.
    pub left_side_bearing: i32,
    /// Horizontal advance from this glyph's origin to the next.
    pub advance_width: u32,
    /// Horizontal space between the right edge of the ink and the advance edge.
    pub right_side_bearing: i32,
    /// Vertical space between the top of the em box and the top of the ink.
    pub top_side_bearing: i32,
    /// Vertical advance.
    pub advance_height: u32,
    /// Vertical space between the bottom of the ink and the bottom of the em box.
    pub bottom_side_bearing: i32,
    /// Baseline-relative vertical origin used for vertical writing.
    pub vertical_origin_y: i32,
}

/// A font face — the resolved, shapeable font a glyph run's ids index into.
///
/// Obtained from a harvested [`GlyphRun`] rather than constructed: DirectWrite
/// picks the face during layout (family + weight + style + fallback), so the
/// face a run actually used is only knowable after shaping.
#[derive(Clone)]
pub struct FontFace {
    raw: IDWriteFontFace,
}

impl FontFace {
    /// Face-wide metrics in design units. Cache these per face — they do not
    /// change with em size.
    pub fn metrics(&self) -> FontMetrics {
        let mut m = DWRITE_FONT_METRICS::default();
        unsafe { self.raw.GetMetrics(&mut m) };
        FontMetrics {
            design_units_per_em: m.designUnitsPerEm,
            ascent: m.ascent,
            descent: m.descent,
            line_gap: m.lineGap,
            cap_height: m.capHeight,
            x_height: m.xHeight,
            underline_position: m.underlinePosition,
            underline_thickness: m.underlineThickness,
            strikethrough_position: m.strikethroughPosition,
            strikethrough_thickness: m.strikethroughThickness,
        }
    }

    /// Design-space metrics for each of `glyph_indices`, in the same order.
    /// `sideways` requests the rotated (vertical-writing) metrics.
    pub fn design_glyph_metrics(
        &self,
        glyph_indices: &[u16],
        sideways: bool,
    ) -> Result<Vec<GlyphMetrics>> {
        if glyph_indices.is_empty() {
            return Ok(Vec::new());
        }
        let mut raw = vec![DWRITE_GLYPH_METRICS::default(); glyph_indices.len()];
        unsafe {
            self.raw
                .GetDesignGlyphMetrics(
                    glyph_indices.as_ptr(),
                    glyph_indices.len() as u32,
                    raw.as_mut_ptr(),
                    sideways,
                )
                .ok()?;
        }
        Ok(raw
            .iter()
            .map(|m| GlyphMetrics {
                left_side_bearing: m.leftSideBearing,
                advance_width: m.advanceWidth,
                right_side_bearing: m.rightSideBearing,
                top_side_bearing: m.topSideBearing,
                advance_height: m.advanceHeight,
                bottom_side_bearing: m.bottomSideBearing,
                vertical_origin_y: m.verticalOriginY,
            })
            .collect())
    }

    /// Map Unicode scalar values to glyph ids in this face, one-to-one and in
    /// order. Unmapped code points come back as glyph `0` (`.notdef`).
    ///
    /// This is the *cmap* lookup only — no shaping, so it does not apply
    /// ligatures, marks, or contextual substitution. Use it for glyph
    /// pre-warming (rasterizing an atlas ahead of first use); take real text
    /// through [`TextLayout::glyph_runs`].
    pub fn glyph_indices(&self, code_points: &[u32]) -> Result<Vec<u16>> {
        if code_points.is_empty() {
            return Ok(Vec::new());
        }
        let mut indices = vec![0u16; code_points.len()];
        unsafe {
            self.raw
                .GetGlyphIndices(
                    code_points.as_ptr(),
                    code_points.len() as u32,
                    indices.as_mut_ptr(),
                )
                .ok()?;
        }
        Ok(indices)
    }

    /// Returns the underlying `IDWriteFontFace`.
    pub fn raw(&self) -> &IDWriteFontFace {
        &self.raw
    }
}

/// One shaped glyph run harvested from a [`TextLayout`] — a maximal span of
/// glyphs sharing a font face, em size, and direction.
///
/// Owned plain data: the arrays are copied out of DirectWrite's callback, so a
/// run may be cached and replayed for as long as it is useful. The three
/// per-glyph arrays are parallel and all `glyph_indices.len()` long — except
/// `glyph_offsets`, which is empty when the run needs no positional
/// adjustments (the common case).
#[derive(Clone)]
pub struct GlyphRun {
    /// The face these glyph ids index into.
    pub font_face: FontFace,
    /// Em size in DIPs — the scale the design-unit metrics map through.
    pub font_em_size: f32,
    /// Glyph ids, in visual order.
    pub glyph_indices: Vec<u16>,
    /// Advance (DIPs) from each glyph's origin to the next.
    pub glyph_advances: Vec<f32>,
    /// Per-glyph positional nudges. Empty when the run has none.
    pub glyph_offsets: Vec<GlyphOffset>,
    /// Origin of the run's baseline, in the coordinate space the layout was
    /// walked in (see [`TextLayout::glyph_runs_at`]).
    pub baseline_origin: Vector2,
    /// True if the glyphs are rotated 90° for vertical writing.
    pub is_sideways: bool,
    /// Bidi embedding level; odd means right-to-left.
    pub bidi_level: u32,
}

impl GlyphRun {
    /// Number of glyphs in the run.
    pub fn len(&self) -> usize {
        self.glyph_indices.len()
    }

    /// True if the run carries no glyphs.
    pub fn is_empty(&self) -> bool {
        self.glyph_indices.is_empty()
    }

    /// Total advance width of the run, in DIPs.
    pub fn width(&self) -> f32 {
        self.glyph_advances.iter().sum()
    }

    /// Build the DirectWrite ABI view of this run and hand it to `f`.
    ///
    /// Scoped rather than returned: `DWRITE_GLYPH_RUN` borrows this run's three
    /// arrays as raw pointers and owns a font-face reference that has to be
    /// released again, so the ABI struct cannot outlive the borrow.
    pub(crate) fn with_abi<R>(&self, f: impl FnOnce(&DWRITE_GLYPH_RUN) -> R) -> R {
        let mut run = DWRITE_GLYPH_RUN {
            fontFace: core::mem::ManuallyDrop::new(Some(self.font_face.raw.clone())),
            fontEmSize: self.font_em_size,
            glyphCount: self.glyph_indices.len() as u32,
            glyphIndices: self.glyph_indices.as_ptr(),
            glyphAdvances: self.glyph_advances.as_ptr(),
            glyphOffsets: if self.glyph_offsets.is_empty() {
                std::ptr::null()
            } else {
                self.glyph_offsets.as_ptr() as *const DWRITE_GLYPH_OFFSET
            },
            isSideways: self.is_sideways.into(),
            bidiLevel: self.bidi_level,
        };
        let result = f(&run);
        // The face reference the struct took above is not dropped by
        // `ManuallyDrop`; release it now that the ABI view is dead.
        unsafe { core::mem::ManuallyDrop::drop(&mut run.fontFace) };
        result
    }
}

// `GlyphOffset` is copied straight into the ABI array pointer above, so its
// layout must match DirectWrite's. Both are two `f32`s in the same order; assert
// it rather than trusting the two declarations to stay in step.
const _: () = {
    assert!(size_of::<GlyphOffset>() == size_of::<DWRITE_GLYPH_OFFSET>());
    assert!(align_of::<GlyphOffset>() == align_of::<DWRITE_GLYPH_OFFSET>());
};

impl TextLayout {
    /// Walk the shaped layout and collect its glyph runs, with the layout box's
    /// top-left at the origin.
    ///
    /// The layout is shaped on demand, so this reflects the text, format, and
    /// constraints as they stand at the call. Cache the result and re-walk when
    /// any of those change.
    pub fn glyph_runs(&self) -> Result<Vec<GlyphRun>> {
        self.glyph_runs_at(0.0, 0.0)
    }

    /// Walk the shaped layout with the layout box's top-left placed at
    /// `(origin_x, origin_y)`. Every returned
    /// [`baseline_origin`](GlyphRun::baseline_origin) is offset accordingly, so
    /// passing the same origin the text will be drawn at yields runs already in
    /// target space.
    pub fn glyph_runs_at(&self, origin_x: f32, origin_y: f32) -> Result<Vec<GlyphRun>> {
        Ok(self.shape_at(origin_x, origin_y)?.runs)
    }

    /// Walk the shaped layout for its glyph runs AND its decorations, with the
    /// layout box's top-left at the origin.
    ///
    /// Prefer this over [`glyph_runs`](Self::glyph_runs) wherever a layout can
    /// carry an underline or a strikethrough — an IME composition span, most
    /// obviously. `glyph_runs` discards them, which is silent: the text renders
    /// correctly and the rule simply never appears.
    pub fn shape(&self) -> Result<ShapedText> {
        self.shape_at(0.0, 0.0)
    }

    /// [`shape`](Self::shape) with the layout box's top-left placed at
    /// `(origin_x, origin_y)`.
    pub fn shape_at(&self, origin_x: f32, origin_y: f32) -> Result<ShapedText> {
        let collector = ComObject::new(RunCollector::default());
        let renderer: IDWriteTextRenderer = collector.to_interface();
        unsafe { self.raw().Draw(None, &renderer, origin_x, origin_y).ok()? };
        let c = collector.get();
        Ok(ShapedText {
            runs: c.runs.take(),
            decorations: c.decorations.take(),
        })
    }
}

impl DrawingSession<'_> {
    /// Draw one glyph run at its own [`baseline_origin`](GlyphRun::baseline_origin).
    ///
    /// The direct counterpart to [`glyph_runs`](TextLayout::glyph_runs): a run
    /// harvested at the origin it will be drawn at replays exactly as
    /// [`draw_text_layout`](Self::draw_text_layout) would have drawn it, but one
    /// run at a time and without re-shaping.
    pub fn draw_glyph_run(&self, run: &GlyphRun, brush: &impl Paint) {
        self.draw_glyph_run_at(run.baseline_origin, run, brush);
    }

    /// Draw one glyph run with its baseline origin relocated to `baseline_origin`,
    /// ignoring the run's own. Lets a cached run be stamped anywhere — the same
    /// shaped run reused across rows, or a scrolled line redrawn at a new offset.
    pub fn draw_glyph_run_at(
        &self,
        baseline_origin: Vector2,
        run: &GlyphRun,
        brush: &impl Paint,
    ) {
        run.with_abi(|abi| unsafe {
            self.raw().DrawGlyphRun(
                baseline_origin,
                abi,
                None,
                brush.as_raw_brush(),
                DWRITE_MEASURING_MODE_NATURAL,
            );
        });
    }
}

// ── Glyph-run collection ─────────────────────────────────────────────────────

/// A [`IDWriteTextRenderer`] that draws nothing and records everything: the
/// callback DirectWrite drives from `IDWriteTextLayout::Draw`.
///
/// Glyph runs and decorations are both recorded. A decoration is not a glyph —
/// it has no outline and no atlas entry, and whoever renders one draws a
/// rectangle — but it is still something DirectWrite resolved and the caller
/// cannot re-derive without redoing the layout's own arithmetic, so dropping it
/// here would be losing information rather than declining to draw.
///
/// Inline objects are asked to draw THEMSELVES back into this renderer, which is
/// what turns an ellipsis into recorded glyphs. A trimming sign is an inline
/// object — DirectWrite builds one inside `set_trimming` — so a callback that
/// succeeded without doing anything dropped it silently: the run was still
/// shortened at the trim point, and the `…` that says so simply never arrived.
/// Text then read as cut off mid-word rather than as ellipsized, and only where
/// it happened to overflow.
///
/// An app-supplied inline object (an image, say) draws whatever it draws through
/// the same path; anything it emits that is not a glyph run or a decoration is
/// still dropped, which is the original claim narrowed to the case it is true of.
#[derive(Default)]
struct RunCollector {
    runs: RefCell<Vec<GlyphRun>>,
    decorations: RefCell<Vec<TextDecoration>>,
}

implement_decl! {
    impl RunCollector as RunCollector_Impl: [IDWriteTextRenderer]
}

impl IDWritePixelSnapping_Impl for RunCollector_Impl {
    /// Snapping is disabled: this renderer is measuring, not rasterizing, and
    /// wants the unrounded baseline DirectWrite shaped to. Whoever rasterizes
    /// the runs later applies its own pixel grid.
    fn IsPixelSnappingDisabled(&self, _context: *const core::ffi::c_void) -> Result<BOOL> {
        Ok(BOOL(1))
    }

    /// Identity — runs come back in the layout's own coordinate space, and any
    /// world transform is applied at draw time by the target context.
    fn GetCurrentTransform(
        &self,
        _context: *const core::ffi::c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> Result<()> {
        if transform.is_null() {
            return Err(Error::from_hresult(HRESULT(-2147467261))); // E_POINTER
        }
        unsafe {
            *transform = DWRITE_MATRIX {
                m11: 1.0,
                m12: 0.0,
                m21: 0.0,
                m22: 1.0,
                dx: 0.0,
                dy: 0.0,
            };
        }
        Ok(())
    }

    /// 1.0 — pairs with the identity transform above to keep the walk in DIPs.
    fn GetPixelsPerDip(&self, _context: *const core::ffi::c_void) -> Result<f32> {
        Ok(1.0)
    }
}

impl IDWriteTextRenderer_Impl for RunCollector_Impl {
    fn DrawGlyphRun(
        &self,
        _context: *const core::ffi::c_void,
        baseline_origin_x: f32,
        baseline_origin_y: f32,
        _measuring_mode: DWRITE_MEASURING_MODE,
        glyph_run: *const DWRITE_GLYPH_RUN,
        _glyph_run_description: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _client_drawing_effect: Ref<IUnknown>,
    ) -> Result<()> {
        // A null run, or one with no face, is nothing to record — not an error:
        // returning a failure here would abort the whole `Draw` walk and lose
        // the runs already collected.
        if glyph_run.is_null() {
            return Ok(());
        }
        let run = unsafe { &*glyph_run };
        let Some(face) = run.fontFace.as_ref() else {
            return Ok(());
        };

        let count = run.glyphCount as usize;
        // Every array below is borrowed for the duration of this callback only,
        // hence the copies. DirectWrite guarantees `glyphIndices` and
        // `glyphAdvances` are `glyphCount` long; `glyphOffsets` is optional and
        // arrives null when the run needs no adjustments.
        let glyph_indices = copy_from_raw(run.glyphIndices, count);
        let glyph_advances = copy_from_raw(run.glyphAdvances, count);
        let glyph_offsets = if run.glyphOffsets.is_null() {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(run.glyphOffsets, count) }
                .iter()
                .map(|o| GlyphOffset {
                    advance_offset: o.advanceOffset,
                    ascender_offset: o.ascenderOffset,
                })
                .collect()
        };

        self.runs.borrow_mut().push(GlyphRun {
            font_face: FontFace { raw: face.clone() },
            font_em_size: run.fontEmSize,
            glyph_indices,
            glyph_advances,
            glyph_offsets,
            baseline_origin: Vector2 {
                x: baseline_origin_x,
                y: baseline_origin_y,
            },
            is_sideways: run.isSideways.as_bool(),
            bidi_level: run.bidiLevel,
        });
        Ok(())
    }

    fn DrawUnderline(
        &self,
        _context: *const core::ffi::c_void,
        baseline_origin_x: f32,
        baseline_origin_y: f32,
        underline: *const DWRITE_UNDERLINE,
        _client_drawing_effect: Ref<IUnknown>,
    ) -> Result<()> {
        // As in `DrawGlyphRun`: a null payload is nothing to record, not an
        // error — failing here would abort the walk and lose what came before.
        if underline.is_null() {
            return Ok(());
        }
        let u = unsafe { &*underline };
        self.decorations.borrow_mut().push(TextDecoration {
            kind: DecorationKind::Underline,
            baseline_origin: Vector2 {
                x: baseline_origin_x,
                y: baseline_origin_y,
            },
            width: u.width,
            thickness: u.thickness,
            offset: u.offset,
        });
        Ok(())
    }

    fn DrawStrikethrough(
        &self,
        _context: *const core::ffi::c_void,
        baseline_origin_x: f32,
        baseline_origin_y: f32,
        strikethrough: *const DWRITE_STRIKETHROUGH,
        _client_drawing_effect: Ref<IUnknown>,
    ) -> Result<()> {
        if strikethrough.is_null() {
            return Ok(());
        }
        let s = unsafe { &*strikethrough };
        self.decorations.borrow_mut().push(TextDecoration {
            kind: DecorationKind::Strikethrough,
            baseline_origin: Vector2 {
                x: baseline_origin_x,
                y: baseline_origin_y,
            },
            width: s.width,
            thickness: s.thickness,
            offset: s.offset,
        });
        Ok(())
    }

    fn DrawInlineObject(
        &self,
        context: *const core::ffi::c_void,
        origin_x: f32,
        origin_y: f32,
        inline_object: Ref<IDWriteInlineObject>,
        is_sideways: BOOL,
        is_right_to_left: BOOL,
        client_drawing_effect: Ref<IUnknown>,
    ) -> Result<()> {
        // Re-enter with this same renderer so the object's own glyphs land in
        // `runs` — for a trimming sign that is the ellipsis, and dropping it is
        // the difference between text that reads as ellipsized and text that
        // reads as truncated.
        let Some(obj) = inline_object.as_ref() else {
            return Ok(());
        };
        let renderer: IDWriteTextRenderer = self.to_interface();
        unsafe {
            obj.Draw(
                Some(context),
                &renderer,
                origin_x,
                origin_y,
                is_sideways.as_bool(),
                is_right_to_left.as_bool(),
                client_drawing_effect.as_ref(),
            )
            .ok()
        }
    }
}

/// Copy `count` elements out of a callback-borrowed array, tolerating a null
/// pointer (which DirectWrite pairs with a zero count).
fn copy_from_raw<T: Copy>(ptr: *const T, count: usize) -> Vec<T> {
    if ptr.is_null() || count == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(ptr, count) }.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walking a laid-out string must yield glyphs that account for the whole
    /// text and measure the same width the layout reports. This exercises the
    /// real `IDWriteTextRenderer` callback end to end (DirectWrite drives it),
    /// which is the part that cannot be checked by compiling alone.
    #[test]
    fn walks_a_layout_into_glyph_runs() {
        let format = TextFormat::new("Segoe UI", 16.0).unwrap();
        let layout = TextLayout::new("Hello glyphs", &format, 1000.0, 100.0).unwrap();

        let runs = layout.glyph_runs().unwrap();
        assert!(!runs.is_empty(), "a non-empty layout must produce runs");

        let glyphs: usize = runs.iter().map(GlyphRun::len).sum();
        assert_eq!(glyphs, "Hello glyphs".len(), "one glyph per ASCII char");

        for run in &runs {
            assert_eq!(run.glyph_advances.len(), run.glyph_indices.len());
            assert_eq!(run.font_em_size, 16.0);
            let metrics = run.font_face.metrics();
            assert!(metrics.design_units_per_em > 0);
            let per_glyph = run
                .font_face
                .design_glyph_metrics(&run.glyph_indices, false)
                .unwrap();
            assert_eq!(per_glyph.len(), run.glyph_indices.len());
        }

        let walked: f32 = runs.iter().map(GlyphRun::width).sum();
        let measured = layout.metrics().unwrap().width;
        assert!(
            (walked - measured).abs() < 0.5,
            "glyph advances ({walked}) must sum to the layout width ({measured})"
        );
    }

    /// An underline applied to a range must come back from the walk with
    /// geometry that lands under exactly that range.
    ///
    /// This is the callback that used to discard its argument, so the failure it
    /// guards is silent: the text renders perfectly and the rule is simply
    /// absent — which for an IME composition span means the user cannot see what
    /// they are composing.
    #[test]
    fn an_underline_is_recorded_with_usable_geometry() {
        let format = TextFormat::new("Segoe UI", 16.0).unwrap();
        let text = "abcdefghij";
        let layout = TextLayout::new(text, &format, 1000.0, 100.0).unwrap();

        // Nothing marked: no decorations, and the runs are unaffected.
        let bare = layout.shape().unwrap();
        assert!(bare.decorations.is_empty(), "unmarked text has no rules");
        assert!(!bare.runs.is_empty());

        // Mark the middle four code units, as an IME marks its composition.
        layout.set_underline(true, 3, 4).unwrap();
        let marked = layout.shape().unwrap();
        assert_eq!(marked.decorations.len(), 1, "one span, one rule");

        let d = marked.decorations[0];
        assert_eq!(d.kind, DecorationKind::Underline);
        assert!(d.thickness > 0.0, "a rule with no thickness draws nothing");
        assert!(d.width > 0.0);

        // The rule sits BELOW the baseline: DirectWrite's offset is positive
        // downward, which is the opposite of `GlyphOffset::ascender_offset`.
        assert!(
            d.offset > 0.0,
            "underline offset {} should be below the baseline",
            d.offset
        );

        // And it spans the marked range, not the whole string. Compare against
        // what the layout itself says those code units occupy.
        let range = layout.hit_test_range(3, 4, 0.0, 0.0).unwrap();
        assert_eq!(range.len(), 1, "an unwrapped range is one rect");
        let (rx, _, rw, _) = range[0];
        let (dx, dy, dw, dh) = d.rect(false);
        assert!(
            (dx - rx).abs() < 0.5,
            "rule starts at {dx}, range at {rx}"
        );
        assert!(
            (dw - rw).abs() < 0.5,
            "rule spans {dw}, range spans {rw}"
        );
        assert!(dh > 0.0);
        // Strictly inside the string's width — the whole point of a span.
        let full = layout.metrics().unwrap().width;
        assert!(dw < full, "rule ({dw}) must be narrower than the text ({full})");
        assert!(dy > 0.0, "rule must be below the layout's top edge");

        // Clearing it takes the rule away again.
        layout.set_underline(false, 3, 4).unwrap();
        assert!(layout.shape().unwrap().decorations.is_empty());
    }

    /// The runs a decoration walk yields must be the same runs `glyph_runs`
    /// yields — recording decorations must not perturb glyph collection, since
    /// every existing caller goes through the narrower entry point.
    #[test]
    fn recording_decorations_leaves_the_runs_alone() {
        let format = TextFormat::new("Segoe UI", 16.0).unwrap();
        let layout = TextLayout::new("Hello glyphs", &format, 1000.0, 100.0).unwrap();
        layout.set_underline(true, 0, 5).unwrap();

        let shaped = layout.shape().unwrap();
        let runs = layout.glyph_runs().unwrap();
        assert!(!shaped.decorations.is_empty(), "the mark must be recorded");
        assert_eq!(runs.len(), shaped.runs.len());
        for (a, b) in runs.iter().zip(&shaped.runs) {
            assert_eq!(a.glyph_indices, b.glyph_indices);
            assert_eq!(a.glyph_advances, b.glyph_advances);
            assert_eq!(a.baseline_origin, b.baseline_origin);
        }
    }

    /// Rasterize one glyph at four fractional baseline origins and compare the
    /// coverage bytes.
    ///
    /// A glyph atlas that caches N subpixel phases is betting that a fractional
    /// origin changes the pixels. If the rasterizer grid-fits horizontally it
    /// snaps that origin away, every phase collapses to one image, and the
    /// atlas pays N× the memory for nothing. A composition surface has no CPU
    /// read path, so this asks DirectWrite directly — the same rasterizer D2D
    /// draws glyph runs with.
    ///
    /// Reports rather than asserts a specific answer: which grid-fit modes
    /// preserve phase is a property of the platform's rasterizer, not of this
    /// code, and the useful output is the count.
    #[test]
    fn subpixel_phases_are_distinct_or_are_not() {
        use crate::bindings::{
            IDWriteFactory2, DWRITE_GRID_FIT_MODE_DEFAULT, DWRITE_GRID_FIT_MODE_DISABLED,
            DWRITE_GRID_FIT_MODE_ENABLED, DWRITE_MEASURING_MODE_NATURAL,
            DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC, DWRITE_TEXTURE_ALIASED_1x1,
            DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
        };
        use windows_core::Interface;

        // Grayscale antialiasing produces a 1-byte-per-pixel texture. Asking
        // for the 3x1 ClearType type against a grayscale analysis is not an
        // error — DirectWrite just reports empty bounds, which reads exactly
        // like "the glyph rasterized to nothing".
        const TEX: DWRITE_TEXTURE_TYPE = DWRITE_TEXTURE_ALIASED_1x1;

        const EM: f32 = 14.0;
        let fmt = TextFormat::new("Segoe UI", EM).unwrap();
        let layout = TextLayout::new("n", &fmt, 500.0, 100.0).unwrap();
        let runs = layout.glyph_runs().unwrap();
        let run = runs.into_iter().next().expect("one run");
        eprintln!(
            "  run: {} glyph(s) {:?} em {} advances {:?}",
            run.len(),
            run.glyph_indices,
            run.font_em_size,
            run.glyph_advances
        );

        let factory = dwrite_factory()
            .unwrap()
            .cast::<IDWriteFactory2>()
            .expect("IDWriteFactory2");

        for (label, mode) in [
            ("default ", DWRITE_GRID_FIT_MODE_DEFAULT),
            ("disabled", DWRITE_GRID_FIT_MODE_DISABLED),
            ("enabled ", DWRITE_GRID_FIT_MODE_ENABLED),
        ] {
            // (bounds, coverage) per phase — bounds are part of the identity:
            // a phase that only shifts the box is still a distinct raster.
            let mut rasters: Vec<((i32, i32, i32, i32), Vec<u8>)> = Vec::new();
            for step in 0..4u32 {
                let phase = step as f32 / 4.0;
                let analysis = run.with_abi(|abi| unsafe {
                    factory.CreateGlyphRunAnalysis(
                        abi,
                        None,
                        DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
                        DWRITE_MEASURING_MODE_NATURAL,
                        mode,
                        DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                        phase,
                        0.0,
                    )
                });
                let Ok(analysis) = analysis else {
                    eprintln!("  {label}: analysis unavailable");
                    break;
                };
                let b = unsafe { analysis.GetAlphaTextureBounds(TEX) }
                    .unwrap();
                let len = ((b.right - b.left) * (b.bottom - b.top) * 3).max(0) as usize;
                let mut buf = vec![0u8; len];
                if len > 0 {
                    unsafe {
                        analysis.CreateAlphaTexture(TEX, &b, &mut buf)
                    }
                    .ok()
                    .unwrap();
                }
                rasters.push(((b.left, b.top, b.right, b.bottom), buf));
            }

            let mut distinct: Vec<&((i32, i32, i32, i32), Vec<u8>)> = Vec::new();
            for r in &rasters {
                if !distinct.contains(&r) {
                    distinct.push(r);
                }
            }
            eprintln!(
                "  grid-fit {label}: {}/{} phases distinct  (bounds {:?})",
                distinct.len(),
                rasters.len(),
                rasters.iter().map(|r| r.0).collect::<Vec<_>>()
            );
            // Measured: all four phases differ under every grid-fit mode, so
            // caching per phase buys real subpixel positioning. Asserted at
            // ">1 distinct" rather than "all 4" because the exact count is the
            // platform rasterizer's business — what must not silently become
            // true is that the origin is snapped away and every phase is the
            // same image, which would make the whole atlas dimension dead
            // weight.
            assert!(
                distinct.len() > 1,
                "grid-fit {label}: every subpixel phase rasterized identically — \
                 the phase dimension of the glyph atlas is pure overhead"
            );
        }
    }

    /// A trimmed layout must yield the ellipsis as GLYPHS, not merely truncate.
    ///
    /// DirectWrite renders a trimming sign as an *inline object*, delivered
    /// through `DrawInlineObject` rather than `DrawGlyphRun`. A collector that
    /// ignored that callback still saw the shortened text — so the failure was
    /// silent and partial: text ellipsized correctly on the painted path and
    /// came out cut mid-word, with no ellipsis, once the same layout was walked
    /// into sprites. Only text narrow enough to overflow showed it.
    #[test]
    fn a_trimmed_layout_yields_its_ellipsis_as_glyphs() {
        const TEXT: &str = "A Deliberately Long Window Title";
        let format = TextFormat::new("Segoe UI", 16.0).unwrap();
        let layout = TextLayout::new(TEXT, &format, 10_000.0, 100.0).unwrap();
        layout.set_word_wrap(false).unwrap();

        let full: usize = layout.glyph_runs().unwrap().iter().map(GlyphRun::len).sum();
        assert_eq!(full, TEXT.len(), "untrimmed, every character is shaped");

        // Narrow it to roughly a third and arm the trimming sign.
        layout.set_trimming(Trimming::CharacterEllipsis, &format).unwrap();
        let narrow = layout.metrics().unwrap().width / 3.0;
        layout.set_max_width(narrow).unwrap();

        let runs = layout.glyph_runs().unwrap();
        let trimmed: usize = runs.iter().map(GlyphRun::len).sum();
        assert!(
            trimmed < full,
            "the run must actually shorten ({trimmed} of {full})",
        );
        assert!(
            runs.len() >= 2,
            "the sign arrives as its own run: the text, then the ellipsis              (got {} run(s) — DrawInlineObject dropped it)",
            runs.len(),
        );
        // The sign sits at the trim point, so it is the last thing drawn and it
        // carries at least one glyph of its own.
        assert!(
            runs.last().is_some_and(|r| !r.is_empty()),
            "the trailing ellipsis run must carry glyphs",
        );
    }
}
