//! **Run atlas** — one rasterized coverage mask per whole shaped RUN, the coarse
//! sibling of the per-glyph [`glyph_atlas`](super::glyph_atlas).
//!
//! Where the glyph atlas caches one mask per glyph and a label mounts one sprite
//! per glyph, this caches one mask per *run* and a label mounts one sprite per
//! *line*. Both are keyings of the same [`MaskCache`](super::mask_cache): the LRU,
//! the epoch, the id minting, the surface-mint seam and the mask-vs-picture rule
//! all live there, once. This module owns only what is run-specific — the content
//! [`RunKey`] and the whole-run [`rasterize_run`].
//!
//! ## When each atlas wins
//!
//! The glyph atlas exists to make text whose CONTENT changes per frame cheap: a
//! meter that ticks re-places cached glyph masks and rasterizes nothing. The run
//! atlas is the opposite trade — one sprite instead of N, at the cost of a
//! re-raster when the run's shaped content changes. So STATIC text (labels, prose,
//! chrome) draws through here and LIVE text (readouts, the editor, the knob)
//! stays on the glyph atlas. See [`glyph_text::TextMode`](super::glyph_text).
//!
//! ## Why one coverage call does a whole run
//!
//! [`glyph_run_coverage`] wraps `CreateGlyphRunAnalysis`, which rasterizes an
//! entire `GlyphRun` — every glyph at its shaped advance and offset — into one
//! coverage bitmap. The glyph atlas happens to call it with a one-glyph run; a
//! full run is the same call with the full arrays, and the bounds it returns are
//! ink-tight, so the surface carries no advance-box padding and the mask is as
//! small as the pixels allow.
//!
//! ## Subpixel
//!
//! Internal glyph fractions are baked into the coverage — a run is rasterized once
//! at a zero baseline, so the letters inside it sit exactly where shaping put
//! them. Only the run's WHOLE origin is snapped to a pixel at placement, a uniform
//! sub-pixel shift of the line that carries none of the cumulative drift the glyph
//! path rasterizes phases to avoid. If a left edge ever reads soft, a run-origin
//! phase (as the glyph atlas does) is the lever — deliberately not paid up front.

use std::hash::{Hash, Hasher};

use rustc_hash::FxHasher;
use windows_canvas::{
    glyph_run_coverage, ColorF, DrawingSession, GlyphRun, ID2D1DeviceContext, Rect,
};
use windows_numerics::Matrix3x2;

use super::glyph_atlas::{face_id, quant_em, quant_scale};
use super::mask_cache::{Atlas, MaskCache, MaskGeom, MaskSurfaces, Raster, Rasterized};

/// Hard cap on live run rasters, enforced by LRU eviction on a miss at capacity.
///
/// A run mask is a whole line's ink, so the population is counted in lines on
/// screen, not glyphs: a busy window shows a few hundred static runs (labels,
/// nav rows, an inspector's config lines). 1024 holds that working set plus
/// headroom for a scroll, so steady state never evicts; a run dropped under
/// memory pressure simply re-rasterizes on its next appearance.
const RUN_ATLAS_CAP: usize = 1024;

/// Identity of one rasterized run.
///
/// `content` folds everything about the shaped run that changes the PIXELS —
/// the glyph ids, their shaped advances and their per-glyph offsets — into one
/// hash; `face`, `em` and `scale` are the rest, keyed exactly as the glyph atlas
/// keys them. Colour is absent for the same reason it is there: the raster is a
/// mask, and a recolour is a `SetSource`.
///
/// Two DISTINCT runs sharing a `content` hash is a 1-in-2^64 event whose only
/// consequence is one line drawn with another's glyphs; two IDENTICAL runs
/// sharing it is the point — a repeated unit or config key rasterizes once.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct RunKey {
    content: u64,
    face: usize,
    em: u32,
    scale: u32,
}

impl RunKey {
    fn new(run: &GlyphRun, scale: f32) -> Self {
        let mut h = FxHasher::default();
        run.glyph_indices.hash(&mut h);
        for &a in &run.glyph_advances {
            a.to_bits().hash(&mut h);
        }
        for o in &run.glyph_offsets {
            o.advance_offset.to_bits().hash(&mut h);
            o.ascender_offset.to_bits().hash(&mut h);
        }
        Self {
            content: h.finish(),
            face: face_id(&run.font_face),
            em: quant_em(run.font_em_size, scale),
            scale: quant_scale(scale),
        }
    }
}

/// Rasterized run masks, shared across every static label that draws them.
pub(crate) struct RunAtlas {
    cache: MaskCache<RunKey>,
}

