//! Drives the pointer stack against a real window and prints a census of the run.
//!
//! The run measures four properties of the stack:
//!
//! 1. The unit the WinRT pointer statics answer in. `PointerPoint.Position` is documented as
//!    client coordinates in device-independent pixels, for a UWP view; this is a
//!    per-monitor-v2 desktop window with no view and no rasterization scale, so the unit is
//!    measured from the first contact and reported by [`Router::measured_unit`]. If the
//!    statics answer in client pixels, every gesture threshold is off by the display scale.
//! 2. Whether hover is frame-bounded. The census counts hover hit tests separately from
//!    discrete ones, so a sweep raises `hover_hits` no faster than `ticks`.
//! 3. Whether a legacy mouse message arrives. The handler counts every message by number,
//!    and a legacy one prints as a bare code because neither binding filter generates a
//!    constant to match on.
//! 4. Which of the redacted exports this build resolved. `GetPointerTouchpadInfo` and
//!    `ReportWindowContentInertia` are resolved by name, and [`Router::capability`] reports
//!    which of them bound.
//!
//! ```text
//! cargo run -p windows-ui --example pointer
//! ```
//!
//! Move over the four targets, click them, drag across one, wheel over one, press `Tab` and
//! `Esc`. Press `Q` to quit; the summary prints on the way out.

use std::cell::RefCell;
use std::rc::Rc;

use windows_color::{DisplayCapability, OutputTransform};
use windows_scene::{ControlId, Env, HitEntry, HitFlags, HitTable, Ids, NO_ENTRY, NodeId};
use windows_ui::Result;
use windows_ui::gesture::{DragDecl, GestureDecl, Recognised};
use windows_ui::input::{Doorbell, Report, Router};
use windows_window::Window;

/// Bounds the number of messages kept in the ordered trace.
const TRACE_MAX: usize = 4000;

/// Four targets across the top of the window, in DIPs.
const TARGETS: [(&str, f32, f32, f32, f32); 4] = [
    ("tap", 20.0, 20.0, 140.0, 80.0),
    ("drag", 160.0, 20.0, 280.0, 80.0),
    ("wheel", 300.0, 20.0, 420.0, 80.0),
    ("tiny", 440.0, 20.0, 456.0, 36.0),
];

