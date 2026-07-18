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
    ICompositor2, ICompositor4, IKeyFrameAnimation,
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

/// Spring tuning, matching the retired CPU spring (`node::Spring`: `k = 520`,
/// `c = 40`): natural period `2π/√k`, damping ratio `c / (2√k)`. Shared with
/// the scroll-carrier glide (`Node::scroll_glide`) so scrolling feels the same
/// as it did on the CPU spring.
pub(crate) const SPRING_PERIOD: f32 = 0.2756;
pub(crate) const SPRING_DAMPING: f32 = 0.877;

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
    /// The property token an animation was started on (`"Offset"`, `"Offset.X"`,
    /// …) — the exact token a snap has to `StopAnimation`.
    pub(super) type Prop = &'static str;

    pub(super) struct Channel {
        /// The last target REQUESTED, not where the visual is: while `animated`
        /// is set this is not evidence of anything (see [`super::Part::place`]).
        target: Option<(f32, f32)>,
        /// The property an animation may currently hold.
        animated: Option<Prop>,
    }

    impl Channel {
        pub(super) const fn new() -> Self {
            Self { target: None, animated: None }
        }

        /// Has this channel ever been written? A first write must snap —
        /// mounting must never fly in from the visual's zeroed defaults.
        pub(super) fn placed(&self) -> bool {
            self.target.is_some()
        }

        /// The last requested target, if one is still meaningful.
        pub(super) fn target(&self) -> Option<(f32, f32)> {
            self.target
        }

        /// Begin an authoritative snap: yields the property token the caller
        /// MUST stop (if any) and leaves the channel un-animated. A `Some`
        /// result also means the cached target is stale, so the caller must
        /// write unconditionally.
        #[must_use]
        pub(super) fn begin_snap(&mut self) -> Option<Prop> {
            self.animated.take()
        }

        /// Record a plain property write of `t` (a snap).
        pub(super) fn wrote(&mut self, t: (f32, f32)) {
            self.target = Some(t);
        }

        /// A spring this `Part` owns now drives `prop` toward `t`.
        pub(super) fn animating(&mut self, prop: Prop, t: (f32, f32)) {
            self.target = Some(t);
            self.animated = Some(prop);
        }

        /// Hand `prop` to an OUT-OF-BAND animation whose value this part does
        /// not track — a forever-looping sweep, an expression derivation. Both
        /// consequences follow from this single call, which is the whole point:
        /// the cached target no longer describes the visual (so it must not
        /// suppress a later write) AND a snap must stop `prop`.
        pub(super) fn ceded(&mut self, prop: Prop) {
            self.target = None;
            self.animated = Some(prop);
        }

        /// The caller has ALREADY stopped whatever held the property. Nothing
        /// animates it now, but the value it left behind is unknown, so the next
        /// write must be unconditional.
        pub(super) fn reclaimed(&mut self) {
            self.target = None;
            self.animated = None;
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
        if off_held.is_some() || self.off.target() != Some((x, y)) {
            let _ = self.vis.SetOffset(Vector3::new(x, y, 0.0));
            self.off.wrote((x, y));
        }
        let size_held = self.size.begin_snap();
        if let Some(prop) = size_held {
            let _ = self.obj.StopAnimation(prop);
        }
        if size_held.is_some() || self.size.target() != Some((w, h)) {
            let _ = self.vis.SetSize(Vector2::new(w, h));
            self.size.wrote((w, h));
        }
    }

    /// Spring-glide position + size to a new target. First placement snaps
    /// (mounting must never fly in from the visual's zeroed defaults).
    fn glide(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if !self.off.placed() || !self.size.placed() {
            self.place(x, y, w, h);
            return;
        }
        if self.off.target() != Some((x, y)) {
            if self.glide_offset(x, y).is_some() {
                self.off.animating("Offset", (x, y));
            } else {
                self.place(x, y, w, h);
                return;
            }
        }
        if self.size.target() != Some((w, h)) {
            if self.glide_size(w, h).is_some() {
                self.size.animating("Size", (w, h));
            } else {
                self.place(x, y, w, h);
            }
        }
    }

    fn glide_offset(&mut self, x: f32, y: f32) -> Option<()> {
        if self.s_off.is_none() {
            let c = self.obj.Compositor().ok()?;
            let a = c.cast::<ICompositor4>().ok()?.CreateSpringVector3Animation().ok()?;
            let sa: ISpringVector3NaturalMotionAnimation = a.cast().ok()?;
            sa.SetDampingRatio(SPRING_DAMPING).ok()?;
            sa.SetPeriod(ts_secs(SPRING_PERIOD)).ok()?;
            self.s_off = Some(a);
        }
        let a = self.s_off.as_ref()?;
        a.cast::<IVector3NaturalMotionAnimation>()
            .ok()?
            .SetFinalValue(Some(Vector3::new(x, y, 0.0)))
            .ok()?;
        self.obj
            .StartAnimation("Offset", &a.cast::<CompositionAnimation>().ok()?)
            .ok()
    }

    fn glide_size(&mut self, w: f32, h: f32) -> Option<()> {
        if self.s_size.is_none() {
            let c = self.obj.Compositor().ok()?;
            let a = c.cast::<ICompositor4>().ok()?.CreateSpringVector2Animation().ok()?;
            let sa: ISpringVector2NaturalMotionAnimation = a.cast().ok()?;
            sa.SetDampingRatio(SPRING_DAMPING).ok()?;
            sa.SetPeriod(ts_secs(SPRING_PERIOD)).ok()?;
            self.s_size = Some(a);
        }
        let a = self.s_size.as_ref()?;
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
        let _ = self.vis.SetOpacity(a);
        self.opacity = Some(a);
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
    /// Last glided-to selection (segmented / nav).
    sel: i32,
    /// Last toggle state.
    on: bool,
    /// Last slider fraction.
    frac: f32,
    /// Last node size; a change snaps (resize must not glide).
    geom: (f32, f32),
    /// Segmented: checksum of the segment edges (labels / widths changed).
    edges_sig: f32,
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
            sel: -1,
            on: false,
            frac: 0.0,
            geom: (0.0, 0.0),
            edges_sig: 0.0,
            looping: false,
            clip: None,
            clip_w: 0.0,
            grad_key: None,
            fill_live: std::rc::Rc::new(std::cell::Cell::new(false)),
            fill_gen: std::rc::Rc::new(std::cell::Cell::new(0)),
            _fill_settle: None,
        }
    }

    pub(crate) fn below_visuals(&self) -> impl Iterator<Item = Visual> + '_ {
        self.below.iter().filter_map(Part::visual)
    }
    pub(crate) fn above_visuals(&self) -> impl Iterator<Item = Visual> + '_ {
        self.above.iter().filter_map(Part::visual)
    }
}

