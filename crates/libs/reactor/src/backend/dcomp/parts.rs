//! Retained **chrome parts**: the animated fragments of the drawn controls
//! (indicator pills, toggle knobs, slider fills, hover/press ink) lifted out of
//! the per-node painted surface into their own compositor sprites, so their
//! motion runs entirely on the system compositor (DWM) — no app tick, no
//! repaint, no timer while they move.
//!
//! Three pieces:
//!
//! - [`Atlas`] — tiny FP16 source surfaces (a solid, a rounded bar, a circle)
//!   rasterized ONCE per (shape, colour, scale) and shared by every part that
//!   needs that look. Rounded bars stretch through a `CompositionNineGridBrush`
//!   (9-slice), so one source serves any width with pristine corners. Cleared
//!   whenever the mapped colours may have changed (display change, DPI change,
//!   device loss) — parts re-bind by epoch.
//!
//!   **Bounding.** Two of the key's dimensions are LAYOUT-derived, not
//!   token-derived — a bar's height comes from `node.rect.h`, and a slider's
//!   fill colour is whatever the app authored — so the raw key is unbounded: a
//!   drag-resize or an animated fill colour would mint one FP16 surface per
//!   frame. The key is therefore QUANTIZED at construction ([`snap_extent`],
//!   [`snap_len`], [`quant_channel`]) onto the granularity the raster itself
//!   already has, and [`ATLAS_CAP`] caps the map with LRU eviction as a
//!   backstop. See those items for the exact grids and why each is lossless.
//! - [`Part`] — one `SpriteVisual` plus cached **retargetable compositor
//!   springs** for Offset / Size / Opacity. A state change is `SetFinalValue`
//!   + `StartAnimation` on the cached object (no per-event allocation); a drag
//!   snap is `StopAnimation` + a plain property set. Spring tuning matches the
//!   retired CPU springs (`k = 520, c = 40`), so the feel is unchanged.
//! - Per-kind `sync` — the single writer that reconciles a control's parts
//!   against its logical state from the paint pass: glides on a state change,
//!   snaps on first placement / resize (mounting never flies in).
//!
//! Interaction events (hover, press, drag) retarget the springs directly via
//! [`ink_state_changed`] / [`slider_drag`] / [`seg_hot_changed`] — no frame
//! timer is involved anywhere in control motion.
//!
//! Z-order contract (upheld here at creation and by `layout::sync` on a child
//! re-sync): *below* parts sit under the node's painted surface (tray / pill /
//! indicator under the labels), *above* parts sit over it (ink wash, slider
//! fill + thumb over the painted groove).
//!
//! A node with **no** surface has no band to straddle — the button family draws
//! nothing at all (`Node::has_chrome`). The two groups then keep their relative
//! order simply by being created in it, which is the same z-order the banded
//! case produces; see [`ensure`].

use rustc_hash::FxHashMap;

use super::bootstrap::Compositing;
use super::nav;
use super::node::{linear, Node};
use super::theme;
use crate::backend::ControlKind;
use crate::system_bindings::{
    AnimationIterationBehavior, CompositionAnimation, CompositionBrush, CompositionClip,
    CompositionDrawingSurface, CompositionEasingFunction, CompositionNineGridBrush,
    CompositionObject, CompositionSurfaceBrush, ICompositionAnimation, ICompositionObject,
    ICompositionObject4, ICompositor4, IKeyFrameAnimation,
    CompositionBatchTypes, CompositionScopedBatch, ISpringVector2NaturalMotionAnimation,
    ISpringVector3NaturalMotionAnimation, IVector2NaturalMotionAnimation,
    IVector3NaturalMotionAnimation, IVisual, InsetClip, SpringVector2NaturalMotionAnimation,
    SpringVector3NaturalMotionAnimation, SpriteVisual, TimeSpan, Visual,
};
use windows_canvas_core::{
    Brush, ColorF, DrawingSession, Ellipse, GradientStop, Rect, RoundedRect, Vector2 as CVec2,
};
use windows_core::Interface;
use windows_numerics::{Matrix3x2, Vector2, Vector3};

/// The SCROLL-CARRIER tuning, matching the retired CPU spring (`node::Spring`:
/// `k = 520`, `c = 40`): natural period `2π/√k`, damping ratio `c / (2√k)`.
/// Read by `Node::scroll_glide`, where carrying momentum is the point.
pub(crate) const SPRING_PERIOD: f32 = 0.2756;
pub(crate) const SPRING_DAMPING: f32 = 0.877;

/// The CONTROL-CHROME tuning — every [`Part::glide`], and nothing else.
///
/// Split from the scroll carrier because they are different motions: a scroll
/// surface carries momentum, an indicator reports a choice the user already
/// made and should simply be there. Inheriting the carrier's period made a
/// selection pill travel for roughly a fifth of a second, which reads as lag
/// rather than as motion.
///
/// Tuned BY EYE, and it has to be: the textbook settling time for a spring of
/// this period predicts a motion several times shorter than what the compositor
/// actually plays, so `Period` here does not mean the undamped natural period a
/// second-order model would assume. Do not re-derive this constant from the
/// scroll carrier's `k`/`c` — that derivation is what made a selection pill
/// travel for what felt like a fifth of a second.
///
/// What IS dependable is that duration scales linearly with `PERIOD` at a fixed
/// damping ratio, so halving this halves the travel. Tune the period; leave the
/// damping ratio alone unless the complaint is overshoot rather than speed.
///
/// What must NOT change is that ONE pair serves every `Part::glide` —
/// `slider_settle` reads a batch completion as "the derived fill has arrived",
/// which is only sound while every animation in that batch shares this tuning.
const CHROME_SPRING_PERIOD: f32 = 0.025;
const CHROME_SPRING_DAMPING: f32 = 0.90;

/// The travel [`CHROME_SPRING_PERIOD`] is tuned against — about one segment of a
/// selector bar, or one row of a nav pane.
const REF_TRAVEL: f32 = 60.0;

/// The spring period a glide covering `dist` DIPs plays at.
///
/// A spring settles in the same time WHATEVER the distance, so a long move does
/// not take longer — it travels faster. Uncorrected, that makes a single tuning
/// impossible to choose: the value that keeps a segment pill crisp throws a
/// pane-height indicator across the pane, and the value that makes the long move
/// calm leaves the short one wallowing.
///
/// Duration therefore follows distance the way Fluent and Material both specify
/// it: SUB-linearly, so travel ten times longer takes about three times longer
/// rather than ten, and clamped at both ends so nothing is instant and nothing
/// drags. Equal distances still yield equal periods, which is what keeps the
/// slider's settle batch (`slider_settle`) coherent — its halo and thumb track
/// the same value and so always travel together.
pub(crate) fn spring_period(dist: f32) -> f32 {
    let scale = (dist.max(1.0) / REF_TRAVEL).sqrt().clamp(0.65, 3.0);
    CHROME_SPRING_PERIOD * scale
}

/// `TimeSpan` (100 ns units) from seconds.
fn ts_secs(s: f32) -> TimeSpan {
    TimeSpan {
        duration: (s.max(0.001) * 1.0e7) as i64,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Atlas — shared rasterized part sources
// ─────────────────────────────────────────────────────────────────────────────

// ── Key quantization ─────────────────────────────────────────────────────────
//
// Everything that reaches an `AtlasKey` passes through one of these first. They
// exist to make the key space FINITE: without them the key carries raw layout
// floats and raw app-authored colours, and the cache grows without limit.

/// Physical-pixel granularity for a source's EXTENTS (bar height, circle
/// diameter, checkbox side). Whole physical pixels — and that is exactly
/// lossless, not merely close: [`rasterize`] already sizes the surface
/// `round(dip · scale)` px, so two DIP extents that land on the same pixel count
/// produce byte-identical pixels today. Snapping here just stops a drag-resize
/// from minting one FP16 surface per distinct sub-pixel height.
const EXTENT_STEPS_PER_PX: f32 = 1.0;

/// Granularity for RADII and STROKE WIDTHS — a quarter physical pixel. These are
/// token constants or derived from an already-snapped extent (`h / 2`), so they
/// are bounded regardless; the finer grid costs nothing and keeps a 1.5-DIP
/// hairline from rounding up to 2. As a bonus a nine-grid inset (`r · scale`)
/// now lands on a quarter-pixel instead of an arbitrary fraction.
const DETAIL_STEPS_PER_PX: f32 = 4.0;

/// Snap a DIP length onto a physical-pixel grid, returning the canonical `f32`
/// to key on. Deterministic (equal inputs give bit-identical output) and
/// non-negative; a non-finite input collapses to `0.0` so a stray NaN cannot
/// mint one entry per distinct NaN payload.
fn snap_len(dip: f32, scale: f32, steps_per_px: f32) -> f32 {
    if !dip.is_finite() || dip <= 0.0 {
        return 0.0;
    }
    let grid = (scale * steps_per_px).max(1.0e-3);
    (dip * grid).round() / grid
}

/// [`snap_len`] for an extent: whole physical pixels, and a positive extent
/// never collapses to zero (a sub-pixel bar still rasterizes one pixel tall,
/// which is what `rasterize`'s own `.max(1)` would have produced anyway).
fn snap_extent(dip: f32, scale: f32) -> f32 {
    if !dip.is_finite() || dip <= 0.0 {
        return 0.0;
    }
    let grid = (scale * EXTENT_STEPS_PER_PX).max(1.0e-3);
    (dip * grid).round().max(1.0) / grid
}

/// Canonical DIP→px scale. Display scales are a short list (1.0, 1.25, 1.5, …);
/// rounding to 1/1000 keeps any float noise in the DPI computation from forking
/// the whole atlas, and every dimension above is snapped against THIS value so
/// the key is self-consistent.
fn snap_scale(scale: f32) -> f32 {
    if !scale.is_finite() || scale <= 0.0 {
        return 1.0;
    }
    (scale * 1000.0).round() / 1000.0
}

/// Colour-channel quantization steps per unit of the signed-sqrt encoding.
const COLOR_STEPS: f32 = 4096.0;

/// Quantize one scRGB channel to a bounded integer code.
///
/// This is an FP16 **extended-range** pipeline: a channel may be negative (an
/// out-of-gamut primary) or far above 1.0 (an above-paper-white highlight), so
/// the quantizer must NOT clamp to `[0, 1]` — doing so would crush exactly the
/// HDR values the FP16 surfaces exist to carry. Instead the channel is encoded
/// through a SIGNED square root before the uniform step, which is sign-symmetric
/// and exact at zero, and spends resolution where the eye is: the resulting step
/// in linear light is `≈ 2·√|v| / COLOR_STEPS`, i.e. ~1/2048 at paper white
/// (finer than 16-bit), quadratically finer approaching black, and ~1.7e-3 at
/// 12× paper white — a relative error of 1.4e-4 up in the highlights. Magnitudes
/// are preserved; nothing is clipped.
///
/// Bounded by construction: |code| ≤ `√|v| · COLOR_STEPS`, so even a 16.0
/// channel (≈3200 nits at 203-nit paper white) codes to ±16384.
fn quant_channel(v: f32) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    let e = v.abs().sqrt().copysign(v);
    (e * COLOR_STEPS).round() as i32
}

/// Inverse of [`quant_channel`] — the colour the raster is actually painted in.
fn dequant_channel(q: i32) -> f32 {
    let e = q as f32 / COLOR_STEPS;
    (e * e).copysign(e)
}

fn color_bits(c: crate::Color) -> [i32; 4] {
    [
        quant_channel(c.r),
        quant_channel(c.g),
        quant_channel(c.b),
        quant_channel(c.a),
    ]
}

fn color_of(bits: [i32; 4]) -> crate::Color {
    crate::Color {
        r: dequant_channel(bits[0]),
        g: dequant_channel(bits[1]),
        b: dequant_channel(bits[2]),
        a: dequant_channel(bits[3]),
    }
}

/// One quantized gradient stop — position and colour, both on the grids above.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct QStop {
    /// Position in 1/65536ths of the ramp (positions are a `0..=1` fraction).
    pos: i32,
    color: [i32; 4],
}

impl QStop {
    fn of((p, c): &(f64, crate::Color)) -> Self {
        let pos = if p.is_finite() { (*p * 65536.0).round() as i32 } else { 0 };
        Self { pos, color: color_bits(*c) }
    }
    fn position(self) -> f32 {
        self.pos as f32 / 65536.0
    }
    fn color(self) -> crate::Color {
        color_of(self.color)
    }
}

/// The rasterized shape of an atlas source. Dimensions are quantized DIP `f32`
/// bit patterns so the key is `Eq + Hash` without float caveats.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ShapeKey {
    /// A solid fill; stretches to any size (4×4 px source).
    Solid,
    /// A horizontally stretchable rounded bar of fixed DIP height `h` and
    /// corner radius `r`; filled when `stroke_w == 0`, stroked otherwise.
    /// Served through a per-part nine-grid brush.
    HBar { h: u32, r: u32, stroke_w: u32 },
    /// An exact circle of DIP diameter `d` (drawn 1:1, no stretch).
    Circle { d: u32 },
    /// A checkmark glyph (two strokes) in a `d`×`d` DIP box (drawn 1:1).
    Check { d: u32 },
    /// A horizontal linear-gradient bar of DIP height `h` and corner radius `r`,
    /// rasterized at a fixed source width and stretched to any destination width
    /// (the stretch interpolates the ramp linearly, which is exactly the
    /// gradient's own math). When `r > 0` it is served through a per-part
    /// nine-grid brush so the rounded ends stay crisp while the middle stretches
    /// (the meter fill); `r == 0` is a plain full-bleed stretch (the knob arc
    /// stroke brush).
    ///
    /// The key carries the QUANTIZED STOP LIST ITSELF, not a digest of it. A
    /// digest is not injective: `FxHash` is a fast, non-cryptographic mixer, so
    /// two different ramps can collide, and a collision on a cache key means the
    /// meter silently renders the WRONG colour ramp with no error anywhere.
    /// Hashing may still collide here — that is fine and expected, because the
    /// map resolves a bucket collision with `Eq`, and `Eq` now compares the
    /// stops. The list is `Rc`-shared, so the common case is a pointer compare
    /// and the worst case a walk of a handful of stops.
    GradBar { stops: std::rc::Rc<[QStop]>, r: u32, h: u32 },
}

/// Atlas cache key: shape + the *authored* token colour (the display colour
/// map is applied at rasterize time) + the DIP→px scale it was drawn at.
///
/// Every field is quantized at construction — see the module header. Not `Copy`:
/// a gradient bar owns its stop list.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct AtlasKey {
    shape: ShapeKey,
    color: [i32; 4],
    scale: u32,
}

impl AtlasKey {
    fn solid(c: crate::Color, scale: f32) -> Self {
        let scale = snap_scale(scale);
        Self { shape: ShapeKey::Solid, color: color_bits(c), scale: scale.to_bits() }
    }
    fn hbar(h: f32, r: f32, stroke_w: f32, c: crate::Color, scale: f32) -> Self {
        let scale = snap_scale(scale);
        Self {
            shape: ShapeKey::HBar {
                h: snap_extent(h, scale).to_bits(),
                r: snap_len(r, scale, DETAIL_STEPS_PER_PX).to_bits(),
                stroke_w: snap_len(stroke_w, scale, DETAIL_STEPS_PER_PX).to_bits(),
            },
            color: color_bits(c),
            scale: scale.to_bits(),
        }
    }
    fn circle(d: f32, c: crate::Color, scale: f32) -> Self {
        let scale = snap_scale(scale);
        Self {
            shape: ShapeKey::Circle { d: snap_extent(d, scale).to_bits() },
            color: color_bits(c),
            scale: scale.to_bits(),
        }
    }
    fn check(d: f32, c: crate::Color, scale: f32) -> Self {
        let scale = snap_scale(scale);
        Self {
            shape: ShapeKey::Check { d: snap_extent(d, scale).to_bits() },
            color: color_bits(c),
            scale: scale.to_bits(),
        }
    }
    fn grad_bar(stops: &[(f64, crate::Color)], r: f32, h: f32, scale: f32) -> Self {
        let scale = snap_scale(scale);
        Self {
            shape: ShapeKey::GradBar {
                stops: stops.iter().map(QStop::of).collect(),
                r: snap_len(r, scale, DETAIL_STEPS_PER_PX).to_bits(),
                h: snap_extent(h, scale).to_bits(),
            },
            color: [0; 4],
            scale: scale.to_bits(),
        }
    }

    /// Whether this key is exactly the gradient bar `stops` / `r` / `h` / `scale`
    /// describe. Allocation-free, so a steady repaint can reuse a cached key
    /// (see [`grad_bar_key`]) instead of building a fresh stop list each frame.
    fn is_grad_bar(&self, stops: &[(f64, crate::Color)], r: f32, h: f32, scale: f32) -> bool {
        let scale = snap_scale(scale);
        let ShapeKey::GradBar { stops: have, r: kr, h: kh } = &self.shape else {
            return false;
        };
        self.scale == scale.to_bits()
            && *kr == snap_len(r, scale, DETAIL_STEPS_PER_PX).to_bits()
            && *kh == snap_extent(h, scale).to_bits()
            && have.len() == stops.len()
            && have.iter().zip(stops).all(|(q, s)| *q == QStop::of(s))
    }

    /// The nine-grid corner inset in source pixels (`r * scale`), 0 for the
    /// shapes that stretch uniformly.
    fn inset_px(&self) -> f32 {
        let scale = f32::from_bits(self.scale);
        match &self.shape {
            ShapeKey::HBar { r, .. } | ShapeKey::GradBar { r, .. } => f32::from_bits(*r) * scale,
            _ => 0.0,
        }
    }
    /// Whether this source is served through a horizontal nine-grid brush
    /// (rounded ends preserved, middle stretched) rather than a plain stretch.
    fn uses_nine_grid(&self) -> bool {
        match &self.shape {
            ShapeKey::HBar { .. } => true,
            ShapeKey::GradBar { r, .. } => f32::from_bits(*r) > 0.0,
            _ => false,
        }
    }
}

/// The gradient-bar key for these stops, reusing `cache` when nothing about the
/// ramp changed — the reuse path clones an `Rc` (a refcount bump), so a meter
/// repaint allocates nothing.
fn grad_bar_key(
    cache: &mut Option<AtlasKey>,
    stops: &[(f64, crate::Color)],
    r: f32,
    h: f32,
    scale: f32,
) -> AtlasKey {
    if let Some(k) = cache.as_ref()
        && k.is_grad_bar(stops, r, h, scale)
    {
        return k.clone();
    }
    let k = AtlasKey::grad_bar(stops, r, h, scale);
    *cache = Some(k.clone());
    k
}

