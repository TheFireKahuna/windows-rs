use super::*;

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlignment {
    /// Align to the leading edge (left in LTR).
    #[default]
    Leading,
    /// Center horizontally.
    Center,
    /// Align to the trailing edge (right in LTR).
    Trailing,
}

/// Vertical paragraph alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParagraphAlignment {
    /// Align to the top edge.
    #[default]
    Top,
    /// Center vertically.
    Center,
    /// Align to the bottom edge.
    Bottom,
}

/// Font weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontWeight(pub i32);

impl FontWeight {
    /// Normal (regular) weight, 400.
    pub const NORMAL: Self = Self(400);
    /// Bold weight, 700.
    pub const BOLD: Self = Self(700);
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// A text format describing font family, size, weight, and alignment.
///
/// ```ignore
/// let format = TextFormat::new("Segoe UI", 24.0)?
///     .with_alignment(TextAlignment::Center);
/// ```
#[derive(Clone)]
pub struct TextFormat {
    raw: IDWriteTextFormat,
}

impl TextFormat {
    /// Creates a text format with normal weight.
    pub fn new(family: &str, size: f32) -> Result<Self> {
        Self::with_weight(family, size, FontWeight::NORMAL)
    }

    /// Creates a text format with bold weight.
    pub fn new_bold(family: &str, size: f32) -> Result<Self> {
        Self::with_weight(family, size, FontWeight::BOLD)
    }

    /// Creates a text format with the given font weight.
    pub fn with_weight(family: &str, size: f32, weight: FontWeight) -> Result<Self> {
        let factory = dwrite_factory()?;

        // "Segoe UI" is the legacy *static* name of the Win11 system UI face; request
        // the variable font ("Segoe UI Variable") instead so the optical-size (`opsz`)
        // axis is available (see the automatic-axes call below). DWrite falls back to
        // the static face if the variable family is absent, and other families (mono,
        // Fluent icons) pass through unchanged.
        let family = if family == "Segoe UI" {
            "Segoe UI Variable"
        } else {
            family
        };

        let family_wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
        let locale_wide: Vec<u16> = "en-us\0".encode_utf16().collect();

        let raw = unsafe {
            factory.CreateTextFormat(
                PCWSTR(family_wide.as_ptr()),
                None,
                weight.0,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                PCWSTR(locale_wide.as_ptr()),
            )?
        };

        // Drive the optical-size (`opsz`) axis from the em size on variable fonts —
        // DirectWrite derives the correct DIP→point mapping and selects the
        // size-appropriate glyph outlines (crisper small UI text, better-proportioned
        // large text). A no-op on static faces. The modern system-font path (v3 API).
        if let Ok(fmt3) = Interface::cast::<IDWriteTextFormat3>(&raw) {
            unsafe {
                let _ = fmt3.SetAutomaticFontAxes(DWRITE_AUTOMATIC_FONT_AXES_OPTICAL_SIZE);
            }
        }

        Ok(Self { raw })
    }

    /// Sets the horizontal text alignment.
    pub fn with_alignment(self, alignment: TextAlignment) -> Self {
        let value = match alignment {
            TextAlignment::Leading => DWRITE_TEXT_ALIGNMENT_LEADING,
            TextAlignment::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            TextAlignment::Trailing => DWRITE_TEXT_ALIGNMENT_TRAILING,
        };
        unsafe { _ = self.raw.SetTextAlignment(value) };
        self
    }

    /// Sets the vertical paragraph alignment.
    pub fn with_paragraph_alignment(self, alignment: ParagraphAlignment) -> Self {
        let value = match alignment {
            ParagraphAlignment::Top => DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
            ParagraphAlignment::Center => DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ParagraphAlignment::Bottom => DWRITE_PARAGRAPH_ALIGNMENT_FAR,
        };
        unsafe { _ = self.raw.SetParagraphAlignment(value) };
        self
    }

    /// Sets word wrapping on / off (default: wrap). Turning it off keeps text on
    /// one line — the common case for value pills, axis labels, and readouts.
    pub fn with_word_wrap(self, wrap: bool) -> Self {
        let value = if wrap {
            DWRITE_WORD_WRAPPING_WRAP
        } else {
            DWRITE_WORD_WRAPPING_NO_WRAP
        };
        unsafe { _ = self.raw.SetWordWrapping(value) };
        self
    }

