//! A fixed vocabulary rasterized once and selected per frame (feature `d2d`).
//!
//! Shaping and a glyph draw stand at either end of this crate; between them sits the thing a
//! per-frame surface actually needs — pixels for a vocabulary it knows in advance, selected
//! by index. A caller that states its words up front pays shaping and rasterization once per
//! scale, ink or face change, and pays one sprite batch per frame.
//!
//! The vocabulary is closed by construction. A value outside it has no tile, which is what
//! makes the per-frame path free of text-engine calls rather than nearly free of them.

use super::*;
use windows_color::Scrgb;
use windows_core::HRESULT;
use windows_d2d::{Draw, Gpu, Interp, Opacity, Rect, SpriteBatch, Target};

/// `E_INVALIDARG`, the refusal for a word no packing places.
const E_INVALIDARG: HRESULT = HRESULT(-2_147_024_809);

/// One entry of a vocabulary: what is rasterized, how it is set, and what it is inked with.
#[derive(Copy, Clone)]
pub struct Word<'a> {
    /// The text this tile holds. Two entries may carry the same text at different faces or
    /// inks; each is its own tile.
    pub text: &'a str,
    /// The face and size it is set at.
    pub spec: FontSpec,
    /// The colour it is rasterized in, already through whatever transform the caller's
    /// surface takes.
    ///
    /// Declared here and never read back off a brush: the atlas inspects nothing it is
    /// given, so two strengths of one word are two entries rather than one tile drawn
    /// twice.
    pub ink: Scrgb,
}

/// One rasterized word: where it sits in the atlas, how big that tile is, and what the run
/// itself measured.
struct Tile {
    /// The tile's pixels in the atlas, as `[left, top, right, bottom]`.
    source: [u32; 4],
    /// The tile's DIP size, which includes the one-pixel border the rasterizer's antialias
    /// coverage reaches into.
    size: Vector2,
    /// The run's own advance, which is what a caller lays text out with. Wider than the ink
    /// box for a trailing space, narrower for an overhanging glyph, and never the tile.
    advance: Vector2,
}

/// A packed atlas of rasterized words: the pixels, and what each tile measures.
///
/// Immutable once built and therefore shareable: every consumer whose vocabulary, face,
/// ink and scale are the same reads one of these. What differs per consumer — which tiles
/// are on screen and where — lives in its own [`Placements`].
pub struct Tiles {
    atlas: Target,
    tiles: Vec<Tile>,
}

