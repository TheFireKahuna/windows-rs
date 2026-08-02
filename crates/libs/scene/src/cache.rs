//! `Cache<K: Cell>`: the second of the two storage machines. **Front half.**
//!
//! Everything this crate rasterizes lands in one of exactly two machines, and which one is
//! decided by a single question: **is the key an identity the model minted, or a value
//! derived from something unbounded?**
//!
//! | | keyed by | lifetime | hashing | eviction | holds |
//! |---|---|---|---|---|---|
//! | `Res<T>` | an id the model minted | refcount from sprites, exact | none | none | geometry · ramp strips · run tiles · region surfaces |
//! | `Cache<K>` | a derived, unbounded value | LRU | yes | LRU | box atlas cells · solid colour cells |
//!
//! Only the two families here have keys a drag-resize or an animated fill could mint one
//! surface per frame from, which is why those two are the ones quantized.
//!
//! What the generic saves is the **rasterize step**, not the map: eviction, generations,
//! surface allocation, the draw bracket and the device-loss arm are written once, and each
//! family is then its key, its extent and its draw.

use crate::quant::{Q, extent_px, snap_detail};
use crate::sink::Corners;
use rustc_hash::FxHashMap;
use windows_color::Scrgb;
use windows_composition::{CompositionDrawingSurface, CompositionSurfaceBrush, Stretch};
use windows_core::Result;
use windows_d2d::{Draw, Gpu, Opacity, Rect, SceneSurface, Solid, SurfaceDraw};

/// What invalidates a rasterized cell.
///
/// Three counters and not one epoch, because their inputs are independent: a single epoch
/// bumped on device loss, a DPI change, a display-capability change **or** a theme flip
/// throws away every glyph tile in the application because the accent colour moved.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Gen {
    /// Device loss. Takes everything, as it must.
    pub device: u32,
    /// A DPI change: every snapped dimension is different, so geometry and text
    /// re-rasterize and colour is untouched.
    pub dpi: u32,
    /// A display-capability change or a theme flip: the output transform moved, so every
    /// colour cell is wrong and no glyph tile is.
    pub color: u32,
}

/// Which generations a family reads.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GenMask {
    device: bool,
    dpi: bool,
    color: bool,
}

impl GenMask {
    /// Geometry and coverage: rasterized shapes whose colour comes from elsewhere.
    pub const GEOMETRY: Self = Self {
        device: true,
        dpi: true,
        color: false,
    };
    /// Light: cells whose whole content is a colour that has been through the output
    /// transform.
    pub const LIGHT: Self = Self {
        device: true,
        dpi: false,
        color: true,
    };
    /// Nothing: content whose realized object survives every one of them.
    ///
    /// Not a degenerate case — it is what a *shared* resource reads, because re-rasterizing
    /// one moves the brush every sprite already holds instead of asking them to rebuild.
    pub const NONE: Self = Self {
        device: false,
        dpi: false,
        color: false,
    };

    /// Everything either side reads. A sprite's chain is a mask *and* a paint, so its
    /// freshness is theirs together.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            device: self.device || other.device,
            dpi: self.dpi || other.dpi,
            color: self.color || other.color,
        }
    }

    /// Whether an entry built at `built` is still good under `now`.
    #[must_use]
    pub fn fresh(self, built: Gen, now: Gen) -> bool {
        (!self.device || built.device == now.device)
            && (!self.dpi || built.dpi == now.dpi)
            && (!self.color || built.color == now.color)
    }
}

/// One rasterizable family.
///
/// Device loss then needs **no per-kind recovery code at all**: every brush in the system
/// is a pure function of a key or of a resource id, so recovery is "invalidate, then rebind
/// from the key", travelling the same path as the first bind. That property is the reason
/// this trait exists.
pub trait Cell: Clone + Eq + core::hash::Hash {
    /// Which invalidation generations this family reads.
    const DEPS: GenMask;
    /// Whether this family carries coverage rather than colour.
    ///
    /// A coverage cell is allocated one byte a pixel, at an eighth of the memory. A colour
    /// family declared as coverage loses its colour outright.
    const COVERAGE: bool;

    /// The device resources this family draws with — **built once per device, never per
    /// cell**.
    ///
    /// `Draw` deliberately carries no device: `Pass` holds one and `Draw` does not, and the
    /// surface bridge hands a draw callback the target alone. That boundary is the drawing
    /// crate's, and it says in its own words what its constructors are for — a flat-colour
    /// brush is to be *retinted* rather than rebuilt, a stroke style is "reusable". So the
    /// resources arrive from outside, and the only thing built inside a draw is content
    /// genuinely particular to one key.
    type Res;

