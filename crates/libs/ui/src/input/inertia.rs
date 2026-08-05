//! Content inertia, and the system's part in it.
//!
//! Three wires make the OS a participant:
//!
//! * `ReportWindowContentInertia` — reports inertia start and end.
//! * `WM_STOPINERTIA` — a system request to stop, which a finger landing on the touchpad
//!   during a fling produces.
//! * `WM_ENDINERTIA` — the system's signal that the motion is over.
//!
//! Reporting decides what the system does with a touchpad tap: a window whose content is in
//! inertia and has not said so turns the tap into an ordinary click, so a gesture that meant
//! "stop the fling" lands as an edit to whatever was moving under it.
//!
//! # The two messages have no numbers at the platform floor
//!
//! `WM_STOPINERTIA` and `WM_ENDINERTIA` are **redacted from the 26100 SDK's own
//! `winuser.h`** — it carries `// TODO(…): Make public when Feature_TouchpadPublicApis3 is
//! enabled` exactly where they belong — and are absent from the vendored metadata. A message
//! number is not an export, so unlike [`Late`](super::dynamic::Late)'s two functions there is
//! nothing to resolve by name and no window-procedure arm to write. A guessed number gives a
//! handler that either never fires or fires on an unrelated message, and neither is
//! distinguishable from working.
//!
//! [`Router::stop_inertia`] is the entry point such an arm calls: it stops the recogniser on
//! the pacer tick exactly as a message-driven stop does. The reporting half is live.
//!
//! # Reporting can be refused, and it fails quietly
//!
//! `ReportWindowContentInertia` answers `E_ACCESSDENIED` for a window that is **not active**,
//! through a `BOOL` and nothing louder. So [`Inertia::set`] records what the system was told
//! rather than what it was asked, which is what makes the next tick retry.
//!
//! [`Router::stop_inertia`]: super::Router::stop_inertia

use super::dynamic::Late;
use crate::bindings::HWND;
use core::cell::Cell;

/// Tracks the window's content-inertia state as the system has been told it.
pub struct Inertia {
    hwnd: HWND,
    late: Late,
    /// What the system was **told**, not what it was asked — [`Inertia::set`] moves this on
    /// success only. Holding it makes the report an edge: the system tracks one window at a
    /// time and replaces what it was tracking, so reporting every frame would be a syscall
    /// per frame on a path that has to stay empty.
    reported: Cell<bool>,
}

impl Inertia {
    /// Creates an inertia reporter for `window`.
    #[must_use]
    pub fn new(window: &windows_window::Window, late: Late) -> Self {
        Self {
            hwnd: window.hwnd(),
            late,
            reported: Cell::new(false),
        }
    }

    /// Returns whether this build of `user32` exports the reporting call.
    #[must_use]
    pub const fn available(&self) -> bool {
        self.late.has_inertia_reporting()
    }

    /// Returns whether the system believes this window's content is moving.
    #[must_use]
    pub fn reported(&self) -> bool {
        self.reported.get()
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
