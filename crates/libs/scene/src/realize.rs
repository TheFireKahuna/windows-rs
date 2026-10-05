//! The four handles, the two storage machines, and the one brush chain. **Scene half.**
//!
//! ```text
//! Sprite.Brush = MaskBrush { Mask = <box | run | shape alpha>, Source = <paint brush> }
//! ```
//!
//! The chain is flat. A mask brush is never the mask or the source of another, because the
//! platform documents that combination as throwing, so a gradient is one premultiplied FP16
//! strip rather than a nested brush.
//!
//! Rasterized content is stored one of exactly two ways. Content keyed by an identity the
//! model minted lives in [`Resources`], refcounted by the sprites holding it and never
//! evicted. Content keyed by a *derived* value — a corner profile, a colour — lives in
//! [`Cache`], because a drag-resize or an animated fill can mint one key per frame; those
//! two families are exactly the ones quantized, so the key population stays bounded.

use crate::arena::{Arena, GlowState, Held, PROPS, Route, ShapeState};
use crate::sink::*;
use rustc_hash::FxHashMap;
use windows_color::{Radiance, Scrgb};
use windows_composition::{
    Animatable, BorderMode, Brush, Color, CompositionBrush, CompositionDrawingSurface,
    CompositionEffectFactory, CompositionGraphicsDevice,
    CompositionPath, CompositionPathGeometry, CompositionSurfaceBrush, CompositeMode,
    Compositor, EffectBorderMode, EffectGraph, SpriteVisual, Stretch, StrokeCap, StrokeJoin, Surface, Visual,
};
use windows_core::{Interface, Result};
use windows_d2d::{
    Draw, Extend, GlyphRun, Gpu, Opacity, SceneSurface, Solid, Stop, SurfaceDraw, note,
};
use windows_numerics::Vector2;
use windows_text::TextEngine;

// ── the four handles ────────────────────────────────────────────────────────────────

/// Everything needed to mint a composition object or rasterize a surface, and no fact about
/// the display it will appear on: those arrive as an [`Env`] at every operation.
///
/// Built and owned by the application. The font ladder is shared because two engines
/// interning names independently agree on index zero and disagree on everything after it,
/// and the symptom is a run drawn in the wrong face rather than an error; a compositor
/// proves its own precondition, since one cannot exist on a thread with no dispatcher queue;
/// and device loss is repaired by the GPU's owner through [`adopt`](Backends::adopt).
pub struct Backends {
    pub(crate) compositor: Compositor,
    pub(crate) gpu: Gpu,
    graphics: CompositionGraphicsDevice,
    text: TextEngine,
    /// Whether this device allocates coverage at one byte a pixel. No query reports it, so
    /// the first failed allocation answers it and every tile after takes the same route.
    masks_a8: core::cell::Cell<bool>,
    /// Opaque white, a mask's multiplicative identity, so a coverage cell is never retinted
    /// and one instance serves every tile for the life of the device.
    white: core::cell::OnceCell<Solid>,
    /// The glow graph's factory, minted once per compositor. The graph is fixed; only its
    /// brushes' sources and sigma move, so every lit node shares the one factory.
    glow: core::cell::OnceCell<CompositionEffectFactory>,
    backdrop: core::cell::OnceCell<CompositionEffectFactory>,
    /// The smooth-stroke graph's factory, minted on the first smooth stroke and shared by
    /// every one after; its brushes differ only in their sources and their sigma.
    smooth: core::cell::OnceCell<CompositionEffectFactory>,
}

