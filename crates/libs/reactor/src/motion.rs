//! The system **visual-effects preferences** — Settings → Accessibility →
//! Visual effects — read from `Windows.UI.ViewManagement.UISettings`:
//! [`reduced_motion`] (*Animation effects*), [`advanced_effects_enabled`]
//! (*Transparency effects*) and [`auto_hide_scroll_bars`] (*Always show
//! scrollbars*, inverted).
//!
//! Live here rather than in a backend because they are user-level preferences:
//! the same for every window, every backend, and for app code drawing its own
//! motion on a surface the reactor only hands out. The dcomp backend consumes
//! [`reduced_motion`] to gate its compositor animations; all three are public
//! so an app can gate its own drawing.
//!
//! Each is cached in an atomic and kept current by a different signal, because
//! the three do not share one — [`ui_settings`] records which, and what was
//! measured about each.
//!
//! ## What honouring the motion preference means
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

#[cfg(feature = "dcomp-backend")]
use crate::system_bindings::{IUISettings4, IUISettings5, UISettings};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "dcomp-backend")]
use windows_core::Interface as _;

/// The cached preferences. Seeded at host creation, then kept current by
/// whichever signal that preference actually has (see [`ui_settings`]); each
/// starts at the value its setting ships as, so a read taken before the first
/// refresh — or after one that failed — is the default experience.
static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);
static ADVANCED_EFFECTS: AtomicBool = AtomicBool::new(true);
static AUTO_HIDE_SCROLL_BARS: AtomicBool = AtomicBool::new(true);

/// Whether the user has asked the system to minimise animation.
///
/// Cheap enough to call per frame — a relaxed atomic load. See the module docs
/// for what honouring it should and should not mean.
#[must_use]
pub fn reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Relaxed)
}

/// Whether the user wants transparency effects drawn.
///
/// Settings → Accessibility → Visual effects → *Transparency effects*: the
/// switch that turns acrylic, mica and every other see-through material into a
/// flat one. Cheap enough to call per frame — a relaxed atomic load.
#[must_use]
pub fn advanced_effects_enabled() -> bool {
    ADVANCED_EFFECTS.load(Ordering::Relaxed)
}

/// Whether scroll bars should collapse to a thin indicator when idle.
///
/// Settings → Accessibility → Visual effects → *Always show scrollbars*,
/// inverted: `true` means the user is happy for a scroll bar to shrink out of
/// the way. Cheap enough to call per frame — a relaxed atomic load.
#[must_use]
pub fn auto_hide_scroll_bars() -> bool {
    AUTO_HIDE_SCROLL_BARS.load(Ordering::Relaxed)
}

/// Re-read the motion preference into the cache.
///
/// Returns `true` when the value **changed**, which is the caller's signal that
/// anything built against the old value is stale. A re-read finding the same
/// value must not trigger that work — `WM_SETTINGCHANGE` broadcasts for every
/// unrelated setting in the system.
/// Only the DComp backend consumes this: it drives motion from its own
/// compositor-side animation vocabulary and re-reads on `WM_SETTINGCHANGE`.
/// XAML applies the system preference to its own storyboards.
#[cfg(feature = "dcomp-backend")]
pub(crate) fn refresh_reduced_motion() -> bool {
    let now = read_reduced_motion();
    REDUCED_MOTION.swap(now, Ordering::Relaxed) != now
}

/// Seed the transparency cache and, as a side effect of the first read, install
/// the subscription that keeps it current from then on. Same change-only
/// contract as [`refresh_reduced_motion`].
///
/// Nothing else needs to call this: `AdvancedEffectsEnabledChanged` maintains
/// the cache, and it is the only signal that fires for this preference.
#[cfg(feature = "dcomp-backend")]
pub(crate) fn refresh_advanced_effects_enabled() -> bool {
    let now = read_ui_setting(|s| s.cast::<IUISettings4>()?.AdvancedEffectsEnabled(), true);
    ADVANCED_EFFECTS.swap(now, Ordering::Relaxed) != now
}

/// Re-read the scroll-bar preference into the cache. Same change-only contract
/// as [`refresh_reduced_motion`].
///
/// This preference has no signal of its own — see [`ui_settings`] — so this is
/// the only thing that ever updates the cache, and it is only as current as its
/// callers are frequent.
#[cfg(feature = "dcomp-backend")]
pub(crate) fn refresh_auto_hide_scroll_bars() -> bool {
    let now = read_ui_setting(|s| s.cast::<IUISettings5>()?.AutoHideScrollBars(), true);
    AUTO_HIDE_SCROLL_BARS.swap(now, Ordering::Relaxed) != now
}

/// Force the cached value — test seam only, so a test does not depend on the
/// developer machine's accessibility settings.
///
/// Reached through `dcomp_test_api` and this module's own tests, both of which
/// are DComp-side; the XAML backend applies the system preference through its
/// own storyboards and has nothing to force.
#[cfg(all(any(test, feature = "test"), feature = "dcomp-backend"))]
pub(crate) fn set_reduced_motion_for_test(reduced: bool) {
    REDUCED_MOTION.store(reduced, Ordering::Relaxed);
}

