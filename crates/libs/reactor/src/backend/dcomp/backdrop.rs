//! The window backdrop: a retained compositor layer stack, built **before the
//! window is shown** and moved entirely by DWM.
//!
//! An app-supplied gradient backdrop used to be an ordinary reactor element — a
//! `surface_painter` filling the root cell. That costs a full-window FP16
//! surface and, worse, cannot exist on the first composited frame: a painter's
//! surface arrives only after its size is delivered cross-thread and its surface
//! request is serviced on a later commit, so the shell's content landed on an
//! unpainted window and the backdrop appeared two commits later. Here the whole
//! stack is minted inside [`Compositing::new`], before `ShowWindow`, so there is
//! no frame in which it does not exist.
//!
//! ## The stack (bottom to top)
//!
//! - **base** — a tiny solid FP16 surface stretched over the window;
//! - **glows** — one small FP16 blob per radial layer, each a circular
//!   smootherstep alpha ramp stretched into its ellipse by the sprite's own
//!   aspect. A blob is resolution-independent in the way that matters: the
//!   falloff carries no high-frequency content, so 64x64 tracks the exact
//!   profile to well under a hundredth of an 8-bit LSB at these amplitudes;
//! - **dither** — the blue-noise grain, screen-fixed at one texel per physical
//!   pixel.
//!
//! Layers composite **source-over**, which is the same operation a single
//! surface performs when it draws them in sequence — source-over is associative,
//! so splitting them across sprites is not an approximation of the old drawing,
//! it is the identical composite.
//!
//! ## Colour stays FP16; only alpha reaches the compositor's 8-bit brushes
//!
//! Every layer's colour rides an FP16 (`R16G16B16A16Float`) surface, never a
//! `CompositionColorBrush` — an 8-bit `Windows.UI.Color` cannot carry a
//! near-black few-LSB gradient, let alone the above-paper-white range the
//! palette is authored in. This is the same rule the knob's value arc follows
//! (`knob.rs`): the FP16 surface is the colour, the compositor only moves it.
//!
//! The app hands over colours **already display-fitted** — the fork holds no
//! colour policy — so the draw session writes them raw (FP16 targets do not
//! re-encode; see `DrawingSession::encode_srgb_target`).
//!
//! ## Why the dither layer is pre-compensated
//!
//! The old dither was a D2D `PRIMITIVE_BLEND_ADD` fill of signed FP16 texels.
//! The compositor has no additive blend, so the grain instead rides a static
//! full-window sprite at a low constant opacity `a`. Source-over gives
//! `dst + a*(c - dst)`, so writing `c = m + d/a` over a stack pre-compensated by
//! `v' = (v - a*m) / (1 - a)` composites to exactly `v + d`: the additive dither,
//! reproduced. Pre-compensating each layer's colour pre-compensates the whole
//! composite, because source-over of an opaque base is a convex combination and
//! the correction is affine.
//!
//! The grain must be **screen-fixed**: baked into the glow surfaces it would
//! translate with them, and drifting grain reads as shimmer — far worse than the
//! banding it exists to break.

use std::sync::OnceLock;

use super::bootstrap::Compositing;
use crate::system_bindings::{
    AnimationIterationBehavior, CompositionAnimation, CompositionBrush, CompositionDrawingSurface,
    CompositionSurfaceBrush, ContainerVisual, ICompositionDrawingSurfaceInterop, ICompositionObject,
    IKeyFrameAnimation, IVector3KeyFrameAnimation, SpriteVisual, TimeSpan, Visual, POINT,
};
use windows_canvas_core::{ColorF, DrawingSession, GradientStop, Matrix3x2, Rect, Vector2 as CVec2};
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

// ── The spec: plain data the app hands over ─────────────────────────────────