/// Opaque white, the only colour a coverage cell draws in.
const WHITE: Scrgb = Scrgb {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

impl Backends {
    /// Requests publication of pending composition changes without waiting for completion.
    pub fn request_commit(&self) -> Result<()> {
        self.compositor.request_commit().map(drop)
    }

    /// The minted glow factory, for the debug probe to read its load status. `None` until
    /// the first lit node minted it.
    #[cfg(test)]
    pub(crate) fn minted_glow_factory(&self) -> Option<&CompositionEffectFactory> {
        self.glow.get()
    }

    /// Binds a compositor, a GPU and a font ladder together.
    ///
    /// `gpu` must be the only GPU used with `compositor`: when the compositor realizes a
    /// composition path it asks the geometry source for geometry belonging to a factory of
    /// its own choosing, and neither side of that callback checks the match, so a path built
    /// on a second GPU is content that never appears rather than an error.
    ///
    /// # Errors
    ///
    /// Fails if the graphics device or the text engine cannot be created.
    pub fn new(compositor: Compositor, gpu: &Gpu, fonts: FontLadder) -> Result<Self> {
        Ok(Self {
            graphics: gpu.graphics_device(&compositor)?,
            text: TextEngine::new(fonts)?,
            compositor,
            gpu: gpu.clone(),
            masks_a8: core::cell::Cell::new(true),
            white: core::cell::OnceCell::new(),
            glow: core::cell::OnceCell::new(),
            backdrop: core::cell::OnceCell::new(),
            smooth: core::cell::OnceCell::new(),
        })
    }

    /// Adopts a replacement GPU after device loss.
    ///
    /// Device loss takes the Direct2D device and everything drawn with it; the compositor
    /// and the text engine survive, so only the graphics device is rebuilt and the engine
    /// keeps its resolved faces.
    ///
    /// # Errors
    ///
    /// Fails if the replacement graphics device cannot be created.
    pub fn adopt(&mut self, gpu: &Gpu) -> Result<()> {
        self.graphics = gpu.graphics_device(&self.compositor)?;
        self.gpu = gpu.clone();
        self.white = core::cell::OnceCell::new();
        note!("scene", "graphics device adopted: brushes, cells and masks rebuild on the next pass");
        Ok(())
    }

    /// The ladder every run's face index resolves against. The shaping thread's own engine
    /// must be built over this one.
    #[must_use]
    pub fn ladder(&self) -> &FontLadder {
        self.text.ladder()
    }

    /// Whether coverage is allocated at a byte a pixel on this device.
    #[must_use]
    pub fn masks_are_a8(&self) -> bool {
        self.masks_a8.get()
    }

    /// A coverage surface, at one byte a pixel where the device allows it.
    ///
    /// An FP16 colour surface carries the same coverage, so the fallback changes the
    /// allocation and nothing else.
    fn surface(
        &self,
        px: (i32, i32),
        coverage: bool,
        o: Opacity,
    ) -> Result<CompositionDrawingSurface> {
        if coverage && self.masks_a8.get() {
            match self.graphics.mask(px) {
                Ok(surface) => return Ok(surface),
                Err(e) => {
                    note!("scene", "a8 mask probe failed — falling back to a colour surface, every mask after is fp16: {}", e);
                    self.masks_a8.set(false);
                }
            }
        }
        // The only surface allocator this crate names, so no UINT8 composition surface can
        // be minted for colour content.
        self.graphics.color(px, o)
    }

    fn white(&self) -> Result<&Solid> {
        if let Some(white) = self.white.get() {
            return Ok(white);
        }
        let white = self.gpu.solid(WHITE)?;
        Ok(self.white.get_or_init(|| white))
    }

    /// The glow graph's factory, minted on the first lit node and shared by every one
    /// after. The graph is fixed ([`glow_graph`]); a re-pointed tint or silhouette is a
    /// `set_source_parameter` on the brush, so the factory never re-keys.
    fn glow_factory(&self) -> Result<&CompositionEffectFactory> {
        if let Some(factory) = self.glow.get() {
            return Ok(factory);
        }
        let factory = self
            .compositor
            .create_effect_factory(&glow_graph(), &["blur.BlurAmount"])?;
        Ok(self.glow.get_or_init(|| factory))
    }

    fn backdrop_factory(&self) -> Result<&CompositionEffectFactory> {
        if let Some(factory) = self.backdrop.get() {
            return Ok(factory);
        }
        let graph = EffectGraph::GaussianBlur {
            name: "blur", sigma: 0.0, border: EffectBorderMode::Hard,
            input: Box::new(EffectGraph::Parameter("backdrop")),
        };
        let factory = self.compositor.create_effect_factory(&graph, &["blur.BlurAmount"])?;
        Ok(self.backdrop.get_or_init(|| factory))
    }

    /// The smooth-stroke graph's factory, minted on first use. See [`smooth_graph`].
    fn smooth_factory(&self) -> Result<&CompositionEffectFactory> {
        if let Some(factory) = self.smooth.get() {
            return Ok(factory);
        }
        let factory = self
            .compositor
            .create_effect_factory(&smooth_graph(), &["edge.BlurAmount"])?;
        Ok(self.smooth.get_or_init(|| factory))
    }

    /// Rasterizes one cell, surfacing the callback's error alongside the bridge's.
    ///
    /// One `BeginDraw` per graphics device at a time: a concurrent second fails
    /// `0x80131509`. Ownership enforces it rather than a rule, since every rasterization in
    /// this crate happens behind `&mut Scene` on one thread, and a presentation region uses
    /// its own device.
    ///
    /// The bridge's callback cannot fail, so the callback's error travels out in a slot and
    /// is raised once the bracket has closed: the surface publishes whatever the cell
    /// managed and the frame is not failed over one cell. `Ok(None)` is device loss; the
    /// cell is not cached, so a later pass rasterizes it again.
    fn rasterize(
        &self,
        px: (i32, i32),
        coverage: bool,
        o: Opacity,
        dpi: f32,
        draw: impl FnOnce(&Draw<'_>) -> Result<()>,
    ) -> Result<Option<CompositionDrawingSurface>> {
        let surface = self.surface(px, coverage, o)?;
        let mut raised = Ok(());
        let live = surface.draw(dpi, o, |d| raised = draw(d))?;
        raised?;
        if !live {
            note!("scene", "rasterize loss: the {}x{} cell (coverage {}) did not publish and nothing owns its recovery here", px.0, px.1, coverage);
        }
        Ok(live.then_some(surface))
    }

    /// A brush over `surface`, anchored top-left rather than at composition's centred
    /// default.
    pub(crate) fn brush(
        &self,
        surface: &impl Surface,
        stretch: Stretch,
    ) -> CompositionSurfaceBrush {
        let brush = self.compositor.create_surface_brush(surface);
        brush.set_alignment_ratio(0.0, 0.0);
        brush.set_stretch(stretch);
        brush
    }

    /// Builds a composition path from `verbs`, closing any figure the verbs leave open.
    pub(crate) fn path(&self, verbs: &[PathVerb]) -> Result<CompositionPath> {
        let path = self.gpu.path(|sink| {
            let mut open = false;
            for verb in verbs {
                // The three figure-starting verbs close whatever was left open; the other
                // three continue it, and one arriving with no figure open is dropped, because
                // a segment or an end written outside a figure puts the sink in an error state
                // that fails the whole path.
                let continues = matches!(
                    verb,
                    PathVerb::Line(_) | PathVerb::Cubic { .. } | PathVerb::End { .. }
                );
                if open != continues {
                    if open {
                        sink.close(windows_d2d::End::Open);
                        open = false;
                    } else {
                        continue;
                    }
                }
                match *verb {
                    PathVerb::Move { to, filled } => {
                        let kind = if filled {
                            windows_d2d::Figure::Filled
                        } else {
                            windows_d2d::Figure::Hollow
                        };
                        sink.figure(to, kind);
                        open = true;
                    }
                    PathVerb::Line(to) => {
                        sink.lines(core::slice::from_ref(&to));
                    }
                    PathVerb::Cubic { c1, c2, to } => {
                        sink.beziers(&[windows_d2d::Bezier { c1, c2, to }]);
                    }
                    PathVerb::RoundRect {
                        origin,
                        size,
                        radius,
                    } if size.x > 0.0 && size.y > 0.0 => {
                        let box_ = windows_d2d::Rect::sized(origin.x, origin.y, size.x, size.y);
                        sink.rounded_box(box_, [radius; 4]);
                    }
                    PathVerb::RoundRect { .. } => {}
                    PathVerb::Segment { from, to } => {
                        sink.figure(from, windows_d2d::Figure::Hollow)
                            .lines(&[to])
                            .close(windows_d2d::End::Open);
                    }
                    PathVerb::End { closed } => {
                        sink.close(if closed {
                            windows_d2d::End::Closed
                        } else {
                            windows_d2d::End::Open
                        });
                        open = false;
                    }
                }
            }
            if open {
                sink.close(windows_d2d::End::Open);
            }
            Ok(())
        })?;
        self.compositor.create_path(path.geometry())
    }

    /// Rasterizes the ground's grain: one screen-sized surface carrying a blue-noise tile
    /// at one texel per physical pixel.
    ///
    /// **It is a dither, and that is why it is screen-sized.** A grain is only a dither
    /// where one texel lands on one pixel; stretched, resampled or carried on a layer that
    /// moves, it is texture or it is shimmer. The compositor cannot tile a surface brush, so
    /// the tile is laid down here by Direct2D and handed over already repeated.
    ///
    /// `mid` is the value the stack beneath was pre-compensated against. `bands` is the
    /// peak-to-peak grain for each equal horizontal strip of the surface, per channel, in
    /// display-referred light: a code spans more linear light the higher it sits, so one
    /// amplitude for the whole window is the right size in one place and short everywhere
    /// brighter. Stepping it down the window is what keeps it worth the same number of
    /// codes throughout, and the steps are a tile apart in a quantity that moves by a few
    /// percent between them.
    ///
    /// The texels carry `mid + offset * band / GRAIN_ALPHA`, lifted by the layer's own
    /// opacity so that compositing scales it back down: source-over gives `dst + a * (c -
    /// dst)`, so over a stack pre-compensated by `(v - a * mid) / (1 - a)` this composites
    /// to exactly `v + offset * band`. The compositor has no additive blend and this is what
    /// stands in for one.
    pub(crate) fn raster_grain(
        &self,
        px: (i32, i32),
        mid: Scrgb,
        bands: &[[f32; 3]],
    ) -> Result<Option<CompositionDrawingSurface>> {
        if bands.is_empty() {
            return Ok(None);
        }
        let tile = grain_tile();
        let side = GRAIN_TILE as u32;
        let (w, h) = (px.0 as f32, px.1 as f32);
        // One brush per strip, because the amplitude is baked into the texels and the
        // compositor's only knob on a brush is a transform. Each is sixty-four squared.
        let mut brushes = Vec::with_capacity(bands.len());
        for band in bands {
            let texels: Vec<Scrgb> = tile
                .iter()
                .map(|&offset| {
                    let lift = |c: usize| offset * band[c] / GRAIN_ALPHA;
                    Scrgb {
                        r: mid.r + lift(0),
                        g: mid.g + lift(1),
                        b: mid.b + lift(2),
                        a: 1.0,
                    }
                })
                .collect();
            let source = self.gpu.pixels((side, side), &texels)?;
            // Nearest and wrap: a filtered grain is a blur of a grain, which is a grain with
            // its high frequencies spent — the only part of it that was doing any work.
            brushes.push((
                self.gpu
                    .tile(&source, Extend::Wrap, windows_d2d::Interp::Nearest)?,
                source,
            ));
        }
        // Authored at 96 DPI like every other raster that carries no snapped dimension: the
        // extent is already in physical pixels and the tile must not be scaled.
        self.rasterize(px, false, Opacity::Opaque, 96.0, |d| {
            let strips = brushes.len() as f32;
            for (at, (brush, _)) in brushes.iter().enumerate() {
                // Whole pixels, and the last strip takes the remainder, so the strips tile
                // the surface exactly however the height divides.
                let top = (h * at as f32 / strips).floor();
                let bottom = if at + 1 == brushes.len() {
                    h
                } else {
                    (h * (at + 1) as f32 / strips).floor()
                };
                d.fill(windows_d2d::Rect::new(0.0, top, w, bottom), brush);
            }
            Ok(())
        })
    }

    /// Rasterizes a gradient into one premultiplied FP16 strip carrying colour *and* alpha
    /// in the same texels.
    ///
    /// A composition gradient brush carries eight-bit stops, which would quantize a narrow
    /// alpha ramp (0.02 to 0.06) to almost nothing and need normalizing; FP16 alpha has no
    /// such floor, so that step does not exist. The strip is stretched to fill, so it
    /// carries none of the sprite's extent and a resize re-points nothing.
    ///
    /// `gain` multiplies every stop's presented colour, after the display transform, where
    /// the value is linear in the light the display emits.
    pub(crate) fn raster_ramp(
        &self,
        stops: &[(u16, Radiance)],
        spread: Spread,
        env: Env,
        beneath: Option<Beneath>,
        gain: f32,
    ) -> Result<Option<CompositionDrawingSurface>> {
        // The wire quantizes a stop's position to bound the identity that keys it; the
        // sampler takes a fraction. One pass per ramp declaration, not per frame.
        let ladder: Vec<(f32, Radiance)> = stops
            .iter()
            .map(|&(at, light)| (stop_fraction(at), light))
            .collect();
        if let Spread::Conic { center, start } = spread {
            return self.raster_conic(&ladder, center, start, env, beneath, gain);
        }
        // Along the axis for the two cardinal directions; square for a diagonal, which has
        // no single axis to lay a strip along, and for a radial, which has none at all. The
        // radial's profile carries no high-frequency content, so 64 texels miss it by under
        // a hundredth of an 8-bit level at the amplitudes a glow is authored with.
        let px = match spread {
            Spread::Horizontal => (256, 1),
            Spread::HorizontalFeathered { edge, .. } => (feather_texels(edge), 1),
            Spread::Vertical => (1, 256),
            // Colour is already resampled at 64 stops, so two texels per interval retain its
            // profile without a redundant vertical dimension.
            Spread::VerticalFeathered { .. } => (512, 128),
            Spread::DiagonalDown | Spread::DiagonalUp => (128, 128),
            Spread::Radial => (64, 64),
            Spread::Conic { .. } => unreachable!("rasterized above"),
        };
        // The stops are resampled rather than passed through: the drawing stack interpolates
        // linearly and the palette is authored in ICtCp, so the mix is taken perceptually,
        // and each sample goes through the display transform here — which is why a
        // capability change bumps the colour generation.
        const SAMPLES: usize = 64;
        let sampled: Vec<Stop> = (0..SAMPLES)
            .map(|at| {
                let t = at as f32 / (SAMPLES - 1) as f32;
                let color = brighter(env.apply(Radiance::sample(&ladder, t)), gain);
                Stop {
                    at: t,
                    color: beneath.map_or(color, |b| b.apply(color)),
                }
            })
            .collect();

        let (w, h) = (px.0 as f32, px.1 as f32);
        let box_ = windows_d2d::Rect::new(0.0, 0.0, w, h);
        // The strip is drawn in its own pixel space, so it is authored at 96 DPI whatever
        // the display's is: it carries no snapped dimension and is stretched to fill.
        self.rasterize(px, false, Opacity::Translucent, 96.0, |d| {
            d.clear(Scrgb::TRANSPARENT);
            let inset = match spread {
                Spread::HorizontalFeathered { inset, .. } => inset.clamp(0.0, 0.5),
                _ => 0.0,
            };
            let feather = match spread.edge() {
                Some(edge) if edge > 0.0 => Some(self.gpu.ramp(
                    &feather(edge.clamp(0.0, 0.5 - inset), inset),
                    Vector2 { x: 0.5, y: 0.0 },
                    Vector2 { x: w - 0.5, y: 0.0 },
                    Extend::Clamp,
                )?),
                _ => None,
            };
            let _coverage = feather.as_ref().map(|mask| {
                d.layer(
                    windows_d2d::Layer::mask_brush(mask)
                        .bounds(box_)
                        .replacing(),
                )
            });
            // A centred form is stretched with radii half the tile, so the profile's last
            // stop lands exactly on the edge; stretching that square tile into the sprite is
            // what makes the ellipse.
            if let Some((from, to)) = spread.ends() {
                let ramp = self.gpu.ramp(
                    &sampled,
                    Vector2 {
                        x: from[0] * w,
                        y: from[1] * h,
                    },
                    Vector2 {
                        x: to[0] * w,
                        y: to[1] * h,
                    },
                    Extend::Clamp,
                )?;
                d.fill(box_, &ramp);
            } else {
                let half = Vector2 {
                    x: w * 0.5,
                    y: h * 0.5,
                };
                let ramp = self.gpu.radial(&sampled, half, half, Extend::Clamp)?;
                d.fill(box_, &ramp);
            }
            Ok(())
        })
    }

    /// A smooth colour field, independent of path coverage and display scale, written texel
    /// by texel because no gradient brush sweeps an angle. Minted with the ramp resource
    /// only, never on a gain edit or an animation tick.
    fn raster_conic(
        &self,
        stops: &[(f32, Radiance)],
        center: [f32; 2],
        start: f32,
        env: Env,
        beneath: Option<Beneath>,
        gain: f32,
    ) -> Result<Option<CompositionDrawingSurface>> {
        const SIDE: u32 = 256;
        let pixels: Vec<Scrgb> = (0..SIDE * SIDE)
            .map(|at| {
                let x = (at % SIDE) as f32 + 0.5 - center[0] * SIDE as f32;
                let y = (at / SIDE) as f32 + 0.5 - center[1] * SIDE as f32;
                let t = (y.atan2(x) - start).rem_euclid(core::f32::consts::TAU)
                    / core::f32::consts::TAU;
                let color = brighter(env.apply(Radiance::sample(stops, t)), gain);
                beneath.map_or(color, |b| b.apply(color))
            })
            .collect();
        let source = self.gpu.pixels((SIDE, SIDE), &pixels)?;
        let side = SIDE as f32;
        self.rasterize(
            (SIDE as i32, SIDE as i32),
            false,
            Opacity::Translucent,
            96.0,
            |d| {
                d.clear(Scrgb::TRANSPARENT);
                d.blit(
                    &source,
                    windows_d2d::Rect::new(0.0, 0.0, side, side),
                    None,
                    windows_d2d::Interp::Linear,
                );
                Ok(())
            },
        )
    }

    /// Rasterizes one shaped run into an alpha-carrying coverage tile.
    ///
    /// The tile is a mask: the glyphs are drawn opaque white, the multiplicative identity,
    /// so the paint beside it in the brush chain supplies the colour unchanged.
    ///
    /// `segs` is a list because font fallback splits one line across faces, and each segment
    /// carries its own origin, so a bidi line — where visual order and advance order
    /// disagree — needs no second rule. The baseline is snapped here: `DrawGlyphRun` takes
    /// no options parameter, so it performs none of the baseline snapping the text-layout
    /// APIs do. Horizontal positions stay subpixel, because advances carry ideal metrics
    /// independent of display resolution. Coverage is raw coverage, with explicitly
    /// constructed rendering parameters, because inheriting the system's makes text
    /// systematically thin or fat and reads as a font choice.
    pub(crate) fn raster_run(
        &self,
        segs: &[GlyphSeg],
        glyphs: &[u16],
        floats: &[f32],
        ink: Ink,
        env: Env,
    ) -> Result<Option<CompositionDrawingSurface>> {
        let scale = env.scale();
        let px = (
            extent_px(ink.size.x, scale) as i32,
            extent_px(ink.size.y, scale) as i32,
        );
        let white = self.white()?;
        self.rasterize(px, true, Opacity::Translucent, env.dpi(), |d| {
            d.clear(Scrgb::TRANSPARENT);
            // The rendering mode is stated here and scoped by the guard, so it cannot leak
            // into whatever the surface's context draws next.
            let _params = d.text_params(self.text.rendering_params());
            // Every segment on a line shares one baseline, so the whole tile is nudged onto
            // a physical pixel once; per-segment rounding would break the shaped spacing.
            let dy = d.snap(ink.baseline.y) - ink.baseline.y;
            for seg in segs {
                // A face that does not resolve draws nothing rather than failing the frame,
                // so the rest of the line still renders.
                let Ok(face) = self.text.face(seg.face) else {
                    continue;
                };
                let Ok(face) = face.as_interface().cast::<windows_core::IUnknown>() else {
                    continue;
                };
                // Two floats per glyph, laid out as pairs by the emitter; a trailing odd
                // float is not an offset and is dropped rather than read as half of one.
                let (offsets, _) = span(floats, seg.offsets).as_chunks::<2>();
                d.glyphs(
                    Vector2 {
                        x: seg.origin.x,
                        y: seg.origin.y + dy,
                    },
                    &GlyphRun {
                        face: &face,
                        em: seg.em,
                        glyphs: span(glyphs, seg.glyphs),
                        advances: span(floats, seg.advances),
                        offsets,
                        bidi: seg.bidi,
                    },
                    white,
                );
            }
            Ok(())
        })
    }
}

fn span<T>(buffer: &[T], span: Span) -> &[T] {
    let (off, len) = (span.off as usize, span.len as usize);
    buffer.get(off..off + len).unwrap_or_default()
}

/// The affine correction a layer under the ground's grain carries.
///
/// The compositor has no additive blend, so the grain rides a layer at a constant
/// opacity `alpha` around a flat `mid`. Source-over gives `a*c + (1-a)*v`, so a layer
/// pre-compensated to `(v - a*mid) / (1 - a)` composites to `v` when the grain's texel is
/// `mid`, and to `v + d` when it is `mid + d/a` — the additive dither, reproduced.
///
/// Applied **after** the display transform rather than to the authored light, so it rests
/// on no claim about the transform being linear over these values. It is affine, and
/// source-over over an opaque base is a convex combination, so correcting each layer's
/// colour corrects the whole composite.
#[derive(Copy, Clone)]
pub(crate) struct Beneath {
    pub alpha: f32,
    pub mid: Scrgb,
}

impl Beneath {
    fn apply(self, c: Scrgb) -> Scrgb {
        let fix = |v: f32, m: f32| (v - self.alpha * m) / (1.0 - self.alpha);
        Scrgb {
            r: fix(c.r, self.mid.r),
            g: fix(c.g, self.mid.g),
            b: fix(c.b, self.mid.b),
            // Alpha is the layer's own coverage and the grain does not touch it: the
            // correction is on the colour a coverage is weighting, not on the weight.
            a: c.a,
        }
    }
}

/// The source-over alpha the ground's grain composites at.
///
/// Small keeps the pre-compensation below it gentle; too small pushes the grain sprite's
/// own colour far from the ground it sits on, where FP16's exponent is all it has.
pub const GRAIN_ALPHA: f32 = 1.0 / 16.0;

/// Peak-to-peak grain, in display codes.
///
/// Twice the classical half-code. The stages between this layer and the panel have slopes
/// below one — a display's own calibration LUT, then the panel's — so a grain sized to
/// exactly one code arrives smaller than one and the contours it broke re-form.
pub(crate) const GRAIN_CODES: f32 = 2.0;

/// Edge of the blue-noise tile, in texels. Tiled, so it must be toroidal.
pub(crate) const GRAIN_TILE: usize = 64;

/// Returns the toroidal blue-noise tile the ground's grain is cut from: `GRAIN_TILE`
/// squared zero-mean offsets in `[-0.5, 0.5]`, row-major.
///
/// Void-and-cluster (Ulichney). A blue spectrum is what makes a one-code grain invisible
/// as texture while still breaking a contour: white noise puts energy at low frequencies,
/// where the eye integrates it back into the blotches it was meant to remove, and an
/// ordered matrix puts it at a few exact frequencies, which reads as weave.
///
/// Built once for the process. The Gaussian is truncated at three sigma, so each rank
/// costs a fixed neighbourhood rather than a pass over the tile.
fn grain_tile() -> &'static [f32] {
    static TILE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TILE.get_or_init(|| {
        let last = (GRAIN_CELLS - 1) as f32;
        GRAIN_RANKS
            .chunks_exact(2)
            .map(|r| f32::from(u16::from_le_bytes([r[0], r[1]])) / last - 0.5)
            .collect()
    })
}

/// The tile's ranks, one little-endian `u16` each, in row-major order.
///
/// **Designed once and shipped, not built at start-up.** Void-and-cluster is a few thousand
/// scans of the whole tile and costs milliseconds even after the arithmetic below was cut to
/// fit a frame — and it would cost them on every machine, on the one path between a window
/// appearing and its first composited pixel. It is coefficient design, so it belongs with
/// the other things this project resolves before it runs.
///
/// `emit_tile` writes this file from [`build_grain_tile`], and a test regenerates it and
/// fails on any difference, so the two cannot drift.
static GRAIN_RANKS: &[u8; 2 * GRAIN_CELLS] = include_bytes!("grain.bin");

/// Cells in the tile.
const GRAIN_CELLS: usize = GRAIN_TILE * GRAIN_TILE;
/// Three sigma of the energy kernel, where the Gaussian is spent.
#[cfg(test)]
const GRAIN_REACH: usize = 5;

/// How many horizontal strips the grain's amplitude is stepped over.
///
/// The step a code spans follows the ground's own level, which moves by about a fifth down
/// a column and a twentieth across a row, so stepping it vertically carries nearly all of
/// it. Twenty-four puts each step under two percent of an amplitude that is already below
/// one code.
pub(crate) const GRAIN_STRIPS: i32 = 24;

/// Edge of that kernel.
#[cfg(test)]
const GRAIN_SIDE: usize = 2 * GRAIN_REACH + 1;

#[cfg(test)]
/// The energy field a point deposits, and the wrapped indices a splat writes through.
struct GrainKernel {
    weight: [f32; GRAIN_SIDE * GRAIN_SIDE],
    /// `wrap[c + d]` is `c + d - GRAIN_REACH` modulo the edge, for a centre `c` and a
    /// kernel column `d`. The tile is toroidal, so a splat wraps; indexed rather than
    /// divided, because this runs four hundred thousand times.
    wrap: [usize; GRAIN_TILE + 2 * GRAIN_REACH],
}

#[cfg(test)]
/// Void-and-cluster, over two mirrored energy fields.
///
/// The algorithm is a sequence of "where is the tightest cluster" and "where is the largest
/// void" questions — one per cell, each answered over the whole tile — so the shape of the
/// state is what decides the cost. The shape here is two fields rather than one:
///
/// - `lit` holds a point's energy where one sits and **negative infinity** where none does;
/// - `dark` holds the energy where none sits and **positive infinity** where one does.
///
/// Infinity absorbs addition, so a splat adds to both without testing either, and the two
/// questions become a plain maximum over `lit` and a plain minimum over `dark`: no branch
/// per cell, and a reduction that fits in vector registers. Toggling a cell is the two
/// fields trading a value, because whichever holds it always has it.
fn build_grain_tile() -> Vec<f32> {
    const SIGMA: f32 = 1.5;
    let mut kernel = GrainKernel {
        weight: [0.0; GRAIN_SIDE * GRAIN_SIDE],
        wrap: [0; GRAIN_TILE + 2 * GRAIN_REACH],
    };
    for dy in 0..GRAIN_SIDE {
        for dx in 0..GRAIN_SIDE {
            let (ox, oy) = (dx as f32 - GRAIN_REACH as f32, dy as f32 - GRAIN_REACH as f32);
            kernel.weight[dy * GRAIN_SIDE + dx] =
                (-(ox * ox + oy * oy) / (2.0 * SIGMA * SIGMA)).exp();
        }
    }
    for (i, at) in kernel.wrap.iter_mut().enumerate() {
        *at = (i + GRAIN_TILE - GRAIN_REACH) % GRAIN_TILE;
    }

    let mut lit = vec![f32::NEG_INFINITY; GRAIN_CELLS];
    let mut dark = vec![0.0f32; GRAIN_CELLS];

    // A deterministic scatter to start from: the tile must not vary between runs, or two
    // windows of one application dither differently.
    let seeds = GRAIN_CELLS / 10;
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut placed = 0;
    while placed < seeds {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let at = (state >> 33) as usize % GRAIN_CELLS;
        if lit[at] == f32::NEG_INFINITY {
            grain_light(&mut lit, &mut dark, at, &kernel);
            placed += 1;
        }
    }
    // Break the seed's own clusters before ranking anything, or the ranks inherit them.
    loop {
        let tight = grain_peak::<true>(&lit);
        grain_douse(&mut lit, &mut dark, tight, &kernel);
        let void = grain_peak::<false>(&dark);
        if void == tight {
            grain_light(&mut lit, &mut dark, tight, &kernel);
            break;
        }
        grain_light(&mut lit, &mut dark, void, &kernel);
    }

    let mut rank = vec![0u16; GRAIN_CELLS];
    // Phase one: pull the seeds apart, ranking downward from the last one placed.
    let (mut spent_lit, mut spent_dark) = (lit.clone(), dark.clone());
    for r in (0..seeds).rev() {
        let tight = grain_peak::<true>(&spent_lit);
        grain_douse(&mut spent_lit, &mut spent_dark, tight, &kernel);
        rank[tight] = r as u16;
    }
    // Phase two: fill the voids, ranking upward from the seeds.
    for r in seeds..GRAIN_CELLS {
        let void = grain_peak::<false>(&dark);
        grain_light(&mut lit, &mut dark, void, &kernel);
        rank[void] = r as u16;
    }
    // A rank is a position in the ordering; the offset is that position centred.
    let last = (GRAIN_CELLS - 1) as f32;
    rank.iter().map(|&r| f32::from(r) / last - 0.5).collect()
}

#[cfg(test)]
/// Puts a point at `at`: the fields trade the cell's energy, then the kernel goes down.
fn grain_light(lit: &mut [f32], dark: &mut [f32], at: usize, kernel: &GrainKernel) {
    lit[at] = dark[at];
    dark[at] = f32::INFINITY;
    grain_splat(lit, dark, at, 1.0, kernel);
}

#[cfg(test)]
/// Takes the point at `at` away, and its kernel with it.
fn grain_douse(lit: &mut [f32], dark: &mut [f32], at: usize, kernel: &GrainKernel) {
    dark[at] = lit[at];
    lit[at] = f32::NEG_INFINITY;
    grain_splat(lit, dark, at, -1.0, kernel);
}

#[cfg(test)]
/// Adds `sign` times the kernel around `at`, to both fields.
///
/// Both, and without testing either: an infinity absorbs the addition, so the field standing
/// in for "not a candidate here" stays exactly that.
fn grain_splat(lit: &mut [f32], dark: &mut [f32], at: usize, sign: f32, kernel: &GrainKernel) {
    let (cx, cy) = (at % GRAIN_TILE, at / GRAIN_TILE);
    for dy in 0..GRAIN_SIDE {
        let row = kernel.wrap[cy + dy] * GRAIN_TILE;
        let weights = &kernel.weight[dy * GRAIN_SIDE..(dy + 1) * GRAIN_SIDE];
        let columns = &kernel.wrap[cx..cx + GRAIN_SIDE];
        for (&w, &dx) in weights.iter().zip(columns) {
            let k = sign * w;
            lit[row + dx] += k;
            dark[row + dx] += k;
        }
    }
}

#[cfg(test)]
/// Index of the greatest value when `HIGH`, the least when not, first of any tie.
///
/// Eight accumulators, so each lane is an independent compare-and-select the compiler keeps
/// in one vector register; the horizontal step runs once at the end in a fixed order, so the
/// answer does not depend on how the reduction was split. The index comes from a second pass
/// rather than from a selected lane, because a conditional index update is what stops the
/// first one vectorizing — and the second pass stops at the first match.
fn grain_peak<const HIGH: bool>(v: &[f32]) -> usize {
    const LANES: usize = 8;
    let seed = if HIGH { f32::NEG_INFINITY } else { f32::INFINITY };
    let mut lane = [seed; LANES];
    for chunk in v.chunks_exact(LANES) {
        let chunk: &[f32; LANES] = chunk.try_into().expect("chunks_exact yields this width");
        for i in 0..LANES {
            if HIGH {
                if chunk[i] > lane[i] {
                    lane[i] = chunk[i];
                }
            } else if chunk[i] < lane[i] {
                lane[i] = chunk[i];
            }
        }
    }
    let mut best = seed;
    for &l in &lane {
        if HIGH {
            if l > best {
                best = l;
            }
        } else if l < best {
            best = l;
        }
    }
    // Eight compares and a bit scan per step, so the search for which cell held it runs at
    // the width the reduction did rather than one cell at a time.
    for (at, chunk) in v.chunks_exact(LANES).enumerate() {
        let mut hit = 0u8;
        for i in 0..LANES {
            hit |= u8::from(chunk[i] == best) << i;
        }
        if hit != 0 {
            return at * LANES + hit.trailing_zeros() as usize;
        }
    }
    unreachable!("the extreme was taken from this slice")
}

/// How many texels a feathered strip needs across its width for each taper to span
/// [`FEATHER_TEXELS`] of them.
///
/// The strip is stretched over its box, so the taper's texel count is fixed by its fraction
/// of the box rather than by the box's size.
fn feather_texels(edge: f32) -> i32 {
    if edge <= 0.0 {
        return 512;
    }
    ((FEATHER_TEXELS / edge).ceil() as i32).clamp(512, MAX_FEATHER_TEXELS)
}

/// The texels one taper of a feathered strip spans, so its eased profile survives the stretch.
const FEATHER_TEXELS: f32 = 16.0;

/// The widest feathered strip: a taper narrower than `FEATHER_TEXELS / MAX_FEATHER_TEXELS` of
/// its box takes fewer texels.
const MAX_FEATHER_TEXELS: i32 = 4096;

/// A squared-smoothstep coverage ladder, sixteen intervals per edge.
///
/// Zero slope at either end avoids a visible seam into the full-strength body, and the
/// coverage is authored directly so colour resampling cannot widen it.
fn feather(edge: f32, inset: f32) -> Vec<Stop> {
    const STEPS: usize = 16;
    let mut stops = Vec::with_capacity(2 * (STEPS + 1));
    for at in 0..=STEPS {
        let t = at as f32 / STEPS as f32;
        let eased = t * t * (3.0 - 2.0 * t);
        let color = Scrgb {
            a: eased * eased,
            ..WHITE
        };
        stops.push(Stop {
            at: inset + edge * t,
            color,
        });
        stops.push(Stop {
            at: 1.0 - inset - edge * t,
            color,
        });
    }
    stops.sort_by(|a, b| a.at.total_cmp(&b.at));
    stops
}

// ── the evicting cache ──────────────────────────────────────────────────────────────

/// A cell keyed by a value the application derived rather than declared.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum CellKey {
    Box(BoxKey),
    Solid(Q),
}

