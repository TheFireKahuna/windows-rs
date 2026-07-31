//! Naming a font, and resolving that name to a face.
//!
//! A shaped run crosses a thread boundary as plain data, so it cannot carry a family
//! *name*: a `String` is `Send` but not `Copy`, and the buffer it would ride in has to be
//! `Copy` for the seam to prove that nothing thread-affine entered it. So a run names its
//! family by an index into an application-wide [`FontLadder`], and the thread that
//! rasterizes resolves that index against the same ladder.
//!
//! The ladder being shared is the load-bearing part. Two [`TextEngine`]s that interned
//! names independently would agree on `0` and disagree on everything after it, and the
//! symptom would be a run rendered in the wrong face rather than an error.

use super::*;
use core::cell::RefCell;
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// A family's position in the application's [`FontLadder`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FamilyId(pub u16);

/// The families an application uses, in a fixed order, shared by every thread.
///
/// Built once and cloned — it is `Send + Sync` and immutable, so both halves of a
/// shaping/rasterizing split resolve the same [`FamilyId`] to the same name by
/// construction rather than by staying in step.
#[derive(Clone, Debug, Default)]
pub struct FontLadder(Arc<[Box<str>]>);

impl FontLadder {
    /// Builds a ladder from family names, in the order ids are assigned.
    pub fn new<S: Into<Box<str>>>(families: impl IntoIterator<Item = S>) -> Self {
        Self(families.into_iter().map(Into::into).collect())
    }

    /// The id of `family`, if the ladder carries it.
    ///
    /// Linear, and deliberately: a ladder is a handful of families, this is called when a
    /// view is built rather than per glyph, and a map here would cost more to keep than
    /// the scan costs to run.
    #[must_use]
    pub fn id(&self, family: &str) -> Option<FamilyId> {
        let index = self.0.iter().position(|name| &**name == family)?;
        u16::try_from(index).ok().map(FamilyId)
    }

    /// The family `id` names, or `""` if it names nothing in this ladder.
    #[must_use]
    pub fn name(&self, id: FamilyId) -> &str {
        self.0.get(id.0 as usize).map_or("", |name| name)
    }

    /// How many families it carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether it carries none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
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
    fn dwrite(self) -> DWRITE_FONT_STYLE {
        match self {
            Self::Normal => DWRITE_FONT_STYLE_NORMAL,
            Self::Italic => DWRITE_FONT_STYLE_ITALIC,
            Self::Oblique => DWRITE_FONT_STYLE_OBLIQUE,
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
    fn dwrite(self) -> DWRITE_FONT_STRETCH {
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
}

/// Everything that picks a face, plus the size to draw it at.
///
/// `Copy` and small, because it rides in a buffer that has to be `Copy` for the seam's
/// `Send` proof to mean what it says.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FontSpec {
    pub family: FamilyId,
    /// Em size, in DIPs.
    pub size: f32,
    /// 100–950, the standard weight axis.
    pub weight: u16,
    pub style: FontStyle,
    pub stretch: FontStretch,
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

    /// What actually selects a face. Size is not part of it — one face serves every size,
    /// which is why the cache is keyed on this and not on the whole `FontSpec`.
    fn face_key(self) -> FaceKey {
        FaceKey {
            family: self.family,
            weight: self.weight,
            style: self.style,
            stretch: self.stretch,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct FaceKey {
    family: FamilyId,
    weight: u16,
    style: FontStyle,
    stretch: FontStretch,
}

/// A resolved font face.
///
/// It leaves this crate as `&impl Interface` and never as the DirectWrite type, so a
/// consumer can hand it to a drawing call without naming DirectWrite itself.
#[derive(Clone)]
pub struct FontFace(IDWriteFontFace);

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
    collection: IDWriteFontCollection,
    params: IDWriteRenderingParams3,
    ladder: FontLadder,
    faces: RefCell<FxHashMap<FaceKey, FontFace>>,
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

        // The factory itself is not held: a shared DirectWrite factory is a process
        // singleton, and the collection and the parameters keep their own references to
        // whatever they need from it. Shaping will want one and will hold it then.
        Ok(Self {
            params: Self::coverage_params(&factory)?,
            collection,
            ladder,
            faces: RefCell::new(FxHashMap::default()),
        })
    }

    /// The ladder this engine resolves family ids against.
    #[must_use]
    pub fn ladder(&self) -> &FontLadder {
        &self.ladder
    }

    /// The face `spec` selects, from the cache or freshly resolved.
    ///
    /// Cached on everything but the size, since a face is size-independent and the
    /// lookup — family name to index to family to font to face — is four cross-process
    /// calls for a value that never changes.
    pub fn face(&self, spec: FontSpec) -> Result<FontFace> {
        let key = spec.face_key();
        if let Some(face) = self.faces.borrow().get(&key) {
            return Ok(face.clone());
        }
        let face = self.resolve(key)?;
        self.faces.borrow_mut().insert(key, face.clone());
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

    fn resolve(&self, key: FaceKey) -> Result<FontFace> {
        let name: Vec<u16> = self
            .ladder
            .name(key.family)
            .encode_utf16()
            .chain(core::iter::once(0))
            .collect();

        // SAFETY: every out-parameter is a stack local outliving its call, and `name` is
        // a NUL-terminated buffer alive for the whole of `FindFamilyName`.
        unsafe {
            let (mut index, mut exists) = (0u32, windows_core::BOOL::from(false));
            self.collection
                .FindFamilyName(
                    windows_core::PCWSTR(name.as_ptr()),
                    &mut index,
                    &mut exists,
                )
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

/// `DWRITE_E_NOFONT` — the family is not installed, so nothing can be drawn in it. Not in
/// the metadata as a constant, and it is the one `HRESULT` this crate has to name.
fn no_font() -> windows_core::Error {
    windows_core::Error::from(windows_core::HRESULT(-2003283965))
}