impl Tiles {
    /// Shapes `words`, packs them into rows at most `width` DIPs wide and rasterizes them at
    /// `dpi`.
    ///
    /// Rasterizes through `pass`. The caller must discard the atlas if the pass fails.
    ///
    /// # Errors
    ///
    /// Fails when shaping, the atlas allocation or the rasterizing pass fails. A word wider
    /// than `width` is refused, because no packing places it.
    pub fn build(
        gpu: &Gpu,
        pass: &mut windows_d2d::Pass<'_>,
        dpi: f32,
        ladder: FontLadder,
        width: f32,
        words: &[Word<'_>],
    ) -> Result<Self> {
        let scale = dpi / 96.0;
        let engine = TextEngine::new(ladder)?;
        let mut runs = Vec::with_capacity(words.len());
        let mut tiles = Vec::with_capacity(words.len());
        // Pack short tiles beside each other rather than reserving a full-width strip for
        // every glyph, which is what keeps the atlas bounded as the vocabulary grows.
        let bound = (width * scale).ceil() as u32;
        let (mut left, mut top, mut row_h) = (0, 0, 0);
        for word in words {
            let mut run = engine.shape(word.text, &word.spec, Flow::default())?;
            engine.harvest(&mut run)?;
            let advance = run.measure(None);
            let ink = run.line_ink(0).size;
            // One pixel of border on each side: the rasterizer's antialias coverage reaches
            // outside the ink box, and a tile cropped to it samples its neighbour.
            let w = (ink.x * scale).ceil() as u32 + 2;
            let h = (ink.y * scale).ceil() as u32 + 2;
            if w > bound {
                return Err(windows_core::Error::new(
                    E_INVALIDARG,
                    "a word is wider than the atlas",
                ));
            }
            if left + w > bound {
                top += row_h;
                left = 0;
                row_h = 0;
            }
            tiles.push(Tile {
                source: [left, top, left + w, top + h],
                size: Vector2::new(w as f32 / scale, h as f32 / scale),
                advance,
            });
            runs.push(run);
            left += w;
            row_h = row_h.max(h);
        }
        let atlas = gpu.offscreen((bound.max(1), (top + row_h).max(1)), dpi, Opacity::Translucent)?;
        let brush = gpu.solid(Scrgb::TRANSPARENT)?;
        let mut segments = SegBuffers::default();
        {
            let draw = pass.draw(&atlas);
            draw.clear(Scrgb::TRANSPARENT);
            for ((word, run), tile) in words.iter().zip(&runs).zip(&tiles) {
                brush.set(word.ink);
                segments.clear();
                let span = run.segments(0, &mut segments);
                // Fully qualified: `Draw` carries a `line` of its own, which strokes a
                // segment between two points.
                GlyphDraw::line(
                    &draw,
                    Vector2::new(
                        (tile.source[0] + 1) as f32 / scale,
                        (tile.source[1] + 1) as f32 / scale,
                    ),
                    span.of(&segments.segs),
                    &segments,
                    &engine,
                    &brush,
                );
            }
        }
        Ok(Self { atlas, tiles })
    }

    /// Creates a placement buffer over this atlas, reserving `slots` placements.
    ///
    /// One per consumer. `slots` bounds what a frame may place, because the batch behind it
    /// is rewritten on the per-frame path and may not grow there.
    ///
    /// # Errors
    ///
    /// Fails when the sprite batch cannot be created.
    pub fn placements(&self, gpu: &Gpu, slots: usize) -> Result<Placements> {
        let batch = gpu.batch()?;
        let destinations = vec![Rect::default(); slots];
        let sources = vec![[0, 0, 1, 1]; slots];
        batch.set_tiles(&destinations, &sources)?;
        Ok(Placements {
            batch,
            destinations,
            sources,
            used: 0,
            dirty: false,
        })
    }

    /// Returns the advance of the run in tile `index`, which is what a caller lays out with.
    ///
    /// # Panics
    ///
    /// Panics when `index` is outside the vocabulary. The vocabulary is the caller's own
    /// list, so an index off it is a construction error rather than a value.
    #[must_use]
    pub fn advance(&self, index: usize) -> Vector2 {
        self.tiles[index].advance
    }

    /// Returns how many tiles the vocabulary holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Returns whether the vocabulary is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Returns the atlas allocation in pixels, which is what a consumer's own bound is
    /// stated against.
    #[must_use]
    pub fn size_px(&self) -> (u32, u32) {
        self.atlas.size_px()
    }

    /// Returns the DIP-to-pixel factor the tiles were rasterized at.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.atlas.dpi() / 96.0
    }

    /// Draws `placements` from this atlas.
    ///
    /// Nearest sampling: the tiles were rasterized at this surface's own scale and land on
    /// the pixel grid, so filtering them would be work with no result.
    pub fn draw(&self, draw: &Draw<'_>, placements: &Placements) {
        draw.sprites(&placements.batch, &self.atlas, Interp::Nearest);
    }
}

/// One consumer's selection from a [`Tiles`]: which tiles are on screen this frame, and
/// where.
///
/// A placement pass is [`begin`](Self::begin), one [`place`](Self::place) per visible tile,
/// then [`commit`](Self::commit). The storage is reserved at construction and reused, so the
/// pass allocates nothing.
pub struct Placements {
    batch: SpriteBatch,
    destinations: Vec<Rect>,
    sources: Vec<[u32; 4]>,
    used: usize,
    dirty: bool,
}

