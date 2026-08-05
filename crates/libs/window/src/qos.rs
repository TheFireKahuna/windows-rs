//! What a thread asks the scheduler for: an execution-speed request on the calling thread.
//!
//! The per-thread lever rather than the process-wide one, which cannot express a process
//! running its front thread at full speed while its present thread is parked.

use crate::bindings::*;

/// Selects how the calling thread asks to be scheduled.
///
/// All three states the system distinguishes. Which one fits depends on whether the reason a
/// thread went idle is visible from outside it: Windows demotes a minimized or fully-occluded
/// window's process on its own, so a window thread takes [`Managed`] and leaves it to the
/// system, while a present thread's idleness — a display that is off, or a producer with
/// nothing to publish — appears in no window state, so that thread asks for [`Eco`] outright.
///
/// [`Managed`]: Self::Managed
/// [`Eco`]: Self::Eco
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Speed {
    /// This thread's latency is observable. Do not throttle it.
    Full,
    /// This thread has stopped for a reason nothing outside it can infer. Throttle it.
    Eco,
    /// The system's own heuristic decides.
    #[default]
    Managed,
}

/// Tags the **calling** thread with `speed`.
pub fn set(speed: Speed) {
    // Three encodings: control set with state clear asks for full speed, control set with
    // state set is an explicit EcoQoS request, and control clear hands the decision back to
    // the system. An explicit request stands until it is withdrawn, so a caller that cannot
    // say when to withdraw one asks for `Speed::Managed`.
    let mut state = THREAD_POWER_THROTTLING_STATE {
        Version: THREAD_POWER_THROTTLING_CURRENT_VERSION as u32,
        ControlMask: match speed {
            Speed::Full | Speed::Eco => THREAD_POWER_THROTTLING_EXECUTION_SPEED as u32,
            Speed::Managed => 0,
        },
        StateMask: match speed {
            Speed::Eco => THREAD_POWER_THROTTLING_EXECUTION_SPEED as u32,
            Speed::Full | Speed::Managed => 0,
        },
    };
    // SAFETY: the pseudo-handle names the calling thread and needs no close; the descriptor is
    // a stack local of the stated size.
    unsafe {
        _ = SetThreadInformation(
            GetCurrentThread(),
            ThreadPowerThrottling,
            (&raw mut state).cast(),
            size_of::<THREAD_POWER_THROTTLING_STATE>() as u32,
        );
    }
}
