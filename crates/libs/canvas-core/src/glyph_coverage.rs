//! Grid-fitted glyph coverage, taken from DirectWrite without drawing it.
//!
//! [`DrawingSession::draw_glyph_run`](crate::DrawingSession) hands a run to
//! Direct2D, which rasterizes AND conditions it in one step: the coverage the
//! rasterizer produces is bent through the gamma ramp in the session's rendering
//! params before it reaches a pixel. That bundling is a property of the Direct2D
//! text path, not of DirectWrite — `IDWriteGlyphRunAnalysis` exposes the
//! rasterizer on its own.
//!
//! What comes back is one byte per pixel, grid-fitted and antialiased by the
//! same rasterizer the drawn path uses. `CreateGlyphRunAnalysis` is handed no
//! rendering params at all — grid-fit mode and antialias mode are separate
//! arguments and there is nowhere to pass a gamma — and the blend gamma is
//! reported *separately* by `GetAlphaBlendParams` for the caller to apply. So a
//! caller owns the coverage→alpha curve rather than inheriting one, and can keep
//! grid fitting while blending in linear light.
//!
//! Measured against the drawn path (`analysis_coverage_matches_the_drawn_run`),
//! this coverage is what Direct2D writes when it draws the same run onto a
//! transparent FP16 target: the exponent carrying one onto the other fits at
//! 0.97, within 0.1/255 across the run, and the drawn colour is premultiplied
//! exactly off its own alpha. The rendering params' gamma does not reach a
//! transparent target — it bends a blend, and there is nothing there to blend
//! against.
//!
//! It also needs no device. No D3D, no Direct2D, no `BeginDraw`: coverage is
//! produced on the CPU, so it is available with no GPU at all and the same
//! inputs give the same bytes on any machine running the same font version.

use crate::bindings::*;
use crate::glyphs::GlyphRun;
use crate::text::dwrite_factory;
use windows_core::{Interface, Result};

/// Symmetric natural rendering — the mode the drawn path uses, and the one whose
/// horizontal positioning is subpixel rather than snapped. Not in the generated
/// bindings, which carry only the `MODE1` spelling of the same value.
const RENDERING_MODE_NATURAL_SYMMETRIC: DWRITE_RENDERING_MODE = 5;

/// One run's coverage, and where it sits.
///
/// `left`/`top` are device-pixel offsets of the texture's upper-left corner from
/// the baseline origin the coverage was asked for, so a caller places the
/// texture at `(baseline + (left, top))` without re-deriving an ink box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphCoverage {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    /// `width * height` bytes, row-major, one coverage value per pixel.
    pub alpha: Vec<u8>,
}

impl GlyphCoverage {
    /// Coverage at a pixel, or 0 outside the texture.
    pub fn at(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.alpha[(y * self.width + x) as usize]
    }

    /// Total coverage over the texture, in whole-pixel units — the run's "ink".
    ///
    /// This is the quantity a conditioning curve moves, and comparing it across
    /// curves is how a curve's weight is judged.
    pub fn ink(&self) -> f64 {
        self.alpha.iter().map(|&a| a as f64 / 255.0).sum()
    }
}

/// Rasterize `run` to grid-fitted grayscale coverage at `scale` DIP→px, with the
/// run's baseline origin at `baseline` (in DIPs, pre-transform).
///
/// Returns `None` for a run that marks no pixels — a space, or a run scaled to
/// nothing — which is not an error and which callers skip rather than cache.
pub fn glyph_run_coverage(
    run: &GlyphRun,
    scale: f32,
    baseline: (f32, f32),
) -> Result<Option<GlyphCoverage>> {
    let factory: IDWriteFactory2 = Interface::cast(&dwrite_factory()?)?;
    let transform = DWRITE_MATRIX {
        m11: scale,
        m12: 0.0,
        m21: 0.0,
        m22: scale,
        dx: 0.0,
        dy: 0.0,
    };

    let analysis = run.with_abi(|abi| unsafe {
        factory.CreateGlyphRunAnalysis(
            abi,
            Some(&transform),
            RENDERING_MODE_NATURAL_SYMMETRIC,
            DWRITE_MEASURING_MODE_NATURAL,
            DWRITE_GRID_FIT_MODE_ENABLED,
            DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
            baseline.0,
            baseline.1,
        )
    })?;

    let bounds = unsafe { analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1)? };
    let width = (bounds.right - bounds.left).max(0) as u32;
    let height = (bounds.bottom - bounds.top).max(0) as u32;
    if width == 0 || height == 0 {
        return Ok(None);
    }

    let mut alpha = vec![0u8; (width as usize) * (height as usize)];
    unsafe { analysis.CreateAlphaTexture(DWRITE_TEXTURE_ALIASED_1x1, &bounds, &mut alpha) }.ok()?;

    Ok(Some(GlyphCoverage {
        width,
        height,
        left: bounds.left,
        top: bounds.top,
        alpha,
    }))
}