struct AtlasEntry {
    brush: CompositionSurfaceBrush,
    // Keeps the pixels alive behind the brush.
    _surface: CompositionDrawingSurface,
    /// Logical clock reading of the last bind — the LRU ordering.
    used: u64,
}

/// Hard cap on live atlas sources, enforced by LRU eviction.
///
/// Sized to hold the entire legitimate working set with headroom: ~16 converted
/// control kinds bind 1–4 sources each, times the handful of distinct pixel
/// heights and token colours live at one scale — a rich window sits well under a
/// hundred. 256 therefore never evicts in steady state, while capping the
/// worst case: sources are tiny (a solid is 4×4 px; the widest, a gradient bar,
/// is 256 px × the bar height in FP16), so a full atlas is single-digit MB
/// rather than unbounded. Eviction is an O(n) scan of at most 256 entries, and
/// only on a miss at capacity.
///
/// Evicting a source a sprite is still using is safe: the sprite's
/// `CompositionSurfaceBrush` holds its own reference to the surface, so it keeps
/// rendering the pixels it bound. The part simply re-rasterizes if it ever
/// re-binds that key.
const ATLAS_CAP: usize = 256;

/// Rasterized part sources, shared across every control.
///
/// Bounded two ways: the key is quantized so layout floats and app-authored
/// colours cannot fork it without limit (module header), and [`ATLAS_CAP`] caps
/// the live count with LRU eviction as a backstop. Cleared wholesale on any edge
/// that can change the mapped colours or the pixel scale.
#[derive(Default)]
pub(crate) struct Atlas {
    map: FxHashMap<AtlasKey, AtlasEntry>,
    /// Bumped on [`clear`](Self::clear); parts re-bind when their bound epoch
    /// no longer matches.
    epoch: u32,
    /// Monotonic bind counter driving the LRU ordering.
    clock: u64,
}

impl Atlas {
    /// Drop every cached source (display / DPI / theme / device edge). Parts
    /// keep their current brush alive via the sprite's own COM reference until
    /// they re-bind on the next sync.
    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.epoch = self.epoch.wrapping_add(1);
    }

    /// The current cache epoch — non-`Part` chrome (the Knob) reads it to know
    /// when its own rasterized brushes must be rebuilt (display/DPI/theme edge).
    pub(crate) fn epoch(&self) -> u32 {
        self.epoch
    }

    fn entry(&mut self, comp: &Compositing, key: &AtlasKey) -> Option<&AtlasEntry> {
        self.clock += 1;
        let now = self.clock;
        if !self.map.contains_key(key) {
            let entry = rasterize(comp, key)?;
            if self.map.len() >= ATLAS_CAP {
                self.evict_lru();
            }
            self.map.insert(key.clone(), AtlasEntry { used: now, ..entry });
        }
        let e = self.map.get_mut(key)?;
        e.used = now;
        Some(e)
    }

    /// Drop the least recently bound source. Called only on a miss at capacity.
    fn evict_lru(&mut self) {
        if let Some(k) = self
            .map
            .iter()
            .min_by_key(|(_, e)| e.used)
            .map(|(k, _)| k.clone())
        {
            self.map.remove(&k);
        }
    }
}

/// Fixed source width (px) a gradient bar is rasterized at; the sprite's
/// Fill-stretch interpolates it to any destination width losslessly.
const GRAD_SRC_W: f32 = 256.0;
/// Gradient bar source height (px) — the ramp is horizontal, so height is a
/// uniform stretch.
const GRAD_SRC_H: f32 = 16.0;

/// Draw one atlas source: an FP16 surface of the shape's exact pixel size,
/// painted through the app's output colour map ([`linear`]).
///
/// The gradient stops come out of the KEY, so the pixels cannot disagree with
/// the thing they are cached under.
fn rasterize(comp: &Compositing, key: &AtlasKey) -> Option<AtlasEntry> {
    let scale = f32::from_bits(key.scale).max(0.01);
    let color = color_of(key.color);
    // DIP geometry of the source.
    let (dip_w, dip_h) = match &key.shape {
        ShapeKey::Solid => (4.0 / scale, 4.0 / scale),
        // Corners plus a 2-DIP stretchable centre column.
        ShapeKey::HBar { h, r, .. } => (2.0 * f32::from_bits(*r) + 2.0, f32::from_bits(*h)),
        ShapeKey::Circle { d } | ShapeKey::Check { d } => {
            (f32::from_bits(*d), f32::from_bits(*d))
        }
        // Wide source for gradient resolution; rasterized at the bar's actual
        // DIP height so a rounded end's corner is circular (never vertically
        // stretched by the nine-grid, whose insets are horizontal only).
        ShapeKey::GradBar { h, .. } => {
            let hh = f32::from_bits(*h);
            (GRAD_SRC_W / scale, if hh > 0.0 { hh } else { GRAD_SRC_H / scale })
        }
    };
    let px_w = ((dip_w * scale).round() as i32).max(1);
    let px_h = ((dip_h * scale).round() as i32).max(1);

    let (surface, interop, brush) = comp.new_source_surface(px_w, px_h).ok()?;
    let mut origin = crate::system_bindings::POINT::default();
    comp.device_lost.set(false);
    let ctx = unsafe { interop.BeginDraw(None, &mut origin).ok()? };
    let session = DrawingSession::new_borrowed(&ctx, &comp.device_lost);
    session.set_transform(&Matrix3x2 {
        m11: scale,
        m12: 0.0,
        m21: 0.0,
        m22: scale,
        m31: origin.x as f32,
        m32: origin.y as f32,
    });
    session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
    if let ShapeKey::GradBar { r, stops, .. } = &key.shape {
        // Each stop rides the same output colour map as every solid; the
        // FP16 stop collection keeps subtle ramps from posterizing.
        let mapped: Vec<GradientStop> = stops
            .iter()
            .map(|s| GradientStop::new(s.position(), linear(s.color())))
            .collect();
        if let Ok(g) = session.create_linear_gradient(
            CVec2::new(0.0, 0.0),
            CVec2::new(dip_w, 0.0),
            &mapped,
        ) {
            let rect = Rect::from_xywh(0.0, 0.0, dip_w, dip_h);
            let radius = f32::from_bits(*r);
            if radius > 0.0 {
                session.fill_rounded_rect(&RoundedRect::uniform(rect, radius), &g);
            } else {
                session.fill_rect(&rect, &g);
            }
        }
    } else if let Ok(b) = session.create_solid_brush(linear(color)) {
        draw_shape(&session, &b, &key.shape, dip_w, dip_h);
    }
    unsafe { interop.EndDraw() }.ok().ok()?;
    Some(AtlasEntry { brush, _surface: surface, used: 0 })
}

/// A standalone FP16 gradient-bar surface brush (the same display-mapped raster
/// the meter fill uses), for callers outside the `Part` model — the Knob strokes
/// its value arc with this so the arc stays HDR-mapped like all chrome. Not
/// atlas-cached; the caller holds it and rebuilds on an epoch/stops change.
pub(crate) fn build_gradient_surface(
    comp: &Compositing,
    stops: &[(f64, crate::Color)],
    scale: f32,
) -> Option<CompositionSurfaceBrush> {
    // Plain full-bleed stretch (r = 0): the knob strokes an arc SHAPE with this
    // as a Fill surface brush, so it must have no rounded (transparent) ends.
    rasterize(comp, &AtlasKey::grad_bar(stops, 0.0, GRAD_SRC_H, scale)).map(|e| e.brush)
}

/// A standalone FP16 solid-color surface brush (display-mapped), for the Knob's
/// needle. See [`build_gradient_surface`].
pub(crate) fn build_solid_surface(
    comp: &Compositing,
    color: crate::Color,
    scale: f32,
) -> Option<CompositionSurfaceBrush> {
    rasterize(comp, &AtlasKey::solid(color, scale)).map(|e| e.brush)
}


