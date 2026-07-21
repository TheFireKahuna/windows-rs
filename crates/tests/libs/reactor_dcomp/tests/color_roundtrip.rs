//! `color.rs` — the currency every DComp draw call is authored in.
//!
//! `Color::rgb` gamma-decodes 8-bit sRGB into linear scRGB via a 256-entry
//! `const` table; `to_srgb8` re-encodes with the piecewise sRGB OETF. The DComp
//! backend draws linear FP16 and passes the value through raw, so the decode is
//! the *only* place a chrome token's meaning is fixed — a table off by one
//! shifts every surface in the app.

use windows_reactor::Color;

/// The documented anchors: the Fluent surface ladder and the accent must
/// survive sRGB → linear → sRGB unchanged.
#[test]
fn documented_hex_values_round_trip_exactly() {
    for (r, g, b) in [
        (0x28, 0x28, 0x28), // SolidBackgroundFillColorTertiary  — card surface
        (0x1c, 0x1c, 0x1c), // SolidBackgroundFillColorSecondary — window base
        (0x0e, 0xa5, 0xe9), // an accent with three distinct channels
    ] {
        let c = Color::rgb(r, g, b);
        assert_eq!(
            c.to_srgb8(),
            (r, g, b, 255),
            "#{r:02x}{g:02x}{b:02x} did not round-trip"
        );
    }
}

/// Every one of the 256 table entries round-trips. A single wrong entry is a
/// silently wrong color for whichever token happens to use that byte.
#[test]
fn all_256_srgb_bytes_round_trip() {
    for v in 0u8..=255 {
        let (r, g, b, a) = Color::rgb(v, v, v).to_srgb8();
        assert_eq!((r, g, b, a), (v, v, v, 255), "byte {v} did not round-trip");
    }
}

/// The decode is monotone: a brighter sRGB byte is never a darker linear value.
/// A non-monotone table would invert a gradient ramp.
#[test]
fn decode_is_strictly_monotone() {
    let mut prev = f32::NEG_INFINITY;
    for v in 0u8..=255 {
        let l = Color::rgb(v, v, v).r;
        assert!(
            l > prev,
            "linear value for byte {v} ({l}) did not increase from {prev}"
        );
        prev = l;
    }
}

/// The table's endpoints and its documented interior value. `0x28` = 40 decodes
/// to ~0.021219 — the base the dark wash cubic was fitted against, so this is
/// load-bearing for `theme::dark_wash_alpha` too.
#[test]
fn decode_endpoints_and_the_dark_surface_base() {
    assert_eq!(Color::rgb(0, 0, 0).r, 0.0);
    assert_eq!(Color::rgb(255, 255, 255).r, 1.0);

    let base = Color::rgb(0x28, 0x28, 0x28).r;
    assert!(
        (base - 0.021_219).abs() < 1e-5,
        "#282828 decoded to {base}, expected ~0.021219"
    );

    // The table must equal the sRGB EOTF, computed independently here.
    for v in [1u8, 10, 40, 128, 200, 249, 254] {
        let expect = srgb_eotf(v as f32 / 255.0);
        let got = Color::rgb(v, v, v).r;
        assert!(
            (got - expect).abs() < 1e-6,
            "byte {v}: table {got} vs EOTF {expect}"
        );
    }
}

/// 8-bit alpha is *not* gamma-decoded — it is already linear. `rgba(_,_,_,0x80)`
/// must be 128/255, not a gamma-mangled value.
#[test]
fn alpha_is_linear_not_gamma_decoded() {
    for a in [0u8, 1, 0x5d, 0x80, 0x87, 0xc5, 255] {
        let c = Color::rgba(0, 0, 0, a);
        assert!(
            (c.a - a as f32 / 255.0).abs() < 1e-7,
            "alpha {a} decoded to {}",
            c.a
        );
    }
}

/// Extended-range (HDR headroom / wide-gamut) channels clamp gracefully at the
/// 8-bit boundary rather than wrapping.
#[test]
fn extended_range_channels_clamp_at_the_srgb_boundary() {
    assert_eq!(
        Color::scrgb(4.0, 4.0, 4.0, 1.0).to_srgb8(),
        (255, 255, 255, 255)
    );
    assert_eq!(
        Color::scrgb(-1.0, -1.0, -1.0, 1.0).to_srgb8(),
        (0, 0, 0, 255)
    );
    assert_eq!(Color::scrgb(0.0, 0.0, 0.0, 2.0).to_srgb8().3, 255);
    assert_eq!(Color::transparent().to_srgb8(), (0, 0, 0, 0));
}

/// The standard sRGB EOTF — an oracle independent of the library's table.
fn srgb_eotf(s: f32) -> f32 {
    if s <= 0.040_45 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}
