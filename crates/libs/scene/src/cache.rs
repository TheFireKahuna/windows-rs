//! `Cache<K: Cell>`: rasterized cells keyed by value, evicted least-recently-used.
//!
//! Rasterized content is stored one of two ways. Content keyed by an identity the model
//! minted lives in [`Resources`](crate::res::Resources), refcounted by the sprites holding
//! it and never evicted. Content keyed by a derived value — a corner profile, a colour —
//! lives here, because a drag-resize or an animated fill can mint one key per frame; those
//! two families are the ones quantized, so the key population stays bounded.
//!
//! [`Cache`] owns eviction, generation checking, surface allocation, the draw bracket and
//! the device-loss arm. A family supplies its key, its extent and its draw through [`Cell`].

use crate::quant::{Q, extent_px, snap_detail};
use crate::sink::Corners;
use rustc_hash::FxHashMap;
use windows_color::Scrgb;
use windows_composition::{CompositionDrawingSurface, CompositionSurfaceBrush, Stretch};
use windows_core::Result;
use windows_d2d::{Draw, Gpu, Opacity, Rect, SceneSurface, Solid, SurfaceDraw};

/// Counts the invalidations a rasterized cell can be built against.
///
/// Three independent counters rather than one epoch, so a cell is thrown away only by the
/// events it depends on: a theme flip leaves every glyph tile standing.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Gen {
    /// Device loss, which invalidates every cell.
    pub device: u32,
    /// A DPI change: every snapped dimension moves, so geometry and text re-rasterize and
    /// colour does not.
    pub dpi: u32,
    /// A display-capability change or a theme flip: the output transform moved, so colour
    /// cells are stale and coverage cells are not.
    pub color: u32,
}

/// Selects the generations a family's freshness depends on.
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
    /// Colour: cells whose whole content is a colour already through the output transform.
    pub const LIGHT: Self = Self {
        device: true,
        dpi: false,
        color: true,
    };
    /// Nothing: content whose realized object survives all three, so a sprite holding it
    /// never rebinds. What a shared resource reads.
    pub const NONE: Self = Self {
        device: false,
        dpi: false,
        color: false,
    };

    /// Returns a mask reading every generation either side reads. A sprite's chain is a
    /// mask and a paint, and is fresh only while both are.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            device: self.device || other.device,
            dpi: self.dpi || other.dpi,
            color: self.color || other.color,
        }
    }

    /// Returns whether an entry built at `built` is still fresh under `now`.
    #[must_use]
    pub fn fresh(self, built: Gen, now: Gen) -> bool {
        (!self.device || built.device == now.device)
            && (!self.dpi || built.dpi == now.dpi)
            && (!self.color || built.color == now.color)
    }
}

/// Describes one rasterizable family: how its key sizes, and how it draws.
///
/// A cell is a pure function of its key, so recovery after device loss is to clear the cache
/// and rebind, down the same path a first bind takes.
pub trait Cell: Clone + Eq + core::hash::Hash {
    /// Which generations this family's cells are invalidated by.
    const DEPS: GenMask;
    /// Whether this family carries coverage rather than colour.
    ///
    /// A coverage cell is allocated one byte per pixel, an eighth of the memory. A colour
    /// family that declares coverage loses its colour.
    const COVERAGE: bool;

    /// The device resources this family draws with, built once per device rather than per
    /// cell.
    ///
    /// A [`Draw`] carries no device, so everything a draw needs beyond the target and the
    /// key arrives through this bundle. Only content particular to one key is built inside
    /// a draw.
    type Res;

    /// Builds this family's resources. Called on the first miss, and again after device
    /// loss, which clears the cache.
    fn resources(gpu: &Gpu) -> Result<Self::Res>;

    /// Returns the cell's pixel extent. Already snapped: the key's constructor snaps it.
    fn px(&self) -> (u32, u32);
    /// Returns what the content does with alpha, which decides the surface's alpha mode. A
    /// coverage family is always translucent.
    fn opacity(&self) -> Opacity;
    /// Paints the cell at `(0, 0)`, in DIPs.
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

/// Caches the rasterized cells of one family, evicting the least recently used.
///
/// A hit is one map lookup and one store of the recency stamp. Finding the oldest entry
/// scans the map instead, and runs only while the cache is at capacity.
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

/// Holds every evicting cache. Content keyed by an identity the model owns lives in
/// [`Resources`](crate::res::Resources) instead.
#[derive(Default)]
pub(crate) struct Cells {
    pub(crate) boxes: Cache<BoxKey>,
    pub(crate) solids: Cache<SolidKey>,
}

impl Cells {
    /// Drops every cell in both caches. The whole of what device loss does here.
    pub(crate) fn clear(&mut self) {
        self.boxes.clear();
        self.solids.clear();
    }
}

/// How many cells one cache keeps. Both families are small — a handful of corner profiles,
/// a palette's worth of colours — so the cap bounds a key population that turns out wider
/// than expected rather than limiting ordinary use.
pub const CACHE_CAP: usize = 256;

impl<K: Cell> Default for Cache<K> {
    fn default() -> Self {
        Self::new(CACHE_CAP)
    }
}

impl<K: Cell> Cache<K> {
    /// Creates an empty cache holding at most `cap` cells. A `cap` of zero is raised to one.
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

