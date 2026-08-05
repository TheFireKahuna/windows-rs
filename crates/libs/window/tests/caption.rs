//! Tests the custom caption's `WM_NCHITTEST` answers and the widths of its resize frame.
//!
//! `SendMessageW` from the window's own thread calls the window procedure directly, so no
//! test here needs a pump, injected input, the foreground or a visible window: each answer
//! comes from the production path, resolved synchronously. Dragging, double-click to
//! maximize, `Win`+arrow, the window menu and a maximized window's frame need real input
//! against a foreground window, and the `caption` example drives those.
//!
//! Every window here asks for pointer input and is created hidden. The pointer-input opt-in
//! is process-wide, one way, and rejected once the process owns a window, so a plain window
//! created first would fail a later caption window's creation.

use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::Mutex;

use windows_window::{CaptionButton, CaptionButtons, CaptionHit, CaptionSpec, Metrics, Window};

const WM_NCHITTEST: u32 = 0x0084;

const HTCLIENT: isize = 1;
const HTCAPTION: isize = 2;
const HTMINBUTTON: isize = 8;
const HTMAXBUTTON: isize = 9;
const HTLEFT: isize = 10;
const HTRIGHT: isize = 11;
const HTTOP: isize = 12;
const HTTOPLEFT: isize = 13;
const HTTOPRIGHT: isize = 14;
const HTBOTTOM: isize = 15;
const HTBOTTOMLEFT: isize = 16;
const HTBOTTOMRIGHT: isize = 17;
const HTCLOSE: isize = 20;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct Point {
    x: i32,
    y: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageW(hwnd: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
    fn ClientToScreen(hwnd: *mut c_void, point: *mut Point) -> i32;
}

/// Serializes window creation across the test binary's threads.
///
/// The pointer-input opt-in every window here asks for is process-wide and is rejected once
/// the process owns a window, so one creation must finish before the next one starts,
/// whichever test runs first.
static CREATE: Mutex<()> = Mutex::new(());

/// Creates a hidden window, with a caption of its own when `caption` is `Some`.
fn create(caption: Option<CaptionSpec>) -> Window {
    let _serial = CREATE.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut builder = Window::new("windows-window — caption hit test")
        .size(900, 600)
        .pointer_input()
        .hidden();
    if let Some(spec) = caption {
        builder = builder.custom_caption(spec);
    }
    builder.create().expect("a window can be created")
}

/// Returns `window`'s metrics, which every window here has because it outlives the test
/// that made it.
fn metrics(window: &Window) -> Metrics {
    window.metrics().expect("an open window")
}

/// A caption hit authority whose answer the test sets, recording how many times it was
/// asked and the last client-space point it was asked about.
#[derive(Clone)]
struct Authority {
    answer: Rc<Cell<CaptionHit>>,
    calls: Rc<Cell<u32>>,
    asked_at: Rc<Cell<(f32, f32)>>,
}

impl Authority {
    fn install(window: &Window) -> Self {
        let this = Self {
            answer: Rc::new(Cell::new(CaptionHit::Drag)),
            calls: Rc::new(Cell::new(0)),
            asked_at: Rc::new(Cell::new((f32::NAN, f32::NAN))),
        };
        let scripted = this.clone();
        window
            .on_caption_hit(move |x, y| {
                scripted.calls.set(scripted.calls.get() + 1);
                scripted.asked_at.set((x, y));
                scripted.answer.get()
            })
            .expect("a window with a caption of its own");
        this
    }

    fn answers(&self, hit: CaptionHit) {
        self.answer.set(hit);
    }
}

fn window_rect(window: &Window) -> Rect {
    let mut rect = Rect::default();
    // SAFETY: `GetWindowRect` accepts any handle value and reports failure for one it does
    // not recognise; it writes only through `rect`, a live stack local of the size it
    // expects.
    assert!(
        unsafe { GetWindowRect(window.hwnd(), &mut rect) } != 0,
        "the window has a rect"
    );
    rect
}

/// Returns the window's client origin in screen coordinates.
fn client_origin(window: &Window) -> (i32, i32) {
    let mut point = Point::default();
    // SAFETY: `ClientToScreen` accepts any handle value and writes only through `point`, a
    // live stack local of the size it expects.
    assert!(
        unsafe { ClientToScreen(window.hwnd(), &mut point) } != 0,
        "the window has a client origin"
    );
    (point.x, point.y)
}

/// Returns the `HT*` code the window answers for the screen point (`x`, `y`).
fn hit(window: &Window, x: i32, y: i32) -> isize {
    let lparam = ((y as isize & 0xffff) << 16) | (x as isize & 0xffff);
    // SAFETY: `SendMessageW` accepts any handle value, and `WM_NCHITTEST` reads the point
    // out of `lparam` rather than through a pointer. The window belongs to this thread, so
    // the call runs its window procedure inline.
    unsafe { SendMessageW(window.hwnd(), WM_NCHITTEST, 0, lparam) }
}

#[track_caller]
fn assert_hit(window: &Window, (x, y): (i32, i32), expected: isize, what: &str) {
    let actual = hit(window, x, y);
    assert!(
        actual == expected,
        "{what} at ({x}, {y}): {} ({actual}) rather than {} ({expected})",
        name(actual),
        name(expected)
    );
}

fn name(code: isize) -> &'static str {
    match code {
        HTCLIENT => "HTCLIENT",
        HTCAPTION => "HTCAPTION",
        HTMINBUTTON => "HTMINBUTTON",
        HTMAXBUTTON => "HTMAXBUTTON",
        HTLEFT => "HTLEFT",
        HTRIGHT => "HTRIGHT",
        HTTOP => "HTTOP",
        HTTOPLEFT => "HTTOPLEFT",
        HTTOPRIGHT => "HTTOPRIGHT",
        HTBOTTOM => "HTBOTTOM",
        HTBOTTOMLEFT => "HTBOTTOMLEFT",
        HTBOTTOMRIGHT => "HTBOTTOMRIGHT",
        HTCLOSE => "HTCLOSE",
        _ => "an unnamed hit code",
    }
}