    /// Builds this family's resources. Once when the cache is first missed, and again after
    /// device loss — which is the same path, because loss clears the cache.
    fn resources(gpu: &Gpu) -> Result<Self::Res>;

    /// Pixel extent. Already snapped — the key's constructor did it.
    fn px(&self) -> (u32, u32);
    /// What the content does with alpha, which decides the surface's alpha mode. Coverage
    /// is always translucent: an opaque mask is not a mask.
    fn opacity(&self) -> Opacity;
    /// Paints at `(0, 0)` in DIPs.
    fn draw(&self, d: &Draw<'_>, res: &Self::Res) -> Result<()>;
}

struct Entry {
    #[expect(
        dead_code,
        reason = "the brush holds the surface; the surface is kept so a resize can reuse it"
    )]
    surface: CompositionDrawingSurface,
    brush: CompositionSurfaceBrush,
    built_at: Gen,
    /// When this entry was last reached, against the cache's own monotonic counter.
    used: u64,
}

/// A cache of rasterized cells, evicting the least recently used.
///
/// **A hit is one map lookup and one store** — no list to walk, no element to shift. The
/// a lookup happens on every bind, and a recency deque costs a linear search plus a shift
/// each time. Eviction is a backstop against a key that turned out unbounded, not a working
/// mechanism, so the cost belongs there.
pub struct Cache<K: Cell> {
    map: FxHashMap<K, Entry>,
    res: Option<K::Res>,
    /// Monotonic, incremented per lookup. Wrapping is not a concern: at one lookup per
    /// nanosecond this takes five centuries to reach the end.
    clock: u64,
    cap: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
}

/// The evicting caches, together. Everything keyed by an identity the model owns lives in
/// [`Resources`](crate::res::Resources) instead.
#[derive(Default)]
pub(crate) struct Cells {
    pub(crate) boxes: Cache<BoxKey>,
    pub(crate) solids: Cache<SolidKey>,
}

impl Cells {
    /// Device loss takes both, which is the whole of its per-cell recovery.
    pub(crate) fn clear(&mut self) {
        self.boxes.clear();
        self.solids.clear();
    }
}

/// How many cells of one family are kept. Both families are small — a handful of corner
/// profiles and a palette's worth of colours — so the cap is a backstop against a key that
/// turns out to be less bounded than it looked, not a working limit.
pub const CACHE_CAP: usize = 256;

impl<K: Cell> Default for Cache<K> {
    fn default() -> Self {
        Self::new(CACHE_CAP)
    }
}

impl<K: Cell> Cache<K> {
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            map: FxHashMap::default(),
            res: None,
            clock: 0,
            cap: cap.max(1),
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    /// The brush for `key`, rasterizing it if this is the first time or if it went stale.
    ///
    /// `Ok(None)` means the device was lost while drawing: the caller drops its binding
    /// and rebinds after recovery, which is the same path as a first bind.
    pub(crate) fn brush(
        &mut self,
        back: &crate::backends::Backends,
        env: crate::Env,
        now: Gen,
        key: &K,
    ) -> Result<Option<&CompositionSurfaceBrush>> {
        self.clock += 1;
        let stamp = self.clock;
        if let Some(entry) = self.map.get_mut(key)
            && K::DEPS.fresh(entry.built_at, now)
        {
            entry.used = stamp;
            self.hits += 1;
            // Re-borrowed rather than returned from the branch above: the mutable borrow
            // that stamped it cannot also be handed out as a shared one.
            return Ok(self.map.get(key).map(|entry| &entry.brush));
        }

        self.misses += 1;
        if self.res.is_none() {
            self.res = Some(K::resources(&back.gpu)?);
        }
        let res = self.res.as_ref().expect("built on the line above");
        let (w, h) = key.px();
        let opacity = key.opacity();
        let px = (w as i32, h as i32);
        let surface = if K::COVERAGE {
            back.mask_surface(px)?
        } else {
            back.graphics().color(px, opacity)?
        };
        // One `BeginDraw` per graphics device at a time — a concurrent second fails
        // outright. Here that is a consequence of ownership rather than a rule to keep:
        // every rasterization in this crate happens behind one `&mut Scene` on one thread,
        // and a presentation region uses its own device entirely.
        // A cell that fails to draw leaves the surface published as whatever it managed,
        // which for a mask is a missing shape rather than a wrong one. Raising here would
        // fail a frame over one cell; the error is carried out so the caller sees it.
        let mut drawn = Ok(());
        if !surface.draw(env.dpi(), opacity, |d| drawn = key.draw(d, res))? {
            return Ok(None);
        }
        drawn?;
        let brush = back.brush(&surface, Stretch::Fill);

        self.evict_for_one();
        self.map.insert(
            key.clone(),
            Entry {
                surface,
                brush,
                built_at: now,
                used: stamp,
            },
        );
        Ok(self.map.get(key).map(|entry| &entry.brush))
    }

