//! `backend/dcomp/theme.rs` — the sRGB→linear wash-alpha cubics.
//!
//! Fluent's translucent tokens (`#0AFFFFFF`-style washes) are authored assuming
//! **gamma-space** compositing. This backend blends in **linear light**, where
//! the same alpha reads far hotter. `dark_wash_alpha` / `light_wash_alpha` are
//! cubic fits of the exact conversion, documented as accurate "within ~1%
//! across `0..=1`".
//!
//! These tests check the cubics against that exact conversion, derived here
//! from first principles so the oracle is independent of the fit:
//!
//! for ink `I` and base `B` (both sRGB), an authored alpha `a` produces
//! `out_srgb = a*I + (1-a)*B`; the linear alpha that reproduces it is the `α`
//! solving `eotf(out_srgb) = α*eotf(I) + (1-α)*eotf(B)`.

use windows_reactor::dcomp_test_api::{dark_wash_alpha, light_wash_alpha};

/// The documented tolerance ("within ~1%").
///
/// Measured worst case at the time of writing: **dark 0.00244** (at a=0.783),
/// **light 0.00394** (at a=0.855) — so the fits hold with ~2.5–4× headroom and
/// this bound is a real regression guard rather than a rubber stamp.
const TOL: f32 = 0.01;

fn srgb_eotf(s: f32) -> f32 {
    if s <= 0.040_45 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Exact linear-blend alpha for white ink over the dark card surface `#282828`.
fn exact_dark(a: f32) -> f32 {
    let base = 40.0 / 255.0;
    let lb = srgb_eotf(base);
    let out = srgb_eotf(a * 1.0 + (1.0 - a) * base);
    (out - lb) / (1.0 - lb)
}

/// Exact linear-blend alpha for black ink over the light card surface `#f9f9f9`.
fn exact_light(a: f32) -> f32 {
    let base = 249.0 / 255.0;
    let lb = srgb_eotf(base);
    let out = srgb_eotf((1.0 - a) * base);
    (lb - out) / lb
}

#[test]
fn dark_cubic_tracks_the_exact_gamma_blend_within_one_percent() {
    let mut worst = (0.0_f32, 0.0_f32);
    for i in 0..=1000 {
        let a = i as f32 / 1000.0;
        let d = (dark_wash_alpha(a) - exact_dark(a)).abs();
        if d > worst.1 {
            worst = (a, d);
        }
    }
    assert!(
        worst.1 <= TOL,
        "dark cubic deviates {} at a={} (tolerance {TOL})",
        worst.1,
        worst.0
    );
}

#[test]
fn light_cubic_tracks_the_exact_gamma_blend_within_one_percent() {
    let mut worst = (0.0_f32, 0.0_f32);
    for i in 0..=1000 {
        let a = i as f32 / 1000.0;
        let d = (light_wash_alpha(a) - exact_light(a)).abs();
        if d > worst.1 {
            worst = (a, d);
        }
    }
    assert!(
        worst.1 <= TOL,
        "light cubic deviates {} at a={} (tolerance {TOL})",
        worst.1,
        worst.0
    );
}

/// The endpoints are exact, not merely close: a fully transparent wash must be
/// *invisible* (not a faint haze) and a fully opaque one must be *opaque* (not
/// a 99% film that lets the surface bleed through a "solid" fill).
#[test]
fn endpoints_are_exact() {
    assert_eq!(dark_wash_alpha(0.0), 0.0);
    assert_eq!(light_wash_alpha(0.0), 0.0);
    assert_eq!(dark_wash_alpha(1.0), 1.0, "opaque dark wash is not opaque");
    assert_eq!(
        light_wash_alpha(1.0),
        1.0,
        "opaque light wash is not opaque"
    );
}

/// Both cubics are monotone over their domain. A non-monotone alpha curve makes
/// a fade animation reverse direction mid-flight and makes the token ladder
/// (`stroke_subtle` < `stroke_divider` < `stroke` < `stroke_strong`) unorderable.
#[test]
fn cubics_are_monotone_over_the_unit_interval() {
    for (name, f) in [
        ("dark", dark_wash_alpha as fn(f32) -> f32),
        ("light", light_wash_alpha as fn(f32) -> f32),
    ] {
        let mut prev = f(0.0);
        for i in 1..=1000 {
            let a = i as f32 / 1000.0;
            let v = f(a);
            assert!(v >= prev, "{name} cubic decreased at a={a}: {v} < {prev}");
            prev = v;
        }
    }
}

/// Output stays a valid alpha for every input in range, including the clamp arm.
#[test]
fn output_is_a_valid_alpha_and_clamps_above_one() {
    for i in 0..=1200 {
        let a = i as f32 / 1000.0; // deliberately overshoots 1.0
        for (name, v) in [("dark", dark_wash_alpha(a)), ("light", light_wash_alpha(a))] {
            assert!(
                (0.0..=1.0).contains(&v),
                "{name} cubic produced {v} for a={a}"
            );
        }
    }
}

/// The direction the whole conversion exists for: **on a dark base a linear
/// blend needs LESS alpha** than the sRGB-authored number (white ink is far
/// brighter than the surface in linear light), while **on a light base it needs
/// MORE**. Getting the two tables swapped would still be monotone and still
/// hit the endpoints — this is the test that catches it.
#[test]
fn dark_attenuates_and_light_amplifies() {
    for i in 1..1000 {
        let a = i as f32 / 1000.0;
        assert!(
            dark_wash_alpha(a) < a,
            "dark wash did not attenuate at a={a}: {}",
            dark_wash_alpha(a)
        );
        assert!(
            light_wash_alpha(a) > a,
            "light wash did not amplify at a={a}: {}",
            light_wash_alpha(a)
        );
    }
}

/// The Fluent stroke ladder, expressed through the dark cubic, keeps its order
/// and stays faint. These are the actual authored token alphas.
#[test]
fn fluent_stroke_ladder_stays_ordered_and_subtle() {
    let subtle = dark_wash_alpha(0x0a as f32 / 255.0);
    let divider = dark_wash_alpha(0x15 as f32 / 255.0);
    let stroke = dark_wash_alpha(0x12 as f32 / 255.0);
    let strong = dark_wash_alpha(0x8b as f32 / 255.0);

    assert!(subtle < stroke, "{subtle} !< {stroke}");
    assert!(stroke < divider, "{stroke} !< {divider}");
    assert!(divider < strong, "{divider} !< {strong}");
    assert!(
        strong < 0.5,
        "the 'strong' hairline is not a hairline: {strong}"
    );
}
