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
use std::time::Duration;

use super::bootstrap::Compositing;
use windows_canvas::{
    ColorF, DrawingSession, GradientStop, ID2D1DeviceContext, Matrix3x2, Rect, Vector2 as CVec2,
};
use windows_composition::{
    CompositionDrawingSurface, CompositionSurfaceBrush, ContainerVisual, SpriteVisual,
};
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

/// The ellipse a glow occupies, as FRACTIONS of the window:
/// `(offset_x, offset_y, width, height)`, each to be multiplied by the window's
/// width or height.
///
/// CSS's default ending shape is `farthest-corner`: the ellipse passes through
/// the box corner farthest from the centre while keeping the aspect ratio of the
/// farthest horizontal/vertical edge distances, so for centre `(cx, cy)` with
/// farthest-edge distances `(fx, fy)` the radii are `(fx*sqrt2, fy*sqrt2)`. The
/// `transparent stop%` then scales that ray.
///
/// Every term of that carries exactly one factor of the window extent —
/// `cx = c·w` and `rx = stop·√2·max(c, 1-c)·w` — so the whole rectangle divides
/// through by `(w, h)` and what is left depends only on the glow's own spec.
/// That is why placement can be stated ONCE as a relative size and offset and
/// never revisited: a resize changes `w` and `h`, and the compositor multiplies.
fn glow_frac(center: (f32, f32), stop: f32) -> (f32, f32, f32, f32) {
    // Half-extent of the ellipse along each axis, as a fraction of that axis.
    let half = |c: f32| stop * c.max(1.0 - c) * std::f32::consts::SQRT_2;
    let (ax, ay) = (half(center.0), half(center.1));
    (center.0 - ax, center.1 - ay, 2.0 * ax, 2.0 * ay)
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

/// Run `f` inside one `begin_draw`/`end_draw` on `surface`, with the transform
/// already translated to the surface's atlas origin (a drawing surface may be
/// placed inside a larger texture, so drawing at 0,0 without this lands in the
/// wrong tile).
fn draw_into(
    comp: &Compositing,
    surface: &CompositionDrawingSurface,
    f: impl FnOnce(&DrawingSession),
) -> Option<()> {
    let (ctx, (origin_x, origin_y)) = match surface.begin_draw::<ID2D1DeviceContext>() {
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
        Matrix3x2::translation(origin_x as f32, origin_y as f32),
    );
    f(&session);
    if let Err(e) = surface.end_draw() {
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
    let (surface, brush) = comp.new_source_surface(BASE_SRC, BASE_SRC).ok()?;
    draw_into(comp, &surface, |s| {
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
    let (surface, brush) = comp.new_source_surface(res, res).ok()?;
    draw_into(comp, &surface, |s| {
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

/// Fill `surface` (a full-window surface) with the wrap-tiled grain.
///
/// The texels carry `m + d/alpha` at alpha 1 and the SPRITE carries `alpha` as
/// its opacity, rather than baking `alpha` into a premultiplied surface: the
/// compositor then performs `c*alpha + dst*(1-alpha)` itself and the surface
/// never holds a value scaled down into the bottom of FP16's precision.
fn paint_dither(
    comp: &Compositing,
    surface: &CompositionDrawingSurface,
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
    draw_into(comp, surface, |s| {
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

/// A drift period as a [`Duration`], clamped exactly as the retired `ts_secs`
/// clamped its 100ns tick count.
///
/// The floor is not cosmetic here: `period_secs` is app-supplied, and
/// `Duration::from_secs_f32` PANICS on a negative or non-finite value where the
/// old integer conversion merely saturated. The clamp is what keeps a bad spec
/// a slow glow rather than a crash.
fn drift_period(secs: f32) -> Duration {
    Duration::from_secs_f32(if secs.is_finite() { secs.max(0.001) } else { 0.001 })
}

/// Start a glow's endless drift about `base` (its placed offset).
///
/// Keyframes are sampled off a lemniscate and joined with LINEAR easing: the
/// path's own curvature supplies the organic feel, so per-segment easing would
/// only add a pulse at every keyframe. The frames at `0.0` and `1.0` coincide by
/// construction, which is what makes `Forever` seamless.
fn start_drift(sprite: &SpriteVisual, base: (f32, f32), d: GlowDrift) {
    let compositor = sprite.compositor();
    let anim = compositor.create_vector3_key_frame_animation();
    let ease = compositor.create_linear_easing_function();
    for i in 0..=DRIFT_KEYS {
        let t = i as f32 / DRIFT_KEYS as f32;
        let th = std::f32::consts::TAU * (t + d.phase);
        let x = base.0 + d.amplitude.0 * th.sin();
        let y = base.1 + d.amplitude.1 * (2.0 * th).sin();
        anim.insert_key_frame_with_easing(t, Vector3::new(x, y, 0.0), &ease);
    }
    anim.set_duration(drift_period(d.period_secs));
    anim.set_iterate_forever();
    sprite.start_animation("Offset", &anim);
}

// ── The retained stack ──────────────────────────────────────────────────────

/// One glow, split across TWO visuals on purpose.
///
/// The host carries the layer's PLACEMENT (which scales with the window) and the
/// sprite carries its MOTION (which does not). Both are declared once at build
/// time — placement as a relative offset and size, so the compositor re-derives
/// it from the window's own extent — and neither is written again.
///
/// Folding both into one visual is what the drift cannot survive: the animation
/// owns `Offset`, so placing on that same visual would mean stopping it, writing
/// the new anchor and starting a fresh animation — restarting the path at phase
/// 0. The split keeps placement and motion on different visuals, and expressing
/// placement relatively then removes the resize write entirely, so a `WM_SIZE`
/// storm during a drag touches neither.
struct Glow {
    /// Carries the layer's PLACEMENT, as fractions of the window — and is
    /// therefore the sprite's own size anchor. Read once more at teardown.
    host: ContainerVisual,
    /// Carries its MOTION. Held only to keep the drift's target alive; its size
    /// and offset are both stated at build time.
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
    sprite: SpriteVisual,
    /// Fixed backing size in physical pixels.
    px: (i32, i32),
    _surface: CompositionDrawingSurface,
    _brush: CompositionSurfaceBrush,
}

/// The built backdrop. Dropping it does NOT detach the visuals — call
/// [`Backdrop::remove`] with the compositing state that owns the root.
pub(crate) struct Backdrop {
    base: SpriteVisual,
    glows: Vec<Glow>,
    dither: Option<Dither>,
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
        let base_sprite = comp.new_sprite();
        base_sprite.set_brush(&base_brush);
        // A tiny surface stretched over the whole window — so say "the whole
        // window" once, against the backdrop band, rather than re-stating the
        // window's size here on every resize.
        base_sprite.fill_parent();
        comp.attach_backdrop_visual(&base_sprite);

        // ── glows ──
        let mut glows = Vec::with_capacity(spec.glows.len());
        for g in &spec.glows {
            let res = g.resolution.clamp(8, 1024) as i32;
            let peak = precompensate(g.peak, m, alpha);
            let Some((surface, brush)) = paint_glow(comp, res, peak) else {
                continue;
            };
            let sprite = comp.new_sprite();
            sprite.set_brush(&brush);
            let host = comp.new_container();
            host.children().insert_at_top(&sprite);
            comp.attach_backdrop_visual(&host);
            // ── Placement, stated once ──
            // The host carries the ellipse's box as pure fractions of the window
            // (see `glow_frac`), which makes it a size anchor in its own right;
            // the sprite then fills it. A resize multiplies both by the new
            // extent inside the compositor, so this layer has no resize path at
            // all — which is also what protects the drift below, since re-placing
            // a running animation's target would have meant stopping it.
            let (fx, fy, fw, fh) = glow_frac(g.center, g.stop);
            host.set_relative_offset_adjustment(Vector3::new(fx, fy, 0.0));
            host.set_relative_size_adjustment(Vector2::new(fw, fh));
            sprite.fill_parent();
            // Motion is anchored at the host's origin and started ONCE. The host
            // carries placement underneath it, so the path is never interrupted.
            match g.drift {
                Some(d) => start_drift(&sprite, (0.0, 0.0), d),
                // A pinned layer sits at its host's origin and stays there.
                None => sprite.set_offset(0.0, 0.0, 0.0),
            }
            glows.push(Glow {
                host,
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
            let (surface, brush) = comp.new_source_surface(px.0, px.1).ok()?;
            paint_dither(comp, &surface, px, m, d)?;
            let sprite = comp.new_sprite();
            sprite.set_brush(&brush);
            sprite.set_opacity(d.opacity);
            comp.attach_backdrop_visual(&sprite);
            Some(Dither {
                sprite,
                px,
                _surface: surface,
                _brush: brush,
            })
        });

        let out = Self {
            base: base_sprite,
            glows,
            dither,
            _base_surface: base_surface,
            _base_brush: base_brush,
        };
        out.place(scale);
        Some(out)
    }

    /// Re-place what the window's SIZE cannot carry.
    ///
    /// The base and the glows are stated as fractions of the window at build
    /// time, so a resize re-derives them inside the compositor and this function
    /// does not touch them — a `WM_SIZE` storm during a drag costs zero writes
    /// for every layer but one.
    ///
    /// The grain is the exception, and not by omission: it is sized to the
    /// MONITOR in physical pixels so that one texel lands on one physical pixel,
    /// which is a ratio against `scale`, not against the window. It therefore
    /// changes on a DPI change and on nothing else — a resize at constant DPI
    /// writes the same value back.
    pub(crate) fn place(&self, scale: f32) {
        if let Some(d) = &self.dither {
            // Presented at the surface's own pixel size (converted to DIPs,
            // since the root applies the DPI scale), so texels stay 1:1 with
            // physical pixels. Independent of the window size — the overhang is
            // clipped — so this only ever changes on a DPI change.
            let s = scale.max(0.01);
            d.sprite.set_size(d.px.0 as f32 / s, d.px.1 as f32 / s);
        }
    }

    /// Detach every layer from the compositor root (a rebuild, or teardown).
    pub(crate) fn remove(&self, comp: &Compositing) {
        if let Some(d) = &self.dither {
            comp.remove_backdrop_visual(&d.sprite);
        }
        for g in &self.glows {
            comp.remove_backdrop_visual(&g.host);
        }
        comp.remove_backdrop_visual(&self.base);
    }
}

#[cfg(test)]
mod tests {
    use super::glow_frac;

    /// The absolute ellipse, computed the way it was before placement became
    /// relative: everything in DIPs, from the window's own extent.
    fn absolute(dip: (f32, f32), center: (f32, f32), stop: f32) -> (f32, f32, f32, f32) {
        let (w, h) = dip;
        let (cx, cy) = (center.0 * w, center.1 * h);
        let rx = stop * cx.max(w - cx) * std::f32::consts::SQRT_2;
        let ry = stop * cy.max(h - cy) * std::f32::consts::SQRT_2;
        (cx - rx, cy - ry, 2.0 * rx, 2.0 * ry)
    }

    /// The compositor multiplies the fractions by the parent's extent, so the
    /// contract is that doing the same by hand reproduces the absolute rect.
    ///
    /// Compared with a tolerance rather than bit-exactly on purpose: factoring
    /// the extent out of `stop * (c*w).max(w - c*w) * √2` into
    /// `w * (stop * max(c, 1-c) * √2)` reassociates the multiply, which is a
    /// legal ULP's worth of difference and not a behavioural one.
    #[test]
    fn fractions_scale_back_to_the_absolute_rect() {
        let dips = [(1280.0, 800.0), (1750.0, 640.0), (320.0, 1200.0), (1.0, 1.0)];
        let centers = [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.15, 0.85), (0.72, 0.31)];
        let stops = [0.05, 0.5, 1.0, 1.6];

        for dip in dips {
            for center in centers {
                for stop in stops {
                    let (fx, fy, fw, fh) = glow_frac(center, stop);
                    let got = (fx * dip.0, fy * dip.1, fw * dip.0, fh * dip.1);
                    let want = absolute(dip, center, stop);
                    // Relative to the extent: these are DIP lengths, so a fixed
                    // epsilon would be far too strict at 1750 and far too loose at 1.
                    let tol = dip.0.max(dip.1) * 1e-6;
                    for (g, w) in [
                        (got.0, want.0),
                        (got.1, want.1),
                        (got.2, want.2),
                        (got.3, want.3),
                    ] {
                        assert!(
                            (g - w).abs() <= tol,
                            "dip {dip:?} center {center:?} stop {stop}: got {g} want {w}"
                        );
                    }
                }
            }
        }
    }

    /// A glow's box must not depend on the window at all — that independence is
    /// the whole reason placement can be declared once and never revisited.
    #[test]
    fn fractions_are_independent_of_the_window() {
        assert_eq!(glow_frac((0.3, 0.7), 0.8), glow_frac((0.3, 0.7), 0.8));
        // A centred, full-stop glow reaches √2 times the half-extent each way,
        // i.e. it overhangs the box — which is what `farthest-corner` means.
        let (fx, fy, fw, fh) = glow_frac((0.5, 0.5), 1.0);
        let half = 0.5 * std::f32::consts::SQRT_2;
        assert!((fw - 2.0 * half).abs() < 1e-6 && (fh - 2.0 * half).abs() < 1e-6);
        assert!((fx - (0.5 - half)).abs() < 1e-6 && (fy - (0.5 - half)).abs() < 1e-6);
    }
}
