//! Drives every stream against a real window and reports what arrived.
//!
//! Four questions, each answered by observation:
//!
//! 1. Does each stream arrive as pointer input, and as the right device? The handler reads
//!    the pointer behind every pointer message and tallies by type, so `PT_PEN` from the pen
//!    stream is observed rather than assumed. A call that returns `TRUE` and delivers nothing
//!    is the shape of every mistake here, so a return code is not evidence.
//! 2. Does any legacy mouse message arrive? Every message is counted by number, so a legacy
//!    one shows up as a raw code — the only way it can, because this crate's binding filter
//!    generates no constant to name it with.
//! 3. Is the mouse placed exactly? The run reports the calibration verdict and injects a drag
//!    of known total travel, so the window's arrival positions can be compared against it.
//! 4. Which of the redacted exports does this build have? [`injector::Capability`] says.
//!
//! ```text
//! cargo run -p injector --example streams
//! cargo run -p injector --example streams -- --inertia
//! ```
//!
//! `--inertia` runs a different arm. `TA_INERTIA_STOP` and `TA_INERTIA_END` each produce one
//! `WM_STOPINERTIA` and one `WM_ENDINERTIA`, to the window that last reported content
//! inertia. Those two message numbers are redacted from the platform floor's SDK, and unlike
//! an export a number cannot be resolved by name, so this arm injects each action and reports
//! whatever unnamed message arrives.
//!
//! Read off 26200: `WM_STOPINERTIA` is `0x023B` and `WM_ENDINERTIA` is `0x023C`, one of each,
//! immediately after its action. Two prerequisites: the window must be active, or
//! `ReportWindowContentInertia` answers `E_ACCESSDENIED` through a `BOOL` return; and the
//! report must be re-asserted before each action, because the system tracks one window and
//! the stop consumes the tracking.

use std::cell::RefCell;
use std::rc::Rc;

use injector::{Button, Injector, Key, Point, Rate, Space, TouchpadAction, line, zigzag};
use windows_window::Window;

/// A pointer message and the device behind it.
struct Arrival {
    message: u32,
    ptype: u32,
    at: (i32, i32),
    /// Every sample the message carried, oldest first, in raw screen pixels.
    ///
    /// A message can carry a frame of samples rather than one, so counting messages measures
    /// the pump while reading the history measures the input. An integral over message
    /// positions is a lower bound rather than the path.
    samples: Vec<(i32, i32)>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arrivals: Rc<RefCell<Vec<Arrival>>> = Rc::new(RefCell::new(Vec::new()));
    let counts: Rc<RefCell<Vec<(u32, u32)>>> = Rc::new(RefCell::new(Vec::new()));

    let window = Window::new("injector — streams")
        .size_dips(720.0, 420.0)
        .pointer_input()
        .touchpad_capable()
        .on_message({
            let arrivals = Rc::clone(&arrivals);
            let counts = Rc::clone(&counts);
            move |_, message, wparam, _| {
                {
                    let mut counts = counts.borrow_mut();
                    match counts.iter_mut().find(|(code, _)| *code == message) {
                        Some((_, count)) => *count += 1,
                        None => counts.push((message, 1)),
                    }
                }
                if !observe::is_pointer(message) {
                    return None;
                }
                let id = (wparam & 0xFFFF) as u32;
                let (ptype, at, samples) = observe::pointer(id);
                arrivals.borrow_mut().push(Arrival {
                    message,
                    ptype,
                    at,
                    samples,
                });
                // Consumed: `DefWindowProc` promotes a pointer message into a legacy mouse
                // one, so falling through here would manufacture the legacy messages this
                // run counts.
                Some(0)
            }
        })
        .create()
        .map_err(|error| format!("creating the window: {error}"))?;
    _ = window.show();
    // Injected absolute input lands on whatever window is at that screen point, which is a
    // z-order question rather than a focus one. Without this, a run under a terminal that
    // overlaps the target reads zero arrivals and looks like a stack that delivered nothing.
    observe::topmost(window.hwnd());
    settle(&window, 400);

