//! Whether this process may create a virtual input device.
//!
//! `Windows.UI.Input.Preview.Injection` is documented to require the
//! **`inputInjectionBrokered` restricted capability**, and a capability is declared in a
//! package manifest — so an unpackaged process does not have it and cannot acquire it at
//! run time. That one fact explains every silent failure this crate ran into:
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
//! Every one of the "no" rows **returns success**. There is no error to read, which is why
//! this is checked rather than discovered: a stream that cannot be delivered is refused
//! with the capability's own name in it, instead of running and asserting nothing.
//!
//! Package identity is the testable proxy. Having it does not prove the capability was
//! declared, but *not* having it proves the capability cannot have been.

use crate::bindings::*;

/// Whether this process has package identity, and therefore whether it could carry a
/// restricted capability at all.
pub(crate) fn packaged() -> bool {
    let mut length = 0u32;
    // SAFETY: a zero length with a null buffer is the documented way to ask for the size;
    // the call writes only through the length pointer, which is a stack local.
    let answer = unsafe { GetCurrentPackageFullName(&mut length, windows_core::PWSTR::null()) };
    answer != APPMODEL_ERROR_NO_PACKAGE
}
