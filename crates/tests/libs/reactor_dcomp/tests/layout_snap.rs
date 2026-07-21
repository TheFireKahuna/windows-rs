//! `backend/dcomp/layout.rs` — pixel-grid snapping.
//!
//! The root visual carries `SetScale(scale)`, so a fractional DIP offset lands
//! between physical pixels and the compositor bilinear-resamples the surface —
//! text and hairlines blur. `snap` puts every assigned rect back on the grid.
//!
//! `assign` snaps *edges*, never sizes:
//!
//! ```text
//! sx = snap(ax);  w = snap(ax + aw) - sx
//! ```
//!
//! and recurses with the **unsnapped** absolute origin. Those two decisions are
//! what these tests are about; both are pure arithmetic, no window needed.

use windows_reactor::dcomp_test_api::{snap, snap_edge};

/// Every DPI the backend actually sees.
const SCALES: [f32; 6] = [1.0, 1.25, 1.5, 1.75, 2.0, 3.0];

/// A snapped coordinate sits on the physical pixel grid.
#[test]
fn snapped_coordinates_land_on_the_physical_grid() {
    for scale in SCALES {
        for i in 0..500 {
            let v = i as f32 * 0.317 - 40.0;
            let px = snap(v, scale) * scale;
            assert!(
                (px - px.round()).abs() < 1e-3,
                "snap({v}, {scale}) * scale = {px} is not an integer pixel"
            );
        }
    }
}

/// Snapping is idempotent — a second pass over an already-snapped tree must
/// not move anything (which would repaint every surface for nothing).
#[test]
fn snap_is_idempotent() {
    for scale in SCALES {
        for i in 0..500 {
            let v = i as f32 * 0.317 - 40.0;
            let once = snap(v, scale);
            assert_eq!(
                snap(once, scale),
                once,
                "snap not idempotent at {v} / {scale}"
            );
        }
    }
}

/// Snapping moves a coordinate by at most half a physical pixel.
#[test]
fn snap_moves_at_most_half_a_pixel() {
    for scale in SCALES {
        for i in 0..2000 {
            let v = i as f32 * 0.137 - 100.0;
            let d = (snap(v, scale) - v).abs();
            assert!(
                d <= 0.5 / scale + 1e-4,
                "snap({v}, {scale}) moved {d} DIPs, more than half a pixel"
            );
        }
    }
}

/// Integer DIPs at integer scales are already on the grid and must not move.
#[test]
fn snap_is_identity_for_grid_aligned_values() {
    for scale in [1.0f32, 2.0, 3.0] {
        for i in -50..50 {
            let v = i as f32;
            assert_eq!(
                snap(v, scale),
                v,
                "snap moved an aligned value {v} at {scale}"
            );
        }
    }
}

/// **The reason edges are snapped rather than sizes.**
///
/// A row of siblings laid out end-to-end must stay flush after snapping: every
/// child's snapped right edge is exactly the next child's snapped left edge —
/// no seam, no overlap — even when each child's DIP width is fractional.
///
/// Snapping *widths* independently is the failure this guards: it would round
/// each width on its own and let the error accumulate along the row.
#[test]
fn snapped_siblings_stay_flush_with_no_gaps_or_overlaps() {
    for scale in SCALES {
        for width in [33.333_33_f32, 17.5, 0.7, 101.0 / 3.0, 12.345] {
            let mut origin = 7.25_f32; // a deliberately off-grid row start
            let mut prev_right: Option<f32> = None;

            for _ in 0..64 {
                let (x, w) = snap_edge(origin, width, scale);
                if let Some(right) = prev_right {
                    assert_eq!(
                        x, right,
                        "seam at scale {scale}, width {width}: previous right {right} != next left {x}"
                    );
                }
                assert!(w >= 0.0, "negative snapped width {w}");
                prev_right = Some(x + w);
                origin += width;
            }

            // The row's total snapped extent tracks the exact extent — errors
            // did not accumulate over 64 children.
            let exact_end = 7.25 + 64.0 * width;
            let drift = (prev_right.unwrap() - exact_end).abs();
            assert!(
                drift <= 0.5 / scale + 1e-3,
                "row drifted {drift} DIPs over 64 children at scale {scale}, width {width}"
            );
        }
    }
}