    let mut injector = Injector::for_window(window.hwnd())?;
    // Read once and passed down: the window does not move during a run, and asking the
    // injector for it while a stream is live would borrow the injector twice.
    let space = injector.space();
    println!("{:#?}", injector.capability());
    println!("space {:?} scale {:.2}", space.origin_px(), space.scale());

    if std::env::args().any(|arg| arg == "--inertia") {
        return inertia(&window, &mut injector, &counts);
    }

    // ── Mouse ─────────────────────────────────────────────────────────────────
    // Opened first, so its calibration happens before anything is counted.
    let mut mouse = injector.mouse()?;
    println!("\nmouse: calibration held");
    mouse.move_to((100.0, 60.0))?;
    drop(mouse);
    settle(&window, 200);
    arrivals.borrow_mut().clear();

    // A drag of known total travel and nothing else in this arm, so the integral picks up
    // only what the drive put there. A zigzag rather than a line: the length of a polyline
    // through collinear points does not change when one is removed, so a straight drive
    // reports its full length however many samples were dropped. Every point here is a
    // corner.
    let path = zigzag((100.0, 60.0), (400.0, 60.0), 40, 6.0);
    let asked = space.placed_length(&whole(Point::new(100.0, 60.0), &path));
    let mut mouse = injector.mouse()?;
    mouse
        .down((100.0, 60.0))?
        .polyline(&path, Rate::PerMs(2))?
        .up()?;
    drop(mouse);
    settle(&window, 300);
    report("mouse drag", &arrivals, Some(asked), space);

    let mut mouse = injector.mouse()?;
    mouse
        .tap((200.0, 200.0))?
        .press(Button::Secondary)?
        .release(Button::Secondary)?
        .wheel(-2.0)?;
    drop(mouse);
    settle(&window, 300);
    report("mouse buttons", &arrivals, None, space);

    // ── Touch ─────────────────────────────────────────────────────────────────
    let mut touch = injector.touch(2)?;
    touch.down((150.0, 120.0))?;
    drop(touch);
    settle(&window, 200);
    arrivals.borrow_mut().clear();

    let path = zigzag((150.0, 120.0), (150.0, 300.0), 30, 6.0);
    let asked = space.placed_length(&whole(Point::new(150.0, 120.0), &path));
    let mut touch = injector.touch(2)?;
    touch.polyline(&path, Rate::PerMs(2))?.up()?;
    drop(touch);
    settle(&window, 300);
    report("touch drag", &arrivals, Some(asked), space);

    let mut touch = injector.touch(2)?;
    touch
        .tap((150.0, 120.0))?
        .down((150.0, 120.0))?
        .cancel()?
        .pinch((300.0, 200.0), 40.0, 160.0, Rate::PerFrame)?
        .lift(0)?
        .lift(1)?
        .frame()?;
    drop(touch);
    settle(&window, 300);
    report("touch gestures", &arrivals, None, space);

    // ── Pen ───────────────────────────────────────────────────────────────────
    // A pen needs a virtual device, so an unpackaged process is refused rather than run:
    // every pen call there returns success and delivers nothing.
    match injector.pen() {
        Ok(mut pen) => {
            pen.pressure(0.8)
                .tilt(20, -10)
                .hover_to((250.0, 150.0))?
                .polyline(&line((250.0, 150.0), (350.0, 150.0), 10), Rate::PerMs(4))?
                .down((350.0, 150.0))?
                .polyline(&line((350.0, 150.0), (450.0, 150.0), 20), Rate::PerMs(4))?
                .up()?
                .leave()?;
            drop(pen);
            settle(&window, 300);
            report("pen", &arrivals, None, space);
        }
        Err(error) => println!("\npen: unavailable — {error}"),
    }

    // ── Precision touchpad ────────────────────────────────────────────────────
    // Touchpad contacts need a virtual device and are refused unpackaged; its actions do
    // not, and the `--inertia` arm drives those.
    match injector.touchpad() {
        Ok(mut pad) => {
            pad.pan(2, (0.3, 0.5), (0.7, 0.5), 24, Rate::PerFrame)?
                .action(TouchpadAction::ThreeFingerTap)?;
            drop(pad);
            settle(&window, 300);
            report("touchpad", &arrivals, None, space);
        }
        Err(error) => println!("\ntouchpad: unavailable — {error}"),
    }

