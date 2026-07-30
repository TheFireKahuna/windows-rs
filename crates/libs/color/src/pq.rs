//! SMPTE ST 2084 (Perceptual Quantizer), from its exact rationals.
//!
//! PQ appears in exactly two places in this crate: the `I` axis of [`Ictcp`] and the
//! BT.2390 tone curve, which is defined on a PQ signal. It is never a surface
//! encoding and never a colour space handed to Direct2D or to the compositor.
//!
//! [`Ictcp`]: crate::Ictcp

const M1: f32 = 1305.0 / 8192.0;
const M2: f32 = 2523.0 / 32.0;
const C1: f32 = 107.0 / 128.0;
const C2: f32 = 2413.0 / 128.0;
const C3: f32 = 2392.0 / 128.0;

/// The PQ signal peak: `E' = 1.0` encodes exactly 10,000 cd/m².
pub(crate) const PEAK_NITS: f32 = 10_000.0;

/// Inverse EOTF: absolute luminance in cd/m² -> PQ signal. Negative input clamps to
/// zero, because PQ is defined on light.
#[inline]
pub(crate) fn encode(nits: f32) -> f32 {
    let y = (nits / PEAK_NITS).max(0.0);
    let yp = y.powf(M1);
    ((C1 + C2 * yp) / (1.0 + C3 * yp)).powf(M2)
}

/// EOTF: PQ signal -> absolute luminance in cd/m². `decode(0.0)` is exactly zero.
#[inline]
pub(crate) fn decode(e: f32) -> f32 {
    let ep = e.max(0.0).powf(1.0 / M2);
    let num = (ep - C1).max(0.0);
    let den = C2 - C3 * ep;
    (num / den).powf(1.0 / M1) * PEAK_NITS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ST 2084 and BT.2408 anchors. These are the numbers a reader can check against
    /// the published curve, so a transcription error in the rationals cannot survive.
    #[test]
    fn published_anchors() {
        assert!((encode(10_000.0) - 1.0).abs() < 1e-6);
        assert!((encode(0.0) - 7.3e-7).abs() < 1e-7);
        assert_eq!(decode(0.0), 0.0);
        assert!(
            (encode(203.0) - 0.5806).abs() < 5e-4,
            "PQ(203) = {}",
            encode(203.0)
        );
        assert!(
            (encode(100.0) - 0.5081).abs() < 5e-4,
            "PQ(100) = {}",
            encode(100.0)
        );
        assert!(
            (encode(1000.0) - 0.7518).abs() < 5e-4,
            "PQ(1000) = {}",
            encode(1000.0)
        );
    }

    #[test]
    fn round_trips() {
        for nits in [
            0.005f32, 0.1, 1.0, 2.0, 61.0, 203.0, 438.0, 1000.0, 4000.0, 9999.0,
        ] {
            let rt = decode(encode(nits));
            assert!(
                (rt - nits).abs() <= nits * 1e-4 + 1e-6,
                "round trip {nits} -> {rt}"
            );
        }
    }

    #[test]
    fn is_monotonic() {
        let mut last = -1.0;
        for step in 0..2000 {
            let e = encode(step as f32 * 5.0);
            assert!(e > last, "not monotonic at step {step}");
            last = e;
        }
    }
}
