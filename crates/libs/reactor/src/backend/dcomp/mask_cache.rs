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
//!
//! ## One surface, many rasters
//!
//! Both caches pack their rasters into regions of a shared [`Atlas`] rather than
//! giving each one a surface of its own. A surface per raster is a composition
//! object, a texture allocation and per-surface bookkeeping in the composition
//! engine for every glyph the app has ever drawn — a census of a single idle
//! window found 230 of them standing. Packing costs one of each per page, and a
//! virtual surface still only materializes storage for the regions actually
//! drawn into, so a page can be declared far larger than it will be filled.
//!
//! ## A region is re-let only when nothing can still be showing it
//!
//! The one hazard packing introduces is that a region is shared ground. A sprite
//! binds a raster's brush into a `CompositionMaskBrush` and re-binds only when it
//! next syncs and sees a different [`Raster::id`] — a label that never changes
//! never syncs, and holds its binding indefinitely. Re-letting that label's
//! region would silently redraw it with another glyph's ink.
//!
//! So a region is not owned by the cache entry; it is owned by an [`Rc<Tile>`]
//! that the entry and every binding hold together, and it returns to the free
//! list in the tile's `Drop`. Eviction drops the cache's handle and nothing
//! more: the region comes back only once the last sprite bound to it has re-bound
//! or gone away. Consumers opt in by *storing* the tile beside the sprite (see
//! `glyph_text::GlyphSprite::lease`), which makes the guarantee a matter of
//! ownership rather than a protocol a caller could forget to observe.

use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::Rc;

use rustc_hash::FxHashMap;
use windows_canvas::FontFace;
use windows_composition::{
    CompositionSurfaceBrush, CompositionVirtualDrawingSurface, PixelFormat, Stretch, Vector2,
};

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

/// The seam pages are minted through.
///
/// Exists so rasterization can be exercised against a windowless composition
/// device in tests: the shipping implementation is [`Compositing`] and forwards
/// straight to it, so a test that passes here is a statement about the real
/// surface path, not a mock of it.
pub(crate) trait MaskSurfaces {
    /// Mint an atlas page — a virtual surface `px_w`×`px_h` pixels in its
    /// declared size, holding no storage until something is drawn into it.
    ///
    /// An unsupported format must surface as `Err` rather than a fallback — that
    /// failure is the only reliable probe for whether a format is usable at all.
    fn mint_page(
        &self,
        px_w: i32,
        px_h: i32,
        format: PixelFormat,
    ) -> windows_core::Result<CompositionVirtualDrawingSurface>;

    /// A brush over `page`, before the atlas aims it at one region.
    fn page_brush(&self, page: &CompositionVirtualDrawingSurface) -> CompositionSurfaceBrush;

    /// The flag a drawing session reports device loss through.
    fn device_lost(&self) -> &Cell<bool>;
}

impl MaskSurfaces for Compositing {
    fn mint_page(
        &self,
        px_w: i32,
        px_h: i32,
        format: PixelFormat,
    ) -> windows_core::Result<CompositionVirtualDrawingSurface> {
        self.new_mask_page(px_w, px_h, format)
    }

    fn page_brush(&self, page: &CompositionVirtualDrawingSurface) -> CompositionSurfaceBrush {
        self.compositor().create_surface_brush(page)
    }

    fn device_lost(&self) -> &Cell<bool> {
        &self.device_lost
    }
}

// ── The atlas ────────────────────────────────────────────────────────────────

/// Declared size of one page, per axis. The platform caps a virtual surface at
/// 2^24 pixels in total, and this sits an order of magnitude inside that: it is
/// a coordinate space to pack within, not an allocation, and only the regions
/// drawn into cost anything.
const PAGE_PX: i32 = 2048;

/// How many pages may be minted before an allocation is refused. Reached only if
/// the live working set genuinely exceeds four pages, which the caches' own
/// entry caps make unreachable in practice — regions are reused, so packing does
/// not creep upward with time.
const MAX_PAGES: usize = 4;

/// Transparent margin around every region, in pixels.
///
/// A region is cleared to its full reserved size and the content placed one pixel
/// in, so every raster is surrounded by transparency it owns. Without it the
/// bilinear sample at a sprite's outer edge could reach a neighbouring raster's
/// ink, which reads as a stray mark along one side of a glyph and only under some
/// scales.
const PAD: i32 = 1;

