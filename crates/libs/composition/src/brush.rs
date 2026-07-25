use super::*;

/// The base type shared by every composition brush. A [`Brush`] can be turned
/// into one via [`Brush::as_brush`] to paint a visual or fill a shape.
#[derive(Clone)]
pub struct CompositionBrush(pub(crate) bindings::CompositionBrush);

/// A brush that can paint a [`SpriteVisual`](crate::SpriteVisual), fill a
/// [`CompositionSpriteShape`](crate::CompositionSpriteShape), or serve as the
/// source of a [`CompositionNineGridBrush`].
///
/// This trait is sealed: only the brush types in this crate implement it.
pub trait Brush: Sealed {
    /// Returns this brush as the shared [`CompositionBrush`] base type.
    fn as_brush(&self) -> CompositionBrush;
}

/// A brush that paints with a single solid color.
#[derive(Clone)]
pub struct CompositionColorBrush(pub(crate) bindings::CompositionColorBrush);

impl CompositionColorBrush {
    /// Sets the brush's color.
    pub fn set_color(&self, color: Color) {
        self.0.SetColor(color.0).unwrap();
    }

    /// Returns the brush's color.
    pub fn color(&self) -> Color {
        Color(self.0.Color().unwrap())
    }
}

impl Sealed for CompositionColorBrush {}

impl Brush for CompositionColorBrush {
    fn as_brush(&self) -> CompositionBrush {
        CompositionBrush(self.0.cast().unwrap())
    }
}

/// A brush that stretches its source as a nine-grid: the four corners keep their
/// size, the edges stretch along one axis, and the center stretches (or is left
/// [hollow](Self::set_center_hollow)) to fill the remaining space. Used here to
/// draw a hollow selection border.
#[derive(Clone)]
pub struct CompositionNineGridBrush(pub(crate) bindings::CompositionNineGridBrush);

impl CompositionNineGridBrush {
    /// Sets the left, top, right, and bottom inset widths, in DIPs.
    pub fn set_insets(&self, left: f32, top: f32, right: f32, bottom: f32) {
        self.0
            .SetInsetsWithValues(left, top, right, bottom)
            .unwrap();
    }

    /// Sets a single scale factor applied to all four [insets](Self::set_insets).
    ///
    /// The insets are expressed in the source surface's pixels, but the brush
    /// stretches the grid across a destination measured in DIPs. When the source
    /// is rasterized at a display scale other than `1.0`, an inset of `r` DIPs is
    /// `r * scale` pixels in the source, and this factor is what maps it back:
    /// without it the corners of a nine-grid are sampled at the wrong radius and
    /// smear as the bar stretches.
    pub fn set_inset_scale(&self, scale: f32) {
        self.0.SetInsetScales(scale).unwrap();
    }

    /// Sets whether the center of the grid is left unpainted (hollow).
    pub fn set_center_hollow(&self, hollow: bool) {
        self.0.SetIsCenterHollow(hollow).unwrap();
    }

    /// Sets the brush stretched across the nine-grid.
    pub fn set_source(&self, brush: &impl Brush) {
        self.0.SetSource(&brush.as_brush().0).unwrap();
    }
}

impl Sealed for CompositionNineGridBrush {}

impl Brush for CompositionNineGridBrush {
    fn as_brush(&self) -> CompositionBrush {
        CompositionBrush(self.0.cast().unwrap())
    }
}

/// A brush that paints its [source](Self::set_source) through the alpha channel
/// of its [mask](Self::set_mask): the source supplies the color, the mask
/// supplies the coverage.
///
/// The split matters because the two halves are not interchangeable. Coverage is
/// a single channel that a rasterizer can produce cheaply, while color may be
/// wide-gamut — outside what a [`CompositionColorBrush`] can express, since that
/// carries an 8-bit-per-channel color. Painting a shape in a color the color
/// brush cannot hold therefore means rasterizing the shape's *coverage* into the
/// mask and putting the color in the source, rather than rasterizing a colored
/// shape directly.
#[derive(Clone)]
pub struct CompositionMaskBrush(pub(crate) bindings::CompositionMaskBrush);

impl CompositionMaskBrush {
    /// Sets the brush whose alpha channel is used as the coverage mask.
    pub fn set_mask(&self, brush: &impl Brush) {
        self.0.SetMask(&brush.as_brush().0).unwrap();
    }

