//! The Win32 calls the input thread makes that are not input, each with its documented quirk.
//!
//! Four things sit here because they are the same thing: one platform call, one fact about how
//! it misbehaves, and no state beyond what the call needs to be made again.
//!
//! # Two exports no header at the platform floor declares
//!
//! `GetPointerTouchpadInfo` and `ReportWindowContentInertia` are documented on Learn against
//! Windows 11 and both are **absent from the 26100 SDK's own `winuser.h`**, which carries a
//! redaction marker — `// TODO(…): Make public when Feature_TouchpadPublicApis3 is enabled` —
//! exactly where they and the two inertia messages belong. They are absent from the vendored
//! metadata for the same reason.
//!
//! A static import of a symbol the running `user32` does not export fails at **load**, which
//! would stop every process linking this crate from starting. Resolving by name costs a
//! machine without them the feature instead.
//!
//! `WM_STOPINERTIA` and `WM_ENDINERTIA` cannot be resolved this way. A message number is not
//! an export, so there is nothing to look up, and neither number is in the generated
//! bindings, so there is no constant to write a window-procedure arm with.
//! [`Router::stop_inertia`](super::Router::stop_inertia) is the entry point such an arm calls:
//! it stops the recogniser on the pacer tick exactly as a message-driven stop does.
//!
//! # Reporting inertia can be refused, and it fails quietly
//!
//! `ReportWindowContentInertia` answers `E_ACCESSDENIED` for a window that is **not active**,
//! through a `BOOL` and nothing louder. So [`Inertia::set`] records what the system was told
//! rather than what it was asked, which is what makes the next tick retry. Reporting decides
//! what the system does with a touchpad tap: a window whose content is in inertia and has not
//! said so turns the tap into an ordinary click, so a gesture that meant "stop the fling"
//! lands as an edit to whatever was moving under it.
//!
//! # Nothing here decides anything
//!
//! [`Capability`] is read once and is diagnostic. Affordances are per-interaction, not
//! per-device-mode: a touch contact gets touch treatment, a mouse move gets mouse treatment,
//! and the per-interaction signal is the contact patch on the sample, which no global flag
//! can state. Windows 11 removed Tablet Mode and points at Convertible Slate Mode for
//! keyboard attach and detach, so the mode is weak at the platform floor by the platform's
//! own account.

use crate::bindings::*;
use core::cell::Cell;

/// `BOOL GetPointerTouchpadInfo(UINT32, POINTER_TOUCH_INFO*)`.
///
/// A touchpad contact answers in a `POINTER_TOUCH_INFO`, not a structure of its own: the
/// extended fields are identical for touch and touchpad.
type GetTouchpadInfo =
    unsafe extern "system" fn(u32, *mut POINTER_TOUCH_INFO) -> windows_core::BOOL;

/// `BOOL ReportWindowContentInertia(HWND, windows_core::BOOL)`.
type ReportInertia = unsafe extern "system" fn(HWND, windows_core::BOOL) -> windows_core::BOOL;

/// Holds the entry points the running `user32` exports, empty where it does not.
#[derive(Copy, Clone, Default, Debug)]
pub struct Late {
    touchpad_info: Option<GetTouchpadInfo>,
    report_inertia: Option<ReportInertia>,
}

impl Late {
    /// Resolves both entry points from the loaded `user32`.
    ///
    /// An export that is absent leaves its slot empty and the capability reads as
    /// unavailable.
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
                    .map(|found| core::mem::transmute::<_, GetTouchpadInfo>(found)),
                report_inertia: GetProcAddress(
                    user32,
                    windows_core::s!("ReportWindowContentInertia"),
                )
                .map(|found| core::mem::transmute::<_, ReportInertia>(found)),
            }
        }
    }

    /// Returns a touchpad contact's detail. `None` where the export is absent or `id` is not a
    /// touchpad contact.
    pub(crate) fn touchpad_info(&self, id: u32) -> Option<POINTER_TOUCH_INFO> {
        let call = self.touchpad_info?;
        let mut info = POINTER_TOUCH_INFO::default();
        // SAFETY: the address was resolved from the documented name and holds that name's
        // signature, and `info` is a stack local of the type it writes.
        unsafe { call(id, &mut info) }.as_bool().then_some(info)
    }
}

/// What the machine reports about its pointer input, read once at startup.
///
/// A later change means a device arrived, which alters nothing about how a contact already in
/// flight is treated. A capability that cannot be read is reported absent, and nothing
/// branches on one.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Capability {
    pub touch: bool,
    pub pen: bool,
    pub touchpad: bool,
    /// The largest simultaneous contact count any attached digitizer reports. Zero where there
    /// is no digitizer at all.
    pub max_contacts: u32,
    /// Whether the machine says it is being held for direct touch. `None` where the mode could
    /// not be read, which is not an error: a hint decides nothing.
    pub touch_mode: Option<bool>,
    /// Whether precision-touchpad contact detail is readable on this build.
    pub touchpad_info: bool,
    /// Whether content inertia can be reported to the system on this build.
    pub inertia_reporting: bool,
}