/// Kinds whose dynamic chrome is fully part-driven (their springs never enter
/// the frame tick; hover / press / activation retarget compositor springs or
/// repaint once, event-driven). The HyperlinkButton has no parts at all — it
/// is listed so its hover recolor stays a single repaint instead of a tick.
pub(crate) fn converted(kind: ControlKind) -> bool {
    matches!(
        kind,
        ControlKind::Button
            | ControlKind::ToggleButton
            | ControlKind::RepeatButton
            | ControlKind::SplitButton
            | ControlKind::HyperlinkButton
            | ControlKind::ComboBox
            | ControlKind::DropDownButton
            | ControlKind::ToggleSwitch
            | ControlKind::CheckBox
            | ControlKind::Slider
            | ControlKind::SelectorBar
            | ControlKind::NavigationView
            | ControlKind::Expander
            | ControlKind::ProgressBar
            | ControlKind::ProgressRing
            | ControlKind::Meter
    )
}

/// Ensure `node.parts` exists with `n_below`/`n_above` parts, inserted at the
/// correct band positions around the painted surface sprite.
fn ensure(comp: &Compositing, node: &mut Node, n_below: usize, n_above: usize) -> bool {
    if node.parts.is_some() {
        return true;
    }
    let Some(surf) = node.surf.as_ref() else { return false };
    let Ok(surf_vis) = surf.sprite.cast::<Visual>() else { return false };
    let Ok(children) = node.container.Children() else { return false };

    let mut parts = Box::new(Parts::new());
    // Creation order = bottom→top within the band: each `InsertBelow(surface)`
    // lands directly under the surface, pushing earlier parts further down.
    for _ in 0..n_below {
        let Some(p) = Part::new(comp) else { return false };
        let Some(v) = p.visual() else { return false };
        if children.InsertBelow(&v, &surf_vis).is_err() {
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
    match node.kind {
        ControlKind::ToggleSwitch => toggle_sync(comp, atlas, node, scale),
        ControlKind::CheckBox => check_sync(comp, atlas, node, scale),
        ControlKind::Slider => slider_sync(comp, atlas, node, scale, scrubbing),
        ControlKind::Meter => meter_sync(comp, atlas, node, scale, scrubbing),
        ControlKind::SelectorBar => segmented_sync(comp, atlas, node, scale),
        ControlKind::NavigationView => nav_sync(comp, atlas, node, scale),
        ControlKind::Expander => expander_sync(comp, atlas, node, scale),
        ControlKind::ProgressBar => progress_sync(comp, atlas, node, scale),
        ControlKind::ProgressRing => ring_sync(comp, node),
        ControlKind::Button
        | ControlKind::ToggleButton
        | ControlKind::RepeatButton
        | ControlKind::SplitButton
        | ControlKind::ComboBox
        | ControlKind::DropDownButton => ink_sync(comp, atlas, node, scale),
        // HyperlinkButton: painted only (hover recolor is an event repaint).
        _ => {}
    }
}

// ── Hover / press ink (button family + select triggers) ──────────────────────

/// Button-family ink geometry: full node rect at the control's corner radius.
fn ink_radius(node: &Node) -> f32 {
    match node.kind {
        ControlKind::ComboBox | ControlKind::DropDownButton => theme::RADIUS_SM,
        _ => node.paint.corner_radius.max(theme::RADIUS_MD),
    }
}

fn ink_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 0, 1) {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let key = AtlasKey::hbar(h, ink_radius(node), 0.0, theme::w(1.0), scale);
    let target = ink_target(node);
    let Some(parts) = node.parts.as_mut() else { return };
    parts.above[0].bind(comp, atlas, key);
    parts.above[0].place(0.0, 0.0, w, h);
    if parts.init {
        parts.above[0].fade_to(target);
    } else {
        parts.above[0].set_opacity(target);
        parts.init = true;
    }
    parts.geom = (w, h);
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

fn toggle_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 3, 0) {
        return;
    }
    let cy = node.rect.h / 2.0;
    let on = node.ctrl().is_on;
    let dim = dim_of(node);
    let (kx_off, kx_on) = knob_xs();
    let kx = if on { kx_on } else { kx_off };

    let k_on = AtlasKey::hbar(TRACK_H, TRACK_H / 2.0, 0.0, theme::accent(), scale);
    let k_off = AtlasKey::hbar(TRACK_H, TRACK_H / 2.0, 1.5, theme::w(OUTLINE_AUTHORED), scale);
    let k_knob = AtlasKey::circle(KNOB_D, theme::w(1.0), scale);

    let (on_t, off_t) = track_targets(on, node.hovered, dim);
    let geom = (node.rect.w, node.rect.h);
    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != geom;

    parts.below[0].bind(comp, atlas, k_on);
    parts.below[1].bind(comp, atlas, k_off);
    parts.below[2].bind(comp, atlas, k_knob);
    parts.below[0].place(0.0, cy - TRACK_H / 2.0, TRACK_W, TRACK_H);
    parts.below[1].place(0.0, cy - TRACK_H / 2.0, TRACK_W, TRACK_H);

    let ky = cy - KNOB_D / 2.0;
    if snap || parts.on == on {
        parts.below[2].place(kx, ky, KNOB_D, KNOB_D);
        parts.below[0].set_opacity(on_t);
        parts.below[1].set_opacity(off_t);
    } else {
        parts.below[2].glide(kx, ky, KNOB_D, KNOB_D);
        parts.below[0].fade_to(on_t);
        parts.below[1].fade_to(off_t);
    }
    parts.on = on;
    parts.geom = geom;
    parts.init = true;
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
const CHECK_BOX_D: f32 = 18.0;

/// Below: `[accent box fill]` (under the painted stroke + label). Above:
/// `[checkmark]`. A check/uncheck is a pair of compositor fades — endpoint
/// parity with the retired painted crossfade (`transparent→accent` fill,
/// `w(on)` checkmark).
fn check_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 1, 1) {
        return;
    }
    let on = node.ctrl().is_checked;
    let t = if on { dim_of(node) } else { 0.0 };
    let y = node.rect.h / 2.0 - CHECK_BOX_D / 2.0;
    let k_fill = AtlasKey::hbar(CHECK_BOX_D, theme::RADIUS_SM, 0.0, theme::accent(), scale);
    let k_check = AtlasKey::check(CHECK_BOX_D, theme::w(1.0), scale);
    let geom = (node.rect.w, node.rect.h);
    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != geom;

    parts.below[0].bind(comp, atlas, k_fill);
    parts.above[0].bind(comp, atlas, k_check);
    parts.below[0].place(0.0, y, CHECK_BOX_D, CHECK_BOX_D);
    parts.above[0].place(0.0, y, CHECK_BOX_D, CHECK_BOX_D);
    if snap || parts.on == on {
        parts.below[0].set_opacity(t);
        parts.above[0].set_opacity(t);
    } else {
        parts.below[0].fade_to(t);
        parts.above[0].fade_to(t);
    }
    parts.on = on;
    parts.geom = geom;
    parts.init = true;
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
    parts.frac = frac;
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
    // through `Part::glide` and so shares `SPRING_PERIOD` / `SPRING_DAMPING` —
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
    // `Part::glide` is a spring built with the one shared `SPRING_PERIOD` /
    // `SPRING_DAMPING` pair: same tuning, both retargeted in the same tick, so
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
    parts.frac = frac;
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

    parts.frac = frac;
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
fn segmented_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 4, 0) {
        return;
    }
    let n = node.ctrl().items.len();
    let (w, h) = (node.rect.w, node.rect.h);
    let accent = node.paint.style_variant == 1;
    let m = super::controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
    let edges = super::controls::segment_edges(node);
    let edges_sig = edges.iter().sum::<f32>() + edges.len() as f32;
    let dim = dim_of(node);

    let tray_radius = if accent { h / 2.0 } else { theme::RADIUS_SM };
    let tray_bg = if accent { theme::w(0.06) } else { theme::stroke_subtle() };
    let pill_h = (h - 2.0 * m.tray).max(0.0);
    let seg_radius = if accent { pill_h / 2.0 } else { theme::RADIUS_BADGE };
    let pill_fill = if accent { theme::accent() } else { theme::stroke() };

    let k_tray = AtlasKey::hbar(h, tray_radius, 0.0, tray_bg, scale);
    let k_stroke = AtlasKey::hbar(h, tray_radius, theme::BORDER_W, theme::stroke(), scale);
    let k_pill = AtlasKey::hbar(pill_h, seg_radius, 0.0, pill_fill, scale);
    let k_ink = AtlasKey::hbar(pill_h, seg_radius, 0.0, theme::w(1.0), scale);

    let sel = if n == 0 { -1 } else { (node.ctrl().selected_index.max(0)).min(n as i32 - 1) };
    let seg_rect = |i: i32| -> Option<(f32, f32, f32, f32)> {
        let i = usize::try_from(i).ok()?;
        let (a, b) = (*edges.get(i)?, *edges.get(i + 1)?);
        Some((a, m.tray, b - a, pill_h))
    };
    let pill = seg_rect(sel);
    let hot = node.ctrl().hot_index;
    let ink = seg_rect(hot);
    let ink_t = seg_ink_target(node);

    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != (w, h) || parts.edges_sig != edges_sig;

    parts.below[0].bind(comp, atlas, k_tray);
    parts.below[1].bind(comp, atlas, k_stroke);
    parts.below[2].bind(comp, atlas, k_pill);
    parts.below[3].bind(comp, atlas, k_ink);

    parts.below[0].place(0.0, 0.0, w, h);
    parts.below[1].place(0.0, 0.0, w, h);
    parts.below[0].set_opacity(dim);
    parts.below[1].set_opacity(dim);

    match pill {
        Some(r) => {
            if snap || parts.sel == sel {
                parts.below[2].place(r.0, r.1, r.2, r.3);
            } else {
                parts.below[2].glide(r.0, r.1, r.2, r.3);
            }
            parts.below[2].set_opacity(dim);
        }
        None => parts.below[2].set_opacity(0.0),
    }
    if let Some(r) = ink {
        parts.below[3].place(r.0, r.1, r.2, r.3);
    }
    if parts.init {
        parts.below[3].fade_to(ink_t);
    } else {
        parts.below[3].set_opacity(ink_t);
    }

    parts.sel = sel;
    parts.geom = (w, h);
    parts.edges_sig = edges_sig;
    parts.init = true;
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

