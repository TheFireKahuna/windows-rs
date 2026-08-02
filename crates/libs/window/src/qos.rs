//! What a thread asks the scheduler for.
//!
//! The thread lever rather than the process-wide twin: one process legitimately runs a front
//! thread at full speed while its present thread is parked, which the process-wide form cannot
//! express.

use crate::bindings::*;

/// How the calling thread asks to be scheduled.
///
/// All three states the system distinguishes, because the right one depends on whether the
/// *reason* a thread went idle is visible from outside it. A window thread's is: Windows
/// already demotes a minimized or fully-occluded window's process on its own, so [`Managed`]
/// says nothing and lets it. A present thread's is not — a display that is off, or a producer
/// with nothing to publish, shows up in no window state — so it says [`Eco`] outright.
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

/// Tags the **calling** thread.
pub fn set(speed: Speed) {
    // Three encodings, and the third is not "neither": control set with state clear asks for
    // full speed, control set with state set is an explicit EcoQoS request, and control clear
    // hands the decision back. An explicit request outlives its reason — nothing re-raises one
    // that was never made — so a caller that cannot say when to withdraw it wants `Managed`
    // rather than a request it will not take back.
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