/// The corner profile a rounded box or outline is cut for, sized to the profile alone.
///
/// The nine-grid stretches one raster to any width and height with the corners intact, so
/// the key carries the profile and not the box. The fields are private and the constructors
/// snap, so no un-snapped key exists to lint for: the invariant holds at construction and no
/// later pass fixes it up.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct BoxKey {
    /// Quarter-pixel radii, in the order the corners are drawn.
    radius: [i32; 4],
    width: i32,
    open: Option<Side>,
    /// Two insets and the one flat pixel between them, which is what the middle slice
    /// stretches from.
    px: u32,
    inset: u32,
}

impl BoxKey {
    /// The key for corner profile `radius` at `scale`, snapping every dimension.
    #[must_use]
    pub fn new(radius: Corners, scale: f32) -> Self {
        Self::outline(radius, -1.0, None, scale)
    }

    /// A shared outline profile. Zero width has no coverage; the atlas centre is
    /// transparent, so a translucent fill does not reveal a solid backing.
    #[must_use]
    pub fn outline(radius: Corners, width: f32, open: Option<Side>, scale: f32) -> Self {
        let quarter = |r: f32| (snap_detail(r, scale) * scale * DETAIL_STEPS_PER_PX).round() as i32;
        let corners = [
            quarter(radius.tl),
            quarter(radius.tr),
            quarter(radius.br),
            quarter(radius.bl),
        ];
        // Rounded up to the pixel the widest arc ends inside, so the one middle pixel the
        // nine-grid stretches carries no part of a curve. One pixel wider would cost the
        // profile a pixel of radius for nothing, since a box has to hold two insets.
        let widest = corners.into_iter().max().unwrap_or(0).max(0) as u32;
        let inset = widest
            .div_ceil(DETAIL_STEPS_PER_PX as u32)
            .max((width.max(0.0) * scale).ceil() as u32)
            .max(1);
        Self {
            radius: corners,
            open,
            width: if width < 0.0 { -1 } else { quarter(width) },
            px: inset * 2 + 1,
            inset,
        }
    }

    /// The nine-grid's inset on each edge, in physical pixels.
    ///
    /// **The inset cuts the *source***, so it is stated in the raster's own units; a smaller
    /// number leaves the tail of the arc in the middle slice, which stretches it across the
    /// box and smears the edge.
    #[must_use]
    pub fn inset_px(&self) -> f32 {
        self.inset as f32
    }

    fn corners(&self, scale: f32) -> [f32; 4] {
        self.radius
            .map(|q| q as f32 / (DETAIL_STEPS_PER_PX * scale))
    }
}

struct Entry {
    /// Held because the brush holds it, and a resize reuses it.
    #[expect(dead_code, reason = "owns the surface the brush is painting from")]
    surface: CompositionDrawingSurface,
    brush: CompositionSurfaceBrush,
    built: Gen,
    /// When this entry was last reached, against the cache's own monotonic counter.
    used: u64,
}

/// Rasterized cells keyed by a derived value, evicted least-recently-used.
///
/// A hit is one map lookup and one recency stamp. Finding the oldest entry scans the map
/// instead, and runs only while the cache is at capacity.
pub struct Cache {
    map: FxHashMap<CellKey, Entry>,
    /// The one brush every [`Paint::Clear`] sprite carries. A colour brush holds no surface,
    /// so device loss leaves it valid.
    clear: Option<windows_composition::CompositionColorBrush>,
    cap: usize,
    /// Monotonic, incremented per lookup. At one lookup per nanosecond this takes five
    /// centuries to wrap.
    clock: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

/// How many cells the cache keeps.
///
/// Both families are small — a handful of corner profiles, a palette's worth of colours — so
/// the cap bounds a key population that turns out wider than expected rather than limiting
/// ordinary use.
pub const CACHE_CAP: usize = 256;

impl Default for Cache {
    fn default() -> Self {
        Self {
            map: FxHashMap::default(),
            clear: None,
            cap: CACHE_CAP,
            clock: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }
}

impl Cache {
    /// Drops every cell. The whole of what device loss does here; a merely stale entry is
    /// re-rasterized in place on its next use rather than swept.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// The transparent brush a [`Paint::Clear`] sprite carries, created on first use.
    fn clear_brush(&mut self, back: &Backends) -> CompositionBrush {
        self.clear
            .get_or_insert_with(|| back.compositor.create_color_brush(Color::rgba(0, 0, 0, 0)))
            .as_brush()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The brush for `key`, rasterizing it on a miss or when its generations went stale.
    ///
    /// `Ok(None)` reports device loss during the draw: the caller drops its binding and
    /// rebinds after recovery, down the same path a first bind takes.
    ///
    /// # Errors
    ///
    /// Fails if the cell's surface cannot be built, or if the cell's own draw failed.
    pub fn brush(
        &mut self,
        key: CellKey,
        back: &Backends,
        env: Env,
        now: Gen,
    ) -> Result<Option<&CompositionSurfaceBrush>> {
        self.clock += 1;
        let stamp = self.clock;
        if let Some(entry) = self.map.get_mut(&key)
            && key.deps().fresh(entry.built, now)
        {
            entry.used = stamp;
            self.hits += 1;
            // Re-borrowed rather than returned from the branch above: the mutable borrow
            // that stamped it cannot also be handed out as a shared one.
            return Ok(self.map.get(&key).map(|entry| &entry.brush));
        }

        self.misses += 1;
        let scale = env.scale();
        let Some(surface) =
            back.rasterize(key.px(), key.coverage(), key.opacity(), env.dpi(), |d| {
                key.draw(d, back, scale)
            })?
        else {
            return Ok(None);
        };
        let brush = back.brush(&surface, Stretch::Fill);
        while self.map.len() >= self.cap {
            self.evict();
        }
        self.map.insert(
            key.clone(),
            Entry {
                surface,
                brush,
                built: now,
                used: stamp,
            },
        );
        Ok(self.map.get(&key).map(|entry| &entry.brush))
    }

    fn evict(&mut self) {
        let Some(oldest) = self
            .map
            .iter()
            .min_by_key(|(_, entry)| entry.used)
            .map(|(key, _)| key.clone())
        else {
            return;
        };
        self.map.remove(&oldest);
        self.evictions += 1;
    }
}

impl CellKey {
    fn deps(&self) -> GenMask {
        match self {
            // A colour cell's whole content is already through the output transform, so a
            // capability change invalidates it and a DPI change does not.
            Self::Solid(_) => GenMask::LIGHT,
            Self::Box(_) => GenMask::GEOMETRY,
        }
    }

