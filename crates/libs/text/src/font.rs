//! Naming a font, and resolving that name to a face.
//!
//! A shaped run crosses a thread boundary as plain `Copy` data, so it cannot carry a family
//! *name*. It names a font by an index into an application-wide [`FontLadder`], and the
//! thread that rasterizes resolves that index against the same ladder. Two [`TextEngine`]s
//! interning independently would agree on `0` and disagree on everything after it, and the
//! symptom is a run drawn in the wrong face rather than an error.
//!
//! ## Two tables, because a run names what it *got*
//!
//! Fallback resolves a face the requested family does not name, and a glyph index is an
//! index into a face — so a segment carrying only the requested [`FontSpec`] draws the
//! fallback run's ids through the primary family. Not tofu: arbitrary glyphs at arbitrary
//! positions. [`FamilyId`] is what an application declares, [`FaceId`] is what shaping
//! resolved, and only the second travels.
//!
//! The face table is append-only, which keeps what matters: an id is never reassigned, and
//! a reader only ever asks for one the writer already minted.

use super::*;
use rustc_hash::FxHashMap;
use std::sync::{Arc, RwLock};
use windows_core::ComObject;

/// A family's position in the application's [`FontLadder`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FamilyId(pub u16);

/// A resolved face's position in the application's [`FontLadder`].
///
/// Minted by shaping, consumed by rasterization. Unlike [`FamilyId`] it is not something an
/// application declares — it names the face DirectWrite chose, fallback included.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceId(pub u16);

impl FaceId {
    /// Names no face, and never will: [`FontLadder::intern`] refuses to grow into this
    /// index and returns it on overflow, so the failure is a run that does not draw.
    pub const NONE: Self = Self(u16::MAX);
}

/// Everything that selects one face. Size is absent: one face serves every size.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FaceKey {
    pub family: Box<str>,
    pub weight: u16,
    pub style: FontStyle,
    pub stretch: FontStretch,
}

#[derive(Debug, Default)]
struct Ladder {
    families: Box<[Box<str>]>,
    faces: RwLock<Vec<FaceKey>>,
}

/// The families an application uses, in a fixed order, plus the faces shaping has
/// resolved, shared by every thread.
///
/// Built once and cloned — the family half is immutable and the face half is append-only,
/// so both halves of a shaping/rasterizing split resolve the same id to the same font by
/// construction rather than by staying in step.
#[derive(Clone, Debug, Default)]
pub struct FontLadder(Arc<Ladder>);

impl FontLadder {
    /// Builds a ladder from family names, in the order ids are assigned.
    pub fn new<S: Into<Box<str>>>(families: impl IntoIterator<Item = S>) -> Self {
        Self(Arc::new(Ladder {
            families: families.into_iter().map(Into::into).collect(),
            faces: RwLock::new(Vec::new()),
        }))
    }

    /// The id of `family`, if the ladder carries it.
    ///
    /// Linear, and deliberately: a ladder is a handful of families, this is called when a
    /// view is built rather than per glyph, and a map here would cost more to keep than
    /// the scan costs to run.
    #[must_use]
    pub fn id(&self, family: &str) -> Option<FamilyId> {
        let index = self.0.families.iter().position(|name| &**name == family)?;
        u16::try_from(index).ok().map(FamilyId)
    }

    /// The family `id` names, or `""` if it names nothing in this ladder.
    #[must_use]
    pub fn name(&self, id: FamilyId) -> &str {
        self.0.families.get(id.0 as usize).map_or("", |name| name)
    }

    /// How many families it carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.families.len()
    }

    /// Whether it carries none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.families.is_empty()
    }

    /// The id of `key`, appending it if this is the first time shaping produced it.
    ///
    /// Written by the shaping thread only, and rarely — a screen's worth of text resolves
    /// a handful of faces and then never appends again.
    pub fn intern(&self, key: &FaceKey) -> FaceId {
        if let Some(id) = self.face_id(key) {
            return id;
        }
        let mut faces = self.0.faces.write().unwrap_or_else(|e| e.into_inner());
        // Re-checked under the write lock: two threads may have raced the read above, and
        // a duplicated entry is a face rasterized twice rather than an error.
        if let Some(id) = index_of(&faces, key) {
            return id;
        }
        // A ladder holds a handful of faces, so this is a backstop and not a limit. It
        // fails closed because the alternative — wrapping — would silently alias one face
        // onto another's index, which renders as the wrong font and reports nothing.
        let Ok(index) = u16::try_from(faces.len()) else {
            debug_assert!(false, "a font ladder overflowed {} faces", u16::MAX);
            return FaceId::NONE;
        };
        if index == FaceId::NONE.0 {
            debug_assert!(false, "a font ladder overflowed {} faces", u16::MAX);
            return FaceId::NONE;
        }
        faces.push(key.clone());
        FaceId(index)
    }

    /// The face `id` names, or `None` if it names nothing in this ladder.
    #[must_use]
    pub fn face_key(&self, id: FaceId) -> Option<FaceKey> {
        let faces = self.0.faces.read().unwrap_or_else(|e| e.into_inner());
        faces.get(id.0 as usize).cloned()
    }

    fn face_id(&self, key: &FaceKey) -> Option<FaceId> {
        let faces = self.0.faces.read().unwrap_or_else(|e| e.into_inner());
        index_of(&faces, key)
    }
}

