//! A title bar the application draws that still behaves like the system's: drag,
//! double-click maximize, Aero shake, `Win`+arrow, the eight resize edges, the window menu,
//! rounded corners and the frame colour.
//!
//! Contacts over the three button regions are consumed, on a primary press only. Every
//! other non-client contact forwards to `DefWindowProc`, which is what keeps those
//! behaviours the system's.

use crate::bindings::*;
use crate::dpi::Metrics;
use core::cell::{Cell, RefCell};
use windows_color::Scrgb;

/// How the application wants its own title bar framed.
#[derive(Copy, Clone, Debug)]
pub struct CaptionSpec {
    /// Bar height in DIPs, or `None` for the system's own at the window's DPI.
    pub height: Option<f32>,
    pub corners: CornerPreference,
    /// Which of the three window commands this application draws.
    pub buttons: CaptionButtons,
}

impl Default for CaptionSpec {
    fn default() -> Self {
        Self {
            height: None,
            corners: CornerPreference::Round,
            buttons: CaptionButtons::ALL,
        }
    }
}

/// What DWM rounds the window's corners to.
///
/// The attribute rounds the **frame**, not composited content, so a matching rounded clip on
/// the root visual is the application's half — without it a square backdrop paints into the
/// corners of a round window.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CornerPreference {
    /// The system's full radius — what an ordinary Windows 11 window has.
    Round,
    /// The smaller radius the system uses for transient surfaces.
    RoundSmall,
    /// Square, as on a maximized window.
    Square,
}

impl CornerPreference {
    /// The radius DWM actually draws, in DIPs, and what the root visual's clip is built from.
    ///
    /// Observed: the attribute takes a preference, not a length.
    #[must_use]
    pub const fn radius_dips(self) -> f32 {
        match self {
            Self::Round => 8.0,
            Self::RoundSmall => 4.0,
            Self::Square => 0.0,
        }
    }

    const fn attribute(self) -> DWM_WINDOW_CORNER_PREFERENCE {
        match self {
            Self::Round => DWMWCP_ROUND,
            Self::RoundSmall => DWMWCP_ROUNDSMALL,
            Self::Square => DWMWCP_DONOTROUND,
        }
    }
}

/// Which window commands the application draws in its bar.
///
/// A veto, enforced at the hit test *and* at every non-client pointer arm. A window that
/// draws no maximize button must not answer `HTMAXBUTTON`: what the system opens on that
/// answer offers to maximize a window with no way back.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CaptionButtons {
    pub minimize: bool,
    pub maximize: bool,
    pub close: bool,
}

impl CaptionButtons {
    /// All three, which is what an ordinary top-level window has.
    pub const ALL: Self = Self {
        minimize: true,
        maximize: true,
        close: true,
    };

    const fn has(self, button: CaptionButton) -> bool {
        match button {
            CaptionButton::Minimize => self.minimize,
            CaptionButton::Maximize => self.maximize,
            CaptionButton::Close => self.close,
        }
    }
}

/// One of the three window commands.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CaptionButton {
    Minimize,
    /// Maximize, or restore when the window already is. Decided at the press, from the
    /// window's own state.
    Maximize,
    Close,
}

impl CaptionButton {
    const fn hit_code(self) -> i32 {
        match self {
            Self::Minimize => HTMINBUTTON,
            Self::Maximize => HTMAXBUTTON,
            Self::Close => HTCLOSE,
        }
    }

    const fn from_hit_code(code: i32) -> Option<Self> {
        match code {
            HTMINBUTTON => Some(Self::Minimize),
            HTMAXBUTTON => Some(Self::Maximize),
            HTCLOSE => Some(Self::Close),
            _ => None,
        }
    }
}

/// What the application's hit authority found at a point in the caption band.
///
/// The argument is a **client-space point in DIPs** — the space the layout solved in, so the
/// answer costs no conversion and cannot disagree with the bar by a rounding.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CaptionHit {
    /// Nothing interactive. The band drags the window, and the system provides double-click
    /// maximize, Aero shake and the keyboard window commands off that one answer.
    Drag,
    /// An ordinary control. Input belongs to the client area.
    Client,
    /// One of the window commands the application draws. This is what raises the Snap
    /// Layouts flyout.
    Button(CaptionButton),
}

/// The one-pixel frame DWM draws around the window.
///
/// Takes an [`Scrgb`] because a border is display-referred output, and the only supplier of
/// one is `OutputTransform::apply`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BorderColor {
    /// The system's, which follows the user's accent and light/dark choice.
    System,
    /// No border at all. Removed rather than painted over: a border painted in the
    /// backdrop's colour is still a border on a display that renders them differently.
    None,
    /// A colour from the application's own palette.
    Solid(Scrgb),
}

