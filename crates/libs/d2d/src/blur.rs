//! The one effect this crate carries: a Gaussian blur over a target's pixels.
//!
//! Everything else here draws a shape. A blur is the one thing no arrangement of shapes
//! produces: a widened stroke has a hard edge, and a figure reduced and carried back up
//! bilinearly is a tent kernel that shows its own footprint as facets along a diagonal.
//! Both are approximations of this, and this is what Direct2D has.
//!
//! # The extended range survives it
//!
//! Direct2D makes no guarantee about if or where it materializes a graph's intermediates,
//! and their default precision is limited-range. The device sets `16BPC_FLOAT` before any
//! caller sees the context, so a blur inherits it and the above-white and outside-Rec.709
//! values this stack carries pass through unclamped. There is no per-effect override here
//! for the same reason there is no format parameter anywhere else: one precision, set once.
//!
//! # What it does not open
//!
//! [`Draw::blurred`] takes a [`Blur`] and nothing else, so `DrawImage` — which accepts any
//! `ID2D1Image` and runs the image command graph to discover what it was handed — cannot be
//! reached with a bitmap. A blit is still `DrawBitmap`, and the general image draw still
//! does not exist.

use super::*;

/// A Gaussian blur over a source target's pixels.
///
/// It holds the source, because an effect reads its input when it is drawn rather than when
/// it is built: a source dropped between the two leaves the graph reading a released
/// surface. Rebuilt when the source is, which is what its `&Target` argument records.
pub struct Blur {
    /// The blurred image, resolved once at construction. `GetOutput` is a COM call and the
    /// answer does not change while the graph does not.
    output: ID2D1Image,
    /// The graph the output belongs to, and the surface it reads. Held and never read: an
    /// output outlives neither, and an effect reads its input when it is drawn rather than
    /// when it is built, so dropping either would leave the graph reading a released object.
    _effect: ID2D1Effect,
    _source: ID2D1Bitmap1,
}

impl Blur {
    pub(crate) fn image(&self) -> &ID2D1Image {
        &self.output
    }
}

impl Gpu {
    /// Builds a Gaussian blur of `source` at `sigma` standard deviations, in DIPs.
    ///
    /// `source` must be a target this device minted, and it must outlive nothing: the blur
    /// holds it. Draw the result with [`Draw::blurred`]. From here on, every binding of
    /// `source` flushes the context when it drops, so a blur drawn later in the same pass
    /// reads what that binding drew.
    ///
    /// The blur spreads about **three sigma**, so a source whose content reaches its own
    /// edge is cut off there — the border mode lets the blur spread past the edge rather
    /// than smearing the edge value outward, but it cannot invent what was never drawn.
    /// Allocate the source with room for the spread.
    ///
    /// # Errors
    ///
    /// The device refused the effect, or one of its properties.
    pub fn blur(&self, source: &Target, sigma: f32) -> Result<Blur> {
        windows_census::count!("d2d.effect.blur");
        debug_assert!(sigma > 0.0, "a blur at {sigma} sigma spreads nothing");
        let effect = unsafe { self.ctx().CreateEffect(&CLSID_D2D1GaussianBlur)? };
        source.feeds_effect.set(true);
        unsafe {
            effect.SetInput(0, &source.bitmap, true);
            // Quality rather than speed or balanced: the two cheaper paths downsample before
            // blurring, which is the artefact this exists instead of.
            effect
                .SetValue(
                    D2D1_GAUSSIANBLUR_PROP_OPTIMIZATION as u32,
                    D2D1_PROPERTY_TYPE_ENUM,
                    &D2D1_GAUSSIANBLUR_OPTIMIZATION_QUALITY.to_le_bytes(),
                )
                .ok()?;
            // Soft: the source is transparent outside its own bitmap, so the blur spreads
            // past the edge instead of clamping the edge value outward as a smear.
            effect
                .SetValue(
                    D2D1_GAUSSIANBLUR_PROP_BORDER_MODE as u32,
                    D2D1_PROPERTY_TYPE_ENUM,
                    &D2D1_BORDER_MODE_SOFT.to_le_bytes(),
                )
                .ok()?;
            effect
                .SetValue(
                    D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION as u32,
                    D2D1_PROPERTY_TYPE_FLOAT,
                    &sigma.to_le_bytes(),
                )
                .ok()?;
            let output = effect.GetOutput()?;
            Ok(Blur {
                output,
                _effect: effect,
                _source: source.bitmap.clone(),
            })
        }
    }
}