    /// Returns the underlying `IDWriteTextFormat`.
    pub fn raw(&self) -> &IDWriteTextFormat {
        &self.raw
    }
}

/// Where text trims when it overflows its layout box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Trimming {
    /// No trimming — overflowing text is clipped by the draw rect.
    #[default]
    None,
    /// Trim at a character boundary and show an ellipsis (`…`).
    CharacterEllipsis,
    /// Trim at a word boundary and show an ellipsis (`…`).
    WordEllipsis,
}

/// Glyph-run-resolved metrics of a laid-out string (a measured
/// [`TextLayout`]). All values are in DIPs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    /// Distance from the layout box's left edge to the leftmost drawn glyph.
    pub left: f32,
    /// Distance from the layout box's top edge to the topmost drawn glyph.
    pub top: f32,
    /// Width of the formatted text, ignoring trailing whitespace. This is the
    /// value to size a value-pill / readout to.
    pub width: f32,
    /// Width including trailing whitespace.
    pub width_including_trailing_whitespace: f32,
    /// Height of the formatted text (ascent + descent + line gap, summed over
    /// lines).
    pub height: f32,
    /// The `max_width` the layout was created/constrained with.
    pub layout_width: f32,
    /// The `max_height` the layout was created/constrained with.
    pub layout_height: f32,
    /// Number of lines after wrapping.
    pub line_count: u32,
}

/// Result of a hit-test: which text position a point maps to (and whether it
/// fell on the trailing half of that glyph). Drives caret placement and
/// click-to-edit in numeric fields.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitTestResult {
    /// The text position (UTF-16 code-unit index) under the point.
    pub text_position: u32,
    /// True if the point is on the trailing (right) half of the glyph — i.e.
    /// the caret belongs *after* `text_position`.
    pub is_trailing_hit: bool,
    /// True if the point is inside the text (vs. past the end / above-below).
    pub is_inside: bool,
    /// Bounding box of the hit glyph cluster, in DIPs: `(left, top, width, height)`.
    pub glyph_rect: (f32, f32, f32, f32),
    /// Length of the hit cluster in code units. A cluster is not always one
    /// code unit: a surrogate pair, a combining sequence and a ligature are all
    /// one indivisible caret stop spanning several.
    pub length: u32,
    /// Bidi embedding level of the run this position sits in. **Odd means
    /// right-to-left** — the one field that says whether visual and logical
    /// order agree here, and so whether caret affinity is observable at all.
    pub bidi_level: u32,
    /// False for a position that is not backed by a character — the region past
    /// the end of a line, most commonly.
    pub is_text: bool,
    /// True if the cluster was trimmed away (ellipsis) and is not drawn.
    pub is_trimmed: bool,
}

impl HitTestResult {
    /// True if this position sits in a right-to-left run.
    pub fn is_rtl(&self) -> bool {
        self.bidi_level % 2 == 1
    }
}

/// A laid-out, measurable run of text — wraps `IDWriteTextLayout`.
///
/// Unlike [`TextFormat`] (which only describes font + alignment and is drawn
/// via [`DrawingSession::draw_text`]), a `TextLayout` is *measured*: it resolves
/// glyph runs against a constraint box so you can size UI to content
/// ([`metrics`](Self::metrics)), hit-test for caret placement
/// ([`hit_test_point`](Self::hit_test_point)), and trim with an ellipsis. Build
/// once and cache it (recreate on text/format/constraint change), then draw with
/// [`DrawingSession::draw_text_layout`].
#[derive(Clone)]
pub struct TextLayout {
    raw: IDWriteTextLayout,
}

impl TextLayout {
    /// Lay out `text` with `format` inside a `max_width` × `max_height` box (DIPs).
    /// Pass `f32::INFINITY` for an unconstrained axis (e.g. single-line measure).
    pub fn new(text: &str, format: &TextFormat, max_width: f32, max_height: f32) -> Result<Self> {
        let factory = dwrite_factory()?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        let raw = unsafe { factory.CreateTextLayout(&wide, format.raw(), max_width, max_height)? };
        Ok(Self { raw })
    }

    /// Update the layout box width (re-flows wrapping / trimming).
    pub fn set_max_width(&self, max_width: f32) -> Result<()> {
        unsafe { self.raw.SetMaxWidth(max_width).ok() }
    }

    /// Update the layout box height.
    pub fn set_max_height(&self, max_height: f32) -> Result<()> {
        unsafe { self.raw.SetMaxHeight(max_height).ok() }
    }