/// One radial glow layer, in the CSS `radial-gradient(ellipse at X% Y%, peak 0%,
/// transparent N%)` idiom the design is authored in.
#[derive(Clone, Debug)]
pub struct BackdropGlow {
    /// The composite colour AT the layer's centre — an authored peak, not an
    /// alpha wash. Linear scRGB, already display-fitted by the app.
    pub peak: [f32; 3],
    /// Centre as a fraction of the window (`0.0..=1.0` per axis).
    pub center: (f32, f32),
    /// Where full transparency lands along the farthest-corner ray, as a
    /// fraction of it (CSS `transparent N%`).
    pub stop: f32,
    /// Blob edge length in pixels. 64 is exact for a smootherstep falloff at
    /// these amplitudes; larger buys nothing.
    pub resolution: u32,
    /// Compositor-side drift. `None` pins the layer (an inverse vignette wants
    /// to stay anchored — a moving one reads as the window tilting).
    pub drift: Option<GlowDrift>,
}

/// A closed, seamless drift path for one glow: a lemniscate
/// (`x = ax*sin t`, `y = ay*sin 2t`), which is smooth, non-circular and reads as
/// organic wander rather than a visible orbit. Give sibling glows mutually
/// incommensurate periods and the composite never repeats.
#[derive(Clone, Copy, Debug)]
pub struct GlowDrift {
    /// Peak excursion per axis, in DIPs.
    pub amplitude: (f32, f32),
    /// Seconds for one full traversal.
    pub period_secs: f32,
    /// Starting position along the path, `0.0..1.0`.
    pub phase: f32,
}

/// The blue-noise grain layer.
#[derive(Clone, Debug)]
pub struct BackdropDither {
    /// `tile_size * tile_size` zero-mean offsets in `[-0.5, 0.5)`, row-major.
    pub tile: Vec<f32>,
    pub tile_size: u32,
    /// One tile unit in linear scRGB — i.e. the peak-to-peak grain is this.
    /// Sized to the worst downstream quantiser at the backdrop's own level.
    pub amplitude: f32,
    /// The source-over alpha the grain composites at. Small keeps the
    /// pre-compensation gentle; too small pushes the sprite's own colour far
    /// from the base. `1/16` is comfortable.
    pub opacity: f32,
}

/// Everything the backdrop is. Colours are linear scRGB, display-fitted by the
/// app: the fork applies no colour policy of its own.
#[derive(Clone, Debug)]
pub struct BackdropSpec {
    /// The opaque base the glows composite onto.
    pub base: [f32; 3],
    /// Radial layers, back-to-front (first listed draws first).
    pub glows: Vec<BackdropGlow>,
    pub dither: Option<BackdropDither>,
}

type Provider = Box<dyn Fn() -> Option<BackdropSpec> + Send + Sync + 'static>;

/// The app-registered provider. Set once, before the window is created.
static PROVIDER: OnceLock<Provider> = OnceLock::new();

/// Register the process-global backdrop provider. Call **before**
/// `DCompHost::render` (like [`crate::set_display_change_callback`]); only the
/// first registration is kept.
///
/// It is a provider rather than a one-shot value because the backdrop's colours
/// are display-fitted: the host re-queries it whenever the display's colour
/// capability may have changed, *after* the app's own re-fit has run, so the
/// backdrop can never be left holding a stale mapping. Returning `None` leaves
/// the plain window background in place.
pub fn set_root_backdrop_provider(p: impl Fn() -> Option<BackdropSpec> + Send + Sync + 'static) {
    let _ = PROVIDER.set(Box::new(p));
}

pub(crate) fn spec() -> Option<BackdropSpec> {
    PROVIDER.get().and_then(|p| p())
}

// ── Geometry ────────────────────────────────────────────────────────────────

/// Gradient stops per glow. Enough that piecewise-linear interpolation between
/// them tracks the smootherstep profile to well under one 8-bit LSB.
const EASED_STOPS: usize = 9;
/// Keyframes sampled along a drift path. A sinusoid sampled 24x has a
/// linear-interpolation error under 0.2% of its amplitude — invisible on a soft
/// glow, and the whole animation is built once.
const DRIFT_KEYS: u32 = 24;
/// The base layer's source surface. Any size works (it is one flat colour
/// stretched); this is the smallest that avoids a degenerate 1px surface.
const BASE_SRC: i32 = 8;

/// Ken Perlin's smootherstep — zero first derivative at BOTH ends, so the
/// falloff leaves its peak and arrives at transparent tangentially and no
/// Mach-band ring forms at the rim.
fn smootherstep(t: f32) -> f32 {
    t * t * t * (t * (6.0 * t - 15.0) + 10.0)
}