impl Placements {
    /// Starts a placement pass, retiring the previous one.
    ///
    /// Real-time: resets the placement cursor and allocates nothing.
    pub fn begin(&mut self) {
        self.used = 0;
    }

    /// Places `tiles`' tile `index` with the top-left of its ink at `at`, snapped to the
    /// pixel grid.
    ///
    /// Snapping is here rather than at the caller because the tile's pixels were rasterized
    /// on the grid: a destination off it resamples them and the text comes out soft.
    ///
    /// A placement past the reserved slots is dropped, and one naming a tile outside the
    /// vocabulary draws nothing.
    ///
    /// Real-time: writes reserved storage and allocates nothing.
    pub fn place(&mut self, tiles: &Tiles, index: usize, at: Vector2) {
        let (Some(tile), true) = (tiles.tiles.get(index), self.used < self.destinations.len())
        else {
            return;
        };
        let scale = tiles.scale();
        let snap = |v: f32| (v * scale).round() / scale;
        let pad = 1.0 / scale;
        let (x, y) = (snap(at.x) - pad, snap(at.y) - pad);
        let destination = Rect::new(x, y, x + tile.size.x, y + tile.size.y);
        if self.destinations[self.used] != destination || self.sources[self.used] != tile.source {
            self.destinations[self.used] = destination;
            self.sources[self.used] = tile.source;
            self.dirty = true;
        }
        self.used += 1;
    }

    /// Returns how many placements the last pass made.
    #[must_use]
    pub fn placed(&self) -> usize {
        self.used
    }