    fn px(&self) -> (i32, i32) {
        match self {
            // Four by four rather than one by one for filtering margin: a brush stretched
            // from a single texel samples its own edge.
            Self::Solid(_) => (4, 4),
            Self::Box(key) => (key.px as i32, key.px as i32),
        }
    }

    /// A coverage cell is one byte a pixel where the device allows it; a colour family that
    /// declared coverage would lose its colour.
    fn coverage(&self) -> bool {
        matches!(self, Self::Box(_))
    }

    fn opacity(&self) -> Opacity {
        match self {
            Self::Solid(q) if q.is_opaque() => Opacity::Opaque,
            // The corners are not covered, so a box cell carries real alpha.
            _ => Opacity::Translucent,
        }
    }

    fn draw(&self, d: &Draw<'_>, back: &Backends, scale: f32) -> Result<()> {
        match self {
            // The retained path's draw choke: the one place a quantized colour becomes the
            // value a surface is painted in. The field is private and `Q` keeps no copy of
            // what it was given, so the cell is painted in exactly the colour the key
            // round-trips to.
            Self::Solid(q) => d.clear(q.dequant()),
            Self::Box(key) => {
                d.clear(Scrgb::TRANSPARENT);
                if key.width == 0 {
                    return Ok(());
                }
                let white = back.white()?;
                let side = key.px as f32 / scale;
                let box_ = windows_d2d::Rect::new(0.0, 0.0, side, side);
                let [tl, tr, br, bl] = key.corners(scale);
                if key.width > 0 {
                    // Drawn inset by half its width with correspondingly smaller radii, and
                    // clipped on the open edge so an attached side shows no cap.
                    let width = key.width as f32 / (DETAIL_STEPS_PER_PX * scale);
                    let half = width * 0.5;
                    let centre = windows_d2d::Rect::new(half, half, side - half, side - half);
                    let radii = [tl, tr, br, bl].map(|r| (r - half).max(0.0));
                    let path = back.gpu.path(|sink| {
                        sink.rounded_box(centre, radii);
                        Ok(())
                    })?;
                    let mut clip = box_;
                    match key.open {
                        Some(Side::Left) => clip.left += width,
                        Some(Side::Top) => clip.top += width,
                        Some(Side::Right) => clip.right -= width,
                        Some(Side::Bottom) => clip.bottom -= width,
                        None => {}
                    }
                    let _clip = d.clip(clip);
                    d.stroke(&path, white, windows_d2d::Stroke::width(width));
                } else if tl == tr && tr == br && br == bl {
                    // A uniform profile has an analytic rounded rectangle, which Direct2D
                    // rasterizes by the pixels it touches rather than by tessellating.
                    d.fill(box_.rounded(tl), white);
                } else {
                    // Four independent radii have no analytic form, so this profile builds a
                    // path for this key alone — on a miss, not per frame.
                    let path = back.gpu.path(|sink| {
                        sink.rounded_box(box_, [tl, tr, br, bl]);
                        Ok(())
                    })?;
                    d.fill(&path, white);
                }
            }
        }
        Ok(())
    }
}

// ── shared resources ────────────────────────────────────────────────────────────────

/// What one resource row holds.
pub enum ResObj {
    /// The geometry and the path it was built from. Re-pointing takes the *path*, and the
    /// geometry exposes no getter for the one it holds, so the row carries both.
    Geom(CompositionPathGeometry, CompositionPath),
    /// A ramp strip, a run tile and a region buffer are all surface brushes, so re-pointing
    /// one is re-surfacing the object every sprite already holds. The surface is held for
    /// everything this crate rasterizes and `None` for a region, whose buffer the producer
    /// owns.
    Brush(CompositionSurfaceBrush, Option<CompositionDrawingSurface>),
    /// A ramp strip, with what it was drawn from so a smooth stroke can have it drawn
    /// brighter.
    Ramp(CompositionSurfaceBrush, CompositionDrawingSurface, Box<RampSource>),
    Dash([f32; 8], u8),
    /// A region slot declared but not yet pointed at a buffer. The buffer arrives out of
    /// band, as the one kernel handle that legitimately crosses from the present thread.
    Pending,
}

/// The stops and spread a ramp strip was drawn from, and the brighter strips drawn from them.
pub struct RampSource {
    pub stops: Vec<(u16, Radiance)>,
    pub spread: Spread,
    /// One strip per quantized gain a smooth stroke has asked for. A re-declared ramp
    /// re-surfaces each, so a sprite holding one follows the ramp as it would the base.
    pub brighter: Vec<(u16, CompositionSurfaceBrush, CompositionDrawingSurface)>,
}

/// A gain quantized for a key: 1/64 of a stop is well under a visible step in light.
#[must_use]
pub fn gain_key(gain: f32) -> u16 {
    (gain * 64.0).round().clamp(1.0, f32::from(u16::MAX)) as u16
}

/// A quantized gain's value.
#[must_use]
pub fn gain_of(key: u16) -> f32 {
    f32::from(key) / 64.0
}

/// A presented colour multiplied by `gain`, alpha unchanged.
///
/// Applied after the display transform, where scRGB is linear in emitted light, so the
/// display shows exactly `gain` times the light; the transform's tone stage is not run
/// again, and a colour near the display's peak is left to the compositor's clip.
#[must_use]
pub fn brighter(c: Scrgb, gain: f32) -> Scrgb {
    Scrgb { r: c.r * gain, g: c.g * gain, b: c.b * gain, a: c.a }
}

/// A resource and the two independent claims on it.
///
/// The model disclaims when its declaration goes away and a sprite releases when it is
/// destroyed or re-declared, in either order, so the entry lives until both are gone: a
/// resource neither outlives its last holder nor is pulled out from under one.
struct Row {
    generation: u32,
    family: u8,
    obj: ResObj,
    /// Sprites painting with it.
    rc: u32,
    /// Whether the model still declares it.
    claimed: bool,
}

/// One table over all five families, keyed by one index space.
///
/// The family rides on the id, so naming it wrong answers `None` exactly as a stale
/// generation does, and a disclaim is one lookup instead of five.
#[derive(Default)]
pub struct Resources(Vec<Option<Row>>);

impl Resources {
    fn at(&self, id: ResId) -> Option<&Row> {
        self.0
            .get(id.index())?
            .as_ref()
            .filter(|row| row.generation == id.generation() && row.family == id.family())
    }

    #[must_use]
    pub fn obj(&self, id: ResId) -> Option<&ResObj> {
        self.at(id).map(|row| &row.obj)
    }

    #[must_use]
    pub fn brush(&self, id: ResId) -> Option<&CompositionSurfaceBrush> {
        match self.obj(id)? {
            ResObj::Brush(brush, _) | ResObj::Ramp(brush, ..) => Some(brush),
            _ => None,
        }
    }

    /// The gains a ramp's brighter strips are held at, so a re-declaration can draw them
    /// again from the new stops.
    #[must_use]
    pub fn ramp_gains(&self, id: ResId) -> Vec<u16> {
        match self.obj(id) {
            Some(ResObj::Ramp(_, _, source)) => source.brighter.iter().map(|b| b.0).collect(),
            _ => Vec::new(),
        }
    }

    /// Returns ramp `id` drawn at `gain`, drawing and holding the strip on first use.
    ///
    /// `Ok(None)` where the ramp is not declared yet, or its strip was lost with the
    /// device; the sprite waits, as it does for a ramp it paints at its own light.
    ///
    /// # Errors
    ///
    /// Fails if the strip cannot be rasterized.
    pub fn brighter_ramp(
        &mut self,
        id: ResId,
        gain: u16,
        back: &Backends,
        env: Env,
    ) -> Result<Option<CompositionSurfaceBrush>> {
        let Some(ResObj::Ramp(_, _, source)) = self.row_mut(id).map(|row| &mut row.obj) else {
            return Ok(None);
        };
        if let Some(held) = source.brighter.iter().find(|b| b.0 == gain) {
            return Ok(Some(held.1.clone()));
        }
        let Some(surface) = back.raster_ramp(&source.stops, source.spread, env, None, gain_of(gain))? else {
            return Ok(None);
        };
        let brush = back.brush(&surface, Stretch::Fill);
        source.brighter.push((gain, brush.clone(), surface));
        Ok(Some(brush))
    }

    #[must_use]
    pub fn geom(&self, id: GeomId) -> Option<&CompositionPathGeometry> {
        match self.obj(id.erased())? {
            ResObj::Geom(geometry, _) => Some(geometry),
            _ => None,
        }
    }

    #[must_use]
    pub fn dashes(&self, id: DashId) -> &[f32] {
        match self.obj(id.erased()) {
            Some(ResObj::Dash(runs, len)) => &runs[..*len as usize],
            _ => &[],
        }
    }

    /// Re-points the held object rather than replacing it, so every sprite painting with the
    /// resource moves together and none of them needs a sweep.
    pub fn declare(&mut self, id: ResId, obj: ResObj) {
        if self.0.len() <= id.index() {
            self.0.resize_with(id.index() + 1, || None);
        }
        match &mut self.0[id.index()] {
            Some(row) if row.generation == id.generation() && row.family == id.family() => {
                row.claimed = true;
                match (&mut row.obj, obj) {
                    (ResObj::Geom(held, path), ResObj::Geom(_, next)) => {
                        held.set_path(&next);
                        *path = next;
                    }
                    (ResObj::Brush(held, surface), ResObj::Brush(_, Some(next))) => {
                        held.set_surface(&next);
                        *surface = Some(next);
                    }
                    (ResObj::Ramp(held, surface, source), ResObj::Ramp(_, next, next_source)) => {
                        held.set_surface(&next);
                        *surface = next;
                        // The declaration drew a strip for every gain held; each brush a
                        // sprite already paints with takes its replacement.
                        let RampSource { stops, spread, brighter } = *next_source;
                        for (gain, _, next) in brighter {
                            if let Some(held) = source.brighter.iter_mut().find(|b| b.0 == gain) {
                                held.1.set_surface(&next);
                                held.2 = next;
                            }
                        }
                        source.stops = stops;
                        source.spread = spread;
                    }
                    // A region slot re-declared keeps whatever it already points at.
                    (ResObj::Brush(_, None), ResObj::Pending) => {}
                    (slot, next) => *slot = next,
                }
            }
            slot => {
                *slot = Some(Row {
                    generation: id.generation(),
                    family: id.family(),
                    obj,
                    rc: 0,
                    claimed: true,
                });
            }
        }
    }

    /// Takes a sprite's hold. Retain before release, unconditionally: re-declaring the same
    /// resource must not let its count touch zero on the way through.
    pub fn retain(&mut self, holding: Option<Holding>) {
        if let Some(row) = holding.and_then(|held| self.row_mut(held.id())) {
            row.rc += 1;
        }
    }

    /// Gives up a sprite's hold, dropping the entry if that was the last claim on it.
    pub fn release(&mut self, holding: Option<Holding>) {
        let Some(id) = holding.map(Holding::id) else {
            return;
        };
        if let Some(row) = self.row_mut(id) {
            row.rc = row.rc.saturating_sub(1);
            if row.rc == 0 && !row.claimed {
                self.0[id.index()] = None;
            }
        }
    }

    /// The declaration is gone; the sprites holding it may not be.
    pub fn disclaim(&mut self, id: ResId) {
        if let Some(row) = self.row_mut(id) {
            row.claimed = false;
            if row.rc == 0 {
                self.0[id.index()] = None;
            }
        }
    }

    pub fn clear_region(&mut self, id: RegionId) {
        if let Some(row) = self.row_mut(id.erased()) {
            row.obj = ResObj::Pending;
        }
        self.disclaim(id.erased());
    }

    /// Returns the run and region brushes whose texels map to physical pixels.
    pub fn pixel_brushes(&self) -> impl Iterator<Item = &CompositionSurfaceBrush> {
        self.0
            .iter()
            .flatten()
            .filter_map(|row| match (&row.obj, row.family) {
                (ResObj::Brush(brush, _), RUN | REGION) => Some(brush),
                _ => None,
            })
    }