fn main() -> Result<()> {
    let bell = Rc::new(Doorbell::new());
    let seen: Rc<RefCell<Vec<(u32, u32)>>> = Rc::new(RefCell::new(Vec::new()));
    // The first messages in arrival order. The tally in `seen` says whether a legacy message
    // arrived; its index in this trace says when, which separates a leaking pointer arm from
    // a message that arrived while the window was still being created.
    let trace: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));

    let window = Window::new("windows-ui — pointer stack")
        .size_dips(720.0, 420.0)
        .pointer_input()
        .touchpad_capable()
        .on_message({
            let bell = Rc::clone(&bell);
            let seen = Rc::clone(&seen);
            let trace = Rc::clone(&trace);
            move |_, message, wparam, lparam| {
                // Counted before the doorbell, so a message the doorbell consumes is still
                // visible here. A legacy mouse message has no constant in this crate and so
                // prints as a bare number.
                {
                    let mut seen = seen.borrow_mut();
                    match seen.iter_mut().find(|(code, _)| *code == message) {
                        Some((_, count)) => *count += 1,
                        None => seen.push((message, 1)),
                    }
                }
                if trace.borrow().len() < TRACE_MAX {
                    trace.borrow_mut().push(message);
                }
                bell.wndproc(message, wparam, lparam)
            }
        })
        .create()?;

    let pacer = window.pacer()?;
    let mut router = Router::new(&bell, &window, pacer.wake())?;

    // The hit array, built by hand: this example has no scene, and the router reads the array
    // whatever produced it.
    let mut hits = HitTable::default();
    hits.replace(&targets());

    for (index, (name, ..)) in TARGETS.iter().enumerate() {
        let decl = match *name {
            "drag" => GestureDecl::default().with_drag(DragDecl::reorder()),
            "wheel" => GestureDecl::slider(false),
            _ => GestureDecl::default(),
        };
        router.declare(target_id(index), decl);
    }

    println!("{:#?}", router.capability());
    println!(
        "dial: {}",
        match router.attach_rotary(&window) {
            Ok(true) => "attached",
            Ok(false) => "none attached",
            Err(_) => "refused",
        }
    );
    println!(
        "scale {:.2}   targets {:?}",
        window.scale().unwrap_or(1.0),
        TARGETS.map(|(name, ..)| name)
    );
    if std::env::args().any(|arg| arg == "--drive") {
        let origin = pump::client_origin(window.hwnd());
        let scale = window.scale().unwrap_or(1.0);
        println!("driving from client origin {origin:?}\n");
        std::thread::spawn(move || drive(origin, scale));
    } else {
        println!("move, click, drag, wheel, Tab, Esc … then Q to finish\n");
    }

    let mut reports = Vec::new();
    let quit = Rc::new(std::cell::Cell::new(false));

    // The tick below runs on `WM_FRAME`; nothing else drives it.
    let tick = {
        let quit = Rc::clone(&quit);
        let mut last: Option<(&'static str, windows_scene::Point)> = None;
        RefCell::new(
            move |router: &mut Router, hits: &HitTable, env: Env| -> Result<()> {
                reports.clear();
                router.tick(hits, env, &mut reports)?;
                for report in &reports {
                    match report {
                        Report::Key { event, .. } if event.key == b'Q' as u16 => quit.set(true),
                        Report::HoverChanged { from, to, .. } => {
                            println!("hover {} → {}", label(*from), label(*to));
                        }
                        Report::Pressed { target, sample, .. } => {
                            println!(
                                "press {} ({:?}{})",
                                label(Some(*target)),
                                sample.ptype,
                                match sample.pen {
                                    Some(pen) => format!(", pressure {:.2}", pen.pressure),
                                    None => String::new(),
                                }
                            );
                        }
                        // One per moved contact per frame; carries the pen detail.
                        Report::Moved { .. } => {}
                        Report::Dragged { update, .. } => {
                            if update.decided {
                                println!("drag locked to {:?}", update.phase);
                            }
                        }
                        // Summarised rather than printed: a manipulation raises one report per
                        // sample, so only the last cumulative translation is kept and printed
                        // when the manipulation completes.
                        Report::Gesture { target, event, .. } => match event {
                            Recognised::ManipulationUpdated { cumulative, .. } => {
                                last = Some((label(Some(*target)), cumulative.translation));
                            }
                            other => println!("gesture {} {other:?}", label(Some(*target))),
                        },
                        Report::Wheel {
                            target, notches, ..
                        } => println!("wheel {} {notches:+.1}", label(*target)),
                        Report::FocusChanged { to, .. } => println!("focus → {}", label(*to)),
                        other => println!("{other:?}"),
                    }
                }
                if let Some((target, travel)) = last.take()
                    && reports.iter().any(|report| {
                        matches!(
                            report,
                            Report::Gesture {
                                event: Recognised::ManipulationCompleted { .. },
                                ..
                            }
                        )
                    })
                {
                    println!(
                        "  … manipulation on {target} travelled {:.2} DIPs",
                        travel.x
                    );
                }
                Ok(())
            },
        )
    };

    // The window's handler forwards to the doorbell. The frame message is serviced by this
    // pump rather than by a second handler, because `on_message` holds exactly one.
    let mut message = MSG::default();
    loop {
        // SAFETY: `message` is a stack local valid for writes for the whole call, and a null
        // window handle is the documented request for every message on this thread.
        let more = unsafe { GetMessageW(&mut message, core::ptr::null_mut(), 0, 0) };
        if more.0 <= 0 || quit.get() {
            break;
        }
        // SAFETY: `GetMessageW` returned a positive result above, so `message` is fully
        // initialized, and both calls take it by shared reference.
        unsafe {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if message.message == windows_window::WM_FRAME {
            // Rebuilt each tick: DPI and colour capability belong to the monitor the window
            // is on, and a copy held across a monitor change would be stale.
            let env = Env::new(
                window.metrics().map_or(96.0, |m| m.dpi as f32),
                OutputTransform::for_display(
                    window.color_capability().unwrap_or(DisplayCapability::Sdr),
                    203.0,
                ),
            );
            (tick.borrow_mut())(&mut router, &hits, env)?;
        }
    }

    println!("\n── what the run did ──");
    println!("{:#?}", router.census());
    println!("measured unit: {:?}", router.measured_unit());
    println!("doorbell: {:?}", bell.health());
    println!("pacer: {:?}", pacer.health());
    // Ticks may exceed display frames, because a press asks to be serviced at once; hover
    // resolves at most once per frame. `hover_hits` counts samples examined, bounded by the
    // pointer's own report rate rather than by the frame clock, so a crossing that falls
    // between two samples is not erased. `hover_changes` counts crossings published, which is
    // the count bounded by the frame clock. `hover_hits` no larger than `hover_changes` means
    // the platform coalesced above this stack and each batch carried one entry.
    println!(
        "display frames {}   ticks {}   samples examined {}   crossings {}   deepest batch {}",
        pacer.wake().frames(),
        router.census().ticks,
        router.census().hover_hits,
        router.census().hover_changes,
        router.census().deepest_batch
    );
    // The index of each legacy arrival in the stream, and of the first pointer message. An
    // index separates a leaking arm from a message that arrived before the pointer path was
    // in place.
    let trace = trace.borrow();
    let first_pointer = trace.iter().position(|code| *code == 0x0245);
    let legacy: Vec<usize> = trace
        .iter()
        .enumerate()
        .filter(|(_, code)| **code == 0x0200 || **code == 0x00A0)
        .map(|(index, _)| index)
        .collect();
    println!(
        "first WM_POINTERUPDATE at index {first_pointer:?}; legacy mouse at {legacy:?} of {} messages",
        trace.len()
    );
    for at in &legacy {
        let from = at.saturating_sub(3);
        let to = (at + 4).min(trace.len());
        let around: Vec<String> = trace[from..to].iter().map(|c| name_of(*c)).collect();
        println!("  around {at}: {around:?}");
    }
    println!("messages seen, by code:");
    for (code, count) in seen.borrow().iter() {
        println!("  {}  ×{count}", name_of(*code));
    }
    Ok(())
}

fn targets() -> Vec<HitEntry> {
    TARGETS
        .iter()
        .enumerate()
        .map(|(index, (_, x0, y0, x1, y1))| HitEntry {
            x0: *x0,
            y0: *y0,
            x1: *x1,
            y1: *y1,
            // Only the smallest target takes the platform's default touch inflation; the
            // other three are hit at their declared bounds.
            touch_inflate: if index == 3 {
                windows_scene::default_inflation(x1 - x0, y1 - y0)
            } else {
                0.0
            },
            clip_parent: NO_ENTRY,
            parent: NO_ENTRY,
            flags: HitFlags::INTERACTIVE | HitFlags::GESTURE,
            scroll_src: NodeId::NONE,
            id: target_id(index),
        })
        .collect()
}

/// Returns the [`ControlId`] naming the target at `index`.
///
/// A `ControlId` is a generational index, so it is minted from an [`Ids`] authority rather
/// than written out. Minting densely from a fresh authority puts the nth target at slot
/// n + 1, because slot zero is reserved so that `NONE` names no control.
fn target_id(index: usize) -> ControlId {
    let mut ids = Ids::<windows_scene::Control>::new();
    let mut id = ids.mint();
    for _ in 0..index {
        id = ids.mint();
    }
    id
}

fn label(id: Option<ControlId>) -> &'static str {
    match id {
        Some(id) => TARGETS
            .get(id.index().wrapping_sub(1))
            .map_or("?", |(name, ..)| *name),
        None => "—",
    }
}

