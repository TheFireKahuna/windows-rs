//! What a stroke style and a flattening tolerance actually cost when widening a
//! live trace's geometry.
//!
//! A trace is a Catmull-Rom spline: one cubic per sample gap, several hundred of
//! them, rebuilt every publish. Turning that into the region its stroke covers
//! ([`Path::widen`]) is the price of drawing it through a clip instead of a
//! capture, so the two knobs that price depends on are worth knowing rather than
//! guessing:
//!
//! * the **join style** — a round join emits an arc at every vertex, a bevel one
//!   segment, a miter one point (falling back to bevel past its limit);
//! * the **flattening tolerance** — how finely both the cubics and those arcs
//!   are reduced to line segments. Direct2D's default is 0.25, in the geometry's
//!   own units, which here are DIPs. What matters on screen is the error in
//!   DEVICE pixels, so the honest knob is `device_px / scale`.
//!
//! Reports the per-widen time and the vertex count of the result, because the
//! output feeds a `CompositionPath` and the compositor pays for its size too.
//!
//! ```text
//! cargo run -p windows-canvas --example widen_cost --release
//! ```

use windows_canvas::*;

/// Sample count of the geometry under test — a full-width analyzer trace.
const SAMPLES: usize = 512;
/// Stroke width in DIPs. A trace is thin, which is the whole reason the join
/// question is interesting: the join arc's radius is half of this.
const WIDTH: f32 = 1.5;
/// The DIP→px scale the geometry will rasterize under.
const SCALE: f32 = 1.5;

const REPS: u32 = 200;

fn main() -> Result<()> {
    let device = GpuDevice::new()?;
    let path = trace_path(&device)?;

    println!("geometry: {SAMPLES} samples, {} cubics, width {WIDTH} DIP, scale {SCALE}", SAMPLES - 1);
    println!();
    println!(
        "{:<10} {:>12} {:>10} {:>10} {:>12} {:>10}  {}",
        "join", "tolerance", "us/widen", "segments", "us/+outline", "segments", "device-px error"
    );

    for (name, join) in [
        ("round", LineJoin::Round),
        ("bevel", LineJoin::Bevel),
        ("miter", LineJoin::Miter),
    ] {
        let style = device.create_stroke_style(
            &StrokeStyleBuilder::new().caps(CapStyle::Round).line_join(join),
        )?;
        for device_px in [0.25f32, 0.5, 1.0] {
            // The tolerance is applied in the geometry's space, so a target
            // stated in device pixels divides by the scale it will be drawn at.
            let tol = device_px / SCALE;
            // Warm the driver before timing.
            let widened = path.widen(&device, WIDTH, &style, tol)?;
            let segments = widened.segment_count();

            let t0 = std::time::Instant::now();
            for _ in 0..REPS {
                let _ = path.widen(&device, WIDTH, &style, tol)?;
            }
            let us = t0.elapsed().as_secs_f64() * 1e6 / f64::from(REPS);

            // ...and the same again with the self-intersections resolved. A
            // widened outline laps over itself at every sharp turn and relies on
            // nonzero winding to still describe one region; `Outline` removes
            // that, which is only worth doing if the geometry gets SMALLER.
            let outlined = widened.outline(&device, tol)?;
            let out_segments = outlined.segment_count();
            let t1 = std::time::Instant::now();
            for _ in 0..REPS {
                let w = path.widen(&device, WIDTH, &style, tol)?;
                let _ = w.outline(&device, tol)?;
            }
            let out_us = t1.elapsed().as_secs_f64() * 1e6 / f64::from(REPS);

            println!(
                "{name:<10} {tol:>12.3} {us:>10.1} {segments:>10} {out_us:>12.1} {out_segments:>10}  {device_px:.2}"
            );
        }
    }
    Ok(())
}

/// A spline shaped like a real trace: one cubic per sample gap, over a noisy
/// response. The exact values do not matter, but the DENSITY does — that is what
/// decides how much there is to flatten.
fn trace_path(device: &GpuDevice) -> Result<Path> {
    let pts: Vec<(f32, f32)> = (0..SAMPLES)
        .map(|i| {
            let t = i as f32 / SAMPLES as f32;
            let x = t * 1400.0;
            // Something with both smooth sweeps and local roughness, so the
            // flattener meets curvature rather than a straight line.
            let y = 200.0
                + 90.0 * (t * 6.0).sin()
                + 18.0 * (t * 47.0).sin()
                + 6.0 * (t * 131.0).cos();
            (x, y)
        })
        .collect();

    let mut fig = PathBuilder::new(device)?.begin_hollow(Vector2::new(pts[0].0, pts[0].1));
    let at = |i: isize| pts[i.clamp(0, SAMPLES as isize - 1) as usize];
    for i in 0..SAMPLES as isize - 1 {
        let (pp, pa, pb, pn) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        fig = fig.bezier_to_flat(&[
            pa.0 + (pb.0 - pp.0) / 6.0,
            pa.1 + (pb.1 - pp.1) / 6.0,
            pb.0 - (pn.0 - pa.0) / 6.0,
            pb.1 - (pn.1 - pa.1) / 6.0,
            pb.0,
            pb.1,
        ]);
    }
    fig.end_open().build()
}