    fn row_mut(&mut self, id: ResId) -> Option<&mut Row> {
        self.0
            .get_mut(id.index())?
            .as_mut()
            .filter(|row| row.generation == id.generation() && row.family == id.family())
    }
}

// ── realizing one sprite ────────────────────────────────────────────────────────────

/// Everything realizing a sprite reaches for, and nothing else, so what these functions
/// touch is a fact the compiler checks.
pub struct Ctx<'a> {
    pub back: &'a Backends,
    pub env: Env,
    pub generation: Gen,
    pub res: &'a mut Resources,
    pub cache: &'a mut Cache,
    /// Visuals this realize minted and freed, for the applier to fold into its census.
    ///
    /// A glow is three visuals a node did not ask for by name, and the census is where the
    /// scene's standing cost is read. Counted here rather than in the applier because this
    /// is the half that knows whether one was built.
    pub minted: i32,
    pub freed: i32,
}

/// The construction that realizes a shape mask.
///
/// A total function of the mask's value and the sprite's held channels, so an author never
/// names a route; a clip-route sprite that later receives a trim, a dash phase or its own
/// clip is promoted onto the capture with the same geometry, so a shape's clip colliding
/// with the sink's costs a promotion rather than a wrong render. A halo also requires brush
/// alpha: clipping the visual cuts off its own shadow.
#[must_use]
pub fn route(mask: &Mask, held: bool, has_clip: bool, has_halo: bool) -> Route {
    match mask {
        Mask::Shape { space: PathSpace::Unit, .. } => Route::Capture,
        Mask::Shape {
            stroke: Some(_), ..
        } => Route::Capture,
        Mask::Shape { .. } if held || has_clip || has_halo => Route::Capture,
        Mask::Shape { .. } => Route::Clip,
        _ => Route::Capture,
    }
}

/// Builds or rebuilds a sprite's brush chain from the declaration held on its node.
///
/// Everything needed is on the node already, so device-loss recovery, a DPI change and a
/// first bind are one call: every brush is a pure function of a cache key or a resource id.
///
/// `glow` is the captured group's visual, which the caller resolves because it belongs to a
/// different node.
///
/// # Errors
///
/// Fails if a cell or a capture cannot be built.
pub fn realize(
    arena: &mut Arena,
    id: NodeId,
    glow: Option<&Visual>,
    ctx: &mut Ctx<'_>,
) -> Result<()> {
    let Some(row) = arena.painted(id) else {
        return Ok(());
    };
    let (mask, paint, halo) = (row.mask, row.paint, row.halo);
    let owned_clip = row.owns_the_clip();
    // The narrow view, taken here rather than stored: the brush and the shadow are the two
    // slots only a sprite carries, and both are written from this one call.
    let Some(sprite) = arena.visual(id).and_then(Visual::as_sprite) else {
        return Ok(());
    };
    sprite.set_pixel_snapping(matches!(mask, Mask::Run(_)) || matches!(paint,
        Paint::PresentedView { view: RegionView { sampling: RegionSampling::Pixels, .. }, .. })
        || matches!(paint, Paint::Presented { origin, .. } if origin != Vector2::zero()));
    let held = held_capture_channel(arena, id);
    let has_clip = arena.aux(id).is_some_and(|aux| aux.clip.is_some());
    let has_halo = halo.is_some() || matches!(paint, Paint::Captured { .. });
    let route = route(&mask, held, has_clip, has_halo);

    let (alpha, key) = mask_brush(arena, id, &mask, route, ctx)?;
    // A shape leaving the clip route takes its clip with it, or it is masked twice by itself
    // and an outward stroke is cut in half along the fill's own outline. The other way onto
    // the capture is the sink claiming the slot, and there the sink's clip is already on the
    // visual.
    if owned_clip
        && route == Route::Capture
        && !has_clip
        && let Some(visual) = arena.visual(id)
    {
        visual.clear_clip();
    }
    // The reverse: a mask that stops being a shape leaves a capture behind whose channels
    // would keep taking writes nothing renders. Guarded on the row existing, because a sprite
    // that never took the capture route must not be given one to hold the absence in.
    if !matches!(mask, Mask::Shape { .. }) && arena.has_aux(id) {
        arena.aux_mut(id).shape = None;
    }

    // One capture serves both halves of a captured glow: the brush the chain paints with and
    // the silhouette the shadow blurs are the same surface.
    let captured = match (paint, glow) {
        (Paint::Captured { .. }, Some(source)) => Some(ctx.back.compositor.capture(
            source,
            arena.size(id),
            ctx.env.scale(),
        )),
        _ => None,
    };
    let smooth = match mask {
        Mask::Shape { stroke: Some(style @ StrokeStyle { smooth: true, .. }), .. }
            if route == Route::Capture => Some(style.width),
        _ => None,
    };
    let source = match paint {
        Paint::PresentedView { region, view } => presented_view(arena, id, region, PresentedSource::View(view), ctx),
        Paint::Presented { region, origin } if origin != Vector2::zero() =>
            presented_view(arena, id, region, PresentedSource::Pixels(origin), ctx),
        _ => {
        if arena.has_aux(id) {
            arena.aux_mut(id).region_view = None;
        }
        let gain = smooth.map_or(1.0, |width| smooth_edge().boost(width * ctx.env.scale()));
        paint_brush(&paint, captured.as_ref(), gain, ctx)?
        }
    };

    // A mask and a paint arrive as separate ops in either order, so a half-declared sprite
    // waits rather than failing. `Mask::None` skips the outer brush entirely, because a mask
    // brush in the chain disqualifies a presented buffer from a display plane.
    let chain = match (&alpha, &source, smooth) {
        (Some(alpha), Some(source), Some(_)) => Some(smooth_chain(alpha, source, ctx)?),
        (Some(alpha), Some(source), None) => {
            let chain = ctx.back.compositor.create_mask_brush();
            chain.set_mask(alpha);
            chain.set_source(source);
            Some(chain.as_brush())
        }
        _ => None,
    };
    // Built before it is bound, because the glow needs it twice: as the silhouette its blur
    // reads, and to decide where the paint goes. A lit node paints through a child of its
    // own, since a visual's own brush draws under its children.
    let brush = match (&chain, &source) {
        (Some(chain), _) => Some(chain.clone()),
        (None, Some(source)) => Some(source.clone()),
        _ => None,
    };
    let lit = cast_glow(arena, id, &sprite, halo, &paint, brush.as_ref(), captured, ctx)?;
    let target = lit.as_ref().unwrap_or(&sprite);
    match &brush {
        Some(brush) => target.set_brush(brush),
        None => target.clear_brush(),
    }

    // The one line naming what the glow's caster would paint, if it lit: the silhouette
    // content is what the blur reads, and a capture-source paint is the case that has to
    // be watched.
    if has_halo {
        let box_ = arena.size(id);
        note!(
            "scene",
            "glow id={} inputs: paint={:?}, route={:?}, alpha-cell={}, silhouette-from={}, box=({:.0},{:.0})",
            id.index(),
            paint,
            route,
            alpha.is_some(),
            if chain.is_some() { "chain" } else { "source" },
            box_.x,
            box_.y,
        );
    }

    let built = ctx.generation;
    if let Some(row) = arena.painted_mut(id) {
        row.chain = chain;
        row.alpha = alpha;
        row.key = key;
        row.route = route;
        row.built = built;
    }
    Ok(())
}

/// True where a channel only the capture route can carry is held.
///
/// A held channel counts whether or not its animation is running, so a control does not
/// demote to the clip route between two hovers.
fn held_capture_channel(arena: &Arena, id: NodeId) -> bool {
    [Prop::TrimStart, Prop::TrimEnd, Prop::DashOffset]
        .iter()
        .any(|prop| arena.held(id, &PROPS[*prop as usize]) != Held::Free)
}

/// The alpha half of the chain, and the box key it was cut from.
fn mask_brush(
    arena: &mut Arena,
    id: NodeId,
    mask: &Mask,
    route: Route,
    ctx: &mut Ctx<'_>,
) -> Result<(Option<CompositionBrush>, Option<BoxKey>)> {
    let scale = ctx.env.scale();
    let size = arena.size(id);
    match *mask {
        Mask::Box { radius } | Mask::Outline { radius, .. } => {
            let (width, open) = outline_of(mask, size, scale);
            let key = BoxKey::outline(fit(radius, size, scale), width, open, scale);
            let Some(cell) =
                ctx.cache
                    .brush(CellKey::Box(key), ctx.back, ctx.env, ctx.generation)?
            else {
                return Ok((None, None));
            };
            // Nine-slice, so one raster serves any width and height with exact corners, and
            // it reaches the mask slot as the base brush type. The inset cuts the source, so
            // the inset scale is the reciprocal of the display scale: what is painted is
            // `inset * inset_scale` in the visual's own units, and a corner cut at n pixels
            // would otherwise be painted n DIPs wide.
            let nine = ctx.back.compositor.create_nine_grid_brush();
            let inset = key.inset_px();
            nine.set_insets(inset, inset, inset, inset);
            nine.set_inset_scales(1.0 / scale);
            nine.set_center_hollow(width > 0.0);
            nine.set_source(cell);
            Ok((Some(nine.as_brush()), Some(key)))
        }
        Mask::Run(run) => Ok((ctx.res.brush(run.erased()).map(Brush::as_brush), None)),
        Mask::Shape { geom, stroke, space } => {
            let Some(path) = ctx.res.geom(geom).cloned() else {
                if arena.has_aux(id) { arena.aux_mut(id).shape = None; }
                return Ok((None, None));
            };
            match route {
                Route::Clip => {
                    // No mask brush at all: the geometry occupies the visual's one clip
                    // slot, with a soft border for an antialiased edge.
                    let clip = ctx.back.compositor.create_geometric_clip(&path);
                    if let Some(visual) = arena.visual(id) {
                        visual.set_border_mode(BorderMode::Soft);
                        visual.set_clip(Some(&clip));
                    }
                    if arena.has_aux(id) {
                        arena.aux_mut(id).shape = None;
                    }
                    Ok((None, None))
                }
                Route::Capture => {
                    if space == PathSpace::Unit
                        && let Some(state) = arena.aux(id).and_then(|aux| aux.shape.as_ref())
                        && let Some(fitted) = &state.fitted
                        && fitted.geom == geom && fitted.style == stroke
                        && GenMask::GEOMETRY.fresh(fitted.built, ctx.generation)
                    {
                        return Ok((Some(state.capture.brush.as_brush()), None));
                    }
                    let mut state = build_capture(&path, stroke, size, scale, ctx);
                    if space == PathSpace::Unit {
                        let previous = arena.aux_mut(id).shape.as_mut().and_then(|shape| {
                            if shape.fitted.as_ref().is_some_and(|fitted| fitted.geom == geom && fitted.style == stroke) {
                                shape.fitted.take()
                            } else { None }
                        });
                        fit_capture(&mut state, arena.visual(id).unwrap(), geom, stroke, previous, scale, ctx);
                    }
                    let brush = state.capture.brush.as_brush();
                    arena.aux_mut(id).shape = Some(state);
                    Ok((Some(brush), None))
                }
            }
        }
        Mask::None => Ok((None, None)),
    }
}

/// The stroke width a box mask is cut with, clamped against the box.
///
/// A nine-grid does not clamp its own insets, so an outline wider than half the shorter side
/// would overlap its opposite edge. `-1.0` is the filled form, which has no width.
fn outline_of(mask: &Mask, size: Vector2, scale: f32) -> (f32, Option<Side>) {
    match *mask {
        Mask::Outline { width, open, .. } => (
            width
                .max(0.0)
                .min((size.x.min(size.y) * 0.5 - 1.0 / scale).max(0.0)),
            open,
        ),
        _ => (-1.0, None),
    }
}

/// An off-tree shape visual captured through a visual surface, filled or stroked with opaque
/// white.
///
/// The only route that can stroke, trim or dash, because those properties live on a sprite
/// shape; a sprite shape's fill and stroke brushes do not accept a surface brush, so an FP16
/// colour cannot reach a shape directly and the capture carries alpha alone. A visual surface
/// is a live capture rather than a snapshot, so re-pointing the geometry reaches the screen
/// through it.
///
/// A promotion keeps whatever the channels had reached, because they live on the node and
/// not in this state: a shape that acquires a trim mid-animation does not restart.
fn build_capture(
    path: &CompositionPathGeometry,
    stroke: Option<StrokeStyle>,
    size: Vector2,
    scale: f32,
    ctx: &mut Ctx<'_>,
) -> ShapeState {
    let host = ctx.back.compositor.create_shape_visual();
    let host_visual: &Visual = &host;
    host_visual.set_border_mode(BorderMode::Soft);
    let shape = ctx.back.compositor.create_sprite_shape(path);
    // Opaque white: the capture is a mask, so its colour comes from the paint beside it, and
    // white is the multiplicative identity that leaves that paint alone.
    let white = ctx
        .back
        .compositor
        .create_color_brush(Color::rgb(255, 255, 255));
    match stroke {
        Some(stroke) => {
            shape.set_stroke_brush(&white);
            shape.set_stroke_thickness(stroke.width);
            shape.set_stroke_caps(cap_of(stroke.cap));
            shape.set_stroke_dash_cap(cap_of(stroke.cap));
            shape.set_stroke_join(join_of(stroke.join));
            if !stroke.dash.is_none() {
                shape.set_stroke_dashes(ctx.res.dashes(stroke.dash));
            }
        }
        None => shape.set_fill_brush(&white),
    }
    host.shapes().append(&shape);
    let capture = ctx.back.compositor.capture(host_visual, size, scale);
    let state = ShapeState {
        host,
        shape,
        geom: path.clone(),
        capture,
        fitted: None,
    };
    state.resize(size, scale);
    state
}

fn fit_capture(state: &mut ShapeState, visual: &Visual, geom: GeomId, style: Option<StrokeStyle>, previous: Option<Box<crate::arena::FittedShape>>, scale: f32, ctx: &Ctx<'_>) {
    let compositor = &ctx.back.compositor;
    state.shape.set_stroke_non_scaling(true);
    let extent = compositor.create_expression_animation("Max(v.Size * dpi, Vector2(1, 1))");
    extent.set_reference_parameter("v", visual);
    extent.set_scalar_parameter("dpi", scale);
    state.host.start_animation("Size", &extent);
    state.capture.surface.start_animation("SourceSize", &extent);
    let mapping = compositor.create_expression_animation("Max(v.Size, Vector2(0, 0)) * dpi");
    mapping.set_reference_parameter("v", visual);
    mapping.set_scalar_parameter("dpi", scale);
    state.shape.start_animation("Scale", &mapping);
    state.capture.brush.set_stretch(Stretch::None);
    state.capture.brush.set_source_transform(Vector2::zero(), Vector2::new(1.0 / scale, 1.0 / scale));
    state.capture.brush.set_linear_sampling();

    // The ordinary stroke channels remain in DIPs; only the non-scaling capture uses pixels.
    let mut fitted = previous.unwrap_or_else(|| {
        let stroke = compositor.create_property_set();
        stroke.insert_scalar("StrokeThickness", style.map_or(0.0, |s| s.width));
        stroke.insert_scalar("StrokeDashOffset", 0.0);
        Box::new(crate::arena::FittedShape { stroke, geom, style, built: ctx.generation })
    });
    fitted.built = ctx.generation;
    for (property, expression) in [
        ("StrokeThickness", "p.StrokeThickness * dpi"),
        ("StrokeDashOffset", "p.StrokeDashOffset"),
    ] {
        let animation = compositor.create_expression_animation(expression);
        animation.set_reference_parameter("p", &fitted.stroke);
        animation.set_scalar_parameter("dpi", scale);
        state.shape.start_animation(property, &animation);
    }
    state.fitted = Some(fitted);
}

/// Resolves a shared paint source. Presented views own their mapping brush on the sprite.
///
/// `gain` multiplies a solid's or a ramp's presented colour: a smooth
/// stroke's paint is drawn brighter by the light its edge filter spreads out of the centre.
/// A captured or presented paint has no light of its own to scale and ignores it.
fn paint_brush(
    paint: &Paint,
    captured: Option<&windows_composition::Captured>,
    gain: f32,
    ctx: &mut Ctx<'_>,
) -> Result<Option<CompositionBrush>> {
    Ok(match *paint {
        Paint::Solid(light) => {
            // The retained path's draw choke: the one place in this crate a scene-referred
            // value becomes a display-referred one.
            let key = CellKey::Solid(Q::new(brighter(ctx.env.apply(light), gain)));
            ctx.cache
                .brush(key, ctx.back, ctx.env, ctx.generation)?
                .map(Brush::as_brush)
        }
        Paint::Ramp(id) if gain_key(gain) != gain_key(1.0) => ctx
            .res
            .brighter_ramp(id.erased(), gain_key(gain), ctx.back, ctx.env)?
            .map(|brush| brush.as_brush()),
        Paint::Ramp(id) => ctx.res.brush(id.erased()).map(Brush::as_brush),
        Paint::Backdrop { sigma } => {
            let brush = ctx.back.backdrop_factory()?.create_brush();
            brush.set_source_parameter("backdrop", &ctx.back.compositor.create_backdrop_brush()?);
            brush.properties().insert_scalar("blur.BlurAmount", sigma);
            Some(brush.as_brush())
        }
        Paint::Presented { region, .. } => ctx.res.brush(region.erased()).map(Brush::as_brush),
        Paint::PresentedView { .. } => unreachable!("view brushes are owned by their sprite"),
        Paint::Captured { .. } => captured.map(|held| held.brush.as_brush()),
        Paint::Clear => Some(ctx.cache.clear_brush(ctx.back)),
        Paint::None => None,
    })
}

pub struct PresentedView {
    pub(crate) brush: CompositionSurfaceBrush,
    source: PresentedSource,
    scale: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum PresentedSource {
    View(RegionView),
    Pixels(Vector2),
}

impl Drop for PresentedView {
    fn drop(&mut self) {
        self.brush.stop_animation("Scale");
        self.brush.stop_animation("Offset");
    }
}

fn presented_view(
    arena: &mut Arena,
    id: NodeId,
    region: RegionId,
    source: PresentedSource,
    ctx: &mut Ctx<'_>,
) -> Option<CompositionBrush> {
    let Some(surface) = ctx.res.brush(region.erased()).and_then(CompositionSurfaceBrush::surface) else {
        if arena.has_aux(id) {
            arena.aux_mut(id).region_view = None;
        }
        return None;
    };
    let visual = arena.visual(id)?.clone();
    let scale = ctx.env.scale();
    let held = &mut arena.aux_mut(id).region_view;
    if let Some(held) = held.as_ref().filter(|held| held.source == source && held.scale == scale) {
        held.brush.set_surface(&surface);
        return Some(held.brush.as_brush());
    }
    *held = None;
    let brush = ctx.back.brush(&surface, Stretch::None);
    brush.set_alignment_ratio(0.0, 0.0);
    let origin = match source {
        PresentedSource::View(view) => Vector2::new(view.rect[0], view.rect[1]),
        PresentedSource::Pixels(origin) => origin,
    };
    let origin = Vector2::new((origin.x * scale).round(), (origin.y * scale).round());
    match source {
        PresentedSource::Pixels(_) | PresentedSource::View(RegionView { sampling: RegionSampling::Pixels, .. }) => {
            brush.set_nearest_sampling();
            brush.set_source_transform(Vector2::new(-origin.x / scale, -origin.y / scale),
                Vector2::new(1.0 / scale, 1.0 / scale));
        }
        PresentedSource::View(view) => {
            let span = Vector2::new(((view.rect[2] * scale).round() - origin.x).max(1.0),
                ((view.rect[3] * scale).round() - origin.y).max(1.0));
            brush.set_linear_sampling();
            for (property, expression) in [
                ("Scale", "v.Size / span"),
                ("Offset", "-origin * v.Size / span"),
            ] {
                let expression = ctx.back.compositor.create_expression_animation(expression);
                expression.set_reference_parameter("v", &visual);
                expression.set_vector2_parameter("span", span);
                expression.set_vector2_parameter("origin", origin);
                brush.start_animation(property, &expression);
            }
        }
    }
    let result = brush.as_brush();
    *held = Some(Box::new(PresentedView { brush, source, scale }));
    Some(result)
}

/// Casts, or removes, the light this node spends past its own silhouette.
///
/// **The blur carries alpha and never colour.** So the caster paints the silhouette in
/// the alpha it already has, the compositor blurs it through a Gaussian whose output
/// never leaves its own float pipeline, and a composite node multiplies that coverage
/// into an FP16 cell drawn at the same draw choke every other paint goes through. There
/// is no mask brush and no 8-bit read-back of a blurred result: the multiply happens
/// inside the effect graph, which is why the halo keeps the chroma and the above-white
/// range the tint is authored with ([07-COLOR.md §6.1] of the GUI spec).
///
/// The silhouette is whatever the node paints, so a node with no paint yet casts nothing and
/// the op that brings the paint builds the glow. A captured glow blurs the group it paints
/// with, which is the same brush.
///
/// Returns the sprite the node's paint belongs on: a child while the node is lit, because a
/// visual's own brush draws *under* its children and the halo has to sit below the paint.
///
/// # Errors
///
/// Fails if the tint's cell cannot be rasterized or the glow factory cannot be created.
fn cast_glow(
    arena: &mut Arena,
    id: NodeId,
    sprite: &SpriteVisual,
    halo: Option<Halo>,
    paint: &Paint,
    silhouette: Option<&CompositionBrush>,
    group: Option<windows_composition::Captured>,
    ctx: &mut Ctx<'_>,
) -> Result<Option<SpriteVisual>> {
    let lit = match (paint, halo) {
        (Paint::Captured { sigma, tint, .. }, _) => {
            debug_assert!(halo.is_none(), "a captured glow and a halo on one sprite");
            Some((*sigma, *tint, Vector2::default()))
        }
        (_, Some(halo)) => Some((halo.sigma, halo.tint, halo.offset)),
        _ => None,
    };
    if let Some((sigma, tint, _)) = lit {
        let display = ctx.env.apply(tint);
        note!(
            "scene",
            "glow id={} tint: sigma={} scene=({:.3},{:.3},{:.3},{:.3}) display=({:.4},{:.4},{:.4},{:.4})",
            id.index(), sigma, tint.r, tint.g, tint.b, tint.a,
            display.r, display.g, display.b, display.a,
        );
    }
    let Some(((sigma, tint, offset), silhouette)) = lit.zip(silhouette) else {
        // A node with no halo and no captured paint unlights by design. A lit one whose
        // silhouette never arrived is the failure family: the mask or the paint half of the
        // chain came back `None`, and the glow goes with it.
        if lit.is_some() {
            let kind = match paint {
                Paint::Captured { .. } => "captured",
                _ => "halo",
            };
            let size = arena.size(id);
            note!("scene", "glow id={} unlit: silhouette none, paint={}, size=({:.0},{:.0})", id.index(), kind, size.x, size.y);
        }
        unlight(arena, id, sprite, ctx);
        return Ok(None);
    };
    // The draw choke, the same one `Paint::Solid` goes through: the one place in this crate a
    // scene-referred value becomes a display-referred one.
    let key = CellKey::Solid(Q::new(ctx.env.apply(tint)));
    let Some(cell) = ctx
        .cache
        .brush(key, ctx.back, ctx.env, ctx.generation)?
        .map(Brush::as_brush)
    else {
        note!("scene", "glow id={} unlit: the tint cell did not rasterize", id.index());
        unlight(arena, id, sprite, ctx);
        return Ok(None);
    };
    // A Gaussian is spent by three sigma, and a displaced silhouette carries that reach
    // with it. Fixed at the sigma the construction was built for: the captured region is a
    // property write and the sigma is animatable, so a spring overshooting its declaration
    // reaches past what was allocated for it rather than resizing the capture every frame.
    let bleed = 3.0 * sigma + offset.x.abs().max(offset.y.abs());
    let size = arena.size(id);
    let scale = ctx.env.scale();

    // Re-point rather than re-mint wherever the region still fits. A hover restates the
    // paint, and with it the halo, on a path that must not allocate five composition objects
    // per pointer move.
    let fits = arena
        .aux(id)
        .and_then(|aux| aux.glow.as_ref())
        .is_some_and(|glow| glow.bleed == bleed);
    if fits {
        note!("scene", "glow id={} re-pointed: sigma stays, the silhouette and tint are restated", id.index());
        let glow = arena
            .aux(id)
            .and_then(|aux| aux.glow.as_ref())
            .expect("the branch above found one");
        glow.caster.set_brush(silhouette);
        glow.brush.set_source_parameter("tint", &cell);
        let target = glow.paint.clone();
        if let Some(glow) = arena.aux_mut(id).glow.as_mut() {
            glow.group = group;
            glow.offset = offset;
        }
        if let Some(glow) = arena.aux(id).and_then(|aux| aux.glow.as_ref()) {
            glow.resize(id, size, scale);
        }
        drive_blur(arena, id, sigma, false);
        return Ok(Some(target));
    }

    let comp = &ctx.back.compositor;
    // The caster paints the silhouette alone: it is the blur's input, and nothing the
    // halo sprite shows is sharp. The capture reads the host in physical pixels, so the
    // caster carries the display scale and the displacement, and the blur and offset
    // stay in DIPs, where their channels animate them.
    let host = comp.create_container_visual();
    let caster = comp.create_sprite_visual();
    caster.set_brush(silhouette);
    host.children().insert_at_top(&caster);
    let factory = ctx.back.glow_factory()?;
    note!("scene", "glow id={} rebuilt: sigma={}, bleed={}, four visuals minted", id.index(), sigma, bleed);
    let brush = factory.create_brush();

    let capture = comp.capture_bleeding(&host, bleed, size, scale);
    capture.brush.set_stretch(Stretch::None);
    capture.brush.set_nearest_sampling();
    brush.set_source_parameter("silhouette", &capture.brush);
    brush.set_source_parameter("tint", &cell);
    // One expression the construction starts once: the channels write the property set
    // it reads, so a sigma restated by CPU or driven by a spring reaches the graph
    // without a second StartAnimation on the brush.
    let props = comp.create_property_set();
    props.insert_scalar("Sigma", sigma);
    let sigma_expr = comp.create_expression_animation("glow.Sigma");
    sigma_expr.set_reference_parameter("glow", &props);
    brush.start_animation("blur.BlurAmount", &sigma_expr);
    let metrics = host.properties();
    metrics.insert_scalar("Dpi", scale);
    let bounds: &Visual = sprite;
    let extent = comp.create_expression_animation("v.Size * metrics.Dpi");
    extent.set_reference_parameter("v", bounds);
    extent.set_reference_parameter("metrics", &metrics);
    host.start_animation("Size", &extent);
    let extent = comp.create_expression_animation("v.Size");
    extent.set_reference_parameter("v", bounds);
    caster.start_animation("Size", &extent);
    let extent = comp.create_expression_animation("Max((v.Size + Vector2(pad, pad)) * metrics.Dpi, Vector2(1, 1))");
    extent.set_reference_parameter("v", bounds);
    extent.set_reference_parameter("metrics", &metrics);
    extent.set_scalar_parameter("pad", 2.0 * bleed);
    capture.surface.start_animation("SourceSize", &extent);

    // Both children are stated relative to the node, so the compositor re-derives them from
    // the one extent it already carries and a resize writes no property for either. A
    // visual's size is its own plus its relative adjustment of the parent's, which is how the
    // halo is exactly the node grown by the bleed on all four sides.
    let whole = Vector2 { x: 1.0, y: 1.0 };
    let glow_sprite = comp.create_sprite_visual();
    glow_sprite.set_size(2.0 * bleed, 2.0 * bleed);
    glow_sprite.set_relative_size_adjustment(whole);
    glow_sprite.set_offset(-bleed, -bleed, 0.0);
    glow_sprite.set_brush(&brush);
    let paint_sprite = comp.create_sprite_visual();
    paint_sprite.set_relative_size_adjustment(whole);
    // The host, the caster, the halo and the paint. The capture, the property set and the
    // two brushes are not visuals and cost the tree walk nothing.
    ctx.minted += 4;

    // The node stops painting itself and becomes the host of the two.
    sprite.clear_brush();
    let kids = sprite.children();
    kids.remove_all();
    kids.insert_at_bottom(&paint_sprite);
    kids.insert_at_bottom(&glow_sprite);
    // A clip bounds the node's paint and not the light it casts, so the node's own clip
    // follows the paint onto its child: left on the host it would cut the halo off at the
    // box. The clip object moves, so its animated sides and radii keep running.
    if let Some(clip) = arena.aux(id).and_then(|aux| aux.clip.as_ref()) {
        sprite.clear_clip();
        clip.apply(&paint_sprite);
    }

    let target = paint_sprite.clone();
    let glow = GlowState {
        host,
        caster,
        capture,
        props,
        sigma_expr,
        group,
        sprite: glow_sprite,
        brush,
        paint: paint_sprite,
        bleed,
        offset,
    };
    glow.resize(id, size, scale);
    arena.aux_mut(id).glow = Some(glow);
    drive_blur(arena, id, sigma, true);
    Ok(Some(target))
}

/// The glow graph, the one factory's description: the silhouette Gaussian-blurred, the
/// tint multiplied onto its coverage, both sources named. The sigma animates as
/// `"blur.BlurAmount"`, and the construction drives it through a property set rather
/// than by restarting the expression.
fn glow_graph() -> EffectGraph {
    EffectGraph::Composite {
        mode: CompositeMode::SourceIn,
        source: Box::new(EffectGraph::Parameter("tint")),
        destination: Box::new(EffectGraph::GaussianBlur {
            name: "blur",
            sigma: 4.0,
            border: EffectBorderMode::Soft,
            input: Box::new(EffectGraph::Parameter("silhouette")),
        }),
    }
}

/// How a smooth stroke's edge is filtered: a Gaussian of `sigma_px` physical pixels over
/// the captured coverage, and the colour gain that holds the stroke's centre at its
/// authored light. `boost` of `None` derives the gain from the stroke's width.
#[derive(Copy, Clone, Debug)]
struct SmoothEdge {
    sigma_px: f32,
    boost: Option<f32>,
}

/// The edge filter every smooth stroke uses.
///
/// `NEWAPO_SMOOTH_STROKE="<sigma_px> [boost]"` overrides it for the life of the process,
/// read once, so it can be tuned on the panel without a rebuild.
fn smooth_edge() -> SmoothEdge {
    static EDGE: std::sync::OnceLock<SmoothEdge> = std::sync::OnceLock::new();
    *EDGE.get_or_init(|| {
        let default = SmoothEdge { sigma_px: 0.65, boost: None };
        let Ok(text) = std::env::var("NEWAPO_SMOOTH_STROKE") else {
            return default;
        };
        let mut numbers = text.split_whitespace().map(str::parse::<f32>);
        match numbers.next() {
            Some(Ok(sigma_px)) if sigma_px >= 0.0 => SmoothEdge {
                sigma_px,
                boost: numbers.next().and_then(|b| b.ok()).filter(|b| *b > 0.0),
            },
            _ => default,
        }
    })
}

/// The deviation, in physical pixels, of the filtering a captured stroke already carries
/// from the rasterizer's antialiasing and the capture's sampling, and the fraction of its
/// nominal width the captured stroke's profile spans. Both are fitted to the centre light
/// of 1-, 1.5- and 2-DIP captured strokes at 150% on the linear frame, to within 3%.
const RASTER_SIGMA_PX: f32 = 0.49;
const RASTER_WIDTH: f32 = 0.845;

impl SmoothEdge {
    /// The gain on a `width_px` stroke's paint that returns its centre to the light it had
    /// before the Gaussian.
    ///
    /// A stroke `w` wide under a Gaussian of deviation `s` peaks at `erf(w / (2√2 s))` of
    /// full coverage, so the gain is the ratio of that peak with and without this filter's
    /// deviation added to the rasterizer's. The total light rises by the same factor; the
    /// width, and so the shape the blur gave the edge, is unchanged.
    fn boost(self, width_px: f32) -> f32 {
        if let Some(boost) = self.boost {
            return boost;
        }
        let width = RASTER_WIDTH * width_px;
        let peak = |s: f32| erf(width / (2.0 * core::f32::consts::SQRT_2 * s.max(1e-3)));
        let with = (RASTER_SIGMA_PX * RASTER_SIGMA_PX + self.sigma_px * self.sigma_px).sqrt();
        peak(RASTER_SIGMA_PX) / peak(with).max(1e-3)
    }
}

/// The error function, to 1.5e-7 (Abramowitz and Stegun 7.1.26).
fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t * (0.254_829_6 + t * (-0.284_496_7 + t * (1.421_413_7 + t * (-1.453_152 + t * 1.061_405_4))));
    (1.0 - poly * (-x * x).exp()).copysign(x)
}

/// The smooth-stroke graph: `paint` shown through the stroke's coverage after a
/// sub-pixel Gaussian.
///
/// The Gaussian widens the filter across the edge and conserves coverage, which lowers the
/// centre of a stroke only a few pixels wide; the paint arrives already brighter by
/// [`SmoothEdge::boost`], leaving the coverage profile, and so the edge, as the blur
/// shaped it.
///
/// Composition evaluates an effect graph on encoded values, so the graph scales no
/// colour: it multiplies the paint by coverage alone, which holds in any encoding, and an
/// FP16 value above paper white reaches the screen unclamped.
fn smooth_graph() -> EffectGraph {
    EffectGraph::Composite {
        mode: CompositeMode::SourceIn,
        source: Box::new(EffectGraph::Parameter("paint")),
        destination: Box::new(EffectGraph::GaussianBlur {
            name: "edge",
            sigma: 0.0,
            border: EffectBorderMode::Soft,
            input: Box::new(EffectGraph::Parameter("coverage")),
        }),
    }
}

/// Builds a smooth stroke's brush over its coverage capture and its paint.
///
/// The blur's deviation is in DIPs, so the pixel sigma is divided by the display scale the
/// capture was taken at; a scale change rebuilds the chain.
///
/// # Errors
///
/// Fails if the graph's factory cannot be created.
fn smooth_chain(
    coverage: &CompositionBrush,
    paint: &CompositionBrush,
    ctx: &Ctx<'_>,
) -> Result<CompositionBrush> {
    let brush = ctx.back.smooth_factory()?.create_brush();
    brush.set_source_parameter("paint", paint);
    brush.set_source_parameter("coverage", coverage);
    let sigma = ctx.back.compositor.create_expression_animation("sigma");
    sigma.set_scalar_parameter("sigma", smooth_edge().sigma_px / ctx.env.scale());
    brush.start_animation("edge.BlurAmount", &sigma);
    Ok(brush.as_brush())
}

/// Writes the two channels the glow owns onto it.
///
/// A fresh construction seeds them from what the declaration asked for. After that the
/// channels outlive the object they drive, so a rebind restates them rather than resetting to
/// the declaration: a device loss part way through a hover must not snap the halo back to its
/// authored width. Sigma goes through the property set the effect's expression reads;
/// opacity is the halo sprite's own.
fn drive_blur(arena: &mut Arena, id: NodeId, declared: f32, fresh: bool) {
    let sigma = PROPS[Prop::GlowSigma as usize].chan;
    if fresh {
        arena.set_chan(id, sigma, declared);
        arena.set_chan(id, sigma + 1, 1.0);
    }
    let (value, opacity) = (arena.chan(id, sigma), arena.chan(id, sigma + 1));
    if let Some(glow) = arena.aux(id).and_then(|aux| aux.glow.as_ref()) {
        glow.props.insert_scalar("Sigma", value);
        glow.sprite.set_opacity(opacity);
    }
}

/// Drops a node's glow and gives it its own brush slot back.
///
/// The two children go with it: a node that stopped casting light paints itself again, and
/// leaving its paint on a child would keep a visual in the tree for nothing.
fn unlight(arena: &mut Arena, id: NodeId, sprite: &SpriteVisual, ctx: &mut Ctx<'_>) {
    if arena.aux(id).is_some_and(|aux| aux.glow.is_some()) {
        note!("scene", "glow id={} dropped: four visuals freed", id.index());
        sprite.children().remove_all();
        arena.aux_mut(id).glow = None;
        // The paint returns to the node, and its clip with it.
        if let Some(clip) = arena.aux(id).and_then(|aux| aux.clip.as_ref()) {
            clip.apply(sprite);
        }
        ctx.freed += 4;
    }
}

fn cap_of(cap: Cap) -> StrokeCap {
    match cap {
        Cap::Flat => StrokeCap::Flat,
        Cap::Round => StrokeCap::Round,
        Cap::Square => StrokeCap::Square,
        Cap::Triangle => StrokeCap::Triangle,
    }
}

fn join_of(join: Join) -> StrokeJoin {
    match join {
        Join::Miter => StrokeJoin::Miter,
        Join::MiterOrBevel => StrokeJoin::MiterOrBevel,
        Join::Bevel => StrokeJoin::Bevel,
        Join::Round => StrokeJoin::Round,
    }
}

/// Clamps a corner profile to what the box can carry, in DIPs.
///
/// A nine-grid does not clamp its own insets, and an inset covers the whole arc: the raster
/// is two insets and the one flat pixel the middle slice stretches from, so a box has to hold
/// `2*inset + 1` pixels on its shorter axis. A fully round control is therefore always one
/// pixel short of a semicircle, which is that flat pixel; where two opposite insets exceed
/// the extent the corner slices overlap and a pill comes out a lens, a circle a diamond.
///
/// The cap is solved in pixels, because that is the grid the raster is cut on: taken in DIPs
/// it is snapped back up to a whole pixel, and a radius that rounds past its own cap overlaps
/// the opposite corner by one pixel — a notch, not a curve.
///
/// A box with no extent yet keeps the profile it asked for: its size arrives in the same
/// patch and rebuilds this.
#[must_use]
pub fn fit(radius: Corners, size: Vector2, scale: f32) -> Corners {
    if size.x <= 0.0 || size.y <= 0.0 {
        return radius;
    }
    let scale = scale.max(1.0);
    // Floored: a box covering a fraction of its last pixel cannot carry a profile cut for
    // the whole one.
    let extent = (size.x.min(size.y) * scale).floor();
    let cap = ((extent - 1.0) * 0.5).floor().max(0.0) / scale;
    Corners {
        tl: radius.tl.min(cap),
        tr: radius.tr.min(cap),
        br: radius.br.min(cap),
        bl: radius.bl.min(cap),
    }
}

/// The nine-grid inset and inset scale that paint `key`'s raster one raster pixel to one
/// physical pixel.
///
/// What the compositor paints is `inset × inset_scale` in the *visual's* units, and the
/// visual hangs under a root carrying the display scale. At the default scale of one, a
/// corner cut at `n` pixels is painted `n` DIPs — `n · scale` pixels — and a box shorter than
/// twice that has its opposite corners overlap, which is a stadium whatever radius it asked
/// for. `1 / scale` is what puts the corner back on the pixels it was drawn for.
#[must_use]
pub fn nine_slice(key: &BoxKey, scale: f32) -> (f32, f32) {
    (key.inset_px(), 1.0 / scale)
}

#[cfg(test)]
mod grain_tests {
    use super::*;

