//! Reports what pointer devices the machine has and how the user is holding it.
//!
//! **Affordances are per-interaction, not per-device-mode.** A touch contact gets touch
//! treatment; a mouse move gets mouse treatment; both may occur seconds apart on one machine.
//! Nothing here decides anything: [`Devices`] is diagnostic and [`Interaction`] is a hint.
//!
//! The per-interaction signal is the contact itself: `rcContactRaw` at gesture time
//! ([`Sample::contact`](super::Sample)) states how large the contact was, which no global
//! flag can.

use crate::bindings::*;
use windows_core::Result;

/// Holds what the attached digitizers report.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Devices {
    /// A touch digitizer is attached.
    pub touch: bool,
    /// A pen digitizer is attached.
    pub pen: bool,
    /// A precision touchpad is attached.
    pub touchpad: bool,
    /// The largest simultaneous contact count any attached digitizer reports. Zero where
    /// there is no digitizer at all.
    pub max_contacts: u32,
}

impl Devices {
    /// Enumerates the attached pointer devices.
    ///
    /// Read once at startup. A later change means a device arrived, which alters nothing
    /// about how a contact already in flight is treated.
    ///
    /// # Errors
    ///
    /// Fails when `GetPointerDevices` or a device property read does.
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

/// States how the user is holding the machine, **as a hint only**.
///
/// Windows 11 removed Tablet Mode and points at Convertible Slate Mode for keyboard
/// attach/detach, so the signal is weak at the platform floor by the platform's own account.
/// It is read for diagnostics — a report saying "touch mode, and yet no touch contact has
/// ever arrived" is worth having — and it decides nothing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// The machine reports a mode driven by an indirect pointer.
    Mouse,
    /// The machine reports a mode driven by direct touch.
    Touch,
}

impl Interaction {
    /// Returns the interaction mode `window` is in.
    ///
    /// A desktop app has no `CoreWindow`, so the settings come from the interop factory
    /// rather than from `GetForCurrentView`.
    ///
    /// # Errors
    ///
    /// Fails when the interop factory, `GetForWindow`, or the mode read does.
    pub fn for_window(window: &windows_window::Window) -> Result<Self> {
        let hwnd = window.hwnd();
        let interop = windows_core::factory::<UIViewSettings, IUIViewSettingsInterop>()?;
        // SAFETY: `hwnd` is live for the call, and `UIViewSettings` is the class the interop
        // factory returns for a window, so the interface asked for is one it implements.
        let settings: UIViewSettings = unsafe { interop.GetForWindow(hwnd)? };
        Ok(match settings.UserInteractionMode()? {
            UserInteractionMode::Touch => Self::Touch,
            _ => Self::Mouse,
        })
    }
}

/// Holds what the machine reports about its pointer input, read once.
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct Capability {
    /// The digitizers attached at the time of the read.
    pub devices: Devices,
    /// `None` where the mode could not be read, which is not an error: it decides nothing.
    pub interaction: Option<Interaction>,
    /// Whether precision-touchpad contact detail is readable on this build.
    pub touchpad_detail: bool,
    /// Whether content inertia can be reported to the system on this build.
    pub inertia_reporting: bool,
}

impl Capability {
    /// Reads every capability for `window`. Never fails: a capability that cannot be read is
    /// reported absent, and nothing branches on one.
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
