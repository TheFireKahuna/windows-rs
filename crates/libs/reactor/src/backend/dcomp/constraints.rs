//! Window size constraints → `WM_GETMINMAXINFO`.
//!
//! The dcomp host takes only a title and a render fn — there is no per-window
//! options object to hang constraints off — so, mirroring
//! [`crate::set_window_visibility_callback`], the app registers process-global
//! [`InnerConstraints`] BEFORE the window is created and the host applies them
//! from its `wnd_proc` on every `WM_GETMINMAXINFO`. This is the dcomp half of
//! the WinUI backend's `App::inner_constraints`, which reaches the same
//! behaviour through `IOverlappedPresenter3::SetPreferredMinimumWidth`.
//!
//! Constraints are **client** (inner) sizes in DIPs, matching
//! [`crate::DCompHost::new_sized`]. `WM_GETMINMAXINFO` speaks physical-pixel
//! **window** (frame) sizes, so each value is DPI-scaled and the non-client
//! delta is added. That delta is measured live as `GetWindowRect -
//! GetClientRect` rather than computed with `AdjustWindowRectExForDpi`: the
//! host's `WM_NCCALCSIZE` gives the window an extended frame (the caption band
//! is client area), so the standard frame metrics would over-report the delta
//! by a whole caption height and the enforced minimum would sit that much too
//! large.
//!
//! Note that a maximum constrains the **maximized** size too — `ptMaxTrackSize`
//! caps every sizing path, not just border drags. That matches the WinUI
//! backend's `PreferredMaximumWidth`/`Height`, which Windows documents the same
//! way.

use std::sync::OnceLock;

use crate::style::InnerConstraints;
use crate::system_bindings::*;

/// The app-registered constraints. Set once before the window exists; a later
/// registration is ignored (matches the visibility callback's before-window
/// contract).
static CONSTRAINTS: OnceLock<InnerConstraints> = OnceLock::new();

/// Register process-global window size constraints, as **client** sizes in
/// DIPs. Call **before** the window is created (like
/// [`crate::set_window_visibility_callback`]); only the first registration is
/// kept.
///
/// A minimum stops the user dragging the frame below it and clamps programmatic
/// sizing; a maximum also caps the maximized size, so prefer leaving the maxima
/// unset unless the window genuinely must not grow.
///
/// ```ignore
/// windows_reactor::set_inner_constraints(InnerConstraints {
///     min_width: Some(960.0),
///     min_height: Some(640.0),
///     ..Default::default()
/// });
/// ```
pub fn set_inner_constraints(constraints: InnerConstraints) {
    let _ = CONSTRAINTS.set(constraints);
}

/// Apply the registered constraints to a `WM_GETMINMAXINFO` payload, whose
/// members the system has already pre-filled with its own defaults — only the
/// axes the app actually constrained are overwritten.
///
/// Returns `false` when nothing was registered, so the caller can leave the
/// message to `DefWindowProc` entirely.
pub(crate) fn apply(hwnd: HWND, scale: f32, mmi: &mut MINMAXINFO) -> bool {
    let Some(c) = CONSTRAINTS.get() else {
        return false;
    };

    let (nc_w, nc_h) = nonclient_delta(hwnd);
    let to_frame_px = |client_dip: f64, nc: i32| {
        ((client_dip * scale as f64).round() as i32).saturating_add(nc)
    };

    if let Some(w) = c.min_width {
        mmi.ptMinTrackSize.x = to_frame_px(w, nc_w);
    }
    if let Some(h) = c.min_height {
        mmi.ptMinTrackSize.y = to_frame_px(h, nc_h);
    }
    if let Some(w) = c.max_width {
        mmi.ptMaxTrackSize.x = to_frame_px(w, nc_w);
    }
    if let Some(h) = c.max_height {
        mmi.ptMaxTrackSize.y = to_frame_px(h, nc_h);
    }

    // A maximum below the minimum would leave the frame draggable below the
    // minimum — Windows tracks against whichever bound it hits first. The
    // minimum wins.
    mmi.ptMaxTrackSize.x = mmi.ptMaxTrackSize.x.max(mmi.ptMinTrackSize.x);
    mmi.ptMaxTrackSize.y = mmi.ptMaxTrackSize.y.max(mmi.ptMinTrackSize.y);

    true
}

/// Frame-minus-client size in physical pixels, measured live so it reflects the
/// host's extended (caption-less) frame rather than the standard metrics.
///
/// `(0, 0)` while the window has no usable client rect — during creation, and
/// while minimized, where the frame rect is the off-screen minimized placement
/// and a difference against it would be meaningless.
fn nonclient_delta(hwnd: HWND) -> (i32, i32) {
    let mut frame = RECT::default();
    let mut client = RECT::default();
    unsafe {
        if !GetWindowRect(hwnd, &mut frame).as_bool() || !GetClientRect(hwnd, &mut client).as_bool()
        {
            return (0, 0);
        }
    }
    let (cw, ch) = (client.right - client.left, client.bottom - client.top);
    if cw <= 0 || ch <= 0 {
        return (0, 0);
    }
    (
        ((frame.right - frame.left) - cw).max(0),
        ((frame.bottom - frame.top) - ch).max(0),
    )
}
