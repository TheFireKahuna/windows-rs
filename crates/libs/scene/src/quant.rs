//! Snapping and quantization: how a derived value becomes a cache key. **App half.**
//!
//! Two of this crate's rasterized families are keyed on values the application computes
//! rather than declares — a box cell on a laid-out size, a colour cell on whatever light a
//! widget resolved. Left raw, a drag-resize mints one FP16 surface per frame and an
//! animated fill mints one per step.
//!
//! So both dimensions are snapped onto the granularity the raster already has. Every
//! function here is pure, deterministic to the bit, non-negative where that is meaningful,
//! and collapses a non-finite input rather than minting an entry per `NaN` payload.

/// Extents snap to whole physical pixels: a raster cannot hold a fraction of one.
const EXTENT_STEPS_PER_PX: f32 = 1.0;

/// Radii and stroke widths snap to quarter pixels. Finer than an extent because a corner
/// profile is what the nine-grid stretches from, and a quarter-pixel change in it is
/// visible where a quarter-pixel change in a box's width is not.
const DETAIL_STEPS_PER_PX: f32 = 4.0;

/// Steps per unit of the signed-square-root colour encoding.
const COLOR_STEPS: f32 = 4096.0;

/// Snaps a DIP length onto the physical grid at `steps_per_px` steps per pixel.
#[must_use]
pub fn snap_len(dip: f32, scale: f32, steps_per_px: f32) -> f32 {
    if !dip.is_finite() || dip <= 0.0 {
        return 0.0;
    }
    let grid = (scale * steps_per_px).max(1.0e-3);
    (dip * grid).round() / grid
}

/// Snaps a detail length — a corner radius, a stroke width — onto quarter pixels.
#[must_use]
pub fn snap_detail(dip: f32, scale: f32) -> f32 {
    snap_len(dip, scale, DETAIL_STEPS_PER_PX)
}

/// Snaps an extent onto whole physical pixels, and never to zero.
///
/// A positive extent that rounds below one pixel still rasterizes one: a sub-pixel bar is
/// a faint bar, and a zero-sized surface is an allocation failure.
#[must_use]
pub fn snap_extent(dip: f32, scale: f32) -> f32 {
    if !dip.is_finite() || dip <= 0.0 {
        return 0.0;
    }
    let grid = (scale * EXTENT_STEPS_PER_PX).max(1.0e-3);
    (dip * grid).round().max(1.0) / grid
}

/// Canonicalizes a DIP-to-pixel factor.
///
/// Display scales are a short list — 1.0, 1.25, 1.5, 1.75, 2.0 — and rounding to a
/// thousandth keeps float noise in `dpi / 96.0` from forking the whole cache into two
/// populations that differ in the last bit. Every dimension above is snapped against
/// *this* value, so a key is self-consistent.
#[must_use]
pub fn snap_scale(scale: f32) -> f32 {
    if !scale.is_finite() || scale <= 0.0 {
        return 1.0;
    }
    (scale * 1000.0).round() / 1000.0
}

/// The pixel extent a snapped DIP size occupies, for an allocation.
#[must_use]
pub fn extent_px(dip: f32, scale: f32) -> u32 {
    let px = (snap_extent(dip, scale) * scale).round();
    px.clamp(1.0, u32::from(u16::MAX) as f32) as u32
}

/// A quantized display-referred colour: the only colour a rasterizer can reach.
///
/// The encoding is a **signed square root** before a uniform step, which is what lets it
/// key an extended-range pipeline at all. A quantizer that clamped to `[0, 1]` would crush
/// both of the things FP16 surfaces exist to carry: a negative component (a colour outside
/// Rec.709 on a wide-gamut display) and a component far above one (luminance above the
/// display's white). Sign-symmetric, exact at zero, and spending resolution where the eye
/// is — the step in linear light is `≈ 2·√|v| / 4096`, about 1/2048 at scRGB 1.0 and
/// quadratically finer approaching black.
///
/// Its fields are private and the only constructor quantizes, so "round-trip the key
/// before drawing" is not a rule to keep: there is no un-round-tripped value in scope.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Q([i32; 4]);

impl Q {
    /// Quantizes a display-referred colour.
    #[must_use]
    pub fn new(c: windows_color::Scrgb) -> Self {
        Self([
            quant_channel(c.r),
            quant_channel(c.g),
            quant_channel(c.b),
            quant_channel(c.a),
        ])
    }

