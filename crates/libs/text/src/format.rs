//! Turning a [`FontSpec`] into the DirectWrite format a layout is built from.
//!
//! Formats are cached because a screen resolves a handful of type rungs and then asks for
//! them once per label, and building one is a family lookup plus three property sets.
//!
//! **A format carries no alignment.** A layout property belongs to the container, so text
//! is always laid out leading and near, and where it sits inside its box is decided by
//! whoever placed the box.

use super::*;

/// The locale a format is built for, as the reference has it. It drives number
/// substitution and the Han unification fallback picks with it, so it is a stated choice
/// rather than a default to inherit.
const LOCALE: &str = "en-us";

/// How a run occupies the width it is given.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Flow {
    /// One line, whatever the width. Its intrinsic width *is* its measurement, which is
    /// what a pill or a label sizes to.
    #[default]
    Line,
    /// One line, trimmed at the width with an ellipsis.
    Ellipsis,
    /// Wrapped into as many lines as the width needs.
    Wrap,
}

impl Flow {
    const fn wrapping(self) -> DWRITE_WORD_WRAPPING {
        match self {
            Self::Wrap => DWRITE_WORD_WRAPPING_WRAP,
            _ => DWRITE_WORD_WRAPPING_NO_WRAP,
        }
    }
}

/// What selects a cached format. `size` is carried as bits so the key can be hashed —
/// two sizes that compare equal as `f32` produce the same format either way.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FormatKey {
    family: FamilyId,
    size: u32,
    weight: u16,
    style: FontStyle,
    stretch: FontStretch,
    flow: Flow,
}

impl FormatKey {
    fn new(spec: &FontSpec, flow: Flow) -> Self {
        Self {
            family: spec.family,
            size: spec.size.to_bits(),
            weight: spec.weight,
            style: spec.style,
            stretch: spec.stretch,
            flow,
        }
    }
}

impl TextEngine {
    /// The format `spec` and `flow` select, from the cache or freshly built.
    ///
    /// Features are absent from the key: they are applied to the *layout* as a typography
    /// range, because that is the only place DirectWrite accepts them.
    pub(crate) fn format(&self, spec: &FontSpec, flow: Flow) -> Result<IDWriteTextFormat> {
        let key = FormatKey::new(spec, flow);
        if let Some(format) = self.formats.borrow().get(&key) {
            return Ok(format.clone());
        }
        let format = self.build_format(spec, flow)?;
        self.formats.borrow_mut().insert(key, format.clone());
        Ok(format)
    }

    fn build_format(&self, spec: &FontSpec, flow: Flow) -> Result<IDWriteTextFormat> {
        let family = wide(self.ladder().name(spec.family));
        let locale = wide(LOCALE);

        // SAFETY: both wide buffers are NUL-terminated and outlive the call, and every
        // setter below takes only scalars or an interface this frame owns.
        unsafe {
            let format = self.factory.CreateTextFormat(
                windows_core::PCWSTR(family.as_ptr()),
                &self.collection,
                i32::from(spec.weight),
                spec.style.dwrite(),
                spec.stretch.dwrite(),
                spec.size,
                windows_core::PCWSTR(locale.as_ptr()),
            )?;

            // Drive the optical-size axis from the em size on a variable font, so a size
            // change selects the letterforms designed for it instead of scaling one set.
            // A no-op on a static face. An application wanting this asks its ladder for a
            // variable family by name; substituting one here would be a policy the crate
            // has no standing to have.
            if let Ok(format3) = format.cast::<IDWriteTextFormat3>() {
                let _ = format3.SetAutomaticFontAxes(DWRITE_AUTOMATIC_FONT_AXES_OPTICAL_SIZE);
            }

            // Stated rather than inherited: a layout property belongs to the container.
            format
                .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)
                .ok()?;
            format
                .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)
                .ok()?;
            format.SetWordWrapping(flow.wrapping()).ok()?;

            if flow == Flow::Ellipsis {
                let sign = self.factory.CreateEllipsisTrimmingSign(&format)?;
                let options = DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                };
                format.SetTrimming(&options, &sign).ok()?;
            }

            Ok(format)
        }
    }

    /// The typography object for `features`, or `None` where the family's own defaults are
    /// wanted. Cached: there are as many of these as there are feature sets, which is two.
    pub(crate) fn typography(&self, features: FontFeatures) -> Option<IDWriteTypography> {
        if features.is_empty() {
            return None;
        }
        if let Some(typo) = self.typography.borrow().get(&features) {
            return typo.clone();
        }
        let built = self.build_typography(features).ok();
        self.typography.borrow_mut().insert(features, built.clone());
        built
    }

    fn build_typography(&self, features: FontFeatures) -> Result<IDWriteTypography> {
        // SAFETY: calls on interfaces this frame owns, taking only scalars.
        unsafe {
            let typo = self.factory.CreateTypography()?;
            if features.has(FontFeatures::TABULAR) {
                typo.AddFontFeature(DWRITE_FONT_FEATURE {
                    nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                    parameter: 1,
                })
                .ok()?;
            }
            Ok(typo)
        }
    }
}
