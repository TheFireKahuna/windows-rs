//! Placing a sample in time: the two rates a multi-sample drive is paced at, and the clocks
//! they wait on.
//!
//! Neither rate substitutes for the other:
//!
//! * [`Rate::PerMs`] places samples faster than the display refreshes, which is what
//!   separates a stack that consumes the whole batch from one that consumes only the newest
//!   sample.
//! * [`Rate::PerFrame`] places one sample per composition frame, waited on the compositor's
//!   own clock rather than on a duration derived from a separately queried refresh rate.
//!
//! Every wait here uses a high-resolution waitable timer or the compositor clock.
//! `thread::sleep` resolves to the scheduler's tick — 15.6 ms on a default system — which
//! cannot express a 1 ms rate and turns a 900 ms hold into 900 ms plus one tick. The timer
//! needs no `timeBeginPeriod`, which would change the tick system-wide.

use crate::bindings::*;
use crate::{Error, Result};

/// How fast a multi-sample operation places its samples.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Rate {
    /// One sample every `n` milliseconds. `PerMs(0)` does not wait, so samples are placed as
    /// fast as the injection call returns, which is faster than any physical device reports.
    PerMs(u32),
    /// One sample per composition frame, waited on `DCompositionWaitForCompositorClock`.
    PerFrame,
}

/// Holds the clocks a stream waits on, for the injector's life.
///
/// The timer is created on first use: a run that only ever asks for [`Rate::PerFrame`] never
/// creates a kernel object it does not wait on.
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
        // A negative due time is relative, in 100 ns units. An absolute due time would follow
        // the system clock, which can be stepped underneath a running drive.
        let due = -(i64::try_from(duration.as_nanos() / 100).unwrap_or(i64::MAX));
        // SAFETY: `timer` comes from `Self::timer`, the only producer of this handle, and it
        // stays live until `Drop`; `due` is a stack local the call reads. No completion
        // routine and no resume are asked for.
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
        // SAFETY: `timer` is the same live handle; `INFINITE` is bounded because that timer
        // is armed and signals the wait.
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
    /// Callable from any thread and needing no compositor of its own, so the injecting thread
    /// waits on it while the window under test pumps on another.
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
        // The high-resolution flag asks for the ~0.5 ms timer rather than the ~15.6 ms one,
        // and is available at the platform floor.
        // SAFETY: a null security descriptor and a null name are the documented defaults;
        // the call reads its arguments and returns a handle this type owns.
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
            // SAFETY: `Self::timer` is the only producer of this handle, and `take` leaves
            // nothing behind for a second close.
            unsafe {
                _ = CloseHandle(timer);
            }
        }
    }
}