    /// Sets the brush supplying the color that the mask reveals.
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

impl PartialEq for CompositionMaskBrush {
    fn eq(&self, other: &Self) -> bool {
        canonical(&self.0) == canonical(&other.0)
    }
}

impl Eq for CompositionMaskBrush {}

/// Where a gradient's start and end points are measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MappingMode {
    /// Points are DIPs in the painted visual's own space.
    Absolute,
    /// Points are fractions of the painted visual's size, so the ramp follows a
    /// resize with no property write at all.
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

/// A brush that interpolates linearly between colour stops along a line.
///
/// **Not a colour carrier.** A stop is a `Windows.UI.Color` — 8 bits a channel,
/// `0..1` — so a gradient bound as a visual's or a shape's own brush clamps at
/// paper white on a wide-gamut display, and distorts the transfer function on
/// the way. Bound as a [`CompositionMaskBrush::set_mask`] it is a different
/// thing entirely: the compositor evaluates it per pixel and reads only its
/// ALPHA, which is the one quantity `0..1` describes exactly, while colour stays
/// in an app-allocated wide-gamut source. That is the only use this wrapper is
/// for, and why the stops it takes are alphas.
///
/// A radial gradient is deliberately absent: as a mask it binds, throws nothing,
/// and silently paints itself as colour instead of masking.
#[derive(Clone)]
pub struct CompositionLinearGradientBrush(pub(crate) bindings::CompositionLinearGradientBrush);

impl CompositionLinearGradientBrush {
    /// Sets the line the ramp runs along, in the units [`Self::set_mapping_mode`]
    /// selects.
    pub fn set_line(&self, start: Vector2, end: Vector2) {
        let brush: bindings::ICompositionLinearGradientBrush = self.0.cast().unwrap();
        brush.SetStartPoint(start).unwrap();
        brush.SetEndPoint(end).unwrap();
    }

    /// Sets whether [`Self::set_line`]'s points are DIPs or fractions of the
    /// painted visual.
    pub fn set_mapping_mode(&self, mode: MappingMode) {
        let brush: bindings::ICompositionGradientBrush2 = self.0.cast().unwrap();
        brush.SetMappingMode(mode.into()).unwrap();
    }

    /// Replaces the stop list with `stops`, each an `(offset, alpha)` pair.
    ///
    /// The colour written is white, since only the alpha is ever read — see the
    /// type's own documentation.
    pub fn set_alpha_stops(&self, compositor: &Compositor, stops: &[(f32, f32)]) {
        let factory: bindings::ICompositor4 = compositor.0.cast().unwrap();
        let brush: bindings::ICompositionGradientBrush = self.0.cast().unwrap();
        let collection: windows_collections::IVector<bindings::CompositionColorGradientStop> =
            brush.ColorStops().unwrap().cast().unwrap();
        collection.Clear().unwrap();
        for &(offset, alpha) in stops {
            let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            let stop = factory
                .CreateColorGradientStopWithOffsetAndColor(
                    offset,
                    Color::rgba(255, 255, 255, a).0,
                )
                .unwrap();
            collection.Append(&stop).unwrap();
        }
    }
}

impl Sealed for CompositionLinearGradientBrush {}

impl Brush for CompositionLinearGradientBrush {
    fn as_brush(&self) -> CompositionBrush {
        CompositionBrush(self.0.cast().unwrap())
    }
}

impl Sealed for CompositionBrush {}

/// The base type is itself a brush, so a caller that has erased which kind of
/// brush it built — a solid source, a mask, a gradient — can still bind it.
impl Brush for CompositionBrush {
    fn as_brush(&self) -> CompositionBrush {
        self.clone()
    }
}

impl Compositor {
    /// Creates a brush that paints one brush through the alpha of another.
    pub fn create_mask_brush(&self) -> CompositionMaskBrush {
        bump_count(Count::Brush);
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        CompositionMaskBrush(compositor.CreateMaskBrush().unwrap())
    }

    /// Creates a linear gradient brush, for use as a mask's alpha ramp.
    pub fn create_linear_gradient_brush(&self) -> CompositionLinearGradientBrush {
        bump_count(Count::Brush);
        let compositor: bindings::ICompositor4 = self.0.cast().unwrap();
        CompositionLinearGradientBrush(compositor.CreateLinearGradientBrush().unwrap())
    }
}
