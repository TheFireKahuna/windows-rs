//! Content inertia, and the system's part in it.
//!
//! Three wires make the OS a participant:
//!
//! * `ReportWindowContentInertia` — report inertia start and end.
//! * `WM_STOPINERTIA` — honour a system request to stop, which is what a finger landing on
//!   the touchpad during a fling produces.
//! * `WM_ENDINERTIA` — the system's signal that the motion is over.
//!
//! Reporting matters because of what the system does *instead* when it is not told: a window
//! whose content is in inertia and has not said so turns a touchpad tap into an ordinary
//! click, so the user's attempt to stop a fling lands as an edit to whatever was moving under
//! it. In this application that means an unintended change to live audio.
//!
//! # Two of the three cannot be written at the platform floor
//!
//! `WM_STOPINERTIA` and `WM_ENDINERTIA` are **redacted from the 26100 SDK's own
//! `winuser.h`** — it carries `// TODO(…): Make public when Feature_TouchpadPublicApis3 is
//! enabled` exactly where they belong — and are absent from the vendored metadata. A message
//! number is not an export, so unlike [`Late`](super::dynamic::Late)'s two functions there is
//! nothing to resolve by name and no arm that can be written. Guessing a number would give a
//! handler that either never fires or fires on an unrelated message, and neither is
//! distinguishable from working.
//!
//! So the mechanism is complete except for its trigger: [`Router::stop_inertia`] is the entry
//! point the message arm would call, the recogniser is stopped on the pacer tick exactly as a
//! message-driven stop would be, and the arm lands the day the constants are published. What
//! *is* live is the reporting half — which is the half that changes the system's behaviour.
//!
//! [`Router::stop_inertia`]: super::Router::stop_inertia

use super::dynamic::Late;
use crate::bindings::HWND;
use core::cell::Cell;

/// The window's inertia state, as the system has been told it.
pub struct Inertia {
    hwnd: HWND,
    late: Late,
    /// What was last reported, so the report is made on the **edge**. The system tracks one
    /// window at a time and replaces whatever it was tracking, so a report per frame would be
    /// a syscall per frame on the one path this design exists to keep empty.
    reported: Cell<bool>,
}

impl Inertia {
    /// An inertia reporter for `window`.
    #[must_use]
    pub fn new(window: &windows_window::Window, late: Late) -> Self {
        Self {
            hwnd: window.hwnd(),
            late,
            reported: Cell::new(false),
        }
    }

    /// Whether this build of `user32` can be told at all.
    #[must_use]
    pub const fn available(&self) -> bool {
        self.late.has_inertia_reporting()
    }

    /// Whether the system currently believes this window's content is moving.
    #[must_use]
    pub fn reported(&self) -> bool {
        self.reported.get()
    }

    /// States whether content is in inertia, reporting only on a change.
    ///
    /// The platform additionally requires that the thread has retrieved input in the last two
    /// seconds when reporting a *start*, which is always true here: a start is reached from
    /// the tick that consumed the contact which produced it.
    pub fn set(&self, moving: bool) {
        if self.reported.replace(moving) == moving {
            return;
        }
        // A refusal is not worth handling: the failure mode is the system treating a touchpad
        // tap as a click, and there is no second way to ask.
        _ = self.late.report_inertia(self.hwnd, moving);
    }
}

impl core::fmt::Debug for Inertia {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Inertia")
            .field("available", &self.available())
            .field("reported", &self.reported())
            .finish()
    }
}
