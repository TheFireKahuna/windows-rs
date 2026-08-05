//! Walks a laid-out string for the glyphs DirectWrite resolved.
//!
//! `IDWriteTextLayout` reports what it shaped only by drawing, so [`Collector`] is a
//! renderer that draws nothing and records everything. One walk is the whole view of the
//! result: itemization, bidi, fallback, features and line breaking have already happened
//! inside the layout.
//!
//! One collector serves an engine, appending into buffers a run lends it for the length of
//! the walk and takes back after.

use super::*;
use windows_core::{ComObject, IUnknownImpl, Ref};

/// Records one segment of a walked line: a face, the glyphs drawn with it, and where.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct HarvestSeg {
    pub seg: GlyphSeg,
    /// Baseline origin in the layout's own coordinate space.
    pub origin: Vector2,
    /// Which line of the layout it sits on. Filled after the walk by [`ShapedRun`].
    pub line: u16,
}

/// Describes a rule DirectWrite resolved that is not made of glyphs.
///
/// Whoever renders one draws a rectangle. The geometry comes out of the layout's own
/// arithmetic, so the walk records it rather than leaving it to be re-derived.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Decoration {
    pub kind: DecorationKind,
    pub origin: Vector2,
    pub width: f32,
    pub thickness: f32,
    /// Baseline-relative offset; positive is below the baseline.
    pub offset: f32,
}

/// Names which rule a [`Decoration`] describes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DecorationKind {
    Underline,
    Strikethrough,
}

/// Holds the buffers one walk fills. A run lends its own and takes them back, so nothing
/// here outlives a call and the capacity survives between walks.
#[derive(Debug, Default)]
pub(crate) struct Harvest {
    pub glyphs: Vec<u16>,
    pub advances: Vec<f32>,
    pub offsets: Vec<[f32; 2]>,
    pub segs: Vec<HarvestSeg>,
    pub decorations: Vec<Decoration>,
}

impl Harvest {
    fn clear(&mut self) {
        self.glyphs.clear();
        self.advances.clear();
        self.offsets.clear();
        self.segs.clear();
        self.decorations.clear();
    }
}

/// Implements the renderer `IDWriteTextLayout::Draw` drives.
#[windows_core::implement(IDWriteTextRenderer)]
pub(crate) struct Collector {
    out: RefCell<Harvest>,
    /// Maps a face to its id by pointer identity, holding the face so its pointer cannot
    /// be reused for another. Scanned linearly: the list is as long as the distinct faces
    /// on screen.
    memo: RefCell<Vec<(IDWriteFontFace, Option<FaceId>)>>,
    ladder: FontLadder,
}

impl Collector {
    /// Creates a collector that interns the faces it walks into `ladder`.
    pub(crate) fn new(ladder: FontLadder) -> ComObject<Self> {
        ComObject::new(Self {
            out: RefCell::new(Harvest::default()),
            memo: RefCell::new(Vec::new()),
            ladder,
        })
    }

    /// Runs one walk against `layout`, with `into` lent to the collector for its duration
    /// and swapped back before returning.
    pub(crate) fn walk(this: &ComObject<Self>, layout: &IDWriteTextLayout, into: &mut Harvest) {
        into.clear();
        core::mem::swap(&mut *this.get().out.borrow_mut(), into);
        let renderer: IDWriteTextRenderer = this.to_interface();
        // A failed walk keeps whatever it recorded, matching what the callbacks do with a
        // malformed payload: the caller reads a short run rather than an error.
        // SAFETY: a call on an interface the caller owns, taking two scalars and a
        // renderer `this` keeps alive across the call.
        let _ = unsafe { layout.Draw(None, &renderer, 0.0, 0.0) };
        core::mem::swap(&mut *this.get().out.borrow_mut(), into);
    }

    /// Returns the id for the face DirectWrite chose, interning it on first sight.
    ///
    /// Returns `None` when the face cannot be described, and the caller then drops the
    /// segment: `FaceId(0)` is a real face, so substituting it would draw one font's glyph
    /// indices through another's.
    fn face_id(&self, face: &IDWriteFontFace) -> Option<FaceId> {
        let raw = face.as_raw();
        if let Some((_, id)) = self.memo.borrow().iter().find(|(f, _)| f.as_raw() == raw) {
            return *id;
        }
        let id = self.describe(face).ok().map(|key| self.ladder.intern(&key));
        // Memoized either way, so a face that cannot be described is interrogated once.
        // The face is held because its pointer is the memo's key.
        self.memo.borrow_mut().push((face.clone(), id));
        id
    }