    /// The grain is worth a fixed number of codes, and that is the claim: an amplitude
    /// chosen in linear light at one level is short everywhere brighter, so what the
    /// strips deliver is checked against the transfer they will meet.
    #[test]
    fn the_grain_is_worth_the_same_codes_at_every_level() {
        let sdr = windows_color::OutputTransform::for_display(
            windows_color::DisplayCapability::Sdr,
            windows_color::REFERENCE_WHITE_NITS,
        );
        let code = |v: f32| {
            let e = if v <= 0.003_130_8 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            };
            e * 255.0
        };
        // Across the range a ground occupies, from its floor to well past its ceiling.
        for &level in &[0.0009_f32, 0.0024, 0.0052, 0.0100, 0.0200] {
            let amp = GRAIN_CODES * sdr.quantum_at(level).expect("the desktop quantises");
            let spans = code(level + 0.5 * amp) - code(level - 0.5 * amp);
            assert!(
                (spans - f64::from(GRAIN_CODES) as f32).abs() < 0.1,
                "at {level} the grain spans {spans} codes, not {GRAIN_CODES}"
            );
        }
        // Sizing it once at the toe is what this replaces, and the gap is the point.
        let toe = GRAIN_CODES * sdr.quantum().expect("the desktop quantises");
        let short = code(0.02 + 0.5 * toe) - code(0.02 - 0.5 * toe);
        assert!(
            short < 0.75 * GRAIN_CODES,
            "a toe-sized grain was not short at the top of the range: {short} codes"
        );
    }