/// The ellipse a glow occupies, in DIPs: `(offset_x, offset_y, width, height)`.
///
/// CSS's default ending shape is `farthest-corner`: the ellipse passes through
/// the box corner farthest from the centre while keeping the aspect ratio of the
/// farthest horizontal/vertical edge distances, so for centre `(cx, cy)` with
/// farthest-edge distances `(fx, fy)` the radii are `(fx*sqrt2, fy*sqrt2)`. The
/// `transparent stop%` then scales that ray.
fn glow_rect(dip: (f32, f32), center: (f32, f32), stop: f32) -> (f32, f32, f32, f32) {
    let (w, h) = dip;
    let (cx, cy) = (center.0 * w, center.1 * h);
    let rx = stop * cx.max(w - cx) * std::f32::consts::SQRT_2;
    let ry = stop * cy.max(h - cy) * std::f32::consts::SQRT_2;
    (cx - rx, cy - ry, 2.0 * rx, 2.0 * ry)
}

/// `v' = (v - a*m) / (1 - a)` — see the module note. With no dither layer this
/// is the identity.
fn precompensate(v: [f32; 3], m: [f32; 3], alpha: f32) -> [f32; 3] {
    if alpha <= 0.0 || alpha >= 1.0 {
        return v;
    }
    let mut out = [0.0; 3];
    for i in 0..3 {
        out[i] = (v[i] - alpha * m[i]) / (1.0 - alpha);
    }
    out
}

// ── Painting ────────────────────────────────────────────────────────────────

/// Run `f` inside one `BeginDraw`/`EndDraw` on `interop`, with the transform
/// already translated to the surface's atlas origin (a drawing surface may be
/// placed inside a larger texture, so drawing at 0,0 without this lands in the
/// wrong tile).
fn draw_into(
    comp: &Compositing,
    interop: &ICompositionDrawingSurfaceInterop,
    f: impl FnOnce(&DrawingSession),
) -> Option<()> {
    let mut origin = POINT::default();
    let ctx = match unsafe { interop.BeginDraw(None, &mut origin) } {
        Ok(c) => c,
        Err(e) => {
            comp.note_error(&e);
            return None;
        }
    };
    // The atlas origin rides on the session, so a caller's own `set_transform`
    // composes with it instead of dropping it.
    let session = DrawingSession::from_borrowed_context(
        &ctx,
        Matrix3x2::translation(origin.x as f32, origin.y as f32),
    );
    f(&session);
    if let Err(e) = unsafe { interop.EndDraw() }.ok() {
        comp.note_error(&e);
        return None;
    }
    Some(())
}

/// A flat opaque FP16 surface — the base layer's source.
fn paint_base(
    comp: &Compositing,
    color: [f32; 3],
) -> Option<(CompositionDrawingSurface, CompositionSurfaceBrush)> {
    let (surface, interop, brush) = comp.new_source_surface(BASE_SRC, BASE_SRC).ok()?;
    draw_into(comp, &interop, |s| {
        s.clear(ColorF::scrgb(color[0], color[1], color[2], 1.0));
    })?;
    Some((surface, brush))
}

/// One glow blob: a circular smootherstep alpha ramp from the authored peak at
/// the centre to fully transparent at the inscribed rim. The square's corners
/// fall outside that circle and stay transparent (the radial brush clamps to its
/// last stop), so stretching the square into the layer's ellipse yields exactly
/// the authored `radial-gradient(ellipse ...)`.
///
/// Only alpha varies across the stops; every stop carries the peak's own
/// chromaticity, so the straight-alpha interpolation the FP16 stop collection
/// uses fades without a premultiplied dark-edge halo.
fn paint_glow(
    comp: &Compositing,
    res: i32,
    peak: [f32; 3],
) -> Option<(CompositionDrawingSurface, CompositionSurfaceBrush)> {
    let (surface, interop, brush) = comp.new_source_surface(res, res).ok()?;
    draw_into(comp, &interop, |s| {
        s.clear(ColorF::scrgb(0.0, 0.0, 0.0, 0.0));
        let r = res as f32 / 2.0;
        let stops: [GradientStop; EASED_STOPS] = std::array::from_fn(|i| {
            let t = i as f32 / (EASED_STOPS - 1) as f32;
            GradientStop::new(
                t,
                ColorF::scrgb(peak[0], peak[1], peak[2], 1.0 - smootherstep(t)),
            )
        });
        if let Ok(g) = s.create_radial_gradient(CVec2::new(r, r), r, r, &stops) {
            s.fill_rect(&Rect::from_xywh(0.0, 0.0, res as f32, res as f32), &g);
        }
    })?;
    Some((surface, brush))
}

