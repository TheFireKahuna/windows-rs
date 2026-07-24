//! A shared cache of rasterized coverage **masks**, keyed by whatever identifies
//! the pixels.
//!
//! Both text raster caches are instances of it: the per-glyph
//! [`glyph_atlas`](super::glyph_atlas) keys on `(face, glyph, size, phase,
//! scale)` and rasterizes one glyph; the per-run [`run_atlas`](super::run_atlas)
//! keys on a whole run's shaped content and rasterizes it in one call. They
//! differ ONLY in what a key is and how a miss is drawn — the LRU, the epoch, the
//! monotonic id, the surface-mint seam and the identity/eviction rules are all
//! here, once.
//!
//! ## Everything is a mask, never a picture
//!
//! A cached raster carries **coverage and no colour**. Colour arrives later, when
//! a caller pairs the mask with an FP16 scRGB source through a
//! `CompositionMaskBrush` — exactly the construction
//! [`path_shape::PathLayer`](super::path_shape::PathLayer) and the knob arc use.
//! So colour is deliberately absent from every key: one raster serves every
//! colour, emphasis and disabled state its text is ever drawn in, and a recolour
//! is a `SetSource` on the mask brush — no re-raster, no repaint, no loss of
//! dynamic range. See [`super::glyph_atlas`]'s header for the full HDR argument;
//! it holds verbatim at either grain.

use std::cell::Cell;
use std::hash::Hash;

use rustc_hash::FxHashMap;
use windows_canvas::FontFace;
use windows_composition::{CompositionDrawingSurface, CompositionSurfaceBrush, PixelFormat};

use super::bootstrap::Compositing;

/// The pixel format every mask is rasterized in.
///
/// Coverage arrives from the rasterizer as `u8`, so this FP16 currently stores
/// 8-bit data in 64 bits per pixel and carries no information the source did not
/// have. It can become A8 for an 8× memory cut with no change to the pixels; what
/// remains unproven is only that the compositor honours an A8 surface as a
/// `CompositionMaskBrush`'s mask (see `glyph_atlas::a8_is_a_mintable_mask_surface`).
pub(crate) const MASK_FORMAT: PixelFormat = PixelFormat::Rgba16Float;

/// The placement-relevant geometry of a rasterized mask: the size a sprite is
/// given (DIPs), and the anchor origin within the raster, in whole physical
/// pixels.
///
/// Everything here is a whole physical pixel except the DIP size, which is the
/// pixel size divided by scale. A sprite placed at a fractional pixel offset is
/// bilinearly resampled and reads soft and heavy, so a caller lands the box at
/// `(pen_px - origin_px) / scale`, both subtrahends integers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MaskGeom {
    /// Surface size in DIPs — `px / scale`, the exact size a sprite must be given
    /// for a 1:1 blit.
    pub size_dip: (f32, f32),
    /// The mask's anchor (a glyph's or a run's baseline origin) measured in whole
    /// physical pixels from the box's top-left, subpixel phase excluded.
    pub origin_px: (i32, i32),
}

/// The seam surfaces are minted through.
///
/// Exists so rasterization can be exercised against a windowless composition
/// device in tests: the shipping implementation is [`Compositing`] and forwards
/// straight to it, so a test that passes here is a statement about the real
/// surface path, not a mock of it.
pub(crate) trait MaskSurfaces {
    /// Mint a surface of exactly `px_w`×`px_h` pixels in `format`, and a
    /// Fill-stretch brush over it.
    ///
    /// An unsupported format must surface as `Err` rather than a fallback — that
    /// failure is the only reliable probe for whether a format is usable at all.
    fn mint(
        &self,
        px_w: i32,
        px_h: i32,
        format: PixelFormat,
    ) -> windows_core::Result<(CompositionDrawingSurface, CompositionSurfaceBrush)>;

    /// The flag a drawing session reports device loss through.
    fn device_lost(&self) -> &Cell<bool>;
}

impl MaskSurfaces for Compositing {
    fn mint(
        &self,
        px_w: i32,
        px_h: i32,
        format: PixelFormat,
    ) -> windows_core::Result<(CompositionDrawingSurface, CompositionSurfaceBrush)> {
        self.new_surface_with_format(px_w, px_h, format)
    }

    fn device_lost(&self) -> &Cell<bool> {
        &self.device_lost
    }
}