impl Default for RunAtlas {
    fn default() -> Self {
        Self {
            cache: MaskCache::new(RUN_ATLAS_CAP),
        }
    }
}

impl RunAtlas {
    /// Drop every cached raster (display / DPI / device edge) — see
    /// [`MaskCache::clear`](super::mask_cache::MaskCache::clear).
    pub(crate) fn clear(&mut self) {
        self.cache.clear();
    }

    pub(crate) fn epoch(&self) -> u32 {
        self.cache.epoch()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.cache.len()
    }

    /// Fetch (rasterizing on a miss) the mask for a whole shaped run.
    ///
    /// `scale` is the DIP→px factor. The returned [`Raster`]'s `geom.origin_px` is
    /// the run's baseline origin measured from the mask's top-left, so a caller
    /// places it exactly as it places a glyph: box top-left at
    /// `(pen_px - origin_px) / scale`.
    pub(crate) fn get(&mut self, dev: &impl MaskSurfaces, run: &GlyphRun, scale: f32) -> Option<Raster> {
        let key = RunKey::new(run, scale);
        self.cache
            .get(key, |atlas| rasterize_run(dev, atlas, run, scale))
    }
}

/// Rasterize a whole run's coverage and upload it as one mask.
///
/// The run is rasterized at a ZERO baseline, so the coverage bounds locate the
/// ink relative to that origin: the surface is sized exactly to the bounds and
/// the baseline lands at `(-left, -top)` inside it. Nothing is drawn but the
/// upload — the coverage is DirectWrite's own, produced on the CPU, so the bytes
/// do not depend on the surface format (see [`glyph_atlas::rasterize`]'s note).
fn rasterize_run(
    dev: &impl MaskSurfaces,
    atlas: &mut Atlas,
    run: &GlyphRun,
    scale: f32,
) -> Option<Rasterized> {
    let cov = glyph_run_coverage(run, scale, (0.0, 0.0)).ok().flatten();
    // A run that marks no pixels — pure whitespace — keeps a minimal cleared
    // surface rather than failing, so the cache entry stops it being asked again.
    let (px_w, px_h, left, top) = match &cov {
        Some(c) => ((c.width as i32).max(1), (c.height as i32).max(1), c.left, c.top),
        None => (1, 1, 0, 0),
    };

    let tile = atlas.alloc(dev, px_w, px_h, scale)?;
    let (ctx, (origin_x, origin_y)) = match tile.begin_draw::<ID2D1DeviceContext>() {
        Ok(c) => c,
        Err(e) => {
            if super::bootstrap::is_device_loss(&e) {
                dev.device_lost().set(true);
            }
            return None;
        }
    };
    let session = DrawingSession::from_borrowed_context(
        &ctx,
        Matrix3x2::translation(origin_x as f32, origin_y as f32),
    );
    // Everything below is confined to this run's own region — see `Tile::clip`.
    let (cx, cy, cw, ch) = tile.clip();
    session.push_clip(&Rect::from_xywh(cx, cy, cw, ch));
    session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));

    if let Some(c) = &cov {
        if c.width > 0 && c.height > 0 {
            // Premultiplied white at the coverage — exactly what drawing an opaque
            // white run produced. Uploaded unmapped (coverage is not a colour); the
            // tonemap belongs on the FP16 source the mask brush is paired with.
            let mut rgba = vec![0.0f32; (c.width as usize) * (c.height as usize) * 4];
            for (i, &a) in c.alpha.iter().enumerate() {
                let v = f32::from(a) / 255.0;
                rgba[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, v]);
            }
            if let Ok(bitmap) = session.create_bitmap_fp16(c.width, c.height, &rgba) {
                // The surface is sized exactly to the ink bounds, so the coverage
                // lands at the box's own top-left.
                session.draw_bitmap(
                    &bitmap,
                    &Rect::from_xywh(0.0, 0.0, c.width as f32, c.height as f32),
                    1.0,
                );
            }
        }
    }

    session.pop_clip();
    tile.end_draw().ok()?;
    Some(Rasterized {
        tile,
        geom: MaskGeom {
            size_dip: (px_w as f32 / scale, px_h as f32 / scale),
            // The baseline origin (0,0) measured from the tight box's top-left.
            origin_px: (-left, -top),
        },
        face: run.font_face.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    use windows_canvas::{GpuDevice, TextFormat, TextLayout};
    use windows_composition::{
        AlphaMode, CompositionGraphicsDevice, CompositionSurfaceBrush,
        CompositionVirtualDrawingSurface, Compositor, DispatcherQueueController, PixelFormat,
    };

    /// A windowless composition graphics device — the same chain `Compositing::new`
    /// builds minus the HWND, so the rasterizer under test is the shipping one on
    /// the shipping surface path.
    struct Headless {
        _queue: Option<DispatcherQueueController>,
        _gpu: GpuDevice,
        graphics: CompositionGraphicsDevice,
        compositor: Compositor,
        lost: Cell<bool>,
    }

    impl Headless {
        fn new() -> windows_core::Result<Self> {
            let queue = DispatcherQueueController::create_on_current_thread().ok();
            let gpu = GpuDevice::new_or_warp()?;
            let compositor = Compositor::new()?;
            let graphics = compositor.create_graphics_device(gpu.d2d_device())?;
            Ok(Self {
                _queue: queue,
                _gpu: gpu,
                graphics,
                compositor,
                lost: Cell::new(false),
            })
        }
    }

    impl MaskSurfaces for Headless {
        fn mint_page(
            &self,
            px_w: i32,
            px_h: i32,
            format: PixelFormat,
        ) -> windows_core::Result<CompositionVirtualDrawingSurface> {
            // The same virtual-surface factory the shipping device uses: the
            // whole point of the seam is that a test exercises the real surface
            // path rather than a stand-in for it.
            self.graphics
                .create_virtual_drawing_surface(px_w, px_h, format, AlphaMode::Premultiplied)
        }

        fn page_brush(&self, page: &CompositionVirtualDrawingSurface) -> CompositionSurfaceBrush {
            self.compositor.create_surface_brush(page)
        }

        fn device_lost(&self) -> &Cell<bool> {
            &self.lost
        }
    }

    fn run_for(text: &str, em: f32) -> GlyphRun {
        let format = TextFormat::new("Segoe UI", em).unwrap();
        let layout = TextLayout::new(text, &format, 1000.0, 100.0).unwrap();
        layout.glyph_runs().unwrap().into_iter().next().unwrap()
    }

    /// A whole run rasterizes to one cached mask, a re-fetch is the identical
    /// surface (not a re-raster), and a DIFFERENT run is its own entry — the whole
    /// point of the coarse grain, exercised on the shipping surface path.
    #[test]
    fn a_run_rasterizes_once_and_caches() {
        let Ok(dev) = Headless::new() else {
            eprintln!("no composition graphics device available; skipping");
            return;
        };
        let mut atlas = RunAtlas::default();
        assert_eq!(atlas.len(), 0);

        let run = run_for("Hamburgefonstiv", 14.0);
        let first = atlas.get(&dev, &run, 1.0).expect("rasterize a run");
        assert_eq!(atlas.len(), 1, "one run, one entry");
        // A run of real letters has a positive-sized mask.
        assert!(first.geom.size_dip.0 > 0.0 && first.geom.size_dip.1 > 0.0);

        let again = atlas.get(&dev, &run, 1.0).expect("cache hit");
        assert_eq!(atlas.len(), 1, "a hit must not add an entry");
        assert!(
            first.brush() == again.brush(),
            "a hit returns the identical surface brush, not a re-raster"
        );
        assert_eq!(first.id, again.id, "…and the identity a caller compares agrees");

        // Different words → different pixels → its own entry.
        let other = atlas.get(&dev, &run_for("Different", 14.0), 1.0).expect("second run");
        assert_eq!(atlas.len(), 2, "a distinct run is a distinct entry");
        assert_ne!(first.id, other.id);

        // Clearing drops the rasters and bumps the epoch.
        let epoch = atlas.epoch();
        atlas.clear();
        assert_eq!(atlas.len(), 0);
        assert_ne!(atlas.epoch(), epoch);
    }

    /// The whole point of a content key: the SAME shaped run, fetched twice, is one
    /// entry — a repeated unit or config key rasterizes once — while its baseline
    /// origin (which the caller applies at placement) is deliberately not keyed, so
    /// the same words on two different lines share the mask.
    #[test]
    fn identical_runs_share_one_mask() {
        let Ok(dev) = Headless::new() else {
            eprintln!("no composition graphics device available; skipping");
            return;
        };
        let mut atlas = RunAtlas::default();
        // Two independently shaped runs of the same word.
        let a = atlas.get(&dev, &run_for("dB", 12.0), 1.0).expect("first");
        let b = atlas.get(&dev, &run_for("dB", 12.0), 1.0).expect("second");
        assert_eq!(atlas.len(), 1, "the same shaped run must not fork the cache");
        assert_eq!(a.id, b.id);
    }
}