/// Reserved sizes are rounded up to a multiple of this on both axes.
///
/// Quantizing is what makes reuse exact: a freed region is offered back only to a
/// request of the same class, so it always fits and never leaves a sliver behind.
/// The cost is under `GRAIN` pixels of slack per axis per raster; the benefit is
/// an allocator with no fragmentation to manage at all.
const GRAIN: i32 = 4;

fn class_of(px: i32) -> i32 {
    let want = px.max(1) + 2 * PAD;
    // `div_ceil` is still unstable for integers here, so the round-up is written out.
    (want + GRAIN - 1) / GRAIN * GRAIN
}

/// One reserved region: which page it is on, and the rectangle it holds there.
/// The rectangle is the size CLASS, not the request — see [`GRAIN`].
#[derive(Clone, Copy, PartialEq, Eq)]
struct Slot {
    page: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

/// A row of equal-height regions packed left to right.
///
/// Shelves key on the exact height class, so a shelf never holds a region
/// shorter than itself and no vertical space is lost to a mismatch.
struct Shelf {
    y: i32,
    h: i32,
    next_x: i32,
}

struct Page {
    surface: CompositionVirtualDrawingSurface,
    shelves: Vec<Shelf>,
    next_y: i32,
}

/// A reserved region of a page, the brush aimed at it, and the right to draw
/// there — held jointly by the cache entry and every sprite bound to it.
///
/// Dropping the last one returns the region for reuse. Nothing else does: that is
/// the whole of the safety argument in this module's header.
pub(crate) struct Tile {
    slot: Slot,
    page: CompositionVirtualDrawingSurface,
    brush: CompositionSurfaceBrush,
    recycle: Rc<RefCell<Vec<Slot>>>,
}

impl Drop for Tile {
    fn drop(&mut self) {
        // Through a shared list rather than straight into the allocator: a tile
        // outlives the borrow that made it, and may be dropped from anywhere.
        // The allocator drains this before it looks anywhere else.
        self.recycle.borrow_mut().push(self.slot);
    }
}

impl Tile {
    /// The brush to bind as a `CompositionMaskBrush`'s mask. It shows this
    /// region and nothing else.
    pub(crate) fn brush(&self) -> &CompositionSurfaceBrush {
        &self.brush
    }

    /// Begin drawing this region, returning the target and the offset to
    /// translate drawing by.
    ///
    /// The whole reserved rectangle is opened, [`PAD`] included, because a
    /// region's initial contents are undefined and the margin has to be cleared
    /// rather than merely skipped. The returned offset already accounts for the
    /// margin, so a caller draws its content at its own origin.
    pub(crate) fn begin_draw<T: windows_core::Interface>(
        &self,
    ) -> windows_core::Result<(T, (i32, i32))> {
        let (target, (ox, oy)) =
            self.page
                .begin_draw::<T>(self.slot.x, self.slot.y, self.slot.w, self.slot.h)?;
        Ok((target, (ox + PAD, oy + PAD)))
    }

    /// The reserved rectangle as `(x, y, w, h)`, in the coordinates
    /// [`begin_draw`](Self::begin_draw)'s offset establishes — content origin at
    /// zero, so the margin lies at negative coordinates.
    ///
    /// **Push this as a clip before drawing anything.** `BeginDraw` hands back a
    /// context on whichever tile the engine allocated, not on this region alone,
    /// and a `Clear` obeys the clip stack and nothing else: unclipped, it would
    /// wipe every other raster sharing that tile. The clip also holds a
    /// rasterizer that overshoots its measured bounds to its own ground.
    pub(crate) fn clip(&self) -> (f32, f32, f32, f32) {
        (
            -(PAD as f32),
            -(PAD as f32),
            self.slot.w as f32,
            self.slot.h as f32,
        )
    }

    pub(crate) fn end_draw(&self) -> windows_core::Result<()> {
        self.page.end_draw()
    }