    // ── Keys ──────────────────────────────────────────────────────────────────
    injector.key(Key::Tab)?.key(Key::Escape)?;
    settle(&window, 200);

    println!("\n── every message, by code ──");
    for (code, count) in counts.borrow().iter() {
        println!("  {}  ×{count}", observe::name(*code));
    }
    let legacy: u32 = counts
        .borrow()
        .iter()
        .filter(|(code, _)| observe::is_legacy(*code))
        .map(|(_, count)| count)
        .sum();
    println!(
        "\nlegacy mouse messages: {legacy} — every one of them unhandled, because no arm \
         exists to handle one"
    );
    Ok(())
}

/// Reads the two redacted inertia message numbers off the running system.
fn inertia(
    window: &Window,
    injector: &mut Injector,
    counts: &Rc<RefCell<Vec<(u32, u32)>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("touchpad-capable: {}", window.is_touchpad_capable());
    // Activate the window before asking the system to track it: `ReportWindowContentInertia`
    // refuses a background window, which is a different finding from refusing outright.
    {
        let mut mouse = injector.mouse()?;
        mouse.tap((300.0, 200.0))?;
    }
    settle(window, 200);
    if let Err(why) = observe::report_inertia(window.hwnd(), true) {
        println!("ReportWindowContentInertia refused: {why}");
        println!("nothing tracks this window's inertia, so the two actions have nowhere to land");
        return Ok(());
    }
    let before: Vec<(u32, u32)> = counts.borrow().clone();
    let mut pad = injector.touchpad()?;
    for action in [TouchpadAction::InertiaStop, TouchpadAction::InertiaEnd] {
        // Re-asserted before each action: the system tracks one window's content inertia and
        // the stop consumes that tracking, so a report made once leaves the second action
        // nowhere to land.
        if let Err(why) = observe::report_inertia(window.hwnd(), true) {
            println!("re-reporting inertia refused: {why}");
            break;
        }
        let mark: Vec<(u32, u32)> = counts.borrow().clone();
        pad.action(action)?;
        drop(pad);
        settle(window, 300);
        println!("\nafter {action:?}:");
        for (code, count) in counts.borrow().iter() {
            let was = mark
                .iter()
                .find(|(seen, _)| seen == code)
                .map_or(0, |(_, count)| *count);
            if *count > was {
                println!("  {} ×{}", observe::name(*code), count - was);
            }
        }
        pad = injector.touchpad()?;
    }
    drop(pad);
    _ = observe::report_inertia(window.hwnd(), false);
    println!(
        "\n{} message codes were already present before the run",
        before.len()
    );
    Ok(())
}

/// The whole path the window will see: where the contact starts, then every sample.
fn whole(start: Point, path: &[Point]) -> Vec<Point> {
    let mut all = Vec::with_capacity(path.len() + 1);
    all.push(start);
    all.extend_from_slice(path);
    all
}

/// Pumps for roughly `ms`, so the window sees what was injected before it is asked about it.
fn settle(window: &Window, ms: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < deadline && window.is_open() {
        windows_window::pump();
        std::thread::yield_now();
    }
}

