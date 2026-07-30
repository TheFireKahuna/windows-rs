//! Masking, and the gradient that is a mask rather than a colour (feature `system`).
//!
//! A mask brush multiplies one brush's alpha by another brush's colour, as a single
//! sprite. That separation is the whole reason the retained tree can carry wide-gamut
//! colour: coverage comes from an 8-bit alpha source, colour comes from a surface the
//! app allocated in a float format, and neither constrains the other.

use super::*;

/// A brush that paints `source`'s colour through `mask`'s alpha.
///
/// The universal retained construction. The mask supplies coverage — a rasterized
/// coverage surface, a gradient ramp, a captured subtree — and the source supplies
/// colour, which may be an FP16 surface holding values above paper white that no
/// `CompositionColorBrush` could express.
///
/// **Nesting is legal exactly two levels deep**: a mask brush whose *source* is itself
/// a mask brush works, and is how a gradient ramp stays unrasterized. Three levels
/// throws.
#[derive(Clone)]
pub struct CompositionMaskBrush(pub(crate) bindings::CompositionMaskBrush);

impl CompositionMaskBrush {
    /// Sets the brush supplying alpha. Only its alpha channel is read.
    pub fn set_mask(&self, brush: &impl Brush) {
        self.0.SetMask(&brush.as_brush().0).unwrap();
    }

    /// Sets the brush supplying colour.
    pub fn set_source(&self, brush: &impl Brush) {
        self.0.SetSource(&brush.as_brush().0).unwrap();
    }
}

impl Sealed for CompositionMaskBrush {}

impl Brush for CompositionMaskBrush {
    fn as_brush(&self) -> CompositionBrush {
        CompositionBrush(self.0.cast().unwrap())
    }
}

/// Whether a gradient's start and end points are absolute or relative to the painted
/// area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MappingMode {
    /// Points are DIPs in the painted visual's own space, so a resize leaves the ramp
    /// where it was.
    Absolute,
    /// Points are fractions of the painted area, so the ramp follows a resize with no
    /// property write and no raster at all.
    Relative,
}

impl From<MappingMode> for bindings::CompositionMappingMode {
    fn from(mode: MappingMode) -> Self {
        match mode {
            MappingMode::Absolute => Self::Absolute,
            MappingMode::Relative => Self::Relative,
        }
    }
}

/// A linear gradient, used here as an **alpha ramp** rather than as a colour source.
///
/// Its stops are `Windows.UI.Color`, which is 8-bit and clamps below paper white on an
/// HDR desktop — so a gradient is the wrong way to carry colour in this stack. Bound as
/// a [`CompositionMaskBrush`]'s mask it is exactly right: the compositor evaluates the
/// ramp per pixel, an app-allocated float surface still supplies the colour, and a
/// resize under [`MappingMode::Relative`] costs nothing.
#[derive(Clone)]
pub struct CompositionLinearGradientBrush(pub(crate) bindings::CompositionLinearGradientBrush);

impl CompositionLinearGradientBrush {
    /// Sets the ramp's start and end points, interpreted per the
    /// [mapping mode](Self::set_mapping_mode).
    pub fn set_line(&self, start: Vector2, end: Vector2) {
        self.0.SetStartPoint(start).unwrap();
        self.0.SetEndPoint(end).unwrap();
    }

    /// Sets whether the points are absolute or relative to the painted area.
    pub fn set_mapping_mode(&self, mode: MappingMode) {
        let brush: bindings::ICompositionGradientBrush2 = self.0.cast().unwrap();
        brush.SetMappingMode(mode.into()).unwrap();
    }

    /// Replaces the ramp with `stops` of `(offset, alpha)`, each in `0.0..=1.0`.
    ///
    /// Every stop is white at the given alpha, because this brush exists to supply
    /// coverage: the colour is the other half of the mask brush. Alpha is quantized to
    /// 8 bits by the composition `Color` ABI, so a narrow ramp — a fade from `0.10` to
    /// `0.14`, say — resolves to a handful of distinct steps and bands. Normalize such a
    /// ramp to full range and scale the *source* brush instead.
    pub fn set_alpha_stops(&self, stops: &[(f32, f32)]) {
        let compositor = self.compositor();
        let brush: bindings::ICompositionGradientBrush = self.0.cast().unwrap();
        let collection: windows_collections::IVector<bindings::CompositionColorGradientStop> =
            brush.ColorStops().unwrap().cast().unwrap();
        collection.Clear().unwrap();
        for &(offset, alpha) in stops {
            let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            let color = Color::rgba(255, 255, 255, alpha);
            let stop = compositor
                .CreateColorGradientStopWithOffsetAndColor(offset, color.0)
                .unwrap();
            collection.Append(&stop).unwrap();
        }
    }

    /// The compositor that created this brush, reached through the object base rather
    /// than passed in by the caller.
    fn compositor(&self) -> bindings::ICompositor4 {
        let object: bindings::ICompositionObject = self.0.cast().unwrap();
        object.Compositor().unwrap().cast().unwrap()
    }
}

impl Sealed for CompositionLinearGradientBrush {}

impl Brush for CompositionLinearGradientBrush {
    fn as_brush(&self) -> CompositionBrush {
        CompositionBrush(self.0.cast().unwrap())
    }
}

impl Sealed for CompositionBrush {}

/// The base type is itself a brush, so a caller that has erased which kind it built — a
/// solid source, a mask, a gradient, a surface — can still bind it into either half of a
/// mask brush.
impl Brush for CompositionBrush {
    fn as_brush(&self) -> CompositionBrush {
        self.clone()
    }
}

impl CompositionNineGridBrush {
    /// Scales the nine-grid's insets, so one brush serves several DPI scales without
    /// its insets being recomputed per scale.
    pub fn set_inset_scale(&self, scale: f32) {
        self.0.SetInsetScales(scale).unwrap();
    }
}

impl Compositor {
    /// Creates a mask brush, with neither half assigned yet.
    pub fn create_mask_brush(&self) -> CompositionMaskBrush {
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        CompositionMaskBrush(compositor.CreateMaskBrush().unwrap())
    }

    /// Creates a linear gradient brush with no stops — see
    /// [`set_alpha_stops`](CompositionLinearGradientBrush::set_alpha_stops).
    pub fn create_linear_gradient_brush(&self) -> CompositionLinearGradientBrush {
        let compositor: bindings::ICompositor4 = self.0.cast().unwrap();
        CompositionLinearGradientBrush(compositor.CreateLinearGradientBrush().unwrap())
    }
}
