//! The half of the pointer stack's contract that can be asserted, and it is the half that
//! regresses silently.
//!
//! Two seams, split by what a claim needs to be true:
//!
//! * **The doorbell's arm coverage.** `DefWindowProc` is what promotes pointer input into
//!   legacy mouse messages, so "no legacy mouse message is ever handled" reduces to "every
//!   pointer arm returns `Some`" — which needs no window, no pump and no input. That is
//!   asserted here, exhaustively, message by message.
//! * **The router's frame contract.** Ticking against a real window and a hand-built hit
//!   array proves the ordering, the census and the frame-clock guard. Real *input* against a
//!   real foreground window is what `examples/pointer` drives instead; a run that needs an
//!   injector is a harness, not a gate.
//!
//! The message numbers below are written out rather than imported, deliberately: this file
//! is checking that the crate's own arms cover them, and taking both sides from the same
//! constant would check nothing.

use std::rc::Rc;

use windows_color::{DisplayCapability, OutputTransform};
use windows_scene::{ControlId, Env, HitEntry, HitFlags, HitTable, NO_ENTRY, NodeId};
use windows_ui::gesture::GestureDecl;
use windows_ui::input::{Doorbell, Inertia, Late, Report, Router};
use windows_window::Window;

const WM_POINTERUPDATE: u32 = 0x0245;
const WM_POINTERDOWN: u32 = 0x0246;
const WM_POINTERUP: u32 = 0x0247;
const WM_POINTERENTER: u32 = 0x0249;
const WM_POINTERLEAVE: u32 = 0x024A;
const WM_POINTERCAPTURECHANGED: u32 = 0x024C;
const WM_POINTERWHEEL: u32 = 0x024E;
const WM_POINTERHWHEEL: u32 = 0x024F;
const WM_CAPTURECHANGED: u32 = 0x0215;

const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_CHAR: u32 = 0x0102;
const WM_KILLFOCUS: u32 = 0x0008;

/// Every legacy mouse message, by number. None of these has a constant in either binding
/// filter, which is the enforcement; this list exists so the *absence* of an arm for them is
/// asserted rather than assumed.
const LEGACY: [u32; 12] = [
    0x0200, // WM_MOUSEMOVE
    0x0201, 0x0202, // WM_LBUTTONDOWN / UP
    0x0204, 0x0205, // WM_RBUTTONDOWN / UP
    0x0207, 0x0208, // WM_MBUTTONDOWN / UP
    0x020A, 0x020E, // WM_MOUSEWHEEL / WM_MOUSEHWHEEL
    0x02A3, // WM_MOUSELEAVE
    0x00A0, // WM_NCMOUSEMOVE
    0x00A2, // WM_NCMOUSELEAVE
];

/// A `wParam` carrying a pointer id and the message's flag word.
fn wparam(id: u32, flags: u32) -> usize {
    (id as usize) | ((flags as usize) << 16)
}

// ── the doorbell ────────────────────────────────────────────────────────────────

#[test]
fn every_pointer_arm_is_handled_so_nothing_can_be_promoted() {
    let bell = Doorbell::new();
    for message in [
        WM_POINTERDOWN,
        WM_POINTERUP,
        WM_POINTERUPDATE,
        WM_POINTERENTER,
        WM_POINTERCAPTURECHANGED,
        WM_POINTERWHEEL,
        WM_POINTERHWHEEL,
        WM_CAPTURECHANGED,
    ] {
        assert_eq!(
            bell.wndproc(message, wparam(1, 0), 0),
            Some(0),
            "0x{message:04X} fell through to DefWindowProc, which promotes it to legacy mouse"
        );
    }
}

#[test]
fn a_legacy_mouse_message_reaches_no_arm_at_all() {
    // Not "is never observed" — the system's own cursor resynchronisation posts
    // `WM_MOUSEMOVE` after a capture change whatever is consumed. The achievable property,
    // and the one the cost argument rests on, is that none of them is *handled*.
    let bell = Doorbell::new();
    for message in LEGACY {
        assert_eq!(
            bell.wndproc(message, wparam(1, 0), 0),
            None,
            "0x{message:04X} was handled; the pointer path is supposed to have replaced it"
        );
    }
}

