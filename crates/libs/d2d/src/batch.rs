//! Sprite batches — N rectangles sampled from one target, drawn as one primitive.
//!
//! A field of rectangles drawn with `fill` is one Direct2D primitive per rectangle, and
//! the per-primitive overhead is the cost rather than the pixels. Direct2D's own guidance
//! is explicit: sprite batches "incur dramatically less per-image CPU overhead" and are
//! the tool when an app "needs to draw hundreds or thousands of images every frame".
//!
//! The case this exists for is a spectrum's bars. Eighty-five bars, each a body and a cap,
//! is a hundred and seventy primitives a frame — and the body's gradient has to be re-aimed
//! at each bar in turn, because the fade is anchored to the bar's own top edge rather than
//! to the plot. As two batches it is two draws, and the per-bar fade comes for free: a
//! source holding the ramp once, stretched into each destination rectangle, *is* a fade
//! normalized to each bar.
//!
//! # Carry destination rectangles and nothing else
//!
//! A sprite has four properties — destination rectangle, source rectangle, colour,
//! transform — and Direct2D allocates a parallel array for any property *any* sprite in
//! the batch sets, defaulting every other sprite in it. Passing identity transforms "for
//! symmetry" costs a matrix per sprite per frame. So [`set`](SpriteBatch::set) writes
//! destination rectangles only, which is the configuration the documentation calls the
//! fastest, and a field wanting two source images is two batches rather than one with
//! per-sprite source rectangles.

use super::*;

/// How a batch or a blit samples its source when the destination is a different size.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Interp {
    /// Blend between texels — the right choice when the source is stretched, so a ramp
    /// held in a tall thin target stays smooth at any bar height.
    #[default]
    Linear,
    /// Take the nearest texel. Cheaper, and correct for a source meant to land
    /// pixel-for-pixel or one that is a single flat colour.
    Nearest,
}

impl Interp {
    pub(crate) fn bitmap(self) -> D2D1_BITMAP_INTERPOLATION_MODE {
        match self {
            Self::Linear => D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            Self::Nearest => D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        }
    }

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
    /// The batch size some drivers cap at.
    ///
    /// Microsoft's own Direct2D wrapper splits at this count and issues an explicit
    /// `Flush` between the halves to work around older Qualcomm drivers — the `Flush`
    /// being needed because Direct2D otherwise re-batches the calls that were just
    /// manually unbatched. Windows 11 includes Snapdragon machines, so it is on this
    /// stack's floor.
    ///
    /// Nothing here splits, because a `Flush` with a layer outstanding puts the target into
    /// an error state and the largest field this application draws is 170 sprites. A batch
    /// over the ceiling trips a debug assertion instead of silently depending on the
    /// driver.
    pub const CEILING: u32 = 256;

    /// How many sprites the batch holds.
    #[must_use]
    pub fn len(&self) -> usize {
        unsafe { self.0.GetSpriteCount() as usize }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Replaces the contents with `rects`, each sampling the whole source.
    ///
    /// Grows and shrinks in place: the sprites that already exist are rewritten and only
    /// the surplus is added, so a field whose length is stable between relayouts never
    /// reallocates. Shrinking is the one case that pays, because Direct2D can only clear a
    /// batch whole — and a bar count changes on relayout, not per frame.
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
    /// An empty sprite batch.
    pub fn batch(&self) -> Result<SpriteBatch> {
        Ok(SpriteBatch(unsafe { self.ctx().CreateSpriteBatch()? }))
    }
}
