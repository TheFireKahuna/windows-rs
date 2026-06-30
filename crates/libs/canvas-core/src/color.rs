use super::*;

/// RGBA color with f32 components.
#[doc(alias = "Color")]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ColorF {
    /// Red component, in the range 0.0–1.0.
    pub r: f32,
    /// Green component, in the range 0.0–1.0.
    pub g: f32,
    /// Blue component, in the range 0.0–1.0.
    pub b: f32,
    /// Alpha (opacity) component, in the range 0.0–1.0.
    pub a: f32,
}

impl ColorF {
    /// Creates a color from red, green, blue, and alpha components.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates an opaque color from red, green, and blue components.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Creates a color from 8-bit red, green, blue, and alpha components.
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    /// Creates an opaque color from 8-bit red, green, and blue components.
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba8(r, g, b, 255)
    }

    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    /// Opaque black.
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    /// Opaque white.
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    /// Opaque red.
    pub const RED: Self = Self::rgb(1.0, 0.0, 0.0);
    /// Opaque green.
    pub const GREEN: Self = Self::rgb(0.0, 1.0, 0.0);
    /// Opaque blue.
    pub const BLUE: Self = Self::rgb(0.0, 0.0, 1.0);
    /// Opaque cornflower blue.
    pub const CORNFLOWER_BLUE: Self = Self::rgb(0.392, 0.584, 0.929);
    /// Opaque dark slate blue.
    pub const DARK_SLATE_BLUE: Self = Self::rgb(0.05, 0.05, 0.1);

    /// Re-interpret this sRGB-encoded color as linear scRGB (the standard
    /// piecewise sRGB EOTF applied per RGB channel; alpha is left untouched).
    ///
    /// Colors throughout the UI are authored in sRGB. An 8-bit `B8G8R8A8` surface
    /// stores sRGB directly, so they need no conversion there. An FP16
    /// `R16G16B16A16Float` (scRGB) surface, however, stores *linear* values and is
    /// composited to the display without a gamma encode — so an sRGB value written
    /// raw renders far too bright. Converting here lands the authored color at the
    /// right luminance on a linear surface (and lets a value exceed 1.0 to reach
    /// HDR headroom). The single decode used by every linearizing draw path.
    pub fn to_linear(self) -> Self {
        Self {
            r: srgb_to_linear(self.r),
            g: srgb_to_linear(self.g),
            b: srgb_to_linear(self.b),
            a: self.a,
        }
    }
}

/// One sRGB channel (0.0–1.0, gamma-encoded) to linear. The standard piecewise
/// sRGB EOTF.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

impl From<ColorF> for D2D1_COLOR_F {
    fn from(c: ColorF) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}
