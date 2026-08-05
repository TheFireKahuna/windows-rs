//! Names a font, and resolves that name to a face.
//!
//! A shaped run crosses a thread boundary as plain `Copy` data, so it carries no family
//! *name*. It names a font by an index into an application-wide [`FontLadder`], and the
//! thread that rasterizes resolves that index against the same ladder. Every thread must
//! share one ladder: two [`TextEngine`]s interning independently agree on `0` and disagree
//! on everything after it, and the symptom is a run drawn in the wrong face rather than an
//! error.
//!
//! ## Two id spaces
//!
//! [`FamilyId`] names what an application declared; [`FaceId`] names the face shaping
//! resolved, fallback included, and only [`FaceId`] travels. Fallback resolves a face the
//! requested family does not name, and a glyph index is an index into a face, so a segment
//! carrying only the requested [`FontSpec`] draws the fallback run's ids through the
//! primary family — arbitrary glyphs at arbitrary positions, not tofu.
//!
//! The face table is append-only: an id is never reassigned, and a reader only ever asks
//! for one the writer already minted.

use super::*;
use rustc_hash::FxHashMap;
use std::sync::{Arc, RwLock};
use windows_core::ComObject;

/// Names a family by its position in the application's [`FontLadder`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FamilyId(pub u16);

/// Names a resolved face by its position in the application's [`FontLadder`].
///
/// Minted by shaping, consumed by rasterization. Unlike [`FamilyId`], an application never
/// declares one: it names the face DirectWrite chose, fallback included.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceId(pub u16);

impl FaceId {
    /// Names no face. [`FontLadder::intern`] never assigns this index and returns it on
    /// overflow, so an overflowed intern yields a run that does not draw.
    pub const NONE: Self = Self(u16::MAX);
}

/// Selects one face. Size is absent: one face serves every size.
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