    /// Reads a resolved face back as a [`FaceKey`], the terms another thread looks it up
    /// by.
    fn describe(&self, face: &IDWriteFontFace) -> Result<FaceKey> {
        let face: IDWriteFontFace3 = face.cast()?;
        // SAFETY: `names` outlives both calls, and the buffer handed to `GetString` is
        // sized from the length the call before it reported.
        unsafe {
            let names = face.GetFamilyNames()?;
            let len = names.GetStringLength(0)? as usize;
            let mut buffer = vec![0u16; len + 1];
            names.GetString(0, &mut buffer).ok()?;
            Ok(FaceKey {
                family: String::from_utf16_lossy(&buffer[..len]).into(),
                weight: face.GetWeight().clamp(1, 999) as u16,
                style: FontStyle::from_dwrite(face.GetStyle()),
                stretch: FontStretch::from_dwrite(face.GetStretch()),
            })
        }
    }
}

impl IDWritePixelSnapping_Impl for Collector_Impl {
    /// Disables pixel snapping: the walk measures rather than rasterizes, so it records
    /// the unrounded baseline DirectWrite shaped to. Whoever rasterizes applies its own
    /// pixel grid.
    fn IsPixelSnappingDisabled(&self, _: *const core::ffi::c_void) -> Result<windows_core::BOOL> {
        Ok(windows_core::BOOL(1))
    }

    /// Reports the identity transform, so runs come back in the layout's own space; a
    /// world transform is applied by whatever draws them.
    fn GetCurrentTransform(
        &self,
        _: *const core::ffi::c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> Result<()> {
        if transform.is_null() {
            return Err(windows_core::Error::from_hresult(windows_core::HRESULT(
                -2147467261, // E_POINTER
            )));
        }
        // SAFETY: checked non-null above, and DirectWrite owns a writable matrix there.
        unsafe {
            *transform = DWRITE_MATRIX {
                m11: 1.0,
                m22: 1.0,
                ..Default::default()
            };
        }
        Ok(())
    }

    /// Reports one pixel per DIP, which with the identity transform keeps every coordinate
    /// the walk records in DIPs.
    fn GetPixelsPerDip(&self, _: *const core::ffi::c_void) -> Result<f32> {
        Ok(1.0)
    }
}

impl IDWriteTextRenderer_Impl for Collector_Impl {
    fn DrawGlyphRun(
        &self,
        _: *const core::ffi::c_void,
        origin_x: f32,
        origin_y: f32,
        _: DWRITE_MEASURING_MODE,
        run: *const DWRITE_GLYPH_RUN,
        _: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _: Ref<windows_core::IUnknown>,
    ) -> Result<()> {
        // A null run, or one with no face, records nothing and still returns success: an
        // error here aborts the walk and loses the runs already collected.
        if run.is_null() {
            return Ok(());
        }
        // SAFETY: non-null, and DirectWrite guarantees the struct and its two mandatory
        // arrays live for the length of this callback.
        let run = unsafe { &*run };
        let Some(face) = run.fontFace.as_ref() else {
            return Ok(());
        };

        // The face resolves before anything is appended, so one that cannot be described
        // costs no buffer space and leaves no segment addressing it.
        let Some(face) = self.face_id(face) else {
            return Ok(());
        };

        let count = run.glyphCount as usize;
        let mut out = self.out.borrow_mut();
        let at = Spans {
            glyphs: Span::new(out.glyphs.len() as u32, count as u32),
            advances: Span::new(out.advances.len() as u32, count as u32),
            offsets: Span::new(out.offsets.len() as u32, count as u32),
        };
        // SAFETY: `glyphIndices` and `glyphAdvances` are `glyphCount` long; `glyphOffsets`
        // is optional and arrives null when the run needs no adjustments. Reading its
        // elements as `[f32; 2]` is sound by the layout assertion in this module.
        unsafe {
            extend(&mut out.glyphs, run.glyphIndices, count);
            extend(&mut out.advances, run.glyphAdvances, count);
            if run.glyphOffsets.is_null() {
                let filled = out.offsets.len() + count;
                out.offsets.resize(filled, [0.0, 0.0]);
            } else {
                extend(&mut out.offsets, run.glyphOffsets.cast(), count);
            }
        }
        out.segs.push(HarvestSeg {
            seg: GlyphSeg {
                face,
                em: run.fontEmSize,
                bidi: run.bidiLevel,
                // Rebased onto its tile by `ShapedRun::segments`; the walk records the
                // layout-space origin in the field beside it.
                origin: Vector2::default(),
                glyphs: at.glyphs,
                advances: at.advances,
                offsets: at.offsets,
            },
            origin: Vector2 {
                x: origin_x,
                y: origin_y,
            },
            line: 0,
        });
        Ok(())
    }