/// Returns a name for `code`, or its hexadecimal value when the message is not in the table.
///
/// The two legacy mouse messages are named out here because neither binding filter generates
/// a constant for them, so nothing else in the run would identify them.
fn name_of(code: u32) -> String {
    let named = [
        (0x0245, "WM_POINTERUPDATE"),
        (0x0246, "WM_POINTERDOWN"),
        (0x0247, "WM_POINTERUP"),
        (0x0249, "WM_POINTERENTER"),
        (0x024A, "WM_POINTERLEAVE"),
        (0x024C, "WM_POINTERCAPTURECHANGED"),
        (0x024E, "WM_POINTERWHEEL"),
        (0x024F, "WM_POINTERHWHEEL"),
        (0x0100, "WM_KEYDOWN"),
        (0x0101, "WM_KEYUP"),
        (0x0102, "WM_CHAR"),
        (0x0007, "WM_SETFOCUS"),
        (0x0008, "WM_KILLFOCUS"),
        (0x0005, "WM_SIZE"),
        (0x0084, "WM_NCHITTEST"),
        (0x0014, "WM_ERASEBKGND"),
        (0x0444, "WM_FRAME"),
        (0x0200, "WM_MOUSEMOVE — LEGACY"),
        (0x0215, "WM_CAPTURECHANGED"),
        (0x0020, "WM_SETCURSOR"),
        (0x00A0, "WM_NCMOUSEMOVE — LEGACY"),
    ];
    match named.iter().find(|(value, _)| *value == code) {
        Some((_, name)) => (*name).to_string(),
        None => format!("0x{code:04X}"),
    }
}

