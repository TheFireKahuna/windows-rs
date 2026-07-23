//! Display colour-capability → app callback, read and delivered from **one WinRT
//! object**.
//!
//! The dcomp host hands the app only a render fn, so — mirroring
//! [`crate::set_window_visibility_callback`] and [`crate::set_output_color_transform`]
//! — the app registers a **process-global** callback BEFORE the window is created,
//! and the host invokes it with the display's [`AdvancedColor`] snapshot whenever
//! that capability may have changed.
//!
//! The one object is `Windows.Graphics.Display.DisplayInformation`, obtained for the
//! window through the desktop interop factory (there is no UWP `CoreWindow` to use
//! `GetForCurrentView`). It is the single owner of everything colour here:
//!
//! - it raises `AdvancedColorInfoChanged` on an HDR toggle, an SDR-white-level
//!   change, and a monitor hop (it tracks whichever display the window is on), and
//! - `GetAdvancedColorInfo` reads the capability itself — kind (SDR/WCG/HDR),
//!   panel primaries, and luminances in nits.
//!
//! Both come off the same object, so there is exactly one `DisplayInformation` and
//! one read path in the whole app: the host reads the snapshot and hands the app
//! plain data ([`AdvancedColor`] carries no `windows` type), and the app maps it to
//! its colour policy. This replaces the old `WM_DISPLAYCHANGE` / `WM_SETTINGCHANGE`
//! / `WM_WINDOWPOSCHANGED` monitor-diff plumbing *and* the app's separate DXGI read.
//!
//! The callback runs on the UI thread (the DispatcherQueue the event is delivered
//! on); keep it cheap and non-blocking.

use std::cell::RefCell;
use std::sync::OnceLock;

use windows_core::Interface;

use crate::system_bindings::{DisplayInformation, HWND, IDisplayInformation5, IDisplayInformationStaticsInterop, Point};

/// A plain-data snapshot of a display's advanced-colour capability, read verbatim
/// from `AdvancedColorInfo`. Carries no `windows` type, so the app-facing seam stays
/// projection-free; the app maps it to its own colour policy.
///
/// The primaries (`*_primary` / `white_point`, CIE xy chromaticities) and `kind`
/// describe the panel's gamut and mode for a colour pipeline that gamut-maps; a
/// pipeline that only tracks white level and headroom needs just `kind`,
/// `max_luminance_nits`, and `sdr_white_level_nits`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AdvancedColor {
    /// `AdvancedColorKind`: `0` = SDR, `1` = WCG, `2` = HDR.
    pub kind: i32,
    /// Red primary chromaticity `(x, y)`.
    pub red_primary: (f32, f32),
    /// Green primary chromaticity `(x, y)`.
    pub green_primary: (f32, f32),
    /// Blue primary chromaticity `(x, y)`.
    pub blue_primary: (f32, f32),
    /// White point chromaticity `(x, y)`.
    pub white_point: (f32, f32),
    /// Peak luminance (usually of a small window), in nits.
    pub max_luminance_nits: f32,
    /// Minimum luminance, in nits.
    pub min_luminance_nits: f32,
    /// Maximum full-frame (whole-display) luminance, in nits.
    pub max_full_frame_luminance_nits: f32,
    /// The OS SDR white level for this display, in nits (the HDR slider value; the
    /// nominal 80 on a display-referred SDR/WCG desktop).
    pub sdr_white_level_nits: f32,
}

type DisplayChangeCallback = Box<dyn Fn(AdvancedColor) + Send + Sync + 'static>;

/// The app-registered callback. Set once before the window exists; a later
/// registration is ignored (matches the visibility / white-level setters).
static DISPLAY_CB: OnceLock<DisplayChangeCallback> = OnceLock::new();

thread_local! {
    /// The window's `DisplayInformation` and its `AdvancedColorInfoChanged`
    /// subscription, held for the window's lifetime. `GetForWindow` hooks the
    /// window's message loop, so [`detach`] drops this on `WM_DESTROY` while the
    /// `HWND` is still valid.
    static SUBSCRIPTION: RefCell<Option<(DisplayInformation, windows_core::EventRevoker)>> =
        const { RefCell::new(None) };
}

