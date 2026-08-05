//! Resolves by name the three `user32` exports this crate uses that no header at the platform
//! floor declares.
//!
//! `RegisterTouchpadCapableWindow` (which `windows-window` resolves), `GetPointerTouchpadInfo`
//! and `ReportWindowContentInertia` are all documented on Learn against Windows 11 and all
//! three are **absent from the 26100 SDK's own `winuser.h`**, which carries a redaction
//! marker — `// TODO(…): Make public when Feature_TouchpadPublicApis3 is enabled` — exactly
//! where they and the two inertia messages belong. They are absent from the vendored metadata
//! for the same reason.
//!
//! A static import of a symbol the running `user32` does not export fails at **load**, which
//! would stop every process linking this crate from starting. Resolving by name costs a
//! machine without them the feature instead: probe by name, record the result, and disable
//! what depends on it.
//!
//! `WM_STOPINERTIA` and `WM_ENDINERTIA` cannot be resolved this way. A message number is not
//! an export, so there is nothing to look up and no window-procedure arm to write; see
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

/// Holds the entry points the running `user32` exports, empty where it does not.
#[derive(Copy, Clone, Default)]
pub struct Late {
    touchpad_info: Option<GetTouchpadInfo>,
    report_inertia: Option<ReportInertia>,
}

impl Late {
    /// Resolves both entry points from the loaded `user32`.
    ///
    /// An export that is absent leaves its slot empty and the capability reads as
    /// unavailable; the module failing to load does the same for both.
    #[must_use]
    pub fn resolve() -> Self {
        // SAFETY: `GetModuleHandleW` answers a borrowed handle that needs no free, and
        // `user32` stays loaded for as long as this process has a window. Each address is
        // transmuted to the signature the documentation gives for the name it resolved from.
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

    /// Returns a touchpad contact's detail. `None` where the export is absent or `id` is not
    /// a touchpad contact.
    pub(crate) fn touchpad_info(&self, id: u32) -> Option<POINTER_TOUCH_INFO> {
        let call = self.touchpad_info?;
        let mut info = POINTER_TOUCH_INFO::default();
        // SAFETY: the address was resolved from the documented name and holds that name's
        // signature, and `info` is a stack local of the type it writes.
        unsafe { call(id, &mut info) }.as_bool().then_some(info)
    }

    /// Tells the system whether `hwnd`'s content is in inertia, and returns whether the call
    /// succeeded.
    ///
    /// `false` covers both an absent export and a refusal: the call answers `E_ACCESSDENIED`
    /// for a window that is not active. [`Inertia::set`](super::Inertia::set) retries on
    /// either.
    pub(crate) fn report_inertia(&self, hwnd: HWND, started: bool) -> bool {
        let Some(call) = self.report_inertia else {
            return false;
        };
        // SAFETY: the address was resolved from the documented name and holds that name's
        // signature, and `hwnd` is live for the call.
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
