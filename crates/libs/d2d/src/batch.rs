//! Sprite batches: N rectangles sampled from one target, drawn as one primitive.
//!
//! A field of rectangles drawn with `fill` is one Direct2D primitive per rectangle, and the
//! per-primitive overhead dominates the pixel cost. A batch draws the whole field in one
//! call, and a source holding a ramp once, stretched into each destination rectangle, gives
//! every rectangle the same fade normalized to its own extent.
//!
//! # Carry destination rectangles and nothing else
//!
//! A sprite has four properties — destination rectangle, source rectangle, colour,
//! transform — and Direct2D allocates a parallel array for any property *any* sprite in
//! the batch sets, defaulting every other sprite in it. [`set`](SpriteBatch::set) writes
//! destination rectangles only, so no sprite pays for a property it does not use, and a
//! field wanting two source images uses two batches. An atlas explicitly opts into the
//! source-rectangle array through [`set_tiles`](SpriteBatch::set_tiles).

use super::*;

/// How a batch or a blit samples its source when the destination is a different size.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Interp {
    /// Blends between texels, so a stretched source stays smooth at any destination size.
    #[default]
    Linear,
    /// Takes the nearest texel. Cheaper, and exact for a source landing pixel-for-pixel or
    /// one holding a single flat colour.
    Nearest,
}

impl Interp {
    /// The two-value mode `DrawSpriteBatch` takes.
    pub(crate) fn bitmap(self) -> D2D1_BITMAP_INTERPOLATION_MODE {
        match self {
            Self::Linear => D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            Self::Nearest => D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        }
    }

    /// The mode `DrawBitmap` and a bitmap brush take.
    pub(crate) fn image(self) -> D2D1_INTERPOLATION_MODE {
        match self {
            Self::Linear => D2D1_INTERPOLATION_MODE_LINEAR,
            Self::Nearest => D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        }
    }
}

/// A batch of rectangles sampling one target.
///
/// Build it once and rewrite it per frame with [`set`](Self::set): the batch keeps its
/// allocation, so a field that changes every frame allocates nothing after the first. It is
/// a device resource — rebuild it when the device is lost.
pub struct SpriteBatch(ID2D1SpriteBatch, core::cell::Cell<bool>);

impl SpriteBatch {
    /// The sprite count some drivers cap a batch at.
    ///
    /// Splitting a larger batch takes an explicit `Flush` between the halves, since
    /// Direct2D otherwise re-batches the calls that were manually unbatched, and a `Flush`
    /// with a layer outstanding puts the target into an error state. Nothing here splits: a
    /// batch over the ceiling trips a debug assertion.
    pub const CEILING: u32 = 256;

    /// Returns the number of sprites in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        unsafe { self.0.GetSpriteCount() as usize }
    }

    /// Returns `true` when the batch holds no sprites.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Replaces the contents with `rects`, each sampling the whole source.
    ///
    /// Grows and shrinks in place: the sprites that already exist are rewritten and only
    /// the surplus is added, so a field whose length is stable between relayouts allocates
    /// nothing. Shrinking clears the batch and refills it, because Direct2D clears a batch
    /// only as a whole.
    pub fn set(&self, rects: &[Rect]) -> Result<()> {
        self.replace(rects, None)
    }

    /// Replaces the batch with atlas tiles. Sources are pixel `[left, top, right, bottom]`
    /// rectangles; destinations are DIPs. The slices must have equal lengths.
    ///
    /// Keep the count fixed after preparation to avoid growing or clearing native storage.
    /// An unused slot may have an empty destination. Unlike [`set`](Self::set), this
    /// allocates the source-rectangle array as well as the destinations. Switching between
    /// this method and `set` clears and rebuilds the batch.
    pub fn set_tiles(&self, rects: &[Rect], sources: &[[u32; 4]]) -> Result<()> {
        assert_eq!(rects.len(), sources.len());
        self.replace(rects, Some(sources))
    }

    fn replace(&self, rects: &[Rect], sources: Option<&[[u32; 4]]>) -> Result<()> {
        // Switching between whole-source and atlas sampling resets the property arrays.
        // A null SetSprites source leaves existing coordinates unchanged; it cannot reset
        // atlas entries to the whole-source default. Steady-mode updates keep storage.
        if self.1.replace(sources.is_some()) != sources.is_some() {
            unsafe { self.0.Clear() };
        }
        let have = self.len();
        let want = rects.len();
        if want == 0 {
            if have > 0 {
                unsafe { self.0.Clear() };
            }
            return Ok(());
        }
        debug_assert!(
            want as u32 <= Self::CEILING,
            "{want} sprites is over the {} some drivers cap a batch at",
            Self::CEILING
        );
        // `Rect` is a `#[repr(C)]` four-float left/top/right/bottom, which is exactly
        // `D2D_RECT_F` — so the slice is passed at its natural stride rather than copied.
        let ptr = rects.as_ptr().cast::<D2D_RECT_F>();
        let stride = size_of::<Rect>() as u32;
        // A four-u32 array has the same layout as D2D_RECT_U. The length assertion in
        // set_tiles makes both strided slices cover every sprite passed below.
        let src = sources.map(|s| s.as_ptr().cast::<D2D_RECT_U>());
        let src_stride = if src.is_some() {
            size_of::<[u32; 4]>() as u32
        } else {
            0
        };
        if want < have {
            unsafe { self.0.Clear() };
            return self.add(ptr, src, want as u32, stride, src_stride);
        }
        let overlap = have.min(want) as u32;
        if overlap > 0 {
            unsafe {
                self.0
                    .SetSprites(
                        0,
                        overlap,
                        Some(ptr),
                        src,
                        None,
                        None,
                        stride,
                        src_stride,
                        0,
                        0,
                    )
                    .ok()?;
            }
        }
        if want > have {
            // SAFETY: `have < want`, so this offset is in bounds of `rects`.
            let rest = unsafe { ptr.add(have) };
            // SAFETY: the source slice, when supplied, has the same length as rects.
            let src = src.map(|ptr| unsafe { ptr.add(have) });
            return self.add(rest, src, (want - have) as u32, stride, src_stride);
        }
        Ok(())
    }

    fn add(
        &self,
        rects: *const D2D_RECT_F,
        sources: Option<*const D2D_RECT_U>,
        count: u32,
        stride: u32,
        source_stride: u32,
    ) -> Result<()> {
        unsafe {
            self.0
                .AddSprites(
                    count,
                    rects,
                    sources,
                    None,
                    None,
                    stride,
                    source_stride,
                    0,
                    0,
                )
                .ok()
        }
    }

    pub(crate) fn raw(&self) -> &ID2D1SpriteBatch {
        &self.0
    }
}