/// Drains what arrived and prints what it was.
///
/// `travel` is what the drive asked for, and it is compared against the path integral over
/// the arrivals — the sum of the segments, not the distance from the first to the last. A
/// drag that goes out and comes back has an integral and no displacement, and a dropped
/// sample shortens the integral.
fn report(what: &str, arrivals: &Rc<RefCell<Vec<Arrival>>>, travel: Option<f32>, space: Space) {
    let arrivals = arrivals.borrow_mut().drain(..).collect::<Vec<_>>();
    let mut by_type: Vec<(u32, u32)> = Vec::new();
    let mut by_message: Vec<(u32, u32)> = Vec::new();
    let mut samples = 0;
    for arrival in &arrivals {
        match by_type.iter_mut().find(|(kind, _)| *kind == arrival.ptype) {
            Some((_, count)) => *count += 1,
            None => by_type.push((arrival.ptype, 1)),
        }
        match by_message
            .iter_mut()
            .find(|(code, _)| *code == arrival.message)
        {
            Some((_, count)) => *count += 1,
            None => by_message.push((arrival.message, 1)),
        }
        samples += arrival.samples.len() as u32;
    }
    println!(
        "\n{what}: {} pointer messages carrying {samples} samples",
        arrivals.len()
    );
    for (kind, count) in by_type {
        println!("  {} x{count}", observe::type_name(kind));
    }
    for (code, count) in by_message {
        println!("  {} x{count}", observe::name(code));
    }
    if let Some(asked) = travel {
        let scale = space.scale();
        let points: Vec<(i32, i32)> = arrivals
            .iter()
            .flat_map(|arrival| arrival.samples.iter().copied())
            .collect();
        let seen: f32 = points
            .windows(2)
            .map(|pair| ((pair[1].0 - pair[0].0) as f32).hypot((pair[1].1 - pair[0].1) as f32))
            .sum::<f32>()
            / scale;
        println!("  path: asked {asked:.1} DIPs, samples carried {seen:.1}");
    }
    if let Some(last) = arrivals.last() {
        let origin = space.origin_px();
        println!(
            "  last at ({:.3}, {:.3}) DIPs",
            (last.at.0 - origin.0) as f32 / space.scale(),
            (last.at.1 - origin.1) as f32 / space.scale()
        );
    }
}