impl BorderColor {
    fn attribute(self) -> u32 {
        match self {
            Self::System => DWMWA_COLOR_DEFAULT,
            Self::None => DWMWA_COLOR_NONE,
            Self::Solid(c) => {
                let [r, g, b, _] = c.to_srgb8();
                // `0x00bbggrr`. Alpha is not carried: DWM composites the border against the
                // desktop, and a transparent one is `None` rather than a low alpha.
                u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16)
            }
        }
    }
}

/// Which caption button the pointer is over, and which it is pressing.
///
/// Both arrive as non-client messages the application never sees: once `WM_NCHITTEST`
/// answers `HTMAXBUTTON` the pointer stream for that button is the system's. So the window
/// forwards the state and the application draws from it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CaptionState {
    pub hover: Option<CaptionButton>,
    pub pressed: Option<CaptionButton>,
}

type HitAuthority = Box<dyn FnMut(f32, f32) -> CaptionHit>;
type StateSink = Box<dyn FnMut(CaptionState)>;

pub(crate) struct Caption {
    spec: CaptionSpec,
    /// Absent until the surface that owns the hit array exists, which is necessarily after
    /// the window does. The band answers [`CaptionHit::Drag`] until then.
    hit: RefCell<Option<HitAuthority>>,
    state: Cell<CaptionState>,
    on_state: RefCell<Option<StateSink>>,
    border: Cell<BorderColor>,
}

impl Caption {
    pub(crate) fn new(spec: CaptionSpec) -> Self {
        Self {
            spec,
            hit: RefCell::new(None),
            state: Cell::new(CaptionState::default()),
            on_state: RefCell::new(None),
            border: Cell::new(BorderColor::System),
        }
    }

    pub(crate) fn spec(&self) -> CaptionSpec {
        self.spec
    }

    pub(crate) fn set_hit_authority(&self, f: HitAuthority) {
        *self.hit.borrow_mut() = Some(f);
    }

    pub(crate) fn set_state_sink(&self, f: StateSink) {
        *self.on_state.borrow_mut() = Some(f);
    }