    /// Which page, and where on it. The identity a test compares to tell a
    /// re-let region from a fresh one.
    #[cfg(test)]
    pub(crate) fn origin(&self) -> (usize, i32, i32) {
        (self.slot.page, self.slot.x, self.slot.y)
    }
}

/// Packs rasters into pages and hands out [`Tile`]s.
pub(crate) struct Atlas {
    format: PixelFormat,
    pages: Vec<Page>,
    /// Regions returned by dropped tiles, by size class. A request of the same
    /// class is served from here before any page is touched, so a steady app
    /// re-lets the same ground instead of creeping across the page.
    free: FxHashMap<(i32, i32), Vec<Slot>>,
    /// Where [`Tile::drop`] leaves a region; drained into `free` on the next
    /// allocation.
    recycle: Rc<RefCell<Vec<Slot>>>,
    /// Set once, when an allocation has been refused — the refusal is silent to
    /// the user (a raster simply is not placed) so it must not be silent here.
    exhausted: bool,
}

impl Atlas {
    pub(crate) fn new(format: PixelFormat) -> Self {
        Self {
            format,
            pages: Vec::new(),
            free: FxHashMap::default(),
            recycle: Rc::new(RefCell::new(Vec::new())),
            exhausted: false,
        }
    }

    /// Reserve a region big enough for `px_w`×`px_h` pixels of content.
    ///
    /// `scale` is the DIP→pixel factor the content was rasterized at; it sets the
    /// brush transform so the region paints one surface pixel per physical pixel,
    /// exactly as a surface sized to the content did.
    pub(crate) fn alloc(
        &mut self,
        dev: &impl MaskSurfaces,
        px_w: i32,
        px_h: i32,
        scale: f32,
    ) -> Option<Tile> {
        self.drain_recycled();
        let class = (class_of(px_w), class_of(px_h));
        let slot = self.take_free(class).or_else(|| self.place(dev, class))?;

        let page = self.pages[slot.page].surface.clone();
        let brush = dev.page_brush(&page);
        // Anchor the page's top-left to the sprite's, then aim: scale maps a
        // surface pixel onto a physical one, and the offset carries the content's
        // origin — the reserved rectangle's, one margin in — back to zero. See
        // `CompositionSurfaceBrush::set_source_transform` for why this order.
        brush.set_stretch(Stretch::None);
        brush.set_alignment_ratio(0.0, 0.0);
        let inv = 1.0 / scale.max(f32::MIN_POSITIVE);
        brush.set_source_transform(
            Vector2 {
                x: -((slot.x + PAD) as f32) * inv,
                y: -((slot.y + PAD) as f32) * inv,
            },
            Vector2 { x: inv, y: inv },
        );
        Some(Tile {
            slot,
            page,
            brush,
            recycle: self.recycle.clone(),
        })
    }

    fn drain_recycled(&mut self) {
        let mut pending = self.recycle.borrow_mut();
        for slot in pending.drain(..) {
            self.free.entry((slot.w, slot.h)).or_default().push(slot);
        }
    }

    fn take_free(&mut self, class: (i32, i32)) -> Option<Slot> {
        self.free.get_mut(&class)?.pop()
    }

    /// Put a fresh region of `class` on a page: on a shelf of that exact height
    /// if one has room, else a new shelf, else a new page.
    fn place(&mut self, dev: &impl MaskSurfaces, class: (i32, i32)) -> Option<Slot> {
        let (w, h) = class;
        if w > PAGE_PX || h > PAGE_PX {
            return None;
        }
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(shelf) = page
                .shelves
                .iter_mut()
                .find(|s| s.h == h && s.next_x + w <= PAGE_PX)
            {
                let slot = Slot { page: i, x: shelf.next_x, y: shelf.y, w, h };
                shelf.next_x += w;
                return Some(slot);
            }
            if page.next_y + h <= PAGE_PX {
                let y = page.next_y;
                page.next_y += h;
                page.shelves.push(Shelf { y, h, next_x: w });
                return Some(Slot { page: i, x: 0, y, w, h });
            }
        }
        if self.pages.len() >= MAX_PAGES {
            if !self.exhausted {
                self.exhausted = true;
                super::animate::warn(format_args!(
                    "mask atlas exhausted at {MAX_PAGES} pages of {PAGE_PX}²; further rasters \
                     will not be placed"
                ));
            }
            return None;
        }
        let surface = dev.mint_page(PAGE_PX, PAGE_PX, self.format).ok()?;
        let i = self.pages.len();
        self.pages.push(Page {
            surface,
            shelves: vec![Shelf { y: 0, h, next_x: w }],
            next_y: h,
        });
        Some(Slot { page: i, x: 0, y: 0, w, h })
    }

    #[cfg(test)]
    pub(crate) fn pages(&self) -> usize {
        self.pages.len()
    }
}

