#![doc = include_str!("../readme.md")]
#![forbid(unsafe_code)]

mod gamut;
mod ictcp;
mod matrix;
mod output;
mod pq;

pub use gamut::Gamut;
pub use ictcp::Ictcp;
pub use output::{DisplayCapability, OutputTransform};

/// scRGB's definition: `1.0` is 80 cd/m². A fact about the encoding, not a policy.
pub const SCRGB_UNITY_NITS: f32 = 80.0;

/// BT.2408 HDR graphics ("paper") white — the luminance a user interface's diffuse
/// white is authored at, and the anchor the scene-referred exposure tracks.
pub const REFERENCE_WHITE_NITS: f32 = 203.0;

/// Scene-referred light: linear **Rec.2020** primaries, absolute **cd/m²**, unbounded
/// above and, for chromaticities outside Rec.2020, below zero.
///
/// Everything a user interface computes happens here — mixing, coverage, gradients,
/// washes, the accent ladder. Nothing above the display transform has met a display.
///
/// The working space is Rec.2020 because BT.2100 defines its LMS matrix *from*
/// Rec.2020, which makes the conversion to and from [`Ictcp`] the Recommendation's
/// chain verbatim. It also means `max(r, g, b)` is a true peak — everything a UI
/// authors is inside Rec.2020 and so has no negative component — and that "out of
/// gamut" becomes a property of the *display*, decided inside
/// [`OutputTransform::apply`], rather than a property of how the value happened to be
/// encoded.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Radiance {
    /// Red, in cd/m².
    pub r: f32,
    /// Green, in cd/m².
    pub g: f32,
    /// Blue, in cd/m².
    pub b: f32,
    /// Alpha, linear `0..=1`. Straight, not premultiplied.
    pub a: f32,
}

impl Radiance {
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// Construct from channels in cd/m².
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// The same light at a different alpha.
    #[must_use]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// The brightest channel any display will be asked to present, in cd/m².
    ///
    /// This is what an application maximises over its resolved palette to declare its
    /// content peak to [`OutputTransform::for_display`]. Two things about it are easy
    /// to get wrong, and both cost real pixels:
    ///
    /// It is **not** the colour's ICtCp intensity. A saturated colour carries a
    /// channel well above the achromatic luminance it was authored at, and what a
    /// display's container bounds is channels.
    ///
    /// It is taken in **Rec.709**, not in this type's own Rec.2020 basis, because the
    /// working-space peak does not bound the presented one: the matrix into a
    /// display's primaries has negative off-diagonal terms, so a saturated colour's
    /// presented channel runs above its working one. Rec.709 is the narrowest standard
    /// container, so a peak taken there bounds every wider display too — which makes
    /// this conservative in the safe direction. Over-declaring costs a little
    /// unnecessary compression; under-declaring lets the compositor clip.
    #[must_use]
    pub fn peak_nits(self) -> f32 {
        let v = matrix::apply(&Gamut::REC709.matrix_from_2020(), [self.r, self.g, self.b]);
        v[0].max(v[1]).max(v[2])
    }
}

/// Display-referred output: linear **Rec.709** primaries on the scRGB scale, which is
/// what Direct2D and the Windows compositor accept.
///
/// Reachable from exactly one place — [`OutputTransform::apply`] — and never converted
/// back. That is what makes "the display transform runs exactly once per colour" a
/// property of the type system rather than of a convention someone has to remember:
/// applying it twice is unrepresentable, and applying it zero times fails to compile
/// at the point a brush is built.
///
/// Values outside `[0, 1]` are correct and are the point. A negative component carries
/// a colour outside Rec.709 — which is how scRGB represents a wide gamut, not an
/// overflow — and a component above `1.0` carries luminance above the display's white.
/// Nothing here clamps.
///
/// `#[repr(C)]` with RGBA `f32` order, so it is layout-compatible with
/// `D2D1_COLOR_F`.
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Scrgb {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Alpha, linear `0..=1`. Straight, not premultiplied.
    pub a: f32,
}

impl Scrgb {
    /// Fully transparent. Zero is zero under any transform, so this needs no display.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Encode for an 8-bit file — a screenshot, or a reference image for a rendering
    /// parity check.
    ///
    /// This encodes an **already-transformed** value, treating `1.0` as white, and it
    /// clamps because 8-bit sRGB is a bounded container. There is deliberately **no
    /// inverse** anywhere in this crate: nothing produces an [`Ictcp`] or a
    /// [`Radiance`] from encoded sRGB, so no colour can be authored from an 8-bit
    /// anchor and quietly make a display-referred artefact the reference the rest of
    /// the pipeline is fitted to.
    #[must_use]
    pub fn to_srgb8(self) -> [u8; 4] {
        fn oetf(c: f32) -> u8 {
            let c = c.clamp(0.0, 1.0);
            let e = if c <= 0.003_130_8 {
                12.92 * c
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            };
            (e * 255.0).round() as u8
        }
        [
            oetf(self.r),
            oetf(self.g),
            oetf(self.b),
            (self.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `windows-d2d` reinterprets this as a `D2D1_COLOR_F`, so the layout is load
    /// bearing rather than incidental.
    #[test]
    fn scrgb_layout_is_four_contiguous_f32_in_rgba_order() {
        assert_eq!(size_of::<Scrgb>(), 16);
        assert_eq!(align_of::<Scrgb>(), align_of::<f32>());
        assert_eq!(core::mem::offset_of!(Scrgb, r), 0);
        assert_eq!(core::mem::offset_of!(Scrgb, g), 4);
        assert_eq!(core::mem::offset_of!(Scrgb, b), 8);
        assert_eq!(core::mem::offset_of!(Scrgb, a), 12);
    }

    #[test]
    fn srgb8_encodes_the_expected_anchors() {
        assert_eq!(Scrgb::TRANSPARENT.to_srgb8(), [0, 0, 0, 0]);
        let white = Scrgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        assert_eq!(white.to_srgb8(), [255, 255, 255, 255]);
        // Above white and outside Rec.709 both clamp at this boundary, and only here.
        let wild = Scrgb {
            r: -0.4,
            g: 3.0,
            b: 0.5,
            a: 1.0,
        };
        let out = wild.to_srgb8();
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 255);
        assert!(
            out[2] > 180 && out[2] < 200,
            "mid grey encoded to {}",
            out[2]
        );
    }
}
