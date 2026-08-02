//! What the custom caption answers `WM_NCHITTEST`, and where its frame is.
//!
//! `SendMessageW` from the window's own thread calls the window procedure directly, so
//! nothing here needs a pump, injected input, the foreground or a visible window — every
//! answer below is the production path resolved synchronously. That is the seam: the rest
//! of the caption's contract — dragging, double-click to maximize, `Win`+arrow, the window
//! menu, and a maximized window's frame — needs real input against a real foreground
//! window and is driven by `examples/caption` instead.
//!
//! Every window here asks for pointer input and is created hidden. The opt-in is
//! process-wide, one way, and rejected once the process owns a window, so a plain window
//! created first would make a later caption window's creation fail — which is also why
//! creation is serialized.

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

/// Serialized, and every window asks for pointer input: the opt-in is process-wide and is
/// rejected once the process owns a window, so the first creation in this binary has to
/// finish before the second starts whichever test runs first.
static CREATE: Mutex<()> = Mutex::new(());

/// A hidden window, with a caption of its own or with the system's.
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

/// Every window here outlives the test that made it, so its metrics are always there.
fn metrics(window: &Window) -> Metrics {
    window.metrics().expect("an open window")
}

/// The application's hit authority, scripted: what it answers, how often it was asked, and
/// with what.
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
    // SAFETY: the window is live and the destination is a stack local.
    assert!(
        unsafe { GetWindowRect(window.hwnd(), &mut rect) } != 0,
        "the window has a rect"
    );
    rect
}

/// The window's client origin in screen coordinates.
fn client_origin(window: &Window) -> (i32, i32) {
    let mut point = Point::default();
    // SAFETY: the window is live and the point is a stack local the call writes back
    // through.
    assert!(
        unsafe { ClientToScreen(window.hwnd(), &mut point) } != 0,
        "the window has a client origin"
    );
    (point.x, point.y)
}

/// What the window answers for a screen point.
fn hit(window: &Window, x: i32, y: i32) -> isize {
    let lparam = ((y as isize & 0xffff) << 16) | (x as isize & 0xffff);
    // SAFETY: the window is live and belongs to this thread, so this calls its window
    // procedure directly; `WM_NCHITTEST` reads only the packed point.
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

/// The caption band's height in physical pixels.
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

/// A point inside the band and clear of the resize frame. The same place in every test, so
/// what changes between them is the answer.
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

/// The system's own resize bands, read off `DefWindowProc`'s answers for an ordinary window
/// of the same style on the same display.
///
/// The oracle. A frame width this crate computed, checked against a frame width this crate
/// computed, would agree however wrong both were.
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

/// The eight resize zones, and their boundaries where the system puts its own — on all
/// four edges, including the narrow top one.
///
/// The top band is not the frame width: the window rect extends past the visible frame on
/// the left, right and bottom, so a band there is mostly outside the window, while at the
/// top it comes entirely out of the caption the application drew. The system takes the
/// border's width back off it, and so does this ([`Metrics::frame_top`]).
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

    // Where the zones end, which is what "at the DPI-correct widths" means: a zone one
    // pixel narrower than the system's is a frame the user grabs at and misses.
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

/// The band's answer is the application's, whatever it is — including `HTMAXBUTTON`,
/// without which the Snap Layouts flyout never appears.
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

/// The authority is asked in the space the layout solved in, so its answer cannot disagree
/// with the bar it drew by a conversion.
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

/// Below the band the client owns the point outright, and the authority is not consulted
/// at all — so an application that answers carelessly outside its bar cannot claim one.
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

/// Before the surface that owns the hit array exists there is no authority, and the band
/// drags — a window you can move being the better intermediate state.
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

/// A button the window does not draw is never answered for, however the authority reports
/// it: what the system opens on `HTMAXBUTTON` offers to maximize a window with no way back.
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

/// A stated band height moves the boundary, and `caption_height_dips` reports the same
/// number the hit test resolves against.
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
