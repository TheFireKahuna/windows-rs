//! What a window's creation and destruction do to the thread it belongs to: its handlers, its
//! message queue, whether anything it draws can be seen, and what a second pacer does.
//!
//! Each test runs on its own harness thread, and a message queue is thread-affine, so the quit
//! message one test posts cannot reach another's pump.

use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;

use windows_window::{Window, pump, quit};

const WM_USER: u32 = 0x0400;
const WM_SIZE: u32 = 0x0005;

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageW(hwnd: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
    fn IsWindow(hwnd: *mut c_void) -> i32;
}

/// A window built hidden, so nothing appears and no test needs the foreground.
fn create(quit_on_close: bool) -> Window {
    Window::new("windows-window — lifecycle")
        .size(400, 300)
        .hidden()
        .quit_on_close(quit_on_close)
        .create()
        .expect("a window can be created")
}

#[test]
fn a_new_window_is_open_and_measurable() {
    let window = create(false);
    assert!(window.is_open());
    assert_ne!(unsafe { IsWindow(window.hwnd()) }, 0);
    let (width, height) = window.client_size().expect("an open window");
    assert!(width > 0 && height > 0);
    assert!(
        width <= 400 && height <= 300,
        "the client area is inside the window it was asked for"
    );
}

#[test]
fn dropping_the_window_destroys_it() {
    let window = create(false);
    let hwnd = window.hwnd();
    drop(window);
    assert_eq!(unsafe { IsWindow(hwnd) }, 0);
}

/// A handle value can outlive the window it named, and a `Window` value outlives its window
/// whenever something else closed it. Every accessor answers from identity rather than from
/// liveness, so a stale value stays stale however the handle table is reused afterwards.
#[test]
fn a_closed_window_leaves_a_value_that_claims_nothing() {
    let window = create(false);
    let hwnd = window.hwnd();
    // Behind the value's back, the way the user's close button does it.
    unsafe {
        SendMessageW(hwnd, /* WM_CLOSE */ 0x0010, 0, 0)
    };

    assert!(!window.is_open());
    assert!(window.client_size().is_none());
    assert!(window.metrics().is_none());
    assert!(window.visibility().is_none());
    assert!(window.show().is_none());

    // Churn, so anything the closed window's handle value or state box is reused for has a
    // chance to be mistaken for it.
    let churn: Vec<Window> = (0..16).map(|_| create(false)).collect();
    assert!(
        !window.is_open(),
        "a stale value claimed a window it does not own"
    );
    drop(churn);
}

#[test]
fn handlers_dispatch_through_the_window_procedure() {
    let messages = Rc::new(Cell::new(0));
    let sizes = Rc::new(Cell::new((0, 0)));
    let (seen, captured) = (Rc::clone(&messages), Rc::clone(&sizes));
    let window = Window::new("windows-window — handlers")
        .hidden()
        .quit_on_close(false)
        .on_message(move |_hwnd, message, _wparam, _lparam| {
            (message == WM_USER).then(|| {
                seen.set(seen.get() + 1);
                0
            })
        })
        .on_resize(move |width, height| captured.set((width, height)))
        .create()
        .expect("a window can be created");

    unsafe { SendMessageW(window.hwnd(), WM_USER, 0, 0) };
    assert_eq!(messages.get(), 1);

    unsafe { SendMessageW(window.hwnd(), WM_SIZE, 0, 640 | (480 << 16)) };
    assert_eq!(sizes.get(), (640, 480));
}

/// The window a composition host builds: created hidden, shown once there is something to
/// show. Nothing it draws can be seen until then, and a producer parked on that is what stops
/// a frame being drawn into a window the user has never seen.
#[test]
fn a_window_built_hidden_cannot_be_seen_until_it_is_shown() {
    let window = create(false);
    let visibility = window.visibility().expect("an open window");
    assert!(
        visibility.is_hidden(),
        "a window that has never been shown reports that it can be seen"
    );
    window.show().expect("an open window");
    assert!(
        !visibility.is_hidden(),
        "showing the window did not reach it"
    );
}

/// A pacer and a present thread park on the same window, so a window mints a watch per
/// consumer rather than handing out one shared wake.
#[test]
fn a_window_gives_every_consumer_its_own_watch() {
    let window = create(false);
    let first = window.watch().expect("an open window");
    let second = window.watch().expect("an open window");
    assert!(first.is_hidden() && second.is_hidden());
    window.show().expect("an open window");
    assert!(
        !first.is_hidden() && !second.is_hidden(),
        "a watch did not see the window come up"
    );
}

#[test]
fn closing_the_last_window_ends_the_pump() {
    let window = create(true);
    assert!(pump(), "nothing has quit yet");
    drop(window);
    assert!(!pump(), "destroying the window did not post a quit message");
}

/// An application with more than one window, or one that outlives its window, opts out — and
/// then nothing about the destruction reaches the pump.
#[test]
fn a_window_that_opts_out_leaves_the_pump_running() {
    let window = create(false);
    drop(window);
    assert!(pump(), "destroying the window quit a pump that opted out");
    assert!(pump(), "and it is still running");
}

/// Peeking removes the quit message, so a caller that pumps in a loop and checks the answer on
/// only some iterations would lose it. It is put back.
#[test]
fn the_quit_answer_is_sticky() {
    quit();
    assert!(!pump(), "the quit was not seen");
    assert!(!pump(), "the quit was seen once and then forgotten");
}

/// Two pacers would post frames past each other's gate, so the second is refused rather than
/// silently doubling the window's frame work.
#[test]
fn a_window_takes_one_pacer() {
    let window = create(false);
    let first = window.pacer().expect("a live window can be paced");
    assert!(window.pacer().is_err(), "the window took a second pacer");
    drop(first);
    window
        .pacer()
        .expect("the gate reopened when the pacer was dropped");
}
