//! The COM value types automation speaks in.
//!
//! Every one of these is a constructor and nothing else. They are here so that the
//! provider reads as the answers it gives rather than as union field writes, and so the
//! one `unsafe` block per shape is written once.

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

/// The answer to a property this element does not have.
///
/// **Not an error.** A provider that fails an unsupported property makes a client log
/// noise for every element it walks; `VT_EMPTY` is how the contract says "nothing here".
#[must_use]
pub fn empty() -> VARIANT {
    variant(VT_EMPTY, VARIANT_0_0_0 { llVal: 0 })
}

#[must_use]
pub fn i4(value: i32) -> VARIANT {
    variant(VT_I4, VARIANT_0_0_0 { lVal: value })
}

#[must_use]
pub fn r8(value: f64) -> VARIANT {
    variant(VT_R8, VARIANT_0_0_0 { dblVal: value })
}

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

/// A UTF-16 slice as a `BSTR`, which is the only string automation accepts.
///
/// The tree stores its strings as UTF-16 for exactly this: the conversion is a length
/// prefix and a memcpy, with no transcode on a path a client walks element by element.
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

/// The same, kept as a `BSTR` for the methods that return one directly.
#[must_use]
pub fn bstr(value: &[u16]) -> BSTR {
    BSTR::from_wide(value)
}

/// An element, as automation returns one inside a property.
///
/// The variant takes a reference of its own, which the caller's is independent of.
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

/// A `SAFEARRAY` of `count` elements of `vt`, each written by `fill`.
///
/// One helper for the three array shapes automation asks for, because the difference
/// between them is a variant type and a stride, and three copies of this loop is three
/// places to get the index arithmetic wrong.
fn array<T>(
    vt: VARTYPE,
    values: &[T],
    fill: impl Fn(&T) -> *const core::ffi::c_void,
) -> *mut SAFEARRAY {
    // SAFETY: a vector of the requested length, each index written once and in range. A
    // null return is an allocation failure, which the caller reports as a failed call.
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

/// Rectangles, as automation wants them: one flat array of `left, top, width, height`.
#[must_use]
pub fn rect_array(values: &[f64]) -> *mut SAFEARRAY {
    array(VT_R8, values, |value| (&raw const *value).cast())
}

/// Providers. `SafeArrayPutElement` takes its own reference to each, so the caller's
/// references are still the caller's.
#[must_use]
pub fn provider_array(values: &[crate::bindings::IRawElementProviderSimple]) -> *mut SAFEARRAY {
    array(VT_UNKNOWN, values, |value| {
        windows_core::Interface::as_raw(value).cast_const()
    })
}

/// Text ranges, the same way.
#[must_use]
pub fn range_array(values: &[crate::bindings::ITextRangeProvider]) -> *mut SAFEARRAY {
    array(VT_UNKNOWN, values, |value| {
        windows_core::Interface::as_raw(value).cast_const()
    })
}

/// A runtime id: the append marker, then the two numbers that identify the element.
///
/// Stable across a republish, because a [`ControlId`](windows_scene::ControlId) is
/// generational and lives as long as its mount. That is what lets a client correlate the
/// element it is reading with the one an event named.
#[must_use]
pub fn runtime_id(id: u32, part: u32) -> *mut SAFEARRAY {
    // SAFETY: a vector of three `i32`s, each written once at a valid index. A null return
    // is an allocation failure, which the caller reports as a failed property.
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
