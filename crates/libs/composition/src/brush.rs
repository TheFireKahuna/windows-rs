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

impl Compositor {
    /// Creates a brush that paints one brush through the alpha of another.
    pub fn create_mask_brush(&self) -> CompositionMaskBrush {
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        CompositionMaskBrush(compositor.CreateMaskBrush().unwrap())
    }
}
