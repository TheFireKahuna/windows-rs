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
        unsafe { self.raw.SetMaxWidth(max_width) }
    }

    /// Update the layout box height.
    pub fn set_max_height(&self, max_height: f32) -> Result<()> {
        unsafe { self.raw.SetMaxHeight(max_height) }
    }

    /// Set word wrapping on / off for this layout.
    pub fn set_word_wrap(&self, wrap: bool) -> Result<()> {
        let value = if wrap {
            DWRITE_WORD_WRAPPING_WRAP
        } else {
            DWRITE_WORD_WRAPPING_NO_WRAP
        };
        unsafe { self.raw.SetWordWrapping(value) }
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
            self.raw.SetTrimming(&options, sign.as_ref())
        }
    }

    /// Measure the laid-out text. The key field for content-sizing is
    /// [`TextMetrics::width`] (formatted width sans trailing whitespace).
    pub fn metrics(&self) -> Result<TextMetrics> {
        let mut m = DWRITE_TEXT_METRICS::default();
        unsafe { self.raw.GetMetrics(&mut m)? };
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
                .HitTestPoint(x, y, &mut is_trailing, &mut is_inside, &mut hm)?;
        };
        Ok(HitTestResult {
            text_position: hm.textPosition,
            is_trailing_hit: is_trailing.as_bool(),
            is_inside: is_inside.as_bool(),
            glyph_rect: (hm.left, hm.top, hm.width, hm.height),
        })
    }

    /// Map a text position to a caret point (layout-relative DIPs) and the
    /// cluster bounds. The returned `(x, y)` is the caret origin; the
    /// [`HitTestResult::glyph_rect`] gives the cluster box.
    pub fn caret_at(&self, text_position: u32, after: bool) -> Result<((f32, f32), HitTestResult)> {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut hm = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
            self.raw
                .HitTestTextPosition(text_position, after, &mut x, &mut y, &mut hm)?;
        };
        Ok((
            (x, y),
            HitTestResult {
                text_position: hm.textPosition,
                is_trailing_hit: after,
                is_inside: true,
                glyph_rect: (hm.left, hm.top, hm.width, hm.height),
            },
        ))
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