/// Holds the families an application uses, in a fixed order, plus the faces shaping has
/// resolved.
///
/// Built once and cloned, so every clone shares one table. The family half is immutable
/// and the face half is append-only, so an id resolves to the same font on the shaping
/// side and on the rasterizing side.
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

    /// Returns the id of `family`, or `None` if the ladder does not carry it.
    ///
    /// Scans the ladder's families linearly. Called when a view is built, not per glyph.
    #[must_use]
    pub fn id(&self, family: &str) -> Option<FamilyId> {
        let index = self.0.families.iter().position(|name| &**name == family)?;
        u16::try_from(index).ok().map(FamilyId)
    }

    /// Returns the family name `id` selects, or `""` if it names nothing in this ladder.
    #[must_use]
    pub fn name(&self, id: FamilyId) -> &str {
        self.0.families.get(id.0 as usize).map_or("", |name| name)
    }

    /// Returns the number of families.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.families.len()
    }

    /// Returns whether the ladder carries no families.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.families.is_empty()
    }

    /// Returns the id of `key`, appending it to the face table on first sight.
    ///
    /// Returns [`FaceId::NONE`] when the table has no index left to assign. Only shaping
    /// appends: a screen's worth of text resolves a handful of faces and then reads.
    pub fn intern(&self, key: &FaceKey) -> FaceId {
        if let Some(id) = self.face_id(key) {
            return id;
        }
        let mut faces = self.0.faces.write().unwrap_or_else(|e| e.into_inner());
        // Re-checked under the write lock: two threads can race the read that precedes it,
        // and a duplicated entry rasterizes one face twice rather than failing.
        if let Some(id) = index_of(&faces, key) {
            return id;
        }
        // Fails closed on overflow: an index that wrapped would alias one face onto
        // another's, which renders as the wrong font and reports nothing.
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

    /// Returns the [`FaceKey`] `id` names, or `None` if it names nothing in this ladder.
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

/// Selects upright letterforms, or one of the two slanted forms.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontStyle {
    #[default]
    Normal,
    /// Slants using the family's designed italic, a different set of letterforms.
    Italic,
    /// Slants by shearing the upright, for a family with no designed italic.
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

/// Selects the width of the letterforms, as the nine standard steps.
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

/// Carries the typographic features applied over a whole run.
///
/// A bitfield rather than a list, so [`FontSpec`] stays `Copy` and small enough to key a
/// cache on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FontFeatures(u8);

impl FontFeatures {
    /// Applies the family's own defaults.
    pub const NONE: Self = Self(0);
    /// Gives every digit one advance, so a numeric read-out does not shift as its value
    /// changes.
    pub const TABULAR: Self = Self(1 << 0);

    /// Returns the features of `self` together with those of `other`.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether every feature in `other` is set.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns whether no feature is set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Picks a face, and carries the size to draw it at.
///
/// Names what a caller asks for. What shaping resolved is a [`FaceId`], and only the
/// [`FaceId`] crosses a thread boundary.
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
    /// Returns a spec for a regular upright face of `family` at `size` DIPs.
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

    /// Returns the same spec at `weight` on the 100–950 axis.
    #[must_use]
    pub const fn weight(self, weight: u16) -> Self {
        Self { weight, ..self }
    }

    /// Returns the same spec in `style`.
    #[must_use]
    pub const fn style(self, style: FontStyle) -> Self {
        Self { style, ..self }
    }

    /// Returns the same spec at `stretch`.
    #[must_use]
    pub const fn stretch(self, stretch: FontStretch) -> Self {
        Self { stretch, ..self }
    }

    /// Returns the same spec with `features`, replacing whatever it carried.
    #[must_use]
    pub const fn features(self, features: FontFeatures) -> Self {
        Self { features, ..self }
    }
}

/// Holds a resolved font face.
///
/// Leaves this crate as `&impl Interface` and never as the DirectWrite type, so a consumer
/// hands it to a drawing call without naming DirectWrite itself.
#[derive(Clone)]
pub struct FontFace(pub(crate) IDWriteFontFace);

impl FontFace {
    /// Returns the face as the interface a drawing call takes.
    pub fn as_interface(&self) -> &impl Interface {
        &self.0
    }
}

/// Holds DirectWrite's factory, the system font collection, and the caches over both.
///
/// One per thread: the caches are `RefCell`s, so an engine is neither `Send` nor `Sync`,
/// and a thread that shapes and a thread that rasterizes each create their own over the
/// same [`FontLadder`].
pub struct TextEngine {
    pub(crate) factory: IDWriteFactory3,
    pub(crate) collection: IDWriteFontCollection,
    params: IDWriteRenderingParams3,
    ladder: FontLadder,
    faces: RefCell<FxHashMap<FaceId, FontFace>>,
    pub(crate) formats: RefCell<FxHashMap<FormatKey, IDWriteTextFormat>>,
    pub(crate) typography: RefCell<FxHashMap<FontFeatures, Option<IDWriteTypography>>>,
    /// One walker for the whole engine, so a walk reuses it and the buffers inside it
    /// rather than allocating per run.
    pub(crate) collector: RefCell<Option<ComObject<Collector>>>,
    /// UTF-16 staging for `CreateTextLayout`, which copies what it is handed.
    pub(crate) scratch: RefCell<Vec<u16>>,
}

impl TextEngine {
    /// Creates an engine over `ladder`.
    ///
    /// # Errors
    ///
    /// Fails when the DirectWrite factory, the system font collection, or the rendering
    /// parameters cannot be created.
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

        // Both flags are false: downloadable fonts would make a lookup depend on the
        // network, and the update check enumerates the collection on every call.
        // SAFETY: the out-parameter is a stack local that outlives the call, and the
        // interface it receives is checked for absence before it is cast.
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

    /// Returns the ladder this engine resolves ids against.
    #[must_use]
    pub fn ladder(&self) -> &FontLadder {
        &self.ladder
    }

    /// Returns the face `id` names, from this engine's cache or freshly resolved.
    ///
    /// Resolving walks family name to index to family to font to face, so the result is
    /// cached and the walk runs once per id per engine.
    ///
    /// # Errors
    ///
    /// Fails when `id` names nothing in the ladder, and when the family it names is not
    /// installed.
    pub fn face(&self, id: FaceId) -> Result<FontFace> {
        if let Some(face) = self.faces.borrow().get(&id) {
            return Ok(face.clone());
        }
        let key = self.ladder.face_key(id).ok_or_else(no_font)?;
        let face = self.resolve(&key)?;
        self.faces.borrow_mut().insert(id, face.clone());
        Ok(face)
    }

    /// Returns the parameters glyph coverage is rasterized under, to be stated on whatever
    /// draws it.
    ///
    /// Constructed here rather than inherited from the system, whose parameters carry a
    /// user's ClearType tuning and a display's gamma and rasterize coverage systematically
    /// thin or fat. ClearType level is zero because coverage is grayscale, enhanced
    /// contrast is zero because contrast belongs to the palette, and gamma is 1.0 because
    /// the surface this lands on is linear.
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
            // A missing family fails rather than resolving to family zero, which would
            // render every missing font as whatever the collection lists first. Falling
            // back to another family is the caller's policy, and it holds the ladder.
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

/// Returns a NUL-terminated UTF-16 copy of `s`, for the APIs that take a `PCWSTR`.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

/// Returns `DWRITE_E_NOFONT`: the family is not installed, so nothing can be drawn in it.
/// The metadata carries no constant for it, so the value is spelled out here.
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

    /// An id minted through one clone names the same font through another, and appending
    /// leaves an id already handed out where it was.
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

    /// [`FaceId::NONE`] resolves to no face, so an overflowed intern cannot alias one face
    /// onto another's index.
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