    /// Drops every entry. What device loss does, and what nothing else does — a stale
    /// entry is re-rasterized in place on next use rather than swept.
    pub fn clear(&mut self) {
        self.map.clear();
        self.res = None;
    }

    /// Hits, misses and evictions so far.
    #[must_use]
    pub fn counts(&self) -> (u64, u64, u64) {
        (self.hits, self.misses, self.evictions)
    }

    /// How many cells are live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Makes room for one entry, oldest first.
    ///
    /// Linear, and that is the trade: it runs only when the cache is full, which for either
    /// family here means a key turned out unbounded and something else is already wrong.
    fn evict_for_one(&mut self) {
        while self.map.len() >= self.cap {
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
}

/// A rounded-box coverage cell, consumed through a nine-grid brush.
///
/// Opaque white in the requested corner profile, at exactly one atlas cell's size — the
/// nine-grid is what makes one raster serve any width and height with pristine corners, so
/// the key carries the *profile* and not the box.
///
/// The fields are private and the only constructor snaps, which is what makes "every key is
/// snapped" unrepresentable to violate rather than a rule to lint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoxKey {
    /// Quarter-pixel radii, in the order the corners are drawn.
    radius: [i32; 4],
    /// The cell's own pixel extent: large enough for the corner profile plus a pixel of
    /// margin on each side, which is what the nine-grid's insets stretch from.
    px: (u32, u32),
    inset: u32,
}

impl BoxKey {
    /// The cell for a corner profile at `scale`.
    #[must_use]
    pub fn new(radius: Corners, scale: f32) -> Self {
        let quarter = |r: f32| (snap_detail(r, scale) * scale * 4.0).round() as i32;
        // A pixel of margin past the largest radius, so the nine-grid's centre is a whole
        // pixel of flat interior and the stretch has something to stretch.
        let inset = extent_px(radius.max(), scale) + 1;
        let side = inset * 2 + 1;
        Self {
            radius: [
                quarter(radius.tl),
                quarter(radius.tr),
                quarter(radius.br),
                quarter(radius.bl),
            ],
            px: (side, side),
            inset,
        }
    }

    /// The nine-grid's inset on each edge, in physical pixels.
    #[must_use]
    pub fn inset_px(&self) -> f32 {
        self.inset as f32
    }

    fn corners(&self, scale: f32) -> [f32; 4] {
        self.radius.map(|q| q as f32 / (4.0 * scale))
    }
}

impl Cell for BoxKey {
    const DEPS: GenMask = GenMask::GEOMETRY;
    const COVERAGE: bool = true;
    type Res = BoxRes;

    fn resources(gpu: &Gpu) -> Result<Self::Res> {
        Ok(BoxRes {
            white: gpu.solid(WHITE)?,
            gpu: gpu.clone(),
        })
    }

    fn px(&self) -> (u32, u32) {
        self.px
    }

    fn opacity(&self) -> Opacity {
        // Coverage: the corners are not covered, so the cell carries real alpha.
        Opacity::Translucent
    }

