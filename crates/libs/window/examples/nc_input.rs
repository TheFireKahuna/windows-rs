//! Measures whether a custom caption can be driven entirely from pointer input, with no
//! legacy mouse.
//!
//! Three conditions have to hold:
//!
//! 1. **Consuming the non-client pointer messages suppresses the legacy stream.** Where it
//!    does not, the legacy messages arrive whatever the caption does with the pointer ones.
//! 2. **The Snap Layouts flyout survives that consumption.** `HTMAXBUTTON` is its only
//!    trigger, and the flyout may hook the hit-test answer alone or may need the system's
//!    own non-client hover tracking. If it needs the tracking, that one button's hover is
//!    legacy by force.
//! 3. **The system's own drag still runs.** Microsoft documents selectively consuming some
//!    pointer input and passing the rest to `DefWindowProc` as *undefined*, so a caption
//!    that eats hover and forwards presses may or may not still drag. The replacement is
//!    `WM_SYSCOMMAND`/`SC_MOVE`, which is equivalent only if it enters the same modal loop
//!    — the loop Aero shake runs on.
//!
//! The consumption strategies are applied from this example's own message handler, which
//! runs ahead of the caption's.
//!
//! ```text
//! cargo run -p windows-window --example nc_input
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use windows_window::{
    CaptionButton, CaptionButtons, CaptionHit, CaptionSpec, CornerPreference, Result, Window,
};

const BAR_H: f32 = 32.0;
const BUTTON_W: f32 = 46.0;

/// Selects which non-client pointer messages this window's handler swallows before the
/// caption sees them.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Consume {
    /// Swallows nothing, leaving the caption's own partition: consumed over its button
    /// regions, forwarded elsewhere.
    Nothing,
    /// Swallows `WM_NCPOINTERUPDATE` only, the minimum a pointer-native caption needs.
    HoverOnly,
    /// Swallows update, down and up: the fully pointer-native shape.
    AllNcPointer,
}

#[derive(Copy, Clone, Default, Debug)]
struct Tally {
    nc_pointer_update: u32,
    nc_pointer_down: u32,
    nc_pointer_up: u32,
    nc_mouse_move: u32,
    nc_lbutton_down: u32,
    enter_size_move: u32,
    exit_size_move: u32,
}

