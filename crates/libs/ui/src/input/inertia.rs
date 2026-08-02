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
//! it — a destructive answer to a gesture that meant "stop".
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
//! # Reporting can be refused, and it fails quietly
//!
//! `ReportWindowContentInertia` answers `E_ACCESSDENIED` for a window that is **not active**,
//! through a `BOOL` and nothing louder. So [`Inertia::set`] records what the system was told
//! rather than what it was asked, which is also what makes the next tick retry.
//!
//! [`Router::stop_inertia`]: super::Router::stop_inertia

use super::dynamic::Late;
use crate::bindings::HWND;
use core::cell::Cell;

/// The window's inertia state, as the system has been told it.
pub struct Inertia {
    hwnd: HWND,
    late: Late,
    /// What the system was **told**, so the report is made on the edge: it tracks one window
    /// at a time and replaces what it was tracking, so a report per frame would be a syscall
    /// per frame on the one path that has to stay empty. Told, not asked — see
    /// [`Inertia::set`].
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

    /// States whether content is in inertia, reporting only on a change, and answers whether
    /// the system now knows.
    ///
    /// **`reported` moves on success, not on intent.** Recording the intent would consume the
    /// edge — the next tick sees no change and never tries again — so a refusal would be
    /// permanent. Since this is called every tick with the current state, leaving it alone
    /// *is* the retry, and the retry ends with the motion that asks for the ticks.
    ///
    /// The platform also wants the thread to have retrieved input in the last two seconds when
    /// reporting a *start*, which is always true here: a start is reached from the tick that
    /// consumed the contact producing it.
    pub fn set(&self, moving: bool) -> bool {
        if self.reported.get() == moving {
            return moving;
        }
        if self.late.report_inertia(self.hwnd, moving) {
            self.reported.set(moving);
        }
        self.reported.get()
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