/// Injects a hover sweep, a tap, a drag, a manipulation, a wheel notch and `Q`, so the run
/// completes with no hand on the mouse.
///
/// Every motion goes through `SendInput`. `SetCursorPos` warps the cursor and leaves the
/// window manager to notice, which yields legacy mouse messages and no pointer messages, so
/// it measures nothing about the stack under test.
fn drive(origin: (i32, i32), scale: f32) {
    let px = |x: f32, y: f32| (origin.0 + (x * scale) as i32, origin.1 + (y * scale) as i32);
    let rest = |ms| std::thread::sleep(std::time::Duration::from_millis(ms));
    rest(600);

    // A sweep across all four targets, far faster than the frame rate. Hover resolves at most
    // once per frame however many of these arrive.
    for step in 0..=60 {
        let (x, y) = px(20.0 + step as f32 * 7.5, 50.0);
        pump::move_to(x, y);
        rest(4);
    }

    // A tap with no motion, which the recogniser reports as `Tapped` — the report a control
    // that declares no manipulation acts on.
    let (x, y) = px(80.0, 50.0);
    pump::move_to(x, y);
    rest(80);
    pump::button(true);
    rest(60);
    pump::button(false);
    rest(200);

    // A two-axis drag on a target that declares one. It crosses the threshold horizontally
    // and then travels further vertically; the axis lock stays where the crossing put it.
    let (x, y) = px(200.0, 50.0);
    pump::move_to(x, y);
    rest(80);
    pump::button(true);
    rest(40);
    for step in 1..=20 {
        let (x, y) = px(200.0 + step as f32 * 4.0, 50.0 + step as f32 * 3.0);
        pump::move_to(x, y);
        rest(8);
    }
    // Out of the window entirely and back. A contact routes to its down-window for its whole
    // life, so leaving cancels nothing, loses no capture, and the release still names the
    // target the drag began on.
    let (x, y) = px(-400.0, -300.0);
    pump::move_to(x, y);
    rest(60);
    let (x, y) = px(280.0, 110.0);
    pump::move_to(x, y);
    rest(60);
    pump::button(false);
    rest(200);

    // A manipulation on a target that declares translation, so the recogniser reports
    // started / updated / completed rather than a tap.
    let (x, y) = px(320.0, 50.0);
    pump::move_to(x, y);
    rest(80);
    pump::button(true);
    rest(40);
    for step in 1..=20 {
        let (x, y) = px(320.0 + step as f32 * 4.0, 50.0);
        pump::move_to(x, y);
        rest(8);
    }
    pump::button(false);
    rest(200);

    pump::wheel(-2);
    rest(300);
    pump::key(b'Q' as u16);
}