    /// The shipped tile is what the generator produces, and this is what says so. Without
    /// it the blob is a number nobody can check, and the generator is code nobody runs.
    #[test]
    fn the_shipped_tile_is_what_the_generator_designs() {
        let want = build_grain_tile();
        let got = grain_tile();
        assert_eq!(got.len(), want.len());
        for (at, (got, want)) in got.iter().zip(&want).enumerate() {
            assert!(
                (got - want).abs() < 1e-6,
                "texel {at}: shipped {got}, designed {want} — rerun `emit_tile`"
            );
        }
    }

    #[test]
    #[ignore = "regenerates the shipped tile"]
    fn emit_tile() {
        let last = (GRAIN_CELLS - 1) as f32;
        let bytes: Vec<u8> = build_grain_tile()
            .iter()
            .flat_map(|&v| (((v + 0.5) * last).round() as u16).to_le_bytes())
            .collect();
        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/grain.bin");
        std::fs::write(&at, bytes).unwrap();
        println!("wrote {}", at.display());
    }

    /// The correction has to be exact, because it runs on every ground whether or not the
    /// grain it compensates for is doing anything. A layer that came back a code off would
    /// trade a contour for a colour shift.
    #[test]
    fn the_correction_returns_the_layer_it_corrected() {
        let mid = Scrgb {
            r: 0.0024,
            g: 0.0031,
            b: 0.0041,
            a: 1.0,
        };
        let beneath = Beneath {
            alpha: GRAIN_ALPHA,
            mid,
        };
        for &(r, g, b) in &[
            (0.0009_f32, 0.0009, 0.0015),
            (0.0024, 0.0031, 0.0041),
            (0.0052, 0.0084, 0.0112),
            (0.0, 0.0, 0.0),
            (1.0, 0.5, 0.25),
        ] {
            let want = Scrgb { r, g, b, a: 1.0 };
            let under = beneath.apply(want);
            // What the compositor does: source-over of the grain's own midpoint texel over
            // the corrected layer. The texel carries `mid` where the offset is zero.
            let over = |u: f32, m: f32| GRAIN_ALPHA * m + (1.0 - GRAIN_ALPHA) * u;
            let got = [
                over(under.r, mid.r),
                over(under.g, mid.g),
                over(under.b, mid.b),
            ];
            for (got, want) in got.into_iter().zip([want.r, want.g, want.b]) {
                assert!(
                    (got - want).abs() < 1e-6,
                    "correction did not round trip: {got} vs {want}"
                );
            }
        }
    }

    /// A grain is only worth its layer if its energy is high-frequency: white noise puts
    /// energy where the eye integrates it back into blotches, and an ordered matrix puts it
    /// at a few exact frequencies, which reads as weave.
    #[test]
    fn the_tile_is_zero_mean_and_its_energy_is_high_frequency() {
        let tile = grain_tile();
        let n = GRAIN_TILE;
        assert_eq!(tile.len(), n * n);

        let mean = tile.iter().sum::<f32>() / tile.len() as f32;
        assert!(mean.abs() < 1e-3, "grain is not zero mean: {mean}");
        // Ranks are a permutation, so the distribution is uniform by construction and the
        // extremes are the ends of it.
        let (lo, hi) = tile.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| {
            (lo.min(v), hi.max(v))
        });
        assert!((lo + 0.5).abs() < 1e-3 && (hi - 0.5).abs() < 1e-3, "{lo}..{hi}");