/// Below-band roles: `[pane background, active tile, accent bar]`; above:
/// `[row hover ink]`.
///
/// Three things move here and all three move on the compositor: the pane's
/// WIDTH when it opens or closes, the selection tile and its accent bar when
/// the selected page changes, and the hover ink as the pointer crosses rows.
/// The pane's painted layer (glyphs, labels, divider) snaps to the new width in
/// the same repaint that starts the glide — the geometry is retained chrome,
/// the text is not, and a text layout cannot be interpolated.
fn nav_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 3, 1) {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let dim = dim_of(node);
    let count = node.ctrl().items.len();
    let has_title = node.nav_text.as_ref().is_some_and(|t| t.title.is_some());
    let m = nav::metrics(node.extras(), w, has_title);
    let n = nav::visible_items(&m, h, count);
    let sel = node.ctrl().selected_index;
    let enabled = node.paint.is_enabled;
    // The selected row's box: a visible menu row, or the settings row when the
    // selection sits at its sentinel slot. `None` (no selection, or a selected
    // row that no longer fits) fades the tile and bar out.
    let sel_row = if sel == nav::SETTINGS_INDEX {
        nav::settings_rect(&m, h)
    } else if sel >= 0 && (sel as usize) < n {
        Some(nav::item_rect(&m, sel))
    } else {
        None
    };
    let visible = sel_row.is_some();

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

    let row = sel_row.unwrap_or_else(|| nav::item_rect(&m, 0));
    let tile = (
        theme::SPACE_4,
        row.top + theme::SPACE_4,
        (m.width - theme::SPACE_8).max(0.0),
        nav::ITEM_H - theme::SPACE_8,
    );
    let bar = (
        0.0,
        row.top + (nav::ITEM_H - bar_h) / 2.0,
        theme::BORDER_W * 3.0,
        bar_h,
    );

    let ink_row = nav_ink_rect(node);

    let Some(parts) = node.parts.as_mut() else { return };
    // A HEIGHT change snaps (a resize must not play as motion); a WIDTH change
    // is exactly the pane opening or closing, so it glides.
    let snap = !parts.init || parts.geom.1 != h;

    parts.below[0].bind(comp, atlas, k_bg);
    parts.below[1].bind(comp, atlas, k_tile);
    parts.below[2].bind(comp, atlas, k_bar);
    parts.above[0].bind(comp, atlas, k_ink);

    if snap {
        parts.below[0].place(0.0, 0.0, m.width, h);
    } else {
        parts.below[0].glide(0.0, 0.0, m.width, h);
    }
    parts.below[0].set_opacity(dim);

    if visible {
        if snap || parts.sel == sel {
            parts.below[1].place(tile.0, tile.1, tile.2, tile.3);
            parts.below[2].place(bar.0, bar.1, bar.2, bar.3);
        } else {
            parts.below[1].glide(tile.0, tile.1, tile.2, tile.3);
            parts.below[2].glide(bar.0, bar.1, bar.2, bar.3);
        }
        parts.below[1].set_opacity(dim);
        parts.below[2].set_opacity(dim);
    } else {
        parts.below[1].set_opacity(0.0);
        parts.below[2].set_opacity(0.0);
    }

    match ink_row.filter(|_| enabled) {
        Some(r) => {
            // Snap the ink to the newly hovered row, then fade it in: a glide
            // would draw a wash sliding down the pane between two rows the
            // pointer never paused on.
            parts.above[0].place(r.0, r.1, r.2, r.3);
            if parts.init {
                parts.above[0].fade_to(wash(0.06) * dim);
            } else {
                parts.above[0].set_opacity(wash(0.06) * dim);
            }
        }
        None if parts.init => parts.above[0].fade_to(0.0),
        None => parts.above[0].set_opacity(0.0),
    }

    parts.sel = sel;
    parts.geom = (m.width, h);
    parts.init = true;
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
        // The two chrome buttons wash on the node's own surface (a flat state
        // fill, like the caption band's back button), not through this sprite.
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
    let rect = nav_ink_rect(node).filter(|_| node.paint.is_enabled && node.hovered);
    let dim = dim_of(node);
    let Some(parts) = node.parts.as_mut() else { return };
    if parts.above.is_empty() {
        return;
    }
    match rect {
        Some((x, y, w, h)) => {
            parts.above[0].place(x, y, w, h);
            parts.above[0].fade_to(wash(0.06) * dim);
        }
        None => parts.above[0].fade_to(0.0),
    }
}