/// One cached raster, handed back to a caller that will place it.
#[derive(Clone)]
pub(crate) struct Raster {
    /// The region this raster occupies, and the right to keep showing it.
    ///
    /// A caller that binds [`brush`](Self::brush) must **keep this** for as long
    /// as the binding stands — that is what stops the region being re-let under
    /// a sprite that is still showing it. Dropping it is the unbind.
    pub tile: Rc<Tile>,
    /// Which raster this is, from a counter that only ever increases. It lets a
    /// caller answer "is this the mask I already bound?" with an integer compare,
    /// avoiding a `QueryInterface` per side on the re-place path; and being minted
    /// rather than derived removes the ABA hazard a pointer identity would carry.
    pub id: u64,
    pub geom: MaskGeom,
}

impl Raster {
    /// The mask. Bind it as a `CompositionMaskBrush`'s MASK, with an FP16 solid
    /// as the source — never as a sprite brush directly, which would paint the
    /// mask's own (colourless) pixels.
    pub(crate) fn brush(&self) -> &CompositionSurfaceBrush {
        self.tile.brush()
    }
}

/// What a miss-handler produces for the cache to keep.
pub(crate) struct Rasterized {
    pub tile: Tile,
    pub geom: MaskGeom,
    /// The font face this raster keyed on, kept alive for as long as the entry is
    /// cached so its pointer identity in the key cannot be recycled under us (the
    /// ABA argument the glyph key makes). One COM reference per distinct face,
    /// bounded by the faces in use, not by the raster count.
    pub face: FontFace,
}

struct Entry {
    /// The cache's own share of the region. Evicting drops this and no more —
    /// see the module header.
    tile: Rc<Tile>,
    id: u64,
    /// Keeps the KEYED FACE alive, so its pointer cannot be recycled under us.
    _face: FontFace,
    geom: MaskGeom,
    /// Logical clock reading of the last bind — the LRU ordering.
    used: u64,
}

/// Rasterized masks, shared across every piece of text that draws them.
pub(crate) struct MaskCache<K> {
    map: FxHashMap<K, Entry>,
    /// The pages the rasters are packed into.
    atlas: Atlas,
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
            atlas: Atlas::new(MASK_FORMAT),
            cap,
            epoch: 0,
            clock: 0,
            next_id: 0,
        }
    }

    /// Drop every cached raster (display / DPI / device edge). A theme change does
    /// NOT belong here: the masks carry no colour, so a recolour re-binds the mask
    /// brush's source and leaves every raster valid.
    ///
    /// The pages go with them rather than being emptied and re-packed. Every
    /// reason to clear is a reason the *sizes* change too — a new DPI rasterizes
    /// into different size classes — so re-packing would leave the old classes
    /// standing as materialized storage nothing will ask for again. Releasing the
    /// surfaces reclaims that storage wholesale, and a sprite still bound to an
    /// old region holds its page alive through its own tile until it re-binds, so
    /// nothing blanks in the meantime.
    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.atlas = Atlas::new(MASK_FORMAT);
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
    /// `raster` is called at most once, only on a miss, and is handed the atlas to
    /// reserve its region from. It returns `None` to abandon the fetch (a
    /// device-loss edge, an unsupported format, an atlas with no room) — the
    /// caller then places nothing rather than a half-bound mask.
    ///
    /// Evicting an entry a sprite is still bound to is safe, though no longer for
    /// the reason it once was: the region is owned jointly, so eviction drops the
    /// cache's share alone and the sprite keeps both the pixels and the ground
    /// under them until it re-binds.
    pub(crate) fn get(
        &mut self,
        key: K,
        raster: impl FnOnce(&mut Atlas) -> Option<Rasterized>,
    ) -> Option<Raster> {
        self.clock += 1;
        let now = self.clock;

        if !self.map.contains_key(&key) {
            // Evicted BEFORE rasterizing, not after: an eviction is what offers a
            // region back, and the raster about to run is the one that wants it.
            // The other order asks the atlas for new ground on every miss while a
            // cache at capacity hands its old ground back a moment too late.
            if self.map.len() >= self.cap {
                self.evict_lru();
            }
            let r = raster(&mut self.atlas)?;
            // Never reset, not even by `clear` — an id that came back would let a
            // caller's cached one match a raster it has never seen.
            self.next_id += 1;
            self.map.insert(
                key,
                Entry {
                    tile: Rc::new(r.tile),
                    id: self.next_id,
                    _face: r.face,
                    geom: r.geom,
                    used: now,
                },
            );
        }

        let e = self.map.get_mut(&key)?;
        e.used = now;
        Some(Raster {
            tile: e.tile.clone(),
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