    /// The colour this key is actually painted in.
    #[must_use]
    pub fn dequant(self) -> windows_color::Scrgb {
        windows_color::Scrgb {
            r: dequant_channel(self.0[0]),
            g: dequant_channel(self.0[1]),
            b: dequant_channel(self.0[2]),
            a: dequant_channel(self.0[3]),
        }
    }

    /// Whether the quantized alpha is fully opaque, which is what decides a cell's
    /// alpha mode.
    #[must_use]
    pub fn is_opaque(self) -> bool {
        self.0[3] >= COLOR_STEPS as i32
    }
}

impl From<windows_color::Scrgb> for Q {
    fn from(c: windows_color::Scrgb) -> Self {
        Self::new(c)
    }
}

fn quant_channel(v: f32) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    let e = v.abs().sqrt().copysign(v);
    (e * COLOR_STEPS).round() as i32
}

fn dequant_channel(q: i32) -> f32 {
    let e = q as f32 / COLOR_STEPS;
    (e * e).copysign(e)
}

/// Quantizes a gradient stop's position to 1/65536 of the ramp.
///
/// A ramp is rasterized into 256 texels, so a stop resolved to more precision than this
/// cannot move one and only forks the identity that holds them.
#[must_use]
pub fn quant_stop(at: f32) -> u16 {
    if !at.is_finite() {
        return 0;
    }
    (at.clamp(0.0, 1.0) * f32::from(u16::MAX)).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_color::Scrgb;

    const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

    #[test]
    fn a_non_finite_input_collapses_rather_than_minting_a_key() {
        for scale in SCALES {
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
                assert_eq!(snap_extent(bad, scale), 0.0);
                assert_eq!(snap_detail(bad, scale), 0.0);
            }
        }
        assert_eq!(snap_scale(f32::NAN), 1.0);
        assert_eq!(snap_scale(0.0), 1.0);
    }

    #[test]
    fn a_positive_extent_never_collapses_to_nothing() {
        for scale in SCALES {
            let snapped = snap_extent(0.1, scale);
            assert!(snapped > 0.0, "0.1 DIP at {scale}x snapped to {snapped}");
            assert_eq!(extent_px(0.1, scale), 1);
        }
    }

    #[test]
    fn snapping_lands_on_the_physical_grid() {
        for scale in SCALES {
            for dip in [1.0_f32, 7.3, 12.9, 100.4] {
                let px = snap_extent(dip, scale) * scale;
                assert!(
                    (px - px.round()).abs() < 1.0e-3,
                    "{dip} DIP at {scale}x is {px} px"
                );
                let detail = snap_detail(dip, scale) * scale * DETAIL_STEPS_PER_PX;
                assert!((detail - detail.round()).abs() < 1.0e-3);
            }
        }
    }

    #[test]
    fn quantization_is_sign_symmetric_and_exact_at_zero() {
        let zero = Q::new(Scrgb::TRANSPARENT).dequant();
        assert_eq!((zero.r, zero.g, zero.b, zero.a), (0.0, 0.0, 0.0, 0.0));
        for v in [0.25_f32, 1.0, 4.0, 12.0] {
            let plus = Q::new(Scrgb {
                r: v,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
            .dequant()
            .r;
            let minus = Q::new(Scrgb {
                r: -v,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
            .dequant()
            .r;
            assert!((plus + minus).abs() < 1.0e-4, "{v}: {plus} vs {minus}");
        }
    }

    #[test]
    fn quantization_clips_neither_end_of_the_extended_range() {
        // Above the display's white, and outside Rec.709. Both are what the FP16
        // surfaces exist to carry, and a clamping quantizer would destroy both.
        let wild = Scrgb {
            r: -0.4,
            g: 12.0,
            b: 0.5,
            a: 1.0,
        };
        let back = Q::new(wild).dequant();
        assert!(back.r < 0.0, "a negative component survived as {}", back.r);
        assert!(
            back.g > 11.9,
            "an above-white component survived as {}",
            back.g
        );
    }

    #[test]
    fn quantization_is_finer_than_sixteen_bits_at_white() {
        let a = Q::new(Scrgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
        let b = Q::new(Scrgb {
            r: 1.0 + 1.0 / 2048.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
        assert_ne!(a, b, "a 1/2048 step at scRGB 1.0 must be distinguishable");
    }

    #[test]
    fn opacity_comes_from_the_quantized_alpha_and_not_the_authored_one() {
        assert!(
            Q::new(Scrgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0
            })
            .is_opaque()
        );
        assert!(
            !Q::new(Scrgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.999
            })
            .is_opaque()
        );
    }
}
