//! The four handles a scene makes things with. **Front half.**
//!
//! A factory bundle and nothing else: everything needed to *mint* a composition object or
//! rasterize a surface, and no fact about the display it will appear on. Those arrive as an
//! [`Env`](crate::Env) at every operation that depends on them.
//!
//! # Built and owned by the application, not by the scene
//!
//! Every one of these is the *application's*, and a scene is one consumer of them. Holding
//! them here would misplace three things at once, each of which is a real defect and not a
//! matter of taste:
//!
//! - **The font ladder has to be shared.** `windows-text` is explicit that two
//!   [`TextEngine`]s interning names independently "*would agree on `0` and disagree on
//!   everything after it*", and that the symptom is a run drawn in the wrong face rather
//!   than an error. The thread that shapes holds one engine and the thread that rasterizes
//!   holds another, so a scene that *consumed* the ladder would take one input to that
//!   invariant out of reach of the half that needs it.
//! - **A compositor proves its own precondition.** One cannot exist on a thread with no
//!   dispatcher queue. Taking it as an argument means having one *is* the proof the queue
//!   was created; minting it inside would leave that as a comment and a runtime failure.
//! - **Device loss is the owner's to repair.** [`adopt`](Backends::adopt) belongs to
//!   whoever owns the GPU's lifetime, and that is not the tree drawn with it.
//!
//! So this type is passed to the calls that need it and stored by none of them. Every
//! mechanism below it belongs to `windows-composition`, `windows-d2d` or `windows-text`;
//! what is written here is the *recipe* — what to draw, at what extent, sampled how.

use crate::env::Env;
use crate::quant::extent_px;
use crate::sink::{PathVerb, Spread};
use core::cell::{Cell, OnceCell};
use windows_color::{Radiance, Scrgb};
use windows_composition::{
    CompositionDrawingSurface, CompositionGraphicsDevice, CompositionPath, CompositionSurfaceBrush,
    Compositor, Stretch, Surface,
};
use windows_core::Result;
use windows_d2d::{Extend, Gpu, Opacity, SceneSurface, Solid, Stop, SurfaceDraw};
use windows_numerics::Vector2;
use windows_text::{FontLadder, GlyphSeg, Ink, SegBuffers, TextEngine};

pub struct Backends {
    pub(crate) compositor: Compositor,
    pub(crate) gpu: Gpu,
    graphics: CompositionGraphicsDevice,
    text: TextEngine,
    /// Whether this device allocates coverage at one byte a pixel. There is no query for
    /// it, so the failed allocation is the answer — asked once, not once per tile.
    masks_a8: Cell<bool>,
    /// The one opaque white brush every coverage tile draws with. White is a mask's
    /// multiplicative identity, so it is never retinted.
    white: OnceCell<Solid>,
}

impl Backends {
    /// Binds a compositor, a GPU and a font ladder together.
    ///
    /// One GPU per compositor: when the compositor realizes a composition path it asks the
    /// geometry source for geometry belonging to a factory of its own choosing, and neither
    /// side of that callback can check the match — so a path built on a second GPU is
    /// content that never appears rather than an error.
    pub fn new(compositor: Compositor, gpu: &Gpu, fonts: FontLadder) -> Result<Self> {
        Ok(Self {
            graphics: gpu.graphics_device(&compositor)?,
            text: TextEngine::new(fonts)?,
            compositor,
            gpu: gpu.clone(),
            masks_a8: Cell::new(true),
            white: OnceCell::new(),
        })
    }

    /// Adopts a replacement GPU after device loss.
    ///
    /// The compositor and the text engine survive — only the Direct2D device and everything
    /// drawn with it died — so the graphics device is what is rebuilt, and the engine keeps
    /// its resolved faces. Call this, then
    /// [`Scene::device_lost`](crate::Scene::device_lost) to re-realize what was drawn.
    pub fn adopt(&mut self, gpu: &Gpu) -> Result<()> {
        self.gpu = gpu.clone();
        self.graphics = gpu.graphics_device(&self.compositor)?;
        Ok(())
    }

    /// The ladder every run's family index resolves against.
    ///
    /// The shaping thread's own [`TextEngine`] must be built over **this** ladder: the
    /// indices in a run are only meaningful against the one that interned them.
    #[must_use]
    pub fn ladder(&self) -> &FontLadder {
        self.text.ladder()
    }

    pub(crate) fn graphics(&self) -> &CompositionGraphicsDevice {
        &self.graphics
    }