/// **The reason `assign` recurses with the unsnapped absolute origin.**
///
/// Nesting must not accumulate rounding error: at depth 32 with a fractional
/// inset at every level, the deepest node's snapped absolute position is still
/// within half a physical pixel of its exact absolute position.
///
/// If `assign` recursed with the *snapped* origin instead, each level would
/// re-round an already-rounded number and the error would compound.
#[test]
fn nesting_does_not_accumulate_snap_drift() {
    for scale in SCALES {
        for inset in [1.0_f32 / 3.0, 0.7, 4.5, 0.125, 11.0 / 7.0] {
            let mut exact = 0.0_f32;
            for depth in 0..32 {
                exact += inset;
                let snapped = snap(exact, scale);
                let drift = (snapped - exact).abs();
                assert!(
                    drift <= 0.5 / scale + 1e-3,
                    "depth {depth} at scale {scale} inset {inset}: drift {drift} DIPs"
                );
            }
        }
    }
}

/// `assign` writes each node a composition offset **relative to its snapped
/// parent** (`push_offset(sx - sox, …)`), and the compositor re-adds the parent
/// origin. Reconstructing that way must land the child on the same physical
/// pixel its own snap chose — otherwise the snapping is undone by the transform
/// chain and the surface resamples anyway.
///
/// The reconstruction is *not* bit-exact in f32 (`a + (b - a)` can differ from
/// `b` by an ulp, e.g. 47.2 → 47.199997 at scale 1.25). That is ~4e-6 DIP,
/// five orders of magnitude below a pixel; the property worth asserting is
/// therefore the pixel the value lands on, plus a hard bound on the residual.
#[test]
fn relative_offsets_reconstruct_absolute_snapped_positions() {
    for scale in SCALES {
        let parent_abs = 13.4_f32;
        let parent_snapped = snap(parent_abs, scale);

        for i in 0..64 {
            let child_abs = parent_abs + i as f32 * 2.5 / 3.0;
            let child_snapped = snap(child_abs, scale);
            let relative = child_snapped - parent_snapped; // what push_offset receives
            let reconstructed = parent_snapped + relative;

            assert_eq!(
                (reconstructed * scale).round(),
                (child_snapped * scale).round(),
                "child {i} at scale {scale} reconstructs onto a different physical pixel \
                 ({reconstructed} vs {child_snapped})"
            );
            assert!(
                (reconstructed - child_snapped).abs() < 1e-4,
                "child {i} at scale {scale}: residual {} DIPs",
                (reconstructed - child_snapped).abs()
            );
        }
    }
}

/// Negative coordinates (a node scrolled above its viewport, a ghost placed
/// off-screen) round the same way as positive ones — `round()` is symmetric
/// about zero, so no half-pixel jump crossing the origin.
#[test]
fn snapping_is_symmetric_about_the_origin() {
    for scale in SCALES {
        for i in 1..200 {
            let v = i as f32 * 0.211;
            assert_eq!(
                snap(-v, scale),
                -snap(v, scale),
                "snap asymmetric at ±{v}, scale {scale}"
            );
        }
    }
}

/// A degenerate scale must not produce NaN/inf rects. `layout::compute` clamps
/// with `scale.max(0.01)` before calling `assign`; this pins that the snap
/// arithmetic itself is well-behaved for every scale that survives the clamp.
#[test]
fn snap_is_finite_for_every_clamped_scale() {
    for scale in [0.01_f32, 0.5, 1.0, 4.0, 8.0] {
        for v in [-1e4_f32, -0.5, 0.0, 0.5, 1e4] {
            let s = snap(v, scale);
            assert!(s.is_finite(), "snap({v}, {scale}) = {s}");
        }
    }
}
