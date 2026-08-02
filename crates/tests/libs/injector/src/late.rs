//! The two `user32` exports this harness uses that no header on the platform floor names.
//!
//! `CreateSyntheticPointerDevice2` and `InjectTouchpadAction` are documented on Learn
//! against Windows 11 and both are **absent from the 26100 SDK's own `winuser.h`**, which
//! carries `// TODO(47499024): Make public when Feature_TouchpadPublicApis3 is enabled`
//! exactly where they and their types belong. The types are transcribed into
//! `metadata/syntheticinput.rdl` and generated; the functions are resolved here, by name.
//!
//! The reason they are not `link!`ed alongside everything else is the same one
//! `windows-ui` gives for `GetPointerTouchpadInfo`: a static import the running `user32`
//! does not export fails the **process load**, so a machine at the floor without them
//! would lose mouse and touch injection too — streams that never needed either function.
//! Probe by name, record the result, refuse the streams that depend on it.
//!
//! What cannot be resolved this way is `WM_STOPINERTIA` and `WM_ENDINERTIA`, which the same
//! redaction hides. A message number is not an export. `InjectTouchpadAction` is how they
//! are read instead: `TA_INERTIA_STOP` and `TA_INERTIA_END` are documented to produce
//! exactly one of each, to the window that last reported content inertia — see
//! `examples/inertia_numbers`.

use crate::bindings::*;
use crate::{Error, Result};

/// `HSYNTHETICPOINTERDEVICE CreateSyntheticPointerDevice2(SYNTHETIC_DEVICE_CREATION_PARAMS*)`.
type CreateDevice2 =
    unsafe extern "system" fn(*const SYNTHETIC_DEVICE_CREATION_PARAMS) -> HSYNTHETICPOINTERDEVICE;

/// `BOOL InjectTouchpadAction(HSYNTHETICPOINTERDEVICE, TOUCHPAD_ACTION)`.
type InjectAction =
    unsafe extern "system" fn(HSYNTHETICPOINTERDEVICE, TOUCHPAD_ACTION) -> windows_core::BOOL;

/// What the running `user32` turned out to have.
///
/// A capability, never a mode: a stream is refused with the export's own name in the
/// error, rather than quietly becoming a different stream.
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
    /// not sufficient — see `packaged`.
    pub synthetic_devices: bool,
    /// Whether the global touchpad gestures and the two inertia messages can be injected.
    /// Unlike a contact, an action is a message to a tracked window rather than a device
    /// sample, so this one works unpackaged.
    pub touchpad_actions: bool,
}

#[derive(Copy, Clone, Default)]
pub(crate) struct Late {
    create: Option<CreateDevice2>,
    action: Option<InjectAction>,
}

impl Late {
    /// Resolves both, once.
    pub(crate) fn resolve() -> Self {
        // SAFETY: `user32` is loaded into every process that has reached this code, so the
        // handle is live and needs no free, and each signature transmuted onto an address
        // is the documented one.
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

    /// Creates a synthetic device, or says which export was missing.
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
        // SAFETY: as above; `device` is a live device this crate created.
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