fn main() -> Result<()> {
    let mode = Rc::new(Cell::new(Consume::Nothing));
    let tally = Rc::new(Cell::new(Tally::default()));

    let window = Window::new("windows-window — non-client input")
        .size(900, 600)
        .no_redirection_bitmap()
        .pointer_input()
        .on_message({
            let mode = Rc::clone(&mode);
            let tally = Rc::clone(&tally);
            move |_, message, _, _| {
                let mut counts = tally.get();
                match message {
                    msg::NC_POINTER_UPDATE => counts.nc_pointer_update += 1,
                    msg::NC_POINTER_DOWN => counts.nc_pointer_down += 1,
                    msg::NC_POINTER_UP => counts.nc_pointer_up += 1,
                    msg::NC_MOUSE_MOVE => counts.nc_mouse_move += 1,
                    msg::NC_LBUTTON_DOWN => counts.nc_lbutton_down += 1,
                    msg::ENTER_SIZE_MOVE => counts.enter_size_move += 1,
                    msg::EXIT_SIZE_MOVE => counts.exit_size_move += 1,
                    _ => {}
                }
                tally.set(counts);

                // Consuming is returning 0 rather than passing the message on, which is
                // what stops `DefWindowProc` synthesizing the legacy message behind it.
                let swallow = match mode.get() {
                    Consume::Nothing => false,
                    Consume::HoverOnly => message == msg::NC_POINTER_UPDATE,
                    Consume::AllNcPointer => matches!(
                        message,
                        msg::NC_POINTER_UPDATE | msg::NC_POINTER_DOWN | msg::NC_POINTER_UP
                    ),
                };
                swallow.then_some(0)
            }
        })
        .custom_caption(CaptionSpec {
            height: Some(BAR_H),
            corners: CornerPreference::Round,
            // Only the maximize button, which is the one that has to answer a non-client
            // hit code. Close and minimize belong on the client path and are left out.
            buttons: CaptionButtons {
                minimize: false,
                maximize: true,
                close: false,
            },
        })
        .create()?;

    let metrics = window.metrics().expect("an open window");
    let (client_w, _) = window.client_size().expect("an open window");
    let width_dips = metrics.dips(client_w);
    window
        .on_caption_hit(move |x, y| {
            if y >= BAR_H {
                CaptionHit::Client
            } else if width_dips - x < BUTTON_W {
                CaptionHit::Button(CaptionButton::Maximize)
            } else {
                CaptionHit::Drag
            }
        })
        .expect("a window with a caption of its own");

    // Topmost and at a known place: a probe launched from a terminal otherwise hovers the
    // terminal, and every finding then describes the harness.
    sys::place_on_top(&window, 100, 100, 900, 600);
    settle(&window, 200);
    // Clicks the client area to activate. Snap Layouts is not offered to a window that is
    // not foreground, so every arm depends on this having worked.
    sys::move_cursor(400, 500);
    settle(&window, 120);
    sys::mouse_button(true);
    settle(&window, 80);
    sys::mouse_button(false);
    settle(&window, 250);
    let mut foreground = sys::is_foreground(&window);
    if !foreground {
        foreground = sys::activate(&window);
        settle(&window, 250);
    }
    // Leaves the topmost band now that the click has raised and activated the window: a
    // topmost window is not offered Snap Layouts.
    sys::drop_topmost(&window);
    settle(&window, 250);
    println!("  foreground: {foreground}");
    if !foreground {
        println!("  WARNING: not foreground; every arm below measures the harness, not Windows");
    }

    let origin = sys::client_origin(&window);
    let maximize = (
        origin.0 + client_w - metrics.px(BUTTON_W / 2.0),
        origin.1 + metrics.px(BAR_H) / 2,
    );
    let strip = (origin.0 + 200, origin.1 + metrics.px(BAR_H) / 2);
    let away = (origin.0 + 300, origin.1 + 400);
    let restore = sys::cursor_position();

    println!(
        "  window at {origin:?}, maximize button at {maximize:?}, dpi {}",
        metrics.dpi
    );
    // The flyout arm depends on the window answering HTMAXBUTTON, which is 9, at this
    // point.
    let answer_at_button = sys::hit_test(&window, maximize.0, maximize.1);
    println!(
        "  hit test at the maximize button: {answer_at_button} ({})",
        if answer_at_button == 9 {
            "HTMAXBUTTON"
        } else {
            "NOT HTMAXBUTTON"
        }
    );
    println!(
        "  our window is under the maximize point: {}",
        sys::covers(&window, maximize.0, maximize.1)
    );
    let before = sys::visible_classes();

    // ── 1 & 2. hover the maximize button under each consumption strategy ─────────────
    println!("\n  hovering the maximize button, per consumption strategy");
    println!(
        "    {:<16} {:>10} {:>12} {:>10}  reading",
        "consume", "NCPTR_UPD", "NCMOUSEMOVE", "flyout"
    );
    let mut results = Vec::new();
    for strategy in [Consume::Nothing, Consume::HoverOnly, Consume::AllNcPointer] {
        sys::move_cursor(away.0, away.1);
        settle(&window, 250);
        mode.set(strategy);
        tally.set(Tally::default());

        // Several small moves inside the button, so the pointer and legacy counts are a
        // ratio rather than a single sample.
        for step in 0..6 {
            sys::move_cursor(maximize.0 - step, maximize.1);
            settle(&window, 60);
        }
        // The flyout is a hover-intent gesture and takes upwards of 1.6 s to appear.
        settle(&window, 1800);
        let flyout = sys::snap_flyout();
        if strategy == Consume::Nothing {
            let after = sys::visible_classes();
            let fresh: Vec<&String> = after.iter().filter(|c| !before.contains(c)).collect();
            println!("      windows that appeared during the baseline hover: {fresh:?}");
        }
        let counts = tally.get();

        let reading = match (counts.nc_pointer_update > 0, counts.nc_mouse_move > 0) {
            (true, false) => "pointer only",
            (true, true) => "both",
            (false, true) => "legacy only",
            (false, false) => "nothing arrived",
        };
        println!(
            "    {:<16} {:>10} {:>12} {:>10}  {reading}",
            format!("{strategy:?}"),
            counts.nc_pointer_update,
            counts.nc_mouse_move,
            flyout.as_deref().unwrap_or("none")
        );
        results.push((strategy, counts, flyout));
    }
    sys::move_cursor(away.0, away.1);
    settle(&window, 300);

    // ── 3. does the system's own drag survive selective consumption? ─────────────────
    //
    // A press on `HTCAPTION` puts `DefWindowProc` into its modal move loop, which pumps
    // for itself, so the release has to come from another thread or this one never returns.
    println!("\n  pressing the drag strip, per consumption strategy");
    println!(
        "    {:<16} {:>12} {:>14} {:>16}",
        "consume", "NCPTR_DOWN", "NCLBUTTONDOWN", "ENTER/EXITSIZE"
    );
    let mut drag_results = Vec::new();
    for strategy in [Consume::Nothing, Consume::HoverOnly, Consume::AllNcPointer] {
        sys::move_cursor(strip.0, strip.1);
        settle(&window, 250);
        mode.set(strategy);
        tally.set(Tally::default());

        let from = strip;
        let releaser = std::thread::spawn(move || {
            // Moves past the drag threshold, then releases. From another thread because the
            // modal move loop pumps for itself and the main thread stays in it until the
            // release.
            std::thread::sleep(Duration::from_millis(250));
            for step in 1..=6 {
                sys::move_cursor(from.0 + step * 4, from.1);
                std::thread::sleep(Duration::from_millis(30));
            }
            std::thread::sleep(Duration::from_millis(150));
            sys::mouse_button(false);
        });
        sys::mouse_button(true);
        settle(&window, 1200);
        let _ = releaser.join();
        settle(&window, 200);
        // The drag moved the window; put it back so the next arm starts where this did.
        sys::place_on_top(&window, 100, 100, 900, 600);
        settle(&window, 150);

        let counts = tally.get();
        println!(
            "    {:<16} {:>12} {:>14} {:>8} / {:<5}",
            format!("{strategy:?}"),
            counts.nc_pointer_down,
            counts.nc_lbutton_down,
            counts.enter_size_move,
            counts.exit_size_move
        );
        drag_results.push((strategy, counts));
    }

    // ── 4. is SC_MOVE the same modal loop? ───────────────────────────────────────────
    //
    // `SC_MOVE` replaces a system drag only if it enters the loop Aero shake runs on, and
    // `WM_ENTERSIZEMOVE` is that loop announcing itself.
    mode.set(Consume::Nothing);
    tally.set(Tally::default());
    let escaper = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(600));
        sys::press_escape();
    });
    sys::post_syscommand(&window, sys::SC_MOVE);
    settle(&window, 1600);
    let _ = escaper.join();
    settle(&window, 200);
    let sc_move = tally.get();
    println!(
        "\n  WM_SYSCOMMAND/SC_MOVE: ENTERSIZEMOVE {}, EXITSIZEMOVE {}",
        sc_move.enter_size_move, sc_move.exit_size_move
    );

    sys::move_cursor(restore.0, restore.1);

    // ── the three answers ────────────────────────────────────────────────────────────
    let baseline = &results[0];
    let hover_only = &results[1];
    let all_nc = &results[2];

    println!("\n  answers");
    answer(
        "consuming the non-client pointer messages suppresses the legacy stream",
        hover_only.1.nc_mouse_move == 0 && baseline.1.nc_mouse_move > 0,
        format!(
            "baseline NCMOUSEMOVE {}, with hover consumed {}",
            baseline.1.nc_mouse_move, hover_only.1.nc_mouse_move
        ),
    );
    answer(
        "the Snap Layouts flyout survives consuming them",
        baseline.2.is_some() && hover_only.2.is_some(),
        format!(
            "baseline {:?}, hover consumed {:?}, all consumed {:?}",
            baseline.2, hover_only.2, all_nc.2
        ),
    );
    answer(
        "the system's own drag survives selective consumption",
        drag_results[1].1.enter_size_move > 0,
        format!(
            "baseline ENTERSIZEMOVE {}, hover consumed {}, all consumed {}",
            drag_results[0].1.enter_size_move,
            drag_results[1].1.enter_size_move,
            drag_results[2].1.enter_size_move
        ),
    );
    answer(
        "SC_MOVE enters the same modal loop, so Aero shake has somewhere to live",
        sc_move.enter_size_move > 0,
        format!("ENTERSIZEMOVE {}", sc_move.enter_size_move),
    );
    Ok(())
}