    /// Set word wrapping on / off for this layout.
    pub fn set_word_wrap(&self, wrap: bool) -> Result<()> {
        let value = if wrap {
            DWRITE_WORD_WRAPPING_WRAP
        } else {
            DWRITE_WORD_WRAPPING_NO_WRAP
        };
        unsafe { self.raw.SetWordWrapping(value).ok() }
    }

    /// Apply ellipsis trimming. `Trimming::None` removes it. The ellipsis sign is
    /// derived from the supplied `format` (so it matches the run's font).
    pub fn set_trimming(&self, trimming: Trimming, format: &TextFormat) -> Result<()> {
        let granularity = match trimming {
            Trimming::None => DWRITE_TRIMMING_GRANULARITY_NONE,
            Trimming::CharacterEllipsis => DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            Trimming::WordEllipsis => DWRITE_TRIMMING_GRANULARITY_WORD,
        };
        let options = DWRITE_TRIMMING {
            granularity,
            delimiter: 0,
            delimiterCount: 0,
        };
        unsafe {
            let sign = if matches!(trimming, Trimming::None) {
                None
            } else {
                let factory = dwrite_factory()?;
                Some(factory.CreateEllipsisTrimmingSign(format.raw())?)
            };
            // SetTrimming is inherited from IDWriteTextFormat (the layout derefs to it).
            self.raw.SetTrimming(&options, sign.as_ref()).ok()
        }
    }

    /// Measure the laid-out text. The key field for content-sizing is
    /// [`TextMetrics::width`] (formatted width sans trailing whitespace).
    pub fn metrics(&self) -> Result<TextMetrics> {
        let mut m = DWRITE_TEXT_METRICS::default();
        unsafe { self.raw.GetMetrics(&mut m).ok()? };
        Ok(TextMetrics {
            left: m.left,
            top: m.top,
            width: m.width,
            width_including_trailing_whitespace: m.widthIncludingTrailingWhitespace,
            height: m.height,
            layout_width: m.layoutWidth,
            layout_height: m.layoutHeight,
            line_count: m.lineCount,
        })
    }

    /// Single-line content size `(width, height)` in DIPs — the common pill /
    /// label measurement. Equivalent to `metrics()` projected to `(width, height)`.
    pub fn measure(&self) -> Result<(f32, f32)> {
        let m = self.metrics()?;
        Ok((m.width, m.height))
    }