impl Gpu {
    /// Creates an empty sprite batch.
    pub fn batch(&self) -> Result<SpriteBatch> {
        Ok(SpriteBatch(
            unsafe { self.ctx().CreateSpriteBatch()? },
            core::cell::Cell::new(false),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_sources_survive_rewrites_growth_and_mode_changes() {
        let gpu = Gpu::for_presentation().expect("native Direct2D device");
        for dpi in [96.0, 144.0, 192.0] {
            let scale = dpi / 96.0;
            let atlas = gpu.offscreen((8, 4), dpi, Opacity::Translucent).unwrap();
            let out = gpu.offscreen((8, 4), dpi, Opacity::Translucent).unwrap();
            let red = gpu
                .solid(Scrgb {
                    r: 2.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                })
                .unwrap();
            let green = gpu
                .solid(Scrgb {
                    r: 0.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                })
                .unwrap();
            let mut pass = gpu.pass().unwrap();
            {
                let draw = pass.draw(&atlas);
                draw.clear(Scrgb::TRANSPARENT);
                draw.fill(Rect::new(0.0, 0.0, 4.0 / scale, 4.0 / scale), &red);
                draw.fill(
                    Rect::new(4.0 / scale, 0.0, 8.0 / scale, 4.0 / scale),
                    &green,
                );
            }
            pass.end().unwrap();
            let batch = gpu.batch().unwrap();
            let rects = [
                Rect::new(0.0, 0.0, 4.0 / scale, 4.0 / scale),
                Rect::new(4.0 / scale, 0.0, 8.0 / scale, 4.0 / scale),
            ];
            let sources = [[4, 0, 8, 4], [0, 0, 4, 4]];
            let sample = || {
                let mut pass = gpu.pass().unwrap();
                {
                    let draw = pass.draw(&out);
                    draw.clear(Scrgb::TRANSPARENT);
                    draw.sprites(&batch, &atlas, Interp::Nearest);
                }
                pass.end().unwrap();
                gpu.read(&out).unwrap()
            };
            for count in [1, 2, 2, 1, 2] {
                batch.set_tiles(&rects[..count], &sources[..count]).unwrap();
                let read = sample();
                assert_eq!(read.pixel(1, 1), [0.0, 1.0, 0.0, 1.0]);
                if count == 2 {
                    assert_eq!(read.pixel(6, 1), [2.0, 0.0, 0.0, 1.0]);
                }
            }
            // Same count when changing mode exercises reset rather than the shrink path.
            batch.set(&rects).unwrap();
            let read = sample();
            assert_eq!(read.pixel(0, 1), [2.0, 0.0, 0.0, 1.0]);
            assert_eq!(read.pixel(3, 1), [0.0, 1.0, 0.0, 1.0]);
            batch
                .set_tiles(&[rects[0], Rect::default()], &sources)
                .unwrap();
            let read = sample();
            assert_eq!(read.pixel(1, 1), [0.0, 1.0, 0.0, 1.0]);
            assert_eq!(read.pixel(6, 1), [0.0; 4]);
        }
    }
}
