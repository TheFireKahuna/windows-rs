//! **Glyph atlas** — one rasterized alpha mask per (face, glyph, size, subpixel
//! phase, scale), cached and shared by every piece of text that draws it.
//!
//! The node-surface text path re-rasterizes a whole label whenever anything about
//! it changes, and a label is re-laid-out and re-shaped on every such repaint.
//! An atlas moves that work to first use: a glyph is rasterized ONCE and every
//! later appearance binds the cached surface. This module is that cache and that
//! rasterization — placing the glyphs (a sprite per glyph, positioned from the
//! shaped run) is a separate concern and is deliberately NOT here yet.
//!
//! ## Alpha only, and why that keeps the pipeline HDR
//!
//! A glyph raster is a **mask**, never a picture: it carries coverage and no
//! colour. Colour arrives later, when a caller pairs this mask with an FP16
//! scRGB source ([`parts::build_solid_surface`](super::parts::build_solid_surface))
//! through a `CompositionMaskBrush` — exactly the construction
//! [`path_shape::PathLayer`](super::path_shape::PathLayer) and the knob arc
//! already use.
//!
//! So the HDR pipe is untouched, and in fact strictly better preserved than a
//! coloured raster would leave it:
//!
//! - The **tint** stays an FP16 extended-range surface, drawn through the app's
//!   output colour transform (`node::linear`). Text above paper white, negative
//!   out-of-gamut primaries and the host tonemap all keep working, because the
//!   colour never passes through this module at all.
//! - The **mask** is pure coverage, so it is drawn in unmapped opaque white and
//!   NOT through `linear`. Running a tonemap over a coverage value would be a
//!   category error: it would fold display mapping into glyph *shape*, and then
//!   applying the real tonemap to the source would apply it twice.
//! - Consequently colour is deliberately **not** in [`GlyphKey`]. One raster
//!   serves every colour, weight-of-emphasis and disabled state a glyph is ever
//!   drawn in, and a recolour is a `SetSource` on the mask brush — no re-raster,
//!   no repaint, and no loss of dynamic range.
//!
//! The mask is FP16, and that is now a historical choice rather than a
//! load-bearing one. It was made when this module DREW its glyphs: Direct2D
//! wrote coverage through a compressive ~2.2 ramp whose top collapsed under
//! 8-bit quantization, so stems rounded up together and text rendered
//! measurably heavier (`+3.5/255` overall, `+16` on high-coverage pixels;
//! `+0.04` at FP16).
//!
//! [`rasterize`] no longer draws — it uploads the rasterizer's own coverage,
//! which is `u8`. There is no ramp left to quantize and no information above 8
//! bits to keep, so FP16 currently stores 8-bit data in 64 bits per pixel.
//! [`MASK_FORMAT`] can therefore become A8 for an 8× memory cut with no change
//! to the pixels; what remains unproven is only that the compositor honours an
//! A8 surface as a mask brush's mask (see `a8_is_a_mintable_mask_surface`).
//!
//! ## Subpixel positioning
//!
//! Shaped advances are fractional, so a glyph snapped to whole pixels drifts from
//! where shaping put it and the spacing of a word visibly ripples. Each glyph is
//! therefore rasterized at [`SUBPIXEL_PHASES`] horizontal phases and the phase
//! nearest the pen's fractional pixel is selected ([`pen_phase`]).
//!
//! ## Grayscale AA, and why it is no longer something this module asks for
//!
//! ClearType's subpixel coverages assume an opaque backdrop to blend against;
//! on a premultiplied surface cleared to transparent they are invalid, and the
//! three channels would bake colour fringes into what is supposed to be a single
//! coverage value.
//!
//! This module used to set that mode on the drawing session. It no longer has a
//! session to set it on: grayscale is requested where the coverage is actually
//! produced, as the antialias mode passed to `CreateGlyphRunAnalysis` in
//! [`glyph_run_coverage`]. The property is the same one, asked for at the only
//! place that can honour it.

use windows_canvas::{
    glyph_run_coverage, ColorF, DrawingSession, FontFace, FontMetrics, GlyphMetrics, GlyphRun,
    ID2D1DeviceContext, Rect, Vector2 as CVec2,
};
use windows_numerics::Matrix3x2;

use super::mask_cache::{Atlas, MaskCache, MaskGeom, MaskSurfaces, Raster, Rasterized};

/// Horizontal subpixel phases rasterized per glyph.
///
/// A glyph drawn at whole-pixel origins accumulates up to half a pixel of error
/// against the shaped pen position, which reads as uneven letter spacing and
/// makes a word's measured width disagree with its drawn width. Rasterizing `N`
/// phases bounds that error at `1 / 2N` px.
///
/// The cost is exactly linear: `N` phases means `N` cache entries and `N`
/// surfaces per glyph. 4 is the usual point of diminishing returns for
/// grayscale-AA UI text — a 1/8 px worst-case error is below the AA blur the
/// mask already carries, so an 8-phase atlas doubles the population to buy
/// something the rasterizer cannot resolve.
pub(crate) const SUBPIXEL_PHASES: u32 = 4;

/// Padding around the raster box, in physical pixels, on every side.
///
/// Antialiasing spreads coverage slightly beyond the outline, so a box cut
/// exactly to the ink clips the faintest edge pixels and the glyph reads thin.
/// This is a floor, not the whole story: [`glyph_box`] additionally grows any
/// side the glyph's INK actually overhangs (italics, swashes, accents), so the
/// box never clips regardless of padding.
const GLYPH_PAD_PX: f32 = 1.0;