/// Reads `UISettings.AnimationsEnabled` and reports its inverse — the app asks
/// "is motion reduced", the system reports "is animation enabled".
///
/// **Fails open** (animations enabled). A failure to read a preference should
/// not silently disable motion across the whole app; the setting defaults to
/// enabled, so an unreadable one leaves the default experience intact.
#[cfg(feature = "dcomp-backend")]
fn read_reduced_motion() -> bool {
    !read_ui_setting(|s| s.AnimationsEnabled(), true)
}

/// The process-wide `UISettings`, activated on first use, plus the one change
/// subscription worth holding.
///
/// One instance for every read: the class is agile and its properties are plain
/// synchronous reads, so the activation is pure overhead to repeat — and
/// `WM_SETTINGCHANGE` arrives for every unrelated setting in the system.
///
/// Only a **success** is cached. A failed activation is retried by the next
/// caller, because caching it would be permanent: every later refresh would
/// short-circuit, and all three preferences would sit at their defaults for the
/// rest of the process while still looking like they were being read. Failing
/// open on a value is a fallback; failing open on the change tracking is a
/// silent stoppage, and the two must not share a fate.
///
/// ## Which signal announces which preference
///
/// The three properties are siblings on one object but do **not** share a change
/// signal, so each is driven by the one that was measured to fire for it:
///
/// - `AnimationsEnabled` is a `SystemParametersInfo` setting
///   (`SPI_SETCLIENTAREAANIMATION`), so changing it broadcasts
///   `WM_SETTINGCHANGE`, and the property already reads the new value by the
///   time that message is dispatched. Refreshed from the pump. Its
///   `AnimationsEnabledChanged` event does not fire in a desktop process.
/// - `AdvancedEffectsEnabled` gets no `WM_SETTINGCHANGE` — it is not an SPI
///   setting — but `AdvancedEffectsEnabledChanged` fires reliably, within
///   ~15ms, on a thread-pool thread. Hence the subscription below, whose
///   handler writes nothing but the atomic and so needs no marshal.
/// - `AutoHideScrollBars` has neither: no broadcast, and its event does not
///   fire either. Only re-reading the property observes a change, so its cache
///   is refreshed on `WM_SETTINGCHANGE` for whatever that catches and may
///   otherwise lag. Wiring it to a consumer that must react promptly means
///   giving it a real signal first (a registry watch on
///   `HKCU\Control Panel\Accessibility\DynamicScrollbars`).
#[cfg(feature = "dcomp-backend")]
fn ui_settings() -> Option<&'static UISettings> {
    static SETTINGS: std::sync::OnceLock<UISettings> = std::sync::OnceLock::new();
    get_or_try_init(
        &SETTINGS,
        || UISettings::new().ok(),
        subscribe_advanced_effects,
    )
}

/// Fill `cell` from `make` on **success only**, running `on_install` exactly
/// once for the value that ends up stored.
///
/// `OnceLock::get_or_init` cannot express this: its closure has to yield a
/// value, so a failure can only be stored *as* one — and a stored "no value" is
/// indistinguishable from a stored success, which makes the first failed
/// attempt the last attempt the process ever makes. Here a failure stores
/// nothing, so the next caller tries again.
#[cfg(feature = "dcomp-backend")]
fn get_or_try_init<T>(
    cell: &'static std::sync::OnceLock<T>,
    make: impl FnOnce() -> Option<T>,
    on_install: impl FnOnce(&'static T),
) -> Option<&'static T> {
    if let Some(v) = cell.get() {
        return Some(v);
    }
    // Losing the race drops this value without running `on_install`, so two
    // threads racing here still leave exactly one instance and one side effect.
    if cell.set(make()?).is_ok() {
        on_install(cell.get()?);
    }
    cell.get()
}

/// Whether [`subscribe_advanced_effects`] attached — the difference between a
/// cache that tracks the setting and one that is a snapshot.
#[cfg(feature = "dcomp-backend")]
static ADVANCED_EFFECTS_SUBSCRIBED: AtomicBool = AtomicBool::new(false);

/// Hook `AdvancedEffectsEnabledChanged` so the transparency cache tracks the
/// setting. The handler writes one atomic and touches nothing else, so it needs
/// no marshal back to the UI thread and holds no state a late callback could
/// find torn down.
///
/// The subscription is **permanent** — the cache must track the setting for as
/// long as the process draws, and dropping the revoker is what unsubscribes, so
/// it is deliberately leaked rather than held in a static (`EventRevoker` is
/// neither `Send` nor `Sync`). One allocation, once per process.
///
/// A failed subscribe is recorded and swallowed: it costs the live tracking of
/// this one preference and must not take down the reads that do not depend on
/// it.
#[cfg(feature = "dcomp-backend")]
fn subscribe_advanced_effects(ui: &UISettings) {
    let hooked = ui.cast::<IUISettings4>().and_then(|s| {
        s.AdvancedEffectsEnabledChanged(|sender, _| {
            if let Ok(s) = sender.ok()
                && let Ok(v) = s
                    .cast::<IUISettings4>()
                    .and_then(|s| s.AdvancedEffectsEnabled())
            {
                ADVANCED_EFFECTS.store(v, Ordering::Relaxed);
            }
        })
    });
    if let Ok(revoker) = hooked {
        std::mem::forget(revoker);
        ADVANCED_EFFECTS_SUBSCRIBED.store(true, Ordering::Relaxed);
    }
}