    /// Returns the brush for `key`, rasterizing it on a miss or when its generations went
    /// stale.
    ///
    /// `Ok(None)` reports device loss during the draw: the caller drops its binding and
    /// rebinds after recovery, down the same path a first bind takes.
    ///
    /// # Errors
    ///
    /// Fails if the family's resources or the cell's surface cannot be built, or if the
    /// cell's own draw failed.
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
        // A graphics device admits one `BeginDraw` at a time, and a concurrent second fails
        // outright. Every rasterization in this crate runs behind one `&mut Scene` on one
        // thread, and a presentation region draws on its own device, so no second bracket
        // can be open here.
        // The draw's error is carried out of the closure rather than raised inside it: the
        // surface publishes whatever the cell managed, and the caller still sees the
        // failure without the frame being failed over one cell.
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

    /// Drops every entry and the family's resources. What device loss does; a merely stale
    /// entry is re-rasterized in place on its next use rather than swept.
    pub fn clear(&mut self) {
        self.map.clear();
        self.res = None;
    }

    /// Returns hits, misses and evictions so far, in that order.
    #[must_use]
    pub fn counts(&self) -> (u64, u64, u64) {
        (self.hits, self.misses, self.evictions)
    }

    /// Returns how many cells are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns whether the cache holds no cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Evicts least-recently-used entries until one slot is free.
    ///
    /// Scans the map once per eviction, and runs only while the cache is at capacity.
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

/// Keys a rounded-box coverage cell, consumed through a nine-grid brush.
///
/// The cell is opaque white in the requested corner profile, sized to that profile alone:
/// the nine-grid stretches one raster to any width and height with the corners intact, so
/// the key carries the profile and not the box.
///
/// The fields are private and [`BoxKey::new`] is the only constructor, so every key is
/// snapped to the physical grid.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoxKey {
    /// Quarter-pixel radii, in the order the corners are drawn.
    radius: [i32; 4],
    /// The cell's pixel extent: the corner profile plus a pixel of margin on each side,
    /// which is the flat interior the nine-grid's insets stretch from.
    px: (u32, u32),
    inset: u32,
}

impl BoxKey {
    /// Builds the key for corner profile `radius` at `scale`, snapping every dimension.
    #[must_use]
    pub fn new(radius: Corners, scale: f32) -> Self {
        let quarter = |r: f32| (snap_detail(r, scale) * scale * 4.0).round() as i32;
        // A pixel of margin past the largest radius, so the nine-grid's centre is a whole
        // pixel of flat interior for the stretch to sample.
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

    /// Returns the nine-grid's inset on each edge, in physical pixels.
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
            // A uniform profile has an analytic rounded rectangle, which Direct2D
            // rasterizes by the pixels it touches rather than by tessellating a mesh.
            // Filling the shape is one coverage computation; clipping to it would
            // antialias two edges meeting at the same boundary.
            d.fill(box_.rounded(tl), &res.white);
            return Ok(());
        }
        // Four independent radii have no analytic form, so this profile draws a path built
        // for this key alone. It is built on a cache miss, not per frame.
        let path = res.gpu.path(|sink| {
            sink.rounded_box(box_, [tl, tr, br, bl]);
            Ok(())
        })?;
        d.fill(&path, &res.white);
        Ok(())
    }
}

/// The resources a box cell draws with.
///
/// The brush is opaque white and built once per device: a coverage cell is a mask and white
/// is the multiplicative identity, so it is never retinted and one instance serves every box
/// cell for the life of the device.
///
/// The [`Gpu`] is held because a [`Draw`] exposes no route back to one, and a four-radius
/// profile builds a path from the key.
pub struct BoxRes {
    white: Solid,
    gpu: Gpu,
}

/// Opaque white, the only colour a coverage cell draws in.
const WHITE: Scrgb = Scrgb {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// Keys a flat colour cell: one quantized, display-referred colour.
///
/// The cell is four by four rather than one by one for filtering margin — a brush stretched
/// from a single texel samples its own edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SolidKey(Q);

impl SolidKey {
    /// Quantizes `color`, which must already have been through the output transform.
    ///
    /// The caller that knows the display applies that transform, so the key is the
    /// post-transform value: the same authored light keys a different cell on a different
    /// display, which is why a display-capability change bumps [`Gen::color`].
    #[must_use]
    pub fn new(color: Scrgb) -> Self {
        Self(Q::new(color))
    }
}

impl Cell for SolidKey {
    const DEPS: GenMask = GenMask::LIGHT;
    /// Colour, not coverage: the whole cell is the key's colour.
    const COVERAGE: bool = false;
    /// None: the cell is drawn by clearing to a colour, which needs no device resource.
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
        // The dequantized value is the only one in scope: the field is private and `Q`
        // keeps no copy of what it was given, so the cell is painted in exactly the colour
        // the key round-trips to.
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
        // Device loss takes everything.
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