    /// Publishes this pass's placements to the batch.
    /// Leaves native storage untouched when all placements match the committed pass.
    ///
    /// # Errors
    ///
    /// Fails when the sprite batch refuses the set.
    pub fn commit(&mut self) -> Result<()> {
        for destination in &mut self.destinations[self.used..] {
            if *destination != Rect::default() {
                *destination = Rect::default();
                self.dirty = true;
            }
        }
        if self.dirty {
            self.batch.set_tiles(&self.destinations, &self.sources)?;
            self.dirty = false;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UI: FamilyId = FamilyId(0);

    fn vocabulary() -> [(&'static str, f32); 5] {
        [
            ("0", 1.0),
            ("1", 1.0),
            ("20k", 0.65),
            ("-12", 0.65),
            ("W", 0.4),
        ]
    }

    fn build(gpu: &Gpu, dpi: f32) -> Tiles {
        let words: Vec<_> = vocabulary()
            .iter()
            .map(|&(text, alpha)| Word {
                text,
                spec: FontSpec::new(UI, 11.0),
                ink: Scrgb {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: alpha,
                },
            })
            .collect();
        let mut pass = gpu.pass().unwrap();
        let result = Tiles::build(gpu, &mut pass, dpi, FontLadder::new(["Segoe UI"]), 256.0, &words)
            .expect("atlas at every scale");
        pass.end().unwrap();
        result
    }

    #[test]
    fn a_placement_pass_reuses_its_storage_and_stops_at_the_reserved_slots() {
        let gpu = Gpu::for_presentation().expect("native Direct2D device");
        for dpi in [96.0, 144.0, 192.0] {
            let tiles = build(&gpu, dpi);
            assert_eq!(tiles.len(), vocabulary().len());
            let (w, h) = tiles.size_px();
            assert!(w * h * 8 <= 1024 * 1024, "{w}x{h} FP16 pixels at {dpi} DPI");

            let mut placements = tiles.placements(&gpu, 4).expect("placement buffer");
            let backing = (
                placements.destinations.as_ptr(),
                placements.sources.as_ptr(),
            );
            for pass in 0..64 {
                placements.begin();
                // More placements than slots: the surplus is dropped rather than growing
                // storage the per-frame path reads.
                for i in 0..tiles.len() {
                    placements.place(&tiles, i, Vector2::new(pass as f32 + i as f32, 3.0));
                }
                // A tile outside the vocabulary places nothing at all.
                placements.place(&tiles, tiles.len(), Vector2::new(0.0, 0.0));
                assert_eq!(placements.placed(), 4);
                placements.commit().expect("committed placements");
                assert_eq!(
                    backing,
                    (
                        placements.destinations.as_ptr(),
                        placements.sources.as_ptr()
                    ),
                    "a placement pass must not reallocate"
                );
            }
            // An advance is the run's own, and it is the same word for word whatever the
            // scale: a caller lays text out in DIPs.
            assert!(tiles.advance(2).x > tiles.advance(0).x);
        }
    }

    #[test]
    fn unchanged_placements_stay_clean_and_shorter_passes_hide_retired_tiles() {
        let gpu = Gpu::for_presentation().expect("native Direct2D device");
        for dpi in [96.0, 144.0, 192.0] {
            let tiles = build(&gpu, dpi);
            let mut placements = tiles.placements(&gpu, 4).unwrap();
            for _ in 0..2 {
                placements.begin();
                placements.place(&tiles, 0, Vector2::new(4.0, 4.0));
                placements.place(&tiles, 1, Vector2::new(20.0, 4.0));
                placements.commit().unwrap();
                assert!(!placements.dirty);
            }
            placements.begin();
            placements.place(&tiles, 0, Vector2::new(4.0, 4.0));
            placements.place(&tiles, 1, Vector2::new(20.0, 4.0));
            assert!(!placements.dirty, "identical text must not rewrite native sprites");
            placements.commit().unwrap();

            placements.begin();
            placements.place(&tiles, 2, Vector2::new(4.0, 4.0));
            assert!(placements.dirty, "a replacement glyph must be published");
            placements.commit().unwrap();
            assert_eq!(placements.destinations[1], Rect::default());
            placements.begin();
            placements.commit().unwrap();

            let target = gpu.offscreen((64, 32), dpi, Opacity::Translucent).unwrap();
            let mut pass = gpu.pass().unwrap();
            let draw = pass.draw(&target);
            draw.clear(Scrgb::TRANSPARENT);
            tiles.draw(&draw, &placements);
            drop(draw);
            pass.end().unwrap();
            let pixels = gpu.read(&target).unwrap();
            for y in 0..32 {
                for x in 0..64 {
                    assert_eq!(pixels.pixel(x, y)[3], 0.0, "retired glyph remains visible");
                }
            }
        }
    }

    /// Two consumers of one atlas keep their own selections, so the second to place does
    /// not take the first one's pixels with it.
    #[test]
    fn placements_over_one_atlas_are_independent() {
        let gpu = Gpu::for_presentation().expect("native Direct2D device");
        let tiles = build(&gpu, 144.0);
        let mut first = tiles.placements(&gpu, 4).expect("first buffer");
        let mut second = tiles.placements(&gpu, 4).expect("second buffer");
        first.begin();
        first.place(&tiles, 0, Vector2::new(4.0, 4.0));
        first.place(&tiles, 1, Vector2::new(20.0, 4.0));
        first.commit().unwrap();
        second.begin();
        second.place(&tiles, 2, Vector2::new(0.0, 0.0));
        second.commit().unwrap();
        assert_eq!(first.placed(), 2);
        assert_eq!(second.placed(), 1);
        assert_ne!(first.sources[0], second.sources[0]);
        assert_ne!(first.destinations[0], second.destinations[0]);
    }

    #[test]
    fn a_word_wider_than_the_atlas_is_refused() {
        let gpu = Gpu::for_presentation().expect("native Direct2D device");
        let words = [Word {
            text: "a word no packing places",
            spec: FontSpec::new(UI, 11.0),
            ink: Scrgb {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        }];
        let mut pass = gpu.pass().unwrap();
        let refused = Tiles::build(&gpu, &mut pass, 96.0, FontLadder::new(["Segoe UI"]), 8.0, &words);
        assert!(refused.is_err());
    }
}