/// The reading side: the message constants and pointer accessors the injector does not carry.
///
/// The injector's binding filter names no message constant and no pointer accessor, so the
/// program observing arrivals declares its own.
// The names are the platform's, so they are spelled the platform's way.
#[expect(non_snake_case)]
mod observe {
    windows_core::link!("user32.dll" "system" fn GetPointerInfo(pointerid : u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn GetPointerInfoHistory(pointerid : u32, entriescount : *mut u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn SetForegroundWindow(hwnd : *mut core::ffi::c_void) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn SetWindowPos(hwnd : *mut core::ffi::c_void, hwndinsertafter : *mut core::ffi::c_void, x : i32, y : i32, cx : i32, cy : i32, flags : u32) -> windows_core::BOOL);
    windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(name : windows_core::PCWSTR) -> *mut core::ffi::c_void);
    windows_core::link!("kernel32.dll" "system" fn GetProcAddress(module : *mut core::ffi::c_void, name : windows_core::PCSTR) -> Option<unsafe extern "system" fn() -> isize>);

    /// The prefix of `POINTER_INFO` this program reads.
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct POINTER_INFO {
        pub pointerType: u32,
        pub pointerId: u32,
        pub frameId: u32,
        pub pointerFlags: u32,
        pub sourceDevice: *mut core::ffi::c_void,
        pub hwndTarget: *mut core::ffi::c_void,
        pub ptPixelLocation: [i32; 2],
        pub ptHimetricLocation: [i32; 2],
        pub ptPixelLocationRaw: [i32; 2],
        pub ptHimetricLocationRaw: [i32; 2],
        pub dwTime: u32,
        pub historyCount: u32,
        pub InputData: i32,
        pub dwKeyStates: u32,
        pub PerformanceCount: u64,
        pub ButtonChangeType: i32,
    }

    pub const fn is_pointer(message: u32) -> bool {
        message >= 0x0241 && message <= 0x0253
    }

    pub const fn is_legacy(message: u32) -> bool {
        (message >= 0x0200 && message <= 0x020E) || message == 0x02A1 || message == 0x02A3
    }

    /// Returns the device, the raw position and the sample history behind a pointer message.
    ///
    /// Raw, not predicted: the predicted position is an extrapolation the system added, and
    /// it is wrong at contact start and at direction reversals, so what was injected is
    /// compared against what was reported.
    pub fn pointer(id: u32) -> (u32, (i32, i32), Vec<(i32, i32)>) {
        let mut info = POINTER_INFO::default();
        // SAFETY: the destination is a stack local the call writes back through.
        if unsafe { GetPointerInfo(id, &mut info) }.as_bool() {
            let at = (info.ptPixelLocationRaw[0], info.ptPixelLocationRaw[1]);
            let samples = match history(id).as_slice() {
                [] => vec![at],
                read => read.to_vec(),
            };
            (info.pointerType, at, samples)
        } else {
            (0, (0, 0), Vec::new())
        }
    }
    /// Returns every sample the message carried, oldest first, in raw screen pixels.
    ///
    /// A fidelity claim integrates this rather than the message's own position. A window that
    /// is not reading its queue receives one `WM_POINTERUPDATE` carrying a frame of samples
    /// rather than one message per sample, so integrating message positions measures how
    /// often the pump ran. The history is what the input was, and what the framework reads.
    pub fn history(id: u32) -> Vec<(i32, i32)> {
        let mut count = 0u32;
        // SAFETY: a null destination with a zero count is the documented way to ask how many
        // entries there are; the count is a stack local the call writes back through.
        if !unsafe { GetPointerInfoHistory(id, &mut count, core::ptr::null_mut()) }.as_bool()
            || count == 0
        {
            return Vec::new();
        }
        let mut entries = vec![POINTER_INFO::default(); count as usize];
        // SAFETY: the buffer holds exactly the number of entries just asked for.
        if !unsafe { GetPointerInfoHistory(id, &mut count, entries.as_mut_ptr()) }.as_bool() {
            return Vec::new();
        }
        entries.truncate(count as usize);
        // Newest first, as documented; a path reads the other way.
        entries.reverse();
        entries
            .iter()
            .map(|entry| (entry.ptPixelLocationRaw[0], entry.ptPixelLocationRaw[1]))
            .collect()
    }

    pub fn topmost(hwnd: *mut core::ffi::c_void) {
        const HWND_TOPMOST: isize = -1;
        const SWP_NOSIZE: u32 = 0x0001;
        const SWP_NOMOVE: u32 = 0x0002;
        const SWP_SHOWWINDOW: u32 = 0x0040;
        // SAFETY: `hwnd` is live; the insert-after handle is the documented sentinel and the
        // flags say the position and size arguments are unused.
        unsafe {
            _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST as *mut core::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            _ = SetForegroundWindow(hwnd);
        }
    }

    /// Reports this window's content as in or out of inertia, so the two inertia actions have
    /// a window to be delivered to. Redacted from the floor's SDK, so resolved by name, and
    /// an error where this build does not export it.
    pub fn report_inertia(hwnd: *mut core::ffi::c_void, started: bool) -> Result<(), String> {
        type Report = unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::BOOL;
        // SAFETY: `user32` is loaded, and the signature transmuted onto the address is the
        // documented one.
        unsafe {
            let user32 = GetModuleHandleW(windows_core::w!("user32.dll"));
            let Some(address) =
                GetProcAddress(user32, windows_core::s!("ReportWindowContentInertia"))
            else {
                return Err("this build of user32 does not export it".to_string());
            };
            let call =
                core::mem::transmute::<unsafe extern "system" fn() -> isize, Report>(address);
            if call(hwnd, i32::from(started)).as_bool() {
                Ok(())
            } else {
                Err(format!("{}", windows_core::Error::from_thread()))
            }
        }
    }

    pub fn type_name(kind: u32) -> &'static str {
        match kind {
            1 => "PT_POINTER",
            2 => "PT_TOUCH",
            3 => "PT_PEN",
            4 => "PT_MOUSE",
            5 => "PT_TOUCHPAD",
            _ => "unreadable",
        }
    }

    /// Returns the name this program expects for `code`, or its hexadecimal value. An
    /// unnamed code is a legacy mouse message or one of the two redacted inertia numbers.
    pub fn name(code: u32) -> String {
        let named = [
            (0x0241, "WM_NCPOINTERUPDATE"),
            (0x0242, "WM_NCPOINTERDOWN"),
            (0x0243, "WM_NCPOINTERUP"),
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
            (0x0020, "WM_SETCURSOR"),
            (0x0014, "WM_ERASEBKGND"),
            (0x0200, "WM_MOUSEMOVE — LEGACY"),
            (0x00A0, "WM_NCMOUSEMOVE — LEGACY"),
            (0x0215, "WM_CAPTURECHANGED"),
        ];
        match named.iter().find(|(value, _)| *value == code) {
            Some((_, name)) => (*name).to_string(),
            None => format!("0x{code:04X}"),
        }
    }
}
