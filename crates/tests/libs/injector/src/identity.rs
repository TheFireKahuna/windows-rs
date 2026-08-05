//! Whether this process may create a virtual input device.
//!
//! `Windows.UI.Input.Preview.Injection` requires the `inputInjectionBrokered` restricted
//! capability, and a capability is declared in a package manifest, so an unpackaged process
//! does not have it and cannot acquire it at run time. That decides which calls deliver:
//!
//! | | Needs a virtual device | Delivers unpackaged |
//! |---|---|---|
//! | `InjectMouseInput`, `InjectKeyboardInput` | no | **yes** |
//! | `InitializeTouchInjection` + `InjectTouchInput` (**Win32**) | no | **yes** |
//! | `InitializeTouchInjection` + `InjectTouchInput` (**WinRT**) | yes | no |
//! | `InitializePenInjection` + `InjectPenInput` | yes | no |
//! | `CreateSyntheticPointerDevice` / `…2` | yes | no |
//! | `InjectTouchpadAction` | no — it is a message to a tracked window | **yes** |
//!
//! Every one of the "no" rows returns success and delivers nothing. There is no error to
//! read, so the check happens up front: a stream that cannot deliver is refused with the
//! capability's name in the error rather than running and asserting nothing.
//!
//! Package identity is the testable proxy. Having it does not prove the capability was
//! declared, but not having it proves the capability cannot have been.

use crate::bindings::*;

/// Returns whether this process has package identity, and so whether it could carry a
/// restricted capability at all.
pub(crate) fn packaged() -> bool {
    let mut length = 0u32;
    // SAFETY: a zero length with a null buffer is the documented way to ask for the size;
    // the call writes only through the length pointer, which is a stack local.
    let answer = unsafe { GetCurrentPackageFullName(&mut length, windows_core::PWSTR::null()) };
    answer != APPMODEL_ERROR_NO_PACKAGE
}