    fn DrawUnderline(
        &self,
        _: *const core::ffi::c_void,
        origin_x: f32,
        origin_y: f32,
        underline: *const DWRITE_UNDERLINE,
        _: Ref<windows_core::IUnknown>,
    ) -> Result<()> {
        if underline.is_null() {
            return Ok(());
        }
        // SAFETY: checked non-null; borrowed for this callback only.
        let u = unsafe { &*underline };
        self.push_rule(
            DecorationKind::Underline,
            origin_x,
            origin_y,
            u.width,
            u.thickness,
            u.offset,
        );
        Ok(())
    }

    fn DrawStrikethrough(
        &self,
        _: *const core::ffi::c_void,
        origin_x: f32,
        origin_y: f32,
        strikethrough: *const DWRITE_STRIKETHROUGH,
        _: Ref<windows_core::IUnknown>,
    ) -> Result<()> {
        if strikethrough.is_null() {
            return Ok(());
        }
        // SAFETY: checked non-null; borrowed for this callback only.
        let s = unsafe { &*strikethrough };
        self.push_rule(
            DecorationKind::Strikethrough,
            origin_x,
            origin_y,
            s.width,
            s.thickness,
            s.offset,
        );
        Ok(())
    }

    fn DrawInlineObject(
        &self,
        context: *const core::ffi::c_void,
        origin_x: f32,
        origin_y: f32,
        object: Ref<IDWriteInlineObject>,
        sideways: windows_core::BOOL,
        rtl: windows_core::BOOL,
        effect: Ref<windows_core::IUnknown>,
    ) -> Result<()> {
        // Re-enters with this same renderer so the object's own glyphs land in `segs`. A
        // trimming sign is an inline object, so returning success without drawing it drops
        // the `…` and leaves the run reading as cut off mid-word.
        let Some(object) = object.as_ref() else {
            return Ok(());
        };
        let renderer: IDWriteTextRenderer = self.to_interface();
        // SAFETY: every argument is one this callback was handed, forwarded unchanged.
        unsafe {
            object
                .Draw(
                    Some(context),
                    &renderer,
                    origin_x,
                    origin_y,
                    sideways.as_bool(),
                    rtl.as_bool(),
                    effect.as_ref(),
                )
                .ok()
        }
    }
}

impl Collector_Impl {
    fn push_rule(
        &self,
        kind: DecorationKind,
        x: f32,
        y: f32,
        width: f32,
        thickness: f32,
        offset: f32,
    ) {
        self.out.borrow_mut().decorations.push(Decoration {
            kind,
            origin: Vector2 { x, y },
            width,
            thickness,
            offset,
        });
    }
}

/// `DWRITE_GLYPH_OFFSET` has the size, alignment and field order of `[f32; 2]`, which is
/// what lets a callback's offset array be reinterpreted rather than converted element by
/// element. The two declarations live in different crates, so the equality is asserted.
const _: () = {
    assert!(size_of::<[f32; 2]>() == size_of::<DWRITE_GLYPH_OFFSET>());
    assert!(align_of::<[f32; 2]>() == align_of::<DWRITE_GLYPH_OFFSET>());
    assert!(core::mem::offset_of!(DWRITE_GLYPH_OFFSET, advanceOffset) == 0);
    assert!(core::mem::offset_of!(DWRITE_GLYPH_OFFSET, ascenderOffset) == size_of::<f32>());
};

/// Appends `count` elements from a callback-borrowed array.
///
/// # Safety
///
/// `ptr` must be valid for `count` reads of `T`, unless it is null or `count` is zero.
unsafe fn extend<T: Copy>(into: &mut Vec<T>, ptr: *const T, count: usize) {
    if ptr.is_null() || count == 0 {
        return;
    }
    // SAFETY: `ptr` is non-null and `count` is non-zero here, and the caller guarantees
    // `count` valid reads from it.
    into.extend_from_slice(unsafe { core::slice::from_raw_parts(ptr, count) });
}