fn draw_shape(session: &DrawingSession, brush: &Brush, shape: &ShapeKey, w: f32, h: f32) {
    match shape {
        ShapeKey::Solid => session.fill_rect(&Rect::from_xywh(0.0, 0.0, w, h), brush),
        ShapeKey::HBar { r, stroke_w, .. } => {
            let radius = f32::from_bits(*r);
            let sw = f32::from_bits(*stroke_w);
            let rect = Rect::from_xywh(0.0, 0.0, w, h);
            if sw <= 0.0 {
                session.fill_rounded_rect(&RoundedRect::uniform(rect, radius), brush);
            } else {
                // Stroke drawn inset by half its width, like `controls::stroke_rr`.
                let inset =
                    Rect::new(sw / 2.0, sw / 2.0, w - sw / 2.0, h - sw / 2.0);
                session.draw_rounded_rect(&RoundedRect::uniform(inset, radius), brush, sw);
            }
        }
        ShapeKey::Circle { d } => {
            let radius = f32::from_bits(*d) / 2.0;
            session.fill_ellipse(
                &Ellipse::new(CVec2::new(radius, radius), radius, radius),
                brush,
            );
        }
        // Rasterized directly in `rasterize` (needs the stop list).
        ShapeKey::GradBar { .. } => {}
        // Stroke coordinates mirror the retired painted checkmark (authored in
        // an 18-DIP box), scaled to `d`.
        ShapeKey::Check { d } => {
            let s = f32::from_bits(*d) / 18.0;
            session.draw_line(
                CVec2::new(4.0 * s, 9.0 * s),
                CVec2::new(7.5 * s, 12.5 * s),
                brush,
                2.0 * s,
            );
            session.draw_line(
                CVec2::new(7.5 * s, 12.5 * s),
                CVec2::new(14.0 * s, 5.5 * s),
                brush,
                2.0 * s,
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Part — one retained sprite with retargetable compositor springs
// ─────────────────────────────────────────────────────────────────────────────

/// The two facts about one animatable property that must never drift apart:
/// the last target we ASKED for, and which animation (if any) may currently own
/// the property.
///
/// They live in a module of their own with PRIVATE fields so that no site can
/// update one without the other. Before, both were plain fields on `Part` and
/// the pairing was a convention — one an out-of-band site (`loop_x`, the slider
/// fill derivation) could satisfy halfway by nulling the cached target and
/// forgetting the flag, leaving `Part::place` unable to reclaim the property it
/// believed it owned. There is now no way to express that halfway state: every
/// transition is one call, named for what actually happened to the property.
mod channel {
    use std::cell::Cell;
    use std::rc::Rc;

    /// The property token an animation was started on (`"Offset"`, `"Offset.X"`,
    /// …) — the exact token a snap has to `StopAnimation`.
    pub(super) type Prop = &'static str;

    /// What this channel actually KNOWS about the property.
    ///
    /// The distinction this enum exists to force is between a value that
    /// LANDED and a value that was merely ASKED for. Both used to be one
    /// `Option<(f32, f32)>`, and a gate that cannot tell them apart will
    /// suppress the very write that would repair a sprite whose animation was
    /// accepted and then never ran.
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Known {
        /// Nothing. The next write must be unconditional.
        Nothing,
        /// A plain write of this value was accepted by the compositor and no
        /// animation stands between it and the visual. The ONLY state that is
        /// evidence of a position.
        Verified((f32, f32)),
        /// An animation this part owns was accepted and is driving `prop`
        /// toward this DESTINATION. It says where the sprite is going, never
        /// where it is.
        ///
        /// `from` is where the motion BEGAN — the last verified point before
        /// the sprite left it — and it survives retargeting. It is the only
        /// honest figure to measure a move against while one is in flight: the
        /// sprite is somewhere between `from` and `to`, so measuring the next
        /// move from `to` would size it against a distance the sprite has not
        /// travelled yet.
        Animating {
            prop: Prop,
            from: (f32, f32),
            to: (f32, f32),
            flight: u32,
        },
        /// An out-of-band animation owns `prop` and this channel does not track
        /// its value at all — the progress sweep, the slider fill derivation.
        Ceded(Prop),
    }

    pub(super) struct Channel {
        known: Known,
        /// Bumped per glide. The completion callback reports the generation it
        /// was armed for, so a stale completion cannot verify a destination
        /// that has since been superseded.
        flight: u32,
        /// The generation the compositor last reported COMPLETE. Written by a
        /// callback that outlives the borrow which armed it, hence the `Rc`.
        settled: Rc<Cell<u32>>,
    }

    impl Channel {
        pub(super) fn new() -> Self {
            Self { known: Known::Nothing, flight: 0, settled: Rc::new(Cell::new(0)) }
        }

        /// Fold in a completion the compositor has reported: an animation that
        /// RAN to its destination has proven the position, so the destination
        /// it was heading to becomes verified.
        ///
        /// This is the whole of why `Animating` is allowed to suppress a
        /// retarget at all — the claim expires by being confirmed.
        fn absorb(&mut self) {
            if let Known::Animating { to, flight, .. } = self.known
                && self.settled.get() == flight
            {
                self.known = Known::Verified(to);
            }
        }

        /// Has this channel ever been written? A first write must snap —
        /// mounting must never fly in from the visual's zeroed defaults.
        pub(super) fn placed(&self) -> bool {
            self.known != Known::Nothing
        }

        /// The value is KNOWN to be `t`. The only state that may suppress a
        /// plain write of the same value.
        pub(super) fn verified_at(&mut self, t: (f32, f32)) -> bool {
            self.absorb();
            matches!(self.known, Known::Verified(v) if v == t)
        }

        /// Whether the CACHE claims a spring is flying to `t`.
        ///
        /// A claim, not a fact — the spring may have died. Its only job is to
        /// decide whether asking the compositor for the truth could change
        /// anything, so the query stays off the common path.
        pub(super) fn claims_flight_to(&mut self, t: (f32, f32)) -> bool {
            self.absorb();
            matches!(self.known, Known::Animating { to, .. } if to == t)
        }

        /// Whether a retarget to `t` must be issued, GIVEN whether the
        /// compositor still has an animation on this property.
        ///
        /// `live` is an OBSERVATION, passed in rather than assumed, which is the
        /// whole point: the policy stays a pure function of state plus one fact,
        /// and the only impure part of the decision is the single query that
        /// answers it. A bound on how long a claim may suppress would have been
        /// approximating this fact with a clock — early for a long flight, late
        /// for a dead one, and arbitrary in both directions.
        ///
        /// Discovering `live == false` under a flight claim also RETIRES the
        /// claim: it has been proven false, so it must not go on suppressing
        /// anything, including a later snap.
        pub(super) fn needs_retarget(&mut self, t: (f32, f32), live: bool) -> bool {
            self.absorb();
            match self.known {
                // Already there, verifiably.
                Known::Verified(v) if v == t => false,
                Known::Animating { to, .. } if to == t => {
                    if live {
                        // Genuinely in flight: re-issuing would restart the
                        // spring from zero velocity and leave it crawling.
                        false
                    } else {
                        self.known = Known::Nothing;
                        true
                    }
                }
                _ => true,
            }
        }

        /// Begin an authoritative snap: yields the property token the caller
        /// MUST stop (if any) and drops every claim. A `Some` result means the
        /// value left behind is unknown, so the caller must write
        /// unconditionally.
        #[must_use]
        pub(super) fn begin_snap(&mut self) -> Option<Prop> {
            let held = match self.known {
                Known::Animating { prop, .. } | Known::Ceded(prop) => Some(prop),
                Known::Nothing | Known::Verified(_) => None,
            };
            if held.is_some() {
                self.known = Known::Nothing;
            }
            held
        }

        /// Record a plain property write of `t`, given the RESULT of the COM
        /// call meant to land it.
        ///
        /// That result is the only evidence this cache may advance on, so it is
        /// a required argument rather than something the caller checks first. A
        /// discarded failure wedges the part forever: the visual stays put while
        /// the cache claims `t` arrived, and every later request for `t` is then
        /// dropped as redundant.
        pub(super) fn wrote(&mut self, t: (f32, f32), write: windows_core::Result<()>) {
            self.known = match write {
                Ok(()) => Known::Verified(t),
                // An unknown value is left behind, so the next write must be
                // unconditional.
                Err(_) => Known::Nothing,
            };
        }

        /// Arm a glide, yielding the generation its completion must report.
        pub(super) fn arming(&mut self) -> u32 {
            self.flight = self.flight.wrapping_add(1);
            self.flight
        }

        /// The cell a completion callback reports its flight number into.
        pub(super) fn settle_cell(&self) -> Rc<Cell<u32>> {
            self.settled.clone()
        }

        /// The point a move should be MEASURED from: where the sprite verifiably
        /// is, or — while it is in flight — where that flight began.
        ///
        /// Never the in-flight destination. The sprite has not arrived there, so
        /// sizing the next move against it under-measures by exactly the
        /// distance still to be covered, and an under-measured move is given a
        /// duration far too short for the ground it has to make up.
        ///
        /// Not evidence of a position, which is why it is separate from
        /// [`verified_at`](Self::verified_at).
        pub(super) fn travel_origin(&self) -> Option<(f32, f32)> {
            match self.known {
                Known::Verified(v) => Some(v),
                Known::Animating { from, .. } => Some(from),
                Known::Nothing | Known::Ceded(_) => None,
            }
        }

        /// A spring this `Part` owns, armed at `flight`, now drives `prop` to `t`.
        ///
        /// A RETARGET keeps the origin of the flight already in progress. The
        /// sprite has not reached the old destination, so the new move is still
        /// the same journey from the same place, and sizing it from the
        /// abandoned destination would pick a duration for a sliver of travel
        /// while the sprite crosses the whole span — which is a jump.
        pub(super) fn animating(&mut self, prop: Prop, t: (f32, f32), flight: u32) {
            let from = match self.known {
                Known::Verified(v) => v,
                Known::Animating { from, .. } => from,
                Known::Nothing | Known::Ceded(_) => t,
            };
            self.known = Known::Animating { prop, from, to: t, flight };
        }

        /// Hand `prop` to an OUT-OF-BAND animation whose value this part does
        /// not track — a forever-looping sweep, an expression derivation. Both
        /// consequences follow from this single call, which is the whole point:
        /// the cached target no longer describes the visual (so it must not
        /// suppress a later write) AND a snap must stop `prop`.
        pub(super) fn ceded(&mut self, prop: Prop) {
            self.known = Known::Ceded(prop);
        }

        /// The caller has ALREADY stopped whatever held the property. Nothing
        /// animates it now, but the value it left behind is unknown, so the next
        /// write must be unconditional.
        pub(super) fn reclaimed(&mut self) {
            self.known = Known::Nothing;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const A: (f32, f32) = (1.0, 2.0);
        const B: (f32, f32) = (30.0, 4.0);

        fn ok() -> windows_core::Result<()> {
            Ok(())
        }
        fn err() -> windows_core::Result<()> {
            Err(windows_core::Error::from_hresult(windows_core::HRESULT(-1)))
        }

        /// THE invariant. A channel may suppress a write of `t` only when it has
        /// evidence for `t` — a write that landed, or an animation confirmed to
        /// have reached it. Every other state must let the write through, because
        /// the write is the only thing that can repair a stranded sprite.
        #[test]
        fn only_evidence_suppresses_a_write() {
            let mut c = Channel::new();
            assert!(!c.verified_at(A), "a fresh channel knows nothing");

            c.wrote(A, ok());
            assert!(c.verified_at(A), "a landed write is evidence");
            assert!(!c.verified_at(B), "and only of the value it landed");

            c.wrote(B, err());
            assert!(!c.verified_at(B), "a FAILED write is never evidence");
            assert!(!c.verified_at(A), "and it invalidates what came before");
        }

        /// An accepted animation is a destination, not a position: it may hold
        /// off a duplicate retarget, but it must never pass as proof the sprite
        /// arrived.
        #[test]
        fn an_accepted_animation_is_not_a_position() {
            let mut c = Channel::new();
            c.wrote(A, ok());
            let flight = c.arming();
            c.animating("Offset", B, flight);

            assert!(c.claims_flight_to(B), "a retarget to the same destination is redundant");
            assert!(!c.verified_at(B), "but the sprite is NOT known to be there");
            assert!(!c.verified_at(A), "nor still where it started");
        }

        /// The claim expires by being confirmed: a completion for the live
        /// generation promotes the destination to evidence.
        #[test]
        fn a_completed_animation_becomes_evidence() {
            let mut c = Channel::new();
            c.wrote(A, ok());
            let flight = c.arming();
            let cell = c.settle_cell();
            c.animating("Offset", B, flight);

            cell.set(flight);
            assert!(c.verified_at(B), "an animation that ran proves the position");
        }

        /// A completion for a superseded flight must not verify anything — the
        /// sprite is on its way somewhere else entirely.
        #[test]
        fn a_stale_completion_verifies_nothing() {
            let mut c = Channel::new();
            c.wrote(A, ok());
            let stale = c.arming();
            let cell = c.settle_cell();
            let live = c.arming();
            c.animating("Offset", B, live);

            cell.set(stale);
            assert!(!c.verified_at(B), "a stale completion is not evidence");
            assert!(c.claims_flight_to(B), "and the live flight is untouched");
        }

        /// A snap drops every claim and reports what it must stop. Crucially an
        /// animating channel yields its property AND stops being evidence, so
        /// the caller writes unconditionally.
        #[test]
        fn a_snap_drops_every_claim() {
            let mut c = Channel::new();
            let flight = c.arming();
            c.animating("Offset", B, flight);
            assert_eq!(c.begin_snap(), Some("Offset"));
            assert!(!c.verified_at(B) && !c.claims_flight_to(B), "nothing survives a snap");

            // A verified channel has nothing to stop, and stays evidence: a
            // redundant snap to the same value really is redundant.
            let mut c = Channel::new();
            c.wrote(A, ok());
            assert_eq!(c.begin_snap(), None);
            assert!(c.verified_at(A));
        }

        /// Ceded and reclaimed both mean "the value here is unknown", so neither
        /// may suppress. Ceded additionally names a property a snap must stop.
        #[test]
        fn ceded_and_reclaimed_are_never_evidence() {
            let mut c = Channel::new();
            c.wrote(A, ok());
            c.ceded("Offset.X");
            assert!(!c.verified_at(A), "a ceded channel tracks nothing");
            assert_eq!(c.begin_snap(), Some("Offset.X"), "but a snap must stop it");

            c.wrote(A, ok());
            c.reclaimed();
            assert!(!c.verified_at(A));
            assert_eq!(c.begin_snap(), None, "already stopped by the caller");
        }

        /// A LIVE flight to the same destination suppresses the retarget —
        /// re-issuing would restart the spring from zero velocity.
        #[test]
        fn a_live_flight_suppresses_a_duplicate_retarget() {
            let mut c = Channel::new();
            c.wrote(A, ok());
            let f = c.arming();
            c.animating("Offset", B, f);

            assert!(!c.needs_retarget(B, true), "still flying there; leave it alone");
            assert!(c.needs_retarget(A, true), "a DIFFERENT destination always retargets");
        }

        /// THE case a bound would only have approximated. The claim says a
        /// spring is flying to `B`; the compositor says nothing animates the
        /// property. The claim is false, so the retarget must be issued — now,
        /// not after some number of syncs — and the false claim must be retired
        /// so it cannot suppress a later snap either.
        #[test]
        fn a_dead_flight_is_retired_the_moment_it_is_observed_dead() {
            let mut c = Channel::new();
            c.wrote(A, ok());
            let f = c.arming();
            c.animating("Offset", B, f);

            assert!(c.needs_retarget(B, false), "a dead spring must not suppress");
            assert!(!c.verified_at(B), "and it never became evidence");
            assert!(
                !c.claims_flight_to(B),
                "the disproven claim is retired, so it cannot suppress a snap either",
            );
        }

        /// The observation is only consulted where it can change the answer:
        /// a channel with no flight claim to `t` retargets regardless of it.
        #[test]
        fn liveness_is_irrelevant_without_a_matching_claim() {
            let mut c = Channel::new();
            c.wrote(A, ok());
            assert!(c.needs_retarget(B, true));
            assert!(c.needs_retarget(B, false));
            assert!(!c.claims_flight_to(B), "nothing to ask the compositor about");
        }

        /// A completed flight outranks liveness: the animation is gone precisely
        /// BECAUSE it arrived, and that must read as evidence, not as death.
        #[test]
        fn a_completed_flight_is_evidence_not_a_dead_one() {
            let mut c = Channel::new();
            c.wrote(A, ok());
            let f = c.arming();
            let cell = c.settle_cell();
            c.animating("Offset", B, f);
            cell.set(f);

            assert!(!c.needs_retarget(B, false), "it arrived; do not move it again");
            assert!(c.verified_at(B));
        }

        /// Clicking again while the sprite is still travelling must not resize
        /// the move down to the sliver between the abandoned destination and the
        /// new one — the sprite is still back near where it started, and a
        /// duration picked for a sliver makes it cross the whole span at once.
        /// That is the second-click jump.
        #[test]
        fn a_mid_flight_retarget_measures_from_where_the_motion_began() {
            let mut c = Channel::new();
            c.wrote(A, ok());

            let f1 = c.arming();
            c.animating("Offset", B, f1);
            assert_eq!(c.travel_origin(), Some(A), "the flight began at A");

            // Retarget mid-flight to a point just past B.
            let past_b = (B.0 + 1.0, B.1);
            let f2 = c.arming();
            c.animating("Offset", past_b, f2);
            assert_eq!(
                c.travel_origin(),
                Some(A),
                "still the same journey from A — NOT the abandoned destination B",
            );

            // Only arrival re-bases it.
            let cell = c.settle_cell();
            cell.set(f2);
            assert!(c.verified_at(past_b));
            assert_eq!(c.travel_origin(), Some(past_b), "arrival re-bases the origin");
        }

        /// `placed` gates the first-write snap, so it must be true for every
        /// state that has touched the property and false only at birth.
        #[test]
        fn placed_is_false_only_at_birth() {
            let mut c = Channel::new();
            assert!(!c.placed());
            c.wrote(A, ok());
            assert!(c.placed());
            c.reclaimed();
            assert!(!c.placed(), "a reclaimed channel must snap again");
        }
    }
}
use channel::Channel;

/// One chrome-part sprite. All mutation is change-gated against the last
/// written *target* so an unchanged sync costs nothing, and all motion is a
/// retarget of a cached compositor spring — zero allocation per event.
pub(crate) struct Part {
    sprite: SpriteVisual,
    vis: IVisual,
    obj: ICompositionObject,
    /// Nine-grid wrapper (HBar sources only); built once, re-sourced on re-bind.
    nine: Option<CompositionNineGridBrush>,
    /// The atlas source currently bound + the epoch it came from.
    key: Option<AtlasKey>,
    epoch: u32,
    /// Offset / Size: last requested target + who owns the property.
    off: Channel,
    size: Channel,
    opacity: Option<f32>,
    /// Whether a fade may currently hold Opacity (snap must stop it). Opacity
    /// has no out-of-band writer, so it stays a plain flag.
    op_gliding: bool,
    // Cached retargetable motion springs, built on first glide.
    s_off: Option<SpringVector3NaturalMotionAnimation>,
    s_size: Option<SpringVector2NaturalMotionAnimation>,
    /// Keep the settle subscriptions alive: an `EventRevoker` revokes its
    /// handler on drop, so a dropped one is a completion that never arrives —
    /// and a destination that never becomes evidence.
    _settle: [Option<windows_core::EventRevoker>; 2],
}

impl Part {
    fn new(comp: &Compositing) -> Option<Self> {
        let sprite = comp.new_sprite().ok()?;
        let vis: IVisual = sprite.cast().ok()?;
        let obj: ICompositionObject = sprite.cast().ok()?;
        Some(Self {
            sprite,
            vis,
            obj,
            nine: None,
            key: None,
            epoch: 0,
            off: Channel::new(),
            size: Channel::new(),
            opacity: None,
            op_gliding: false,
            s_off: None,
            s_size: None,
            _settle: [None, None],
        })
    }

    /// Scope `start` in a batch and report its completion back into `settled` as
    /// `flight`, so an animation that RAN can promote its destination to evidence.
    ///
    /// Without this, `Channel::animating` would be a claim nothing could ever
    /// confirm, and a spring that was accepted but never ran would suppress the
    /// retarget that would have repaired it — forever, for that destination.
    /// Whether the compositor still has an animation driving `prop`.
    ///
    /// The authoritative answer to "is my claim still true", asked instead of
    /// guessed: `TryGetAnimationController` fails once nothing animates the
    /// property. A timeout would only ever have been approximating this — never
    /// exactly at the moment the flight ended, and arbitrary in how far off it
    /// was — whereas this is neither early nor late.
    fn animation_live(&self, prop: &str) -> bool {
        self.obj
            .cast::<ICompositionObject4>()
            .and_then(|o| o.TryGetAnimationController(prop))
            .is_ok()
    }

    fn open_batch(&self) -> Option<CompositionScopedBatch> {
        self.obj
            .Compositor()
            .ok()
            .and_then(|c| c.CreateScopedBatch(CompositionBatchTypes::Animation).ok())
    }

    /// Close the batch opened around a glide, reporting its completion back as
    /// `flight`. Associated rather than a method so it does not contend with the
    /// `&mut self` the glide itself needs.
    fn close_batch(
        batch: Option<CompositionScopedBatch>,
        settled: std::rc::Rc<std::cell::Cell<u32>>,
        flight: u32,
    ) -> Option<windows_core::EventRevoker> {
        batch.and_then(|b| {
            b.Completed(move |_, _| {
                settled.set(flight);
            })
            .ok()
            .filter(|_| b.End().is_ok())
        })
    }

    /// The sprite as a plain `Visual` (for tree insertion / re-sync).
    pub(crate) fn visual(&self) -> Option<Visual> {
        self.sprite.cast().ok()
    }

    /// Bind (or re-bind) this part's brush to the atlas source for `key`.
    /// No-op while the key and atlas epoch are unchanged. A gradient bar's key
    /// carries its own stop list, so there is nothing to supply alongside it.
    fn bind(&mut self, comp: &Compositing, atlas: &mut Atlas, key: AtlasKey) {
        if self.key.as_ref() == Some(&key) && self.epoch == atlas.epoch {
            return;
        }
        let epoch = atlas.epoch;
        let Some(entry) = atlas.entry(comp, &key) else { return };
        let brush: Option<CompositionBrush> = if key.uses_nine_grid() {
            // Corners map 1:1 back to DIPs: source insets are `r * scale` px,
            // scaled down by `1 / scale` on the destination.
            let nine = match &self.nine {
                Some(n) => n.clone(),
                None => match comp.new_nine_grid() {
                    Ok(n) => {
                        self.nine = Some(n.clone());
                        n
                    }
                    Err(_) => return,
                },
            };
            let inset = key.inset_px();
            let scale = f32::from_bits(key.scale).max(0.01);
            let ok = nine.SetInsetsWithValues(inset, 0.0, inset, 0.0).is_ok()
                && nine.SetInsetScales(1.0 / scale).is_ok()
                && entry
                    .brush
                    .cast::<CompositionBrush>()
                    .and_then(|src| nine.SetSource(&src))
                    .is_ok();
            ok.then(|| nine.cast().ok()).flatten()
        } else {
            entry.brush.cast().ok()
        };
        if let Some(b) = brush
            && self.sprite.SetBrush(&b).is_ok()
        {
            self.key = Some(key);
            self.epoch = epoch;
        }
    }

    /// Snap position + size (stopping any in-flight glide first).
    ///
    /// A snap is AUTHORITATIVE: when a spring may still hold the property it is
    /// stopped and the value rewritten even if the target equals what we last
    /// asked for. `off`/`size` record the last target *requested*, not where the
    /// visual actually is, so while a glide is in flight they are not evidence
    /// of anything. Skipping on a matching target alone would leave that glide
    /// owning the property while the caller believes it snapped — interrupt it
    /// and the visual strands, and since the cache still reads "already at T"
    /// every later request for T is dropped as redundant, wedging the part
    /// until some different target happens to arrive.
    fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let off_held = self.off.begin_snap();
        if let Some(prop) = off_held {
            let _ = self.obj.StopAnimation(prop);
        }
        if off_held.is_some() || !self.off.verified_at((x, y)) {
            let wrote = self.vis.SetOffset(Vector3::new(x, y, 0.0));
            self.off.wrote((x, y), wrote);
        }
        let size_held = self.size.begin_snap();
        if let Some(prop) = size_held {
            let _ = self.obj.StopAnimation(prop);
        }
        if size_held.is_some() || !self.size.verified_at((w, h)) {
            let wrote = self.vis.SetSize(Vector2::new(w, h));
            self.size.wrote((w, h), wrote);
        }
    }

    /// Spring-glide position + size to a new target. First placement snaps
    /// (mounting must never fly in from the visual's zeroed defaults).
    fn glide(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if !self.off.placed() || !self.size.placed() {
            self.place(x, y, w, h);
            return;
        }
        // Suppress only on EVIDENCE (already there) or on a live flight to the
        // same destination (re-issuing would reset the spring's velocity and
        // leave the sprite crawling). A destination merely claimed by an
        // animation that never ran is neither, so it lets the retarget through
        // — which is what repairs it.
        // The compositor is asked whether the claimed flight is still real, and
        // only when that answer can change the decision — everywhere else the
        // retarget is unconditional and the query never happens.
        let live = self.off.claims_flight_to((x, y)) && self.animation_live("Offset");
        if self.off.needs_retarget((x, y), live) {
            // Measured from where the motion BEGAN, not from the destination
            // it is still on its way to — see `travel_origin`.
            let dist = self.off.travel_origin().map_or(0.0, |(px, py)| (x - px).hypot(y - py));
            let flight = self.off.arming();
            let cell = self.off.settle_cell();
            let batch = self.open_batch();
            let started = self.glide_offset(x, y, dist);
            let rev = Self::close_batch(batch, cell, flight);
            if started.is_some() {
                self.off.animating("Offset", (x, y), flight);
                self._settle[0] = rev;
            } else {
                self.place(x, y, w, h);
                return;
            }
        }
        let live = self.size.claims_flight_to((w, h)) && self.animation_live("Size");
        if self.size.needs_retarget((w, h), live) {
            let dist = self.size.travel_origin().map_or(0.0, |(pw, ph)| (w - pw).hypot(h - ph));
            let flight = self.size.arming();
            let cell = self.size.settle_cell();
            let batch = self.open_batch();
            let started = self.glide_size(w, h, dist);
            let rev = Self::close_batch(batch, cell, flight);
            if started.is_some() {
                self.size.animating("Size", (w, h), flight);
                self._settle[1] = rev;
            } else {
                self.place(x, y, w, h);
            }
        }
    }

    fn glide_offset(&mut self, x: f32, y: f32, dist: f32) -> Option<()> {
        if self.s_off.is_none() {
            let c = self.obj.Compositor().ok()?;
            let a = c.cast::<ICompositor4>().ok()?.CreateSpringVector3Animation().ok()?;
            let sa: ISpringVector3NaturalMotionAnimation = a.cast().ok()?;
            sa.SetDampingRatio(CHROME_SPRING_DAMPING).ok()?;
            self.s_off = Some(a);
        }
        // Re-tuned per retarget, not once at construction: the period is a
        // function of THIS move's distance.
        let a = self.s_off.as_ref()?;
        a.cast::<ISpringVector3NaturalMotionAnimation>()
            .ok()?
            .SetPeriod(ts_secs(spring_period(dist)))
            .ok()?;
        a.cast::<IVector3NaturalMotionAnimation>()
            .ok()?
            .SetFinalValue(Some(Vector3::new(x, y, 0.0)))
            .ok()?;
        self.obj
            .StartAnimation("Offset", &a.cast::<CompositionAnimation>().ok()?)
            .ok()
    }

    fn glide_size(&mut self, w: f32, h: f32, dist: f32) -> Option<()> {
        if self.s_size.is_none() {
            let c = self.obj.Compositor().ok()?;
            let a = c.cast::<ICompositor4>().ok()?.CreateSpringVector2Animation().ok()?;
            let sa: ISpringVector2NaturalMotionAnimation = a.cast().ok()?;
            sa.SetDampingRatio(CHROME_SPRING_DAMPING).ok()?;
            self.s_size = Some(a);
        }
        let a = self.s_size.as_ref()?;
        a.cast::<ISpringVector2NaturalMotionAnimation>()
            .ok()?
            .SetPeriod(ts_secs(spring_period(dist)))
            .ok()?;
        a.cast::<IVector2NaturalMotionAnimation>()
            .ok()?
            .SetFinalValue(Some(Vector2::new(w, h)))
            .ok()?;
        self.obj
            .StartAnimation("Size", &a.cast::<CompositionAnimation>().ok()?)
            .ok()
    }

    /// Snap opacity (stopping any in-flight fade first).
    fn set_opacity(&mut self, a: f32) {
        if self.opacity == Some(a) {
            return;
        }
        if self.op_gliding {
            let _ = self.obj.StopAnimation("Opacity");
            self.op_gliding = false;
        }
        // `Channel::wrote`'s evidence rule, for the one property that is not a
        // `Channel`: caching `a` over a failed write would suppress every later
        // attempt to reach `a`, stranding the part at whatever it is showing.
        self.opacity = self.vis.SetOpacity(a).is_ok().then_some(a);
    }

    /// Fade opacity to a target — a compositor keyframe glide (the mechanism
    /// the scroll-thumb reveal already proves out; the scalar natural-motion
    /// spring runs far slower than its tuning promises, so it is not used for
    /// opacity). Quick in, gentler out, retargeting smoothly mid-flight.
    /// First write snaps.
    fn fade_to(&mut self, a: f32) {
        if self.opacity == Some(a) {
            return;
        }
        let Some(prev) = self.opacity else {
            self.set_opacity(a);
            return;
        };
        let run = || -> Option<()> {
            let comp = self.obj.Compositor().ok()?;
            let v = self.sprite.cast::<Visual>().ok()?;
            let ms = if a > prev { FADE_IN_MS } else { FADE_OUT_MS };
            super::animate::fade_opacity(
                &comp,
                &v,
                a,
                std::time::Duration::from_millis(ms),
                crate::style::Easing::EaseOut,
            );
            Some(())
        };
        if run().is_some() {
            self.opacity = Some(a);
            self.op_gliding = true;
        } else {
            self.set_opacity(a);
        }
    }

    /// Start a FOREVER-looping constant-velocity sweep of this sprite's
    /// `Offset.X` from `from` to `to` over `secs` — the indeterminate-progress
    /// travel, playing entirely on the compositor (the app never ticks it).
    /// Set the resting offset (`place`) BEFORE starting: the loop owns only the
    /// X subchannel; Y stays where it was placed.
    fn loop_x(&mut self, from: f32, to: f32, secs: f32) -> bool {
        let run = || -> Option<()> {
            let comp = self.obj.Compositor().ok()?;
            let lin: CompositionEasingFunction =
                comp.CreateLinearEasingFunction().ok()?.cast().ok()?;
            let a = comp.CreateScalarKeyFrameAnimation().ok()?;
            a.InsertKeyFrameWithEasingFunction(0.0, from, &lin).ok()?;
            a.InsertKeyFrameWithEasingFunction(1.0, to, &lin).ok()?;
            let kf: IKeyFrameAnimation = a.cast().ok()?;
            kf.SetDuration(ts_secs(secs)).ok()?;
            kf.SetIterationBehavior(AnimationIterationBehavior::Forever).ok()?;
            let _ = self.obj.StopAnimation("Offset.X");
            self.obj
                .StartAnimation("Offset.X", &a.cast::<CompositionAnimation>().ok()?)
                .ok()
        };
        // The loop owns Offset.X from here. Ceding says BOTH halves at once:
        // the cached target no longer describes the visual, and a later
        // `place` must stop `Offset.X` before it can reclaim the property.
        self.off.ceded("Offset.X");
        run().is_some()
    }

    /// Stop the looping sweep (back to determinate); the next `place`
    /// re-anchors the offset.
    fn stop_loop_x(&mut self) {
        let _ = self.obj.StopAnimation("Offset.X");
        // Stopped here, so nothing animates Offset — but the loop left the
        // visual at an unknown X, so the next write must be unconditional.
        self.off.reclaimed();
    }

    /// Every cached fact this part holds describes a compositor state that no
    /// longer exists. Drop them so the next sync writes unconditionally.
    ///
    /// Reclaiming the channels is also what makes that sync SNAP: `glide`
    /// refuses to spring from an unplaced channel, and motion out of an unknown
    /// position is not motion anyone asked for.
    fn invalidate(&mut self) {
        self.off.reclaimed();
        self.size.reclaimed();
        self.opacity = None;
        self.op_gliding = false;
    }
}

/// Ink/halo fade durations (ms): a quick reveal, a slightly gentler conceal —
/// the perceptual speed of the retired `(520, 40)` CPU hover spring.
const FADE_IN_MS: u64 = 120;
const FADE_OUT_MS: u64 = 220;

// ─────────────────────────────────────────────────────────────────────────────
// Parts — a node's part set + last-synced logical state
// ─────────────────────────────────────────────────────────────────────────────

/// A converted control's retained parts and the logical state they were last
/// reconciled against. Boxed on the node; only converted kinds allocate one.
pub(crate) struct Parts {
    /// Sprites under the node's painted surface (tray / pill / indicator).
    below: Vec<Part>,
    /// Sprites over it (ink wash, slider fill / halo / thumb).
    above: Vec<Part>,
    /// First sync completed — until then every write snaps.
    init: bool,
    /// Last node size; a change snaps (resize must not glide).
    geom: (f32, f32),
    /// The layout signature the parts were last applied against
    /// ([`PartPlan::layout_sig`]) — plan-driven kinds use this in place of the
    /// hand-maintained `sel` / `geom` / `edges_sig` shadows.
    layout_sig: [f32; 3],
    /// Progress: a forever-looping compositor animation is running (the
    /// indeterminate bar sweep / ring spin).
    looping: bool,
    /// Meter: the fill sprite's reveal clip (its `RightInset` follows the
    /// needle via an `ExpressionAnimation`).
    ///
    clip: Option<InsetClip>,
    /// Track width the reveal expression was last built for.
    clip_w: f32,
    /// Meter: the gradient-bar atlas key last built. Reused while the ramp,
    /// bar height and scale are unchanged, so a steady repaint clones an `Rc`
    /// instead of allocating a fresh stop list (see [`grad_bar_key`]).
    grad_key: Option<AtlasKey>,
    /// Slider: whether expressions currently DERIVE the fill'''s rect from the
    /// thumb'''s offset.
    ///
    /// The fill'''s left edge and width cannot simply be sprung alongside the
    /// thumb: the fill spans `origin -> thumb`, so both are `Min`/`Abs` of the
    /// thumb position -- affine only within one side of the origin. Springing
    /// them in parallel across a sign change leaves the left edge still short of
    /// the origin while the right is already past it, so the bar straddles 0 as
    /// a wide band instead of collapsing to nothing as the thumb crosses.
    /// Deriving both from the one thumb spring reproduces the kink exactly.
    ///
    /// Expressions evaluate every composition frame, so they are armed only for
    /// the duration of a glide and torn down to plain values on settle -- at
    /// rest the fill is a plain rect and the compositor has nothing to evaluate.
    ///
    /// Shared with the settle callback, which clears it when it tears the
    /// expressions down -- otherwise this side would still believe it was
    /// following and would skip re-arming, freezing the fill.
    fill_live: std::rc::Rc<std::cell::Cell<bool>>,
    /// Bumped per glide; a settle callback only applies if it still matches, so
    /// a stale completion cannot tear down a newer flight.
    fill_gen: std::rc::Rc<std::cell::Cell<u32>>,
    /// Keeps the settle subscription alive.
    _fill_settle: Option<windows_core::EventRevoker>,
}

impl Parts {
    fn new() -> Self {
        Self {
            below: Vec::new(),
            above: Vec::new(),
            init: false,
            geom: (0.0, 0.0),
            layout_sig: [0.0; 3],
            looping: false,
            clip: None,
            clip_w: 0.0,
            grad_key: None,
            fill_live: std::rc::Rc::new(std::cell::Cell::new(false)),
            fill_gen: std::rc::Rc::new(std::cell::Cell::new(0)),
            _fill_settle: None,
        }
    }

    /// THE reclaim authority: drop every cached compositor fact this node's
    /// parts hold, plus the logical state reconciled against it.
    ///
    /// Called for any event that breaks the correspondence between what these
    /// caches claim and what the compositor actually holds — device loss above
    /// all. Without it a part survives such an edge still asserting a position
    /// its sprite never reached, and self-gates every later write to that
    /// position away, which strands the sprite for the rest of the window's
    /// life.
    pub(crate) fn invalidate(&mut self) {
        for p in self.below.iter_mut().chain(self.above.iter_mut()) {
            p.invalidate();
        }
        self.init = false;
        // Both are claims that a compositor animation / object is live: the
        // indeterminate sweep, and the meter's reveal clip. Neither survives.
        self.looping = false;
        self.clip = None;
        self.clip_w = 0.0;
        // The slider's fill derivation, retired the way the machinery itself
        // retires it. `fill_live` left standing is the failure its own doc
        // names — this side would believe it was still following and skip
        // re-arming, freezing the fill — and bumping the generation is how
        // `slider_fill_static` invalidates an in-flight settle, so a completion
        // for the dead batch can no longer match and write a stale rect.
        self.fill_live.set(false);
        self.fill_gen.set(self.fill_gen.get().wrapping_add(1));
        self._fill_settle = None;
    }

    pub(crate) fn below_visuals(&self) -> impl Iterator<Item = Visual> + '_ {
        self.below.iter().filter_map(Part::visual)
    }
    pub(crate) fn above_visuals(&self) -> impl Iterator<Item = Visual> + '_ {
        self.above.iter().filter_map(Part::visual)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PartPlan — where the parts belong, decided without touching the compositor
// ─────────────────────────────────────────────────────────────────────────────

/// A part rect in node-local DIPs: `(x, y, w, h)`.
pub(crate) type Rect4 = (f32, f32, f32, f32);

/// The widest band any plan-driven control uses.
const MAX_SLOTS: usize = 6;

/// How a slot travels when its rect moves.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Motion {
    /// Jump — chrome that must never be seen in transit.
    Snap,
    /// Spring to the new rect: an indicator following the selection.
    Glide,
}

/// How a slot's opacity changes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Fade {
    Snap,
    Fade,
}

/// One retained part's whole intent for this sync.
#[derive(Clone)]
pub(crate) struct SlotPlan {
    /// `None` binds no source — the variant has nothing to draw in this slot.
    pub(crate) key: Option<AtlasKey>,
    /// `None` leaves the geometry untouched (the part is being hidden, and
    /// moving a sprite nobody can see only risks it being seen moving).
    pub(crate) rect: Option<Rect4>,
    pub(crate) opacity: f32,
    pub(crate) motion: Motion,
    pub(crate) fade: Fade,
}

impl SlotPlan {
    /// A part that JUMPS to `rect`.
    pub(crate) fn snap(key: AtlasKey, rect: Option<Rect4>, opacity: f32) -> Self {
        Self { key: Some(key), rect, opacity, motion: Motion::Snap, fade: Fade::Snap }
    }

    /// A part that SPRINGS to `rect` when it moves.
    pub(crate) fn glide(key: AtlasKey, rect: Option<Rect4>, opacity: f32) -> Self {
        Self { key: Some(key), rect, opacity, motion: Motion::Glide, fade: Fade::Snap }
    }

    /// Nothing to draw here: no source, nothing placed, invisible.
    ///
    /// The sprite stays ALLOCATED rather than being freed — the variant can flip
    /// back, and `Parts` has no per-part free — so this is the difference between
    /// a slot the plan hides and a slot the plan simply does not mention, which
    /// `apply` leaves entirely alone.
    pub(crate) fn hidden() -> Self {
        Self { key: None, rect: None, opacity: 0.0, motion: Motion::Snap, fade: Fade::Snap }
    }

    /// Opacity changes fade rather than jump (hover inks).
    pub(crate) fn fading(mut self) -> Self {
        self.fade = Fade::Fade;
        self
    }
}

/// A [`Rect`] as the flat tuple a [`SlotPlan`] places by.
fn r4(r: Rect) -> Rect4 {
    (r.left, r.top, r.width(), r.height())
}

impl AtlasKey {
    /// This shape, JUMPING to `rect`.
    ///
    /// The shape leads because it is what varies: a plan picks a bar, a circle
    /// or a check and then says where it goes. Reading it in that order puts the
    /// placement — the part a test asserts on — at the end of one line instead
    /// of three lines below five source arguments.
    fn snap_at(self, rect: Option<Rect4>, opacity: f32) -> SlotPlan {
        SlotPlan::snap(self, rect, opacity)
    }

    /// This shape, SPRINGING to `rect` when it moves.
    fn glide_at(self, rect: Option<Rect4>, opacity: f32) -> SlotPlan {
        SlotPlan::glide(self, rect, opacity)
    }
}

/// Where every part of one control belongs this sync, and how it should get
/// there — computed from node state alone.
///
/// This is the placement DECISION, separated from the compositor writes that
/// carry it out ([`apply`]). It touches no COM, so it is a pure function of the
/// node and can be asserted directly in a test; a decision welded to a
/// `SetOffset` cannot be tested at all, which is why this file's placement bugs
/// have historically only ever surfaced as something a user noticed on screen.
#[derive(Clone)]
pub(crate) struct PartPlan {
    /// The layout inputs whose change means the control RE-LAID OUT, so every
    /// part must jump rather than spring from a rect that no longer means
    /// anything.
    ///
    /// One field with one meaning, replacing a per-kind mix of last-size,
    /// last-selection and edge-checksum shadows — each maintained by hand, and
    /// each therefore committable on a path that never placed the sprite it
    /// claimed to describe. Unused lanes stay `0.0`.
    pub(crate) layout_sig: [f32; 3],
    below: [Option<SlotPlan>; MAX_SLOTS],
    above: [Option<SlotPlan>; MAX_SLOTS],
}

impl PartPlan {
    pub(crate) fn new(layout_sig: [f32; 3]) -> Self {
        Self {
            layout_sig,
            below: std::array::from_fn(|_| None),
            above: std::array::from_fn(|_| None),
        }
    }

    pub(crate) fn below(mut self, i: usize, slot: SlotPlan) -> Self {
        self.below[i] = Some(slot);
        self
    }

    pub(crate) fn above(mut self, i: usize, slot: SlotPlan) -> Self {
        self.above[i] = Some(slot);
        self
    }

    /// The focus ring's two rungs, at `base` (inner) and `base + 1` (outer).
    ///
    /// One call because the two are one thing: a ring is a light stroke inside a
    /// dark one, and a plan that placed only one rung would render a ring that
    /// reads correctly against exactly one backdrop.
    fn focus_ring(self, base: usize, focused: bool, w: f32, h: f32, r: f32, scale: f32) -> Self {
        let [inner, outer] = focus_ring_slots(focused, w, h, r, scale);
        self.above(base, inner).above(base + 1, outer)
    }

    /// The ring a self-sized control takes: the whole node, at its own radius.
    ///
    /// The four kinds that spell it this way are the four whose focus target IS
    /// the control. Expander and Hyperlink pass their own box instead — an
    /// expander rings its header strip, not the expanded body under it.
    fn node_focus_ring(self, base: usize, node: &Node, scale: f32) -> Self {
        let r = super::controls::focus_radius(node);
        self.focus_ring(base, node.focus_ring, node.rect.w, node.rect.h, r, scale)
    }

    /// The slot plans, for tests asserting where a control decided its chrome
    /// belongs.
    pub(crate) fn slots(&self) -> (&[Option<SlotPlan>], &[Option<SlotPlan>]) {
        (&self.below, &self.above)
    }
}

/// Carry out a [`PartPlan`] — the one place a plan meets the compositor.
///
/// Returns whether this was a RE-LAYOUT (or the first sync), which is the one
/// fact a caller with its own bespoke motion needs — the indeterminate progress
/// sweep re-arms on it.
fn apply(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, plan: &PartPlan) -> bool {
    let Some(parts) = node.parts.as_mut() else { return false };
    // A re-layout retires every cached position as a MOTION reference: springing
    // from a rect the control no longer has reads as a glitch rather than as
    // motion. The first sync snaps for the same reason — mounting must not fly
    // in from the visual's zeroed defaults.
    let relaid = !parts.init || parts.layout_sig != plan.layout_sig;
    let init = parts.init;
    apply_band(comp, atlas, &mut parts.below, &plan.below, relaid, init);
    apply_band(comp, atlas, &mut parts.above, &plan.above, relaid, init);
    parts.layout_sig = plan.layout_sig;
    parts.init = true;
    relaid
}

/// Apply one band's slots.
///
/// Nothing here gates on "did the plan change": `Part`'s own channels already
/// know whether a target moved, and they are the only cache that knows whether
/// the last write actually LANDED. A second gate at this level would suppress
/// exactly the re-write a failed one needs.
fn apply_band(
    comp: &Compositing,
    atlas: &mut Atlas,
    band: &mut [Part],
    slots: &[Option<SlotPlan>; MAX_SLOTS],
    relaid: bool,
    init: bool,
) {
    for (i, slot) in slots.iter().enumerate() {
        let (Some(slot), Some(part)) = (slot.as_ref(), band.get_mut(i)) else {
            continue;
        };
        if let Some(k) = &slot.key {
            part.bind(comp, atlas, k.clone());
        }
        if let Some(r) = slot.rect {
            if relaid || slot.motion == Motion::Snap {
                part.place(r.0, r.1, r.2, r.3);
            } else {
                part.glide(r.0, r.1, r.2, r.3);
            }
        }
        if init && slot.fade == Fade::Fade {
            part.fade_to(slot.opacity);
        } else {
            part.set_opacity(slot.opacity);
        }
    }
}

/// Kinds whose dynamic chrome is fully part-driven (their springs never enter
/// the frame tick; hover / press / activation retarget compositor springs or
/// repaint once, event-driven). The HyperlinkButton has no parts at all — it
/// is listed so its hover recolor stays a single repaint instead of a tick.
/// One plan-driven kind's retained chrome: how many parts it wants in each
/// band, and the plan that fills them.
///
/// Declared rather than written out per kind because the same three facts used
/// to be restated in three places — [`converted`], the [`sync`] dispatch, and a
/// seven-line wrapper per kind — with nothing making them agree. A kind whose
/// plan grew a slot but whose wrapper kept the old count silently lost it:
/// `ensure` mints the band once, on first sync, and `apply_band` skips whatever
/// the band is too short to hold.
struct Look {
    below: usize,
    above: usize,
    plan: fn(&Node, f32) -> PartPlan,
}

/// The chrome a kind declares, or `None` if it is not plan-driven.
fn look(kind: ControlKind) -> Option<Look> {
    use ControlKind as K;
    let (below, above, plan): (usize, usize, fn(&Node, f32) -> PartPlan) = match kind {
        K::Button | K::ToggleButton | K::RepeatButton | K::SplitButton => {
            (slot::N_BELOW, slot::N_ABOVE, button_plan)
        }
        K::HyperlinkButton => (0, 2, hyperlink_plan),
        // The select triggers paint their own fill + border (`paint_select`), so
        // they take the ink wash alone — a second retained fill under a painted
        // one would double the chrome.
        K::ComboBox | K::DropDownButton => (0, 1, ink_plan),
        K::ToggleSwitch => (3, 2, toggle_plan),
        K::CheckBox => (2, 3, check_plan),
        K::SelectorBar => (4, 2, segmented_plan),
        K::NavigationView => (nav_slot::N_BELOW, nav_slot::N_ABOVE, nav_plan),
        K::Expander => (2, 3, expander_plan),
        K::ProgressBar => (3, 0, progress_plan),
        K::InfoBadge => (1, 0, badge_plan),
        K::InfoBar => (3, 1, bar_plan),
        K::TitleBar => (0, 1, caption_plan),
        _ => return None,
    };
    Some(Look { below, above, plan })
}

/// The three converted kinds that own no [`PartPlan`].
///
/// Each is bound to something a plan cannot describe — an `ExpressionAnimation`
/// following a sibling sprite (the slider's fill, the meter's clip) or a forever
/// keyframe on the surface sprite (the ring) — so each keeps a hand-written
/// sync. A plan says where a sprite comes to REST, and none of these three does.
fn plan_less(kind: ControlKind) -> bool {
    matches!(
        kind,
        ControlKind::Slider | ControlKind::Meter | ControlKind::ProgressRing
    )
}

pub(crate) fn converted(kind: ControlKind) -> bool {
    look(kind).is_some() || plan_less(kind)
}

/// Ensure `node.parts` exists with `n_below`/`n_above` parts, inserted at the
/// correct band positions around the painted surface sprite.
fn ensure(comp: &Compositing, node: &mut Node, n_below: usize, n_above: usize) -> bool {
    if node.parts.is_some() {
        return true;
    }
    let Ok(children) = node.container.Children() else { return false };
    // Absent for a node that draws nothing (the button family). The `below`
    // group then stacks at the top in creation order instead of under the
    // surface — which lands it in exactly the same relative z-order, because
    // "below" only ever meant "below the surface", and there is none.
    let surf_vis = node
        .surf
        .as_ref()
        .and_then(|s| s.sprite.cast::<Visual>().ok());

    let mut parts = Box::new(Parts::new());
    // Creation order = bottom→top within the band: each `InsertBelow(surface)`
    // lands directly under the surface, pushing earlier parts further down.
    for _ in 0..n_below {
        let Some(p) = Part::new(comp) else { return false };
        let Some(v) = p.visual() else { return false };
        let placed = match surf_vis.as_ref() {
            Some(sv) => children.InsertBelow(&v, sv),
            None => children.InsertAtTop(&v),
        };
        if placed.is_err() {
            return false;
        }
        parts.below.push(p);
    }
    for _ in 0..n_above {
        let Some(p) = Part::new(comp) else { return false };
        let Some(v) = p.visual() else { return false };
        if children.InsertAtTop(&v).is_err() {
            return false;
        }
        parts.above.push(p);
    }
    node.parts = Some(parts);
    true
}

/// The ink/halo target opacity for the *converted* alpha of an authored wash
/// (endpoint-exact with the retired painted `theme::w(wash)`).
fn wash(authored: f32) -> f32 {
    theme::wash_alpha(authored)
}

/// The uniform disabled dim, as the paint path applies it.
fn dim_of(node: &Node) -> f32 {
    if node.paint.is_enabled {
        1.0
    } else {
        theme::disabled_opacity()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-kind sync (the single writer, run from the paint pass on a dirty node)
// ─────────────────────────────────────────────────────────────────────────────

/// Reconcile a converted node's parts against its logical state. Called from
/// the paint pass after the node's surface exists (dirty nodes only — every
/// state change that matters marks the node dirty; pure hover/drag updates go
/// through the direct event entry points below instead).
pub(crate) fn sync(
    comp: &Compositing,
    atlas: &mut Atlas,
    node: &mut Node,
    scale: f32,
    scrubbing: bool,
) {
    if let Some(l) = look(node.kind) {
        if !ensure(comp, node, l.below, l.above) {
            return;
        }
        let plan = (l.plan)(node, scale);
        let relaid = apply(comp, atlas, node, &plan);
        // The one plan-driven kind with motion its plan cannot hold: an
        // indeterminate bar's sweep is a forever animation, re-armed whenever
        // the control re-laid out under it.
        if node.kind == ControlKind::ProgressBar {
            progress_sweep(node, relaid);
        }
        return;
    }
    match node.kind {
        ControlKind::Slider => slider_sync(comp, atlas, node, scale, scrubbing),
        ControlKind::Meter => meter_sync(comp, atlas, node, scale, scrubbing),
        ControlKind::ProgressRing => ring_sync(comp, node),
        _ => {}
    }
}

// ── Hover / press ink (button family + select triggers) ──────────────────────

/// Button-family ink geometry: full node rect at the control's corner radius.
fn ink_radius(node: &Node) -> f32 {
    match node.kind {
        ControlKind::ComboBox | ControlKind::DropDownButton => theme::RADIUS_SM,
        // The button family carries its own radius from birth, so the ink
        // follows it exactly — a wash with a different corner than the fill it
        // sits on reads as a rendering fault at any radius but the default.
        // Resolved, not raw: a pill's authored radius is unbounded, and the ink
        // has to land on the same curve the fill under it was cut to.
        _ => super::controls::resolve_radius(node.paint.corner_radius, node.rect.h),
    }
}

/// The button family's full retained chrome: fill and border BELOW the painted
/// surface, the hover/press wash above it.
///
/// Lifting the fill and border out of the surface is what makes a state flip
/// free: an enable/disable is a part opacity, a variant or checked change is a
/// re-bind to a different atlas source, and neither reaches a `BeginDraw`. The
/// label still paints (it is the only thing left on the surface), so a text
/// change is the one edit that costs a repaint.
///
/// Five parts, and no surface between them: fill, border and badge plate in the
/// lower group, ink wash and focus ring in the upper. The label and the two
/// ornament runs are glyph sprites above all of it
/// (`glyph_text::button_sync`), which is the stacking the fully-painted version
/// produced — except for the wash, which now sits under the text rather than
/// over it.
/// The button family's part slots, named next to the counts they must agree
/// with.
///
/// The count passed to [`ensure`] and the indices used to reach the parts were
/// once two independent sets of literals in two places; adding the badge plate
/// raised one and not the other, and the miss is an index-out-of-bounds on the
/// first button that paints. Deriving the counts from the last index is what
/// makes that particular mistake unspellable.
mod slot {
    pub const FILL: usize = 0;
    pub const BORDER: usize = 1;
    pub const PLATE: usize = 2;
    pub const N_BELOW: usize = PLATE + 1;

    pub const INK: usize = 0;
    pub const RING_INNER: usize = 1;
    pub const RING_OUTER: usize = 2;
    pub const N_ABOVE: usize = RING_OUTER + 1;
}

/// The focus visual, WinUI's shape: a solid outer ring with a hairline of the
/// window base between it and the control, both sitting just OUTSIDE the
/// control's bounds.
///
/// Outside, not inset, is the part that matters visually. An inset ring eats
/// into the control's own fill, so a focused button reads as having grown a
/// thick border and lost 3 DIP of its face; WinUI cuts the ring out of the gap
/// around the control instead, which is why its focus never changes how big the
/// control looks.
///
/// Returned as `(outward offset, stroke width, colour)` per ring, outermost
/// last, so the caller places them without restating the ladder.
///
/// A part's stroke runs from its box edge INWARD, so each ring's offset is how
/// far its own outer face sits from the control — the inner ring by its own
/// width, the outer by both. Derived rather than written down, because the two
/// were once a hardcoded `1.0` and `3.0` that only agreed with a 2 DIP stroke
/// and would have opened a gap the moment the width changed.
///
/// ## Both widths are snapped to WHOLE PHYSICAL PIXELS first
///
/// A stroke whose width is a fraction of a pixel has no clean edge on either
/// side: the rasterizer antialiases both, so the ring reads soft, slightly
/// wider than its number, and unmistakably "not high-DPI" — the same tell a
/// hairline drawn at a fractional offset gives. An authored width in DIPs
/// becomes fractional at most display scales (1.5 DIP is 1.875 px at 125%),
/// which is exactly when it happens.
///
/// Snapping the OFFSET matters as much as the width: it is what puts the ring's
/// outer face on a pixel boundary in the first place, and a crisp width at a
/// half-pixel offset is just as soft.
fn focus_rings(scale: f32) -> [(f32, f32, crate::Color); 2] {
    // At least one physical pixel — a ring rounded away is worse than a thin one.
    let px = |v: f32| ((v * scale).round().max(1.0)) / scale;
    let inner = px(super::controls::FOCUS_RING_INNER_W);
    let outer = px(super::controls::FOCUS_RING_W);
    [
        (inner, inner, theme::focus_inner()),
        (inner + outer, outer, theme::focus_outer()),
    ]
}


/// Below: `[fill, border, badge plate]`; above: `[ink, focus ring inner, focus
/// ring outer]`.
///
/// Nothing in the family travels: the control is a stack of sprites cut to one
/// curve, and every state it has is carried by opacity. The ink fades; the rest
/// snaps.
pub(crate) fn button_plan(node: &Node, scale: f32) -> PartPlan {
    let (w, h) = (node.rect.w, node.rect.h);
    let pal = super::controls::button_palette(node);
    // From the palette, not re-derived: the fill, the border and the ink are
    // three sprites cut to the same curve, and the palette is where that curve
    // is decided (see [`ButtonPalette`]).
    let radius = pal.radius;
    let dim = dim_of(node);
    let box_rect = Some((0.0, 0.0, w, h));
    // A fully transparent fill (the bare / chromeless variants at rest) binds
    // nothing: an atlas source that paints no pixels is a wasted raster and a
    // wasted cache slot.
    let fill = match (pal.fill.a > 0.0).then(|| AtlasKey::hbar(h, radius, 0.0, pal.fill, scale)) {
        Some(k) => SlotPlan::snap(k, box_rect, dim),
        None => SlotPlan::hidden(),
    };
    let border = match pal
        .border
        .map(|c| AtlasKey::hbar(h, radius, theme::BORDER_W, c, scale))
    {
        Some(k) => SlotPlan::snap(k, box_rect, dim),
        None => SlotPlan::hidden(),
    };

    // The badge plate: a stadium in the badge's own tint, over the button's
    // fill and under the count's glyph sprites. Its radius is half its height,
    // so it stays round at any width — the dot form is simply the case where
    // that width IS the height.
    let plate = super::controls::button_boxes(node, Rect::from_xywh(0.0, 0.0, w, h))
        .badge
        .zip(super::controls::badge_paint(node, &pal))
        .map(|(b, (fill, _))| {
            let bh = b.height();
            AtlasKey::hbar(bh, bh / 2.0, 0.0, fill, scale).snap_at(Some(r4(b)), dim)
        })
        .unwrap_or_else(SlotPlan::hidden);

    PartPlan::new([w, h, 0.0])
        .below(slot::FILL, fill)
        .below(slot::BORDER, border)
        .below(slot::PLATE, plate)
        .above(
            slot::INK,
            AtlasKey::hbar(h, ink_radius(node), 0.0, theme::w(1.0), scale)
                .snap_at(box_rect, ink_target(node))
            .fading(),
        )
        .focus_ring(slot::RING_INNER, node.focus_ring, w, h, radius, scale)
}

/// The focus visual as two parts rather than a draw, for any control that owns
/// no surface to paint one on.
///
/// Each ring's radius follows the control's AUTHORED one, grown by how far out
/// the ring sits — a ring whose corners disagree with the control inside it is
/// the most visible way for a custom radius to look broken.
///
/// This is deliberately the ONLY focus geometry. The painted
/// `controls::draw_focus_ring` it replaces drew a different visual — a single
/// stroke INSET into the control — and the inset is what
/// [`focus_rings`] exists to argue against: it eats the control's own fill, so
/// a focused control reads as having grown a border and shrunk. Moving a
/// control to retained chrome therefore also moves it onto the correct ring,
/// and the two must not be allowed to coexist per-kind.
pub(crate) fn focus_ring_slots(
    focused: bool,
    w: f32,
    h: f32,
    radius: f32,
    scale: f32,
) -> [SlotPlan; 2] {
    let Some(rings) = focused.then(|| focus_rings(scale)) else {
        return [SlotPlan::hidden(), SlotPlan::hidden()];
    };
    std::array::from_fn(|i| {
        let (out, sw, c) = rings[i];
        let ring = (-out, -out, w + 2.0 * out, h + 2.0 * out);
        AtlasKey::hbar((h + 2.0 * out).max(0.0), radius + out, sw, c, scale)
            .snap_at(Some(ring), 1.0)
    })
}

/// The caption band's chrome: one hover wash, and nothing else.
///
/// The band itself is transparent (the window's backdrop shows through) and its
/// six runs are sprites, so the wash is all the chrome there is. It fades rather
/// than snapping — a pointer sweeping the window cluster crosses three buttons
/// in a few frames, and a plate that popped on and off at that rate reads as a
/// flicker.
pub(crate) fn caption_plan(node: &Node, scale: f32) -> PartPlan {
    let (w, h) = (node.rect.w, node.rect.h);
    let wash = super::caption::hot_wash(node, Rect::from_xywh(0.0, 0.0, w, h))
        .map(|(r, radius, c)| {
            AtlasKey::hbar(r.height(), radius, 0.0, c, scale).snap_at(Some(r4(r)), 1.0)
            .fading()
        })
        .unwrap_or_else(SlotPlan::hidden);
    PartPlan::new([w, h, 0.0]).above(0, wash)
}


/// An `InfoBar`'s chrome: the card, its severity tint, its border, and the
/// close button's hover wash.
///
/// Four sprites cut to one curve, replacing four fills that were redrawn
/// together whenever any one of them changed. The wash is the only part with a
/// state: it fades, because a hover that snapped a plate on and off reads as a
/// flicker at the speed a pointer crosses a button.
///
/// The band takes no focus ring — the BAR is not focusable; only its close
/// button is, and that is a synthetic UIA item rather than a node with a rect
/// of its own.
pub(crate) fn bar_plan(node: &Node, scale: f32) -> PartPlan {
    let (w, h) = (node.rect.w, node.rect.h);
    let x = node.extras();
    let sev = super::info_bar::severity(x);
    let r = theme::RADIUS_SM;
    let dim = dim_of(node);
    let box_rect = Some((0.0, 0.0, w, h));

    // The card, then a wash of the severity role over it, so the bar reads as
    // its status at a glance without the text having to say so.
    let card = AtlasKey::hbar(h, r, 0.0, theme::surface_raised(), scale).snap_at(box_rect, dim);
    let tint_c = theme::with_alpha(sev.color(), 0.10);
    let tint = AtlasKey::hbar(h, r, 0.0, tint_c, scale).snap_at(box_rect, dim);
    let border =
        AtlasKey::hbar(h, r, theme::BORDER_W, theme::stroke(), scale).snap_at(box_rect, dim);

    // The close button's wash. Hidden rather than transparent when the pointer
    // is elsewhere: a part bound to a source it never shows still holds an
    // atlas slot.
    let hot = node.paint.is_enabled && node.ctrl().hot_index == super::info_bar::HOT_CLOSE;
    let wash = super::info_bar::close_rect(w, h, x.bar_closable)
        .filter(|_| hot)
        .map(|c| {
            let a = if node.pressed { 0.10 } else { 0.06 };
            AtlasKey::hbar(c.height(), r, 0.0, theme::w(a), scale).snap_at(Some(r4(c)), dim)
            .fading()
        })
        .unwrap_or_else(SlotPlan::hidden);

    PartPlan::new([w, h, 0.0])
        .below(0, card)
        .below(1, tint)
        .below(2, border)
        .above(0, wash)
}


/// An `InfoBadge`'s whole appearance: one plate, and nothing else.
///
/// Both forms are the same stadium — the dot is the case where the box is
/// square, which is how the in-button badge plate (`button_plan`) has always
/// treated it — so one shape key serves both and there is no second branch to
/// keep in step with `info_badge::measure`. The count rides above as glyph
/// sprites, which leaves the badge with no surface at all.
///
/// No ring and no ink: a badge is not focusable and not interactive. It reports
/// a number, and the only thing that can change about it is that number and its
/// colour.
pub(crate) fn badge_plan(node: &Node, scale: f32) -> PartPlan {
    let (w, h) = (node.rect.w, node.rect.h);
    // The accent role unless the app set an explicit `Background` — what lets a
    // host colour a badge by meaning (an error count in the danger role)
    // without this control modelling severity the way the InfoBar does. A badge
    // carries a number, not a status.
    let fill = node.paint.background.unwrap_or_else(theme::accent);
    let b = super::info_badge::plate_box(node, w, h);
    let bh = b.height();
    PartPlan::new([w, h, 0.0]).below(
        0,
        SlotPlan::snap(
            // Radius half the height, so it stays round at any width.
            AtlasKey::hbar(bh, bh / 2.0, 0.0, fill, scale),
            Some((b.left, b.top, b.width(), bh)),
            dim_of(node),
        ),
    )
}


/// A `HyperlinkButton`'s whole appearance: the focus ring, and nothing else.
///
/// The link's words are glyph sprites and its hover recolour is a `SetSource`
/// on them, so once the ring is retained there is nothing left for a surface to
/// hold — which is the point. It takes no ink wash: a hyperlink's hover
/// affordance is the colour change, not a plate behind it.
pub(crate) fn hyperlink_plan(node: &Node, scale: f32) -> PartPlan {
    let (w, h) = (node.rect.w, node.rect.h);
    PartPlan::new([w, h, 0.0])
        .focus_ring(0, node.focus_ring, w, h, theme::RADIUS_SM, scale)
}


/// Above: `[hover/press wash]`. The select triggers paint their own fill and
/// border, so the ink is the whole of their retained chrome.
pub(crate) fn ink_plan(node: &Node, scale: f32) -> PartPlan {
    let (w, h) = (node.rect.w, node.rect.h);
    PartPlan::new([w, h, 0.0]).above(
        0,
        AtlasKey::hbar(h, ink_radius(node), 0.0, theme::w(1.0), scale)
            .snap_at(Some((0.0, 0.0, w, h)), ink_target(node))
        .fading(),
    )
}

/// The combined hover + press wash target (endpoint parity with the painted
/// `0.06·hover + 0.04·press` white wash).
fn ink_target(node: &Node) -> f32 {
    let mut authored = 0.0;
    if node.hovered {
        authored += 0.06;
    }
    if node.pressed {
        authored += 0.04;
    }
    wash(authored) * dim_of(node)
}

/// Direct event entry: hover / press flipped on a converted control. Retargets
/// the relevant opacity springs; no tick, no repaint.
pub(crate) fn ink_state_changed(node: &mut Node) {
    match node.kind {
        ControlKind::ToggleSwitch => toggle_fades(node),
        ControlKind::Slider => {
            let t = halo_target(node);
            if let Some(parts) = node.parts.as_mut()
                && parts.above.len() == 3
            {
                parts.above[1].fade_to(t);
            }
        }
        // Place *and* fade: on hover entry the hot segment was recorded
        // before this flip, so the ink must land on it, not fade in wherever
        // it last sat.
        ControlKind::SelectorBar => seg_hot_changed(node),
        // Same reason as the SelectorBar, and the same hazard the default arm
        // below would cause: a nav pane's `above[0]` is the ROW ink, placed on
        // the hot row. Fading it to a node-level hover target would light it up
        // wherever it last sat, on whichever row the pointer left.
        ControlKind::NavigationView => nav_hot_changed(node),
        // No ink: the CheckBox / hyperlink hover feedback is painted (the
        // caller repaints once, event-driven); progress is not interactive.
        // Careful: the CheckBox's above[0] is its CHECKMARK — the default arm
        // would fade it on hover.
        ControlKind::CheckBox
        | ControlKind::HyperlinkButton
        | ControlKind::ProgressBar
        | ControlKind::ProgressRing => {}
        _ => {
            let t = ink_target(node);
            if let Some(parts) = node.parts.as_mut()
                && !parts.above.is_empty()
            {
                parts.above[0].fade_to(t);
            }
        }
    }
}

// ── ToggleSwitch ─────────────────────────────────────────────────────────────

/// Track geometry mirrors the retired `paint_toggle_switch`.
pub(crate) const TRACK_W: f32 = 40.0;
const TRACK_H: f32 = 20.0;
const KNOB_D: f32 = 12.0;

/// The off-track outline is authored at the hover-bright alpha; the rest state
/// is expressed as sprite opacity, so hover is a pure compositor fade.
/// (`wash_alpha(0.20) / wash_alpha(0.28)` — endpoint-exact with the painted
/// `w((0.20 + 0.08·hover)·(1−t))` stroke.)
const OUTLINE_AUTHORED: f32 = 0.28;
fn outline_rest_factor() -> f32 {
    wash(0.20) / wash(OUTLINE_AUTHORED)
}


/// Below-band roles: `[on track, off track, knob]`.
///
/// The knob GLIDES and carries no "was it on last time" shadow, for the same
/// reason the segmented pill carries none: its x is a pure function of `is_on`,
/// and `Part`'s own channel already knows whether that moved. The shadow is
/// exactly what stopped the knob sliding — a flip would start the glide, and the
/// next sync would find `parts.on == on` and take the AUTHORITATIVE `place`
/// branch, which stops the spring dead and jumps the knob to the end.
pub(crate) fn toggle_plan(node: &Node, scale: f32) -> PartPlan {
    let cy = node.rect.h / 2.0;
    let on = node.ctrl().is_on;
    let dim = dim_of(node);
    let (kx_off, kx_on) = knob_xs();
    let kx = if on { kx_on } else { kx_off };
    let (on_t, off_t) = track_targets(on, node.hovered, dim);

    let track = Some((0.0, cy - TRACK_H / 2.0, TRACK_W, TRACK_H));
    let knob = Some((kx, cy - KNOB_D / 2.0, KNOB_D, KNOB_D));

    PartPlan::new([node.rect.w, node.rect.h, 0.0])
        // The two tracks are stacked and cross-fade in place; only their opacity
        // carries the state, so neither ever travels.
        .below(
            0,
            AtlasKey::hbar(TRACK_H, TRACK_H / 2.0, 0.0, theme::accent(), scale).snap_at(track, on_t)
            .fading(),
        )
        .below(
            1,
            AtlasKey::hbar(TRACK_H, TRACK_H / 2.0, 1.5, theme::w(OUTLINE_AUTHORED), scale)
                .snap_at(track, off_t)
            .fading(),
        )
        // The knob is the one thing that moves. Opacity 1.0 rather than `dim`
        // preserves the previous behaviour, which never wrote the knob's opacity
        // at all.
        .below(
            2,
            AtlasKey::circle(KNOB_D, theme::w(1.0), scale).glide_at(knob, 1.0),
        )
        // The ring takes the WHOLE node — track, gap and label — not the track
        // alone, which is what the painted ring it replaces did and what WinUI's
        // own switch does. A ring drawn round the track would read as focusing a
        // different, smaller control than the one the label names.
        .node_focus_ring(0, node, scale)
}

fn knob_xs() -> (f32, f32) {
    // Knob centres 8 / 32 DIPs into the 40-DIP track (2-DIP end margins).
    let r = KNOB_D / 2.0;
    (2.0, TRACK_W - 2.0 - 2.0 * r)
}

fn track_targets(on: bool, hovered: bool, dim: f32) -> (f32, f32) {
    let on_t = if on { dim } else { 0.0 };
    let off_t = if on {
        0.0
    } else {
        (outline_rest_factor() + (1.0 - outline_rest_factor()) * f32::from(hovered as u8)) * dim
    };
    (on_t, off_t)
}

/// Hover flipped on the toggle: refade the off-track outline.
fn toggle_fades(node: &mut Node) {
    let (_, off_t) = track_targets(node.ctrl().is_on, node.hovered, dim_of(node));
    if let Some(parts) = node.parts.as_mut()
        && parts.below.len() == 3
    {
        parts.below[1].fade_to(off_t);
    }
}

// ── CheckBox ─────────────────────────────────────────────────────────────────

/// Box side, mirroring the retired `paint_check_box`.
pub(crate) const CHECK_BOX_D: f32 = 18.0;
/// Its outline's stroke width, from the same retired paint.
const CHECK_OUTLINE_W: f32 = 1.5;

/// Below: `[accent box fill, outline]`. Above: `[checkmark, focus ring ×2]`.
/// A check/uncheck is a pair of compositor fades — endpoint parity with the
/// retired painted crossfade (`transparent→accent` fill, `w(on)` checkmark).

/// Below: `[accent box fill]`; above: `[checkmark]`.
///
/// Nothing here travels — a check is a pair of fades — so the `parts.on` shadow
/// was suppressing the fades rather than a glide, snapping the crossfade to its
/// endpoint on the sync after the one that started it.
pub(crate) fn check_plan(node: &Node, scale: f32) -> PartPlan {
    let t = if node.ctrl().is_checked { dim_of(node) } else { 0.0 };
    let y = node.rect.h / 2.0 - CHECK_BOX_D / 2.0;
    let box_rect = Some((0.0, y, CHECK_BOX_D, CHECK_BOX_D));

    // The outline, hover-brightened. A part rather than a stroke on a surface,
    // which is what lets the hover be a re-bind instead of the repaint that
    // used to redraw the label alongside it.
    let stroke = theme::w(if node.hovered { 0.36 } else { 0.30 });
    let outline = AtlasKey::hbar(CHECK_BOX_D, theme::RADIUS_SM, CHECK_OUTLINE_W, stroke, scale)
        .snap_at(box_rect, dim_of(node));

    // The ring takes the WHOLE node — box, gap and label — as WinUI's does, and
    // as the painted ring this replaces did.

    PartPlan::new([node.rect.w, node.rect.h, 0.0])
        .below(
            0,
            AtlasKey::hbar(CHECK_BOX_D, theme::RADIUS_SM, 0.0, theme::accent(), scale)
                .snap_at(box_rect, t)
            .fading(),
        )
        .below(1, outline)
        .above(
            0,
            AtlasKey::check(CHECK_BOX_D, theme::w(1.0), scale).snap_at(box_rect, t).fading(),
        )
        .node_focus_ring(1, node, scale)
}

// ── Slider ───────────────────────────────────────────────────────────────────

fn halo_target(node: &Node) -> f32 {
    if node.hovered || node.pressed {
        wash(0.10) * dim_of(node)
    } else {
        0.0
    }
}

/// The slider's fill-origin as a 0..1 track fraction (`fill_origin` clamped
/// into `[min, max]`; unset = 0.0, i.e. fill from the `min` end).
pub(crate) fn slider_origin_frac(node: &Node) -> f32 {
    let Some(o) = node.ctrl().fill_origin else { return 0.0 };
    let span = node.ctrl().max - node.ctrl().min;
    if span.abs() < f64::EPSILON {
        0.0
    } else {
        ((o - node.ctrl().min) / span).clamp(0.0, 1.0) as f32
    }
}

/// The fill color for a value at `vfrac`, split at the fill origin: at or
/// below → `fill_color`, above → `fill_color_alt` (each falling back toward
/// the theme accent). Authored colors — the atlas raster display-maps them.
fn slider_fill_color(node: &Node, vfrac: f32, ofrac: f32) -> crate::Color {
    let below = node.ctrl().fill_color.unwrap_or_else(theme::accent);
    if vfrac <= ofrac {
        below
    } else {
        node.ctrl().fill_color_alt.unwrap_or(below)
    }
}

/// Above-band roles: `[fill, halo, thumb]`.
fn slider_sync(
    comp: &Compositing,
    atlas: &mut Atlas,
    node: &mut Node,
    scale: f32,
    scrubbing: bool,
) {
    if !ensure(comp, node, 0, 3) {
        return;
    }
    let frac = super::ctrl_value_frac(node) as f32;
    let ofrac = slider_origin_frac(node);
    let dim = dim_of(node);
    let halo_t = halo_target(node);
    let fill_c = slider_fill_color(node, frac, ofrac);
    let k_fill = AtlasKey::hbar(theme::SLIDER_TRACK, theme::SLIDER_TRACK / 2.0, 0.0, fill_c, scale);
    let k_halo = AtlasKey::circle(theme::SLIDER_THUMB + 6.0, theme::w(1.0), scale);
    let k_thumb = AtlasKey::circle(theme::SLIDER_THUMB, theme::w(1.0), scale);

    let g = slider_geom(node.rect.w, node.rect.h, frac, ofrac);
    let geom = (node.rect.w, node.rect.h);
    let Some(parts) = node.parts.as_mut() else { return };
    // Snap while this slider is the one under the pointer (direct manipulation
    // goes exactly where you put it) and while ANY drag is streaming updates
    // (following the dial 1:1 — a spring retargeted per move would pin it).
    // A discrete change still glides.
    let snap = !parts.init || parts.geom != geom || node.pressed || scrubbing;

    parts.above[0].bind(comp, atlas, k_fill);
    parts.above[1].bind(comp, atlas, k_halo);
    parts.above[2].bind(comp, atlas, k_thumb);

    slider_apply(parts, &g, snap);
    parts.above[0].set_opacity(dim);
    parts.above[1].fade_to(halo_t);
    parts.above[2].set_opacity(dim);
    parts.geom = geom;
    parts.init = true;
}

struct SliderGeom {
    /// The static full-track fill sprite.
    fill: (f32, f32, f32, f32),
    /// `(x0, x1, origin_x)` in node coords — the constants the fill
    /// derivation is written against.
    anchor: (f32, f32, f32),
    halo: (f32, f32, f32, f32),
    thumb: (f32, f32, f32, f32),
}

fn slider_geom(w: f32, h: f32, frac: f32, ofrac: f32) -> SliderGeom {
    let cy = h / 2.0;
    let inset = theme::SLIDER_THUMB / 2.0;
    let x0 = inset;
    let x1 = (w - inset).max(x0);
    let frac = frac.clamp(0.0, 1.0);
    let thumb_x = x0 + (x1 - x0) * frac;
    // The fill spans origin → thumb, whichever side of the origin the value
    // sits on (ofrac 0.0 = the classic fill-from-min).
    let origin_x = x0 + (x1 - x0) * ofrac.clamp(0.0, 1.0);
    let fill_lo = thumb_x.min(origin_x);
    let fill_hi = thumb_x.max(origin_x);
    let tr = theme::SLIDER_TRACK;
    let halo_d = theme::SLIDER_THUMB + 6.0;
    SliderGeom {
        // The fill sprite IS the coloured span, so its nine-grid rounds both
        // ends. Its left edge and width are derived from the thumb while it
        // glides (see `slider_fill_follow`) rather than sprung independently.
        fill: (fill_lo, cy - tr / 2.0, fill_hi - fill_lo, tr),
        anchor: (x0, x1, origin_x),
        halo: (thumb_x - halo_d / 2.0, cy - halo_d / 2.0, halo_d, halo_d),
        thumb: (
            thumb_x - theme::SLIDER_THUMB / 2.0,
            cy - theme::SLIDER_THUMB / 2.0,
            theme::SLIDER_THUMB,
            theme::SLIDER_THUMB,
        ),
    }
}

fn slider_apply(parts: &mut Parts, g: &SliderGeom, snap: bool) {
    // The ONE way a part moves here. Everything that animates therefore goes
    // through `Part::glide` and so shares `CHROME_SPRING_PERIOD` / `_DAMPING` —
    // the coupling the settle batch below depends on.
    let put = |p: &mut Part, r: (f32, f32, f32, f32), snap: bool| {
        if snap {
            p.place(r.0, r.1, r.2, r.3);
        } else {
            p.glide(r.0, r.1, r.2, r.3);
        }
    };
    // First write snaps, matching `Part::glide` — mounting must not fly in.
    let snap = snap || (!parts.above[0].off.placed() && !parts.fill_live.get());
    if snap {
        slider_fill_static(parts, g.fill);
        put(&mut parts.above[1], g.halo, true);
        put(&mut parts.above[2], g.thumb, true);
        return;
    }
    // Arm the derivation BEFORE opening the batch: an expression never
    // completes, so one started inside would keep the batch — and therefore the
    // follow — alive forever.
    if !slider_fill_follow(parts, g.anchor) {
        slider_fill_static(parts, g.fill);
        put(&mut parts.above[1], g.halo, true);
        put(&mut parts.above[2], g.thumb, true);
        return;
    }
    // The halo and the thumb BOTH glide inside this batch, so the batch does
    // not complete when the thumb does — it completes when the last of them
    // does. Reading that completion as "the derived fill has arrived" is sound
    // only because every animation in the batch is a `Part::glide`, and every
    // `Part::glide` is a spring built with the one shared `CHROME_SPRING_PERIOD`
    // / `_DAMPING` pair: same tuning, both retargeted in the same tick, so
    // they settle together and the thumb-derived fill is at its final rect
    // whichever one reports last.
    //
    // That is the invariant to preserve: anything added to this batch must go
    // through `Part::glide` (hence the shared tuning). An animation with its own
    // timing — a keyframe fade, a differently tuned spring — would push the
    // completion past the thumb's arrival and leave the derivation evaluating
    // every frame until it finished, or (if shorter) tear the derivation down
    // while the fill was still mid-flight.
    let batch = parts
        .above[2]
        .obj
        .Compositor()
        .ok()
        .and_then(|c| c.CreateScopedBatch(CompositionBatchTypes::Animation).ok());
    put(&mut parts.above[1], g.halo, false);
    put(&mut parts.above[2], g.thumb, false);
    slider_fill_settle(parts, batch, g.fill);
}

/// Put the fill at a plain rect NOW, releasing any derivation driving it.
/// Authoritative, like [`Part::place`]: a live derivation is torn down and the
/// rect rewritten even when the target is unchanged.
fn slider_fill_static(parts: &mut Parts, r: (f32, f32, f32, f32)) {
    // Invalidate any in-flight settle: this write supersedes it.
    parts.fill_gen.set(parts.fill_gen.get().wrapping_add(1));
    let fill = &mut parts.above[0];
    if parts.fill_live.replace(false) {
        let _ = fill.obj.StopAnimation("Offset.X");
        let _ = fill.obj.StopAnimation("Size.X");
        // The derivation wrote X behind `Part`'s back and is now stopped: no
        // animation holds either property, but where it left them is unknown,
        // so the `place` below must write unconditionally.
        fill.off.reclaimed();
        fill.size.reclaimed();
    }
    fill.place(r.0, r.1, r.2, r.3);
}

/// Derive the fill's own rect from the THUMB's live offset for the duration of
/// its glide, then settle to plain values.
///
/// The fill spans `origin → thumb`, so its left edge and width are `Min`/`Abs`
/// of the thumb position — affine only within one side of the origin. Springing
/// them alongside the thumb is exact while the sign holds but wrong across a
/// crossing: the left edge is still short of the origin while the right is
/// already past it, so the bar straddles zero as a wide band instead of
/// collapsing to nothing. Deriving both from the one thumb spring reproduces
/// that kink exactly, and keeps the sprite sized to the visible span so its
/// nine-grid rounds both ends the way a plain placed fill does.
///
/// `anchor` is `(x0, x1, origin_x)` in node coordinates.
fn slider_fill_follow(parts: &mut Parts, anchor: (f32, f32, f32)) -> bool {
    let (_x0, _x1, origin_x) = anchor;
    let armed = parts.fill_live.get()
        || (|| -> Option<()> {
            let comp = parts.above[0].obj.Compositor().ok()?;
            let thumb: CompositionObject =
                windows_core::Interface::cast(&parts.above[2].sprite).ok()?;
            let half = theme::SLIDER_THUMB / 2.0;
            // Thumb sprite sits at `thumb_x - half`, so its centre is X + half.
            for (prop, text) in [
                ("Offset.X", format!("Min(t.Offset.X + {half:.3}, {origin_x:.3})")),
                ("Size.X", format!("Abs(t.Offset.X + {half:.3} - {origin_x:.3})")),
            ] {
                let expr = comp.CreateExpressionAnimationWithExpression(&text).ok()?;
                windows_core::Interface::cast::<ICompositionAnimation>(&expr)
                    .ok()?
                    .SetReferenceParameter("t", &thumb)
                    .ok()?;
                parts.above[0]
                    .obj
                    .StartAnimation(
                        prop,
                        &windows_core::Interface::cast::<CompositionAnimation>(&expr).ok()?,
                    )
                    .ok()?;
            }
            Some(())
        })()
        .is_some();
    if armed {
        parts.fill_live.set(true);
        // The expressions own X now. Ceding is the one call that says both
        // halves: the cached rect no longer describes the visual (so it must
        // not suppress a later write) AND a `Part::place` must stop these
        // subchannel animations before it can reclaim the properties.
        parts.above[0].off.ceded("Offset.X");
        parts.above[0].size.ceded("Size.X");
    }
    armed
}

/// Settle: when the thumb's glide completes, drop the derivation and write the
/// final rect, so an idle slider leaves nothing evaluating per frame.
fn slider_fill_settle(
    parts: &mut Parts,
    batch: Option<CompositionScopedBatch>,
    r: (f32, f32, f32, f32),
) {
    let generation = parts.fill_gen.get().wrapping_add(1);
    parts.fill_gen.set(generation);
    let settled = batch.and_then(|b| {
        let vis = parts.above[0].vis.clone();
        let obj = parts.above[0].obj.clone();
        let cell = parts.fill_gen.clone();
        let live = parts.fill_live.clone();
        let revoker = b
            .Completed(move |_, _| {
                if cell.get() == generation {
                    let _ = obj.StopAnimation("Offset.X");
                    let _ = obj.StopAnimation("Size.X");
                    let _ = vis.SetOffset(Vector3::new(r.0, r.1, 0.0));
                    let _ = vis.SetSize(Vector2::new(r.2, r.3));
                    live.set(false);
                }
            })
            .ok()
            .filter(|_| b.End().is_ok())?;
        Some(revoker)
    });
    match settled {
        Some(rev) => parts._fill_settle = Some(rev),
        // No completion signal will arrive — settle now rather than leave the
        // derivation evaluating forever.
        None => slider_fill_static(parts, r),
    }
}

/// Direct event entry: a pointer drag scrubs the slider 1:1 — snap the fill /
/// halo / thumb to `frac` with plain property sets (no repaint, no tick).
/// The fill *color* is not touched here; an origin-side crossing marks the
/// node dirty (see `input::slider_to`) and the repaint's sync rebinds it.
pub(crate) fn slider_drag(node: &mut Node, frac: f32) -> bool {
    let ofrac = slider_origin_frac(node);
    let g = slider_geom(node.rect.w, node.rect.h, frac, ofrac);
    let Some(parts) = node.parts.as_mut() else { return false };
    if parts.above.len() != 3 {
        return false;
    }
    slider_apply(parts, &g, true);
    true
}

// ── Meter ────────────────────────────────────────────────────────────────────

/// Above-band roles: `[fill, marker, halo, needle]`.
///
/// The fill is a full-track gradient raster revealed by an `InsetClip` whose
/// `RightInset` FOLLOWS the needle sprite's animated `Offset.X` through an
/// `ExpressionAnimation` — the one needle glide (a compositor Vector3 spring)
/// drives both, so the fill edge and the needle never separate and a level
/// change never repaints or ticks.
fn meter_sync(
    comp: &Compositing,
    atlas: &mut Atlas,
    node: &mut Node,
    scale: f32,
    scrubbing: bool,
) {
    if !ensure(comp, node, 0, 4) {
        return;
    }
    let frac = (super::ctrl_value_frac(node) as f32).clamp(0.0, 1.0);
    let (w, h) = (node.rect.w, node.rect.h);
    let dim = dim_of(node);
    let top = theme::METER_INSET;
    let bar_h = (h - 2.0 * top).max(1.0);
    let marker_x = super::controls::meter_marker_frac(node).map(|f| f * w);
    let marker_c = node.ctrl().marker_color.unwrap_or_else(|| theme::w(0.15));

    let k_marker = AtlasKey::solid(marker_c, scale);
    let k_white = AtlasKey::solid(theme::w(1.0), scale);

    let geom = (w, h);
    let gradient = !node.ctrl().stops.is_empty();
    let (ctrl, parts) = node.ctrl_and_parts();
    let Some(parts) = parts else { return };
    let k_fill = if gradient {
        // Rounded ends (nine-grid) so the coloured fill matches the groove's
        // rounded corners; the reveal clip trims the straight leading edge.
        grad_bar_key(&mut parts.grad_key, &ctrl.stops, theme::METER_RADIUS, bar_h, scale)
    } else {
        AtlasKey::hbar(bar_h, theme::METER_RADIUS, 0.0, theme::accent(), scale)
    };
    // A meter is a pure follower: it glides to a discrete change, but tracks a
    // drag 1:1. Springing a stream of per-move updates would restart the needle
    // spring every frame and leave the level pinned until the pointer stopped.
    let snap = !parts.init || parts.geom != geom || scrubbing;

    parts.above[0].bind(comp, atlas, k_fill);
    parts.above[1].bind(comp, atlas, k_marker);
    parts.above[2].bind(comp, atlas, k_white.clone());
    parts.above[3].bind(comp, atlas, k_white);

    parts.above[0].place(0.0, top, w, bar_h);
    if let Some(mx) = marker_x {
        parts.above[1].place(mx - 0.5, 0.0, 1.0, h);
    }
    // The needle (a soft halo under a crisp core) rides the fill edge, full
    // height so it overhangs the groove like the retired drawn meter.
    let nx = frac * w;
    if snap {
        parts.above[2].place(nx - 2.0, 0.0, 4.0, h);
        parts.above[3].place(nx - 1.0, 0.0, 2.0, h);
    } else {
        parts.above[2].glide(nx - 2.0, 0.0, 4.0, h);
        parts.above[3].glide(nx - 1.0, 0.0, 2.0, h);
    }
    parts.above[0].set_opacity(dim);
    parts.above[1].set_opacity(if marker_x.is_some() { dim } else { 0.0 });
    parts.above[2].set_opacity(0.25 * dim);
    parts.above[3].set_opacity(dim);

    meter_arm_clip(parts, w);

    parts.geom = geom;
    parts.init = true;
}

/// Ensure the fill's reveal clip exists and its follower expression matches
/// the current track width. The expression reads the needle core's animated
/// `Offset.X`, so a needle glide sweeps the reveal in lock-step; it rebuilds
/// only on a resize (the width is baked in as a constant).
fn meter_arm_clip(parts: &mut Parts, w: f32) {
    if parts.clip.is_some() && parts.clip_w == w {
        return;
    }
    let run = || -> Option<InsetClip> {
        let comp = parts.above[3].obj.Compositor().ok()?;
        let clip = match &parts.clip {
            Some(c) => c.clone(),
            None => {
                let c = comp.CreateInsetClip().ok()?;
                parts.above[0]
                    .vis
                    .SetClip(&windows_core::Interface::cast::<CompositionClip>(&c).ok()?)
                    .ok()?;
                c
            }
        };
        // Needle core is 2 DIPs wide at `nx - 1` → its centre is Offset.X + 1.
        let expr = comp.CreateExpressionAnimationWithExpression(&format!(
            "Max(0.0, {w:.2} - (n.Offset.X + 1.0))"
        ))
        .ok()?;
        let needle: CompositionObject =
            windows_core::Interface::cast(&parts.above[3].sprite).ok()?;
        windows_core::Interface::cast::<ICompositionAnimation>(&expr)
            .ok()?
            .SetReferenceParameter("n", &needle)
            .ok()?;
        let clip_obj: ICompositionObject = windows_core::Interface::cast(&clip).ok()?;
        clip_obj
            .StartAnimation(
                "RightInset",
                &windows_core::Interface::cast::<CompositionAnimation>(&expr).ok()?,
            )
            .ok()?;
        Some(clip)
    };
    if let Some(clip) = run() {
        parts.clip = Some(clip);
        parts.clip_w = w;
    }
}

// ── Segmented (SelectorBar) ──────────────────────────────────────────────────

/// Below-band roles: `[tray fill, tray stroke, pill, hover ink]`.

/// Below-band roles: `[tray fill, tray stroke, pill, hover ink]`.
///
/// The pill GLIDES and carries no "was the selection different last time"
/// shadow: its rect is a pure function of the selection, and `Part`'s own
/// channel already knows whether that rect moved. The shadow was only ever a
/// proxy for that question, and a proxy that could be committed on a path which
/// never placed the pill — which is precisely how it stranded.
pub(crate) fn segmented_plan(node: &Node, scale: f32) -> PartPlan {
    let n = node.ctrl().items.len();
    let (w, h) = (node.rect.w, node.rect.h);
    let accent = node.paint.style_variant == 1;
    let m = super::controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
    let edges = super::controls::segment_edges(node);
    let dim = dim_of(node);

    let tray_radius = if accent { h / 2.0 } else { theme::RADIUS_SM };
    let tray_bg = if accent { theme::w(0.06) } else { theme::stroke_subtle() };
    let pill_h = (h - 2.0 * m.tray).max(0.0);
    let seg_radius = if accent { pill_h / 2.0 } else { theme::RADIUS_BADGE };
    let pill_fill = if accent { theme::accent() } else { theme::stroke() };

    let sel = if n == 0 { -1 } else { (node.ctrl().selected_index.max(0)).min(n as i32 - 1) };
    let seg_rect = |i: i32| -> Option<Rect4> {
        let i = usize::try_from(i).ok()?;
        let (a, b) = (*edges.get(i)?, *edges.get(i + 1)?);
        Some((a, m.tray, b - a, pill_h))
    };
    let tray = Some((0.0, 0.0, w, h));
    let pill = seg_rect(sel);
    let k_pill = AtlasKey::hbar(pill_h, seg_radius, 0.0, pill_fill, scale);
    let k_ink = AtlasKey::hbar(pill_h, seg_radius, 0.0, theme::w(1.0), scale);

    // Segment boundaries move when a label re-measures, so the edge checksum
    // joins the size: the pill must jump to boundaries that changed under it
    // rather than slide to them.
    let edges_sig = edges.iter().sum::<f32>() + edges.len() as f32;

    PartPlan::new([w, h, edges_sig])
        .below(
            0,
            AtlasKey::hbar(h, tray_radius, 0.0, tray_bg, scale).snap_at(tray, dim),
        )
        .below(
            1,
            AtlasKey::hbar(h, tray_radius, theme::BORDER_W, theme::stroke(), scale)
                .snap_at(tray, dim),
        )
        .below(2, SlotPlan::glide(k_pill, pill, if pill.is_some() { dim } else { 0.0 }))
        // The ink SNAPS to the hot segment and fades: a glide would draw a wash
        // sliding across segments the pointer never crossed.
        .below(
            3,
            SlotPlan::snap(k_ink, seg_rect(node.ctrl().hot_index), seg_ink_target(node)).fading(),
        )
        // The ring rings the TRAY, not the selected segment: focus is on the
        // bar, and the selection already has the pill to show where it is.
        .node_focus_ring(0, node, scale)
}

fn seg_ink_target(node: &Node) -> f32 {
    if node.paint.is_enabled && node.hovered && node.ctrl().hot_index >= 0 {
        wash(0.05)
    } else {
        0.0
    }
}

/// Direct event entry: the hovered segment changed — snap the ink to the hot
/// segment and refade. (The caller still repaints the surface for the label
/// hover brightening.)
pub(crate) fn seg_hot_changed(node: &mut Node) {
    let m = super::controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
    let edges = super::controls::segment_edges(node);
    let pill_h = (node.rect.h - 2.0 * m.tray).max(0.0);
    let hot = node.ctrl().hot_index;
    let rect = usize::try_from(hot).ok().and_then(|i| {
        let (a, b) = (*edges.get(i)?, *edges.get(i + 1)?);
        Some((a, m.tray, b - a, pill_h))
    });
    let t = seg_ink_target(node);
    if let Some(parts) = node.parts.as_mut()
        && parts.below.len() == 4
    {
        if let Some(r) = rect {
            parts.below[3].place(r.0, r.1, r.2, r.3);
        }
        parts.below[3].fade_to(t);
    }
}

// ── NavigationView pane ──────────────────────────────────────────

/// The pane's slots. The counts derive from the last index in each band, so
/// adding a role cannot raise one and leave the other behind — the miss is an
/// index-out-of-bounds on the first pane that syncs.
mod nav_slot {
    pub const BG: usize = 0;
    /// The hairline between pane and content. It was the last thing the pane
    /// painted; as a part it glides in lockstep with the background rather than
    /// snapping to the new width a frame ahead of it.
    pub const DIVIDER: usize = 1;
    pub const MENU_TILE: usize = 2;
    pub const MENU_BAR: usize = 3;
    pub const SET_TILE: usize = 4;
    pub const SET_BAR: usize = 5;
    pub const N_BELOW: usize = SET_BAR + 1;

    pub const INK: usize = 0;
    /// The wash under a hovered back arrow or hamburger. One part for both:
    /// `hot_index` holds a single value, so only one can ever be hot.
    pub const CHROME_INK: usize = 1;
    pub const N_ABOVE: usize = CHROME_INK + 1;
}

/// Below-band roles: `[pane background, divider, menu tile, menu bar, settings
/// tile, settings bar]`; above: `[row hover ink, chrome-button wash]`.
///
/// Everything the pane shows that is not a glyph is here — it owns no surface
/// ([`nav`](super::nav)), so this band and the pane's sprites are its whole
/// appearance. Four things move and all four move on the compositor: the pane's
/// WIDTH when it opens or closes, the selection tile and its accent bar when the
/// selected page changes, and the hover ink as the pointer crosses rows.
///
/// The pane's runs SNAP to the new width in the same repaint that starts the
/// glide — the geometry is retained chrome, the text is not, and a text layout
/// cannot be interpolated.

/// Below-band roles: `[pane background, menu tile, menu bar, settings tile,
/// settings bar]`; above: `[row hover ink]`.
///
/// The menu list and the settings row get their OWN indicators rather than one
/// that travels between them. The settings row is pinned a pane-height below the
/// list, and sliding one indicator across that gap implies a continuity that is
/// not there — it reads as the tile falling past every row on the way. WinUI
/// gives its settings item a separate selection visual for the same reason.
/// Nothing travels between the regions; the two cross-fade in place, and each
/// still glides freely WITHIN its own region.
///
/// Only the HEIGHT enters the layout signature: a width change is exactly the
/// pane opening or closing, which is motion the pane is supposed to play, while
/// a height change is a resize and must not.
pub(crate) fn nav_plan(node: &Node, scale: f32) -> PartPlan {
    let (w, h) = (node.rect.w, node.rect.h);
    let dim = dim_of(node);
    let count = node.ctrl().items.len();
    let has_title = node.nav_text.as_ref().is_some_and(|t| t.title.is_some());
    let m = nav::metrics(node.extras(), w, has_title);
    let n = nav::visible_items(&m, h, count);
    let sel = node.ctrl().selected_index;
    let enabled = node.paint.is_enabled;
    // Each region's own selected row, resolved independently — at most one is
    // ever `Some`. A region with no selection fades its indicator out WITHOUT
    // moving it, so returning to that region later resumes from the row it was
    // last on rather than flying in from wherever the other region sat.
    let menu_row = (sel >= 0 && (sel as usize) < n).then(|| nav::item_rect(&m, sel));
    let settings_row = (sel == nav::SETTINGS_INDEX)
        .then(|| nav::settings_rect(&m, h))
        .flatten();

    let k_bg = AtlasKey::solid(theme::surface_sunken(), scale);
    let k_tile = AtlasKey::hbar(
        nav::ITEM_H - theme::SPACE_8,
        theme::RADIUS_SM,
        0.0,
        theme::accent_fill(),
        scale,
    );
    let bar_h = theme::SPACE_16;
    let k_bar = AtlasKey::hbar(bar_h, theme::BORDER_W, 0.0, theme::accent(), scale);
    let k_ink = AtlasKey::hbar(
        nav::ITEM_H - theme::SPACE_8,
        theme::RADIUS_SM,
        0.0,
        theme::w(1.0),
        scale,
    );

    let tile_of = |row: &Rect| {
        (
            theme::SPACE_4,
            row.top + theme::SPACE_4,
            (m.width - theme::SPACE_8).max(0.0),
            nav::ITEM_H - theme::SPACE_8,
        )
    };
    let bar_of = |row: &Rect| {
        (
            0.0,
            row.top + (nav::ITEM_H - bar_h) / 2.0,
            theme::BORDER_W * 3.0,
            bar_h,
        )
    };
    // One indicator pair per region. Both FADE, so the handoff between regions
    // is a cross-fade in place; within a region the opacity never changes, so
    // the fade never fires and the tile simply glides row to row.
    let pair = |row: Option<&Rect>, tile_key: AtlasKey, bar_key: AtlasKey| {
        (
            SlotPlan::glide(tile_key, row.map(tile_of), if row.is_some() { dim } else { 0.0 })
                .fading(),
            SlotPlan::glide(bar_key, row.map(bar_of), if row.is_some() { dim } else { 0.0 })
                .fading(),
        )
    };
    let (menu_tile, menu_bar) = pair(menu_row.as_ref(), k_tile.clone(), k_bar.clone());
    let (set_tile, set_bar) = pair(settings_row.as_ref(), k_tile, k_bar);

    let ink = nav_ink_rect(node).filter(|_| enabled);
    // The chrome wash takes the same rounded-square treatment the caption band's
    // buttons do, and the same alpha the row ink does — the two are one hover
    // language, and a pane whose head washed differently from its rows would
    // read as two controls stacked.
    let chrome = nav::chrome_rect(&m, node.ctrl().hot_index).filter(|_| enabled);
    let k_chrome = AtlasKey::hbar(nav::CHROME_H, theme::RADIUS_SM, 0.0, theme::w(1.0), scale);

    PartPlan::new([h, 0.0, 0.0])
        .below(
            nav_slot::BG,
            SlotPlan::glide(k_bg, Some((0.0, 0.0, m.width, h)), dim),
        )
        // A plain stretch, not a nine-grid: a hairline has no corners to
        // preserve, and it rides the same glide as the background so the pane's
        // edge stays one line rather than separating as the pane opens.
        .below(
            nav_slot::DIVIDER,
            AtlasKey::solid(theme::stroke_divider(), scale)
                .glide_at(Some((m.width, 0.0, theme::BORDER_W, h)), dim),
        )
        .below(nav_slot::MENU_TILE, menu_tile)
        .below(nav_slot::MENU_BAR, menu_bar)
        .below(nav_slot::SET_TILE, set_tile)
        .below(nav_slot::SET_BAR, set_bar)
        // The ink SNAPS to the hovered row and fades: a glide would draw a wash
        // sliding down the pane between two rows the pointer never paused on.
        .above(
            nav_slot::INK,
            SlotPlan::snap(k_ink, ink, if ink.is_some() { wash(0.06) * dim } else { 0.0 }).fading(),
        )
        // Snaps for the same reason, one row up: the two chrome buttons sit side
        // by side, and a glide between them would wash the gap they share.
        .above(
            nav_slot::CHROME_INK,
            SlotPlan::snap(
                k_chrome,
                chrome.map(|r| (r.left, r.top, r.width(), r.height())),
                if chrome.is_some() { wash(0.06) * dim } else { 0.0 },
            )
            .fading(),
        )
}

/// The hover ink's box for whatever row a nav pane currently calls hot, in
/// node-local DIPs. `None` when nothing is hovered.
///
/// The one definition the full sync and the hover edge below both read, so an
/// ink placed by a hover and an ink placed by a repaint cannot land differently.
/// A settings row hovers at its own sentinel index, so one sprite serves both it
/// and the menu rows without a second part.
fn nav_ink_rect(node: &Node) -> Option<(f32, f32, f32, f32)> {
    let hot = node.ctrl().hot_index;
    if hot == -1 {
        return None;
    }
    let has_title = node.nav_text.as_ref().is_some_and(|t| t.title.is_some());
    let m = nav::metrics(node.extras(), node.rect.w, has_title);
    let n = nav::visible_items(&m, node.rect.h, node.ctrl().items.len()) as i32;
    let row = if (0..n).contains(&hot) {
        nav::item_rect(&m, hot)
    } else if hot == nav::SETTINGS_INDEX {
        nav::settings_rect(&m, node.rect.h)?
    } else {
        // The two chrome buttons take their own wash, on their own part: theirs
        // is a 40-DIP rounded square at the head of the pane, not a full-width
        // row tile, and one sprite cannot be both.
        return None;
    };
    Some((
        theme::SPACE_4,
        row.top + theme::SPACE_4,
        (m.width - theme::SPACE_8).max(0.0),
        nav::ITEM_H - theme::SPACE_8,
    ))
}

/// The hot row moved while the pointer stayed on the pane: place the ink on the
/// new row, then fade to the target. Place *and* fade, for the reason
/// [`seg_hot_changed`] does both — on hover entry the hot row was recorded
/// before this call, so the ink must land on it rather than fade in wherever it
/// last sat.
pub(crate) fn nav_hot_changed(node: &mut Node) {
    let live = node.paint.is_enabled && node.hovered;
    let row = nav_ink_rect(node).filter(|_| live);
    // Resolved before the borrow below: both reads take the node, the writes
    // take it mutably.
    let has_title = node.nav_text.as_ref().is_some_and(|t| t.title.is_some());
    let m = nav::metrics(node.extras(), node.rect.w, has_title);
    let chrome = nav::chrome_rect(&m, node.ctrl().hot_index)
        .filter(|_| live)
        .map(|r| (r.left, r.top, r.width(), r.height()));
    let dim = dim_of(node);

    let Some(parts) = node.parts.as_mut() else { return };
    if parts.above.len() != nav_slot::N_ABOVE {
        return;
    }
    // BOTH washes are written on every edge, not only the one that gained a
    // target. Moving between a chrome button and a row changes WHICH part is
    // lit, and refreshing only the newly-hot one would leave the other still
    // showing the row the pointer has already left.
    for (slot, rect) in [(nav_slot::INK, row), (nav_slot::CHROME_INK, chrome)] {
        match rect {
            Some((x, y, w, h)) => {
                parts.above[slot].place(x, y, w, h);
                parts.above[slot].fade_to(wash(0.06) * dim);
            }
            None => parts.above[slot].fade_to(0.0),
        }
    }
}

// ── Expander ─────────────────────────────────────────────────────────────────

/// Above: `[header ink]` — the hover/press wash over the header strip only
/// (the body below it stays wash-free). Chevron + header chrome are painted;
/// the chevron flip is a single event-driven repaint.

/// Below: `[header fill, header border]`. Above: `[hover wash, focus ring ×2]`.
///
/// Only the HEADER carries any of it — the expanded content below is ordinary
/// layout, not part of the control's chrome, which is also why the ring rings
/// the header strip rather than the whole node: a ring around an expanded
/// Expander would enclose its content and read as a group box.
pub(crate) fn expander_plan(node: &Node, scale: f32) -> PartPlan {
    let header_h = super::controls::expander_header_h();
    let w = node.rect.w;
    let dim = dim_of(node);
    let r = theme::RADIUS_MD;
    let strip = Some((0.0, 0.0, w, header_h));

    PartPlan::new([w, node.rect.h, 0.0])
        .below(
            0,
            AtlasKey::hbar(header_h, r, 0.0, theme::surface_raised(), scale).snap_at(strip, dim),
        )
        .below(
            1,
            AtlasKey::hbar(header_h, r, theme::BORDER_W, theme::stroke(), scale)
                .snap_at(strip, dim),
        )
        .above(
            0,
            AtlasKey::hbar(header_h, r, 0.0, theme::w(1.0), scale)
                .snap_at(strip, ink_target(node))
                .fading(),
        )
        // The header strip, not the node: a ring around an expanded body would
        // ring content the expander does not own.
        .focus_ring(1, node.focus_ring, w, header_h, r, scale)
}

// ── Progress (bar + ring) ────────────────────────────────────────────────────

/// One indeterminate sweep / revolution, matching the retired tick's
/// `phase += dt · 0.6` advance (a full cycle per `1 / 0.6` seconds).
const PROGRESS_CYCLE_SECS: f32 = 1.0 / 0.6;

/// The bar's lane height, mirroring the retired `paint_progress_bar`.
fn progress_bar_h(node_h: f32) -> f32 {
    node_h.min(6.0).max(4.0)
}

/// Below: `[track, determinate fill, indeterminate sweep segment]`. The
/// surface paints nothing — a value change glides the fill (spring size), and
/// the indeterminate sweep is a forever-looping compositor keyframe animation:
/// the app is fully idle while the bar animates. The node's container carries
/// an InsetClip (set at create) so the sweep slides in/out at the track edges
/// instead of overhanging them.

/// Below: `[track, determinate fill, indeterminate sweep segment]`.
///
/// The sweep is deliberately absent from this plan while it is running: it is a
/// FOREVER animation, not a placement, and a plan describes where a sprite comes
/// to rest. `progress_sweep` owns it, and the slot the plan does not mention is
/// the slot `apply` leaves alone.
pub(crate) fn progress_plan(node: &Node, scale: f32) -> PartPlan {
    let (w, h) = (node.rect.w, node.rect.h);
    let bar_h = progress_bar_h(h);
    let y = h / 2.0 - bar_h / 2.0;
    let dim = dim_of(node);
    let frac = (super::ctrl_value_frac(node) as f32).clamp(0.0, 1.0);
    let k_track = AtlasKey::hbar(bar_h, bar_h / 2.0, 0.0, theme::w(0.08), scale);
    let k_fill = AtlasKey::hbar(bar_h, bar_h / 2.0, 0.0, theme::accent(), scale);

    let plan =
        PartPlan::new([w, h, 0.0]).below(0, SlotPlan::snap(k_track, Some((0.0, y, w, bar_h)), dim));

    if node.ctrl().indeterminate {
        // The sweep owns the lane; the determinate fill hides without moving.
        return plan.below(1, SlotPlan::snap(k_fill, None, 0.0));
    }

    // Floor the fill at one full pill so the nine-grid corners never degenerate
    // at tiny fractions.
    let fw = if frac > 0.0 { (w * frac).max(bar_h) } else { 0.0 };
    // No "was the fraction different last time" shadow: the fill's rect is a
    // pure function of `frac`, and the channel already knows whether it moved —
    // `glide` on an unchanged rect is a no-op.
    plan.below(
        1,
        SlotPlan::glide(
            k_fill.clone(),
            Some((0.0, y, fw.max(0.01), bar_h)),
            if frac > 0.0 { dim } else { 0.0 },
        ),
    )
    .below(2, SlotPlan::snap(k_fill, None, 0.0))
}

/// Arm or retire the indeterminate sweep — a travelling lit segment (one-third
/// width) looping forever on the compositor, so the app is fully idle while the
/// bar animates.
///
/// `relaid` comes from [`apply`]: the loop is anchored to a `place` at the track
/// geometry, so a resize has to re-arm it.
fn progress_sweep(node: &mut Node, relaid: bool) {
    let (w, h) = (node.rect.w, node.rect.h);
    let bar_h = progress_bar_h(h);
    let y = h / 2.0 - bar_h / 2.0;
    let dim = dim_of(node);
    let ind = node.ctrl().indeterminate;
    let Some(parts) = node.parts.as_mut() else { return };
    if parts.below.len() < 3 {
        return;
    }
    if ind {
        let seg_w = w * 0.33;
        if relaid || !parts.looping {
            parts.below[2].place(-seg_w, y, seg_w, bar_h);
            parts.looping = parts.below[2].loop_x(-seg_w, w, PROGRESS_CYCLE_SECS);
        }
        parts.below[2].set_opacity(dim);
    } else if parts.looping {
        parts.below[2].stop_loop_x();
        parts.looping = false;
    }
}

/// The ring has no sprite parts — its track + arc stay painted (drawn once).
/// Indeterminate spin is a forever-looping `RotationAngle` animation on the
/// painted surface sprite itself: the track ring is rotation-invariant, so
/// only the arc appears to revolve, and the app never ticks.
fn ring_sync(comp: &Compositing, node: &mut Node) {
    if !ensure(comp, node, 0, 0) {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let ind = node.ctrl().indeterminate;
    let Some(surf) = node.surf.as_ref() else { return };
    let (Ok(vis), Ok(obj)) = (
        surf.sprite.cast::<IVisual>(),
        surf.sprite.cast::<ICompositionObject>(),
    ) else {
        return;
    };
    let Some(parts) = node.parts.as_mut() else { return };

    if ind {
        if !parts.looping || parts.geom != (w, h) {
            let _ = vis.SetCenterPoint(Vector3::new(w / 2.0, h / 2.0, 0.0));
            let run = || -> Option<()> {
                let c = obj.Compositor().ok()?;
                let lin: CompositionEasingFunction =
                    c.CreateLinearEasingFunction().ok()?.cast().ok()?;
                let a = c.CreateScalarKeyFrameAnimation().ok()?;
                a.InsertKeyFrameWithEasingFunction(0.0, 0.0, &lin).ok()?;
                a.InsertKeyFrameWithEasingFunction(1.0, std::f32::consts::TAU, &lin)
                    .ok()?;
                let kf: IKeyFrameAnimation = a.cast().ok()?;
                kf.SetDuration(ts_secs(PROGRESS_CYCLE_SECS)).ok()?;
                kf.SetIterationBehavior(AnimationIterationBehavior::Forever).ok()?;
                let _ = obj.StopAnimation("RotationAngle");
                obj.StartAnimation("RotationAngle", &a.cast::<CompositionAnimation>().ok()?)
                    .ok()
            };
            parts.looping = run().is_some();
        }
    } else if parts.looping {
        let _ = obj.StopAnimation("RotationAngle");
        let _ = vis.SetRotationAngle(0.0);
        parts.looping = false;
    }
    parts.geom = (w, h);
    parts.init = true;
}

// ─────────────────────────────────────────────────────────────────────────────
// Caret — the focused text editor's blinking insertion bar
// ─────────────────────────────────────────────────────────────────────────────

/// The focused editor's caret: a 1-DIP sprite above the painted text whose
/// blink is an INFINITE square-wave opacity animation evaluated by the system
/// compositor. The app touches it only on input edges (type / caret move /
/// focus / activation) — no timer, no per-blink repaint.
pub(crate) struct Caret {
    sprite: SpriteVisual,
    vis: IVisual,
    obj: ICompositionObject,
    /// The atlas source currently bound + the epoch it came from.
    key: Option<AtlasKey>,
    epoch: u32,
    /// Last placed box (change-gated writes).
    rect: Option<(f32, f32, f32, f32)>,
    /// Whether the sprite is currently shown (blink running or solid).
    shown: bool,
}

impl Caret {
    /// Create the sprite as the TOPMOST child of the editor's container, above
    /// its painted surface.
    fn new(comp: &Compositing, node: &Node) -> Option<Self> {
        let sprite = comp.new_sprite().ok()?;
        let vis: IVisual = sprite.cast().ok()?;
        let obj: ICompositionObject = sprite.cast().ok()?;
        let v: Visual = sprite.cast().ok()?;
        node.container.Children().ok()?.InsertAtTop(&v).ok()?;
        Some(Self { sprite, vis, obj, key: None, epoch: 0, rect: None, shown: false })
    }

    /// Bind (or re-bind) the solid atlas source for `key` (same epoch contract
    /// as [`Part::bind`]).
    fn bind(&mut self, comp: &Compositing, atlas: &mut Atlas, key: AtlasKey) {
        if self.key.as_ref() == Some(&key) && self.epoch == atlas.epoch {
            return;
        }
        let epoch = atlas.epoch;
        let Some(entry) = atlas.entry(comp, &key) else { return };
        if let Ok(b) = entry.brush.cast::<CompositionBrush>()
            && self.sprite.SetBrush(&b).is_ok()
        {
            self.key = Some(key);
            self.epoch = epoch;
        }
    }

    fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.rect != Some((x, y, w, h)) {
            let off = self.vis.SetOffset(Vector3::new(x, y, 0.0));
            let size = self.vis.SetSize(Vector2::new(w, h));
            // Cache only a rect BOTH writes landed (`Channel::wrote`'s rule): a
            // half-applied rect the cache calls current would freeze the caret.
            self.rect = (off.is_ok() && size.is_ok()).then_some((x, y, w, h));
        }
    }

    /// Stop the blink and hide the sprite (blur / window deactivated).
    fn hide(&mut self) {
        if !self.shown {
            return;
        }
        let _ = self.obj.StopAnimation("Opacity");
        // `Channel::wrote`'s evidence rule: a failed hide leaves the caret
        // visible, and recording it as hidden would make every later `hide`
        // return at the guard above — a caret blinking over a field that no
        // longer has focus, for as long as the window lives.
        self.shown = self.vis.SetIsVisible(false).is_err();
    }

    /// (Re)start the blink solid-first — or pin a solid caret when the system
    /// blink is disabled (`GetCaretBlinkTime` of 0 / INFINITE).
    fn start_blink(&mut self, comp: &Compositing) {
        self.shown = self.vis.SetIsVisible(true).is_ok();
        let interval = unsafe { crate::system_bindings::GetCaretBlinkTime() };
        if interval == 0 || interval == u32::MAX || self.blink(comp, interval).is_none() {
            // Blinking disabled (or animation setup failed): a solid caret.
            let _ = self.obj.StopAnimation("Opacity");
            let _ = self.vis.SetOpacity(1.0);
        }
    }

    /// A square wave on Opacity: solid for `interval_ms`, hidden for
    /// `interval_ms`, repeated forever, evaluated entirely on the DWM. That is
    /// the Windows caret exactly — `GetCaretBlinkTime` is the time to *invert*
    /// the caret, i.e. the half period, so the cycle is twice it (530 ms on,
    /// 530 ms off on a default system).
    ///
    /// # Why the level is held by keyframes and not by a step easing
    ///
    /// This used to place two keyframes and let a `CreateStepEasingFunction()`
    /// hold each level between them. It does the opposite: the segment takes
    /// its END value immediately, so `[0, ½)` — the half that is supposed to be
    /// the solid one — was already 0. The only frame that ever showed the caret
    /// was the cycle's first, which is why it read as a single-frame flash once
    /// per second rather than a blink.
    ///
    /// The defaults that decide this (`StepCount`, `InitialStep`, `FinalStep`,
    /// `IsFinalStepSingleFrame`) are not reachable through the generated
    /// bindings, so there is nothing to pin them to. Holding each level with a
    /// keyframe of its own needs none of them: the wave is stated in the values,
    /// the fall is one duration-ratio wide, and the shape cannot be changed by
    /// an interpolation default we do not set.
    fn blink(&self, comp: &Compositing, interval_ms: u32) -> Option<()> {
        let compositor = comp.compositor();
        let a = compositor.CreateScalarKeyFrameAnimation().ok()?;
        let kf: IKeyFrameAnimation = a.cast().ok()?;
        let cycle_s = interval_ms as f32 * 2.0 / 1000.0;
        kf.SetDuration(ts_secs(cycle_s)).ok()?;
        kf.SetIterationBehavior(AnimationIterationBehavior::Forever).ok()?;
        // The edge, as a fraction of the cycle: one composition frame at 120 Hz
        // is ~8 ms, so an eighth of that is far below anything that can be
        // sampled — the fall is a jump in every frame the compositor draws, and
        // stays one even if the blink rate is set to something very fast.
        let edge = (0.001 / cycle_s.max(0.001)).min(0.01);
        a.InsertKeyFrame(0.0, 1.0).ok()?;
        a.InsertKeyFrame(0.5 - edge, 1.0).ok()?;
        a.InsertKeyFrame(0.5, 0.0).ok()?;
        a.InsertKeyFrame(1.0, 0.0).ok()?;
        self.obj
            .StartAnimation("Opacity", &a.cast::<CompositionAnimation>().ok()?)
            .ok()
    }
}

/// Reconcile an editor node's caret sprite against the state just painted:
/// shown while the node is focused (and the window active), placed from the
/// same text metrics the paint used, blink restarted solid-first on caret
/// movement. Rides the repaint choke — every state change that can move the
/// caret already repaints the field.
pub(crate) fn sync_caret(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    let show = node.focused
        && node.paint.is_enabled
        && node.editor.as_ref().is_some_and(|e| e.caret_shown);
    if !show {
        if let Some(c) = &mut node.caret {
            c.hide();
        }
        return;
    }
    let Some(bx) = super::controls::editor_caret_box(node, scale) else { return };
    if node.caret.is_none() {
        node.caret = Caret::new(comp, node);
    }
    let Some(mut caret) = node.caret.take() else { return };
    caret.bind(comp, atlas, AtlasKey::solid(theme::text(), scale));
    caret.place(bx.left, bx.top, bx.width(), bx.height());
    let moved = node.editor.as_ref().is_some_and(|e| e.caret_moved);
    if moved || !caret.shown {
        caret.start_blink(comp);
    }
    node.caret = Some(caret);
    if let Some(e) = node.editor.as_mut() {
        e.caret_moved = false;
    }
}