/// Read one boolean property off the shared [`ui_settings`], falling back to
/// `on_failure` when the instance or the property is unavailable. Every caller
/// passes the value the setting ships as, so a failed read is indistinguishable
/// from a machine left at its defaults.
#[cfg(feature = "dcomp-backend")]
fn read_ui_setting(
    read: impl FnOnce(&UISettings) -> windows_core::Result<bool>,
    on_failure: bool,
) -> bool {
    ui_settings()
        .and_then(|s| read(s).ok())
        .unwrap_or(on_failure)
}

// Every test here exercises the DComp-side preference caches; see
// `set_reduced_motion_for_test`.
#[cfg(all(test, feature = "dcomp-backend"))]
mod tests {
    use super::*;

    /// The caches are process-global, so these cannot run at once — one would
    /// overwrite a value another is mid-assertion about.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A re-read that finds no change must say so — this is what keeps a
    /// `WM_SETTINGCHANGE` storm from rebuilding every node's animations for a
    /// setting that has nothing to do with motion.
    // Exercises the WinRT re-read, which only the DComp backend compiles.
    #[cfg(feature = "dcomp-backend")]
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

    /// The other two preferences carry the same contract, and — unlike the
    /// motion one — nothing else in the tree would notice if their reads
    /// silently fell back. So this asserts the instance activated and both
    /// properties answered, not merely that a bool came back: a `false` from a
    /// failed QueryInterface reads exactly like a real one.
    ///
    /// It also proves the transparency subscription installed, which is the
    /// only thing that keeps that cache current: a revoked or never-attached
    /// handler leaves a value that is correct exactly once and then rots.
    #[cfg(feature = "dcomp-backend")]
    #[test]
    fn a_the_other_preferences_are_readable_and_report_change_only() {
        let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let settings = ui_settings().expect("UISettings failed to activate");
        assert!(
            ADVANCED_EFFECTS_SUBSCRIBED.load(Ordering::Relaxed),
            "AdvancedEffectsEnabledChanged did not attach — the transparency \
             cache would never update again"
        );
        settings
            .cast::<IUISettings4>()
            .and_then(|s| s.AdvancedEffectsEnabled())
            .expect("AdvancedEffectsEnabled is unreadable");
        settings
            .cast::<IUISettings5>()
            .and_then(|s| s.AutoHideScrollBars())
            .expect("AutoHideScrollBars is unreadable");

        refresh_advanced_effects_enabled();
        assert!(
            !refresh_advanced_effects_enabled(),
            "an unchanged transparency preference reported a change"
        );
        refresh_auto_hide_scroll_bars();
        assert!(
            !refresh_auto_hide_scroll_bars(),
            "an unchanged scroll-bar preference reported a change"
        );
    }

    /// A failed activation must not be remembered. If it were, the first call —
    /// which happens during host construction, before anything has drawn — could
    /// disable all three preferences for the rest of the process: every later
    /// refresh would short-circuit on the cached failure, and a user toggling a
    /// setting mid-session would see nothing change.
    ///
    /// Needs no `SERIAL` guard: it drives its own cell, not the shared caches.
    #[cfg(feature = "dcomp-backend")]
    #[test]
    fn a_failed_first_attempt_is_retried_not_remembered() {
        static CELL: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        static INSTALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let count = |_: &'static u32| {
            INSTALLS.fetch_add(1, Ordering::Relaxed);
        };

        assert_eq!(get_or_try_init(&CELL, || None, count), None);
        assert_eq!(
            INSTALLS.load(Ordering::Relaxed),
            0,
            "nothing was installed, so the install side effect must not have run"
        );

        assert_eq!(
            get_or_try_init(&CELL, || Some(7), count),
            Some(&7),
            "the earlier failure was remembered — activation would never be \
             retried and the preferences would be frozen at their defaults"
        );
        assert_eq!(INSTALLS.load(Ordering::Relaxed), 1);

        // Every later caller sees the stored value and does not re-install:
        // the subscription this guards must be hooked exactly once.
        assert_eq!(get_or_try_init(&CELL, || Some(9), count), Some(&7));
        assert_eq!(INSTALLS.load(Ordering::Relaxed), 1);
    }

    // The seam it exercises is DComp-side; see `set_reduced_motion_for_test`.
    #[cfg(feature = "dcomp-backend")]
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
