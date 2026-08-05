//! Target primaries, and hue-preserving compression into them.
//!
//! Windows does not gamut-map for an application. Auto colour management is a colour
//! space conversion and a 1D LUT, and colours outside the display's gamut are
//! numerically clipped per channel, which drags hue with them: a saturated red that
//! clips reads orange. Compression here runs before that clip.
//!
//! Containment is a question about chromaticity alone. A channel above the display's
//! ceiling is a luminance statement, and it is spent by the tone stage in
//! [`crate::OutputTransform`].

use crate::ictcp::{self, Ictcp, M_LMS_TO_2020};
use crate::matrix::{self, D65, Mat3, Mat3f, inv, mul, narrow, rgb_to_xyz};

/// Linear Rec.2020 -> XYZ, the bridge from the working space to any set of primaries.
const M_2020_TO_XYZ: Mat3 = rgb_to_xyz((0.708, 0.292), (0.170, 0.797), (0.131, 0.046), D65);

/// Slack on the negative test, in cd/m². The chain runs through two matrices and a PQ
/// round trip, so a colour exactly on a primary can land a hair below zero; without
/// the slack it would be compressed as though it were out of gamut.
const EPS: f32 = 1e-4;

/// Bisection steps. The reproducible set is a contiguous interval in the chroma scale,
/// because chroma zero is the neutral axis and is always reproducible, so plain
/// bisection converges; 18 steps put the bracket below `f32` chroma resolution.
const BISECT_STEPS: u32 = 18;

/// A set of display primaries.
///
/// Built from chromaticities, which the standard targets here do at compile time; a
/// real panel takes one construction from `AdvancedColorInfo`'s red, green and blue
/// primaries and its white point. Both matrices are precomposed, so a containment
/// probe costs one matrix apply and a PQ decode.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Gamut {
    /// LMS -> this gamut's linear RGB.
    from_lms: Mat3f,
    /// Linear Rec.2020 -> this gamut's linear RGB.
    from_2020: Mat3f,
}

impl Gamut {
    /// Builds a gamut from primary and white chromaticities `(x, y)`.
    #[must_use]
    pub const fn from_primaries(
        r: (f64, f64),
        g: (f64, f64),
        b: (f64, f64),
        w: (f64, f64),
    ) -> Self {
        let from_xyz = inv(rgb_to_xyz(r, g, b, w));
        let from_2020 = mul(from_xyz, M_2020_TO_XYZ);
        Self {
            from_lms: narrow(mul(from_2020, M_LMS_TO_2020)),
            from_2020: narrow(from_2020),
        }
    }

    /// BT.709 / sRGB. The target on a desktop with no Advanced Color, where nothing
    /// colour-manages and an out-of-gamut channel clips.
    pub const REC709: Self =
        Self::from_primaries((0.640, 0.330), (0.300, 0.600), (0.150, 0.060), D65);

    /// Display P3, the common wide-gamut panel target.
    pub const DISPLAY_P3: Self =
        Self::from_primaries((0.680, 0.320), (0.265, 0.690), (0.150, 0.060), D65);

    /// BT.2020 / BT.2100, the widest standard container and this crate's working
    /// space. Compression into it returns any colour authored in Rec.2020 unchanged.
    pub const REC2020: Self =
        Self::from_primaries((0.708, 0.292), (0.170, 0.797), (0.131, 0.046), D65);

    /// Returns the linear Rec.2020 -> this gamut's linear RGB matrix.
    #[must_use]
    pub(crate) const fn matrix_from_2020(&self) -> Mat3f {
        self.from_2020
    }

    /// Returns `c` in this gamut's linear RGB, in cd/m². A **negative** component
    /// means the chromaticity is outside these primaries.
    #[must_use]
    pub fn rgb(&self, c: Ictcp) -> [f32; 3] {
        matrix::apply(&self.from_lms, ictcp::decode_lms(c))
    }

    /// Returns whether these primaries can reproduce `c`'s chromaticity. A luminance
    /// above the display's ceiling is in gamut and is left to the tone stage; this
    /// tests reachability of the chromaticity alone, to within a tolerance that
    /// absorbs the rounding of the matrix and PQ chain.
    #[must_use]
    pub fn contains(&self, c: Ictcp) -> bool {
        self.rgb(c).iter().all(|&x| x >= -EPS)
    }