impl Draw<'_> {
    /// Draws a blur's output with the source's origin at `at`, in the bound target's DIPs.
    ///
    /// This is `DrawImage`, and it is the only call in this crate that reaches it. An effect
    /// output is what `DrawImage` is for: it has no bitmap to shortcut to, so the image
    /// command graph the call runs is the work rather than the overhead. A blit is still
    /// [`blit`](Self::blit).
    ///
    /// The blur spreads past the source's own box, so `at` places the source's top-left and
    /// the result reaches outside it on every side.
    pub fn blurred(&self, blur: &Blur, at: Vector2) {
        windows_census::count!("d2d.draw.image");
        unsafe {
            self.ctx.DrawImage(
                blur.image(),
                Some(&raw const at),
                None,
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square drawn into a source and blurred spreads light outside its own edge, keeps
    /// its centre brightest, and carries an above-white value through unclamped.
    ///
    /// Device-backed, and it says so when there is no device rather than reporting a pass:
    /// a headless runner has nothing to build a Direct2D device on, and a test that quietly
    /// succeeds there is a test that never ran.
    #[test]
    fn a_blur_spreads_past_its_source_and_keeps_the_extended_range() {
        let Ok(gpu) = Gpu::for_presentation() else {
            eprintln!("no Direct2D device: the blur's contract is untested on this machine");
            return;
        };
        const N: u32 = 64;
        const SIGMA: f32 = 4.0;
        // Above diffuse white, which is the value the blur exists to carry: the hero's curve
        // is authored there and a limited-range intermediate would clamp it to one.
        const LIT: f32 = 3.0;

        let source = gpu
            .offscreen((N, N), 96.0, Opacity::Translucent)
            .expect("offscreen");
        let out = gpu
            .offscreen((N, N), 96.0, Opacity::Translucent)
            .expect("offscreen");
        let blur = gpu.blur(&source, SIGMA).expect("blur");
        let ink = gpu
            .solid(Scrgb { r: LIT, g: LIT, b: LIT, a: 1.0 })
            .expect("solid brush");

        let mut pass = gpu.pass().expect("pass");
        {
            let draw = pass.draw(&source);
            draw.clear(Scrgb::TRANSPARENT);
            // A square in the middle, well inside the box so the spread has room.
            draw.fill(Rect::new(28.0, 28.0, 36.0, 36.0), &ink);
        }
        {
            let draw = pass.draw(&out);
            draw.clear(Scrgb::TRANSPARENT);
            draw.blurred(&blur, Vector2 { x: 0.0, y: 0.0 });
        }
        pass.end().expect("end");

        let read = gpu.read(&out).expect("readback");
        let alpha_at = |x: u32, y: u32| read.pixel(x, y)[3];
        let centre = alpha_at(32, 32);
        let outside = alpha_at(24, 32);
        let corner = alpha_at(1, 1);

        assert!(centre > 0.0, "the blur put nothing where the square was");
        assert!(
            outside > 0.0,
            "the blur spread nothing past the square's own edge"
        );
        assert!(
            outside < centre,
            "the falloff runs the wrong way: {outside} outside against {centre} at the centre"
        );
        assert!(
            corner < outside * 0.1,
            "the blur reached the corner at {corner}, which is not a falloff"
        );
        // Premultiplied storage, so the channel is the colour times its own alpha: an
        // above-white source is above white wherever the coverage is.
        let lit = read.pixel(32, 32)[0];
        assert!(
            lit > 1.0,
            "the blur clamped an above-white source to {lit}, so an intermediate was \
             limited-range"
        );
    }

    /// Each of several blurs drawn in one pass reads what its own source drew in that pass,
    /// whether the source drew a line or only cleared.
    ///
    /// Each source is drawn and then blurred before the next source is drawn. That puts every
    /// blur after the first one behind commands Direct2D has not run yet.
    #[test]
    fn each_blur_in_a_pass_reads_its_sources_drawing_from_that_pass() {
        let Ok(gpu) = Gpu::for_presentation() else {
            eprintln!("no Direct2D device: the blur's contract is untested on this machine");
            return;
        };
        let ink = gpu
            .solid(Scrgb { r: 1.0, g: 1.0, b: 1.0, a: 0.5 })
            .expect("solid brush");
        let line = gpu
            .path(|s| {
                s.figure(Vector2 { x: 8.0, y: 32.0 }, Figure::Hollow)
                    .lines(&[Vector2 { x: 56.0, y: 32.0 }])
                    .close(End::Open);
                Ok(())
            })
            .expect("path");
        let glows: Vec<_> = (0..3)
            .map(|_| {
                let source = gpu
                    .offscreen((64, 64), 96.0, Opacity::Translucent)
                    .expect("offscreen");
                let blur = gpu.blur(&source, 4.0).expect("blur");
                let out = gpu
                    .offscreen((64, 64), 96.0, Opacity::Translucent)
                    .expect("offscreen");
                (source, blur, out)
            })
            .collect();
        for lit in [true, false] {
            let mut pass = gpu.pass().expect("pass");
            for (source, blur, out) in &glows {
                {
                    let draw = pass.draw(source);
                    draw.clear(Scrgb::TRANSPARENT);
                    if lit {
                        draw.stroke(&line, &ink, Stroke::width(4.0));
                    }
                }
                let draw = pass.draw(out);
                draw.clear(Scrgb::TRANSPARENT);
                draw.blurred(blur, Vector2 { x: 0.0, y: 0.0 });
            }
            pass.end().expect("end");
            for (i, (_, _, out)) in glows.iter().enumerate() {
                // Six DIPs off the line, inside the spread and outside the stroke.
                let glow = gpu.read(out).expect("readback").pixel(32, 26)[3];
                if lit {
                    assert!(glow > 0.01, "blur {i} shows {glow} where its source drew a line");
                } else {
                    assert!(glow == 0.0, "blur {i} shows {glow} where its source only cleared");
                }
            }
        }
    }
}
