//! Constructs the COM value types automation speaks in: `VARIANT`s, `BSTR`s and
//! `SAFEARRAY`s.
//!
//! Every item here is a constructor, so each union write and each `SAFEARRAY` fill is
//! written once rather than at every provider method that answers with one.

use crate::bindings::{SAFEARRAY, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VARTYPE};
use windows_core::BSTR;

// Not in the generated set: automation wants a runtime id as a `SAFEARRAY` of `i32`, and
// nothing else in this crate creates one.
windows_core::link!("oleaut32.dll" "system" fn SafeArrayCreateVector(vt: u16, low: i32, count: u32) -> *mut SAFEARRAY);
windows_core::link!("oleaut32.dll" "system" fn SafeArrayPutElement(array: *mut SAFEARRAY, at: *const i32, value: *const core::ffi::c_void) -> windows_core::HRESULT);

const VT_EMPTY: VARTYPE = 0;
const VT_I4: VARTYPE = 3;
const VT_R8: VARTYPE = 5;
const VT_BSTR: VARTYPE = 8;
const VT_BOOL: VARTYPE = 11;
const VT_UNKNOWN: VARTYPE = 13;

/// First element of a fragment's runtime id: "append this to the host's".
pub const APPEND_RUNTIME_ID: i32 = 3;

fn variant(vt: VARTYPE, value: VARIANT_0_0_0) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: core::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: value,
            }),
        },
    }
}

/// Returns the `VT_EMPTY` variant, which is how a provider answers a property the element
/// does not have.
///
/// An unsupported property is answered rather than failed, so a client walking every
/// element logs nothing.
#[must_use]
pub fn empty() -> VARIANT {
    variant(VT_EMPTY, VARIANT_0_0_0 { llVal: 0 })
}

/// Returns `value` as a `VT_I4` variant.
#[must_use]
pub fn i4(value: i32) -> VARIANT {
    variant(VT_I4, VARIANT_0_0_0 { lVal: value })
}

/// Returns `value` as a `VT_R8` variant.
#[must_use]
pub fn r8(value: f64) -> VARIANT {
    variant(VT_R8, VARIANT_0_0_0 { dblVal: value })
}

/// Returns `value` as a `VT_BOOL` variant.
#[must_use]
pub fn bool(value: bool) -> VARIANT {
    variant(
        VT_BOOL,
        VARIANT_0_0_0 {
            // The OLE truth value is all-ones, not one.
            boolVal: if value { -1 } else { 0 },
        },
    )
}

/// Returns `value` as a `VT_BSTR` variant, or [`empty`] where the slice is empty.
///
/// The tree stores its strings as UTF-16, which is what automation accepts, so the
/// conversion is a length prefix and a memcpy with no transcode.
#[must_use]
pub fn wide(value: &[u16]) -> VARIANT {
    if value.is_empty() {
        return empty();
    }
    variant(
        VT_BSTR,
        VARIANT_0_0_0 {
            bstrVal: core::mem::ManuallyDrop::new(BSTR::from_wide(value)),
        },
    )
}

/// Returns `value` as a `BSTR`, for the methods that return one directly rather than
/// inside a variant.
#[must_use]
pub fn bstr(value: &[u16]) -> BSTR {
    BSTR::from_wide(value)
}

/// Returns `value` as a `VT_UNKNOWN` variant, which is how automation carries an element
/// inside a property.
///
/// The variant holds a reference of its own, so the caller keeps its own.
///
/// # Panics
///
/// Panics if `value` does not answer `IUnknown`, which every COM object does.
#[must_use]
pub fn provider(value: &crate::bindings::IRawElementProviderSimple) -> VARIANT {
    variant(
        VT_UNKNOWN,
        VARIANT_0_0_0 {
            punkVal: core::mem::ManuallyDrop::new(Some(
                windows_core::Interface::cast::<windows_core::IUnknown>(value)
                    .expect("every provider is an IUnknown"),
            )),
        },
    )
}

/// Returns a `SAFEARRAY` of `vt` holding one element per entry of `values`, each written
/// through `fill`.
///
/// `fill` yields a pointer to the bytes `SafeArrayPutElement` copies for that element, so
/// it must point at a value of `vt`. Returns null where the allocation fails, which the
/// caller reports as a failed call.
fn array<T>(
    vt: VARTYPE,
    values: &[T],
    fill: impl Fn(&T) -> *const core::ffi::c_void,
) -> *mut SAFEARRAY {
    // SAFETY: `SafeArrayCreateVector` returns either null, which is checked, or an array
    // of `values.len()` elements of `vt`. Every index written comes from enumerating
    // `values`, so it is below that length, and each pointer `fill` yields addresses a
    // value that outlives the call.
    unsafe {
        let out = SafeArrayCreateVector(vt, 0, values.len() as u32);
        if out.is_null() {
            return out;
        }
        for (at, value) in values.iter().enumerate() {
            let at = at as i32;
            _ = SafeArrayPutElement(out, &raw const at, fill(value));
        }
        out
    }
}

/// Returns `values` as a `SAFEARRAY` of doubles, the flat `left, top, width, height` runs
/// automation expects for rectangles.
#[must_use]
pub fn rect_array(values: &[f64]) -> *mut SAFEARRAY {
    array(VT_R8, values, |value| (&raw const *value).cast())
}

/// Returns `values` as a `SAFEARRAY` of providers. `SafeArrayPutElement` takes its own
/// reference to each, so the caller keeps its own.
#[must_use]
pub fn provider_array(values: &[crate::bindings::IRawElementProviderSimple]) -> *mut SAFEARRAY {
    array(VT_UNKNOWN, values, |value| {
        windows_core::Interface::as_raw(value).cast_const()
    })
}

/// Returns `values` as a `SAFEARRAY` of text ranges, on the same terms as
/// [`provider_array`].
#[must_use]
pub fn range_array(values: &[crate::bindings::ITextRangeProvider]) -> *mut SAFEARRAY {
    array(VT_UNKNOWN, values, |value| {
        windows_core::Interface::as_raw(value).cast_const()
    })
}

/// Returns a runtime id: [`APPEND_RUNTIME_ID`], then the control id and part that identify
/// the element.
///
/// The id is stable across a republish, because a
/// [`ControlId`](windows_scene::ControlId) is generational and lives as long as its mount,
/// which is what lets a client match the element it is reading against the one an event
/// named. Returns null where the allocation fails, which the caller reports as a failed
/// property.
#[must_use]
pub fn runtime_id(id: u32, part: u32) -> *mut SAFEARRAY {
    // SAFETY: `SafeArrayCreateVector` returns either null, which is checked, or an array
    // of three `i32`s. Each of the three indices is written once and is below that length,
    // and each source value is an `i32` alive for the duration of the call.
    unsafe {
        let array = SafeArrayCreateVector(VT_I4, 0, 3);
        if array.is_null() {
            return array;
        }
        for (at, value) in [APPEND_RUNTIME_ID, id as i32, part as i32]
            .iter()
            .enumerate()
        {
            let at = at as i32;
            _ = SafeArrayPutElement(array, &raw const at, (&raw const *value).cast());
        }
        array
    }
}
