//! BT.2100 ICtCp and BT.2124 ΔE-ITP — the authoring space.
//!
//! BT.2100 defines its LMS matrix *from* Rec.2020, so with [`Radiance`] in Rec.2020
//! the conversion is the Recommendation's chain verbatim: one published `/4096`
//! matrix, PQ, one published `/4096` matrix, and two const inverses. Rec.709 does not
//! appear here at all.

use crate::matrix::{Mat3, Mat3f, inv, narrow};
use crate::{Radiance, pq};

/// BT.2100 linear Rec.2020 -> LMS. The Recommendation's exact `/4096` integers, with
/// crosstalk already folded in; the rows sum to exactly 1, so a neutral maps to
/// `L = M = S` and therefore to zero chroma.
pub(crate) const M_2020_TO_LMS: Mat3 = [
    [1688.0 / 4096.0, 2146.0 / 4096.0, 262.0 / 4096.0],
    [683.0 / 4096.0, 2951.0 / 4096.0, 462.0 / 4096.0],
    [99.0 / 4096.0, 309.0 / 4096.0, 3688.0 / 4096.0],
];

/// BT.2100 PQ-encoded L'M'S' -> ICtCp. Also exact `/4096` integers.
const M_LMS_TO_ICTCP: Mat3 = [
    [2048.0 / 4096.0, 2048.0 / 4096.0, 0.0],
    [6610.0 / 4096.0, -13613.0 / 4096.0, 7003.0 / 4096.0],
    [17933.0 / 4096.0, -17390.0 / 4096.0, -543.0 / 4096.0],
];

pub(crate) const M_LMS_TO_2020: Mat3 = inv(M_2020_TO_LMS);
pub(crate) const M_ICTCP_TO_LMS: Mat3 = inv(M_LMS_TO_ICTCP);

const F_2020_TO_LMS: Mat3f = narrow(M_2020_TO_LMS);
const F_LMS_TO_ICTCP: Mat3f = narrow(M_LMS_TO_ICTCP);
const F_LMS_TO_2020: Mat3f = narrow(M_LMS_TO_2020);
pub(crate) const F_ICTCP_TO_LMS: Mat3f = narrow(M_ICTCP_TO_LMS);

/// A BT.2100 ICtCp coordinate, against **absolute** luminance.
///
/// `i` is PQ-encoded achromatic intensity; `ct` and `cp` are the tritan (blue-yellow)
/// and protan (red-green) chroma axes. The space is hue-linear, which is what lets
/// gamut compression hold hue exactly while it moves chroma.
///
/// Never interpolate linear light here, and never interpolate this in linear light.
/// [`Ictcp::mix`] is the one interpolation primitive.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Ictcp {
    /// Intensity: PQ-encoded achromatic axis.
    pub i: f32,
    /// Tritan (blue-yellow) chroma axis.
    pub ct: f32,
    /// Protan (red-green) chroma axis.
    pub cp: f32,
}

impl Ictcp {
    /// Construct from raw axes. Prefer [`Ictcp::polar`] for authoring.
    #[must_use]
    pub const fn new(i: f32, ct: f32, cp: f32) -> Self {
        Self { i, ct, cp }
    }

    /// Author a colour: absolute luminance in cd/m², chroma magnitude, hue in
    /// degrees. The only constructor a palette needs.
    ///
    /// Luminance is absolute because the pipeline is scene-referred throughout. A
    /// ratio to some reference white is a display-referred habit in absolute
    /// clothing, and it would be a second luminance encoding to keep in step.
    #[must_use]
    pub fn polar(nits: f32, chroma: f32, hue_deg: f32) -> Self {
        let (sin, cos) = hue_deg.to_radians().sin_cos();
        Self::new(pq::encode(nits), chroma * cos, chroma * sin)
    }

    /// Chroma magnitude, `sqrt(Ct² + Cp²)`.
    #[must_use]
    pub fn chroma(self) -> f32 {
        self.ct.hypot(self.cp)
    }

    /// Hue angle in degrees, `atan2(Cp, Ct)`, normalised to `[0, 360)` so that it
    /// reads back what [`Ictcp::polar`] was given.
    #[must_use]
    pub fn hue(self) -> f32 {
        let h = self.cp.atan2(self.ct).to_degrees();
        if h < 0.0 { h + 360.0 } else { h }
    }