/// Fill `interop` (a full-window surface) with the wrap-tiled grain.
///
/// The texels carry `m + d/alpha` at alpha 1 and the SPRITE carries `alpha` as
/// its opacity, rather than baking `alpha` into a premultiplied surface: the
/// compositor then performs `c*alpha + dst*(1-alpha)` itself and the surface
/// never holds a value scaled down into the bottom of FP16's precision.
fn paint_dither(
    comp: &Compositing,
    interop: &ICompositionDrawingSurfaceInterop,
    px: (i32, i32),
    m: [f32; 3],
    d: &BackdropDither,
) -> Option<()> {
    let n = d.tile_size as usize;
    if n == 0 || d.tile.len() < n * n || d.opacity <= 0.0 || d.opacity >= 1.0 {
        return None;
    }
    let mut texels = vec![0.0f32; n * n * 4];
    for (i, &o) in d.tile.iter().take(n * n).enumerate() {
        // The grain this texel must contribute, lifted by the sprite's own
        // opacity so that opacity scales it back down on composite.
        let v = o * d.amplitude / d.opacity;
        texels[i * 4] = m[0] + v;
        texels[i * 4 + 1] = m[1] + v;
        texels[i * 4 + 2] = m[2] + v;
        texels[i * 4 + 3] = 1.0;
    }
    draw_into(comp, interop, |s| {
        s.clear(ColorF::scrgb(m[0], m[1], m[2], 1.0));
        // NEAREST_NEIGHBOR + WRAP, one texel per physical pixel: the grain must
        // never be resampled or it stops being a dither.
        if let Ok(tile) = s.create_bitmap_fp16(d.tile_size, d.tile_size, &texels)
            && let Ok(b) = s.create_tiling_brush(&tile)
        {
            s.fill_rect(&Rect::from_xywh(0.0, 0.0, px.0 as f32, px.1 as f32), &b);
        }
    })
}

// ── Motion ──────────────────────────────────────────────────────────────────

fn ts_secs(s: f32) -> TimeSpan {
    TimeSpan {
        duration: (s.max(0.001) * 1.0e7) as i64,
    }
}

/// Start (or restart) a glow's endless drift about `base` (its placed offset).
///
/// Keyframes are sampled off a lemniscate and joined with LINEAR easing: the
/// path's own curvature supplies the organic feel, so per-segment easing would
/// only add a pulse at every keyframe. The frames at `0.0` and `1.0` coincide by
/// construction, which is what makes `Forever` seamless.
fn start_drift(sprite: &SpriteVisual, base: (f32, f32), d: GlowDrift) -> Option<()> {
    let obj: ICompositionObject = sprite.cast().ok()?;
    let compositor = obj.Compositor().ok()?;
    let anim = compositor.CreateVector3KeyFrameAnimation().ok()?;
    let kf3: IVector3KeyFrameAnimation = anim.cast().ok()?;
    let easing = compositor.CreateLinearEasingFunction().ok()?;
    let ease = easing.cast::<crate::system_bindings::CompositionEasingFunction>().ok()?;
    for i in 0..=DRIFT_KEYS {
        let t = i as f32 / DRIFT_KEYS as f32;
        let th = std::f32::consts::TAU * (t + d.phase);
        let x = base.0 + d.amplitude.0 * th.sin();
        let y = base.1 + d.amplitude.1 * (2.0 * th).sin();
        kf3.InsertKeyFrameWithEasingFunction(t, Vector3::new(x, y, 0.0), &ease)
            .ok()?;
    }
    let kf: IKeyFrameAnimation = anim.cast().ok()?;
    kf.SetDuration(ts_secs(d.period_secs)).ok()?;
    kf.SetIterationBehavior(AnimationIterationBehavior::Forever)
        .ok()?;
    obj.StartAnimation("Offset", &anim.cast::<CompositionAnimation>().ok()?)
        .ok()?;
    Some(())
}

