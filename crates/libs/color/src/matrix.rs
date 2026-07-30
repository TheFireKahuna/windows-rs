//! 3x3 linear algebra, derived at compile time in `f64` and stored as `f32`.
//!
//! Every matrix this crate uses is const-derived from primary chromaticities or from
//! a standard's exact rational constants — nothing is a hand-copied decimal that
//! can silently drift. The derivation runs in `f64`, where exactness is free; the
//! runtime form is `f32`, where speed is not.

pub(crate) type Mat3 = [[f64; 3]; 3];
pub(crate) type Mat3f = [[f32; 3]; 3];

/// D65 white chromaticity. sRGB, BT.709 and BT.2020 share it.
pub(crate) const D65: (f64, f64) = (0.3127, 0.3290);

/// `a * b`.
pub(crate) const fn mul(a: Mat3, b: Mat3) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    let mut r = 0;
    while r < 3 {
        let mut c = 0;
        while c < 3 {
            out[r][c] = a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c];
            c += 1;
        }
        r += 1;
    }
    out
}

/// Inverse via the adjugate. No pivoting, which is fine for the well-conditioned
/// colorimetric matrices this crate derives, and which keeps it `const`.
pub(crate) const fn inv(m: Mat3) -> Mat3 {
    let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
    let c01 = m[1][2] * m[2][0] - m[1][0] * m[2][2];
    let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];
    let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;
    [
        [
            c00 / det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / det,
        ],
        [
            c01 / det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / det,
        ],
        [
            c02 / det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / det,
        ],
    ]
}

/// Linear RGB -> XYZ from primary and white chromaticities `(x, y)`. The columns are
/// the primaries' XYZ, scaled so that RGB `(1, 1, 1)` lands exactly on the white
/// point.
pub(crate) const fn rgb_to_xyz(r: (f64, f64), g: (f64, f64), b: (f64, f64), w: (f64, f64)) -> Mat3 {
    // xyY (Y = 1) -> XYZ for each primary.
    let p = [
        [r.0 / r.1, g.0 / g.1, b.0 / b.1],
        [1.0, 1.0, 1.0],
        [
            (1.0 - r.0 - r.1) / r.1,
            (1.0 - g.0 - g.1) / g.1,
            (1.0 - b.0 - b.1) / b.1,
        ],
    ];
    let white = [w.0 / w.1, 1.0, (1.0 - w.0 - w.1) / w.1];
    // Solve P * s = white for the per-primary scales, then scale the columns.
    let pinv = inv(p);
    let s = [
        pinv[0][0] * white[0] + pinv[0][1] * white[1] + pinv[0][2] * white[2],
        pinv[1][0] * white[0] + pinv[1][1] * white[1] + pinv[1][2] * white[2],
        pinv[2][0] * white[0] + pinv[2][1] * white[1] + pinv[2][2] * white[2],
    ];
    [
        [p[0][0] * s[0], p[0][1] * s[1], p[0][2] * s[2]],
        [p[1][0] * s[0], p[1][1] * s[1], p[1][2] * s[2]],
        [p[2][0] * s[0], p[2][1] * s[1], p[2][2] * s[2]],
    ]
}

/// Narrow a derived matrix to its runtime form.
pub(crate) const fn narrow(m: Mat3) -> Mat3f {
    [
        [m[0][0] as f32, m[0][1] as f32, m[0][2] as f32],
        [m[1][0] as f32, m[1][1] as f32, m[1][2] as f32],
        [m[2][0] as f32, m[2][1] as f32, m[2][2] as f32],
    ]
}

/// Matrix-vector product.
#[inline]
pub(crate) fn apply(m: &Mat3f, v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const REC709: Mat3 = rgb_to_xyz((0.640, 0.330), (0.300, 0.600), (0.150, 0.060), D65);
    const REC2020: Mat3 = rgb_to_xyz((0.708, 0.292), (0.170, 0.797), (0.131, 0.046), D65);

    #[test]
    fn inverse_round_trips() {
        let identity = mul(REC709, inv(REC709));
        for (r, row) in identity.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let want = if r == c { 1.0 } else { 0.0 };
                assert!((v - want).abs() < 1e-12, "[{r}][{c}] = {v}");
            }
        }
    }

    #[test]
    fn white_maps_to_white() {
        // RGB (1,1,1) must land exactly on D65, which is what the column scaling is for.
        for m in [REC709, REC2020] {
            let xyz = [
                m[0][0] + m[0][1] + m[0][2],
                m[1][0] + m[1][1] + m[1][2],
                m[2][0] + m[2][1] + m[2][2],
            ];
            let sum = xyz[0] + xyz[1] + xyz[2];
            assert!((xyz[0] / sum - D65.0).abs() < 1e-12);
            assert!((xyz[1] / sum - D65.1).abs() < 1e-12);
        }
    }

    /// BT.2087 publishes the BT.709 -> BT.2020 conversion to four decimals. If the
    /// derivation drifts, this is where it shows.
    #[test]
    fn derived_709_to_2020_matches_bt2087() {
        let m = mul(inv(REC2020), REC709);
        let published: Mat3 = [
            [0.6274, 0.3293, 0.0433],
            [0.0691, 0.9195, 0.0114],
            [0.0164, 0.0880, 0.8956],
        ];
        for (r, row) in m.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                assert!(
                    (v - published[r][c]).abs() < 5e-4,
                    "[{r}][{c}] {v} vs {}",
                    published[r][c]
                );
            }
        }
    }
}
