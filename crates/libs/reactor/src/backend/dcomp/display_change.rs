//! Display-capability → app callback (for re-fitting a colour pipeline to the
//! current display).
//!
//! The dcomp host hands the app only a render fn, so — mirroring
//! [`crate::set_window_visibility_callback`] and [`crate::set_output_color_transform`]
//! — the app registers a **process-global** callback BEFORE the window is created
//! and the host invokes it from its `wnd_proc` whenever the display's colour
//! capability may have changed:
//!
//! - `WM_DISPLAYCHANGE` — resolution / topology / HDR-mode change,
//! - `WM_SETTINGCHANGE` — the OS "SDR content brightness" slider and auto-colour /
//!   theme toggles,
//! - a monitor hop (the window dragged onto a different display), guarded by an
//!   internal monitor diff so an ordinary move does not fire it.
//!
//! These are exactly the edges on which `activeColorMode`, the SDR white level, or
//! the panel's primaries can change — i.e. everything a display-referred colour
//! pipeline must re-fit against. The fork stays policy-free: it knows nothing about
//! colour modes or gamuts, it only reports "this display may be different now" and
//! hands back the window handle so the app can re-query.
//!
//! The callback runs on the UI thread inside the window procedure; keep it cheap and
//! non-blocking (post to another thread for real work). The window handle is passed
//! as a raw `isize` so this seam carries no `windows` type in its public signature.

use std::cell::Cell;
use std::sync::OnceLock;

use crate::system_bindings::{HWND, MonitorFromWindow, MONITOR_DEFAULTTONEAREST};

type DisplayChangeCallback = Box<dyn Fn(isize) + Send + Sync + 'static>;

/// The app-registered callback. Set once before the window exists; a later
/// registration is ignored (matches the visibility / white-level setters).
static DISPLAY_CB: OnceLock<DisplayChangeCallback> = OnceLock::new();

thread_local! {
    /// The `HMONITOR` the window was last seen on (`0` = never), so the
    /// `WM_WINDOWPOSCHANGED` path only fires the callback on a genuine monitor hop.
    /// Independent of the white-level module's own monitor cache — two small,
    /// decoupled guards rather than one shared piece of cross-module state.
    static LAST_MONITOR: Cell<isize> = const { Cell::new(0) };
}

/// Register a process-global display-capability callback: invoked with the window's
/// raw handle (`HWND` as `isize`) whenever the display it is on may have changed
/// colour capability. Call **before** the window is created (like
/// [`crate::set_window_visibility_callback`]); only the first registration is kept.
///
/// The callback runs on the UI thread inside the window procedure — keep it cheap and
/// non-blocking. It may fire when nothing colour-relevant actually changed (the app's
/// re-query is the source of truth); it is an invalidation hint, not an event.
pub fn set_display_change_callback(cb: impl Fn(isize) + Send + Sync + 'static) {
    let _ = DISPLAY_CB.set(Box::new(cb));
}

/// Fire the callback unconditionally (the `WM_DISPLAYCHANGE` / `WM_SETTINGCHANGE`
/// edges, where a capability change cannot be cheaply ruled out). No-op if no
/// callback was registered.
pub(crate) fn note_display_change(hwnd: HWND) {
    if let Some(cb) = DISPLAY_CB.get() {
        cb(hwnd as isize);
    }
}

/// Fire the callback only when the window's nearest monitor actually changed — the
/// `WM_WINDOWPOSCHANGED` path, kept off the hot move/resize case. Seeds the cache on
/// the first call (window creation) so the initial fit lands on the right display.
pub(crate) fn note_possible_monitor_change(hwnd: HWND) {
    if DISPLAY_CB.get().is_none() {
        return;
    }
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) } as isize;
    if monitor != 0 && LAST_MONITOR.with(|c| c.replace(monitor)) != monitor {
        note_display_change(hwnd);
    }
}