        // Energy in the lowest frequencies, against the flat spectrum white noise of the
        // same variance would have. Blue noise is defined by this being small.
        let power = |u: usize, v: usize| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for y in 0..n {
                for x in 0..n {
                    let phase = -core::f64::consts::TAU
                        * ((u * x) as f64 + (v * y) as f64)
                        / n as f64;
                    let s = f64::from(tile[y * n + x]);
                    re += s * phase.cos();
                    im += s * phase.sin();
                }
            }
            (re * re + im * im) / (n * n) as f64
        };
        let variance =
            tile.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() / tile.len() as f64;
        let mut low = 0.0;
        let mut bins = 0;
        for u in 0..4 {
            for v in 0..4 {
                if u == 0 && v == 0 {
                    continue;
                }
                low += power(u, v);
                bins += 1;
            }
        }
        let flat = variance * f64::from(bins);
        assert!(
            low < flat * 0.25,
            "low-frequency energy {low} is not below a quarter of white noise's {flat}"
        );
    }

    /// The whole point, in the units the quantiser works in: a near-black ramp spanning a
    /// handful of codes contours into wide flat bands, and a one-code grain breaks them
    /// without moving where the ramp actually sits.
    #[test]
    fn the_grain_breaks_a_contour_without_moving_the_ramp() {
        const ROWS: usize = 1200;
        const SPAN: f32 = 13.0;
        let tile = grain_tile();
        let n = GRAIN_TILE;

        // Codes, so the quantiser is a round and the grain's amplitude is GRAIN_CODES.
        let ideal = |y: usize| 4.0 + SPAN * y as f32 / ROWS as f32;
        let widest = |f: &dyn Fn(usize) -> i32| {
            let (mut run, mut best, mut last) = (0, 0, i32::MIN);
            for y in 0..ROWS {
                let v = f(y);
                run = if v == last { run + 1 } else { 1 };
                last = v;
                best = best.max(run);
            }
            best
        };

        let plain = |y: usize| ideal(y).round() as i32;
        // One column of the tile, which is what a vertical ramp samples.
        let grained = |y: usize| (ideal(y) + GRAIN_CODES * tile[(y % n) * n + 7]).round() as i32;

        let flat = widest(&plain);
        let broken = widest(&grained);
        // A linear ramp bands at ROWS/SPAN; the real ground's smootherstep profile has
        // flat regions several times wider than that, so this is the gentle case.
        assert!(flat > 60, "the undithered ramp did not band: {flat} rows");
        assert!(
            broken < flat / 8,
            "the grain did not break the contour: {broken} rows against {flat}"
        );

        // And it is still the same ramp: a window wide enough to average the grain out
        // lands within half a code of where the ramp was.
        const WINDOW: usize = 64;
        for start in (0..ROWS - WINDOW).step_by(WINDOW) {
            let mean = (start..start + WINDOW).map(|y| f32::from(grained(y) as i16)).sum::<f32>()
                / WINDOW as f32;
            let want = (start..start + WINDOW).map(ideal).sum::<f32>() / WINDOW as f32;
            assert!(
                (mean - want).abs() < 0.5,
                "the grain moved the ramp at row {start}: {mean} against {want}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filled_shape_with_a_halo_uses_alpha_instead_of_clipping_its_shadow() {
        let shape = Mask::Shape {
            geom: GeomId::FIRST,
            stroke: None,
            space: PathSpace::Local,
        };
        assert_eq!(route(&shape, false, false, false), Route::Clip);
        for held in [false, true] {
            for has_clip in [false, true] {
                assert_eq!(route(&shape, held, has_clip, true), Route::Capture);
            }
        }
        assert_eq!(route(&shape, true, false, false), Route::Capture);
        assert_eq!(route(&shape, false, true, false), Route::Capture);
        // A stroke lives on a sprite shape, so it can only be captured.
        let stroked = Mask::Shape {
            geom: GeomId::FIRST,
            space: PathSpace::Local,
            stroke: Some(StrokeStyle {
                width: 1.0,
                cap: Cap::Flat,
                join: Join::Miter,
                dash: DashId::NONE,
                smooth: false,
            }),
        };
        assert_eq!(route(&stroked, false, false, false), Route::Capture);
    }

    /// A rounded box's corner is cut whole and painted at the size it was drawn.
    ///
    /// Two numbers reach the nine-grid and each has its own failure. An inset short of the
    /// arc leaves the tail of the curve in the middle slice, which stretches it across the
    /// box and smears the edge. An inset scale that does not undo the display scale paints
    /// the corner wider than the raster drew it, and a box under twice that comes out a
    /// stadium. Neither shows up as an error, and both look like a radius that was ignored.
    #[test]
    fn a_nine_grid_corner_is_cut_whole_and_painted_at_the_size_it_was_drawn() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for radius in [1.0_f32, 2.5, 6.0, 7.92, 11.0] {
                let key = BoxKey::new(Corners::all(radius), scale);
                let (inset, inset_scale) = nine_slice(&key, scale);
                let arc = snap_detail(radius, scale) * scale;
                assert!(
                    inset >= arc,
                    "{radius} at {scale}x: an inset of {inset} cuts inside an arc of {arc}"
                );
                let painted = inset * inset_scale * scale;
                assert!(
                    (painted - key.inset_px()).abs() < 1.0e-3,
                    "{radius} at {scale}x: a corner drawn at {} px is painted at {painted}",
                    key.inset_px()
                );
            }
        }
    }

    /// A box cell's inset covers its whole arc, and wastes no pixel past it.
    ///
    /// Both halves are load-bearing. An inset short of the arc smears the edge; an inset
    /// past it costs the profile radius it could have had, because the box has to hold two
    /// insets.
    #[test]
    fn a_box_cells_inset_covers_its_arc_and_wastes_nothing_past_it() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for radius in [0.0_f32, 1.0, 2.5, 6.0, 7.7, 7.92, 11.0, 24.0] {
                let key = BoxKey::new(Corners::all(radius), scale);
                let arc = snap_detail(radius, scale) * scale;
                let inset = key.inset_px();
                assert!(
                    inset >= arc,
                    "{radius} at {scale}x: inset {inset} is inside an arc of {arc}"
                );
                assert!(
                    inset <= arc.max(1.0).ceil(),
                    "{radius} at {scale}x: inset {inset} wastes a pixel past an arc of {arc}"
                );
                // Two insets and the one flat pixel the middle slice stretches from.
                let side = inset as i32 * 2 + 1;
                assert_eq!(CellKey::Box(key).px(), (side, side));
            }
        }
    }

    #[test]
    fn a_box_key_is_snapped_by_construction_at_every_scale() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            let a = BoxKey::new(Corners::all(6.0), scale);
            let b = BoxKey::new(Corners::all(6.0 + 0.01), scale);
            assert_eq!(a, b, "a hundredth of a DIP forked the cache at {scale}x");
        }
    }

    /// A rounded box's profile never exceeds half the box it is cut for.
    ///
    /// The compositor does not clamp a nine-grid's insets: where two opposite insets exceed
    /// the extent the corner slices overlap. A pill exactly twice its own radius tall — a
    /// fully rounded control — comes out as a lens, and a circle as a diamond.
    #[test]
    fn a_rounded_profile_is_cut_to_fit_the_box_it_masks() {
        let at = |x: f32, y: f32| Vector2 { x, y };
        // What the box has to hold: two insets and the flat pixel between them, counted
        // through the key that cuts the raster rather than through a second formula.
        let fits = |r: f32, extent: f32, scale: f32| {
            let inset = BoxKey::new(Corners::all(r), scale).inset_px();
            inset * 2.0 + 1.0 <= (extent * scale).floor()
        };

        // Roomy: the profile is what it asked for.
        assert_eq!(
            fit(Corners::all(10.0), at(120.0, 40.0), 1.5),
            Corners::all(10.0)
        );
        // A pill exactly twice its radius tall — a fully rounded control, and the case that
        // rendered as a lens.
        let pill = fit(Corners::all(11.0), at(37.4, 22.0), 1.5).tl;
        assert!(fits(pill, 22.0, 1.5), "pill radius {pill} does not fit 22");
        // A knob asking to be a circle in a box that is not square, which rendered with its
        // corners cut into notches.
        let knob = fit(Corners::all(7.7), at(13.333374, 15.333344), 1.5).tl;
        assert!(
            fits(knob, 13.333374, 1.5),
            "knob radius {knob} does not fit 13.33"
        );
        // The cap is the *shorter* axis: a wide, short box rounds by its height.
        assert!(fit(Corners::all(40.0), at(400.0, 20.0), 1.5).tl < 10.0);
        // What a control asking to be fully round is left with: the cut costs it the one
        // flat pixel the middle slice stretches, and never more.
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for extent in [15.0_f32, 20.0, 22.0, 13.333374] {
                let cut = fit(Corners::all(extent), at(extent, extent), scale).tl;
                assert!(
                    fits(cut, extent, scale),
                    "{extent} at {scale}x cut to {cut}"
                );
                let short = (extent * scale).floor();
                assert!(
                    cut * scale >= (short - 1.0) * 0.5 - 0.5,
                    "{extent} at {scale}x was cut to {cut}, well under half its box"
                );
            }
        }
        // A box with no extent yet keeps its profile: the size arrives in the same patch.
        assert_eq!(
            fit(Corners::all(11.0), at(0.0, 0.0), 1.5),
            Corners::all(11.0)
        );
    }

    #[test]
    fn a_cell_family_is_invalidated_only_by_what_it_reads() {
        let built = Gen::default();
        let theme = Gen { color: 1, ..built };
        let dpi = Gen { dpi: 1, ..built };
        let lost = Gen { device: 1, ..built };
        let box_ = CellKey::Box(BoxKey::new(Corners::all(4.0), 1.0));
        let solid = CellKey::Solid(Q::new(Scrgb::TRANSPARENT));
        // A theme flip costs a handful of colour cells and no coverage at all.
        assert!(box_.deps().fresh(built, theme));
        assert!(!solid.deps().fresh(built, theme));
        // A DPI change is the other way round.
        assert!(!box_.deps().fresh(built, dpi));
        assert!(solid.deps().fresh(built, dpi));
        // Device loss takes everything.
        assert!(!box_.deps().fresh(built, lost));
        assert!(!solid.deps().fresh(built, lost));
    }

    #[test]
    fn a_colour_cell_carries_coverage_nowhere_and_alpha_from_its_key() {
        let opaque = CellKey::Solid(Q::new(Scrgb {
            r: 0.5,
            g: 0.25,
            b: 0.125,
            a: 1.0,
        }));
        assert!(!opaque.coverage());
        assert_eq!(opaque.opacity(), Opacity::Opaque);
        assert_eq!(opaque.px(), (4, 4));
        let translucent = CellKey::Solid(Q::new(Scrgb {
            r: 12.0,
            g: -0.4,
            b: 1.0,
            a: 0.5,
        }));
        assert_eq!(translucent.opacity(), Opacity::Translucent);
        // A coverage family is always translucent: the corners are not covered.
        let box_ = CellKey::Box(BoxKey::new(Corners::all(4.0), 1.0));
        assert!(box_.coverage());
        assert_eq!(box_.opacity(), Opacity::Translucent);
    }

    #[test]
    fn a_resource_outlives_the_model_but_not_its_last_sprite() {
        let mut res = Resources::default();
        let id = DashId::raw(1, 1);
        res.declare(
            id.erased(),
            ResObj::Dash([2.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 2),
        );
        let holding = Some(Holding::Dash(id));

        res.retain(holding);
        res.retain(holding);
        // The declaration goes first, while two sprites are still painting with it.
        res.disclaim(id.erased());
        assert_eq!(res.dashes(id), &[2.0, 2.0]);

        res.release(holding);
        assert_eq!(res.dashes(id).len(), 2, "one sprite still holds it");
        res.release(holding);
        assert!(
            res.dashes(id).is_empty(),
            "the last holder left and it stayed"
        );
    }

    #[test]
    fn a_resource_no_sprite_ever_took_goes_with_the_declaration() {
        let mut res = Resources::default();
        let id = DashId::raw(1, 1);
        res.declare(id.erased(), ResObj::Dash([1.0; 8], 1));
        res.disclaim(id.erased());
        assert!(res.dashes(id).is_empty());
    }

    #[test]
    fn releasing_more_than_was_taken_cannot_drop_a_declared_resource() {
        let mut res = Resources::default();
        let id = DashId::raw(1, 1);
        res.declare(id.erased(), ResObj::Dash([1.0; 8], 1));
        // An app-side bug must not pull a resource out from under a standing declaration.
        res.release(Some(Holding::Dash(id)));
        res.release(Some(Holding::Dash(id)));
        assert_eq!(res.dashes(id).len(), 1);
    }

    #[test]
    fn one_table_keeps_five_families_apart_in_one_index_space() {
        let mut res = Resources::default();
        let dash = DashId::raw(2, 1);
        res.declare(dash.erased(), ResObj::Dash([3.0; 8], 1));
        // Same index, same generation, different family: the row answers for neither the
        // wrong family nor a stale generation.
        assert!(res.obj(GeomId::raw(2, 1).erased()).is_none());
        assert!(res.obj(DashId::raw(2, 2).erased()).is_none());
        assert!(res.obj(dash.erased()).is_some());
    }

    #[test]
    fn a_region_slot_exists_before_its_buffer_does() {
        let mut res = Resources::default();
        let region = RegionId::raw(1, 1);
        res.declare(region.erased(), ResObj::Pending);
        assert!(res.obj(region.erased()).is_some());
        assert!(
            res.brush(region.erased()).is_none(),
            "a pending slot must paint nothing"
        );
    }

    #[test]
    fn a_feather_ladder_rises_and_falls_and_stays_within_the_ramp() {
        let stops = feather(0.25, 0.0);
        assert!(stops.windows(2).all(|pair| pair[0].at <= pair[1].at));
        assert_eq!(stops.first().unwrap().at, 0.0);
        assert_eq!(stops.last().unwrap().at, 1.0);
        assert_eq!(stops.first().unwrap().color.a, 0.0);
        assert_eq!(stops.last().unwrap().color.a, 0.0);
        // Full strength in the body, with zero slope at either tip.
        assert!(stops.iter().any(|stop| stop.color.a >= 1.0));
    }

    #[test]
    fn an_inset_feather_keeps_a_short_transition_and_a_full_strength_body() {
        let stops = feather(4.0 / 480.0, 2.0 / 480.0);
        assert_eq!(stops.first().unwrap().at, 2.0 / 480.0);
        assert_eq!(stops.last().unwrap().at, 1.0 - 2.0 / 480.0);
        assert_eq!(stops.first().unwrap().color.a, 0.0);
        assert_eq!(stops.last().unwrap().color.a, 0.0);
        let body: Vec<_> = stops.iter().filter(|stop| stop.color.a == 1.0).collect();
        assert_eq!(body.len(), 2);
        assert!((body[0].at - 6.0 / 480.0).abs() < 1e-7);
        assert!((body[1].at - (1.0 - 6.0 / 480.0)).abs() < 1e-7);
    }
}
