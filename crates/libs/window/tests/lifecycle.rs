//! Tests what a window's creation and destruction do to the thread that owns it: its
//! handlers, its message queue, whether anything it draws can be seen, and how many pacers it
//! grants.
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

/// Creates a hidden window, so nothing appears and no test needs the foreground.
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
    // SAFETY: `IsWindow` accepts any handle value, including one that names no window, and
    // reads no memory through it.
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
    // SAFETY: `IsWindow` accepts any handle value, including this destroyed one, and reads
    // no memory through it.
    assert_eq!(unsafe { IsWindow(hwnd) }, 0);
}

/// Answers nothing from a `Window` whose window was closed behind its back.
///
/// A handle value can outlive the window it named, and a `Window` value outlives its window
/// whenever something else closed it. Every accessor answers from identity rather than from
/// liveness, so a stale value stays stale however the handle table is reused afterwards.
#[test]
fn a_closed_window_leaves_a_value_that_claims_nothing() {
    let window = create(false);
    let hwnd = window.hwnd();
    // Closes the window without going through the `Window` value, the way the user's close
    // button does.
    // SAFETY: `SendMessageW` accepts any handle value, and `WM_CLOSE` carries no pointer in
    // either parameter. The window belongs to this thread, so the call runs its window
    // procedure inline.
    unsafe {
        SendMessageW(hwnd, /* WM_CLOSE */ 0x0010, 0, 0)
    };

    assert!(!window.is_open());
    assert!(window.client_size().is_none());
    assert!(window.metrics().is_none());
    assert!(window.visibility().is_none());
    assert!(window.show().is_none());

    // Churns the handle table and the state allocator, so a reuse of the closed window's
    // handle value or state box has a chance to be mistaken for it.
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

    // SAFETY: `SendMessageW` accepts any handle value, and `WM_USER` carries no pointer in
    // either parameter. The window belongs to this thread, so the call runs its window
    // procedure inline.
    unsafe { SendMessageW(window.hwnd(), WM_USER, 0, 0) };
    assert_eq!(messages.get(), 1);

    // SAFETY: `SendMessageW` accepts any handle value, and `WM_SIZE` packs the client size
    // into `lparam` rather than passing a pointer. The window belongs to this thread, so
    // the call runs its window procedure inline.
    unsafe { SendMessageW(window.hwnd(), WM_SIZE, 0, 640 | (480 << 16)) };
    assert_eq!(sizes.get(), (640, 480));
}

/// Reports a window built hidden as hidden until [`Window::show`] runs.
///
/// A composition host creates its window hidden and shows it once there is something to
/// draw. Nothing it draws can be seen until then, and a producer parked on the visibility is
/// what keeps a frame from being drawn into a window the user has never seen.
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

/// Mints a separate visibility watch per caller, each of which sees the window come up.
///
/// A pacer and a present thread park on the same window, and one shared wake would leave
/// one of them parked.
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

/// Leaves the pump running when a window that opted out of `quit_on_close` is destroyed.
///
/// An application with more than one window, or one that outlives its window, opts out.
#[test]
fn a_window_that_opts_out_leaves_the_pump_running() {
    let window = create(false);
    drop(window);
    assert!(pump(), "destroying the window quit a pump that opted out");
    assert!(pump(), "and it is still running");
}

/// Reports the quit from every [`pump`] once one has been posted.
///
/// Peeking removes the quit message from the queue, so [`pump`] puts it back and a caller
/// that checks the answer on only some iterations of its loop still sees it.
#[test]
fn the_quit_answer_is_sticky() {
    quit();
    assert!(!pump(), "the quit was not seen");
    assert!(!pump(), "the quit was seen once and then forgotten");
}

/// Grants one pacer at a time, refusing a second until the first is dropped.
///
/// Two pacers would post frames past each other's gate and double the window's frame work.
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
