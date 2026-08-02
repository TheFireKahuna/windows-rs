use crate::bindings::*;

/// The DPI-derived quantities a custom frame needs, in **physical pixels**.
///
/// Nothing here is stored anywhere. A window's DPI changes on a monitor hop, on a scale change
/// in Settings and on an undock, and the whole set — one `GetDpiForWindow`, four
/// `GetSystemMetricsForDpi` and one `DwmGetWindowAttribute` — measures ~59 ns, which is
/// affordable at the `WM_NCHITTEST` rate the caption asks for it. Re-measure before caching it.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Metrics {
    /// Dots per inch. 96 is unity.
    pub dpi: u32,
    /// `dpi / 96`, the DIP-to-physical factor.
    pub scale: f32,
    /// Resize-border thickness on the left and right edges: the sizing frame **plus** the
    /// padded border. `SM_CXFRAME` alone is the visible frame, which is narrower than the
    /// band the user can actually grab.
    pub frame_x: i32,
    /// Resize-border thickness on the bottom edge.
    pub frame_y: i32,
    /// Resize-border thickness on the **top** edge, which is narrower than the other three.
    ///
    /// The window rect extends past the visible frame on the left, right and bottom, so a
    /// band there is mostly outside the window. The top has no such margin — the band comes
    /// out of the caption the application drew — and the system takes the border's width
    /// back off it. Measured against `DefWindowProc` at 144 DPI: 11 px of frame less a 2 px
    /// border is its own 9, exactly.
    pub frame_top: i32,
    /// The system's own caption height, for a caption that stated none.
    pub caption: i32,
}

impl Metrics {
    pub(crate) fn for_window(hwnd: HWND) -> Self {
        // A window being created, or one whose monitor went away, can report zero. Unity
        // keeps every derived metric positive, which the hit test depends on.
        // SAFETY: `hwnd` is the window this is computed for, live for the call.
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        // SAFETY: each index is a documented `SM_*` constant.
        let padded = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
        // SAFETY: as above.
        let frame_y = unsafe { GetSystemMetricsForDpi(SM_CYFRAME, dpi) } + padded;
        Self {
            dpi,
            scale: dpi as f32 / 96.0,
            // SAFETY: as above.
            frame_x: unsafe { GetSystemMetricsForDpi(SM_CXFRAME, dpi) } + padded,
            frame_y,
            // A window that will not report its border keeps the whole frame width, which
            // is the wider band and so cannot make an edge unreachable.
            frame_top: (frame_y - visible_border(hwnd)).max(1),
            // SAFETY: as above.
            caption: unsafe { GetSystemMetricsForDpi(SM_CYCAPTION, dpi) } + padded,
        }
    }

    /// A length in DIPs as physical pixels, rounded to the nearest — a band one pixel short
    /// of what the renderer snapped to is visible.
    #[must_use]
    pub fn px(self, dips: f32) -> i32 {
        (dips * self.scale).round() as i32
    }

    /// A length in physical pixels as DIPs.
    #[must_use]
    pub fn dips(self, px: i32) -> f32 {
        px as f32 / self.scale
    }
}

/// The width of the border DWM draws around `hwnd`, or zero if it will not say.
fn visible_border(hwnd: HWND) -> i32 {
    let mut thickness = 0u32;
    // SAFETY: `hwnd` is live; the destination is a stack local of the stated size.
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_VISIBLE_FRAME_BORDER_THICKNESS as u32,
            (&raw mut thickness).cast(),
            size_of::<u32>() as u32,
        )
    };
    if hr.is_ok() { thickness as i32 } else { 0 }
}
