//! The system **motion preference** — Settings → Accessibility → Visual
//! effects → *Animation effects*, which is what `SPI_GETCLIENTAREAANIMATION`
//! reports.
//!
//! Lives here rather than in a backend because it is a user-level preference:
//! the same for every window, every backend, and for app code drawing its own
//! motion on a surface the reactor only hands out. The dcomp backend consumes
//! it to gate its compositor animations; [`reduced_motion`] is public so an app
//! can gate its own.
//!
//! ## What honouring it means
//!
//! Not "skip the animation" — most UI animation does not decorate a state the
//! element is already in, it *establishes* it. An enter transition runs opacity
//! 0 → 1, so skipping leaves the element invisible. The rule is **change the
//! path, never the destination**: jump to the end state instead of travelling
//! to it.
//!
//! ## What it does not mean
//!
//! It is a preference about *interface* motion — fades, slides, springs,
//! decorative transitions. It is not an instruction to freeze moving content.
//! An app showing live data (a meter, an analyzer, a video) should keep showing
//! it: the motion there is the content, not the chrome, and smoothing that is
//! sampled from real data is usually *less* jarring than the staircase left
//! without it. Gate the chrome; leave the content alone.

use std::sync::atomic::{AtomicBool, Ordering};

/// The cached preference. Refreshed at host creation and on `WM_SETTINGCHANGE`.
static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);

/// Whether the user has asked the system to minimise animation.
///
/// Cheap enough to call per frame — a relaxed atomic load. See the module docs
/// for what honouring it should and should not mean.
#[must_use]
pub fn reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Relaxed)
}

/// Re-read the preference into the cache.
///
/// Returns `true` when the value **changed**, which is the caller's signal that
/// anything built against the old value is stale. A re-read finding the same
/// value must not trigger that work — `WM_SETTINGCHANGE` broadcasts for every
/// unrelated setting in the system.
pub(crate) fn refresh_reduced_motion() -> bool {
    let now = read_reduced_motion();
    REDUCED_MOTION.swap(now, Ordering::Relaxed) != now
}

/// Force the cached value — test seam only, so a test does not depend on the
/// developer machine's accessibility settings.
pub(crate) fn set_reduced_motion_for_test(reduced: bool) {
    REDUCED_MOTION.store(reduced, Ordering::Relaxed);
}

/// Ask the OS whether client-area animation is enabled.
///
/// Uses the Win32 read rather than WinRT `UISettings.AnimationsEnabled`, which
/// is documented to surface this same setting: the value is identical, the
/// change signal (`WM_SETTINGCHANGE`) is already handled on the pump, and the
/// Win32 read is synchronous on the thread that needs it. `UISettings` would
/// add a WinRT activation and deliver its change event on a thread-pool thread,
/// requiring a marshal back for a value we can simply read here.
///
/// **Fails open** (animations enabled). A transient failure to read a
/// preference should not silently disable motion across the whole app; the
/// setting defaults to enabled, and this call does not realistically fail.
fn read_reduced_motion() -> bool {
    let mut enabled = windows_core::BOOL(1);
    let ok = unsafe {
        crate::system_bindings::SystemParametersInfoW(
            crate::system_bindings::SPI_GETCLIENTAREAANIMATION,
            0,
            (&raw mut enabled).cast(),
            0,
        )
    };
    ok.as_bool() && !enabled.as_bool()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache is process-global, so these two cannot run at once — one would
    /// overwrite the value the other is mid-assertion about.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A re-read that finds no change must say so — this is what keeps a
    /// `WM_SETTINGCHANGE` storm from rebuilding every node's animations for a
    /// setting that has nothing to do with motion.
    #[test]
    fn a_refresh_reports_change_only() {
        let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        // Whatever the machine's real setting is, reading it twice in a row
        // cannot change it.
        refresh_reduced_motion();
        assert!(
            !refresh_reduced_motion(),
            "an unchanged preference reported a change — every unrelated \
             WM_SETTINGCHANGE would now rebuild the whole arena"
        );
    }

    #[test]
    fn the_test_seam_is_observable_in_both_directions() {
        let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let restore = reduced_motion();
        set_reduced_motion_for_test(true);
        assert!(reduced_motion());
        set_reduced_motion_for_test(false);
        assert!(!reduced_motion());
        set_reduced_motion_for_test(restore);
    }
}