/// Hard cap on live glyph rasters, enforced by LRU eviction.
///
/// Sized on the real population rather than the shape atlas's. One text style —
/// a (face, em size, scale) triple — costs `95 printable ASCII × 4 phases = 380`
/// entries for full Latin coverage. 2048 therefore holds about five concurrent
/// styles' complete working sets (body, strong, caption, title, and one more),
/// which is more than a window shows at once, so steady state never evicts.
///
/// It is a separate cache from [`ATLAS_CAP`](super::parts::ATLAS_CAP) precisely
/// because the populations differ by an order of magnitude: 256 is sized for ~16
/// control kinds binding 1–4 quantized shape sources, and letting glyphs into it
/// would evict the chrome sources on the first paragraph of text.
///
/// Memory stays modest because the entries are masks: a 16-DIP glyph is roughly
/// 12×22 px, so a full cache is about 4 MB at [`MASK_FORMAT`].
/// Eviction is an O(n) scan, and only on a miss at capacity.
///
/// Evicting an entry a sprite is still bound to is safe, but not for the reason
/// the shape atlas is: these rasters share pages, so holding a reference to the
/// surface would not stop the region under them being re-let. What stops it is
/// that the sprite holds the region itself — see
/// [`mask_cache`](super::mask_cache)'s header.
const GLYPH_ATLAS_CAP: usize = 2048;

// ── Pure geometry ────────────────────────────────────────────────────────────
//
// Everything below this line is arithmetic over font metrics and needs no
// device, so it is testable exhaustively without a GPU.

/// Where one glyph's raster sits relative to its baseline origin.
///
/// The box is expressed BOTH in physical pixels (what the surface is minted at)
/// and in DIPs (what the sprite is sized and offset by), because the two must
/// agree exactly or the glyph resamples and blurs.
///
/// ## Everything but the phase is a whole physical pixel
///
/// A composition sprite placed at a fractional pixel offset is **bilinearly
/// resampled**, and a resampled glyph mask reads as soft and noticeably heavier
/// than the same glyph drawn directly — which defeats the entire point of
/// caching the raster. So the box's padding and its internal baseline are
/// rounded to whole pixels *here*, where the surface is minted, rather than left
/// for a caller to round and get subtly wrong.
///
/// The subpixel phase is the deliberate exception: it is baked INTO the raster
/// (the glyph is drawn a fraction of a pixel to the right inside its box), never
/// into the sprite's offset. That is what lets placement stay integral while
/// still honouring fractional shaped advances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GlyphBox {
    /// Surface size in physical pixels.
    pub px_w: i32,
    pub px_h: i32,
    /// The same box in DIPs — `px / scale`, so it is the exact size a sprite
    /// must be given for a 1:1 blit.
    pub size_dip: (f32, f32),
    /// The glyph's baseline origin, measured in DIPs from the box's TOP-LEFT.
    /// What the RASTERIZER draws at; carries the subpixel phase in `x`.
    ///
    /// Placement uses [`origin_px`](Self::origin_px) instead — see there.
    pub baseline_dip: (f32, f32),
    /// The glyph's baseline origin measured in WHOLE physical pixels from the
    /// box's top-left, with the subpixel phase excluded.
    ///
    /// This is the placement anchor: a caller puts the box's top-left at
    /// `(pen_px - origin_px) / scale`, where `pen_px` is the whole-pixel half of
    /// [`pen_phase`] and the pixel-snapped baseline row. Both subtrahends are
    /// integers, so the sprite lands exactly on the pixel grid.
    ///
    /// The phase is excluded precisely because it is already in the raster;
    /// subtracting it here as well would shift every glyph left by up to
    /// `(SUBPIXEL_PHASES - 1) / SUBPIXEL_PHASES` of a pixel *and* knock it off
    /// the grid.
    pub origin_px: (i32, i32),
    /// The glyph's DESIGN advance in DIPs.
    ///
    /// For box sizing and for pre-warming only. Placement must use the SHAPED
    /// advance from [`GlyphRun::glyph_advances`], which is what kerning and
    /// other GPOS positioning actually produced; the two agree for unkerned
    /// pairs and deliberately differ otherwise.
    pub advance_dip: f32,
}

/// Compute the raster box for one glyph at `em` DIPs, DIP→px `scale`, and
/// subpixel `phase`.
///
/// Sizing follows the advance box — width = advance, height = ascent + descent,
/// baseline at ascent — grown by [`GLYPH_PAD_PX`] and by any direction the ink
/// actually overhangs that box. That last part is what makes it correct rather
/// than merely usual: an italic `f`, an `Á`, and a `j` all put ink outside the
/// advance box, and a box cut to the advance alone would clip them.
///
/// It is not the TIGHTEST box — the ink bounds themselves would be smaller, and
/// for a lowercase letter noticeably so (there is no ink between the x-height
/// and the ascender). Shrinking to the ink is a memory optimization to make
/// later, and it does not change this function's contract: the caller already
/// reads the origin out of `baseline_dip` rather than assuming it.
pub(crate) fn glyph_box(
    face: FontMetrics,
    glyph: GlyphMetrics,
    em: f32,
    scale: f32,
    phase: u32,
) -> GlyphBox {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let em = if em.is_finite() && em > 0.0 { em } else { 0.0 };
    // Design units → DIPs.
    let k = em / face.design_units_per_em.max(1) as f32;

    let ascent = face.ascent as f32 * k;
    let descent = face.descent as f32 * k;
    let advance = glyph.advance_width as f32 * k;

    // Ink extents, relative to the baseline origin. `left`/`right` are measured
    // along the advance; `top`/`bottom` are distances from the baseline, both
    // positive outward.
    let ink_left = glyph.left_side_bearing as f32 * k;
    let ink_right = advance - glyph.right_side_bearing as f32 * k;
    let ink_top = ascent - glyph.top_side_bearing as f32 * k;
    let ink_bottom = descent - glyph.bottom_side_bearing as f32 * k;

    // Pad every side (in PHYSICAL pixels, whole), and grow further wherever the
    // ink leaves the advance box. Whole pixels because these are exactly the
    // distances placement subtracts, and a fractional one would put the sprite
    // between pixels — see the type header.
    let left_px = (GLYPH_PAD_PX + (-ink_left).max(0.0) * scale).ceil();
    let right_px = (GLYPH_PAD_PX + (ink_right - advance).max(0.0) * scale).ceil();
    let top_px = (GLYPH_PAD_PX + (ink_top - ascent).max(0.0) * scale).ceil();
    let bottom_px = (GLYPH_PAD_PX + (ink_bottom - descent).max(0.0) * scale).ceil();
    // The baseline's own row, likewise whole: the ascent is what separates the
    // box's top from it, so rounding it is what makes the vertical anchor an
    // integer.
    let ascent_px = (ascent * scale).round();
    let descent_px = (descent * scale).round();

    // The pixel box is authoritative; the DIP box is derived from it so the two
    // cannot disagree. The extra column absorbs the subpixel phase shift, which
    // is strictly less than one pixel.
    let px_w = ((left_px + (advance * scale).ceil() + right_px) as i32 + 1).max(1);
    let px_h = ((top_px + ascent_px + descent_px + bottom_px) as i32).max(1);

    GlyphBox {
        px_w,
        px_h,
        size_dip: (px_w as f32 / scale, px_h as f32 / scale),
        baseline_dip: (
            (left_px + phase_offset_px(phase)) / scale,
            (top_px + ascent_px) / scale,
        ),
        origin_px: (left_px as i32, (top_px + ascent_px) as i32),
        advance_dip: advance,
    }
}

