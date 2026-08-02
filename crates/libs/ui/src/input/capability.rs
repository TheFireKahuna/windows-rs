//! What the machine can do, and what it is being used as.
//!
//! **Affordances are per-interaction, not per-device-mode.** A touch contact gets touch
//! treatment; a mouse move gets mouse treatment; both may occur seconds apart on one machine.
//! **Nothing in this design branches on a global "touch mode"** — which is why everything
//! here is diagnostic, and why the one thing that looks like a mode switch is documented as a
//! hint.
//!
//! The honest per-interaction signal is the contact itself: `rcContactRaw` at gesture time
//! ([`Sample::contact`](super::Sample)) says how big the thing touching the screen actually
//! was, which no global flag can.

use crate::bindings::*;
use windows_core::Result;

/// What the attached digitizers report.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Devices {
    pub touch: bool,
    pub pen: bool,
    pub touchpad: bool,
    /// The largest simultaneous contact count any attached digitizer reports. Zero where
    /// there is no digitizer at all.
    pub max_contacts: u32,
}

impl Devices {
    /// Enumerates the pointer devices. Called once at startup and on nothing else: this is a
    /// capability, and a capability that changes is a device arriving, which changes nothing
    /// about how a contact that has already arrived is treated.
    pub fn enumerate() -> Result<Self> {
        let mut devices = Self::default();
        for device in PointerDevice::GetPointerDevices()? {
            let kind = device.PointerDeviceType()?;
            devices.touch |= kind == PointerDeviceType::Touch;
            devices.pen |= kind == PointerDeviceType::Pen;
            devices.touchpad |= kind == PointerDeviceType::Touchpad;
            devices.max_contacts = devices.max_contacts.max(device.MaxContacts()?);
        }
        Ok(devices)
    }
}

/// How the user is currently holding the machine, **as a hint only**.
///
/// Windows 11 removed Tablet Mode and points at Convertible Slate Mode for keyboard
/// attach/detach, so this is weak on the platform floor by the platform's own account. It is
/// read for diagnostics — a report that says "touch mode, and yet no touch contact has ever
/// arrived" is worth having — and it decides nothing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Interaction {
    Mouse,
    Touch,
}

impl Interaction {
    /// The mode the window is in. A desktop app has no `CoreWindow`, so this goes through the
    /// interop factory rather than `GetForCurrentView`.
    pub fn for_window(window: &windows_window::Window) -> Result<Self> {
        let hwnd = window.hwnd();
        let interop = windows_core::factory::<UIViewSettings, IUIViewSettingsInterop>()?;
        // SAFETY: `hwnd` is live for the call, and the interface it is asked for is the one
        // the returned object implements.
        let settings: UIViewSettings = unsafe { interop.GetForWindow(hwnd)? };
        Ok(match settings.UserInteractionMode()? {
            UserInteractionMode::Touch => Self::Touch,
            _ => Self::Mouse,
        })
    }
}

/// Everything the machine reports about itself, read once.
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct Capability {
    pub devices: Devices,
    /// `None` where the mode could not be read, which is not an error: it decides nothing.
    pub interaction: Option<Interaction>,
    /// Whether precision-touchpad contact detail is readable on this build.
    pub touchpad_detail: bool,
    /// Whether content inertia can be reported to the system on this build.
    pub inertia_reporting: bool,
}

impl Capability {
    /// Reads what this machine reports. Never fails: an unreadable capability is one this
    /// design does not branch on.
    #[must_use]
    pub fn read(window: &windows_window::Window, late: &super::dynamic::Late) -> Self {
        Self {
            devices: Devices::enumerate().unwrap_or_default(),
            interaction: Interaction::for_window(window).ok(),
            touchpad_detail: late.has_touchpad_info(),
            inertia_reporting: late.has_inertia_reporting(),
        }
    }
}
