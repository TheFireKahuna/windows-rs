//! Drives the pointer stack against a real window and reports what a run actually did.
//!
//! Four questions this answers that no document can:
//!
//! 1. **Which unit do the WinRT pointer statics answer in?** `PointerPoint.Position` is
//!    documented as "*client coordinates, in device-independent pixel*" and the statics
//!    "*always use the app context*" — but that is UWP's account of itself, and this is a
//!    per-monitor-v2 desktop window with no view and no rasterization scale. If the statics
//!    answer in client *pixels*, every gesture threshold is wrong by the display scale.
//!    [`Router::measured_unit`] reports what the first contact measured.
//! 2. **Is hover really frame-bounded?** The census separates hover hit tests from discrete
//!    ones. Sweep the pointer and watch `hover_hits` rise no faster than `ticks`.
//! 3. **Does a legacy mouse message ever arrive?** This handler counts every message it sees
//!    by number, so a legacy one shows up as a raw code with no name — which is the only way
//!    it *can* show up, since neither binding filter generates a constant to match on.
//! 4. **Which of the redacted exports does this build have?** `GetPointerTouchpadInfo` and
//!    `ReportWindowContentInertia` are resolved by name; the capability report says whether
//!    they resolved.
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

/// How much of the message stream is kept in order.
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
    // The first messages in the order they arrived. A tally says *whether* a legacy message
    // came; only an ordered trace says **when**, which is the difference between "the
    // pointer arms leak" and "the window was still being created".
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
                // visible here. This is how a legacy arrival would be caught: it has no name
                // in this crate, so it prints as a bare number.
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

    // The one authority, built by hand: this example has no scene, and the point is that the
    // router does not care where the array came from.
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

    // The frame clock is the only clock: everything below happens on `WM_FRAME` and nothing
    // else drives it.
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
                        // One per moved contact per frame; the pen detail rides here.
                        Report::Moved { .. } => {}
                        Report::Dragged { update, .. } => {
                            if update.decided {
                                println!("drag locked to {:?}", update.phase);
                            }
                        }
                        // Summarised rather than printed: a manipulation raises one of these per
                        // sample, and sixty lines of identical deltas hide the four events that
                        // matter.
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

    // The window's handler already forwards to the doorbell; this second one services the
    // frame message. Installed by re-entering the pump rather than by a second handler,
    // because `on_message` holds exactly one.
    let mut message = MSG::default();
    loop {
        // SAFETY: no window filter, and the destination is a stack local.
        let more = unsafe { GetMessageW(&mut message, core::ptr::null_mut(), 0, 0) };
        if more.0 <= 0 || quit.get() {
            break;
        }
        // SAFETY: `message` was just filled in by the call above.
        unsafe {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if message.message == windows_window::WM_FRAME {
            // Stated at every tick and never held: the window and its monitor own both
            // facts, and a router that cached them could be left holding a stale one.
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
    // The claim this separation exists for: ticks may exceed display frames — a press asks
    // to be serviced at once — but hover must not resolve more than once per frame.
    // Two counts, because they answer different questions. `hover_hits` is samples
    // *examined* — bounded by the pointer's own report rate, and deliberately not by the
    // frame clock, because a crossing between two samples is an event that sampling would
    // erase. `hover_changes` is crossings *published*, which is the one bounded by what a
    // user can see. A run where the first is not comfortably larger than the second is a run
    // where the platform coalesced above us and the batch carried one entry.
    println!(
        "display frames {}   ticks {}   samples examined {}   crossings {}   deepest batch {}",
        pacer.wake().frames(),
        router.census().ticks,
        router.census().hover_hits,
        router.census().hover_changes,
        router.census().deepest_batch
    );
    // Where each legacy arrival sits in the stream, and what the first pointer message's
    // index was. A count says whether; a position says *when*, which is the whole finding.
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
            // The smallest target declares none, so the platform's ~9 mm guidance applies to
            // it and to nothing else here.
            touch_inflate: if index == 3 {
                windows_scene::default_inflation(x1 - x0, y1 - y0)
            } else {
                0.0
            },
            clip_parent: NO_ENTRY,
            flags: HitFlags::INTERACTIVE | HitFlags::GESTURE,
            scroll_src: NodeId::NONE,
            id: target_id(index),
        })
        .collect()
}