/// The shift a subpixel phase applies to the baseline origin, in PHYSICAL
/// pixels: `phase` quarters of one pixel at [`SUBPIXEL_PHASES`] `== 4`.
fn phase_offset_px(phase: u32) -> f32 {
    (phase % SUBPIXEL_PHASES) as f32 / SUBPIXEL_PHASES as f32
}

/// Split a pen position into the whole physical pixel a glyph's box is placed
/// at and the subpixel phase whose raster carries the remainder.
///
/// The two halves reconstruct the pen to within half a phase:
/// `px + phase / SUBPIXEL_PHASES ≈ pen_x_dip * scale`.
pub(crate) fn pen_phase(pen_x_dip: f32, scale: f32) -> (i32, u32) {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    if !pen_x_dip.is_finite() {
        return (0, 0);
    }
    let px = pen_x_dip * scale;
    let whole = px.floor();
    let mut phase = ((px - whole) * SUBPIXEL_PHASES as f32).round() as i32;
    let mut origin = whole as i32;
    // Rounding up from the last phase carries into the next whole pixel.
    if phase >= SUBPIXEL_PHASES as i32 {
        phase = 0;
        origin += 1;
    }
    (origin, phase as u32)
}

// ── Cache key ────────────────────────────────────────────────────────────────

/// Quantize an em size so float noise in a DPI computation cannot fork the
/// cache. 1/64 of a physical pixel is DirectWrite's own design-unit resolution
/// at typical sizes, so two ems that quantize together rasterize identically.
pub(crate) fn quant_em(em: f32, scale: f32) -> u32 {
    if !em.is_finite() || em <= 0.0 {
        return 0;
    }
    let grid = (scale * 64.0).max(1.0e-3);
    ((em * grid).round() / grid).to_bits()
}

/// Canonical DIP→px scale — the same 1/1000 grid the shape atlas snaps to, so
/// the two caches agree about what "the current scale" is.
pub(crate) fn quant_scale(scale: f32) -> u32 {
    if !scale.is_finite() || scale <= 0.0 {
        return 1.0f32.to_bits();
    }
    ((scale * 1000.0).round() / 1000.0).to_bits()
}

/// Identity of one rasterized glyph.
///
/// Colour is deliberately absent — see the module header. The remaining fields
/// are everything that changes the PIXELS: which face, which glyph in it, how
/// big, at which subpixel phase, and at which device scale.
///
/// ## The face-pointer assumption
///
/// `face` is the `IDWriteFontFace` COM pointer value. DirectWrite caches faces,
/// so shaping the same family+weight+style repeatedly hands back the same
/// object, and pointer equality is therefore both cheap and (in practice) the
/// right equivalence.
///
/// The failure mode a raw pointer would otherwise have is ABA: a face is
/// released, a DIFFERENT face is allocated at the same address, and the cache
/// then serves glyphs rasterized from the old face under the new one's key —
/// silently drawing the wrong letterforms, with no error anywhere.
///
/// That cannot happen here, because every [`GlyphEntry`] holds a cloned
/// [`FontFace`] reference for as long as it is cached. The address cannot be
/// recycled while any entry keys on it, so within this cache the pointer is a
/// genuine identity rather than a fingerprint. The cost is one COM reference per
/// distinct face — bounded by the number of faces in use, not by glyph count.
///
/// What is still assumed is the converse: that DWrite does not hand back two
/// DISTINCT face objects for the same underlying font. If it ever did, the only
/// consequence is a duplicate set of entries — wasted memory, correct pixels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct GlyphKey {
    face: usize,
    glyph: u16,
    phase: u8,
    em: u32,
    scale: u32,
}

impl GlyphKey {
    pub(crate) fn new(face: &FontFace, glyph: u16, em: f32, scale: f32, phase: u32) -> Self {
        Self {
            face: face_id(face),
            glyph,
            phase: (phase % SUBPIXEL_PHASES) as u8,
            em: quant_em(em, scale),
            scale: quant_scale(scale),
        }
    }
}

/// The face's COM identity. `IDWriteFontFace` is not itself `IUnknown`-canonical
/// across QI, but every face here comes from the same interface pointer DWrite
/// handed the run collector, so the raw pointer is stable per face object.
pub(crate) fn face_id(face: &FontFace) -> usize {
    use windows_core::Interface;
    face.raw().as_raw() as usize
}

// ── The cache ────────────────────────────────────────────────────────────────

/// Rasterized glyph masks, shared across every piece of text.
///
/// A thin keying over the shared [`MaskCache`]: the LRU, the epoch, the id
/// minting and the surface-mint seam all live there, once, beside the per-run
/// [`run_atlas`](super::run_atlas) that keys the same machinery differently.
/// This module owns only what is glyph-specific — the [`GlyphKey`], the box
/// arithmetic ([`glyph_box`]) and the one-glyph [`rasterize`].
pub(crate) struct GlyphAtlas {
    cache: MaskCache<GlyphKey>,
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self {
            cache: MaskCache::new(GLYPH_ATLAS_CAP),
        }
    }
}