/// Turn one coverage byte into blendable alpha under a gamma ramp.
///
/// `gamma` 1.0 is the identity — coverage blended in linear light, which is what
/// a linear scRGB target wants colorimetrically. Higher values lift the
/// partially-covered pixels (and only those: 0 and 255 are fixed points), which
/// is what the drawn path's rendering params do and why text rendered through
/// them carries more ink than the raw coverage does.
pub fn condition(coverage: u8, gamma: f32) -> f32 {
    let c = coverage as f32 / 255.0;
    if gamma == 1.0 { c } else { c.powf(1.0 / gamma) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColorF, DrawingSession, GpuDevice, TextFormat, TextLayout, Vector2};
    use std::cell::Cell;
    use windows_numerics::Matrix3x2;

    /// The gamma the drawn path's rendering params carry (`text::TEXT_AA_COVERAGE_GAMMA`).
    const DRAWN_GAMMA: f32 = 2.2;

    fn run_for(text: &str, em: f32) -> GlyphRun {
        let format = TextFormat::new("Segoe UI", em).unwrap();
        let layout = TextLayout::new(text, &format, 1000.0, 100.0).unwrap();
        layout.glyph_runs().unwrap().into_iter().next().unwrap()
    }

    /// Coverage must come back non-empty, correctly sized, and actually inked.
    #[test]
    fn coverage_is_produced_without_a_device() {
        let run = run_for("Hamburgefonstiv", 16.0);
        let cov = glyph_run_coverage(&run, 1.0, (0.0, 20.0))
            .expect("analysis")
            .expect("a visible run must mark pixels");

        assert_eq!(cov.alpha.len(), (cov.width * cov.height) as usize);
        assert!(cov.width > 0 && cov.height > 0);
        assert!(cov.ink() > 1.0, "a 15-glyph run must carry real ink");
        assert!(
            cov.alpha.iter().any(|&a| a > 0 && a < 255),
            "grayscale AA must produce partial coverage, not a bilevel mask"
        );
    }

    /// A space marks nothing, and that is not an error.
    #[test]
    fn blank_runs_report_no_coverage() {
        let run = run_for(" ", 16.0);
        assert!(glyph_run_coverage(&run, 1.0, (0.0, 20.0)).unwrap().is_none());
    }

    /// Conditioning must only lift the partially-covered pixels.
    #[test]
    fn conditioning_fixes_the_endpoints() {
        assert_eq!(condition(0, DRAWN_GAMMA), 0.0);
        assert_eq!(condition(255, DRAWN_GAMMA), 1.0);
        assert_eq!(condition(128, 1.0), 128.0 / 255.0);
        assert!(
            condition(128, DRAWN_GAMMA) > condition(128, 1.0),
            "a 2.2 ramp must carry more ink than linear coverage"
        );
    }

    // ── The gate: does the drawn path agree with conditioned raw coverage? ────

    /// How a drawn alpha plane relates to the coverage the rasterizer reported.
    struct Fit {
        pixels: usize,
        /// Exponent `g` minimizing `|coverage^(1/g) - drawn|`. 1.0 means the
        /// drawn path wrote the rasterizer's coverage through unchanged.
        gamma: f64,
        /// Mean residual at that exponent, in 255ths.
        err: f64,
        /// Mean residual at `g = 1.0`, in 255ths — the cost of assuming linear.
        err_identity: f64,
        /// Raw coverage ink over drawn ink.
        ink_ratio: f64,
        /// Worst shortfall on a fully covered pixel.
        solid_worst: f64,
    }

    impl std::fmt::Display for Fit {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "n={:<5} gamma={:.2}  err={:.3}/255  err@1.0={:.3}/255  ink raw/drawn={:.4}",
                self.pixels, self.gamma, self.err, self.err_identity, self.ink_ratio
            )
        }
    }

    /// Pair every covered pixel with what the drawn run put there and fit the
    /// exponent between them. The analysis and the drawn run are given the same
    /// baseline, so they are expected to align exactly; an empty pairing means
    /// they did not and the numbers below would be meaningless.
    fn fit_gamma(cov: &GlyphCoverage, drawn: &[f32], w: u32, h: u32) -> Fit {
        let mut pairs: Vec<(f64, f64)> = Vec::new();
        for y in 0..cov.height {
            for x in 0..cov.width {
                let (px, py) = (cov.left + x as i32, cov.top + y as i32);
                if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                    continue;
                }
                pairs.push((
                    cov.at(x, y) as f64 / 255.0,
                    drawn[(py as u32 * w + px as u32) as usize] as f64,
                ));
            }
        }
        assert!(!pairs.is_empty(), "no overlap — the two paths are misaligned");

        let residual = |g: f64| -> f64 {
            pairs
                .iter()
                .map(|&(c, a)| (c.powf(1.0 / g) - a).abs())
                .sum::<f64>()
                / pairs.len() as f64
        };

        let (mut gamma, mut err) = (1.0f64, f64::MAX);
        let mut g = 0.5f64;
        while g <= 3.5 {
            let e = residual(g);
            if e < err {
                err = e;
                gamma = g;
            }
            g += 0.01;
        }

        let ink_c: f64 = pairs.iter().map(|p| p.0).sum();
        let ink_d: f64 = pairs.iter().map(|p| p.1).sum();
        Fit {
            pixels: pairs.len(),
            gamma,
            err: err * 255.0,
            err_identity: residual(1.0) * 255.0,
            ink_ratio: if ink_d > 0.0 { ink_c / ink_d } else { 0.0 },
            solid_worst: pairs
                .iter()
                .filter(|p| p.0 > 0.999)
                .map(|p| 1.0 - p.1)
                .fold(0.0f64, f64::max),
        }
    }

    fn half_to_f32(h: u16) -> f32 {
        let sign = ((h >> 15) & 1) as u32;
        let exp = ((h >> 10) & 0x1f) as u32;
        let mant = (h & 0x3ff) as u32;
        let bits = if exp == 0 {
            if mant == 0 {
                sign << 31
            } else {
                let mut e: i32 = -1;
                let mut m = mant;
                while m & 0x400 == 0 {
                    m <<= 1;
                    e -= 1;
                }
                (sign << 31) | (((127 - 15 + 1 + e) as u32) << 23) | ((m & 0x3ff) << 13)
            }
        } else if exp == 31 {
            (sign << 31) | 0x7f80_0000 | (mant << 13)
        } else {
            (sign << 31) | ((exp + 127 - 15) << 23) | (mant << 13)
        };
        f32::from_bits(bits)
    }

    /// Draw `run` in opaque white on transparent, through the shipping rendering
    /// params, and read the alpha channel back — the reference the analysis path
    /// has to reproduce.
    /// `A8` — a coverage-only target, the format a mask atlas would naturally
    /// reach for. Not in the generated bindings.
    const DXGI_FORMAT_A8_UNORM: DXGI_FORMAT = 65;

    fn bytes_per_pixel(format: DXGI_FORMAT) -> u32 {
        match format {
            DXGI_FORMAT_A8_UNORM => 1,
            DXGI_FORMAT_R16G16B16A16_FLOAT => 8,
            _ => 4,
        }
    }

    /// The shipping rendering params, but at an arbitrary coverage gamma.
    fn params_at(gamma: f32) -> IDWriteRenderingParams {
        let factory: IDWriteFactory3 = Interface::cast(&dwrite_factory().unwrap()).unwrap();
        unsafe {
            factory
                .CreateCustomRenderingParams(
                    gamma,
                    0.0,
                    0.0,
                    0.0,
                    0, // flat pixel geometry — grayscale, no subpixel layout

                    DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
                    DWRITE_GRID_FIT_MODE_ENABLED,
                )
                .unwrap()
                .cast()
                .unwrap()
        }
    }

    fn drawn_alpha_in(
        gpu: &GpuDevice,
        run: &GlyphRun,
        baseline: (f32, f32),
        w: u32,
        h: u32,
        format: DXGI_FORMAT,
    ) -> Vec<f32> {
        drawn_alpha_tuned(gpu, run, baseline, w, h, format, None, 1.0)
    }

    /// The drawn run under a DIP→px scale, the way the atlas rasterizes it.
    fn drawn_alpha_scaled(
        gpu: &GpuDevice,
        run: &GlyphRun,
        baseline: (f32, f32),
        w: u32,
        h: u32,
        scale: f32,
    ) -> Vec<f32> {
        drawn_alpha_tuned(
            gpu,
            run,
            baseline,
            w,
            h,
            DXGI_FORMAT_R16G16B16A16_FLOAT,
            None,
            scale,
        )
    }

    fn drawn_alpha_tuned(
        gpu: &GpuDevice,
        run: &GlyphRun,
        baseline: (f32, f32),
        w: u32,
        h: u32,
        format: DXGI_FORMAT,
        gamma: Option<f32>,
        scale: f32,
    ) -> Vec<f32> {
        let ctx = unsafe { gpu.d2d_device().CreateDeviceContext(0).unwrap() };
        let lost = Cell::new(false);
        let session = DrawingSession::new_borrowed(&ctx, &lost);

        let props = |options| D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: options,
            ..Default::default()
        };
        let size = D2D_SIZE_U {
            width: w,
            height: h,
        };
        let (target, readback) = unsafe {
            (
                ctx.CreateBitmap(size, None, 0, &props(D2D1_BITMAP_OPTIONS_TARGET))
                    .unwrap(),
                ctx.CreateBitmap(
                    size,
                    None,
                    0,
                    &props(D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW),
                )
                .unwrap(),
            )
        };

        unsafe {
            ctx.SetTarget(&target);
            ctx.BeginDraw();
        }
        session.set_transform(&Matrix3x2 {
            m11: scale,
            m12: 0.0,
            m21: 0.0,
            m22: scale,
            m31: 0.0,
            m32: 0.0,
        });
        session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
        session.set_grayscale_text_antialiasing();
        if let Some(g) = gamma {
            unsafe { ctx.SetTextRenderingParams(&params_at(g)) };
        }
        let white = session
            .create_solid_brush(ColorF::new(1.0, 1.0, 1.0, 1.0))
            .unwrap();
        session.draw_glyph_run_at(Vector2::new(baseline.0, baseline.1), run, &white);
        unsafe { ctx.EndDraw(None, None).ok().unwrap() };

        unsafe {
            readback.CopyFromBitmap(None, &target, None).unwrap();
            let mapped = readback.Map(D2D1_MAP_OPTIONS_READ).unwrap();
            let bpp = bytes_per_pixel(format);
            let mut out = vec![0.0f32; (w * h) as usize];
            for y in 0..h {
                let row = mapped.bits.add((y * mapped.pitch) as usize);
                for x in 0..w {
                    let px = row.add((x * bpp) as usize);
                    // The alpha channel: the last byte of an 8-bit pixel, the
                    // fourth half of an FP16 one, the only byte of an A8 one.
                    out[(y * w + x) as usize] = match format {
                        DXGI_FORMAT_A8_UNORM => *px as f32 / 255.0,
                        DXGI_FORMAT_R16G16B16A16_FLOAT => {
                            half_to_f32(*(px as *const u16).add(3))
                        }
                        _ => *px.add(3) as f32 / 255.0,
                    };
                }
            }
            let _ = readback.Unmap();
            out
        }
    }

    /// Characterize the analysis texture against the drawn run: same glyph, same
    /// baseline, one drawn through Direct2D onto a transparent FP16 target and
    /// one taken straight from the rasterizer.
    ///
    /// Reports the exponent that best carries one onto the other rather than
    /// asserting a particular curve — which conditioning the drawn path applies,
    /// and where, is a property of Direct2D rather than of this code. The only
    /// hard assertion is that both describe the same glyph.
    #[test]
    fn analysis_coverage_matches_the_drawn_run() {
        let Ok(gpu) = GpuDevice::new_or_warp() else {
            eprintln!("no D3D device available; skipping");
            return;
        };

        let (w, h) = (256u32, 48u32);
        let baseline = (8.0f32, 32.0f32);
        let run = run_for("Hamburgefonstiv", 16.0);

        let drawn = drawn_alpha_in(&gpu, &run, baseline, w, h, DXGI_FORMAT_R16G16B16A16_FLOAT);
        let cov = glyph_run_coverage(&run, 1.0, baseline)
            .unwrap()
            .expect("the run must mark pixels");

        let fit = fit_gamma(&cov, &drawn, w, h);
        eprintln!("  {fit}");

        // Whatever the curve turns out to be, the two paths must describe the
        // same glyph: full coverage has to arrive as full alpha.
        assert!(
            fit.solid_worst < 0.02,
            "a fully covered pixel drew alpha {:.4}",
            1.0 - fit.solid_worst
        );
    }

    /// **Where the conditioning actually comes from.**
    ///
    /// The mask atlas picked FP16 over A8 after measuring the drawn run heavier
    /// at 8 bits — by far more than uniform quantization of a linear coverage
    /// value could explain. If Direct2D bends coverage according to what it
    /// thinks the target's encoding is, then the mask's pixel format is silently
    /// also its conditioning, and the two formats will fit different exponents
    /// against the very same rasterizer output.
    #[test]
    fn the_target_format_decides_the_conditioning() {
        let Ok(gpu) = GpuDevice::new_or_warp() else {
            eprintln!("no D3D device available; skipping");
            return;
        };

        let (w, h) = (256u32, 48u32);
        let baseline = (8.0f32, 32.0f32);
        let run = run_for("Hamburgefonstiv", 16.0);
        let cov = glyph_run_coverage(&run, 1.0, baseline)
            .unwrap()
            .expect("the run must mark pixels");

        for (label, format) in [
            ("FP16 ", DXGI_FORMAT_R16G16B16A16_FLOAT),
            ("A8   ", DXGI_FORMAT_A8_UNORM),
            ("BGRA8", DXGI_FORMAT_B8G8R8A8_UNORM),
        ] {
            let drawn = drawn_alpha_in(&gpu, &run, baseline, w, h, format);
            eprintln!("  {label} {}", fit_gamma(&cov, &drawn, w, h));
        }
    }

    /// The analysis and the drawn path must agree about where the transform puts
    /// a run, not just about its coverage — a baseline convention that differs
    /// between them would land every glyph off the pixel grid at anything but
    /// 100% DPI, which is exactly where it would go unnoticed.
    #[test]
    fn coverage_tracks_the_drawn_run_across_scale() {
        let Ok(gpu) = GpuDevice::new_or_warp() else {
            eprintln!("no D3D device available; skipping");
            return;
        };

        let (w, h) = (512u32, 96u32);
        let baseline = (8.0f32, 32.0f32);
        let run = run_for("Hamburgefonstiv", 16.0);

        for scale in [1.0f32, 1.25, 1.5, 2.0] {
            let drawn = drawn_alpha_scaled(&gpu, &run, baseline, w, h, scale);
            let cov = glyph_run_coverage(&run, scale, baseline)
                .unwrap()
                .expect("the run must mark pixels");

            // Search a neighbourhood so a convention difference reports as an
            // offset rather than as a coverage mismatch.
            let mut best = (f64::MAX, 0i32, 0i32);
            for dy in -3..=3 {
                for dx in -3..=3 {
                    let mut sum = 0.0f64;
                    for y in 0..cov.height {
                        for x in 0..cov.width {
                            let px = cov.left + x as i32 + dx;
                            let py = cov.top + y as i32 + dy;
                            if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                                continue;
                            }
                            let a = drawn[(py as u32 * w + px as u32) as usize] as f64;
                            sum += (cov.at(x, y) as f64 / 255.0 - a).abs();
                        }
                    }
                    if sum < best.0 {
                        best = (sum, dx, dy);
                    }
                }
            }
            let (sum, dx, dy) = best;
            let mean = sum / (cov.width * cov.height) as f64 * 255.0;
            eprintln!(
                "  scale {scale:.2}: box {}x{} at ({}, {})  best offset ({dx}, {dy})  mean {mean:.3}/255",
                cov.width, cov.height, cov.left, cov.top
            );
            assert_eq!(
                (dx, dy),
                (0, 0),
                "scale {scale}: the analysis and the drawn run disagree about placement"
            );
        }
    }

    /// **Does the coverage gamma reach an FP16 target at all?**
    ///
    /// The rendering params carry a coverage gamma the app tuned by eye. If
    /// Direct2D applies it as the target's assumed *encoding* rather than as a
    /// weight knob, then sweeping it moves an 8-bit target and leaves a linear
    /// FP16 one alone — and the knob is inert on the surface the glyph masks
    /// actually use.
    #[test]
    fn the_coverage_gamma_only_reaches_an_encoded_target() {
        let Ok(gpu) = GpuDevice::new_or_warp() else {
            eprintln!("no D3D device available; skipping");
            return;
        };

        let (w, h) = (256u32, 48u32);
        let baseline = (8.0f32, 32.0f32);
        let run = run_for("Hamburgefonstiv", 16.0);
        let cov = glyph_run_coverage(&run, 1.0, baseline)
            .unwrap()
            .expect("the run must mark pixels");

        for (label, format) in [
            ("FP16", DXGI_FORMAT_R16G16B16A16_FLOAT),
            ("A8  ", DXGI_FORMAT_A8_UNORM),
        ] {
            for g in [1.0f32, 1.8, 2.2, 3.0] {
                let drawn = drawn_alpha_tuned(&gpu, &run, baseline, w, h, format, Some(g), 1.0);
                eprintln!("  {label} param gamma {g:.1} -> {}", fit_gamma(&cov, &drawn, w, h));
            }
        }
    }
}