#[test]
fn leave_is_recorded_and_forwarded_because_the_caption_wants_it_too() {
    // The window procedure runs the application's handler before the caption's, so consuming
    // this would leave a window command lit after the pointer had gone. It carries no
    // position and starts no contact, so there is nothing in it for a promoted move to be
    // about.
    let bell = Doorbell::new();
    bell.wndproc(WM_POINTERENTER, wparam(1, 0x2000), 0);
    assert_eq!(bell.hovering(), Some(1));
    assert_eq!(
        bell.wndproc(WM_POINTERLEAVE, wparam(1, 0), 0),
        None,
        "leave was consumed, so a custom caption's hover can never clear"
    );
    assert_eq!(bell.hovering(), None);
}

#[test]
fn a_key_message_is_recorded_without_being_consumed() {
    // `WM_KEYDOWN` has to reach `TranslateMessage` for `WM_CHAR` to exist at all, and the
    // system commands on `WM_SYSKEY*` are the system's — so these are recorded and forwarded.
    let bell = Doorbell::new();
    for message in [WM_KEYDOWN, WM_KEYUP, WM_CHAR, WM_KILLFOCUS] {
        assert_eq!(bell.wndproc(message, 0x51, 0), None);
    }
    assert!(
        !bell.idle(),
        "four transitions arrived and nothing is pending"
    );
}

#[test]
fn the_ring_orders_a_keystroke_against_a_contact() {
    // One ring, because the order between them is observable: a `Tab` that moves focus and a
    // press that changes it have to resolve in the order the user made them.
    let bell = Doorbell::new();
    bell.wndproc(WM_KEYDOWN, 0x09, 0);
    bell.wndproc(WM_POINTERDOWN, wparam(1, 0), 0);
    bell.wndproc(WM_KEYUP, 0x09, 0);

    // All three in one ring: the depth is what says none was merged into another or routed
    // to a second queue. Their *order* within it is asserted where the ring is defined.
    assert_eq!(
        bell.health().peak,
        3,
        "a keystroke and a contact did not share one ring"
    );
}

#[test]
fn a_frame_is_requested_only_while_something_is_pending() {
    let bell = Doorbell::new();
    assert!(bell.idle());
    bell.wndproc(WM_POINTERUPDATE, wparam(1, 0), 0);
    assert!(!bell.idle(), "motion left nothing for the next tick to do");
}

/// Whether a frame message is waiting for this window, consuming it if so.
fn took_frame(hwnd: *mut std::ffi::c_void) -> bool {
    let mut message = [0u8; 48];
    // SAFETY: the buffer is at least `MSG`-sized and the filter names one message.
    unsafe {
        PeekMessageW(
            message.as_mut_ptr().cast(),
            hwnd,
            windows_window::WM_FRAME,
            windows_window::WM_FRAME,
            1,
        ) != 0
    }
}

/// Empties whatever creating the window queued.
fn drain_posted(hwnd: *mut std::ffi::c_void) {
    while took_frame(hwnd) {}
}

#[link(name = "user32")]
unsafe extern "system" {
    fn PeekMessageW(
        msg: *mut core::ffi::c_void,
        hwnd: *mut core::ffi::c_void,
        min: u32,
        max: u32,
        remove: u32,
    ) -> i32;
}

// ── the router ──────────────────────────────────────────────────────────────────

/// A window with the pointer opt-in, created hidden.
///
/// The opt-in is process-wide, one-way and rejected once the process owns a window, so every
/// window in this file asks for it and creation is serialized by the harness below.
fn window(title: &str) -> Window {
    Window::new(title)
        .size_dips(640.0, 400.0)
        .pointer_input()
        .hidden()
        .quit_on_close(false)
        .create()
        .expect("a hidden window can be created")
}

/// The environment a tick is stated with. Restated at every call, like everything else.
fn env() -> Env {
    Env::new(
        96.0,
        OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
    )
}

fn table() -> HitTable {
    let mut table = HitTable::default();
    table.replace(&[HitEntry {
        x0: 20.0,
        y0: 20.0,
        x1: 200.0,
        y1: 80.0,
        touch_inflate: 0.0,
        clip_parent: NO_ENTRY,
        flags: HitFlags::INTERACTIVE | HitFlags::GESTURE,
        scroll_src: NodeId::NONE,
        id: ControlId(7),
    }]);
    table
}

#[test]
fn a_tick_with_nothing_pending_reports_nothing_and_asks_for_no_frame() {
    let bell = Rc::new(Doorbell::new());
    let window = window("windows-ui — idle tick");
    let pacer = window.pacer().expect("a window can be paced");
    let wake = pacer.wake();
    let mut router = Router::new(&bell, &window, wake.clone()).expect("the window is open");
    router.declare(ControlId(7), GestureDecl::default());

    let hits = table();
    let mut reports = Vec::new();
    router
        .tick(&hits, env(), &mut reports)
        .expect("an empty tick");

    assert!(reports.is_empty(), "an idle tick reported {reports:?}");
    assert_eq!(router.census().ticks, 1);
    assert_eq!(
        router.census().hover_hits,
        0,
        "hover resolved with no pointer over the window"
    );
    assert_eq!(
        wake.requesters(),
        0,
        "an idle tick left a frame request outstanding, which is a window that never parks"
    );
}

