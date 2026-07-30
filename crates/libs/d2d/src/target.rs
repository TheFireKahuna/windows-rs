//! The one place a render target is minted.
//!
//! No signature in this module names a pixel format or an alpha mode. Both are decided
//! here, from the content's own [`Opacity`], which is why a surface allocation cannot be
//! the thing that loses a colour: an 8-bit target, an `IGNORE` alpha mode under
//! translucent content, or a `PREMULTIPLIED` one under content claiming to be opaque are
//! each unreachable through this API rather than merely discouraged.
//!
//! A target is **allocated in pixels and drawn in DIPs**, and it carries the DPI that
//! relates the two. That is the whole of this crate's DPI story: the extent is a pixel
//! count because a cache keyed in DIPs would collide two different rasters at 1.5×, and
//! every coordinate is a DIP because that is what layout solves in, what DirectWrite
//! measures in, and what Direct2D scales by default.

use super::*;
use core::mem::ManuallyDrop;

/// A Direct2D render target: FP16, at a known pixel size and a known DPI.
pub struct Target {
    pub(crate) bitmap: ID2D1Bitmap1,
    px: (u32, u32),
    dpi: f32,
    pub(crate) opacity: Opacity,
}

/// The range Direct2D's pipeline is documented to be designed for. A value outside it is
/// almost always a scale factor passed where a DPI was wanted, or a zero from an
/// uninitialized field — both of which produce content at the wrong size rather than an
/// error, so they are worth catching at the boundary.
pub(crate) fn check_dpi(dpi: f32) {
    debug_assert!(
        (96.0..=1200.0).contains(&dpi),
        "{dpi} is not a DPI: Direct2D is designed to scale from 96 to 1200"
    );
}

/// What the content does with alpha — named after the content rather than after the API,
/// so the honest answer is also the easy one.
///
/// This is the same question a presented region answers about itself, and the two move
/// together: a region that leaves any pixel uncovered is `Translucent` here *and* must
/// not ask for displayable buffers, because it will be composed either way.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Opacity {
    /// Some pixel of the box is not fully covered.
    Translucent,
    /// Every pixel of the box is covered opaquely. Direct2D then spares the time it
    /// would spend rendering an alpha channel nothing reads.
    Opaque,
}

impl Opacity {
    pub(crate) fn d2d(self) -> D2D1_ALPHA_MODE {
        match self {
            Self::Translucent => D2D1_ALPHA_MODE_PREMULTIPLIED,
            Self::Opaque => D2D1_ALPHA_MODE_IGNORE,
        }
    }
}

impl Target {
    /// The target's size in whole pixels — its allocation, and half of a cache key.
    #[must_use]
    pub fn size_px(&self) -> (u32, u32) {
        self.px
    }

    /// The target's size in DIPs: what a renderer draws inside.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        let scale = self.dpi / 96.0;
        (self.px.0 as f32 / scale, self.px.1 as f32 / scale)
    }

    /// The DPI this target was built for, which is also the DPI its content is drawn at —
    /// the two cannot disagree, because binding it sets the context from this value.
    #[must_use]
    pub fn dpi(&self) -> f32 {
        self.dpi
    }
}

impl Gpu {
    /// An offscreen target at an **exact pixel size**: the cached-chrome intermediate,
    /// rendered by retargeting the caller's own open bracket.
    ///
    /// Deliberately not called `layer`. A [`Layer`] is a Direct2D layer — transient, torn
    /// down when it pops, and re-derived every frame it is in flight. This is the
    /// opposite: an intermediate keyed on whatever invalidates it, which survives until
    /// that happens. Confusing the two is how a per-frame cost gets reintroduced after
    /// the work of removing it.
    /// The pixel extent is the allocation and the cache key; `dpi` is what its contents are
    /// drawn at, so a cell built for one display is not silently reused on another.
    pub fn offscreen(&self, px: (u32, u32), dpi: f32, opacity: Opacity) -> Result<Target> {
        check_dpi(dpi);
        let props = properties(dpi, opacity);
        let size = D2D_SIZE_U {
            width: px.0,
            height: px.1,
        };
        let bitmap = unsafe { self.ctx().CreateBitmap(size, None, 0, &props)? };
        Ok(Target {
            bitmap,
            px,
            dpi,
            opacity,
        })
    }

    /// Adopts a buffer another crate allocated — a presentation region's own texture —
    /// as a target.
    ///
    /// `surface` must be a DXGI surface created on **this** device, in [`FORMAT`], whose
    /// alpha mode matches `opacity`. Direct2D reports a mismatched alpha mode, but a
    /// surface from another device is diagnosed only by the drawing quietly failing, so
    /// the requirement is the caller's to hold.
    pub fn adopt(&self, surface: &impl Interface, dpi: f32, opacity: Opacity) -> Result<Target> {
        check_dpi(dpi);
        let surface: IDXGISurface = surface.cast()?;
        let props = properties(dpi, opacity);
        let bitmap = unsafe {
            self.ctx()
                .CreateBitmapFromDxgiSurface(&surface, Some(&props))?
        };
        let size = unsafe { bitmap.GetPixelSize() };
        Ok(Target {
            bitmap,
            px: (size.width, size.height),
            dpi,
            opacity,
        })
    }
}