fn answer(claim: &str, held: bool, note: String) {
    println!(
        "    [{}] {claim}\n         {note}",
        if held { "yes" } else { "NO " }
    );
}

/// Pumps for `ms` milliseconds, so the window services the input just injected.
fn settle(_window: &Window, ms: u64) {
    let step = 10;
    for _ in 0..(ms / step).max(1) {
        windows_window::pump();
        std::thread::sleep(Duration::from_millis(step));
    }
}

/// The non-client and modal-loop message identifiers this example counts.
mod msg {
    pub const NC_MOUSE_MOVE: u32 = 0x00A0;
    pub const NC_LBUTTON_DOWN: u32 = 0x00A1;
    pub const NC_POINTER_UPDATE: u32 = 0x0241;
    pub const NC_POINTER_DOWN: u32 = 0x0242;
    pub const NC_POINTER_UP: u32 = 0x0243;
    pub const ENTER_SIZE_MOVE: u32 = 0x0231;
    pub const EXIT_SIZE_MOVE: u32 = 0x0232;
}

/// The raw Win32 this example drives the window with: injected input, z-order, activation
/// and window enumeration.
mod sys {
    use windows_window::Window;

    pub const SC_MOVE: usize = 0xF010;

    type Hwnd = *mut core::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct MouseInput {
        dx: i32,
        dy: i32,
        mouse_data: u32,
        flags: u32,
        time: u32,
        extra: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct KeyboardInput {
        vk: u16,
        scan: u16,
        flags: u32,
        time: u32,
        extra: usize,
    }

    #[repr(C)]
    union Payload {
        mi: MouseInput,
        ki: KeyboardInput,
    }

    #[repr(C)]
    struct Input {
        kind: u32,
        _pad: u32,
        payload: Payload,
    }

    #[link(name = "user32", kind = "raw-dylib")]
    unsafe extern "system" {
        fn ClientToScreen(hwnd: Hwnd, point: *mut Point) -> i32;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn GetSystemMetrics(index: i32) -> i32;
        fn SetWindowPos(
            hwnd: Hwnd,
            after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn SendMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn EnumWindows(
            callback: unsafe extern "system" fn(Hwnd, isize) -> i32,
            param: isize,
        ) -> i32;
        fn GetClassNameW(hwnd: Hwnd, buffer: *mut u16, len: i32) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn WindowFromPoint(point: Point) -> Hwnd;
        fn SetForegroundWindow(hwnd: Hwnd) -> i32;
        fn GetForegroundWindow() -> Hwnd;
    }

    /// Places the window at a screen position and raises it into the topmost band, without
    /// activating it.
    ///
    /// A topmost window is not offered Snap Layouts, so the flyout arm calls
    /// [`drop_topmost`] before it measures.
    pub fn place_on_top(window: &Window, x: i32, y: i32, w: i32, h: i32) {
        const HWND_TOPMOST: isize = -1;
        const SWP_NOACTIVATE: u32 = 0x0010;
        // SAFETY: `SetWindowPos` accepts any handle value and takes only integers besides
        // it.
        unsafe { SetWindowPos(window.hwnd(), HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE) };
    }

    /// Leaves the always-on-top band, keeping the window where the activating click put it.
    pub fn drop_topmost(window: &Window) {
        const HWND_NOTOPMOST: isize = -2;
        const NOMOVE_NOSIZE_NOACTIVATE: u32 = 0x0002 | 0x0001 | 0x0010;
        // SAFETY: `SetWindowPos` accepts any handle value and takes only integers besides
        // it.
        unsafe {
            SetWindowPos(
                window.hwnd(),
                HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                NOMOVE_NOSIZE_NOACTIVATE,
            )
        };
    }

    /// Returns whether `window` is the one under a screen point.
    pub fn covers(window: &Window, x: i32, y: i32) -> bool {
        // SAFETY: `WindowFromPoint` takes its point by value, and the handle it returns is
        // compared rather than dereferenced.
        unsafe { WindowFromPoint(Point { x, y }) == window.hwnd() }
    }

    /// Activates the window, returning whether it became the foreground one.
    ///
    /// Snap Layouts does not offer to arrange a window that is not foreground, and a press
    /// on an inactive window's caption activates it rather than beginning a drag, so every
    /// arm depends on the window being foreground.
    ///
    /// `SetForegroundWindow` is refused to a process that did not receive the last input
    /// event, which a console child launched by a build tool has not, so the caller clicks
    /// the window the way a user would and calls this only as a fallback.
    pub fn activate(window: &Window) -> bool {
        // SAFETY: both calls accept any handle value, and the handle returned is compared
        // rather than dereferenced.
        unsafe {
            SetForegroundWindow(window.hwnd());
            GetForegroundWindow() == window.hwnd()
        }
    }

    /// Returns whether the window is the foreground one.
    pub fn is_foreground(window: &Window) -> bool {
        // SAFETY: `GetForegroundWindow` takes no argument, and the handle it returns is
        // compared rather than dereferenced.
        unsafe { GetForegroundWindow() == window.hwnd() }
    }

    /// Returns the window's client origin in screen coordinates.
    pub fn client_origin(window: &Window) -> (i32, i32) {
        let mut point = Point::default();
        // SAFETY: `ClientToScreen` accepts any handle value and writes only through
        // `point`, a live stack local of the size it expects.
        unsafe { ClientToScreen(window.hwnd(), &mut point) };
        (point.x, point.y)
    }

    /// Returns the pointer's current screen position.
    pub fn cursor_position() -> (i32, i32) {
        let mut point = Point::default();
        // SAFETY: `GetCursorPos` writes only through `point`, a live stack local of the
        // size it expects.
        unsafe { GetCursorPos(&mut point) };
        (point.x, point.y)
    }

    /// Posts a `WM_SYSCOMMAND` to the window, returning before it runs.
    pub fn post_syscommand(window: &Window, command: usize) {
        const WM_SYSCOMMAND: u32 = 0x0112;
        // SAFETY: `PostMessageW` accepts any handle value, and `WM_SYSCOMMAND` carries the
        // command in `wparam` rather than a pointer, so nothing has to outlive the post.
        unsafe { PostMessageW(window.hwnd(), WM_SYSCOMMAND, command, 0) };
    }

    fn send(input: Input) {
        // SAFETY: `SendInput` reads `size_of::<Input>()` bytes from `input`, a live local
        // whose `#[repr(C)]` layout is the `INPUT` it expects, and the size passed says so.
        unsafe { SendInput(1, &input, size_of::<Input>() as i32) };
    }

    /// Moves the pointer to a screen point with an injected input event.
    ///
    /// `SetCursorPos` warps the cursor and lets the window manager notice, where this puts
    /// an event through the input stack, which is what this example counts.
    pub fn move_cursor(x: i32, y: i32) {
        const MOUSEEVENTF_MOVE: u32 = 0x0001;
        const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
        const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;
        // SAFETY: `GetSystemMetrics` takes an index and returns an integer.
        let (vx, vy, vw, vh) = unsafe {
            (
                GetSystemMetrics(76),
                GetSystemMetrics(77),
                GetSystemMetrics(78).max(1),
                GetSystemMetrics(79).max(1),
            )
        };
        send(Input {
            kind: 0,
            _pad: 0,
            payload: Payload {
                mi: MouseInput {
                    dx: ((x - vx) as i64 * 65535 / vw as i64) as i32,
                    dy: ((y - vy) as i64 * 65535 / vh as i64) as i32,
                    flags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    ..Default::default()
                },
            },
        });
    }

    /// Presses or releases the primary button as an injected input event.
    pub fn mouse_button(down: bool) {
        const LEFTDOWN: u32 = 0x0002;
        const LEFTUP: u32 = 0x0004;
        send(Input {
            kind: 0,
            _pad: 0,
            payload: Payload {
                mi: MouseInput {
                    flags: if down { LEFTDOWN } else { LEFTUP },
                    ..Default::default()
                },
            },
        });
    }

    /// Presses and releases Escape, ending a modal move loop that would otherwise run until
    /// the user ends it.
    pub fn press_escape() {
        const VK_ESCAPE: u16 = 0x1B;
        const KEYEVENTF_KEYUP: u32 = 0x0002;
        for flags in [0, KEYEVENTF_KEYUP] {
            send(Input {
                kind: 1,
                _pad: 0,
                payload: Payload {
                    ki: KeyboardInput {
                        vk: VK_ESCAPE,
                        flags,
                        ..Default::default()
                    },
                },
            });
        }
    }

    use std::sync::Mutex;
    static CLASSES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    /// Returns the class name of every visible top-level window.
    ///
    /// Differencing the list across a hover separates a flyout that did not appear from one
    /// that appeared under a class name [`snap_flyout`] does not recognise.
    pub fn visible_classes() -> Vec<String> {
        /// # Safety
        ///
        /// `hwnd` must be a window handle `EnumWindows` supplied for the enumeration in
        /// progress.
        unsafe extern "system" fn collect(hwnd: Hwnd, _: isize) -> i32 {
            // SAFETY: `hwnd` names a live window for the duration of the callback, and
            // `GetClassNameW` writes at most `buffer.len()` code units into `buffer`.
            unsafe {
                if IsWindowVisible(hwnd) == 0 {
                    return 1;
                }
                let mut buffer = [0u16; 128];
                let len = GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
                if len > 0 {
                    CLASSES
                        .lock()
                        .expect("no panic holds this lock")
                        .push(String::from_utf16_lossy(&buffer[..len as usize]));
                }
            }
            1
        }
        CLASSES.lock().expect("no panic holds this lock").clear();
        // SAFETY: `collect` has the signature `EnumWindows` calls, and every handle it
        // passes is one this enumeration produced.
        unsafe { EnumWindows(collect, 0) };
        CLASSES.lock().expect("no panic holds this lock").clone()
    }

    /// Returns the `HT*` code the window answers for a screen point.
    pub fn hit_test(window: &Window, x: i32, y: i32) -> i32 {
        const WM_NCHITTEST: u32 = 0x0084;
        let lparam = ((y as u32 as isize) << 16) | (x as u32 as isize & 0xffff);
        // SAFETY: `SendMessageW` accepts any handle value, and `WM_NCHITTEST` reads the
        // point out of `lparam` rather than through a pointer.
        unsafe { SendMessageW(window.hwnd(), WM_NCHITTEST, 0, lparam) as i32 }
    }

    static mut FOUND: Option<String> = None;

    /// Returns the class name of the Snap Layouts flyout while it is open, matching the two
    /// names the shell gives that separate window.
    ///
    /// A flyout under any other class name reads as absence here, which is what
    /// [`visible_classes`] distinguishes.
    pub fn snap_flyout() -> Option<String> {
        /// # Safety
        ///
        /// `hwnd` must be a window handle `EnumWindows` supplied for the enumeration in
        /// progress, and the calling thread must be the one that owns `FOUND`.
        unsafe extern "system" fn visit(hwnd: Hwnd, _: isize) -> i32 {
            // SAFETY: `hwnd` names a live window for the duration of the callback,
            // `GetClassNameW` writes at most `buffer.len()` code units into `buffer`, and
            // the enumeration this callback runs under is the only writer of `FOUND`.
            unsafe {
                if IsWindowVisible(hwnd) == 0 {
                    return 1;
                }
                let mut buffer = [0u16; 128];
                let len = GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
                if len <= 0 {
                    return 1;
                }
                let class = String::from_utf16_lossy(&buffer[..len as usize]);
                if class.contains("XamlExplorerHostIslandWindow")
                    || class.contains("SnapAssistFlyout")
                {
                    FOUND = Some(class);
                    return 0;
                }
            }
            1
        }
        // SAFETY: `EnumWindows` runs `visit` synchronously on this thread, and no other
        // thread in this example touches `FOUND`, so the two accesses cannot overlap.
        unsafe {
            FOUND = None;
            EnumWindows(visit, 0);
            #[allow(static_mut_refs)]
            FOUND.clone()
        }
    }
}
