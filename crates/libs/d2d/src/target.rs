//! The one place a render target is minted.
//!
//! No signature in this module names a pixel format or an alpha mode. Both follow from the
//! content's own [`Opacity`], so an 8-bit target, an `IGNORE` alpha mode under translucent
//! content, and a `PREMULTIPLIED` one under content claiming to be opaque are each
//! unreachable through this API.
//!
//! A target is **allocated in pixels and drawn in DIPs**, and it carries the DPI relating
//! the two. The extent is a pixel count because a cache keyed in DIPs collides two
//! different rasters at 1.5×, and every coordinate is a DIP because that is what layout
//! solves in, what DirectWrite measures in, and what Direct2D scales by default.

use super::*;
use core::mem::ManuallyDrop;

/// A Direct2D render target: FP16, at a known pixel size and a known DPI.
pub struct Target {
    pub(crate) bitmap: ID2D1Bitmap1,
    px: (u32, u32),
    dpi: f32,
    pub(crate) opacity: Opacity,
}

/// Asserts, in debug builds, that `dpi` is inside the range Direct2D's pipeline is designed
/// for.
///
/// A value outside it is a scale factor passed where a DPI was wanted, or a zero from an
/// uninitialized field. Both produce content at the wrong size rather than an error, which
/// is why they are caught at the boundary.
pub(crate) fn check_dpi(dpi: f32) {
    debug_assert!(
        (96.0..=1200.0).contains(&dpi),
        "{dpi} is not a DPI: Direct2D is designed to scale from 96 to 1200"
    );
}

/// What the content does with alpha, named after the content rather than after the API.
///
/// A presented region answers the same question about itself, and the two answers move
/// together: a region that leaves any pixel uncovered is `Translucent` here *and* must not
/// ask for displayable buffers, because it is composed either way.
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
    /// Returns the size in whole pixels: the allocation, and half of a cache key.
    #[must_use]
    pub fn size_px(&self) -> (u32, u32) {
        self.px
    }

    /// Returns the size in DIPs, the box a renderer draws inside.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        let scale = self.dpi / 96.0;
        (self.px.0 as f32 / scale, self.px.1 as f32 / scale)
    }

    /// Returns the DPI this target was built for, which is also the DPI its content is
    /// drawn at: binding the target sets the context from this value.
    #[must_use]
    pub fn dpi(&self) -> f32 {
        self.dpi
    }
}

impl Gpu {
    /// Allocates an offscreen target at an **exact pixel size**, rendered by retargeting the
    /// caller's own open bracket.
    ///
    /// Not a [`Layer`]. A layer is transient: it is torn down when it pops and re-derived
    /// every frame it is in flight. This target holds its contents until whatever
    /// invalidates them does.
    ///
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
    /// `surface` must be a DXGI surface created on **this** device, in
    /// `DXGI_FORMAT_R16G16B16A16_FLOAT`, whose alpha mode matches `opacity`. Direct2D
    /// reports a mismatched alpha mode, but a surface from another device shows up only as
    /// drawing that quietly fails, so that condition is the caller's to hold.
    ///
    /// # Errors
    ///
    /// When `surface` is not a DXGI surface, or Direct2D rejects it as a bitmap source.
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
/// Yields **numbers and not colour**. The output transform is the only source of an
/// `Scrgb`, so a readback reports raw channel values and the caller compares them against
/// the ones it wrote.
pub struct Readback {
    staging: ID2D1Bitmap1,
    px: (u32, u32),
    pitch: u32,
    bits: *const u8,
}

impl Readback {
    /// Returns the size in whole pixels.
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
            // SAFETY: the assertion above puts `x` and `y` inside the mapped extent, the
            // row stride is the pitch the mapping reported, and the mapping is released
            // only when `self` drops — so the offset is inside the mapped region.
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
/// Written out rather than using `f16`, which stable Rust does not provide. The subnormal
/// case is exact arithmetic rather than a shift loop: a subnormal half is `mantissa × 2⁻²⁴`,
/// and both factors are exactly representable in `f32`.
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
        // Above white, and below zero: the two extremes an FP16 target carries.
        assert_eq!(half_to_f32(0x4200), 3.0);
        assert_eq!(half_to_f32(0xb266), -0.199_951_171_875);
        // The smallest subnormal, and the largest.
        assert_eq!(half_to_f32(0x0001), 1.0 / 16_777_216.0);
        assert_eq!(half_to_f32(0x03ff), 1023.0 / 16_777_216.0);
        assert!(half_to_f32(0x7c00).is_infinite());
        assert!(half_to_f32(0x7e00).is_nan());
    }
}

/// Builds the properties every target in this crate is created with: [`FORMAT`], the alpha
/// mode `opacity` implies, and `dpi`.
///
/// The bitmap carries the **display's** DPI and not 96, which is what holds one coordinate
/// space everywhere. A bitmap's DPI decides its DIP extent, and `DrawBitmap`'s source
/// rectangle is in the *source bitmap's* DIPs, so a 96-DPI source under a display-DPI
/// destination gives one call a DIP destination and a pixel source. At the display's DPI
/// both are DIPs.
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