    /// Hit-test a point (layout-relative DIPs) to a text position. Drives
    /// caret placement / click-to-edit.
    pub fn hit_test_point(&self, x: f32, y: f32) -> Result<HitTestResult> {
        let mut is_trailing = BOOL(0);
        let mut is_inside = BOOL(0);
        let mut hm = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
            self.raw
                .HitTestPoint(x, y, &mut is_trailing, &mut is_inside, &mut hm)
                .ok()?;
        };
        Ok(HitTestResult {
            text_position: hm.textPosition,
            is_trailing_hit: is_trailing.as_bool(),
            is_inside: is_inside.as_bool(),
            glyph_rect: (hm.left, hm.top, hm.width, hm.height),
            length: hm.length,
            bidi_level: hm.bidiLevel,
            is_text: hm.isText.as_bool(),
            is_trimmed: hm.isTrimmed.as_bool(),
        })
    }

    /// Map a text position to a caret point (layout-relative DIPs) and the
    /// cluster bounds. The returned `(x, y)` is the caret origin; the
    /// [`HitTestResult::glyph_rect`] gives the cluster box.
    ///
    /// `after` selects which **edge of the character at `text_position`** is
    /// wanted: `false` its leading edge, `true` its trailing edge. In
    /// left-to-right text `caret_at(i, false)` and `caret_at(i - 1, true)` name
    /// the same point, which is why a caret can get away with only ever asking
    /// for the first. Across a direction boundary they are different points on
    /// opposite sides of a word, and the choice between them is the caret's
    /// affinity.
    ///
    /// The returned `is_trailing_hit` echoes `after` and `is_inside` is always
    /// true: `DWRITE_HIT_TEST_METRICS` carries neither, because both are
    /// answers to *point* hit-testing and a text position is by construction a
    /// position in the text. `bidi_level` is the field to read here — it says
    /// whether this position sits somewhere the affinity is observable.
    pub fn caret_at(&self, text_position: u32, after: bool) -> Result<((f32, f32), HitTestResult)> {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut hm = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
            self.raw
                .HitTestTextPosition(text_position, after, &mut x, &mut y, &mut hm)
                .ok()?;
        };
        Ok((
            (x, y),
            HitTestResult {
                text_position: hm.textPosition,
                is_trailing_hit: after,
                is_inside: true,
                glyph_rect: (hm.left, hm.top, hm.width, hm.height),
                length: hm.length,
                bidi_level: hm.bidiLevel,
                is_text: hm.isText.as_bool(),
                is_trimmed: hm.isTrimmed.as_bool(),
            },
        ))
    }

    /// Selection / range geometry: the rectangles (layout-relative DIPs,
    /// offset by `origin`) covering the code-unit range `[position,
    /// position + length)`. One rect per line the range spans. Drives the
    /// editor's selection highlight and IME candidate-window placement.
    /// Returns an empty vec for a zero-length range.
    pub fn hit_test_range(
        &self,
        position: u32,
        length: u32,
        origin_x: f32,
        origin_y: f32,
    ) -> Result<Vec<(f32, f32, f32, f32)>> {
        if length == 0 {
            return Ok(Vec::new());
        }
        // First call (no buffer) reports the required metric count via the
        // `actual` out-param; the HRESULT is the expected insufficient-buffer
        // error, so it is deliberately ignored.
        let mut count = 0u32;
        unsafe {
            let _ = self
                .raw
                .HitTestTextRange(position, length, origin_x, origin_y, None, &mut count);
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut metrics = vec![DWRITE_HIT_TEST_METRICS::default(); count as usize];
        unsafe {
            self.raw.HitTestTextRange(
                position,
                length,
                origin_x,
                origin_y,
                Some(&mut metrics),
                &mut count,
            )
            .ok()?;
        }
        Ok(metrics
            .iter()
            .take(count as usize)
            .map(|m| (m.left, m.top, m.width, m.height))
            .collect())
    }

    /// Re-weight the code-unit range `[start, start + length)`.
    ///
    /// The layout is built from a single [`TextFormat`], so its weight applies
    /// to the whole run; this overrides it for one span. That is what lets one
    /// layout carry mixed emphasis — a bold lead-in followed by body text —
    /// and still wrap and measure as **one paragraph**, which two separately
    /// laid-out runs cannot do (each would wrap inside its own box, and the
    /// caller would have to re-implement line breaking to flow the second
    /// after the first).
    pub fn set_font_weight(&self, weight: FontWeight, start: u32, length: u32) -> Result<()> {
        let range = DWRITE_TEXT_RANGE {
            startPosition: start,
            length,
        };
        unsafe { self.raw.SetFontWeight(weight.0, range).ok() }
    }

    /// Toggle an underline over the code-unit range `[start, start + length)`.
    /// Used to mark the active IME composition span.
    pub fn set_underline(&self, has_underline: bool, start: u32, length: u32) -> Result<()> {
        let range = DWRITE_TEXT_RANGE {
            startPosition: start,
            length,
        };
        unsafe { self.raw.SetUnderline(has_underline, range).ok() }
    }

    /// Returns the underlying `IDWriteTextLayout`.
    pub fn raw(&self) -> &IDWriteTextLayout {
        &self.raw
    }
}

pub(crate) fn dwrite_factory() -> Result<IDWriteFactory> {
    // The DirectWrite shared factory is thread-safe, but the faithful in-house
    // metadata does not mark `IDWriteFactory` `[agile]`, so it is neither `Send` nor
    // `Sync`. Wrap it for the process-wide `OnceLock`.
    struct SharedFactory(IDWriteFactory);
    unsafe impl Send for SharedFactory {}
    unsafe impl Sync for SharedFactory {}

    static SHARED: std::sync::OnceLock<SharedFactory> = std::sync::OnceLock::new();

    if let Some(factory) = SHARED.get() {
        return Ok(factory.0.clone());
    }

    let mut factory: Option<IDWriteFactory> = None;
    unsafe {
        DWriteCreateFactory(
            DWRITE_FACTORY_TYPE_SHARED,
            &IDWriteFactory::IID,
            &mut factory as *mut _ as *mut _,
        )
        .ok()?;
    }
    let factory = factory.ok_or_else(Error::empty)?;
    Ok(SHARED.get_or_init(|| SharedFactory(factory)).0.clone())
}

