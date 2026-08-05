//! The two `user32` exports this harness resolves by name, because no header on the platform
//! floor declares them.
//!
//! `CreateSyntheticPointerDevice2` and `InjectTouchpadAction` are documented against Windows
//! 11 and absent from the 26100 SDK's own `winuser.h`, which carries
//! `// TODO(47499024): Make public when Feature_TouchpadPublicApis3 is enabled` exactly where
//! they and their types belong. The types are transcribed into `metadata/syntheticinput.rdl`
//! and generated; the functions are resolved here, by name.
//!
//! A static import of an export the running `user32` does not have fails the process load, so
//! `link!`ing these would cost mouse and touch injection on a machine at the floor — streams
//! that need neither function. They are probed by name instead, the result is reported by
//! [`Capability`], and the streams that depend on them are refused.
//!
//! `WM_STOPINERTIA` and `WM_ENDINERTIA` are hidden by the same redaction and cannot be
//! resolved this way, because a message number is not an export. `InjectTouchpadAction`
//! reaches them instead: `TA_INERTIA_STOP` and `TA_INERTIA_END` each produce exactly one of
//! them, to the window that last reported content inertia.

use crate::bindings::*;
use crate::{Error, Result};

/// `HSYNTHETICPOINTERDEVICE CreateSyntheticPointerDevice2(SYNTHETIC_DEVICE_CREATION_PARAMS*)`.
type CreateDevice2 =
    unsafe extern "system" fn(*const SYNTHETIC_DEVICE_CREATION_PARAMS) -> HSYNTHETICPOINTERDEVICE;

/// `BOOL InjectTouchpadAction(HSYNTHETICPOINTERDEVICE, TOUCHPAD_ACTION)`.
type InjectAction =
    unsafe extern "system" fn(HSYNTHETICPOINTERDEVICE, TOUCHPAD_ACTION) -> windows_core::BOOL;

/// Reports what this process and the running `user32` can inject.
///
/// A stream that depends on a missing entry is refused with that entry's name in the error,
/// rather than becoming a different stream.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Capability {
    /// Whether this process could create a virtual input device at all.
    ///
    /// `Windows.UI.Input.Preview.Injection` and `CreateSyntheticPointerDevice2` both need the
    /// `inputInjectionBrokered` restricted capability, which is declared in a package
    /// manifest — so an unpackaged process does not have it and cannot acquire it. This is
    /// package identity, which is the testable proxy: having it does not prove the capability
    /// was declared, but not having it proves the capability cannot have been.
    pub packaged: bool,
    /// Whether `CreateSyntheticPointerDevice2` resolved. Necessary for a touchpad stream, and
    /// not sufficient: [`Capability::packaged`] is the other half.
    pub synthetic_devices: bool,
    /// Whether the global touchpad gestures and the two inertia messages can be injected.
    /// An action is a message to a tracked window rather than a device sample, so it needs no
    /// package identity.
    pub touchpad_actions: bool,
}

#[derive(Copy, Clone, Default)]
pub(crate) struct Late {
    create: Option<CreateDevice2>,
    action: Option<InjectAction>,
}

impl Late {
    /// Resolves both exports, once.
    pub(crate) fn resolve() -> Self {
        // SAFETY: `user32` is loaded into every process that reaches this code, so
        // `GetModuleHandleW` returns a live borrowed handle that needs no free, and each
        // transmuted signature is the documented one for the name it resolved from.
        unsafe {
            let user32 = GetModuleHandleW(windows_core::w!("user32.dll"));
            if user32.is_null() {
                return Self::default();
            }
            Self {
                create: GetProcAddress(user32, windows_core::s!("CreateSyntheticPointerDevice2"))
                    .map(|address| core::mem::transmute::<_, CreateDevice2>(address)),
                action: GetProcAddress(user32, windows_core::s!("InjectTouchpadAction"))
                    .map(|address| core::mem::transmute::<_, InjectAction>(address)),
            }
        }
    }

    pub(crate) fn capability(&self) -> Capability {
        Capability {
            packaged: crate::identity::packaged(),
            synthetic_devices: self.create.is_some(),
            touchpad_actions: self.action.is_some(),
        }
    }

    /// Creates a synthetic device, or reports which export is missing.
    pub(crate) fn create_device(
        &self,
        params: &SYNTHETIC_DEVICE_CREATION_PARAMS,
    ) -> Result<HSYNTHETICPOINTERDEVICE> {
        let call = self.create.ok_or(Error::Unavailable {
            export: "CreateSyntheticPointerDevice2",
        })?;
        // SAFETY: the address resolved from the documented name, and `params` is a
        // reference to a fully initialized structure of the type that name reads.
        let device = unsafe { call(params) };
        if device.is_null() {
            return Err(Error::call(
                "CreateSyntheticPointerDevice2",
                windows_core::Error::from_thread(),
            ));
        }
        Ok(device)
    }

    /// Injects one global touchpad action.
    pub(crate) fn inject_action(
        &self,
        device: HSYNTHETICPOINTERDEVICE,
        action: TOUCHPAD_ACTION,
    ) -> Result<()> {
        let call = self.action.ok_or(Error::Unavailable {
            export: "InjectTouchpadAction",
        })?;
        // SAFETY: the address resolved from the documented name, and `device` is a handle
        // owned by a live `Device`, which destroys it only on drop.
        unsafe { call(device, action) }
            .ok()
            .map_err(|e| Error::call("InjectTouchpadAction", e))
    }
}

impl core::fmt::Debug for Late {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.capability().fmt(f)
    }
}
