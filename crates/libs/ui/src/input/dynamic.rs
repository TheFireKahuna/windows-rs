//! The three `user32` exports this design uses that no header on the platform floor names.
//!
//! `RegisterTouchpadCapableWindow` (which `windows-window` resolves), `GetPointerTouchpadInfo`
//! and `ReportWindowContentInertia` are all documented on Learn against Windows 11 and all
//! three are **absent from the 26100 SDK's own `winuser.h`**, which carries a redaction
//! marker — `// TODO(…): Make public when Feature_TouchpadPublicApis3 is enabled` — exactly
//! where they and the two inertia messages should be. They are absent from the vendored
//! metadata for the same reason.
//!
//! So they are resolved by name, once, and a machine that does not have them loses the
//! feature rather than the process load: a static import the running `user32` does not
//! export fails at **load**, which would mean every process linking this crate refusing to
//! start. That is the rule for a capability that genuinely may not be present — probe by
//! name, record the result, disable what depends on it — rather than build a second path.
//!
//! **What cannot be resolved this way is `WM_STOPINERTIA` and `WM_ENDINERTIA`.** A message
//! number is not an export, so there is nothing to look up and no arm to write; see
//! [`Inertia`](super::Inertia).

use crate::bindings::*;

/// `BOOL GetPointerTouchpadInfo(UINT32, POINTER_TOUCH_INFO*)`.
///
/// A touchpad contact answers in a `POINTER_TOUCH_INFO`, not a structure of its own: the
/// extended fields are identical for touch and touchpad.
type GetTouchpadInfo =
    unsafe extern "system" fn(u32, *mut POINTER_TOUCH_INFO) -> windows_core::BOOL;

/// `BOOL ReportWindowContentInertia(HWND, windows_core::BOOL)`.
type ReportInertia = unsafe extern "system" fn(HWND, windows_core::BOOL) -> windows_core::BOOL;

/// What the running `user32` turned out to have.
#[derive(Copy, Clone, Default)]
pub struct Late {
    touchpad_info: Option<GetTouchpadInfo>,
    report_inertia: Option<ReportInertia>,
}

impl Late {
    /// Resolves both, once.
    #[must_use]
    pub fn resolve() -> Self {
        // SAFETY: `user32` is loaded — this process has a window — so the handle is live and
        // needs no free, and each signature transmuted onto an address is the documented one.
        unsafe {
            let user32 = GetModuleHandleW(windows_core::w!("user32.dll"));
            if user32.is_null() {
                return Self::default();
            }
            Self {
                touchpad_info: GetProcAddress(user32, windows_core::s!("GetPointerTouchpadInfo"))
                    .map(|address| core::mem::transmute::<_, GetTouchpadInfo>(address)),
                report_inertia: GetProcAddress(
                    user32,
                    windows_core::s!("ReportWindowContentInertia"),
                )
                .map(|address| core::mem::transmute::<_, ReportInertia>(address)),
            }
        }
    }

    /// Whether precision-touchpad contact detail is readable on this build.
    #[must_use]
    pub const fn has_touchpad_info(&self) -> bool {
        self.touchpad_info.is_some()
    }

    /// Whether content inertia can be reported on this build.
    #[must_use]
    pub const fn has_inertia_reporting(&self) -> bool {
        self.report_inertia.is_some()
    }

    /// A touchpad contact's detail. `None` where the export is absent or the pointer is not
    /// a touchpad contact.
    pub(crate) fn touchpad_info(&self, id: u32) -> Option<POINTER_TOUCH_INFO> {
        let call = self.touchpad_info?;
        let mut info = POINTER_TOUCH_INFO::default();
        // SAFETY: the address resolved from the documented name, and the destination is a
        // stack local of the type that name writes.
        unsafe { call(id, &mut info) }.as_bool().then_some(info)
    }

    /// Tells the system whether `hwnd`'s content is in inertia. `false` if it could not.
    pub(crate) fn report_inertia(&self, hwnd: HWND, started: bool) -> bool {
        let Some(call) = self.report_inertia else {
            return false;
        };
        // SAFETY: as above; `hwnd` is live for the call.
        unsafe { call(hwnd, started.into()) }.as_bool()
    }
}

impl core::fmt::Debug for Late {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Late")
            .field("GetPointerTouchpadInfo", &self.has_touchpad_info())
            .field("ReportWindowContentInertia", &self.has_inertia_reporting())
            .finish()
    }
}