// ── Expander ─────────────────────────────────────────────────────────────────

/// Above: `[header ink]` — the hover/press wash over the header strip only
/// (the body below it stays wash-free). Chevron + header chrome are painted;
/// the chevron flip is a single event-driven repaint.
fn expander_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 0, 1) {
        return;
    }
    let header_h = theme::ROW_H + theme::SPACE_8;
    let w = node.rect.w;
    let key = AtlasKey::hbar(header_h, theme::RADIUS_MD, 0.0, theme::w(1.0), scale);
    let target = ink_target(node);
    let Some(parts) = node.parts.as_mut() else { return };
    parts.above[0].bind(comp, atlas, key);
    parts.above[0].place(0.0, 0.0, w, header_h);
    if parts.init {
        parts.above[0].fade_to(target);
    } else {
        parts.above[0].set_opacity(target);
        parts.init = true;
    }
    parts.geom = (w, node.rect.h);
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
fn progress_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 3, 0) {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let bar_h = progress_bar_h(h);
    let y = h / 2.0 - bar_h / 2.0;
    let dim = dim_of(node);
    let frac = (super::ctrl_value_frac(node) as f32).clamp(0.0, 1.0);
    let ind = node.ctrl().indeterminate;
    let k_track = AtlasKey::hbar(bar_h, bar_h / 2.0, 0.0, theme::w(0.08), scale);
    let k_fill = AtlasKey::hbar(bar_h, bar_h / 2.0, 0.0, theme::accent(), scale);

    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != (w, h);

    parts.below[0].bind(comp, atlas, k_track);
    parts.below[1].bind(comp, atlas, k_fill.clone());
    parts.below[2].bind(comp, atlas, k_fill);
    parts.below[0].place(0.0, y, w, bar_h);
    parts.below[0].set_opacity(dim);

    if ind {
        parts.below[1].set_opacity(0.0);
        // A travelling lit segment (one-third width), sweeping forever.
        let seg_w = w * 0.33;
        if snap || !parts.looping {
            parts.below[2].place(-seg_w, y, seg_w, bar_h);
            parts.looping = parts.below[2].loop_x(-seg_w, w, PROGRESS_CYCLE_SECS);
        }
        parts.below[2].set_opacity(dim);
    } else {
        if parts.looping {
            parts.below[2].stop_loop_x();
            parts.looping = false;
        }
        parts.below[2].set_opacity(0.0);
        // Floor the fill at one full pill so the nine-grid corners never
        // degenerate at tiny fractions.
        let fw = if frac > 0.0 { (w * frac).max(bar_h) } else { 0.0 };
        if snap || parts.frac == frac {
            parts.below[1].place(0.0, y, fw.max(0.01), bar_h);
        } else {
            parts.below[1].glide(0.0, y, fw.max(0.01), bar_h);
        }
        parts.below[1].set_opacity(if frac > 0.0 { dim } else { 0.0 });
        parts.frac = frac;
    }
    parts.geom = (w, h);
    parts.init = true;
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
            let _ = self.vis.SetOffset(Vector3::new(x, y, 0.0));
            let _ = self.vis.SetSize(Vector2::new(w, h));
            self.rect = Some((x, y, w, h));
        }
    }

    /// Stop the blink and hide the sprite (blur / window deactivated).
    fn hide(&mut self) {
        if !self.shown {
            return;
        }
        let _ = self.obj.StopAnimation("Opacity");
        let _ = self.vis.SetIsVisible(false);
        self.shown = false;
    }

    /// (Re)start the blink solid-first — or pin a solid caret when the system
    /// blink is disabled (`GetCaretBlinkTime` of 0 / INFINITE).
    fn start_blink(&mut self, comp: &Compositing) {
        let _ = self.vis.SetIsVisible(true);
        self.shown = true;
        let interval = unsafe { crate::system_bindings::GetCaretBlinkTime() };
        if interval == 0 || interval == u32::MAX || self.blink(comp, interval).is_none() {
            // Blinking disabled (or animation setup failed): a solid caret.
            let _ = self.obj.StopAnimation("Opacity");
            let _ = self.vis.SetOpacity(1.0);
        }
    }

    /// A square wave on Opacity: solid for `interval_ms`, hidden for
    /// `interval_ms`, repeated forever — steps(1) easing holds each level and
    /// jumps at the segment boundary. Runs entirely on the DWM.
    fn blink(&self, comp: &Compositing, interval_ms: u32) -> Option<()> {
        let compositor = comp.compositor();
        let a = compositor.CreateScalarKeyFrameAnimation().ok()?;
        let kf: IKeyFrameAnimation = a.cast().ok()?;
        kf.SetDuration(ts_secs(interval_ms as f32 * 2.0 / 1000.0)).ok()?;
        kf.SetIterationBehavior(AnimationIterationBehavior::Forever).ok()?;
        let step: CompositionEasingFunction = compositor
            .cast::<ICompositor2>()
            .ok()?
            .CreateStepEasingFunction()
            .ok()?
            .cast()
            .ok()?;
        a.InsertKeyFrame(0.0, 1.0).ok()?;
        a.InsertKeyFrameWithEasingFunction(0.5, 0.0, &step).ok()?;
        a.InsertKeyFrameWithEasingFunction(1.0, 0.0, &step).ok()?;
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
    let Some(bx) = super::controls::editor_caret_box(node) else { return };
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
