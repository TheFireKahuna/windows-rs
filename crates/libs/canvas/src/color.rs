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
    /// Creates a color from **linear** scRGB red, green, blue, and alpha components
    /// (extended range — channels may be `< 0` or `> 1`). Every `ColorF` handed to a
    /// draw session is linear; this is the raw constructor.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// A raw **linear** scRGB color (extended range) — the explicit-intent alias of
    /// [`new`](Self::new), mirroring the reactor `Color::scrgb` constructor so both
    /// color currencies read the same at call sites. Alpha is `1.0` unless given.
    pub const fn scrgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates an opaque color from **linear** scRGB red, green, and blue components.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Creates a **linear** color from 8-bit **sRGB** red, green, blue, and alpha
    /// components — the RGB channels are gamma-decoded (sRGB EOTF) so a legacy hex
    /// literal lands in linear light; alpha is a straight `a/255` (already linear).
    ///
    /// Every `ColorF` past this point is linear scRGB: the FP16 composition surface
    /// consumes it raw and an 8-bit sRGB surface re-encodes it at its boundary (see
    /// [`DrawingSession::encode_srgb_target`]). The decode is a `const` table lookup
    /// so this stays usable in `const` token definitions.
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: SRGB8_TO_LINEAR[r as usize],
            g: SRGB8_TO_LINEAR[g as usize],
            b: SRGB8_TO_LINEAR[b as usize],
            a: a as f32 / 255.0,
        }
    }

    /// Creates an opaque **linear** color from 8-bit **sRGB** red, green, and blue
    /// components (the RGB channels are gamma-decoded; see [`Self::from_rgba8`]).
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

    /// Encode this **linear** scRGB color to **sRGB** for an 8-bit (`B8G8R8A8`)
    /// output surface: clamp the extended-range linear channels to `[0, 1]` (the SDR
    /// gamut) and apply the sRGB OETF; alpha is clamped but left linear. This is the
    /// inverse of [`Self::to_linear`] and the encode an 8-bit
    /// [`DrawingSession`](crate::DrawingSession) applies at its boundary — a linear
    /// value written straight to a UNORM sRGB surface would read far too dark, so it
    /// is gamma-encoded here. HDR headroom (`> 1.0`) clamps gracefully to white.
    pub fn to_srgb(self) -> Self {
        Self {
            r: linear_to_srgb(self.r.clamp(0.0, 1.0)),
            g: linear_to_srgb(self.g.clamp(0.0, 1.0)),
            b: linear_to_srgb(self.b.clamp(0.0, 1.0)),
            a: self.a.clamp(0.0, 1.0),
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

/// One **linear** channel (0.0–1.0) to sRGB. The standard piecewise sRGB OETF —
/// the inverse of [`srgb_to_linear`].
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// 8-bit sRGB → linear, one entry per input byte. Precomputed (the sRGB EOTF uses
/// `powf`, which is not `const`) so [`ColorF::from_rgba8`] and reactor `Color`
/// tokens can gamma-decode in a `const` context. `SRGB8_TO_LINEAR[v]` equals
/// `srgb_to_linear(v as f32 / 255.0)`.
#[rustfmt::skip]
pub(crate) const SRGB8_TO_LINEAR: [f32; 256] = [
    0.0, 0.000303527, 0.000607054, 0.000910581, 0.001214108, 0.001517635,
    0.001821162, 0.0021246888, 0.002428216, 0.0027317428, 0.00303527, 0.0033465358,
    0.0036765074, 0.004024717, 0.004391442, 0.0047769533, 0.0051815165, 0.0056053917,
    0.006048833, 0.0065120906, 0.00699541, 0.007499032, 0.008023193, 0.008568126,
    0.009134059, 0.009721218, 0.010329823, 0.010960094, 0.011612245, 0.012286488,
    0.0129830325, 0.013702083, 0.014443844, 0.015208514, 0.015996294, 0.016807375,
    0.017641954, 0.01850022, 0.019382361, 0.020288562, 0.02121901, 0.022173885,
    0.023153367, 0.024157632, 0.02518686, 0.026241222, 0.027320892, 0.02842604,
    0.029556835, 0.030713445, 0.031896032, 0.033104766, 0.034339808, 0.035601314,
    0.03688945, 0.038204372, 0.039546236, 0.0409152, 0.04231141, 0.04373503,
    0.045186203, 0.046665087, 0.048171826, 0.049706567, 0.051269457, 0.052860647,
    0.054480277, 0.05612849, 0.05780543, 0.059511237, 0.061246052, 0.063010015,
    0.064803265, 0.06662594, 0.06847817, 0.070360094, 0.07227185, 0.07421357,
    0.07618538, 0.07818742, 0.08021982, 0.08228271, 0.08437621, 0.08650046,
    0.08865558, 0.09084171, 0.093058966, 0.09530747, 0.09758735, 0.099898726,
    0.10224173, 0.104616486, 0.107023105, 0.10946171, 0.11193243, 0.114435375,
    0.116970666, 0.11953843, 0.122138776, 0.12477182, 0.12743768, 0.13013647,
    0.13286832, 0.13563333, 0.13843161, 0.14126329, 0.14412847, 0.14702727,
    0.14995979, 0.15292615, 0.15592647, 0.15896083, 0.16202937, 0.1651322,
    0.1682694, 0.17144111, 0.1746474, 0.17788842, 0.18116425, 0.18447499,
    0.18782078, 0.19120169, 0.19461784, 0.19806932, 0.20155625, 0.20507874,
    0.20863687, 0.21223076, 0.2158605, 0.2195262, 0.22322796, 0.22696587,
    0.23074006, 0.23455058, 0.23839757, 0.24228112, 0.24620132, 0.25015828,
    0.2541521, 0.25818285, 0.26225066, 0.2663556, 0.2704978, 0.2746773,
    0.27889428, 0.28314874, 0.28744084, 0.29177064, 0.29613826, 0.30054379,
    0.3049873, 0.30946892, 0.31398872, 0.31854677, 0.3231432, 0.3277781,
    0.33245152, 0.33716363, 0.34191442, 0.34670407, 0.3515326, 0.35640013,
    0.3613068, 0.3662526, 0.3712377, 0.37626213, 0.38132602, 0.38642943,
    0.39157248, 0.39675522, 0.40197778, 0.4072402, 0.4125426, 0.41788507,
    0.42326766, 0.4286905, 0.43415365, 0.43965718, 0.4452012, 0.4507858,
    0.45641103, 0.462077, 0.4677838, 0.47353148, 0.47932017, 0.48514995,
    0.49102086, 0.49693298, 0.5028865, 0.50888133, 0.5149177, 0.52099556,
    0.5271151, 0.5332764, 0.5394795, 0.54572445, 0.55201143, 0.5583404,
    0.5647115, 0.57112485, 0.57758045, 0.58407843, 0.59061885, 0.59720176,
    0.60382736, 0.61049557, 0.6172066, 0.6239604, 0.63075715, 0.63759685,
    0.6444797, 0.65140563, 0.65837485, 0.6653873, 0.67244315, 0.6795425,
    0.6866853, 0.69387174, 0.7011019, 0.70837575, 0.7156935, 0.7230551,
    0.73046076, 0.7379104, 0.7454042, 0.7529422, 0.7605245, 0.76815116,
    0.7758222, 0.7835378, 0.7912979, 0.7991027, 0.80695224, 0.8148466,
    0.82278574, 0.8307699, 0.838799, 0.8468732, 0.8549926, 0.8631572,
    0.8713671, 0.8796224, 0.8879231, 0.8962694, 0.9046612, 0.91309863,
    0.92158186, 0.9301109, 0.9386857, 0.9473065, 0.9559733, 0.9646863,
    0.9734453, 0.9822506, 0.9911021, 1.0,
];

impl From<ColorF> for D2D_COLOR_F {
    fn from(c: ColorF) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}
