//! Placing a sample in time.
//!
//! Two rates, and neither is a fallback for the other — they answer different questions:
//!
//! * **[`Rate::PerMs`]** puts samples *faster than the display*, which is the whole point
//!   of a drag-fidelity test: a stack that consumes the newest sample instead of the batch
//!   is indistinguishable from a correct one until the input outruns the frame clock.
//! * **[`Rate::PerFrame`]** puts one sample per composition frame, waited on the
//!   compositor's own clock rather than on a duration derived from a refresh rate we asked
//!   for separately. There is no arithmetic to be wrong.
//!
//! **`thread::sleep` appears nowhere here.** Its resolution is the scheduler's tick — on a
//! default system, 15.6 ms — so a harness built on it cannot express a 1 ms rate at all,
//! and a "900 ms hold" would be 900 ms plus a coin flip. The high-resolution waitable
//! timer expresses both without `timeBeginPeriod`, which is a system-wide power cost this
//! process has no claim to.

use crate::bindings::*;
use crate::{Error, Result};

/// How fast a multi-sample operation places its samples.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Rate {
    /// One sample every `n` milliseconds. `PerMs(0)` waits not at all, which is as fast as
    /// the injection call returns and faster than any device reports.
    PerMs(u32),
    /// One sample per composition frame, waited on `DCompositionWaitForCompositorClock`.
    PerFrame,
}

/// The clocks a stream waits on, owned once for the injector's life.
///
/// The timer is created on first use rather than at construction: a run that only ever
/// asks for [`Rate::PerFrame`] never needs one, and a kernel object nothing waits on is a
/// handle leaked in every such run.
#[derive(Default)]
pub(crate) struct Pace {
    timer: Option<HANDLE>,
}

impl Pace {
    /// Waits one interval of `rate`.
    pub(crate) fn wait(&mut self, rate: Rate) -> Result<()> {
        match rate {
            Rate::PerMs(0) => Ok(()),
            Rate::PerMs(ms) => self.sleep(core::time::Duration::from_millis(ms.into())),
            Rate::PerFrame => Self::frame(),
        }
    }

    /// Waits `duration`, to the timer's resolution rather than the scheduler's.
    pub(crate) fn sleep(&mut self, duration: core::time::Duration) -> Result<()> {
        if duration.is_zero() {
            return Ok(());
        }
        let timer = self.timer()?;
        // A negative due time is relative, in 100 ns units — the only form that does not
        // depend on the system clock, which a harness must not be perturbed by.
        let due = -(i64::try_from(duration.as_nanos() / 100).unwrap_or(i64::MAX));
        // SAFETY: `timer` is a live waitable timer this type owns; the due time is a stack
        // local; no completion routine and no resume are wanted.
        unsafe {
            SetWaitableTimer(
                timer,
                &due,
                0,
                None,
                core::ptr::null(),
                windows_core::BOOL(0),
            )
        }
        .ok()
        .map_err(|e| Error::call("SetWaitableTimer", e))?;
        // SAFETY: as above. `INFINITE` is correct because the timer is what ends the wait.
        let waited = unsafe { WaitForSingleObject(timer, INFINITE) };
        if waited != WAIT_OBJECT_0 as u32 {
            return Err(Error::call(
                "WaitForSingleObject",
                windows_core::Error::from_thread(),
            ));
        }
        Ok(())
    }

    /// Waits for the next composition frame.
    ///
    /// Callable from any thread and needing no compositor of its own, which is what makes
    /// it usable from the injecting thread while the window under test pumps on another.
    fn frame() -> Result<()> {
        // SAFETY: no handles are passed, so the count is zero and the pointer is null by
        // the call's own contract.
        let waited = unsafe { DCompositionWaitForCompositorClock(0, core::ptr::null(), INFINITE) };
        if waited != WAIT_OBJECT_0 as u32 {
            return Err(Error::call(
                "DCompositionWaitForCompositorClock",
                windows_core::Error::from_thread(),
            ));
        }
        Ok(())
    }

    fn timer(&mut self) -> Result<HANDLE> {
        if let Some(timer) = self.timer {
            return Ok(timer);
        }
        // SAFETY: no security attributes and no name; the flag asks for the ~0.5 ms timer
        // rather than the ~15.6 ms one, and is available at the platform floor.
        let timer = unsafe {
            CreateWaitableTimerExW(
                core::ptr::null(),
                windows_core::PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION as u32,
                TIMER_ALL_ACCESS as u32,
            )
        };
        if timer.is_null() {
            return Err(Error::call(
                "CreateWaitableTimerExW",
                windows_core::Error::from_thread(),
            ));
        }
        self.timer = Some(timer);
        Ok(timer)
    }
}

impl Drop for Pace {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.take() {
            // SAFETY: the handle was created by this type and is closed exactly once.
            unsafe {
                _ = CloseHandle(timer);
            }
        }
    }
}