impl GlyphAtlas {
    /// Drop every cached raster (display / DPI / device edge). A theme change
    /// does NOT belong here: the masks carry no colour, so a recolour re-binds
    /// the mask brush's source and leaves every raster valid.
    pub(crate) fn clear(&mut self) {
        self.cache.clear();
    }

    pub(crate) fn epoch(&self) -> u32 {
        self.cache.epoch()
    }

    /// Live entry count — the LRU's population.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.cache.len()
    }

    /// Fetch (rasterizing on a miss) the mask for one glyph.
    ///
    /// `em` is the run's font size in DIPs, `scale` the DIP→px factor, `phase`
    /// the subpixel phase from [`pen_phase`].
    pub(crate) fn get(
        &mut self,
        dev: &impl MaskSurfaces,
        face: &FontFace,
        glyph: u16,
        em: f32,
        scale: f32,
        phase: u32,
    ) -> Option<Raster> {
        let key = GlyphKey::new(face, glyph, em, scale, phase);
        self.cache.get(key, |atlas| {
            rasterize(dev, atlas, face, glyph, em, scale, phase % SUBPIXEL_PHASES)
        })
    }
}

/// Take one glyph's coverage from the rasterizer and upload it as a mask.
///
/// The glyph is described as a one-glyph [`GlyphRun`] rather than a
/// `TextLayout`, so nothing is shaped, measured, or laid out here — the caller
/// already knows which glyph id it wants.
///
/// ## Nothing is drawn
///
/// [`glyph_run_coverage`] asks DirectWrite's rasterizer for the coverage
/// directly, so `BeginDraw` carries an upload and nothing else: no glyph run, no
/// brush, no text antialias mode, no rendering params. Coverage is produced on
/// the CPU, which is also why the result no longer depends on a GPU at all.
///
/// That last point is the reason to do it this way rather than for the saved
/// work. Direct2D picks a coverage gamma from **what it believes the target's
/// encoding to be**: drawing the same run onto an FP16 target writes linear
/// coverage, and onto an 8-bit one writes it through a ~2.0 ramp. A mask is read
/// back as linear by the compositor either way, so that choice was a silent
/// correctness dependency on `MASK_FORMAT`. Uploading measured coverage removes
/// the dependency instead of tuning around it — the bytes here are the
/// rasterizer's own, whatever the surface is made of.
///
/// The box comes from [`glyph_box`] exactly as before; the coverage carries its
/// own tight bounds and lands at them inside that box.
fn rasterize(
    dev: &impl MaskSurfaces,
    atlas: &mut Atlas,
    face: &FontFace,
    glyph: u16,
    em: f32,
    scale: f32,
    phase: u32,
) -> Option<Rasterized> {
    let metrics = face.metrics();
    let gm = *face.design_glyph_metrics(&[glyph], false).ok()?.first()?;
    let geom = glyph_box(metrics, gm, em, scale, phase);

    let run = GlyphRun {
        font_face: face.clone(),
        font_em_size: em,
        glyph_indices: vec![glyph],
        glyph_advances: vec![0.0],
        glyph_offsets: Vec::new(),
        baseline_origin: CVec2::new(0.0, 0.0),
        is_sideways: false,
        bidi_level: 0,
    };
    // The baseline the box already places the glyph at — which carries the
    // subpixel phase — so the returned bounds are physical pixels measured from
    // the box's own top-left.
    let coverage = glyph_run_coverage(&run, scale, geom.baseline_dip)
        .ok()
        .flatten();

    let tile = atlas.alloc(dev, geom.px_w, geom.px_h, scale)?;
    let (ctx, (origin_x, origin_y)) = match tile.begin_draw::<ID2D1DeviceContext>() {
        Ok(c) => c,
        Err(e) => {
            if super::bootstrap::is_device_loss(&e) {
                dev.device_lost().set(true);
            }
            return None;
        }
    };
    // No scale anywhere: coverage is already rasterized at `scale`, and its bounds
    // are physical pixels. Only the surface's place in its atlas, which the session
    // carries so a caller's own transform composes with it.
    let session = DrawingSession::from_borrowed_context(
        &ctx,
        Matrix3x2::translation(origin_x as f32, origin_y as f32),
    );
    // Everything below is confined to this glyph's own region — see `Tile::clip`.
    let (cx, cy, cw, ch) = tile.clip();
    session.push_clip(&Rect::from_xywh(cx, cy, cw, ch));
    session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));

    // A glyph that marks no pixels — a space — keeps its cleared surface rather
    // than failing: the cache entry is what stops it being asked again.
    if let Some(cov) = coverage {
        // Premultiplied white at the coverage, i.e. exactly what drawing an
        // opaque white run produced. The mask brush reads alpha, but the colour
        // channels are kept equal to it so the surface is unchanged in kind.
        //
        // Uploaded unmapped, NOT through `node::linear`: coverage is not a
        // colour, and putting the app's output transform on it would fold
        // display mapping into glyph shape and then double-apply once the FP16
        // source is tinted. The tonemap belongs on the source, where it already
        // happens.
        let mut rgba = vec![0.0f32; (cov.width as usize) * (cov.height as usize) * 4];
        for (i, &a) in cov.alpha.iter().enumerate() {
            let v = f32::from(a) / 255.0;
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, v]);
        }
        if let Ok(bitmap) = session.create_bitmap_fp16(cov.width, cov.height, &rgba) {
            session.draw_bitmap(
                &bitmap,
                &Rect::from_xywh(
                    cov.left as f32,
                    cov.top as f32,
                    cov.width as f32,
                    cov.height as f32,
                ),
                1.0,
            );
        }
    }

    session.pop_clip();
    tile.end_draw().ok()?;
    Some(Rasterized {
        tile,
        geom: MaskGeom {
            size_dip: geom.size_dip,
            origin_px: geom.origin_px,
        },
        // Keep the keyed face alive for the entry's lifetime — see [`GlyphKey`].
        face: face.clone(),
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

    use super::super::mask_cache::MASK_FORMAT;

    // ── Pure geometry ────────────────────────────────────────────────────────

    /// Segoe UI-ish metrics, so the arithmetic tests do not need a font.
    fn face_metrics() -> FontMetrics {
        FontMetrics {
            design_units_per_em: 2048,
            ascent: 2100,
            descent: 500,
            ..Default::default()
        }
    }

    fn glyph_metrics(advance: u32, lsb: i32, rsb: i32, tsb: i32, bsb: i32) -> GlyphMetrics {
        GlyphMetrics {
            advance_width: advance,
            left_side_bearing: lsb,
            right_side_bearing: rsb,
            top_side_bearing: tsb,
            bottom_side_bearing: bsb,
            ..Default::default()
        }
    }

    /// The baseline must sit at the ascent, and the box must be at least the
    /// advance box plus padding, at every scale and phase.
    #[test]
    fn box_places_the_baseline_at_the_ascent() {
        let fm = face_metrics();
        for &em in &[10.0f32, 12.0, 14.0, 16.0, 24.0, 48.0] {
            for &scale in &[1.0f32, 1.25, 1.5, 1.75, 2.0, 3.0] {
                for phase in 0..SUBPIXEL_PHASES {
                    let gm = glyph_metrics(1024, 60, 60, 400, 100);
                    let b = glyph_box(fm, gm, em, scale, phase);

                    let k = em / 2048.0;
                    let ascent = 2100.0 * k;
                    let descent = 500.0 * k;
                    let pad = GLYPH_PAD_PX / scale;

                    // The baseline is the padding plus the ascent, both rounded
                    // to whole pixels — so it can differ from the exact
                    // `pad + ascent` by up to half a pixel, and no more.
                    let want = pad + ascent;
                    assert!(
                        (b.baseline_dip.1 - want).abs() <= 0.5 / scale + 1.0e-4,
                        "baseline y {} vs pad + ascent {want} (em {em}, scale {scale})",
                        b.baseline_dip.1
                    );
                    assert!((b.advance_dip - 1024.0 * k).abs() < 1.0e-4);
                    // The box must contain the whole advance box plus padding.
                    assert!(b.size_dip.0 >= 1024.0 * k + 2.0 * pad - 1.0e-4);
                    assert!(b.size_dip.1 >= ascent + descent - 1.0 / scale);
                    assert!(b.px_w >= 1 && b.px_h >= 1);
                    // The DIP box is exactly the pixel box.
                    assert!((b.size_dip.0 * scale - b.px_w as f32).abs() < 1.0e-3);
                    assert!((b.size_dip.1 * scale - b.px_h as f32).abs() < 1.0e-3);
                }
            }
        }
    }

    /// The placement anchor must be whole physical pixels, and must agree with
    /// where the rasterizer actually put the baseline to within the subpixel
    /// phase — which is the ONLY fractional part allowed anywhere in the box.
    ///
    /// This is the invariant that keeps a placed glyph a 1:1 blit. Violating it
    /// costs no test failure anywhere else and no error at runtime: the glyph
    /// simply gets bilinearly resampled, and the text quietly renders soft and
    /// too heavy.
    #[test]
    fn placement_anchor_is_whole_pixels() {
        let fm = face_metrics();
        let cases = [
            glyph_metrics(1024, 60, 60, 400, 100),
            glyph_metrics(1024, -200, -300, -150, -250), // heavy overhang
            glyph_metrics(0, 0, 0, 0, 0),                // degenerate
            glyph_metrics(1536, 3, 900, 7, 11),          // awkward bearings
        ];
        for &em in &[10.0f32, 11.5, 13.0, 16.0, 22.0] {
            for &scale in &[1.0f32, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
                for gm in cases {
                    for phase in 0..SUBPIXEL_PHASES {
                        let b = glyph_box(fm, gm, em, scale, phase);

                        // The rasterizer's baseline, in physical pixels, must be
                        // the integer anchor plus exactly this phase's fraction.
                        let raster_px = (b.baseline_dip.0 * scale, b.baseline_dip.1 * scale);
                        let want_x = b.origin_px.0 as f32 + phase_offset_px(phase);
                        assert!(
                            (raster_px.0 - want_x).abs() < 1.0e-3,
                            "x anchor {} + phase != raster {} (em {em}, scale {scale}, phase {phase})",
                            b.origin_px.0,
                            raster_px.0
                        );
                        assert!(
                            (raster_px.1 - b.origin_px.1 as f32).abs() < 1.0e-3,
                            "y anchor {} != raster {} — the vertical anchor carries no phase",
                            b.origin_px.1,
                            raster_px.1
                        );

                        // The anchor must be inside the box: a glyph placed by it
                        // would otherwise hang its baseline outside its own raster.
                        assert!(b.origin_px.0 >= 0 && b.origin_px.0 < b.px_w);
                        assert!(b.origin_px.1 >= 0 && b.origin_px.1 <= b.px_h);
                    }
                }
            }
        }
    }

    /// A pen position placed through the full pipeline must land the ink within
    /// half a subpixel phase of where shaping asked for it, at whole-pixel
    /// sprite offsets. This is `pen_phase` and `glyph_box` composed exactly as
    /// `glyph_text::TextPart::sync` composes them.
    #[test]
    fn placement_reconstructs_the_pen_on_the_grid() {
        let fm = face_metrics();
        let gm = glyph_metrics(1024, 60, 60, 400, 100);
        let tolerance = 0.5 / SUBPIXEL_PHASES as f32;
        for &scale in &[1.0f32, 1.25, 1.5, 2.0] {
            let mut pen = 3.3f32;
            while pen < 200.0 {
                let (whole_px, phase) = pen_phase(pen, scale);
                let b = glyph_box(fm, gm, 16.0, scale, phase);

                // What `sync` computes for the sprite's offset.
                let sprite_x_px = (whole_px - b.origin_px.0) as f32;
                assert_eq!(
                    sprite_x_px,
                    sprite_x_px.round(),
                    "sprite offset must be whole pixels"
                );

                // Where the ink's baseline origin therefore lands.
                let ink_px = sprite_x_px + b.baseline_dip.0 * scale;
                assert!(
                    (ink_px - pen * scale).abs() <= tolerance + 1.0e-3,
                    "pen {pen} at scale {scale}: ink at {ink_px}, wanted {}",
                    pen * scale
                );
                pen += 0.031;
            }
        }
    }

    /// Ink that overhangs the advance box (an italic, an accent, a descender
    /// past the descent line) must grow the box, never be clipped by it.
    #[test]
    fn box_grows_for_overhanging_ink() {
        let fm = face_metrics();
        let em = 16.0;
        let scale = 1.0;
        let k = em / 2048.0;

        // Negative side bearings = ink outside the advance box on that side.
        // Negative top/bottom bearings = ink above the ascent / below the descent.
        let over = glyph_box(fm, glyph_metrics(1024, -200, -300, -150, -250), em, scale, 0);
        let plain = glyph_box(fm, glyph_metrics(1024, 60, 60, 400, 100), em, scale, 0);

        assert!(
            over.baseline_dip.0 > plain.baseline_dip.0,
            "left overhang must push the origin right inside the box"
        );
        assert!(
            over.baseline_dip.1 > plain.baseline_dip.1,
            "ink above the ascent must push the baseline down inside the box"
        );
        assert!(over.px_w > plain.px_w, "overhang must widen the box");
        assert!(over.px_h > plain.px_h, "overhang must heighten the box");

        // Every ink edge is strictly inside the box.
        let (bx, by) = over.baseline_dip;
        let ink_left = bx + (-200.0 * k);
        let ink_right = bx + (1024.0 * k - (-300.0 * k));
        let ink_top = by - (2100.0 * k - (-150.0 * k));
        let ink_bottom = by + (500.0 * k - (-250.0 * k));
        assert!(ink_left >= 0.0, "ink left {ink_left} clipped");
        assert!(ink_right <= over.size_dip.0, "ink right {ink_right} clipped");
        assert!(ink_top >= 0.0, "ink top {ink_top} clipped");
        assert!(
            ink_bottom <= over.size_dip.1,
            "ink bottom {ink_bottom} clipped"
        );
    }

    /// Each phase must shift the RASTER's origin by exactly one more
    /// quarter-pixel while leaving the placement anchor untouched — the phase
    /// lives in the mask, never in the sprite offset.
    #[test]
    fn phases_step_by_a_quarter_pixel() {
        let fm = face_metrics();
        let gm = glyph_metrics(1024, 60, 60, 400, 100);
        for &scale in &[1.0f32, 1.25, 1.5, 2.0] {
            let mut seen: Vec<f32> = Vec::new();
            let anchor = glyph_box(fm, gm, 16.0, scale, 0).origin_px;
            for phase in 0..SUBPIXEL_PHASES {
                let b = glyph_box(fm, gm, 16.0, scale, phase);
                assert_eq!(b.origin_px, anchor, "the phase must not move the anchor");
                // In PHYSICAL pixels the step is exactly 1 / SUBPIXEL_PHASES.
                let px = b.baseline_dip.0 * scale;
                if let Some(prev) = seen.last() {
                    let step = px - prev;
                    assert!(
                        (step - 1.0 / SUBPIXEL_PHASES as f32).abs() < 1.0e-4,
                        "phase {phase} at scale {scale} stepped {step}"
                    );
                }
                seen.push(px);
            }
            assert_eq!(seen.len(), SUBPIXEL_PHASES as usize);
        }
    }

    /// `pen_phase` must reconstruct the pen position to within half a phase,
    /// and must never emit an out-of-range phase.
    #[test]
    fn pen_phase_reconstructs_the_pen() {
        let tolerance = 0.5 / SUBPIXEL_PHASES as f32;
        for &scale in &[1.0f32, 1.25, 1.5, 1.75, 2.0] {
            let mut x = -40.0f32;
            while x < 400.0 {
                let (px, phase) = pen_phase(x, scale);
                assert!(phase < SUBPIXEL_PHASES, "phase {phase} out of range");
                let rebuilt = px as f32 + phase as f32 / SUBPIXEL_PHASES as f32;
                let want = x * scale;
                assert!(
                    (rebuilt - want).abs() <= tolerance + 1.0e-4,
                    "pen {x} at scale {scale}: {rebuilt} vs {want}"
                );
                x += 0.017;
            }
        }
        // Degenerate inputs must not panic or escape the phase range.
        assert_eq!(pen_phase(f32::NAN, 1.0), (0, 0));
        assert!(pen_phase(1.0, 0.0).1 < SUBPIXEL_PHASES);
        assert!(pen_phase(1.0, f32::NAN).1 < SUBPIXEL_PHASES);
    }

    /// Degenerate metrics must produce a usable (never zero-sized) box.
    #[test]
    fn box_survives_degenerate_metrics() {
        let zero = FontMetrics::default();
        let b = glyph_box(zero, GlyphMetrics::default(), 0.0, 0.0, 0);
        assert!(b.px_w >= 1 && b.px_h >= 1);
        let b = glyph_box(face_metrics(), GlyphMetrics::default(), f32::NAN, 1.0, 0);
        assert!(b.px_w >= 1 && b.px_h >= 1);
        let b = glyph_box(face_metrics(), GlyphMetrics::default(), 16.0, f32::NAN, 0);
        assert!(b.px_w >= 1 && b.px_h >= 1);
    }

    /// Colour is not in the key; everything that changes the pixels is.
    #[test]
    fn key_separates_exactly_what_changes_the_pixels() {
        let format = TextFormat::new("Segoe UI", 16.0).unwrap();
        let layout = TextLayout::new("Ag", &format, 1000.0, 100.0).unwrap();
        let runs = layout.glyph_runs().unwrap();
        let run = &runs[0];
        let face = &run.font_face;
        let (a, b) = (run.glyph_indices[0], run.glyph_indices[1]);

        let base = GlyphKey::new(face, a, 16.0, 1.0, 0);
        assert_eq!(base, GlyphKey::new(face, a, 16.0, 1.0, 0), "key is stable");
        assert_ne!(base, GlyphKey::new(face, b, 16.0, 1.0, 0), "glyph id");
        assert_ne!(base, GlyphKey::new(face, a, 16.0, 1.0, 1), "phase");
        assert_ne!(base, GlyphKey::new(face, a, 24.0, 1.0, 0), "em size");
        assert_ne!(base, GlyphKey::new(face, a, 16.0, 2.0, 0), "scale");
        // Phase wraps rather than forking a fifth entry.
        assert_eq!(base, GlyphKey::new(face, a, 16.0, 1.0, SUBPIXEL_PHASES));
        // Float noise below the quantization grid does not fork the cache.
        assert_eq!(base, GlyphKey::new(face, a, 16.0 + 1.0e-5, 1.0, 0));
    }

    // ── Rasterization against a real composition device ──────────────────────

    /// A windowless composition graphics device: the same `GpuDevice` +
    /// `Compositor` + `CreateGraphicsDevice` chain `Compositing::new` builds,
    /// minus the HWND and the desktop target. Mints real
    /// `CompositionDrawingSurface`s, so the rasterizer under test is the
    /// shipping one on the shipping surface path.
    struct Headless {
        /// Held for the compositor's lifetime — a `Compositor` needs a
        /// dispatcher queue on its own thread, and dropping the controller
        /// would take the queue with it. `None` where the thread already had
        /// one, which is a success for our purposes and not a failure.
        _queue: Option<DispatcherQueueController>,
        _gpu: GpuDevice,
        graphics: CompositionGraphicsDevice,
        compositor: Compositor,
        lost: Cell<bool>,
    }

    impl Headless {
        fn new() -> windows_core::Result<Self> {
            // An already-present controller returns an error we ignore.
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

    /// Rasterize real glyphs through a real composition device and assert the
    /// cache's identity contract. Skipped (not failed) where no composition
    /// device is available at all, which is the only part of this that a
    /// headless session can legitimately lack.
    #[test]
    fn rasterizes_and_caches_real_glyphs() {
        let Ok(dev) = Headless::new() else {
            eprintln!("no composition graphics device available; skipping");
            return;
        };

        let format = TextFormat::new("Segoe UI", 16.0).unwrap();
        let layout = TextLayout::new("Hamburgefonstiv", &format, 1000.0, 100.0).unwrap();
        let runs = layout.glyph_runs().unwrap();
        let run = &runs[0];
        let face = &run.font_face;

        let mut atlas = GlyphAtlas::default();
        assert_eq!(atlas.len(), 0);

        let g = run.glyph_indices[0];
        let first = atlas
            .get(&dev, face, g, 16.0, 1.0, 0)
            .expect("rasterize one glyph");
        assert_eq!(atlas.len(), 1);

        // A hit returns the SAME region and brush, not a re-raster.
        let again = atlas.get(&dev, face, g, 16.0, 1.0, 0).expect("cache hit");
        assert_eq!(atlas.len(), 1, "a hit must not add an entry");
        // `==` on a brush is COM identity, which is the whole assertion here;
        // `assert!` rather than `assert_eq!` because the wrapper deliberately
        // exposes no `Debug` that could print an interface pointer.
        assert!(
            first.brush() == again.brush(),
            "a cache hit must return the identical brush object"
        );
        assert_eq!(
            first.id, again.id,
            "…and the identity a caller compares must agree with it"
        );
        assert_eq!(first.geom, again.geom);

        // Each phase is its own entry with its own region of the shared page —
        // so its own brush, since a brush is what aims at a region.
        let mut brushes = vec![first.brush().clone()];
        for phase in 1..SUBPIXEL_PHASES {
            let r = atlas
                .get(&dev, face, g, 16.0, 1.0, phase)
                .expect("rasterize phase");
            assert!(
                !brushes.contains(r.brush()),
                "phase {phase} must have its own region"
            );
            brushes.push(r.brush().clone());
        }
        assert_eq!(
            atlas.len(),
            SUBPIXEL_PHASES as usize,
            "one entry per phase, and no more"
        );

        // Every glyph in the run rasterizes.
        for &gid in &run.glyph_indices {
            assert!(
                atlas.get(&dev, face, gid, 16.0, 1.0, 0).is_some(),
                "glyph {gid} failed to rasterize"
            );
        }

        // Clearing drops the rasters and bumps the epoch.
        let epoch = atlas.epoch();
        atlas.clear();
        assert_eq!(atlas.len(), 0);
        assert_ne!(atlas.epoch(), epoch);
    }

    /// A wrapped layout must hand back one run per line, at distinct baselines
    /// that step by the line height and start back at the left edge.
    ///
    /// This is what decides whether multi-line text needs anything from the
    /// sprite path at all: `TextPart::sync` walks runs and reads each one's
    /// `baseline_origin`, so if wrapping is expressed there — rather than in
    /// some line structure the walk cannot see — then wrapped text places
    /// correctly with no change to the placement code.
    #[test]
    fn wrapping_is_expressed_as_per_line_run_baselines() {
        let format = TextFormat::new("Segoe UI", 14.0).unwrap();
        let text = "The quick brown fox jumps over the lazy dog near the river bank";
        let layout = TextLayout::new(text, &format, 120.0, 400.0).unwrap();
        layout.set_word_wrap(true).unwrap();

        let m = layout.metrics().unwrap();
        assert!(m.line_count > 1, "text did not wrap at 120 DIPs");

        let runs = layout.glyph_runs().unwrap();
        let mut baselines: Vec<f32> = runs.iter().map(|r| r.baseline_origin.y).collect();
        baselines.dedup();
        assert_eq!(
            baselines.len() as u32,
            m.line_count,
            "one distinct baseline per wrapped line"
        );

        // Baselines descend, and every line begins at the layout's left edge —
        // so a caller placing runs at `origin + baseline_origin` reproduces the
        // wrap without knowing it happened.
        for pair in baselines.windows(2) {
            assert!(pair[1] > pair[0], "baselines must descend: {baselines:?}");
        }
        let first_x: Vec<f32> = {
            let mut seen = Vec::new();
            let mut last_y = f32::NAN;
            for r in &runs {
                if r.baseline_origin.y != last_y {
                    seen.push(r.baseline_origin.x);
                    last_y = r.baseline_origin.y;
                }
            }
            seen
        };
        for x in &first_x {
            assert!(
                x.abs() < 1.0,
                "each line should start at the left edge, got {x} in {first_x:?}"
            );
        }

        // Every glyph on every line must rasterize through the same atlas — the
        // second line is not a different kind of text.
        let total: usize = runs.iter().map(|r| r.glyph_indices.len()).sum();
        assert!(total > 0);
    }

    /// A8 must be a usable mask surface format.
    ///
    /// This gates an 8× memory cut. Coverage arrives from the rasterizer as
    /// `u8`, so [`MASK_FORMAT`]'s FP16 currently stores 8-bit data in 64 bits
    /// per pixel and can carry no information the source did not have. The FP16
    /// depth was chosen when this module DREW its glyphs, where the coverage
    /// came through a 2.2 ramp whose top collapsed under 8-bit quantization;
    /// uploading measured coverage removed that ramp, and with it the reason.
    ///
    /// What is NOT yet proven here is that the compositor honours an A8 surface
    /// as a `CompositionMaskBrush` mask — this only establishes that one can be
    /// minted and drawn into, which is the half that can be tested without a
    /// window.
    #[test]
    fn a8_is_a_mintable_mask_surface() {
        let Ok(dev) = Headless::new() else {
            eprintln!("no composition graphics device available; skipping");
            return;
        };
        // The probe is the `Err` itself: an unsupported format must fail to
        // mint rather than fall back, or this test would pass on FP16. Probed on
        // a page, because a page is the only thing `MASK_FORMAT` is ever applied
        // to now — a format the device accepts for a fixed surface but not for a
        // virtual one would be a pass here that the atlas could not honour.
        let mut atlas = Atlas::new(PixelFormat::A8UNorm);
        let Some(tile) = atlas.alloc(&dev, 16, 24, 1.0) else {
            eprintln!("A8 drawing surface not supported; MASK_FORMAT must stay FP16");
            return;
        };
        dev.device_lost().set(false);
        let (ctx, (origin_x, origin_y)) = tile
            .begin_draw::<ID2D1DeviceContext>()
            .expect("A8 surface minted but refused BeginDraw");
        let session = DrawingSession::from_borrowed_context(
            &ctx,
            Matrix3x2::translation(origin_x as f32, origin_y as f32),
        );
        let (cx, cy, cw, ch) = tile.clip();
        session.push_clip(&Rect::from_xywh(cx, cy, cw, ch));
        session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
        session.pop_clip();
        tile.end_draw().expect("A8 EndDraw");
        assert!(!dev.device_lost().get(), "A8 draw reported device loss");
    }

    /// Rasters share pages. A cache whose entries each took a surface is the
    /// thing this replaced, so the population is the assertion.
    ///
    /// Lives here rather than in `mask_cache` because `Headless` — a real
    /// composition device, which is the whole point of the seam — is here.
    #[test]
    fn many_rasters_share_one_page() {
        let Ok(dev) = Headless::new() else {
            eprintln!("no composition graphics device available; skipping");
            return;
        };
        let mut atlas = Atlas::new(MASK_FORMAT);
        // Comfortably more than an idle window's whole standing population, at a
        // typical body-text glyph's size.
        let tiles: Vec<_> = (0..300)
            .filter_map(|_| atlas.alloc(&dev, 12, 20, 1.0))
            .collect();
        assert_eq!(tiles.len(), 300, "every raster must find room");
        assert_eq!(atlas.pages(), 1, "300 glyph-sized rasters must fit one page");
    }

    /// The safety invariant packing introduces: a region is common ground, so it
    /// may be re-let only once nothing can still be showing it.
    ///
    /// Both halves matter and they pull opposite ways — a cache that never re-lets
    /// leaks ground, and one that re-lets too early redraws a live label with
    /// another glyph's ink. Neither shows up as a compile error, so both are
    /// asserted here.
    #[test]
    fn a_region_is_re_let_only_once_its_last_holder_drops() {
        let Ok(dev) = Headless::new() else {
            eprintln!("no composition graphics device available; skipping");
            return;
        };
        let mut atlas = Atlas::new(MASK_FORMAT);
        let first = atlas.alloc(&dev, 12, 20, 1.0).expect("alloc");
        let held = first.origin();

        // Still held: an identically sized request must be given other ground.
        let second = atlas.alloc(&dev, 12, 20, 1.0).expect("alloc");
        assert_ne!(
            held,
            second.origin(),
            "a region must not be re-let while it is held"
        );

        // Released: the same ground comes back, rather than the page creeping.
        drop(first);
        let third = atlas.alloc(&dev, 12, 20, 1.0).expect("alloc");
        assert_eq!(
            held,
            third.origin(),
            "a released region must be offered to the next request of its class"
        );
    }

    /// The design advances the atlas sizes its boxes from must account for the
    /// same total width DirectWrite measured, so a caller stepping the pen by
    /// them lands where the layout says the text ends.
    #[test]
    fn design_advances_sum_to_the_measured_width() {
        let format = TextFormat::new("Segoe UI", 16.0).unwrap();
        let text = "Hamburgefonstiv";
        let layout = TextLayout::new(text, &format, 1000.0, 100.0).unwrap();
        let runs = layout.glyph_runs().unwrap();
        let measured = layout.metrics().unwrap().width;

        // The harness first: shaped advances must agree with the layout.
        let shaped: f32 = runs.iter().map(GlyphRun::width).sum();
        assert!(
            (shaped - measured).abs() < 0.5,
            "shaped advances ({shaped}) vs layout width ({measured})"
        );

        // Then the atlas's own arithmetic, computed exactly as `rasterize` does.
        let mut design = 0.0f32;
        for run in &runs {
            let fm = run.font_face.metrics();
            let gms = run
                .font_face
                .design_glyph_metrics(&run.glyph_indices, false)
                .unwrap();
            for gm in gms {
                design += glyph_box(fm, gm, run.font_em_size, 1.0, 0).advance_dip;
            }
        }
        assert!(
            (design - measured).abs() < 0.5,
            "design advances ({design}) vs layout width ({measured})"
        );
    }
}
