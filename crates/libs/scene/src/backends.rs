//! The four handles a scene mints and rasterizes with. **Front half.**
//!
//! [`Backends`] carries everything needed to mint a composition object or rasterize a
//! surface, and no fact about the display it will appear on. Those arrive as an
//! [`Env`](crate::Env) at every operation that depends on them.
//!
//! # Built and owned by the application, not by the scene
//!
//! The application constructs these handles and passes them to the calls that need them; a
//! scene stores none of them. Three constraints hold that shape:
//!
//! - **The font ladder is shared.** Two [`TextEngine`]s interning names independently agree
//!   on index `0` and disagree on everything after it, and the symptom is a run drawn in the
//!   wrong face rather than an error. The thread that shapes and the thread that rasterizes
//!   each hold an engine, so both must be built over one ladder the application owns.
//! - **A compositor proves its own precondition.** One cannot exist on a thread with no
//!   dispatcher queue, so taking it as an argument makes holding it the proof that the queue
//!   was created.
//! - **Device loss is repaired by the GPU's owner**, through [`adopt`](Backends::adopt).
//!
//! Every mechanism below belongs to `windows-composition`, `windows-d2d` or `windows-text`;
//! what is written here is what to draw, at what extent, sampled how.

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

/// The compositor, GPU, graphics device and text engine a scene mints objects with.
pub struct Backends {
    pub(crate) compositor: Compositor,
    pub(crate) gpu: Gpu,
    graphics: CompositionGraphicsDevice,
    text: TextEngine,
    /// Whether this device allocates coverage at one byte a pixel. No query reports it, so
    /// the first failed allocation answers it and every tile after takes the same route.
    masks_a8: Cell<bool>,
    /// The one opaque white brush every coverage tile draws with. White is a mask's
    /// multiplicative identity, so it is never retinted.
    white: OnceCell<Solid>,
}

impl Backends {
    /// Binds a compositor, a GPU and a font ladder together.
    ///
    /// `gpu` must be the only GPU used with `compositor`. When the compositor realizes a
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
            masks_a8: Cell::new(true),
            white: OnceCell::new(),
        })
    }

    /// Adopts a replacement GPU after device loss.
    ///
    /// Device loss takes the Direct2D device and everything drawn with it; the compositor
    /// and the text engine survive, so only the graphics device is rebuilt and the engine
    /// keeps its resolved faces. Call [`Scene::device_lost`](crate::Scene::device_lost)
    /// afterwards to re-realize what was drawn.
    ///
    /// # Errors
    ///
    /// Fails if the replacement graphics device cannot be created.
    pub fn adopt(&mut self, gpu: &Gpu) -> Result<()> {
        self.gpu = gpu.clone();
        self.graphics = gpu.graphics_device(&self.compositor)?;
        Ok(())
    }

    /// Returns the ladder every run's family index resolves against.
    ///
    /// The shaping thread's own [`TextEngine`] must be built over this ladder: a run's
    /// family indices are meaningful only against the ladder that interned them.
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

    /// Returns a coverage surface, at one byte a pixel where the device allows it.
    ///
    /// An FP16 colour surface carries the same coverage, so the fallback changes the
    /// allocation and nothing else. It is taken once and every tile after it goes straight
    /// there.
    pub(crate) fn mask_surface(&self, px: (i32, i32)) -> Result<CompositionDrawingSurface> {
        if self.masks_a8.get() {
            match self.graphics.mask(px) {
                Ok(surface) => return Ok(surface),
                Err(_) => self.masks_a8.set(false),
            }
        }
        self.graphics.color(px, Opacity::Translucent)
    }

    /// Returns whether coverage is allocated at a byte a pixel on this device.
    #[must_use]
    pub fn masks_are_a8(&self) -> bool {
        self.masks_a8.get()
    }

    /// Returns the opaque white brush every coverage tile draws with, minting it on first
    /// use.
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

    /// Returns a brush over `surface`, anchored top-left rather than at composition's
    /// centred default.
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

    /// Builds a composition path from `verbs`, closing any figure the verbs leave open.
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
    /// A composition gradient brush carries eight-bit stops, which quantizes a narrow alpha
    /// ramp to almost nothing; the FP16 strip carries colour and alpha in the same texels
    /// with no such floor. The strip is stretched to fill, so it carries none of the
    /// sprite's extent and a resize re-rasterizes nothing.
    pub(crate) fn raster_ramp(
        &self,
        env: Env,
        stops: &[(f32, Radiance)],
        spread: Spread,
    ) -> Result<Option<CompositionDrawingSurface>> {
        // Along the axis for the two cardinal directions; square for a diagonal, which has
        // no single axis to lay a strip along, and for a radial, which has none at all.
        //
        // 64 px for the radial: a smootherstep falloff carries no high-frequency content,
        // and what that resolution misses stays under a hundredth of an 8-bit level at the
        // amplitudes a glow is authored with.
        let px = match spread {
            Spread::Horizontal => (256, 1),
            Spread::Vertical => (1, 256),
            Spread::DiagonalDown | Spread::DiagonalUp => (128, 128),
            Spread::Radial => (64, 64),
        };
        let surface = self.surface(px)?;

        // The stops are resampled rather than passed through: the drawing stack interpolates
        // linearly and the palette is authored in ICtCp, so the mix has to be taken
        // perceptually. 64 samples put the linear steps between them below a
        // just-noticeable difference.
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
                // Centred with radii half the tile, so the profile's last stop lands exactly
                // on the edge. Stretching the square tile into the sprite is what makes the
                // ellipse.
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
    /// The tile is a mask: the glyphs are drawn opaque white, the multiplicative identity,
    /// so the paint beside it in the brush chain supplies the colour unchanged.
    ///
    /// `ink` is in DIPs, like every other extent this crate accepts, and the pixel grid is
    /// applied through [`extent_px`], the same function every cache key uses.
    ///
    /// `segs` is a list because font fallback splits one line across faces, and each segment
    /// carries its own origin, so a bidi line — where visual order and advance order
    /// disagree — needs no second rule. The baseline is snapped here: `DrawGlyphRun` takes
    /// no options parameter, so it performs none of the baseline snapping the text-layout
    /// APIs do. Horizontal positions stay subpixel, because advances carry ideal metrics
    /// independent of display resolution.
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
            // The rendering mode is stated here and scoped by the guard, so it cannot leak
            // into whatever the surface's context draws next. Inheriting the system's makes
            // glyphs systematically thin or fat.
            let _params = d.text_params(self.text.rendering_params());
            // Every segment on a line shares one baseline, so the whole tile is nudged onto
            // a physical pixel once; per-segment rounding would break the shaped spacing.
            let at = Vector2 {
                x: 0.0,
                y: d.snap(ink.baseline.y) - ink.baseline.y,
            };
            // Called through the trait: `Draw` has an inherent `line` that draws something
            // else entirely.
            windows_text::GlyphDraw::line(d, at, segs, buffers, &self.text, white);
            Ok(())
        })
    }

    /// Runs one draw bracket, surfacing the callback's error alongside the bridge's.
    ///
    /// The bridge's callback cannot fail, so the callback's result travels out in a slot and
    /// is raised once the bracket has closed. Returns `None` when the bracket produced no
    /// content.
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