    /// Compresses `c` into these primaries: holds `I` and hue **exactly** and shrinks
    /// chroma until the chromaticity is reproducible. A colour already inside is
    /// returned unchanged.
    ///
    /// `Ct` and `Cp` scale by one common factor, so `atan2(Cp, Ct)` is preserved
    /// exactly and only chroma magnitude moves. The result sits farther from `c` in
    /// ΔE-ITP than a per-channel clip would, since a clip lands on the nearest
    /// reproducible colour by construction; what the clip moves instead is hue.
    ///
    /// A colour outside the primaries costs 18 bisection steps, each one matrix apply
    /// and a PQ decode.
    #[must_use]
    pub fn compress(&self, c: Ictcp) -> Ictcp {
        if self.contains(c) {
            return c;
        }
        // `lo` is a reproducible chroma scale — zero is the neutral axis, which every
        // set of primaries reaches — and `hi` is an unreproducible one. The loop moves
        // whichever bound the probe belongs to, so both stay true.
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..BISECT_STEPS {
            let mid = 0.5 * (lo + hi);
            if self.contains(c.scale_chroma(mid)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        c.scale_chroma(lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A saturated Rec.2020 green at ~100 nits, comfortably outside Rec.709.
    fn wide_green() -> Ictcp {
        ictcp::from_2020([0.0, 100.0, 0.0])
    }

    /// The shared fixture sits outside Rec.709 and inside Rec.2020.
    #[test]
    fn fixture_is_genuinely_out_of_gamut() {
        let g = wide_green();
        assert!(
            !Gamut::REC709.contains(g),
            "in 709: {:?}",
            Gamut::REC709.rgb(g)
        );
        assert!(Gamut::REC2020.contains(g), "should be inside 2020");
    }

    #[test]
    fn compression_lands_in_gamut() {
        let m = Gamut::REC709.compress(wide_green());
        assert!(
            Gamut::REC709.contains(m),
            "still out: {:?}",
            Gamut::REC709.rgb(m)
        );
    }

    #[test]
    fn holds_hue_and_intensity_shrinks_only_chroma() {
        let src = wide_green();
        let m = Gamut::REC709.compress(src);
        assert_eq!(m.i, src.i, "intensity moved");
        assert!(
            (m.hue() - src.hue()).abs() < 1e-3,
            "hue moved: {} -> {}",
            src.hue(),
            m.hue()
        );
        assert!(m.chroma() < src.chroma(), "chroma did not shrink");
        assert!(m.chroma() > 0.0, "chroma collapsed to neutral");
    }

    /// Compression reads the target's primaries rather than applying a fixed squeeze.
    #[test]
    fn wider_target_keeps_more_chroma() {
        let src = wide_green();
        let c709 = Gamut::REC709.compress(src).chroma();
        let cp3 = Gamut::DISPLAY_P3.compress(src).chroma();
        let c2020 = Gamut::REC2020.compress(src).chroma();
        assert!(c709 < cp3, "P3 should keep more than 709: {c709} vs {cp3}");
        assert!(
            cp3 < c2020,
            "2020 should keep more than P3: {cp3} vs {c2020}"
        );
        assert_eq!(
            Gamut::REC2020.compress(src),
            src,
            "in-gamut must be identity"
        );
    }

    /// A per-channel clip is closer to the source in ΔE-ITP than compression is, and
    /// moves hue by more than a degree where compression holds it.
    #[test]
    fn trades_delta_e_for_hue_fidelity() {
        let src = wide_green();
        let mapped = Gamut::REC709.compress(src);

        // What a per-channel clip does: negatives pinned to zero, hue rotated toward
        // whatever survived.
        let clipped_2020 = {
            let rgb709 = Gamut::REC709.rgb(src).map(|x| x.max(0.0));
            // Back through 709's own primaries into the working space.
            let xyz = matrix::apply(
                &narrow(rgb_to_xyz(
                    (0.640, 0.330),
                    (0.300, 0.600),
                    (0.150, 0.060),
                    D65,
                )),
                rgb709,
            );
            matrix::apply(&narrow(inv(M_2020_TO_XYZ)), xyz)
        };
        let clipped = ictcp::from_2020(clipped_2020);

        assert!(
            clipped.delta_itp(src) < mapped.delta_itp(src),
            "a clip is ΔE-minimal by construction; got clip {} vs map {}",
            clipped.delta_itp(src),
            mapped.delta_itp(src)
        );
        assert!(
            (mapped.hue() - src.hue()).abs() < 1e-3,
            "the map must hold hue exactly"
        );
        assert!(
            (clipped.hue() - src.hue()).abs() > 1.0,
            "the clip should visibly rotate hue, moved {}",
            (clipped.hue() - src.hue()).abs()
        );
    }

    /// Containment is chromaticity only: a colour above the reference anchor is in
    /// gamut and passes through untouched, so the gamut stage spends no luminance.
    #[test]
    fn above_white_is_in_gamut_and_untouched() {
        let c = Ictcp::polar(380.0, 0.05, 15.0);
        assert!(
            c.to_radiance(1.0).peak_nits() > crate::REFERENCE_WHITE_NITS,
            "fixture is not above white"
        );
        assert!(Gamut::REC709.contains(c), "a specular must be in gamut");
        assert_eq!(
            Gamut::REC709.compress(c),
            c,
            "the gamut stage must not touch luminance"
        );
    }

    #[test]
    fn neutral_axis_is_always_reachable() {
        for nits in [0.0f32, 2.0, 40.0, 203.0, 438.0] {
            let c = Ictcp::polar(nits, 0.0, 0.0);
            assert!(Gamut::REC709.contains(c), "neutral at {nits} out of gamut");
            assert_eq!(Gamut::REC709.compress(c), c);
        }
    }
}