    /// The achromatic luminance this intensity encodes, in cd/m².
    #[must_use]
    pub fn nits(self) -> f32 {
        pq::decode(self.i)
    }

    /// The same hue and intensity at a different chroma. Scaling both axes by one
    /// factor preserves `atan2(Cp, Ct)` exactly, not approximately.
    #[must_use]
    pub fn with_chroma(self, chroma: f32) -> Self {
        let c = self.chroma();
        if c <= 0.0 {
            return self;
        }
        let s = chroma / c;
        Self::new(self.i, self.ct * s, self.cp * s)
    }

    /// The same chromaticity at a different luminance.
    #[must_use]
    pub fn with_nits(self, nits: f32) -> Self {
        Self::new(pq::encode(nits), self.ct, self.cp)
    }

    /// Scale chroma by a factor, holding hue and intensity. The gamut stage's step.
    #[must_use]
    pub(crate) fn scale_chroma(self, s: f32) -> Self {
        Self::new(self.i, self.ct * s, self.cp * s)
    }

    /// BT.2124 ΔE-ITP: perceptual difference, scaled so `1.0` is about one JND. ITP
    /// rescales the chroma axes (`T = 0.5 * Ct`) before the Euclidean distance.
    ///
    /// This is the legibility metric, and it is evaluated on authored values — never
    /// on post-transform panel values, which vary by display.
    #[must_use]
    pub fn delta_itp(self, other: Self) -> f32 {
        let di = self.i - other.i;
        let dt = 0.5 * (self.ct - other.ct);
        let dp = self.cp - other.cp;
        720.0 * (di * di + dt * dt + dp * dp).sqrt()
    }

    /// Perceptually even interpolation, componentwise in ICtCp.
    ///
    /// Endpoints of different hue interpolate through the achromatic axis, which is
    /// what "linear in an opponent space" means and is usually what a UI ramp wants.
    /// A ramp that should travel *around* the hue circle is expressed as extra stops,
    /// which is what this primitive is for.
    #[must_use]
    pub fn mix(a: Self, b: Self, t: f32) -> Self {
        let l = |x: f32, y: f32| x + (y - x) * t;
        Self::new(l(a.i, b.i), l(a.ct, b.ct), l(a.cp, b.cp))
    }

    /// Resolve to scene light at the given alpha.
    #[must_use]
    pub fn to_radiance(self, alpha: f32) -> Radiance {
        let [r, g, b] = to_2020(self);
        Radiance { r, g, b, a: alpha }
    }
}

impl Radiance {
    /// The authoring coordinate for this light. Alpha is dropped — ICtCp is a colour
    /// coordinate, not a composite.
    #[must_use]
    pub fn to_ictcp(self) -> Ictcp {
        from_2020([self.r, self.g, self.b])
    }
}

/// Linear Rec.2020 in cd/m² -> ICtCp.
pub(crate) fn from_2020(rgb: [f32; 3]) -> Ictcp {
    let lms = crate::matrix::apply(&F_2020_TO_LMS, rgb);
    let lms_p = [pq::encode(lms[0]), pq::encode(lms[1]), pq::encode(lms[2])];
    let [i, ct, cp] = crate::matrix::apply(&F_LMS_TO_ICTCP, lms_p);
    Ictcp::new(i, ct, cp)
}

/// ICtCp -> linear Rec.2020 in cd/m².
pub(crate) fn to_2020(c: Ictcp) -> [f32; 3] {
    let lms = decode_lms(c);
    crate::matrix::apply(&F_LMS_TO_2020, lms)
}