    /// Applies the corner preference and forces the frame recalculation that makes the
    /// custom caption take effect. Called once, after the window exists and before it shows.
    pub(crate) fn apply(&self, hwnd: HWND) {
        let preference = self.spec.corners.attribute();
        // SAFETY: `hwnd` is live; the value is a stack local of the stated size.
        unsafe {
            _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                (&raw const preference).cast(),
                size_of_val(&preference) as u32,
            );
        }
        self.apply_border(hwnd);
        // Nothing recalculates the frame on its own: creation's `WM_NCCALCSIZE` is answered
        // before the state is installed, and `ShowWindow` sends none. Without this the
        // window keeps its system caption however `WM_NCCALCSIZE` is answered.
        // SAFETY: `hwnd` is live; every position and size argument is ignored under the
        // `NOMOVE`/`NOSIZE` flags.
        unsafe {
            _ = SetWindowPos(
                hwnd,
                core::ptr::null_mut(),
                0,
                0,
                0,
                0,
                (SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) as u32,
            );
        }
    }

    pub(crate) fn set_border(&self, hwnd: HWND, color: BorderColor) {
        self.border.set(color);
        self.apply_border(hwnd);
    }

    fn apply_border(&self, hwnd: HWND) {
        let color = self.border.get().attribute();
        // SAFETY: as in `apply`.
        unsafe {
            _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                (&raw const color).cast(),
                size_of_val(&color) as u32,
            );
        }
    }

    /// Handles the caption's messages. `None` means this is not one of them.
    pub(crate) fn message(
        &self,
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match message as i32 {
            // Keep the left, right and bottom frames so DWM still resizes and shadows the
            // window; take back only the top border, which is where the caption goes.
            WM_NCCALCSIZE => {
                // Both `wParam` forms occur and they carry different `lParam` types: a bare
                // `RECT` when it is zero, an `NCCALCSIZE_PARAMS` whose first rect is the
                // proposed client area when it is not.
                let target: *mut RECT = if wparam != 0 {
                    // SAFETY: the system passes a writable `NCCALCSIZE_PARAMS` valid for the
                    // duration of this message.
                    unsafe { &raw mut (*(lparam as *mut NCCALCSIZE_PARAMS)).rgrc[0] }
                } else {
                    lparam as *mut RECT
                };
                if target.is_null() {
                    return None;
                }
                // SAFETY: a writable rect of the form selected above.
                let requested = unsafe { *target };
                // Rewrites the rect in place, which is what makes the two reads a before and
                // an after rather than two views of one value.
                // SAFETY: `hwnd` is live and the arguments are the ones just received.
                let default = unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
                // SAFETY: the rect is still the system's and still writable.
                unsafe {
                    (*target).top = if is_zoomed(hwnd) {
                        // A maximized window's frame hangs off the work area by the border on
                        // every edge, so the default inset has to be kept or the client runs
                        // over the taskbar. But it also reserves a caption band this window
                        // does not have. Keep the border and drop the band, taking the border
                        // from what `DefWindowProc` just did to the bottom edge so no
                        // assumption about caption height is made anywhere.
                        requested.top + (requested.bottom - (*target).bottom)
                    } else {
                        requested.top
                    };
                }
                Some(default)
            }

            WM_NCHITTEST => {
                let (x, y) = screen_point(lparam);
                Some(self.hit_code(hwnd, x, y) as isize)
            }

            // Hover. Consumed over a button, forwarded elsewhere so the system keeps the
            // drag strip and the resize edges.
            WM_NCPOINTERUPDATE => {
                let hover = self.contact_button(hwnd, lparam);
                self.publish(CaptionState {
                    hover,
                    pressed: self.state.get().pressed,
                });
                hover.map(|_| 0)
            }

            // Clears hover and **not** pressed: this fires when the pointer leaves the window
            // and also the moment a hover becomes a contact, so clearing the press here would
            // cancel every press as it happened. Not consumed — a client-area pointer
            // consumer wants this same message.
            WM_POINTERLEAVE => {
                self.publish(CaptionState {
                    hover: None,
                    pressed: self.state.get().pressed,
                });
                None
            }

            // Handled rather than deferred: `DefWindowProc`'s own non-client button handling
            // draws the system's chrome into a frame this window no longer has. A secondary
            // press over a button opens the window menu, so it forwards in both halves.
            WM_NCPOINTERDOWN => match self.contact_button(hwnd, lparam) {
                Some(button) if button_change(wparam) == POINTER_CHANGE_FIRSTBUTTON_DOWN => {
                    self.publish(CaptionState {
                        hover: Some(button),
                        pressed: Some(button),
                    });
                    Some(0)
                }
                _ => None,
            },

            WM_NCPOINTERUP => {
                let pressed = self.state.get().pressed;
                match self.contact_button(hwnd, lparam) {
                    // Only a release on the button the press landed on commits, which is what
                    // lets a user slide off a close button to cancel it.
                    Some(button) if pressed == Some(button) => {
                        self.publish(CaptionState {
                            hover: Some(button),
                            pressed: None,
                        });
                        // SAFETY: `hwnd` is live and the command is a documented `SC_*`.
                        unsafe {
                            SendMessageW(hwnd, WM_SYSCOMMAND as u32, command(hwnd, button), 0)
                        };
                        Some(0)
                    }
                    _ => {
                        if pressed.is_some() {
                            self.publish(CaptionState {
                                hover: self.state.get().hover,
                                pressed: None,
                            });
                        }
                        None
                    }
                }
            }

            // Neither activation nor a theme change preserves the frame colour.
            WM_ACTIVATE | WM_THEMECHANGED => {
                self.apply_border(hwnd);
                None
            }

            _ => None,
        }
    }

    /// Asks the application's hit authority, or answers `Drag` if there is not one yet.
    ///
    /// Detached across the call: the authority may install a new one or re-enter the window
    /// procedure, either of which would otherwise be a double borrow. A re-entrant hit test
    /// then answers `Drag`.
    fn hit(&self, x_dips: f32, y_dips: f32) -> CaptionHit {
        let Some(mut authority) = self.hit.borrow_mut().take() else {
            return CaptionHit::Drag;
        };
        let answer = authority(x_dips, y_dips);
        let mut slot = self.hit.borrow_mut();
        if slot.is_none() {
            *slot = Some(authority);
        }
        answer
    }

    /// What is at a screen point: one of the eight resize zones, a caption button, the drag
    /// strip, or the client area.
    ///
    /// Also what the non-client pointer arms resolve through, because those messages do not
    /// carry the hit code — `HIWORD(wParam)` is documented as the hit-test value and
    /// observably is not, it is the same `POINTER_FLAG_*` word the client messages carry.
    /// Both callers resolving the same point through the same authority is the property that
    /// matters; a cached last answer would be a second source of truth.
    fn hit_code(&self, hwnd: HWND, x: i32, y: i32) -> i32 {
        let metrics = Metrics::for_window(hwnd);
        let mut window = RECT::default();
        // SAFETY: `hwnd` is live; the destination is a stack local.
        if !unsafe { GetWindowRect(hwnd, &mut window) }.as_bool() {
            return HTCLIENT;
        }
        // A maximized window has no resize edges, and claiming them there puts a resize
        // cursor over the first row of the application's own bar.
        if !is_zoomed(hwnd)
            && let Some(zone) = resize_zone(&window, metrics, x, y)
        {
            return zone;
        }
        let Some((cx, cy)) = client_point(hwnd, x, y) else {
            return HTCLIENT;
        };
        if cy >= self.band_px(metrics) {
            return HTCLIENT;
        }
        match self.hit(metrics.dips(cx), metrics.dips(cy)) {
            CaptionHit::Drag => HTCAPTION,
            CaptionHit::Client => HTCLIENT,
            CaptionHit::Button(button) if self.spec.buttons.has(button) => button.hit_code(),
            // Falls back to dragging rather than to the client, so the bar keeps working
            // where a stale layout still reports a button this window does not draw.
            CaptionHit::Button(_) => HTCAPTION,
        }
    }

    fn contact_button(&self, hwnd: HWND, lparam: LPARAM) -> Option<CaptionButton> {
        let (x, y) = screen_point(lparam);
        CaptionButton::from_hit_code(self.hit_code(hwnd, x, y))
    }

    fn band_px(&self, metrics: Metrics) -> i32 {
        match self.spec.height {
            Some(dips) => metrics.px(dips),
            None => metrics.caption,
        }
    }

    /// Stores the button state and tells the application, on a change and only on one.
    ///
    /// Stored **before** the sink runs, so a sink that asks for it sees the new value.
    /// Detached across the call, as in [`hit`](Self::hit).
    fn publish(&self, next: CaptionState) {
        if self.state.get() == next {
            return;
        }
        self.state.set(next);
        let Some(mut sink) = self.on_state.borrow_mut().take() else {
            return;
        };
        sink(next);
        let mut slot = self.on_state.borrow_mut();
        if slot.is_none() {
            *slot = Some(sink);
        }
    }
}