// ── The retained stack ──────────────────────────────────────────────────────

/// One glow, split across TWO visuals on purpose.
///
/// `host` carries the layer's PLACEMENT (which depends on the window size) and
/// the sprite carries its MOTION (which does not). A resize therefore only moves
/// the host, leaving the drift animation on the sprite untouched and running.
///
/// Folding both into one visual is what a resize cannot survive: the animation
/// owns `Offset`, so re-placing means stopping it, writing the new anchor and
/// starting a fresh animation — which restarts the path at phase 0. Every
/// `WM_SIZE` in a drag would snap the glow back to the start of its loop.
struct Glow {
    host: ContainerVisual,
    host_visual: Visual,
    sprite_visual: Visual,
    center: (f32, f32),
    stop: f32,
    _sprite: SpriteVisual,
    _surface: CompositionDrawingSurface,
    _brush: CompositionSurfaceBrush,
}

/// The grain layer. Its surface is allocated ONCE, large enough to cover the
/// monitor, and is never resized or repainted.
///
/// It cannot be stretched — the grain is only a dither while one texel lands on
/// one physical pixel — so the alternative would be re-fitting the buffer to the
/// window on every resize. That is both a full-window FP16 repaint per `WM_SIZE`
/// and a correctness hazard: the layers beneath are pre-compensated for this one,
/// so any frame where it is not yet correct shows the whole backdrop jump by
/// `1/(1-opacity)`. Over-allocating instead means a resize touches nothing here;
/// the excess simply falls outside the window and is clipped.
struct Dither {
    visual: Visual,
    /// Fixed backing size in physical pixels.
    px: (i32, i32),
    _sprite: SpriteVisual,
    _surface: CompositionDrawingSurface,
    _brush: CompositionSurfaceBrush,
}

/// The built backdrop. Dropping it does NOT detach the visuals — call
/// [`Backdrop::remove`] with the compositing state that owns the root.
pub(crate) struct Backdrop {
    base: Visual,
    glows: Vec<Glow>,
    dither: Option<Dither>,
    _base_sprite: SpriteVisual,
    _base_surface: CompositionDrawingSurface,
    _base_brush: CompositionSurfaceBrush,
}