// `DWRITE_PIXEL_GEOMETRY_FLAT` — grayscale, no RGB/BGR subpixel channels.
const PIXEL_GEOMETRY_FLAT: i32 = 0;
// `DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC` — the high-quality symmetric
// anti-aliased outline mode modern Windows UI text uses (v1 enum value 5, shared
// by `DWRITE_RENDERING_MODE1`).
const RENDERING_MODE1_NATURAL_SYMMETRIC: i32 = 5;
// `DWRITE_GRID_FIT_MODE_ENABLED` — snap glyph outlines to the pixel grid
// (hinting). This is what makes small UI text crisp instead of soft; the base v1
// `CreateCustomRenderingParams` can't express it, hence the v3 call below.
const GRID_FIT_MODE_ENABLED: i32 = 2;
// Grayscale enhanced contrast for `CreateCustomRenderingParams`. No-op on this path:
// on NATURAL_SYMMETRIC grayscale, sweeping it (0.5 / 1.5 / 2.5) produced byte-identical
// output on the dcomp FP16 target. Left at 0.
const GRAYSCALE_ENHANCED_CONTRAST: f32 = 0.0;
// Gamma for the DirectWrite text-AA coverage ramp. Not a color-space encoding and does
// not change text color — at full coverage the pixel is the exact linear scRGB
// foreground; it only bends the partially-covered (anti-aliased) pixels, which at UI
// sizes is most of a ~1px stem.
//
// The color-correct value for a linear FP16 scRGB target is 1.0 (coverage blended in
// linear light). Direct2D does not auto-detect the linear surface — changing this value
// changes the written pixels, so D2D applies it as the assumed target encoding; at 2.2
// the coverage ramp is written non-linear into the linear buffer. At 1.0, light-on-dark
// UI text is ~26% less ink over the same glyphs (measured). Grayscale enhanced contrast
// does not compensate (inert, above); the variable `wght` axis plateaus below 2.2's
// apparent weight and turns the UI semibold. 2.2 matches the weight small text is
// designed for at the design weight.
const TEXT_AA_COVERAGE_GAMMA: f32 = 2.2;