    pub(crate) fn surface(&self, px: (i32, i32)) -> Result<CompositionDrawingSurface> {
        self.graphics.color(px, Opacity::Translucent)
    }

    /// A coverage surface, at one byte a pixel where the device allows it.
    ///
    /// An FP16 mask is the same coverage, so the fallback is not a degrade: it fails over
    /// silently, once, and every tile after takes the same route without asking.
    pub(crate) fn mask_surface(&self, px: (i32, i32)) -> Result<CompositionDrawingSurface> {
        if self.masks_a8.get() {
            match self.graphics.mask(px) {
                Ok(surface) => return Ok(surface),
                Err(_) => self.masks_a8.set(false),
            }
        }
        self.graphics.color(px, Opacity::Translucent)
    }

    /// Whether coverage is allocated at a byte a pixel on this device.
    #[must_use]
    pub fn masks_are_a8(&self) -> bool {
        self.masks_a8.get()
    }

    /// The one opaque white brush every coverage tile draws with.
    fn white(&self) -> Result<&Solid> {
        if let Some(white) = self.white.get() {
            return Ok(white);
        }
        let white = self.gpu.solid(Scrgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        })?;
        Ok(self.white.get_or_init(|| white))
    }

    /// A brush over `surface`, anchored top-left.
    ///
    /// Composition's default is centred, which is easy to mistake for a placement bug.
    pub(crate) fn brush(
        &self,
        surface: &impl Surface,
        stretch: Stretch,
    ) -> CompositionSurfaceBrush {
        let brush = self.compositor.create_surface_brush(surface);
        brush.set_stretch(stretch);
        brush.set_alignment_ratio(0.0, 0.0);
        brush
    }

    pub(crate) fn path(&self, verbs: &[PathVerb]) -> Result<CompositionPath> {
        let path = self.gpu.path(|sink| {
            let mut open = false;
            for verb in verbs {
                match *verb {
                    PathVerb::Move { to, filled } => {
                        if open {
                            sink.close(windows_d2d::End::Open);
                        }
                        sink.figure(
                            to,
                            if filled {
                                windows_d2d::Figure::Filled
                            } else {
                                windows_d2d::Figure::Hollow
                            },
                        );
                        open = true;
                    }
                    // These are the batched sink calls, so a run is always a slice.
                    PathVerb::Line(to) => {
                        sink.lines(core::slice::from_ref(&to));
                    }
                    PathVerb::Cubic { c1, c2, to } => {
                        sink.beziers(&[windows_d2d::Bezier { c1, c2, to }]);
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

    /// Rasterizes a gradient into one premultiplied FP16 strip.
    ///
    /// **Colour and alpha in the same texels.** A composition gradient brush carries
    /// eight-bit stops, so a narrow alpha ramp quantizes to almost nothing; FP16 has no such
    /// floor. A fixed strip stretched to fill *is* a linear gradient, so the resource does
    /// not carry the sprite's extent and a resize costs nothing.
    pub(crate) fn raster_ramp(
        &self,
        env: Env,
        stops: &[(f32, Radiance)],
        spread: Spread,
    ) -> Result<Option<CompositionDrawingSurface>> {
        // Along the axis for the two cardinal directions; square for a diagonal, which has
        // no single axis to lay a strip along, and for a radial, which has none at all.
        //
        // 64 for the radial is not a compromise: a smootherstep falloff carries no
        // high-frequency content, so the profile it misses at that resolution is well
        // under a hundredth of an 8-bit level at the amplitudes a glow is authored with.
        let px = match spread {
            Spread::Horizontal => (256, 1),
            Spread::Vertical => (1, 256),
            Spread::DiagonalDown | Spread::DiagonalUp => (128, 128),
            Spread::Radial => (64, 64),
        };
        let surface = self.surface(px)?;

        // Sampled, not handed over raw: the drawing stack interpolates linearly and a
        // palette authored in ICtCp means the *perceptual* mix. Enough samples that the
        // linear steps between them sit below a just-noticeable difference.
        const SAMPLES: usize = 64;
        let mut sampled = [Stop {
            at: 0.0,
            color: Scrgb::TRANSPARENT,
        }; SAMPLES];
        for (index, slot) in sampled.iter_mut().enumerate() {
            let t = index as f32 / (SAMPLES - 1) as f32;
            *slot = Stop {
                at: t,
                color: env.apply(Radiance::sample(stops, t)),
            };
        }

        let (w, h) = (px.0 as f32, px.1 as f32);
        // The strip is drawn in its own pixel space, so it is authored at 96 DPI whatever
        // the display's is: it carries no snapped dimension and is stretched to fill.
        self.draw(&surface, 96.0, |d| {
            d.clear(Scrgb::TRANSPARENT);
            let rect = windows_d2d::Rect::new(0.0, 0.0, w, h);
            match spread.ends() {
                Some((from, to)) => {
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
                    d.fill(rect, &ramp);
                }
                // Centred, and its radii half the tile, so the profile's last stop lands
                // exactly on the edge. Stretching that square into the sprite is what
                // turns it into the ellipse the glow wants.
                None => {
                    let ramp = self.gpu.radial(
                        &sampled,
                        Vector2 {
                            x: w / 2.0,
                            y: h / 2.0,
                        },
                        Vector2 {
                            x: w / 2.0,
                            y: h / 2.0,
                        },
                        Extend::Clamp,
                    )?;
                    d.fill(rect, &ramp);
                }
            }
            Ok(())
        })
    }

    /// Rasterizes one shaped run into an alpha-carrying coverage tile.
    ///
    /// The tile is a **mask**: opaque white glyphs, because its colour comes from the paint
    /// beside it in the brush chain and white is the multiplicative identity that leaves
    /// that paint alone.
    ///
    /// `ink` is in **DIPs**, like every other extent this crate accepts, and the pixel grid
    /// is applied here through the same [`extent_px`] every cache key uses — one
    /// implementation of that grid, so nothing can disagree with it at a fractional scale.
    ///
    /// Three things this has to get right, each a whole class of bug. Font fallback splits
    /// a line across faces, so the segments are a list — a single-segment wire fails to
    /// render CJK and emoji rather than degrading. Each segment carries its own origin, so
    /// a bidi line, where visual order and advance order disagree, needs no second rule.
    /// And the baseline is snapped **here**: `DrawGlyphRun` takes no options parameter, so
    /// the free baseline snapping the text-layout APIs perform is unavailable and nothing
    /// will report a run that landed half a pixel low. Horizontal positions stay subpixel
    /// by design — advances carry ideal metrics independent of display resolution.
    pub(crate) fn raster_run(
        &self,
        env: Env,
        segs: &[GlyphSeg],
        buffers: &SegBuffers,
        ink: Ink,
    ) -> Result<Option<CompositionDrawingSurface>> {
        let scale = env.scale();
        let px = (
            extent_px(ink.size.x, scale) as i32,
            extent_px(ink.size.y, scale) as i32,
        );
        let surface = self.mask_surface(px)?;
        let white = self.white()?;
        self.draw(&surface, env.dpi(), |d| {
            d.clear(Scrgb::TRANSPARENT);
            if segs.is_empty() {
                return Ok(());
            }
            // Stated rather than inherited, and as a guard so the mode cannot leak into
            // whatever the surface's context draws next. Left to inherit the system's,
            // glyphs come out systematically thin or fat and read as a font choice rather
            // than as a rasterization setting.
            let _params = d.text_params(self.text.rendering_params());
            // Every segment on a line shares one baseline, so the whole tile is nudged by
            // what it takes to put that baseline on a physical pixel — one correction
            // rather than a per-segment rounding that would break the shaped spacing.
            let at = Vector2 {
                x: 0.0,
                y: d.snap(ink.baseline.y) - ink.baseline.y,
            };
            // Named through the trait: a `Draw` has a `line` of its own, and the two mean
            // very different things.
            windows_text::GlyphDraw::line(d, at, segs, buffers, &self.text, white);
            Ok(())
        })
    }

    /// Runs one draw bracket, surfacing the callback's error and not the bridge's.
    ///
    /// The bridge hands the callback a target and no way to fail, so the result travels out
    /// in a slot — the one place this crate writes that pattern.
    fn draw(
        &self,
        surface: &CompositionDrawingSurface,
        dpi: f32,
        f: impl FnOnce(&windows_d2d::Draw<'_>) -> Result<()>,
    ) -> Result<Option<CompositionDrawingSurface>> {
        let mut inner: Result<()> = Ok(());
        let drawn = surface.draw(dpi, Opacity::Translucent, |d| inner = f(d))?;
        inner?;
        Ok(drawn.then(|| surface.clone()))
    }
}