fn is_zoomed(hwnd: HWND) -> bool {
    // SAFETY: `hwnd` is live.
    unsafe { IsZoomed(hwnd) }.as_bool()
}

/// Which system command a press on `button` issues. Only maximize depends on state, and on
/// the window's rather than on anything the application tracks.
fn command(hwnd: HWND, button: CaptionButton) -> usize {
    let command = match button {
        CaptionButton::Minimize => SC_MINIMIZE,
        CaptionButton::Maximize if is_zoomed(hwnd) => SC_RESTORE,
        CaptionButton::Maximize => SC_MAXIMIZE,
        CaptionButton::Close => SC_CLOSE,
    };
    command as usize
}

/// The eight resize edges, or `None` for anywhere inside them.
fn resize_zone(window: &RECT, metrics: Metrics, x: i32, y: i32) -> Option<i32> {
    let left = x < window.left + metrics.frame_x;
    let right = x >= window.right - metrics.frame_x;
    // The top band is the narrow one — see `Metrics::frame_top`.
    let top = y < window.top + metrics.frame_top;
    let bottom = y >= window.bottom - metrics.frame_y;
    match (top, bottom, left, right) {
        (true, _, true, _) => Some(HTTOPLEFT),
        (true, _, _, true) => Some(HTTOPRIGHT),
        (true, ..) => Some(HTTOP),
        (_, true, true, _) => Some(HTBOTTOMLEFT),
        (_, true, _, true) => Some(HTBOTTOMRIGHT),
        (_, true, ..) => Some(HTBOTTOM),
        (_, _, true, _) => Some(HTLEFT),
        (_, _, _, true) => Some(HTRIGHT),
        _ => None,
    }
}

/// Which button transition produced this pointer message.
///
/// `ButtonChangeType` rather than the `wParam` flags: the flags carry which buttons are
/// *held*, which cannot tell a primary press from a secondary press made while the primary
/// is down. A retired pointer reads `POINTER_CHANGE_NONE`, which forwards.
fn button_change(wparam: WPARAM) -> POINTER_BUTTON_CHANGE_TYPE {
    // `GET_POINTERID_WPARAM` is a C macro with no metadata.
    let id = (wparam & 0xffff) as u32;
    let mut info = POINTER_INFO::default();
    // SAFETY: the destination is a stack local of the type the call writes, and the id came
    // from the message being serviced.
    if unsafe { GetPointerInfo(id, &mut info) }.as_bool() {
        info.ButtonChangeType
    } else {
        POINTER_CHANGE_NONE
    }
}

/// The screen point a non-client message carries, sign-extended.
///
/// The halves are signed: a window left of or above the primary monitor has negative screen
/// coordinates, and reading them unsigned puts every point 65,000 pixels from where it is.
fn screen_point(lparam: LPARAM) -> (i32, i32) {
    (
        lparam as u16 as i16 as i32,
        (lparam >> 16) as u16 as i16 as i32,
    )
}

/// A screen point in the window's client space, in physical pixels.
fn client_point(hwnd: HWND, x: i32, y: i32) -> Option<(i32, i32)> {
    let mut point = POINT { x, y };
    // SAFETY: `hwnd` is live and the point is a stack local the call writes back through.
    if unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() {
        Some((point.x, point.y))
    } else {
        None
    }
}
