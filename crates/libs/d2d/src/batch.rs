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
//! field wanting two source images is two batches rather than one batch with per-sprite
//! source rectangles.

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
pub struct SpriteBatch(ID2D1SpriteBatch);

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
        if want < have {
            unsafe { self.0.Clear() };
            return self.add(ptr, want as u32, stride);
        }
        let overlap = have.min(want) as u32;
        if overlap > 0 {
            unsafe {
                self.0
                    .SetSprites(0, overlap, Some(ptr), None, None, None, stride, 0, 0, 0)
                    .ok()?;
            }
        }
        if want > have {
            // SAFETY: `have < want`, so this offset is in bounds of `rects`.
            let rest = unsafe { ptr.add(have) };
            return self.add(rest, (want - have) as u32, stride);
        }
        Ok(())
    }

    fn add(&self, rects: *const D2D_RECT_F, count: u32, stride: u32) -> Result<()> {
        unsafe {
            self.0
                .AddSprites(count, rects, None, None, None, stride, 0, 0, 0)
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
        Ok(SpriteBatch(unsafe { self.ctx().CreateSpriteBatch()? }))
    }
}