/// Returns the caption band's height in physical pixels.
///
/// # Panics
///
/// Panics unless the band is taller than the resize frame, which every point the tests
/// probe is chosen on the assumption of.
fn band_px(window: &Window) -> i32 {
    let metrics = metrics(window);
    let band = metrics.px(window
        .caption_height_dips()
        .expect("a window with a caption of its own"));
    assert!(
        band > metrics.frame_y,
        "the band ({band} px) is taller than the resize frame ({} px), which every point \
         below is chosen on the assumption of",
        metrics.frame_y
    );
    band
}

/// Returns a screen point inside the caption band and clear of the resize frame.
///
/// Every test probes this same place, so the answer is the only thing that varies between
/// them.
fn band_point(window: &Window) -> (i32, i32) {
    let metrics = metrics(window);
    let (x, y) = client_origin(window);
    (
        x + metrics.px(200.0),
        y + (metrics.frame_y + band_px(window)) / 2,
    )
}

/// How deep each of the system's own resize bands runs, in pixels from the window rect.
struct SystemFrame {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

/// Returns the depth of each of the system's own resize bands, probed from
/// `DefWindowProc`'s answers on an ordinary window of the same style and display.
///
/// The depths come from the system rather than from this crate's metrics, so
/// [`the_resize_frame_is_the_systems`] compares against a width it did not compute itself.
fn system_frame() -> SystemFrame {
    let window = create(None);
    let rect = window_rect(&window);
    let mid_x = (rect.left + rect.right) / 2;
    let mid_y = (rect.top + rect.bottom) / 2;
    let depth = |code: isize, point: &dyn Fn(i32) -> (i32, i32)| {
        (1..64)
            .find(|&d| {
                let (x, y) = point(d);
                hit(&window, x, y) != code
            })
            .expect("the system's resize band ends within 64 pixels")
    };
    SystemFrame {
        left: depth(HTLEFT, &|d| (rect.left + d, mid_y)),
        right: depth(HTRIGHT, &|d| (rect.right - 1 - d, mid_y)),
        top: depth(HTTOP, &|d| (mid_x, rect.top + d)),
        bottom: depth(HTBOTTOM, &|d| (mid_x, rect.bottom - 1 - d)),
    }
}

/// Puts the eight resize zones and their boundaries where the system puts its own, on all
/// four edges including the narrower top one.
///
/// The top band is not the frame width. The window rect extends past the visible frame on
/// the left, right and bottom, so a band on those edges lies mostly outside the window,
/// while at the top it comes entirely out of the caption the application drew. The system
/// subtracts the border's width there, and [`Metrics::frame_top`] reports the result.
#[test]
fn the_resize_frame_is_the_systems() {
    let system = system_frame();
    let window = create(Some(CaptionSpec::default()));
    Authority::install(&window);
    let metrics = metrics(&window);
    let rect = window_rect(&window);
    let mid_x = (rect.left + rect.right) / 2;
    let mid_y = (rect.top + rect.bottom) / 2;
    let (frame_x, frame_top) = (metrics.frame_x, metrics.frame_top);

    assert!(
        [system.left, system.right, system.top, system.bottom]
            == [frame_x, frame_x, frame_top, metrics.frame_y],
        "the caption's bands are left {frame_x} right {frame_x} top {frame_top} bottom {} \
         where the system's are left {} right {} top {} bottom {}, at {} DPI",
        metrics.frame_y,
        system.left,
        system.right,
        system.top,
        system.bottom,
        metrics.dpi
    );
    assert!(
        frame_top < metrics.frame_y,
        "the top band is the frame width less the border DWM draws, so it is narrower than \
         the other three — {frame_top} against {} at {} DPI",
        metrics.frame_y,
        metrics.dpi
    );

    assert_hit(&window, (rect.left, mid_y), HTLEFT, "the left edge");
    assert_hit(&window, (rect.right - 1, mid_y), HTRIGHT, "the right edge");
    assert_hit(&window, (mid_x, rect.top), HTTOP, "the top edge");
    assert_hit(
        &window,
        (mid_x, rect.bottom - 1),
        HTBOTTOM,
        "the bottom edge",
    );
    assert_hit(
        &window,
        (rect.left, rect.top),
        HTTOPLEFT,
        "the top-left corner",
    );
    let top_right = (rect.right - 1, rect.top);
    assert_hit(&window, top_right, HTTOPRIGHT, "the top-right corner");
    let bottom_left = (rect.left, rect.bottom - 1);
    assert_hit(&window, bottom_left, HTBOTTOMLEFT, "the bottom-left corner");
    let bottom_right = (rect.right - 1, rect.bottom - 1);
    assert_hit(
        &window,
        bottom_right,
        HTBOTTOMRIGHT,
        "the bottom-right corner",
    );

    // Where the zones end: the last pixel of a band, and the first pixel inside it.
    let inside_x = (rect.left + frame_x, mid_y);
    let last_x = (rect.left + frame_x - 1, mid_y);
    assert_hit(&window, last_x, HTLEFT, "the last column of the left edge");
    assert_hit(&window, inside_x, HTCLIENT, "the first column inside it");
    let last_y = (mid_x, rect.top + frame_top - 1);
    let inside_y = (mid_x, rect.top + frame_top);
    assert_hit(&window, last_y, HTTOP, "the last row of the top edge");
    assert_hit(
        &window,
        inside_y,
        HTCAPTION,
        "the first row inside it, which is the band",
    );
}

/// Answers the band with whatever the hit authority returns, including `HTMAXBUTTON`, which
/// is the only code the Snap Layouts flyout opens on.
#[test]
fn the_band_answers_from_the_hit_authority() {
    let window = create(Some(CaptionSpec::default()));
    let authority = Authority::install(&window);
    let point = band_point(&window);

    for (answer, expected, what) in [
        (CaptionHit::Drag, HTCAPTION, "empty bar"),
        (CaptionHit::Client, HTCLIENT, "a control in the bar"),
        (
            CaptionHit::Button(CaptionButton::Minimize),
            HTMINBUTTON,
            "the minimize button",
        ),
        (
            CaptionHit::Button(CaptionButton::Maximize),
            HTMAXBUTTON,
            "the maximize button",
        ),
        (
            CaptionHit::Button(CaptionButton::Close),
            HTCLOSE,
            "the close button",
        ),
    ] {
        authority.answers(answer);
        assert_hit(&window, point, expected, what);
    }
}

/// Asks the hit authority in client-space DIPs, the space the layout solves in, so no
/// coordinate conversion sits between the bar it drew and the answer it gives.
#[test]
fn the_authority_is_asked_in_client_space_dips() {
    let window = create(Some(CaptionSpec::default()));
    let authority = Authority::install(&window);
    let metrics = metrics(&window);
    let (origin_x, origin_y) = client_origin(&window);
    let (x, y) = band_point(&window);

    assert_hit(&window, (x, y), HTCAPTION, "the band");

    let asked = authority.asked_at.get();
    let expected = (metrics.dips(x - origin_x), metrics.dips(y - origin_y));
    assert!(
        asked == expected,
        "asked at {asked:?} DIPs for the client point ({}, {}) px, which is {expected:?}",
        x - origin_x,
        y - origin_y
    );
}

/// Answers `HTCLIENT` below the band without consulting the hit authority, so an authority
/// that returns a button for a point outside the bar cannot claim one.
#[test]
fn below_the_band_is_the_clients_and_the_authority_is_not_asked() {
    let window = create(Some(CaptionSpec::default()));
    let authority = Authority::install(&window);
    let band = band_px(&window);
    let (x, origin_y) = (band_point(&window).0, client_origin(&window).1);

    authority.answers(CaptionHit::Button(CaptionButton::Close));
    assert_hit(
        &window,
        (x, origin_y + band - 1),
        HTCLOSE,
        "the last row of the band",
    );

    let asked = authority.calls.get();
    assert_hit(
        &window,
        (x, origin_y + band),
        HTCLIENT,
        "the first row below the band",
    );
    assert!(
        authority.calls.get() == asked,
        "the authority was asked about a point outside the band"
    );
}

/// Drags from the band while no hit authority is installed, which is the window's state
/// until the surface that owns the hit array exists.
#[test]
fn a_band_with_no_authority_drags() {
    let window = create(Some(CaptionSpec::default()));
    let point = band_point(&window);
    assert_hit(
        &window,
        point,
        HTCAPTION,
        "the band before an authority is installed",
    );
}

/// Suppresses a button code for a button the caption spec does not declare, whatever the
/// hit authority answers.
///
/// The flyout the system opens on `HTMAXBUTTON` would otherwise offer to maximize a window
/// that draws no button to restore it.
#[test]
fn a_button_the_window_does_not_draw_is_never_answered_for() {
    let window = create(Some(CaptionSpec {
        buttons: CaptionButtons {
            minimize: true,
            maximize: false,
            close: true,
        },
        ..CaptionSpec::default()
    }));
    let authority = Authority::install(&window);
    let point = band_point(&window);

    authority.answers(CaptionHit::Button(CaptionButton::Maximize));
    assert_hit(
        &window,
        point,
        HTCAPTION,
        "a maximize button this window does not draw",
    );
    authority.answers(CaptionHit::Button(CaptionButton::Close));
    assert_hit(&window, point, HTCLOSE, "a close button it does");
}

/// Moves the band boundary to a stated height, and reports that same height from
/// [`Window::caption_height_dips`].
#[test]
fn a_stated_band_height_moves_the_boundary() {
    const HEIGHT: f32 = 48.0;
    let window = create(Some(CaptionSpec {
        height: Some(HEIGHT),
        ..CaptionSpec::default()
    }));
    let authority = Authority::install(&window);
    let metrics = metrics(&window);
    let (x, origin_y) = (band_point(&window).0, client_origin(&window).1);
    let band = metrics.px(HEIGHT);

    assert!(
        window.caption_height_dips() == Some(HEIGHT),
        "the stated height is reported back"
    );

    authority.answers(CaptionHit::Button(CaptionButton::Close));
    assert_hit(
        &window,
        (x, origin_y + band - 1),
        HTCLOSE,
        "the last row of a 48 DIP band",
    );
    assert_hit(
        &window,
        (x, origin_y + band),
        HTCLIENT,
        "the first row below it",
    );
}
