//! Composition-underline resolution: `ITfDisplayAttributeProvider` /
//! `ITfDisplayAttributeInfo` → a plain [`CompositionUnderline`] the paint side
//! reads back through [`TsfDocument`](super::TsfDocument). No drawing here — this
//! only translates the TIP's requested display attribute into paint-ready data.
//!
//! ## The resolution chain (and where integration plugs in)
//!
//! A TIP marks a composing range with the `GUID_PROP_ATTRIBUTE` property whose
//! value is a `TfGuidAtom`. Turning that into an underline is a fixed sequence,
//! and this module owns the last, load-bearing step:
//!
//! 1. enumerate the composition property's ranges + atom values
//!    (`ITfReadOnlyProperty` / `IEnumTfRanges`) — **integration** (needs the
//!    property/range interfaces; see the WndProc hook list in `tsf::mod`),
//! 2. atom → GUID (`ITfCategoryMgr::GetGUID`) — **integration**,
//! 3. GUID → `ITfDisplayAttributeInfo`
//!    (`ITfDisplayAttributeMgr::GetDisplayAttributeInfo`, which aggregates every
//!    TIP's `ITfDisplayAttributeProvider`) — [`info_for_guid`],
//! 4. `ITfDisplayAttributeInfo::GetAttributeInfo` → `TF_DISPLAYATTRIBUTE` →
//!    [`CompositionUnderline`] — [`resolve_display_attribute`].
//!
//! Steps 1–2 are a mechanical property walk cribbed 1:1 from Chromium
//! `TSFTextStore::GetDisplayAttribute` / Mozilla `TSFTextStore::GetDisplayAttribute`;
//! they are left as a documented seam so this module stays small and does not
//! pull the whole property/range/category surface into the bindings before the
//! store is actually wired.

use crate::system_bindings::{
    ITfDisplayAttributeInfo, ITfDisplayAttributeMgr, TF_DISPLAYATTRIBUTE, TF_LS_DASH, TF_LS_DOT,
    TF_LS_NONE, TF_LS_SOLID, TF_LS_SQUIGGLE,
};

/// The line style under a composing run, mirroring `TF_DA_LINESTYLE`. `Solid`
/// with `bold = false` is the ordinary "clause" underline; `Squiggly` is the
/// "converted / needs attention" style CJK TIPs use for the focused clause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnderlineStyle {
    None,
    Solid,
    Dotted,
    Dashed,
    Squiggly,
}

/// A paint-ready underline for a composition span. Deliberately plain data: the
/// editor stores it beside its composition span and the chrome paints it; the
/// color, if the TIP specified a line colour, is `Some(0x00RRGGBB)` (`COLORREF`
/// low 24 bits), else the caller uses the theme's foreground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompositionUnderline {
    pub style: UnderlineStyle,
    /// `TF_DISPLAYATTRIBUTE::fBoldLine` — a heavier line for the active clause.
    pub bold: bool,
    /// Explicit line colour as an `0x00RRGGBB` value, when the TIP set one.
    pub color: Option<u32>,
}

impl CompositionUnderline {
    /// The default clause underline when no display attribute is available (or a
    /// TIP that supplies none) — a thin solid line in the field's own colour.
    pub const fn solid() -> Self {
        Self { style: UnderlineStyle::Solid, bold: false, color: None }
    }
}

impl Default for CompositionUnderline {
    fn default() -> Self {
        Self::solid()
    }
}

/// Map a `TF_DISPLAYATTRIBUTE` line style to our enum.
fn style_from_linestyle(ls: i32) -> UnderlineStyle {
    match ls {
        x if x == TF_LS_NONE => UnderlineStyle::None,
        x if x == TF_LS_SOLID => UnderlineStyle::Solid,
        x if x == TF_LS_DOT => UnderlineStyle::Dotted,
        x if x == TF_LS_DASH => UnderlineStyle::Dashed,
        x if x == TF_LS_SQUIGGLE => UnderlineStyle::Squiggly,
        // Unknown / future style: fall back to a solid clause underline rather
        // than dropping the visual — a TIP must never make the run look inert.
        _ => UnderlineStyle::Solid,
    }
}

/// Read a `TF_DISPLAYATTRIBUTE` from a resolved `ITfDisplayAttributeInfo` and
/// translate it to a [`CompositionUnderline`]. On any COM failure — a
/// misbehaving TIP is expected — fall back to the default solid underline so the
/// composing run is always visibly marked.
pub fn resolve_display_attribute(info: &ITfDisplayAttributeInfo) -> CompositionUnderline {
    let mut da = TF_DISPLAYATTRIBUTE::default();
    // SAFETY: `da` is a valid, zeroed out-parameter for the duration of the call.
    let hr = unsafe { info.GetAttributeInfo(&mut da) };
    if hr.is_err() {
        return CompositionUnderline::solid();
    }
    from_display_attribute(&da)
}

/// Pure translation of a filled `TF_DISPLAYATTRIBUTE` — factored out so the
/// field-level mapping is unit-testable without a COM object or bindings.
fn from_display_attribute(da: &TF_DISPLAYATTRIBUTE) -> CompositionUnderline {
    // `crLine.type == TF_CT_COLORREF (1)` means the union holds a COLORREF; any
    // other type (system-index / none) leaves the paint side to pick the theme
    // colour, which is what we want for a neutral clause underline.
    const TF_CT_COLORREF: i32 = 1;
    let line_color = if da.crLine.r#type == TF_CT_COLORREF {
        // SAFETY: the union holds `cr` (a `COLORREF` = `u32`) when
        // `type == TF_CT_COLORREF`.
        Some(unsafe { da.crLine.Anonymous.cr })
    } else {
        None
    };
    underline_from_fields(da.lsStyle, da.fBoldLine.as_bool(), line_color)
}

/// The binding-free core of the mapping: line style + bold + optional line
/// colour → [`CompositionUnderline`]. Split out so it can be tested headlessly
/// (the `TF_DISPLAYATTRIBUTE` read is a thin COM shell over this).
pub fn underline_from_fields(
    ls_style: i32,
    bold: bool,
    line_color: Option<u32>,
) -> CompositionUnderline {
    CompositionUnderline {
        style: style_from_linestyle(ls_style),
        bold,
        color: line_color.map(|c| c & 0x00FF_FFFF),
    }
}

/// Resolve a display-attribute GUID (obtained from a range's `TfGuidAtom` via
/// `ITfCategoryMgr::GetGUID`) to its `ITfDisplayAttributeInfo` through the
/// process display-attribute manager, which consults every registered TIP's
/// `ITfDisplayAttributeProvider`. `None` when no TIP claims the attribute.
pub fn info_for_guid(
    mgr: &ITfDisplayAttributeMgr,
    guid: &windows_core::GUID,
) -> Option<ITfDisplayAttributeInfo> {
    let mut info: Option<ITfDisplayAttributeInfo> = None;
    let mut clsid = windows_core::GUID::zeroed();
    // SAFETY: valid out-parameters; `info` is a nullable interface slot.
    let hr = unsafe { mgr.GetDisplayAttributeInfo(guid, &mut info, &mut clsid) };
    if hr.is_err() {
        return None;
    }
    info
}