/// The id the nth target is named by.
///
/// Minted rather than fabricated, because a `ControlId` is a generational index and there is
/// no way to make one but to ask an authority for it. Minting densely from a fresh one means
/// the nth is at slot n + 1, since every arena burns slot zero so that `NONE` names nothing.
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

/// Names the messages this stack expects. **Anything unnamed is the finding**: a legacy
/// mouse message has no constant in either binding filter, so it can only appear as a number.
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

/// Injects a hover sweep, a click and a drag, so the run answers its own questions with no
/// hand on the mouse.
///
/// **`SendInput`, never `SetCursorPos`.** A cursor warp moves the pointer and lets the window
/// manager notice; only the injected event goes through the input stack, and the same probe
/// reads "zero pointer messages, legacy only" under a warp — the right conclusion's exact
/// opposite, from an instrument that was never measuring the stack.
fn drive(origin: (i32, i32), scale: f32) {
    let px = |x: f32, y: f32| (origin.0 + (x * scale) as i32, origin.1 + (y * scale) as i32);
    let rest = |ms| std::thread::sleep(std::time::Duration::from_millis(ms));
    rest(600);

    // A sweep across all four targets, far faster than the frame rate. Hover must resolve
    // once per frame regardless of how many of these arrive.
    for step in 0..=60 {
        let (x, y) = px(20.0 + step as f32 * 7.5, 50.0);
        pump::move_to(x, y);
        rest(4);
    }

    // A tap that does not move: the recogniser's own `Tapped`, which is what a control
    // without a manipulation is waiting for.
    let (x, y) = px(80.0, 50.0);
    pump::move_to(x, y);
    rest(80);
    pump::button(true);
    rest(60);
    pump::button(false);
    rest(200);

    // A two-axis drag on a target that declares one. It crosses the threshold horizontally
    // first and then travels further vertically — the lock must stay where it landed.
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
    // life, so the drag must survive leaving — no cancel, no lost capture, and a release that
    // still names the target it began on.
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

/// The pump's own surface, plus the injector this example drives itself with.
///
/// Declared here rather than filtered into the crate: driving input is a harness concern, and
/// `windows-ui`'s generated surface is what the framework *reads*.
// The names are the platform's, so they are spelled the platform's way.
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

    /// A client point in screen pixels.
    pub fn client_origin(hwnd: *mut core::ffi::c_void) -> (i32, i32) {
        let mut point = [0, 0];
        // SAFETY: `hwnd` is live and the point is a stack local the call writes back through.
        unsafe {
            _ = ClientToScreen(hwnd, &mut point);
        }
        (point[0], point[1])
    }

    fn send(input: INPUT) {
        // SAFETY: one fully initialized record of exactly the size declared.
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

    /// Where the cursor actually is.
    pub fn cursor() -> (i32, i32) {
        let mut point = [0, 0];
        // SAFETY: the destination is a stack local the call writes back through.
        unsafe {
            _ = GetCursorPos(&mut point);
        }
        (point[0], point[1])
    }

    /// Absolute motion, corrected against where the cursor actually landed.
    ///
    /// The correction is not defensive padding: `SendInput`'s absolute coordinates are
    /// normalized over the virtual desktop and rounded to a 16-bit grid, which on a
    /// 7680-pixel-wide desktop quantizes to whole pixels only by luck — and a harness that
    /// aims at a 120-DIP target and lands two pixels outside it reports a stack bug that is
    /// its own. One feedback step removes the whole class.
    pub fn move_to(x: i32, y: i32) {
        absolute(x, y);
        // Corrected from where it landed, never from where it started: an aim taken from a
        // cursor that is still on the other monitor overshoots by the whole desktop, and the
        // path there crosses out of the window and back.
        for _ in 0..2 {
            let (ax, ay) = cursor();
            if (ax, ay) == (x, y) {
                return;
            }
            absolute(x + (x - ax), y + (y - ay));
        }
    }

    /// Absolute motion, normalized over the **virtual** desktop so a secondary monitor is
    /// reachable and the primary one is not silently assumed.
    fn absolute(x: i32, y: i32) {
        // SAFETY: each takes an index and returns a metric.
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