/// One cached raster, handed back to a caller that will place it.
#[derive(Clone)]
pub(crate) struct Raster {
    /// The mask. Bind it as a `CompositionMaskBrush`'s MASK, with an FP16 solid
    /// as the source — never as a sprite brush directly, which would paint the
    /// mask's own (colourless) pixels.
    pub brush: CompositionSurfaceBrush,
    /// Which raster this is, from a counter that only ever increases. It lets a
    /// caller answer "is this the mask I already bound?" with an integer compare,
    /// avoiding a `QueryInterface` per side on the re-place path; and being minted
    /// rather than derived removes the ABA hazard a pointer identity would carry.
    pub id: u64,
    pub geom: MaskGeom,
}

/// What a miss-handler produces for the cache to keep.
pub(crate) struct Rasterized {
    pub brush: CompositionSurfaceBrush,
    pub surface: CompositionDrawingSurface,
    pub geom: MaskGeom,
    /// The font face this raster keyed on, kept alive for as long as the entry is
    /// cached so its pointer identity in the key cannot be recycled under us (the
    /// ABA argument the glyph key makes). One COM reference per distinct face,
    /// bounded by the faces in use, not by the raster count.
    pub face: FontFace,
}

struct Entry {
    brush: CompositionSurfaceBrush,
    id: u64,
    /// Keeps the pixels alive behind the brush.
    _surface: CompositionDrawingSurface,
    /// Keeps the KEYED FACE alive, so its pointer cannot be recycled under us.
    _face: FontFace,
    geom: MaskGeom,
    /// Logical clock reading of the last bind — the LRU ordering.
    used: u64,
}

/// Rasterized masks, shared across every piece of text that draws them.
pub(crate) struct MaskCache<K> {
    map: FxHashMap<K, Entry>,
    /// Hard cap on live rasters, enforced by LRU eviction on a miss at capacity.
    cap: usize,
    /// Bumped on [`clear`](Self::clear); callers re-bind when their bound epoch no
    /// longer matches.
    epoch: u32,
    /// Monotonic bind counter driving the LRU ordering.
    clock: u64,
    /// Monotonic raster counter — the source of [`Raster::id`]. Separate from
    /// `clock`, which is a RECENCY reading overwritten on every bind; an identity
    /// is minted once and never moves.
    next_id: u64,
}

impl<K: Eq + Hash + Copy> MaskCache<K> {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            map: FxHashMap::default(),
            cap,
            epoch: 0,
            clock: 0,
            next_id: 0,
        }
    }

    /// Drop every cached raster (display / DPI / device edge). A theme change does
    /// NOT belong here: the masks carry no colour, so a recolour re-binds the mask
    /// brush's source and leaves every raster valid.
    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.epoch = self.epoch.wrapping_add(1);
    }

    pub(crate) fn epoch(&self) -> u32 {
        self.epoch
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    /// Fetch `key`, rasterizing on a miss via `raster`.
    ///
    /// `raster` is called at most once, only on a miss, and returns `None` to
    /// abandon the fetch (a device-loss edge, an unsupported format) — the caller
    /// then places nothing rather than a half-bound mask. Evicting an entry a
    /// sprite is still bound to is safe: the sprite's own `CompositionSurfaceBrush`
    /// holds a reference to the surface and keeps rendering those pixels until it
    /// re-binds.
    pub(crate) fn get(
        &mut self,
        key: K,
        raster: impl FnOnce() -> Option<Rasterized>,
    ) -> Option<Raster> {
        self.clock += 1;
        let now = self.clock;

        if !self.map.contains_key(&key) {
            let r = raster()?;
            if self.map.len() >= self.cap {
                self.evict_lru();
            }
            // Never reset, not even by `clear` — an id that came back would let a
            // caller's cached one match a raster it has never seen.
            self.next_id += 1;
            self.map.insert(
                key,
                Entry {
                    brush: r.brush,
                    id: self.next_id,
                    _surface: r.surface,
                    _face: r.face,
                    geom: r.geom,
                    used: now,
                },
            );
        }

        let e = self.map.get_mut(&key)?;
        e.used = now;
        Some(Raster {
            brush: e.brush.clone(),
            id: e.id,
            geom: e.geom,
        })
    }

    /// Drop the least recently bound raster. Called only on a miss at capacity.
    fn evict_lru(&mut self) {
        if let Some(k) = self.map.iter().min_by_key(|(_, e)| e.used).map(|(k, _)| *k) {
            self.map.remove(&k);
        }
    }
}