impl Capability {
    /// Reads every capability for `window`. Never fails.
    #[must_use]
    pub fn read(window: &windows_window::Window, late: &Late) -> Self {
        let mut out = Self {
            touchpad_info: late.touchpad_info.is_some(),
            inertia_reporting: late.report_inertia.is_some(),
            touch_mode: touch_mode(window).ok(),
            ..Self::default()
        };
        let Ok(devices) = PointerDevice::GetPointerDevices() else {
            return out;
        };
        for device in devices {
            match device.PointerDeviceType() {
                Ok(PointerDeviceType::Touch) => out.touch = true,
                Ok(PointerDeviceType::Pen) => out.pen = true,
                Ok(PointerDeviceType::Touchpad) => out.touchpad = true,
                _ => continue,
            }
            out.max_contacts = out.max_contacts.max(device.MaxContacts().unwrap_or(0));
        }
        out
    }
}

/// Returns whether the machine reports a mode driven by direct touch.
///
/// A desktop app has no `CoreWindow`, so the settings come from the interop factory rather
/// than from `GetForCurrentView`.
///
/// # Errors
///
/// The interop factory, `GetForWindow`, or the mode read refused.
fn touch_mode(window: &windows_window::Window) -> windows_core::Result<bool> {
    let interop = windows_core::factory::<UIViewSettings, IUIViewSettingsInterop>()?;
    // SAFETY: `hwnd` belongs to `window`, borrowed for the whole call, so the handle cannot be
    // destroyed under it; `UIViewSettings` is the class the interop factory returns for a
    // window, so the interface asked for is one it implements.
    let settings: UIViewSettings = unsafe { interop.GetForWindow(window.hwnd())? };
    Ok(settings.UserInteractionMode()? == UserInteractionMode::Touch)
}

/// Tracks the window's content-inertia state as the system has been told it.
#[derive(Debug)]
pub struct Inertia {
    hwnd: HWND,
    late: Late,
    /// What the system was **told**, not what it was asked. Holding it makes the report an
    /// edge: the system tracks one window at a time and replaces what it was tracking, so
    /// reporting every frame would be a syscall per frame on a path that has to stay empty.
    told: bool,
}

impl Inertia {
    /// Creates an inertia reporter for `window`.
    #[must_use]
    pub fn new(window: &windows_window::Window, late: Late) -> Self {
        Self {
            hwnd: window.input_window().raw(),
            late,
            told: false,
        }
    }

    /// States whether content is in inertia, reporting only on a change, and returns whether
    /// the system now knows.
    ///
    /// **The record moves on success, not on intent.** Recording the intent would consume the
    /// edge — the next tick sees no change and never tries again — so a refusal would be
    /// permanent. The caller passes the current state every tick, so leaving the record alone
    /// *is* the retry, and the retries end with the motion that asks for the ticks.
    ///
    /// The platform also wants the thread to have retrieved input in the last two seconds when
    /// a *start* is reported, which holds here: a start is reached from the tick that consumed
    /// the contact producing it.
    pub fn set(&mut self, moving: bool) -> bool {
        if moving != self.told
            && let Some(call) = self.late.report_inertia
            // SAFETY: the address was resolved from the documented name and holds that name's
            // signature, and `hwnd` is live for the call.
            && unsafe { call(self.hwnd, moving.into()) }.as_bool()
        {
            self.told = moving;
        }
        self.told == moving
    }
}

/// Gates one window's requests for an immediate tick, shared by every producer with something
/// latency-critical to hand over.
///
/// A press, a release, a keystroke and a dial detent do not batch and are not
/// per-frame quantities, so making any of them wait for the display buys a frame of latency.
/// Motion does not come here, and neither does anything the frame clock is *for*.
///
/// The message posted is the **pacer's own**, so this is not a second consumption path: the
/// tick that services the ring is the same code either way, drains in the same order and
/// publishes the same way. Only *when* it runs differs.
#[derive(Default, Debug)]
pub struct Service {
    /// Null until [`Service::attach`] names the window. A doorbell is installed into the
    /// window builder, so it predates the window it serves.
    target: Cell<HWND>,
    /// Whether a request is already in flight. This is the whole of the coalescing: a burst of
    /// contacts lifting together asks once.
    posted: Cell<bool>,
}

impl Service {
    /// Names the window that requests are posted to.
    pub fn attach(&self, hwnd: HWND) {
        self.target.set(hwnd);
    }

    /// Asks for a tick on the next pump iteration. Does nothing when a request is already in
    /// flight or no window is attached.
    pub fn now(&self) {
        let target = self.target.get();
        if target.is_null() || self.posted.replace(true) {
            return;
        }
        // Reusing the pacer's message also re-opens the pacer's own post gate, so the display
        // may post one extra frame after this. That tick finds nothing pending and does
        // nothing.
        //
        // SAFETY: posting is callable from any thread and resolves the handle itself; a window
        // that has gone refuses the post, which is why the result is dropped. The tick
        // re-opens the gate, so a refusal cannot wedge it shut.
        unsafe {
            _ = PostMessageW(target, windows_window::WM_FRAME, 0, 0);
        }
    }

    /// Re-opens the gate.
    ///
    /// The tick calls this **before** it drains, so a transition arriving during the drain
    /// asks again rather than being swallowed.
    pub fn begin(&self) {
        self.posted.set(false);
    }
}