/// Register a process-global display-capability callback: invoked with the current
/// [`AdvancedColor`] snapshot whenever the display's colour capability may have
/// changed. Call **before** the window is created (like
/// [`crate::set_window_visibility_callback`]); only the first registration is kept.
///
/// The callback runs on the UI thread — keep it cheap and non-blocking.
pub fn set_display_change_callback(cb: impl Fn(AdvancedColor) + Send + Sync + 'static) {
    let _ = DISPLAY_CB.set(Box::new(cb));
}

/// Read the capability off a `DisplayInformation`. `GetAdvancedColorInfo` (like the
/// event) lives on the versioned `IDisplayInformation5`, reached through a cast.
fn read(info: &DisplayInformation) -> windows_core::Result<AdvancedColor> {
    let aci = info.cast::<IDisplayInformation5>()?.GetAdvancedColorInfo()?;
    let xy = |p: Point| (p.x, p.y);
    Ok(AdvancedColor {
        kind: aci.CurrentAdvancedColorKind()?.0,
        red_primary: xy(aci.RedPrimary()?),
        green_primary: xy(aci.GreenPrimary()?),
        blue_primary: xy(aci.BluePrimary()?),
        white_point: xy(aci.WhitePoint()?),
        max_luminance_nits: aci.MaxLuminanceInNits()?,
        min_luminance_nits: aci.MinLuminanceInNits()?,
        max_full_frame_luminance_nits: aci.MaxAverageFullFrameLuminanceInNits()?,
        sdr_white_level_nits: aci.SdrWhiteLevelInNits()?,
    })
}

/// Read the current capability, fire the app callback, then repaint all chrome so
/// already-painted static surfaces pick up the new draw-time map (viz surfaces
/// repaint every frame, so only static chrome needs the nudge). Runs on the UI
/// thread. A failed read is logged and swallowed, leaving the previous fit in place.
///
/// The repaint self-gates on the host being up, so the initial call from [`attach`]
/// (before the first paint) simply installs the fit with no repaint.
fn emit(info: &DisplayInformation) {
    match read(info) {
        Ok(caps) => {
            if let Some(cb) = DISPLAY_CB.get() {
                cb(caps);
            }
        }
        Err(e) => {
            eprintln!("dcomp: GetAdvancedColorInfo failed ({e:?}); keeping previous colour fit");
            return;
        }
    }
    if let Some(s) = super::host::shared() {
        s.backend.borrow_mut().mark_all_dirty_and_repaint();
    }
}

/// Subscribe the window's display-capability signal and fire the **initial** fit.
///
/// Called once from window creation, after the `HWND` exists and the UI thread has
/// its `DispatcherQueue` — both requirements of `GetForWindow`. The initial fire is
/// synchronous and lands before the first paint, so frame one is already mapped to
/// the display; there is no separate pre-window fit to keep in sync.
///
/// A failed subscribe is logged and swallowed: the app keeps the identity map
/// (present the absolute palette — correct on any headroom display, only slightly
/// dim on a display-referred one until a fit lands).
pub(crate) fn attach(hwnd: HWND) {
    match subscribe(hwnd) {
        Ok((info, revoker)) => {
            emit(&info);
            SUBSCRIPTION.with(|s| *s.borrow_mut() = Some((info, revoker)));
        }
        Err(e) => eprintln!("dcomp: AdvancedColorInfo subscribe failed ({e:?}); colour fit will not track display changes"),
    }
}

/// Build the `DisplayInformation` for `hwnd` and hook its `AdvancedColorInfoChanged`.
/// The handler reads the fresh capability off the event's own sender.
fn subscribe(hwnd: HWND) -> windows_core::Result<(DisplayInformation, windows_core::EventRevoker)> {
    let interop = windows_core::factory::<DisplayInformation, IDisplayInformationStaticsInterop>()?;
    let info: DisplayInformation = unsafe { interop.GetForWindow(hwnd) }?;
    let info5 = info.cast::<IDisplayInformation5>()?;
    let revoker = info5.AdvancedColorInfoChanged(move |sender, _args| {
        if let Ok(info) = sender.ok() {
            emit(info);
        }
    })?;
    Ok((info, revoker))
}

/// Drop the subscription and the `DisplayInformation`, unhooking the window's
/// message loop. Called from `WM_DESTROY` while the `HWND` is still valid.
pub(crate) fn detach() {
    SUBSCRIPTION.with(|s| *s.borrow_mut() = None);
}