/// ICtCp -> LMS in cd/m². Shared with the gamut stage, which needs LMS so it can go
/// straight to an arbitrary target's primaries without a Rec.2020 detour.
#[inline]
pub(crate) fn decode_lms(c: Ictcp) -> [f32; 3] {
    let lms_p = crate::matrix::apply(&F_ICTCP_TO_LMS, [c.i, c.ct, c.cp]);
    [
        pq::decode(lms_p[0]),
        pq::decode(lms_p[1]),
        pq::decode(lms_p[2]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    /// The `/4096` rows sum to exactly 1, so a neutral must land on `L = M = S` and
    /// therefore on zero chroma. If a matrix entry is mistyped, this fails first.
    #[test]
    fn neutrals_have_zero_chroma() {
        for nits in [0.5f32, 2.0, 61.0, 203.0, 438.0, 1000.0] {
            let c = from_2020([nits, nits, nits]);
            assert!(close(c.i, pq::encode(nits), 1e-5), "I at {nits}");
            assert!(c.chroma() < 1e-5, "chroma at {nits}: {}", c.chroma());
        }
    }

    #[test]
    fn round_trips_including_wide_and_specular() {
        let samples: [[f32; 3]; 5] = [
            [4.3, 4.3, 4.3],     // a near-black surface
            [0.9, 76.4, 165.4],  // a saturated accent
            [175.2, 11.7, 11.7], // a status red
            [3.0, 200.0, 40.0],  // saturated green, outside Rec.709
            [438.0, 29.3, 29.3], // a specular above the 203-nit anchor
        ];
        for s in samples {
            let rt = to_2020(from_2020(s));
            // Tolerance against the vector's own magnitude, not per channel: the chain
            // is a matrix pair around a PQ round trip, so a small channel beside a
            // large one carries the large one's absolute error, and judging it against
            // its own value would be measuring the wrong thing.
            let mag = s[0].abs().max(s[1].abs()).max(s[2].abs());
            for k in 0..3 {
                assert!(
                    close(rt[k], s[k], mag * 1e-3 + 1e-4),
                    "round trip {s:?} -> {rt:?}"
                );
            }
        }
    }

    #[test]
    fn polar_is_the_inverse_of_the_accessors() {
        for (nits, chroma, hue) in [
            (3.7f32, 0.004f32, 250.0f32),
            (165.0, 0.11, 220.0),
            (438.0, 0.19, 15.0),
        ] {
            let c = Ictcp::polar(nits, chroma, hue);
            assert!(
                close(c.nits(), nits, nits * 1e-3),
                "nits {} vs {nits}",
                c.nits()
            );
            assert!(
                close(c.chroma(), chroma, 1e-6),
                "chroma {} vs {chroma}",
                c.chroma()
            );
            assert!(close(c.hue(), hue, 1e-3), "hue {} vs {hue}", c.hue());
        }
    }

    #[test]
    fn with_chroma_holds_hue_and_intensity_exactly() {
        let c = Ictcp::polar(165.0, 0.11, 220.0);
        let d = c.with_chroma(0.04);
        assert_eq!(d.i, c.i);
        assert!(close(d.hue(), c.hue(), 1e-3));
        assert!(close(d.chroma(), 0.04, 1e-6));
    }

    #[test]
    fn delta_itp_basics() {
        let w = |n: f32| from_2020([n, n, n]);
        assert_eq!(w(203.0).delta_itp(w(203.0)), 0.0);
        let (a, b) = (w(203.0), w(61.0));
        assert!(close(a.delta_itp(b), b.delta_itp(a), 0.0));
        // An achromatic pair's distance is exactly 720 * dI.
        let want = 720.0 * (pq::encode(203.0) - pq::encode(61.0));
        assert!(close(a.delta_itp(b), want, 1e-2));
        // A few nits around 200 is a few JND: sane magnitude, not 0.001 and not 10^4.
        let d = w(203.0).delta_itp(w(200.0));
        assert!(d > 0.3 && d < 10.0, "JND scale off: {d}");
    }

    /// The point of mixing in ICtCp: equal steps in `t` are equal steps to the eye,
    /// so the perceptual midpoint of black -> white sits far below the linear-light
    /// midpoint. If this ever reads as a linear lerp, the ramp primitive has been
    /// quietly rewired.
    #[test]
    fn mix_is_perceptual_not_linear() {
        let a = from_2020([2.0, 2.0, 2.0]);
        let b = from_2020([203.0, 203.0, 203.0]);
        let mid = Ictcp::mix(a, b, 0.5);
        assert!(
            mid.nits() < 0.5 * (2.0 + 203.0) * 0.5,
            "midpoint {} nits",
            mid.nits()
        );
        let (da, db) = (mid.delta_itp(a), mid.delta_itp(b));
        assert!((da - db).abs() < 3.0, "midpoint skew: {da} vs {db}");
    }
}