fn index_of(faces: &[FaceKey], key: &FaceKey) -> Option<FaceId> {
    let index = faces.iter().position(|f| f == key)?;
    u16::try_from(index).ok().map(FaceId)
}

/// Upright, or one of the two slanted forms.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontStyle {
    #[default]
    Normal,
    /// Slanted, using the family's designed italic — a different set of letterforms.
    Italic,
    /// Slanted by shearing the upright, for a family with no designed italic.
    Oblique,
}

impl FontStyle {
    pub(crate) fn dwrite(self) -> DWRITE_FONT_STYLE {
        match self {
            Self::Normal => DWRITE_FONT_STYLE_NORMAL,
            Self::Italic => DWRITE_FONT_STYLE_ITALIC,
            Self::Oblique => DWRITE_FONT_STYLE_OBLIQUE,
        }
    }

    pub(crate) fn from_dwrite(v: DWRITE_FONT_STYLE) -> Self {
        match v {
            DWRITE_FONT_STYLE_ITALIC => Self::Italic,
            DWRITE_FONT_STYLE_OBLIQUE => Self::Oblique,
            _ => Self::Normal,
        }
    }
}

/// How wide the letterforms are, as the nine standard steps.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    #[default]
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl FontStretch {
    pub(crate) fn dwrite(self) -> DWRITE_FONT_STRETCH {
        match self {
            Self::UltraCondensed => DWRITE_FONT_STRETCH_ULTRA_CONDENSED,
            Self::ExtraCondensed => DWRITE_FONT_STRETCH_EXTRA_CONDENSED,
            Self::Condensed => DWRITE_FONT_STRETCH_CONDENSED,
            Self::SemiCondensed => DWRITE_FONT_STRETCH_SEMI_CONDENSED,
            Self::Normal => DWRITE_FONT_STRETCH_NORMAL,
            Self::SemiExpanded => DWRITE_FONT_STRETCH_SEMI_EXPANDED,
            Self::Expanded => DWRITE_FONT_STRETCH_EXPANDED,
            Self::ExtraExpanded => DWRITE_FONT_STRETCH_EXTRA_EXPANDED,
            Self::UltraExpanded => DWRITE_FONT_STRETCH_ULTRA_EXPANDED,
        }
    }

    pub(crate) fn from_dwrite(v: DWRITE_FONT_STRETCH) -> Self {
        match v {
            DWRITE_FONT_STRETCH_ULTRA_CONDENSED => Self::UltraCondensed,
            DWRITE_FONT_STRETCH_EXTRA_CONDENSED => Self::ExtraCondensed,
            DWRITE_FONT_STRETCH_CONDENSED => Self::Condensed,
            DWRITE_FONT_STRETCH_SEMI_CONDENSED => Self::SemiCondensed,
            DWRITE_FONT_STRETCH_SEMI_EXPANDED => Self::SemiExpanded,
            DWRITE_FONT_STRETCH_EXPANDED => Self::Expanded,
            DWRITE_FONT_STRETCH_EXTRA_EXPANDED => Self::ExtraExpanded,
            DWRITE_FONT_STRETCH_ULTRA_EXPANDED => Self::UltraExpanded,
            _ => Self::Normal,
        }
    }
}

/// Typographic features applied over a whole run.
///
/// A bitfield rather than a list, so [`FontSpec`] stays `Copy` and small enough to key a
/// cache on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FontFeatures(u8);

impl FontFeatures {
    /// The family's own defaults.
    pub const NONE: Self = Self(0);
    /// Digits share one advance, so a numeric read-out does not shift as its value
    /// changes. Required of every `mono` rung.
    pub const TABULAR: Self = Self(1 << 0);

    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Everything that picks a face, plus the size to draw it at.
///
/// What a caller *asks for*. What shaping resolved is a [`FaceId`], and only that travels:
/// see the module documentation.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FontSpec {
    pub family: FamilyId,
    /// Em size, in DIPs.
    pub size: f32,
    /// 100–950, the standard weight axis.
    pub weight: u16,
    pub style: FontStyle,
    pub stretch: FontStretch,
    pub features: FontFeatures,
}