/// A target's pixels, copied back to the CPU.
///
/// The oracle for everything above: a parity rig comparing a rendered frame against an
/// approved reference, and the proof that an FP16 target really does carry a component
/// above white and one below zero rather than quietly clamping them.
///
/// Deliberately yields **numbers and not colour**. An `Scrgb` comes from the output
/// transform and from nowhere else, and a readback that minted one would be a second
/// source — so this returns raw channel values and the caller compares them to the ones it
/// wrote.
pub struct Readback {
    staging: ID2D1Bitmap1,
    px: (u32, u32),
    pitch: u32,
    bits: *const u8,
}

impl Readback {
    #[must_use]
    pub fn size_px(&self) -> (u32, u32) {
        self.px
    }

    /// The stored channel values at `(x, y)`, in RGBA order.
    ///
    /// **Premultiplied**, because that is how a premultiplied target stores them, where the
    /// caller wrote straight alpha. The two agree wherever alpha is 1.
    ///
    /// # Panics
    ///
    /// If `(x, y)` is outside the target.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> [f32; 4] {
        assert!(x < self.px.0 && y < self.px.1, "pixel out of range");
        let mut out = [0.0; 4];
        for (channel, slot) in out.iter_mut().enumerate() {
            // SAFETY: the row stride is the pitch the mapping reported and the mapping
            // outlives this borrow, so every offset below is inside the mapped region.
            let half = unsafe {
                let offset =
                    y as usize * self.pitch as usize + x as usize * 8 + channel * size_of::<u16>();
                self.bits.add(offset).cast::<u16>().read_unaligned()
            };
            *slot = half_to_f32(half);
        }
        out
    }
}

impl Drop for Readback {
    fn drop(&mut self) {
        unsafe { self.staging.Unmap().ok().ok() };
    }
}

impl Gpu {
    /// Copies `target` back to the CPU.
    ///
    /// Must not be called while a [`Pass`](crate::Pass) has `target` bound: a bitmap being
    /// rendered to cannot also be a copy source.
    pub fn read(&self, target: &Target) -> Result<Readback> {
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: FORMAT,
                alphaMode: target.opacity.d2d(),
            },
            dpiX: target.dpi,
            dpiY: target.dpi,
            // A CPU-readable bitmap cannot be drawn with, and Direct2D requires the two
            // flags to be stated together.
            bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            colorContext: ManuallyDrop::new(None),
        };
        let size = D2D_SIZE_U {
            width: target.px.0,
            height: target.px.1,
        };
        unsafe {
            let staging = self.ctx().CreateBitmap(size, None, 0, &props)?;
            staging.CopyFromBitmap(None, &target.bitmap, None).ok()?;
            let mapped = staging.Map(D2D1_MAP_OPTIONS_READ)?;
            Ok(Readback {
                staging,
                px: target.px,
                pitch: mapped.pitch,
                bits: mapped.bits,
            })
        }
    }
}

/// Decodes an IEEE binary16.
///
/// Written out rather than reached for, because `f16` is not stable on this floor. The
/// subnormal case is exact arithmetic rather than a shift loop: a subnormal half is
/// `mantissa × 2⁻²⁴`, and both factors are exactly representable in `f32`.
fn half_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits >> 15) << 31;
    let exponent = u32::from((bits >> 10) & 0x1f);
    let mantissa = u32::from(bits & 0x3ff);
    match exponent {
        0 if mantissa == 0 => f32::from_bits(sign),
        0 => {
            let value = mantissa as f32 / 16_777_216.0;
            if sign == 0 { value } else { -value }
        }
        0x1f => f32::from_bits(sign | 0x7f80_0000 | (mantissa << 13)),
        _ => f32::from_bits(sign | ((exponent + 112) << 23) | (mantissa << 13)),
    }
}

#[cfg(test)]
mod tests {
    use super::half_to_f32;

    #[test]
    fn halves_decode() {
        assert_eq!(half_to_f32(0x0000), 0.0);
        assert_eq!(half_to_f32(0x8000), -0.0);
        assert_eq!(half_to_f32(0x3c00), 1.0);
        assert_eq!(half_to_f32(0xbc00), -1.0);
        // Above white, and outside Rec.709 — the two the pipeline exists to carry.
        assert_eq!(half_to_f32(0x4200), 3.0);
        assert_eq!(half_to_f32(0xb266), -0.199_951_171_875);
        // The smallest subnormal, and the largest.
        assert_eq!(half_to_f32(0x0001), 1.0 / 16_777_216.0);
        assert_eq!(half_to_f32(0x03ff), 1023.0 / 16_777_216.0);
        assert!(half_to_f32(0x7c00).is_infinite());
        assert!(half_to_f32(0x7e00).is_nan());
    }
}

/// The whole of this crate's format policy, in one value.
///
/// The bitmap carries the **display's** DPI and not 96, which is what makes one coordinate
/// space hold everywhere. A bitmap's DPI decides its DIP extent, and `DrawBitmap`'s source
/// rectangle is in the *source bitmap's* DIPs — so a 96-DPI source under a display-DPI
/// destination would make one call take a DIP destination and a pixel source. At the
/// display's DPI both are DIPs and there is no second space to get wrong.
fn properties(dpi: f32, opacity: Opacity) -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: FORMAT,
            alphaMode: opacity.d2d(),
        },
        dpiX: dpi,
        dpiY: dpi,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
        colorContext: ManuallyDrop::new(None),
    }
}
