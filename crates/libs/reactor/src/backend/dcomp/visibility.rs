//! Window-visibility → app callback (for demand-gating expensive off-screen work).
//!
//! The dcomp host does not hand the app a per-window event object — the app only
//! gives [`crate::DCompHost::render`] a render fn — so, mirroring
//! [`crate::set_hdr_reference_white_nits`], the app registers a **process-global**
//! callback BEFORE the window is created and the host invokes it from its `wnd_proc`
//! whenever the window's visibility changes (minimize ↔ restore). The app uses it to
//! pause expensive off-screen work — e.g. a live analyzer's IPC demand, so a hidden
//! window with audio playing stops driving the 60 Hz compute+redraw chain (the
//! modern-standby power win) — and to re-arm it on restore.
//!
//! The callback runs on the UI thread inside the window procedure; it must stay cheap
//! and non-blocking (post to another thread for real work). It is de-duplicated
//! against the last delivered state, so only a real edge fires it.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

type VisibilityCallback = Box<dyn Fn(bool) + Send + Sync + 'static>;

/// The app-registered callback. Set once before the window exists; a later
/// registration is ignored (matches the white-level setter's before-window contract).
static VISIBILITY_CB: OnceLock<VisibilityCallback> = OnceLock::new();

/// Last visibility delivered to the callback, so a repeated same-state notification
/// (e.g. a resize while already visible) does not re-fire it. `0` = unknown (nothing
/// delivered yet), `1` = visible, `2` = hidden.
static LAST: AtomicU8 = AtomicU8::new(0);

/// Register a process-global window-visibility callback: invoked with `true` when the
/// window becomes visible (restored / shown) and `false` when it becomes hidden
/// (minimized). Call **before** the window is created (like
/// [`crate::set_hdr_reference_white_nits`]); only the first registration is kept.
///
/// The callback runs on the UI thread inside the window procedure — keep it cheap and
/// non-blocking (post to another thread for real work).
pub fn set_window_visibility_callback(cb: impl Fn(bool) + Send + Sync + 'static) {
    let _ = VISIBILITY_CB.set(Box::new(cb));
}

/// Called by the host's `wnd_proc` on a visibility transition. De-duplicates against
/// the last delivered state so the callback fires only on a real edge (and never when
/// no callback was registered).
pub(crate) fn note_visibility(visible: bool) {
    let want = if visible { 1 } else { 2 };
    if LAST.swap(want, Ordering::Relaxed) != want
        && let Some(cb) = VISIBILITY_CB.get()
    {
        cb(visible);
    }
}