/// Custom text rendering params for the self-hosted Direct2D backend: the v3
/// DirectWrite params (`IDWriteFactory3::CreateCustomRenderingParams`) with grid-fit
/// ENABLED, NATURAL_SYMMETRIC grayscale outline mode, ClearType off (subpixel AA is
/// invalid on premultiplied / transparent surfaces), and the coverage gamma from
/// [`TEXT_AA_COVERAGE_GAMMA`]. Grid-fit + NATURAL_SYMMETRIC provide the crispness; the
/// gamma sets the anti-aliasing coverage ramp.
///
/// `linear` selects a cache slot only — both slots build identical params. Falls back
/// to `None` (Direct2D defaults) if `IDWriteFactory3` is unavailable; always present
/// on the Win11 target.
pub(crate) fn text_rendering_params(linear: bool) -> Option<IDWriteRenderingParams> {
    // DirectWrite rendering params are thread-safe, but the faithful in-house
    // metadata does not mark `IDWriteRenderingParams` `[agile]`, so it is neither
    // `Send` nor `Sync`. Wrap it for the process-wide `OnceLock`, exactly as
    // `dwrite_factory` does for the shared factory.
    struct SharedParams(Option<IDWriteRenderingParams>);
    unsafe impl Send for SharedParams {}
    unsafe impl Sync for SharedParams {}

    static LINEAR: std::sync::OnceLock<SharedParams> = std::sync::OnceLock::new();
    static SRGB: std::sync::OnceLock<SharedParams> = std::sync::OnceLock::new();

    let slot = if linear { &LINEAR } else { &SRGB };
    slot.get_or_init(|| {
        SharedParams((|| {
        let factory = dwrite_factory().ok()?;
        let factory3: IDWriteFactory3 = Interface::cast(&factory).ok()?;
        unsafe {
            factory3
                .CreateCustomRenderingParams(
                    TEXT_AA_COVERAGE_GAMMA,
                    0.0, // enhanced contrast (ClearType) — unused for grayscale
                    GRAYSCALE_ENHANCED_CONTRAST,
                    0.0, // ClearType level — grayscale, no subpixel
                    PIXEL_GEOMETRY_FLAT,
                    RENDERING_MODE1_NATURAL_SYMMETRIC,
                    GRID_FIT_MODE_ENABLED,
                )
                .ok()
                // v3 call yields IDWriteRenderingParams3; callers take the base type.
                .and_then(|p| p.cast::<IDWriteRenderingParams>().ok())
        }
        })())
    })
    .0
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `abc` then the Hebrew `אבג`. Logical order is `a b c א ב ג`; visual order
    /// is `a b c ג ב א`, so logical index 3 — the gap between `c` and `א` — has
    /// two correct places on screen.
    const MIXED: &str = "abc\u{05D0}\u{05D1}\u{05D2}";
    /// Index of that boundary, in UTF-16 code units.
    const BOUNDARY: u32 = 3;

    fn layout(text: &str) -> TextLayout {
        let format = TextFormat::new("Segoe UI", 16.0).expect("Segoe UI is a system font");
        TextLayout::new(text, &format, 1000.0, 100.0).expect("layout the fixture")
    }

    /// The premise the whole affinity mechanism rests on: at a direction
    /// boundary the two edges are genuinely different points. If DirectWrite
    /// answered the same x for both, carrying the bit would buy nothing.
    #[test]
    fn the_two_affinities_differ_at_a_direction_boundary() {
        let l = layout(MIXED);
        let ((downstream, _), _) = l.caret_at(BOUNDARY, false).unwrap();
        let ((upstream, _), _) = l.caret_at(BOUNDARY - 1, true).unwrap();
        assert!(
            (downstream - upstream).abs() > 1.0,
            "index {BOUNDARY} must have two visual positions in bidi text, \
             got downstream={downstream} upstream={upstream}"
        );
    }

    /// …and the reason a caret can ship without the bit and look correct: in
    /// pure left-to-right text the same two queries name one point. This is the
    /// regression guard on the 99% case — if it ever fails, the affinity work
    /// has moved the caret in ordinary Latin text.
    #[test]
    fn the_two_affinities_agree_in_ltr_text() {
        let l = layout("abcdef");
        for i in 1..6u32 {
            let ((downstream, _), _) = l.caret_at(i, false).unwrap();
            let ((upstream, _), _) = l.caret_at(i - 1, true).unwrap();
            assert!(
                (downstream - upstream).abs() < 0.01,
                "LTR index {i}: downstream={downstream} upstream={upstream} must coincide"
            );
        }
    }

    /// Each affinity must land on the run it claims to belong to — it is not
    /// enough that they differ. Upstream at the boundary belongs to `c`, so it
    /// sits at the LTR run's trailing edge; downstream belongs to `א`, which is
    /// in the RTL run and therefore further right, past the whole Hebrew word.
    #[test]
    fn each_affinity_sits_on_its_own_run() {
        let l = layout(MIXED);
        let ((upstream, _), up_hit) = l.caret_at(BOUNDARY - 1, true).unwrap();
        let ((downstream, _), down_hit) = l.caret_at(BOUNDARY, false).unwrap();

        assert!(!up_hit.is_rtl(), "index 2 is `c`, a left-to-right character");
        assert!(down_hit.is_rtl(), "index 3 is Hebrew, a right-to-left character");
        assert!(
            downstream > upstream,
            "the RTL run renders to the right of `abc`, so its leading edge \
             ({downstream}) must be past `c`'s trailing edge ({upstream})"
        );
    }

    /// `bidi_level` is the field that says whether affinity is observable at
    /// all, so it must actually be populated rather than defaulted to zero.
    #[test]
    fn bidi_level_is_reported_per_run() {
        let l = layout(MIXED);
        let ltr = l.caret_at(0, false).unwrap().1;
        let rtl = l.caret_at(4, false).unwrap().1;
        assert_eq!(ltr.bidi_level % 2, 0, "`a` is in a left-to-right run");
        assert_eq!(rtl.bidi_level % 2, 1, "`ב` is in a right-to-left run");
        assert!(!ltr.is_rtl() && rtl.is_rtl());
    }

    /// A hit test must report the cluster's true length, not an assumed 1.
    /// Stepping past a trailing hit by one code unit lands the caret inside a
    /// surrogate pair — a position that does not exist.
    #[test]
    fn a_surrogate_pair_is_one_cluster_of_two_units() {
        // U+1D400 MATHEMATICAL BOLD CAPITAL A — one character, two UTF-16 units.
        let l = layout("a\u{1D400}b");
        let hit = l.caret_at(1, false).unwrap().1;
        assert_eq!(
            hit.length, 2,
            "a surrogate pair is one indivisible caret stop spanning two units"
        );
    }

    /// A selection crossing a direction boundary is one logical range but two
    /// visual runs, and `hit_test_range` must return both rather than a single
    /// interval spanning the gap between them.
    #[test]
    fn a_bidi_range_hit_tests_into_several_rects() {
        let l = layout(MIXED);
        // `c` plus the first Hebrew letter: contiguous logically, split visually.
        let rects = l.hit_test_range(2, 2, 0.0, 0.0).unwrap();
        assert!(
            rects.len() >= 2,
            "a range crossing a direction boundary must produce one rect per \
             visual run, got {rects:?}"
        );
    }
}