impl Backdrop {
    /// Build the whole stack and insert it above the window background. Every
    /// layer exists and is painted when this returns, so a caller that runs
    /// before `ShowWindow` gets a complete backdrop on the first composited
    /// frame.
    pub(crate) fn build(
        comp: &Compositing,
        spec: &BackdropSpec,
        dip: (f32, f32),
        scale: f32,
        monitor_px: (i32, i32),
    ) -> Option<Self> {
        let alpha = spec.dither.as_ref().map_or(0.0, |d| d.opacity);
        let m = spec.base;

        // ── base ──
        let base_color = precompensate(spec.base, m, alpha);
        let (base_surface, base_brush) = paint_base(comp, base_color)?;
        let base_sprite = comp.new_sprite().ok()?;
        base_sprite
            .SetBrush(&base_brush.cast::<CompositionBrush>().ok()?)
            .ok()?;
        let base_visual: Visual = base_sprite.cast().ok()?;
        base_visual.SetSize(Vector2::new(dip.0, dip.1)).ok()?;
        comp.attach_backdrop_visual(&base_visual).ok()?;

        // ── glows ──
        let mut glows = Vec::with_capacity(spec.glows.len());
        for g in &spec.glows {
            let res = g.resolution.clamp(8, 1024) as i32;
            let peak = precompensate(g.peak, m, alpha);
            let Some((surface, brush)) = paint_glow(comp, res, peak) else {
                continue;
            };
            let Ok(sprite) = comp.new_sprite() else { continue };
            if brush
                .cast::<CompositionBrush>()
                .and_then(|b| sprite.SetBrush(&b))
                .is_err()
            {
                continue;
            }
            let Ok(sprite_visual) = sprite.cast::<Visual>() else { continue };
            let Ok(host) = comp.new_container() else { continue };
            let Ok(host_visual) = host.cast::<Visual>() else { continue };
            if host
                .Children()
                .and_then(|c| c.InsertAtTop(&sprite_visual))
                .is_err()
                || comp.attach_backdrop_visual(&host_visual).is_err()
            {
                continue;
            }
            // Motion is anchored at the host's origin and started ONCE. Placement
            // moves the host underneath it, so the path is never interrupted.
            match g.drift {
                Some(d) if start_drift(&sprite, (0.0, 0.0), d).is_some() => {}
                _ => {
                    let _ = sprite_visual.SetOffset(Vector3::new(0.0, 0.0, 0.0));
                }
            }
            glows.push(Glow {
                host,
                host_visual,
                sprite_visual,
                center: g.center,
                stop: g.stop,
                _sprite: sprite,
                _surface: surface,
                _brush: brush,
            });
        }

        // ── dither ──
        // Sized to the monitor, not the window: a resize must never have to
        // touch this surface (see [`Dither`]). The overhang is clipped by the
        // window, and one texel still lands on one physical pixel because the
        // sprite is presented at exactly the surface's own pixel size.
        let dither = spec.dither.as_ref().and_then(|d| {
            let px = (
                monitor_px.0.max((dip.0 * scale).round() as i32).max(1),
                monitor_px.1.max((dip.1 * scale).round() as i32).max(1),
            );
            let (surface, interop, brush) = comp.new_source_surface(px.0, px.1).ok()?;
            paint_dither(comp, &interop, px, m, d)?;
            let sprite = comp.new_sprite().ok()?;
            sprite.SetBrush(&brush.cast::<CompositionBrush>().ok()?).ok()?;
            let visual: Visual = sprite.cast().ok()?;
            visual.SetOpacity(d.opacity).ok()?;
            comp.attach_backdrop_visual(&visual).ok()?;
            Some(Dither {
                visual,
                px,
                _sprite: sprite,
                _surface: surface,
                _brush: brush,
            })
        });

        let out = Self {
            base: base_visual,
            glows,
            dither,
            _base_sprite: base_sprite,
            _base_surface: base_surface,
            _base_brush: base_brush,
        };
        out.place(dip, scale);
        Some(out)
    }

    /// Size and position every layer for the current window.
    ///
    /// This is the WHOLE cost of a resize: a handful of visual property writes,
    /// no repaint and no animation restart anywhere. Everything that depends on
    /// pixels — the glow blobs, the grain — is either resolution-independent or
    /// over-allocated, precisely so this stays free.
    pub(crate) fn place(&self, dip: (f32, f32), scale: f32) {
        let _ = self.base.SetSize(Vector2::new(dip.0, dip.1));

        for g in &self.glows {
            let (x, y, w, h) = glow_rect(dip, g.center, g.stop);
            // Size the sprite, move the HOST. The sprite's own offset belongs to
            // its drift animation and is never written here.
            let _ = g.sprite_visual.SetSize(Vector2::new(w.max(1.0), h.max(1.0)));
            let _ = g.host_visual.SetOffset(Vector3::new(x, y, 0.0));
        }

        if let Some(d) = &self.dither {
            // Presented at the surface's own pixel size (converted to DIPs,
            // since the root applies the DPI scale), so texels stay 1:1 with
            // physical pixels. Independent of the window size — the overhang is
            // clipped — so this only ever changes on a DPI change.
            let s = scale.max(0.01);
            let _ = d.visual.SetSize(Vector2::new(
                d.px.0 as f32 / s,
                d.px.1 as f32 / s,
            ));
        }
    }

    /// Detach every layer from the compositor root (a rebuild, or teardown).
    pub(crate) fn remove(&self, comp: &Compositing) {
        if let Some(d) = &self.dither {
            comp.remove_backdrop_visual(&d.visual);
        }
        for g in &self.glows {
            comp.remove_backdrop_visual(&g.host_visual);
        }
        comp.remove_backdrop_visual(&self.base);
    }
}