impl FontSpec {
    /// A regular upright face of `family` at `size` DIPs.
    #[must_use]
    pub const fn new(family: FamilyId, size: f32) -> Self {
        Self {
            family,
            size,
            weight: 400,
            style: FontStyle::Normal,
            stretch: FontStretch::Normal,
            features: FontFeatures::NONE,
        }
    }

    #[must_use]
    pub const fn weight(self, weight: u16) -> Self {
        Self { weight, ..self }
    }

    #[must_use]
    pub const fn style(self, style: FontStyle) -> Self {
        Self { style, ..self }
    }

    #[must_use]
    pub const fn stretch(self, stretch: FontStretch) -> Self {
        Self { stretch, ..self }
    }

    #[must_use]
    pub const fn features(self, features: FontFeatures) -> Self {
        Self { features, ..self }
    }
}

/// A resolved font face.
///
/// It leaves this crate as `&impl Interface` and never as the DirectWrite type, so a
/// consumer can hand it to a drawing call without naming DirectWrite itself.
#[derive(Clone)]
pub struct FontFace(pub(crate) IDWriteFontFace);

impl FontFace {
    /// The face as the interface a drawing call takes.
    pub fn as_interface(&self) -> &impl Interface {
        &self.0
    }
}

/// DirectWrite's factory, the system font collection, and the caches over both.
///
/// One per thread. DirectWrite is agile and a shared factory is cheap to create, so a
/// thread that shapes and a thread that rasterizes each hold their own rather than sharing
/// one behind a lock — and the caches inside are then uncontended.
pub struct TextEngine {
    pub(crate) factory: IDWriteFactory3,
    pub(crate) collection: IDWriteFontCollection,
    params: IDWriteRenderingParams3,
    ladder: FontLadder,
    faces: RefCell<FxHashMap<FaceId, FontFace>>,
    pub(crate) formats: RefCell<FxHashMap<FormatKey, IDWriteTextFormat>>,
    pub(crate) typography: RefCell<FxHashMap<FontFeatures, Option<IDWriteTypography>>>,
    /// One walker for the whole engine. Minting one per walk, with a fresh vector per run
    /// inside it, is three allocations per run per reshape.
    pub(crate) collector: RefCell<Option<ComObject<Collector>>>,
    /// UTF-16 staging for `CreateTextLayout`, which copies what it is handed.
    pub(crate) scratch: RefCell<Vec<u16>>,
}

impl TextEngine {
    /// Creates an engine over `ladder`.
    pub fn new(ladder: FontLadder) -> Result<Self> {
        // SAFETY: the out-parameter is a stack local that outlives the call, and the IID
        // is the one for the interface the pointer is then adopted as.
        let factory: IDWriteFactory3 = unsafe {
            let mut factory = core::ptr::null_mut();
            DWriteCreateFactory(
                DWRITE_FACTORY_TYPE_SHARED,
                &IDWriteFactory3::IID,
                &mut factory,
            )
            .ok()?;
            IDWriteFactory3::from_raw(factory)
        };

        // SAFETY: as above. Both flags are declined: downloadable fonts would make a
        // lookup fallible on the network, and the update check is a per-call enumeration
        // for a set this crate resolves once per face and then caches.
        let collection: IDWriteFontCollection = unsafe {
            let mut collection = None;
            factory
                .GetSystemFontCollection(false, &mut collection, false)
                .ok()?;
            collection.ok_or_else(no_font)?.cast()?
        };

        Ok(Self {
            params: Self::coverage_params(&factory)?,
            factory,
            collection,
            ladder,
            faces: RefCell::new(FxHashMap::default()),
            formats: RefCell::new(FxHashMap::default()),
            typography: RefCell::new(FxHashMap::default()),
            collector: RefCell::new(None),
            scratch: RefCell::new(Vec::new()),
        })
    }

    /// The ladder this engine resolves ids against.
    #[must_use]
    pub fn ladder(&self) -> &FontLadder {
        &self.ladder
    }

    /// The face `id` names, from the cache or freshly resolved.
    ///
    /// The lookup — family name to index to family to font to face — is four cross-process
    /// calls for a value that never changes, so it happens once per id per thread.
    pub fn face(&self, id: FaceId) -> Result<FontFace> {
        if let Some(face) = self.faces.borrow().get(&id) {
            return Ok(face.clone());
        }
        let key = self.ladder.face_key(id).ok_or_else(no_font)?;
        let face = self.resolve(&key)?;
        self.faces.borrow_mut().insert(id, face.clone());
        Ok(face)
    }

    /// How glyph coverage is rasterized, to be stated on whatever draws it.
    ///
    /// Explicitly constructed rather than inherited, and that is the whole point of it
    /// existing: the system's parameters carry a user's ClearType tuning and a display's
    /// gamma, and coverage rasterized under those comes out systematically thin or fat.
    /// The values pin it — ClearType level zero because coverage is grayscale, enhanced
    /// contrast zero because contrast belongs to the palette, gamma at 1.0 because the
    /// surface this lands on is already linear.
    pub fn rendering_params(&self) -> &impl Interface {
        &self.params
    }

