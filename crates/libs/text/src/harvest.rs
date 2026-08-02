//! Walking a laid-out string for the glyphs DirectWrite resolved.
//!
//! `IDWriteTextLayout` draws through a callback, so the way to learn what it shaped is to
//! hand it a renderer that draws nothing and records everything. That one walk is the only
//! view of the result — itemization, bidi, fallback, features and line breaking have all
//! already happened inside it.
//!
//! Everything here is pooled: one collector per engine, appending into buffers a run lends
//! it for the length of the walk and takes back after.

use super::*;
use windows_core::{ComObject, IUnknownImpl, Ref};

/// One segment of a walked line: a face, and the glyphs drawn with it, plus where.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct HarvestSeg {
    pub seg: GlyphSeg,
    /// Baseline origin in the layout's own coordinate space.
    pub origin: Vector2,
    /// Which line of the layout it sits on. Filled after the walk, in [`ShapedRun`].
    pub line: u16,
}

/// A rule DirectWrite resolved that is not made of glyphs.
///
/// Recorded rather than dropped: whoever renders one draws a rectangle, but the geometry
/// comes out of the layout's own arithmetic and cannot be re-derived without redoing it.
/// The consumer is an IME composition span.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Decoration {
    pub kind: DecorationKind,
    pub origin: Vector2,
    pub width: f32,
    pub thickness: f32,
    /// Baseline-relative offset; positive is below the baseline.
    pub offset: f32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DecorationKind {
    Underline,
    Strikethrough,
}

/// The buffers one walk fills. A run lends its own and takes them back, so nothing here
/// outlives a call and no capacity is thrown away between them.
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

/// The renderer `IDWriteTextLayout::Draw` drives.
#[windows_core::implement(IDWriteTextRenderer)]
pub(crate) struct Collector {
    out: RefCell<Harvest>,
    /// Face to id, by pointer identity, with the face kept alive so the pointer cannot be
    /// reused. A screen of Latin text resolves one entry and never reads a family name
    /// again; the list is as long as the distinct faces on screen, so a scan beats a hash.
    memo: RefCell<Vec<(IDWriteFontFace, Option<FaceId>)>>,
    ladder: FontLadder,
}

impl Collector {
    pub(crate) fn new(ladder: FontLadder) -> ComObject<Self> {
        ComObject::new(Self {
            out: RefCell::new(Harvest::default()),
            memo: RefCell::new(Vec::new()),
            ladder,
        })
    }

    /// Runs one walk against `layout`, with `into` lent for its duration.
    pub(crate) fn walk(this: &ComObject<Self>, layout: &IDWriteTextLayout, into: &mut Harvest) {
        into.clear();
        core::mem::swap(&mut *this.get().out.borrow_mut(), into);
        let renderer: IDWriteTextRenderer = this.to_interface();
        // A failed walk leaves whatever it recorded, which is the same policy the callbacks
        // take with a malformed payload: partial text beats none, and the caller sees a
        // short run rather than an error it cannot act on.
        let _ = unsafe { layout.Draw(None, &renderer, 0.0, 0.0) };
        core::mem::swap(&mut *this.get().out.borrow_mut(), into);
    }

    /// The id for the face DirectWrite chose, interning it on first sight — the step that
    /// makes fallback survive the seam.
    ///
    /// `None` where the face cannot be described, and the segment is dropped. There is no
    /// default to fall back to: `FaceId(0)` is a real face, so using it would draw one
    /// font's glyph indices through another's.
    fn face_id(&self, face: &IDWriteFontFace) -> Option<FaceId> {
        let raw = face.as_raw();
        if let Some((_, id)) = self.memo.borrow().iter().find(|(f, _)| f.as_raw() == raw) {
            return *id;
        }
        let id = self.describe(face).ok().map(|key| self.ladder.intern(&key));
        // Memoized either way, so a face that cannot be described is interrogated once. The
        // face is held because its pointer is this memo's key.
        self.memo.borrow_mut().push((face.clone(), id));
        id
    }

    /// Reads back what a resolved face is, in the terms the other thread can look it up by.
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
    /// Disabled: this renderer is measuring, not rasterizing, and wants the unrounded
    /// baseline DirectWrite shaped to. Whoever rasterizes applies its own pixel grid.
    fn IsPixelSnappingDisabled(&self, _: *const core::ffi::c_void) -> Result<windows_core::BOOL> {
        Ok(windows_core::BOOL(1))
    }

    /// Identity — runs come back in the layout's own space, and a world transform is
    /// applied by whatever draws them.
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

    /// Pairs with the identity transform above to keep the walk in DIPs.
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
        // A null run, or one with no face, is nothing to record and not an error: failing
        // aborts the whole walk and loses the runs already collected.
        if run.is_null() {
            return Ok(());
        }
        // SAFETY: non-null, and DirectWrite guarantees the struct and its two mandatory
        // arrays live for the length of this callback.
        let run = unsafe { &*run };
        let Some(face) = run.fontFace.as_ref() else {
            return Ok(());
        };

        // Resolved before anything is appended, so a face this cannot describe costs no
        // buffer space and leaves no segment addressing it.
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
        // is optional and arrives null when the run needs no adjustments.
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
                // Rebased onto its tile by `ShapedRun::segments`; the walk records where
                // it landed in layout space, one field over.
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
        // Re-enter with this same renderer so the object's own glyphs land in `segs`. A
        // trimming sign is an inline object, so a callback that quietly succeeded here
        // would shorten the run at the trim point and never emit the `…` that says so —
        // text reading as cut off mid-word rather than as ellipsized.
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

/// A pair of `f32` per glyph *is* `DWRITE_GLYPH_OFFSET`, so the callback's array is
/// reinterpreted rather than converted element by element. Asserted rather than trusted:
/// the two declarations are in different crates and nothing else would notice them drifting.
const _: () = {
    assert!(size_of::<[f32; 2]>() == size_of::<DWRITE_GLYPH_OFFSET>());
    assert!(align_of::<[f32; 2]>() == align_of::<DWRITE_GLYPH_OFFSET>());
    assert!(core::mem::offset_of!(DWRITE_GLYPH_OFFSET, advanceOffset) == 0);
    assert!(core::mem::offset_of!(DWRITE_GLYPH_OFFSET, ascenderOffset) == size_of::<f32>());
};

/// Appends `count` elements from a callback-borrowed array.
///
/// # Safety
/// `ptr` must be valid for `count` reads, or null with `count` zero.
unsafe fn extend<T: Copy>(into: &mut Vec<T>, ptr: *const T, count: usize) {
    if ptr.is_null() || count == 0 {
        return;
    }
    into.extend_from_slice(unsafe { core::slice::from_raw_parts(ptr, count) });
}