/// Declares the message-pump entry points and the input injector this example drives itself
/// with.
///
/// These live here rather than in the crate's generated bindings, which cover the input the
/// framework reads rather than the input a harness writes.
// The names, field names and layouts are the platform's.
#[expect(non_snake_case, clippy::upper_case_acronyms)]
mod pump {
    windows_core::link!("user32.dll" "system" fn GetMessageW(lpmsg: *mut MSG, hwnd: *mut core::ffi::c_void, wmsgfiltermin: u32, wmsgfiltermax: u32) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn TranslateMessage(lpmsg: *const MSG) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn DispatchMessageW(lpmsg: *const MSG) -> isize);
    windows_core::link!("user32.dll" "system" fn ClientToScreen(hwnd: *mut core::ffi::c_void, lppoint: *mut [i32; 2]) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn SendInput(cinputs: u32, pinputs: *const INPUT, cbsize: i32) -> u32);
    windows_core::link!("user32.dll" "system" fn GetSystemMetrics(nindex: i32) -> i32);
    windows_core::link!("user32.dll" "system" fn GetCursorPos(lppoint: *mut [i32; 2]) -> windows_core::BOOL);

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct MSG {
        pub hwnd: *mut core::ffi::c_void,
        pub message: u32,
        pub wParam: usize,
        pub lParam: isize,
        pub time: u32,
        pub pt: [i32; 2],
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct MOUSEINPUT {
        pub dx: i32,
        pub dy: i32,
        pub mouseData: u32,
        pub dwFlags: u32,
        pub time: u32,
        pub dwExtraInfo: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct KEYBDINPUT {
        pub wVk: u16,
        pub wScan: u16,
        pub dwFlags: u32,
        pub time: u32,
        pub dwExtraInfo: usize,
        pub _pad: [u32; 2],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union INPUT_0 {
        pub mi: MOUSEINPUT,
        pub ki: KEYBDINPUT,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct INPUT {
        pub r#type: u32,
        pub anonymous: INPUT_0,
    }

    const INPUT_MOUSE: u32 = 0;
    const INPUT_KEYBOARD: u32 = 1;
    const MOUSEEVENTF_MOVE: u32 = 0x0001;
    const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
    const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
    const MOUSEEVENTF_WHEEL: u32 = 0x0800;
    const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
    const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const SM_XVIRTUALSCREEN: i32 = 76;
    const SM_YVIRTUALSCREEN: i32 = 77;
    const SM_CXVIRTUALSCREEN: i32 = 78;
    const SM_CYVIRTUALSCREEN: i32 = 79;

    /// Returns the client area's top-left corner, in screen pixels.
    pub fn client_origin(hwnd: *mut core::ffi::c_void) -> (i32, i32) {
        let mut point = [0, 0];
        // SAFETY: `hwnd` names a window this process owns and has not destroyed, and `point`
        // is a stack local valid for writes for the whole call.
        unsafe {
            _ = ClientToScreen(hwnd, &mut point);
        }
        (point[0], point[1])
    }

    fn send(input: INPUT) {
        // SAFETY: `input` is one fully initialized record, and `cbsize` is its exact size, so
        // the count and stride the call reads with match the allocation.
        unsafe {
            SendInput(1, &input, size_of::<INPUT>() as i32);
        }
    }

    fn mouse(flags: u32, dx: i32, dy: i32, data: u32) {
        send(INPUT {
            r#type: INPUT_MOUSE,
            anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        });
    }

    /// Returns the cursor position, in screen pixels.
    pub fn cursor() -> (i32, i32) {
        let mut point = [0, 0];
        // SAFETY: `point` is a stack local valid for writes for the whole call.
        unsafe {
            _ = GetCursorPos(&mut point);
        }
        (point[0], point[1])
    }

    /// Moves the cursor to `(x, y)` in screen pixels, correcting against where it landed.
    ///
    /// `SendInput`'s absolute coordinates are normalized over the virtual desktop onto a
    /// 16-bit grid, so a requested pixel is reachable in one step only when the desktop's
    /// width divides that grid evenly. Up to two feedback steps land the cursor on the
    /// requested pixel, which keeps a miss of a few pixels from reading as a hit-test fault.
    pub fn move_to(x: i32, y: i32) {
        absolute(x, y);
        // The correction is measured from where the cursor landed, not from where it started:
        // an offset taken against a cursor still on another monitor overshoots by the width
        // of the desktop.
        for _ in 0..2 {
            let (ax, ay) = cursor();
            if (ax, ay) == (x, y) {
                return;
            }
            absolute(x + (x - ax), y + (y - ay));
        }
    }

    /// Injects absolute motion to `(x, y)` in screen pixels, normalized over the virtual
    /// desktop so that points on a secondary monitor are reachable.
    fn absolute(x: i32, y: i32) {
        // SAFETY: `GetSystemMetrics` takes an index by value and returns a metric; no pointer
        // crosses the boundary.
        let (left, top, width, height) = unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1),
                GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1),
            )
        };
        let nx = ((x - left) as i64 * 65535 / width as i64) as i32;
        let ny = ((y - top) as i64 * 65535 / height as i64) as i32;
        mouse(
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            nx,
            ny,
            0,
        );
    }

    pub fn button(down: bool) {
        mouse(
            if down {
                MOUSEEVENTF_LEFTDOWN
            } else {
                MOUSEEVENTF_LEFTUP
            },
            0,
            0,
            0,
        );
    }

    pub fn wheel(notches: i32) {
        mouse(MOUSEEVENTF_WHEEL, 0, 0, (notches * 120) as u32);
    }

    pub fn key(vk: u16) {
        for flags in [0, KEYEVENTF_KEYUP] {
            send(INPUT {
                r#type: INPUT_KEYBOARD,
                anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        dwFlags: flags,
                        ..Default::default()
                    },
                },
            });
        }
    }
}

use pump::{DispatchMessageW, GetMessageW, MSG, TranslateMessage};