#[test]
fn forgetting_a_target_aborts_whatever_was_bound_to_it() {
    let bell = Rc::new(Doorbell::new());
    let window = window("windows-ui — forget");
    let pacer = window.pacer().expect("a window can be paced");
    let mut router = Router::new(&bell, &window, pacer.wake()).expect("the window is open");
    router.declare(ControlId(7), GestureDecl::default());

    // Nothing is bound, so this is the no-op half — the assertion is that it stays a no-op
    // rather than counting an abort that did not happen.
    router.forget(ControlId(7));
    assert_eq!(router.census().aborts, 0);

    let hits = table();
    let mut reports = Vec::new();
    router
        .tick(&hits, env(), &mut reports)
        .expect("a tick after unmount");
    assert!(reports.is_empty());
}

#[test]
fn a_discrete_transition_asks_to_be_serviced_before_the_next_display_frame() {
    // **Frame-limiting a press is the opposite of a low-latency design.** A press does not
    // batch, is not a per-frame quantity, and the frame it would wait for is a frame of added
    // latency on the number users notice — so it posts the service message itself rather than
    // waiting for the pacer. Motion deliberately does not: it is coalesced into a bit and no
    // user can observe an intermediate hover state between two presents.
    let bell = Rc::new(Doorbell::new());
    let window = window("windows-ui — urgent");
    let pacer = window.pacer().expect("a window can be paced");
    let _router = Router::new(&bell, &window, pacer.wake()).expect("the window is open");
    drain_posted(window.hwnd());

    bell.wndproc(WM_POINTERUPDATE, wparam(1, 0x2000), 0);
    assert!(
        !took_frame(window.hwnd()),
        "motion asked for immediate service; it is a per-frame quantity"
    );

    bell.wndproc(WM_POINTERDOWN, wparam(1, 0x2000), 0);
    assert!(
        took_frame(window.hwnd()),
        "a press waited for the display, which is a frame of latency for nothing"
    );

    // Coalesced: a burst of contacts lifting together asks once, not once each.
    bell.wndproc(WM_POINTERUP, wparam(1, 0), 0);
    bell.wndproc(WM_POINTERUP, wparam(2, 0), 0);
    assert!(
        !took_frame(window.hwnd()),
        "the service request was not coalesced"
    );
}

#[test]
fn a_capture_change_that_takes_nothing_away_is_not_a_cancel() {
    // Releasing capture ourselves on an up posts this message back at us. Treating that as a
    // loss would abort the gesture that had just completed normally, so a change is a loss
    // only when something was actually held.
    let bell = Rc::new(Doorbell::new());
    let window = window("windows-ui — capture");
    let pacer = window.pacer().expect("a window can be paced");
    let mut router = Router::new(&bell, &window, pacer.wake()).expect("the window is open");

    bell.wndproc(WM_CAPTURECHANGED, 0, 0);
    let hits = table();
    let mut reports = Vec::new();
    router.tick(&hits, env(), &mut reports).expect("a tick");

    assert!(
        !reports.iter().any(|r| matches!(r, Report::CaptureLost)),
        "our own release was reported as a loss"
    );
    assert_eq!(router.census().aborts, 0);
}

/// Content inertia is recorded as **told**, never as asked.
///
/// A hidden window is not active, so `ReportWindowContentInertia` refuses it with
/// `E_ACCESSDENIED` — and a build without the export refuses it too. Either way nothing was
/// told, so nothing may be recorded.
///
/// It regressed by writing the state before the call and discarding the answer, which also
/// consumed the edge: the retry the next tick would have made never happened.
#[test]
fn a_refused_inertia_report_is_not_recorded_as_made() {
    let window = window("inertia");
    let inertia = Inertia::new(&window, Late::resolve());
    assert!(!inertia.reported());

    assert!(
        !inertia.set(true),
        "an inactive window cannot be reported for"
    );
    assert!(!inertia.reported(), "a refused report was recorded as made");
    // The edge survives, so the next tick tries again rather than seeing no change.
    assert!(!inertia.set(true));

    assert!(
        !inertia.set(false),
        "content that is not moving is not moving"
    );
}