    fn coverage_params(factory: &IDWriteFactory3) -> Result<IDWriteRenderingParams3> {
        // SAFETY: a call on an interface the caller owns, taking only scalars.
        unsafe {
            factory.CreateCustomRenderingParams(
                1.0,
                0.0,
                0.0,
                0.0,
                DWRITE_PIXEL_GEOMETRY_FLAT,
                DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
                DWRITE_GRID_FIT_MODE_DISABLED,
            )
        }
    }

    pub(crate) fn resolve(&self, key: &FaceKey) -> Result<FontFace> {
        let name = wide(&key.family);

        // SAFETY: every out-parameter is a stack local outliving its call, and `name` is
        // a NUL-terminated buffer alive for the whole of `FindFamilyName`.
        unsafe {
            let (mut index, mut exists) = (0u32, windows_core::BOOL::from(false));
            self.collection
                .FindFamilyName(windows_core::PCWSTR(name.as_ptr()), &mut index, &mut exists)
                .ok()?;
            // A family the system does not have is not an error to raise here — falling
            // back is the caller's policy and it has the ladder to do it with — but it is
            // also not something to paper over with family zero, which would render every
            // missing font as whatever happened to be first. So it fails, and the caller
            // decides.
            if !exists.as_bool() {
                return Err(no_font());
            }
            let family = self.collection.GetFontFamily(index)?;
            let font = family.GetFirstMatchingFont(
                i32::from(key.weight),
                key.stretch.dwrite(),
                key.style.dwrite(),
            )?;
            Ok(FontFace(font.CreateFontFace()?))
        }
    }
}

/// A NUL-terminated UTF-16 copy of `s`, for the APIs that take a `PCWSTR`.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

/// `DWRITE_E_NOFONT` — the family is not installed, so nothing can be drawn in it. Not in
/// the metadata as a constant, and it is the one `HRESULT` this crate has to name.
pub(crate) fn no_font() -> windows_core::Error {
    windows_core::Error::from(windows_core::HRESULT(-2003283965))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(family: &str) -> FaceKey {
        FaceKey {
            family: family.into(),
            weight: 400,
            style: FontStyle::Normal,
            stretch: FontStretch::Normal,
        }
    }

    #[test]
    fn interning_the_same_face_twice_mints_one_id() {
        let ladder = FontLadder::new(["Segoe UI"]);
        let first = ladder.intern(&key("Yu Gothic UI"));
        assert_eq!(first, ladder.intern(&key("Yu Gothic UI")));
        assert_ne!(first, ladder.intern(&key("Segoe UI")));
    }

    /// The property the whole seam rests on: an id minted on one side names the same font
    /// on the other, and appending never disturbs one already handed out.
    #[test]
    fn a_clone_sees_what_the_other_side_interned() {
        let ladder = FontLadder::new(["Segoe UI"]);
        let far_side = ladder.clone();
        let id = ladder.intern(&key("Cascadia Mono"));
        assert_eq!(far_side.face_key(id), Some(key("Cascadia Mono")));

        // Appending after the fact leaves the earlier id where it was.
        ladder.intern(&key("Segoe UI Emoji"));
        assert_eq!(far_side.face_key(id), Some(key("Cascadia Mono")));
    }

    #[test]
    fn a_weight_is_part_of_a_faces_identity() {
        let ladder = FontLadder::new(["Segoe UI"]);
        let regular = ladder.intern(&key("Segoe UI"));
        let bold = ladder.intern(&FaceKey {
            weight: 700,
            ..key("Segoe UI")
        });
        assert_ne!(regular, bold);
    }

    /// The sentinel exists so an overflowed intern cannot alias one face onto another's
    /// index — the failure that renders as the wrong font and reports nothing.
    #[test]
    fn the_none_face_resolves_to_nothing() {
        let ladder = FontLadder::new(["Segoe UI"]);
        ladder.intern(&key("Segoe UI"));
        assert_eq!(ladder.face_key(FaceId::NONE), None);
        assert!(TextEngine::new(ladder).unwrap().face(FaceId::NONE).is_err());
    }

    #[test]
    fn a_family_the_system_does_not_have_fails_rather_than_falling_to_the_first() {
        let ladder = FontLadder::new(["Segoe UI"]);
        let engine = TextEngine::new(ladder.clone()).unwrap();
        let missing = ladder.intern(&key("a family nothing installs"));
        assert!(engine.face(missing).is_err());
        assert!(engine.face(ladder.intern(&key("Segoe UI"))).is_ok());
    }
}