    fn draw(&self, d: &Draw<'_>, res: &BoxRes) -> Result<()> {
        d.clear(Scrgb::TRANSPARENT);
        let scale = d.scale();
        let [tl, tr, br, bl] = self.corners(scale);
        let (w, h) = (self.px.0 as f32 / scale, self.px.1 as f32 / scale);
        let box_ = Rect::new(0.0, 0.0, w, h);

        if tl == tr && tr == br && br == bl {
            // The analytic form, which Direct2D rasterizes by the pixels it touches rather
            // than by tessellating a mesh — and it is the overwhelmingly common profile, so
            // it is worth the branch. Filling rather than clipping to the shape is also one
            // primitive and one coverage computation instead of two antialiased edges
            // meeting at the same boundary.
            d.fill(box_.rounded(tl), &res.white);
            return Ok(());
        }
        // Four independent radii cannot be expressed analytically, so this one profile
        // needs a path, particular to the key. The drawing crate's rule is "created once,
        // reused every *frame*", and a cache miss is not a frame.
        let path = res.gpu.path(|sink| {
            sink.rounded_box(box_, [tl, tr, br, bl]);
            Ok(())
        })?;
        d.fill(&path, &res.white);
        Ok(())
    }
}

/// What a box cell draws with.
///
/// The brush is **opaque white, built once**: a coverage cell is a mask and white is the
/// multiplicative identity, so it is never retinted and one instance serves every box cell
/// for the life of the device.
///
/// The device is held here deliberately. A `Draw` exposes no route back to a `Gpu` — the
/// drawing crate means resources to be built up front and passed in, and holding one in the
/// bundle the cache owns *is* building it up front.
pub struct BoxRes {
    white: Solid,
    gpu: Gpu,
}

/// Opaque white. The only colour a coverage cell ever draws in.
const WHITE: Scrgb = Scrgb {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// A flat colour cell — **the retained path's draw choke**, and the one place in this crate
/// a scene-referred value becomes a display-referred one.
///
/// Four by four rather than one by one, purely for filtering margin: a brush stretched from
/// a single texel samples its own edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SolidKey(Q);

impl SolidKey {
    /// Quantizes an already-transformed colour.
    ///
    /// It takes an `Scrgb` and not a `Radiance` deliberately: the transform is applied by
    /// the caller that knows the display, and quantizing the *post*-transform value is why
    /// a display-capability change bumps the colour generation — the same authored light
    /// produces a different cell on a different display.
    #[must_use]
    pub fn new(color: Scrgb) -> Self {
        Self(Q::new(color))
    }
}

impl Cell for SolidKey {
    const DEPS: GenMask = GenMask::LIGHT;
    /// **The draw choke.** This is the one place authored light becomes a display-referred
    /// value, so it is the last thing in the crate that could afford to lose its colour.
    const COVERAGE: bool = false;
    /// None at all: the whole cell is one colour, and clearing takes a colour rather than a
    /// brush. A family that needs no device resource says so in its type.
    type Res = ();

    fn resources(_gpu: &Gpu) -> Result<Self::Res> {
        Ok(())
    }

    fn px(&self) -> (u32, u32) {
        (4, 4)
    }

    fn opacity(&self) -> Opacity {
        if self.0.is_opaque() {
            Opacity::Opaque
        } else {
            Opacity::Translucent
        }
    }

    fn draw(&self, d: &Draw<'_>, _res: &()) -> Result<()> {
        // The dequantized value, and there is no other in scope: the key's field is private
        // and `Q` has no way to hand back what it was given. So "always round-trip the key
        // before drawing" is not a rule — the un-round-tripped value does not exist here.
        d.clear(self.0.dequant());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generation_only_invalidates_what_reads_it() {
        let built = Gen::default();
        let theme = Gen {
            color: 1,
            ..Gen::default()
        };
        let dpi = Gen {
            dpi: 1,
            ..Gen::default()
        };
        let lost = Gen {
            device: 1,
            ..Gen::default()
        };

        // A theme flip costs a handful of colour cells and no coverage at all.
        assert!(GenMask::GEOMETRY.fresh(built, theme));
        assert!(!GenMask::LIGHT.fresh(built, theme));
        // A DPI change is the other way round.
        assert!(!GenMask::GEOMETRY.fresh(built, dpi));
        assert!(GenMask::LIGHT.fresh(built, dpi));
        // Device loss takes everything, as it must.
        assert!(!GenMask::GEOMETRY.fresh(built, lost));
        assert!(!GenMask::LIGHT.fresh(built, lost));
    }

    #[test]
    fn a_box_key_is_snapped_by_construction_at_every_scale() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            let a = BoxKey::new(Corners::all(6.0), scale);
            let b = BoxKey::new(Corners::all(6.0 + 0.01), scale);
            assert_eq!(a, b, "a hundredth of a DIP forked the cache at {scale}x");
            let (w, h) = a.px();
            assert!(w > 0 && h > 0);
        }
    }

    #[test]
    fn a_solid_key_is_the_quantized_colour_and_nothing_else() {
        let a = SolidKey::new(Scrgb {
            r: 0.5,
            g: 0.25,
            b: 0.125,
            a: 1.0,
        });
        let b = SolidKey::new(Scrgb {
            r: 0.5 + 1.0e-7,
            g: 0.25,
            b: 0.125,
            a: 1.0,
        });
        assert_eq!(a, b, "float noise forked the colour cache");
        assert_eq!(a.opacity(), Opacity::Opaque);
    }

    #[test]
    fn an_above_white_colour_still_keys_and_still_survives_the_round_trip() {
        let key = SolidKey::new(Scrgb {
            r: 12.0,
            g: -0.4,
            b: 1.0,
            a: 0.5,
        });
        let back = key.0.dequant();
        assert!(
            back.r > 11.9,
            "an above-white channel was crushed to {}",
            back.r
        );
        assert!(
            back.g < 0.0,
            "a wide-gamut channel was crushed to {}",
            back.g
        );
        assert_eq!(key.opacity(), Opacity::Translucent);
    }
}
