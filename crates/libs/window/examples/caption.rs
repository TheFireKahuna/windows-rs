//! A composition-hosted window with a caption of its own, driven and reported on.
//!
//! Removing the system caption removes none of its behaviour: with no title bar, no
//! redirection surface and no content, the window still answers the eight resize zones at
//! its monitor's own frame widths, opens the window menu, drags from the bar, and keeps its
//! client area inside the work area when maximized.
//!
//! Every answer is the window's own, asked with `WM_NCHITTEST` as the system asks it, and
//! every button is driven with real injected input — the caption reads `WM_NCPOINTER*` and
//! asks which button changed, so a synthesized message has nothing behind it to answer
//! with. Snap Layouts is measured by `examples/nc_input` instead.
//!
//! The bar's contents stand in for the hit array a real application owns: one rectangle at
//! the left is a control, the three at the right are the window commands, everything else
//! drags. `on_caption_hit` answers in client-space DIPs and the window turns that into an
//! `HT*` code.
//!
//! ```text
//! cargo run -p windows-window --example caption
//! cargo run -p windows-window --example caption -- --hold   # leave it up to poke at
//! ```

use std::cell::Cell;
use std::rc::Rc;

use windows_window::{
    CaptionButton, CaptionButtons, CaptionHit, CaptionSpec, CaptionState, CornerPreference,
    Feedback, FeedbackPolicy, Result, Window,
};

/// The stand-in bar layout, in DIPs.
const BAR_H: f32 = 32.0;
const CONTROL_W: f32 = 120.0;
const BUTTON_W: f32 = 46.0;

/// Pumps for `ms`, so injected input is delivered before the next question is asked.
///
/// `SendInput` returns once the event is queued, not once the window has seen it. Pumping
/// rather than sleeping: the messages have to be dispatched for anything to change.
fn settle(ms: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < deadline {
        windows_window::pump();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn main() -> Result<()> {
    let hold = std::env::args().any(|a| a == "--hold");

    let state_changes = Rc::new(Cell::new(0u32));
    // What the system actually delivers while the pointer sits on a caption button. Only a
    // tally, and only so that a hover that does not arrive can be told apart from one that
    // arrives by a route this crate is not reading.
    let tally = Rc::new(Cell::new([0u32; 6]));
    let window = Window::new("windows-window — a caption of its own")
        .size_dips(900.0, 600.0)
        .on_message({
            let tally = Rc::clone(&tally);
            move |_, message, _, _| {
                let slot = match message {
                    0x0084 => Some(0), // WM_NCHITTEST
                    0x00A0 => Some(1), // WM_NCMOUSEMOVE
                    0x02A2 => Some(2), // WM_NCMOUSELEAVE
                    0x0241 => Some(3), // WM_NCPOINTERUPDATE
                    0x0200 => Some(4), // WM_MOUSEMOVE
                    0x0245 => Some(5), // WM_POINTERUPDATE
                    _ => None,
                };
                if let Some(slot) = slot {
                    let mut counts = tally.get();
                    counts[slot] += 1;
                    tally.set(counts);
                }
                None
            }
        })
        .no_redirection_bitmap()
        .pointer_input()
        .touchpad_capable()
        // The one decision this example makes about system-drawn feedback, so that the
        // call is exercised rather than merely compiled.
        .feedback(FeedbackPolicy::SYSTEM.without(Feedback::TouchContact))
        .custom_caption(CaptionSpec {
            height: Some(BAR_H),
            corners: CornerPreference::Round,
            buttons: CaptionButtons::ALL,
        })
        .create()?;

    // The stand-in hit array. A real one is the layout's flat array; the shape of the
    // answer is the same either way.
    let width_dips = Cell::new(0.0f32);
    window
        .on_caption_hit(move |x, y| {
            if y >= BAR_H {
                return CaptionHit::Client;
            }
            if x < CONTROL_W {
                return CaptionHit::Client;
            }
            let from_right = width_dips.get() - x;
            if from_right < BUTTON_W {
                CaptionHit::Button(CaptionButton::Close)
            } else if from_right < BUTTON_W * 2.0 {
                CaptionHit::Button(CaptionButton::Maximize)
            } else if from_right < BUTTON_W * 3.0 {
                CaptionHit::Button(CaptionButton::Minimize)
            } else {
                CaptionHit::Drag
            }
        })
        .expect("a window with a caption of its own");

    // The last state the window published, which is what the assertions below read.
    let last_state = Rc::new(Cell::new(CaptionState::default()));
    // Whether a hover was ever reported. Sticky, and reset before the arm that reads it:
    // moving the pointer to the next probe point publishes a leave behind the finding, so
    // reading the *latest* state after the fact would read that leave rather than the hover
    // it is asking about.
    let hover_seen = Rc::new(Cell::new(false));
    {
        let state_changes = Rc::clone(&state_changes);
        let last_state = Rc::clone(&last_state);
        let hover_seen = Rc::clone(&hover_seen);
        window
            .on_caption_state(move |state| {
                state_changes.set(state_changes.get() + 1);
                last_state.set(state);
                if state.hover == Some(CaptionButton::Maximize) {
                    hover_seen.set(true);
                }
                println!("    caption state: {state:?}");
            })
            .expect("a window with a caption of its own");
    }

    // The bar's own width, which the closure above needs and which is only known once the
    // window has a size. Re-read on every probe rather than cached, because a DPI change
    // moves it.
    let metrics = window.metrics().expect("an open window");
    let (client_w, client_h) = window.client_size().expect("an open window");
    let dips_w = metrics.dips(client_w);
    // Re-install now that the width is known. Installing twice is the documented
    // behaviour: the second replaces the first.
    window
        .on_caption_hit(move |x, y| {
            if y >= BAR_H || x < CONTROL_W {
                return CaptionHit::Client;
            }
            let from_right = dips_w - x;
            if from_right < BUTTON_W {
                CaptionHit::Button(CaptionButton::Close)
            } else if from_right < BUTTON_W * 2.0 {
                CaptionHit::Button(CaptionButton::Maximize)
            } else if from_right < BUTTON_W * 3.0 {
                CaptionHit::Button(CaptionButton::Minimize)
            } else {
                CaptionHit::Drag
            }
        })
        .expect("a window with a caption of its own");

    println!("  window");
    println!(
        "    dpi {} (scale {:.2}), frame {}×{} px, system caption {} px",
        metrics.dpi, metrics.scale, metrics.frame_x, metrics.frame_y, metrics.caption
    );
    println!(
        "    client {client_w}×{client_h} px = {dips_w:.0}×{:.0} dips, caption band {:?} dips",
        metrics.dips(client_h),
        window.caption_height_dips()
    );
    println!("    colour capability {:?}", window.color_capability());
    println!(
        "    corner radius {} dips, touchpad-capable {}",
        CornerPreference::Round.radius_dips(),
        window.is_touchpad_capable()
    );

    let mut findings: Vec<(&str, bool, String)> = Vec::new();

    // ── the top border is gone ───────────────────────────────────────────────────────
    let window_rect = probe::window_rect(&window);
    let origin = probe::client_origin(&window);
    findings.push((
        "WM_NCCALCSIZE removed the top border (client top == window top)",
        origin.1 == window_rect.1,
        format!(
            "window top {}, client top {} (delta {})",
            window_rect.1,
            origin.1,
            origin.1 - window_rect.1
        ),
    ));

    // ── the eight zones, the band, a control in it, and the buttons ──────────────────
    let (left, top, right, bottom) = window_rect;
    let mid_x = (left + right) / 2;
    let mid_y = (top + bottom) / 2;
    let band_y = origin.1 + metrics.px(BAR_H) / 2;
    let probes: [(&str, i32, i32, &str); 13] = [
        ("top-left", left + 1, top + 1, "HTTOPLEFT"),
        ("top", mid_x, top + 1, "HTTOP"),
        ("top-right", right - 2, top + 1, "HTTOPRIGHT"),
        ("left", left + 1, mid_y, "HTLEFT"),
        ("right", right - 2, mid_y, "HTRIGHT"),
        ("bottom-left", left + 1, bottom - 2, "HTBOTTOMLEFT"),
        ("bottom", mid_x, bottom - 2, "HTBOTTOM"),
        ("bottom-right", right - 2, bottom - 2, "HTBOTTOMRIGHT"),
        ("bar, empty", mid_x, band_y, "HTCAPTION"),
        (
            "bar, a control in it",
            origin.0 + metrics.px(CONTROL_W / 2.0),
            band_y,
            "HTCLIENT",
        ),
        (
            "bar, close",
            origin.0 + client_w - metrics.px(BUTTON_W / 2.0),
            band_y,
            "HTCLOSE",
        ),
        (
            "bar, maximize",
            origin.0 + client_w - metrics.px(BUTTON_W * 1.5),
            band_y,
            "HTMAXBUTTON",
        ),
        (
            "bar, minimize",
            origin.0 + client_w - metrics.px(BUTTON_W * 2.5),
            band_y,
            "HTMINBUTTON",
        ),
    ];

    println!("\n  WM_NCHITTEST, answered by the window itself");
    let mut all_zones = true;
    for (name, x, y, expected) in probes {
        let got = probe::hit_test(&window, x, y);
        let ok = got == expected;
        all_zones &= ok;
        println!(
            "    {name:<22} -> {got}{}",
            if ok {
                String::new()
            } else {
                format!("   (expected {expected})")
            }
        );
    }
    findings.push((
        "the eight resize zones, the drag strip, a control and the three buttons all resolve",
        all_zones,
        "frame widths from GetSystemMetricsForDpi; the bar from the hit authority".into(),
    ));

    // ── what a REAL hover delivers on a window that asked for pointer input ──────────
    //
    // The arms above were driven with sent messages, which proves the caption's handling
    // and proves nothing about what Windows sends. This moves the actual pointer, and
    // counts what arrives. It is the question of whether any legacy mouse path is left.
    let restore = probe::cursor_position();
    probe::place_on_top(&window, 200, 200, client_w, client_h);
    for _ in 0..10 {
        windows_window::pump();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let origin_now = probe::client_origin(&window);
    let (hover_x, hover_y) = (
        origin_now.0 + client_w - metrics.px(BUTTON_W * 1.5),
        origin_now.1 + metrics.px(BAR_H) / 2,
    );
    probe::move_cursor(hover_x - 40, hover_y);
    windows_window::pump();
    tally.set([0; 6]);
    probe::move_cursor(hover_x, hover_y);
    let on_top = probe::is_on_top(&window, hover_x, hover_y);
    // Long enough to span several of the system's hover-tracking repeats: a shorter
    // sample reads zero legacy messages and says nothing about why.
    settle(1500);
    let counts = tally.get();
    println!(
        "
  what a real hover over the maximize button delivered (window on top: {on_top})"
    );
    for (name, count) in [
        ("WM_NCHITTEST", counts[0]),
        ("WM_NCMOUSEMOVE", counts[1]),
        ("WM_NCMOUSELEAVE", counts[2]),
        ("WM_NCPOINTERUPDATE", counts[3]),
        ("WM_MOUSEMOVE", counts[4]),
        ("WM_POINTERUPDATE", counts[5]),
    ] {
        println!("    {name:<22} {count}");
    }
    // And the same question for the client area, which is the half `EnableMouseInPointer`
    // is actually about.
    let (client_x, client_y) = (
        origin_now.0 + client_w / 2,
        origin_now.1 + metrics.px(BAR_H) + 100,
    );
    probe::move_cursor(client_x - 40, client_y);
    windows_window::pump();
    tally.set([0; 6]);
    probe::move_cursor(client_x, client_y);
    // Long enough to span several of the system's hover-tracking repeats: a shorter
    // sample reads zero legacy messages and says nothing about why.
    settle(1500);
    let client_counts = tally.get();
    probe::move_cursor(restore.0, restore.1);
    probe::drop_topmost(&window);
    windows_window::pump();

    println!("  the same, over the client area");
    for (name, count) in [
        ("WM_MOUSEMOVE", client_counts[4]),
        ("WM_POINTERUPDATE", client_counts[5]),
    ] {
        println!("    {name:<22} {count}");
    }

    // The caption's input arrives as pointer input. That is the assertion.
    //
    // It is deliberately **not** "and no legacy message arrives", because they do and
    // always will: the system re-posts `WM_NCMOUSEMOVE` at a fixed coordinate for as long
    // as the cursor rests in the non-client area, with no new pointer input behind it.
    // That hover-tracking loop is a separate generator from `DefWindowProc`'s
    // pointer-to-mouse promotion, and consuming the pointer message stops the second
    // without touching the first. Asserting zero here passes only by hovering for less
    // time than the repeat interval, which is a measurement of the probe.
    //
    // The count is reported so the number is visible, and so a change in it is noticed.
    findings.push((
        "the caption's own input is pointer input",
        on_top && counts[3] > 0,
        format!(
            "over the maximize button: NCPOINTERUPDATE {}, plus NCMOUSEMOVE {} from the system's |              own non-client hover tracking, unhandled. Client area: POINTERUPDATE {}, MOUSEMOVE {}",
            counts[3], counts[1], client_counts[5], client_counts[4]
        ),
    ));

    // ── the caption buttons: hover, press, and what a release commits ────────────────
    //
    // Driven with real input over the real button rects. On top first, or the findings
    // describe the terminal that launched this.
    probe::place_on_top(&window, 200, 200, client_w, client_h);
    settle(150);
    let origin_now = probe::client_origin(&window);
    let bar_y = origin_now.1 + metrics.px(BAR_H) / 2;
    let maximize_x = origin_now.0 + client_w - metrics.px(BUTTON_W * 1.5);
    let close_x = origin_now.0 + client_w - metrics.px(BUTTON_W * 0.5);

    hover_seen.set(false);
    probe::move_cursor(maximize_x, bar_y);
    settle(120);
    let hovered = hover_seen.get();

    probe::mouse_button(true);
    settle(120);
    let pressed = last_state.get().pressed == Some(CaptionButton::Maximize);

    // A release on a *different* button must not commit: sliding off a button is how a
    // user cancels a press on it.
    probe::move_cursor(close_x, bar_y);
    settle(80);
    probe::mouse_button(false);
    settle(150);
    let cancelled = !probe::is_maximized(&window) && last_state.get().pressed.is_none();

    // And a press followed by a release on the same one must.
    probe::move_cursor(maximize_x, bar_y);
    settle(80);
    probe::mouse_button(true);
    settle(80);
    probe::mouse_button(false);
    settle(250);
    let committed = probe::is_maximized(&window);
    probe::move_cursor(restore.0, restore.1);
    probe::drop_topmost(&window);
    settle(80);

    findings.push((
        "hover and press on a caption button reach the application",
        hovered && pressed,
        format!("hover reported {hovered}, press reported {pressed}"),
    ));
    findings.push((
        "a release off the button cancels the press; one on it commits",
        cancelled && committed,
        format!(
            "cancelled {cancelled}, committed {committed} (window maximized by its own button)"
        ),
    ));

    // ── what the hit test costs, since it runs per pointer move ──────────────────────
    //
    // `WM_NCHITTEST` is not on the frame clock — the system wants a synchronous answer
    // every time the pointer moves — so the metrics query behind it is measured rather
    // than asserted to be cheap.
    let start = std::time::Instant::now();
    let mut sink = 0i32;
    for _ in 0..100_000 {
        sink = sink.wrapping_add(window.metrics().expect("an open window").frame_x);
    }
    let per_query = start.elapsed().as_nanos() as f64 / 100_000.0;
    println!("\n  Metrics::for_window: {per_query:.0} ns per query (sink {sink})");

    // ── maximized: no resize edges, and the client stays on the work area ────────────
    // Already maximized, by the button above.
    windows_window::pump();
    std::thread::sleep(std::time::Duration::from_millis(200));
    windows_window::pump();

    let corner = probe::hit_test(&window, probe::window_rect(&window).0 + 1, {
        let r = probe::window_rect(&window);
        r.1 + 1
    });
    findings.push((
        "a maximized window has no resize edges",
        corner != "HTTOPLEFT",
        format!("its top-left corner answers {corner}"),
    ));

    let overhang = probe::work_area_overhang(&window);
    findings.push((
        "maximizing does not push the client area off the work area",
        overhang == (0, 0, 0, 0),
        format!("overhang l/t/r/b {overhang:?}"),
    ));

    // The bar is drawn at the client's top edge, so a maximized client that starts below
    // the work area's top edge leaves a band of nothing above it — the classic defect of a
    // custom caption that keeps `DefWindowProc`'s whole maximized inset.
    let maximized_gap = probe::client_origin(&window).1 - probe::work_area(&window).1;
    findings.push((
        "a maximized window's bar starts at the top of the work area",
        maximized_gap == 0,
        format!(
            "client top is {maximized_gap} px below the work area; window rect {:?}, work area \
             top {}",
            probe::window_rect(&window),
            probe::work_area(&window).1
        ),
    ));

    probe::system_command(&window, probe::SC_RESTORE);
    windows_window::pump();
    std::thread::sleep(std::time::Duration::from_millis(200));
    windows_window::pump();

    // ── DWM still frames a window with no redirection surface ────────────────────────
    let (extended, ok) = probe::extended_frame(&window);
    let inset = extended.0 - probe::window_rect(&window).0;
    findings.push((
        "DWM still reports an extended frame, so shadow and resize survive",
        ok && inset > 0,
        format!("invisible border {inset} px on the left"),
    ));

    println!("\n  findings");
    let mut all = true;
    for (what, held, note) in &findings {
        println!(
            "    [{}] {what}\n         {note}",
            if *held { "yes" } else { "NO " }
        );
        all &= held;
    }
    println!(
        "\n  verdict: the custom caption {} behave like the system's.",
        if all { "DOES" } else { "does NOT fully" }
    );

    if hold {
        println!(
            "\n  --hold: drag the bar, double-click it, hover the maximize button for Snap \
             Layouts,\n          right-click for the system menu. Close the window to exit."
        );
        windows_window::run();
        println!("  caption state changes observed: {}", state_changes.get());
    }
    Ok(())
}

/// Everything this example needs to *ask* the window questions, and nothing it needs to
/// answer them. Kept apart so the body above reads as the experiment it is.
mod probe {
    use windows_window::Window;

    pub const SC_RESTORE: usize = 0xF120;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    type Hwnd = *mut core::ffi::c_void;

    // Declared here rather than reached through the crate: an example is allowed the
    // literal rects and raw queries that the framework's own source is linted against.
    #[link(name = "user32", kind = "raw-dylib")]
    unsafe extern "system" {
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn ClientToScreen(hwnd: Hwnd, point: *mut Point) -> i32;
        fn SendMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> *mut core::ffi::c_void;
        fn GetMonitorInfoW(monitor: *mut core::ffi::c_void, info: *mut MonitorInfo) -> i32;
        fn IsZoomed(hwnd: Hwnd) -> i32;
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
        fn WindowFromPoint(point: Point) -> Hwnd;
    }
    #[link(name = "dwmapi", kind = "raw-dylib")]
    unsafe extern "system" {
        fn DwmGetWindowAttribute(
            hwnd: Hwnd,
            attribute: u32,
            value: *mut core::ffi::c_void,
            size: u32,
        ) -> i32;
    }

    const WM_NCHITTEST: u32 = 0x0084;
    const WM_SYSCOMMAND: u32 = 0x0112;
    const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    pub fn window_rect(window: &Window) -> (i32, i32, i32, i32) {
        let mut rect = Rect::default();
        unsafe { GetWindowRect(window.hwnd(), &mut rect) };
        (rect.left, rect.top, rect.right, rect.bottom)
    }

    pub fn client_origin(window: &Window) -> (i32, i32) {
        let mut point = Point::default();
        unsafe { ClientToScreen(window.hwnd(), &mut point) };
        (point.x, point.y)
    }

    /// Asks the window the same question the system asks, at a screen point.
    pub fn hit_test(window: &Window, x: i32, y: i32) -> &'static str {
        let lparam = ((y as u32 as isize) << 16) | (x as u32 as isize & 0xffff);
        let code = unsafe { SendMessageW(window.hwnd(), WM_NCHITTEST, 0, lparam) } as i32;
        name(code)
    }

    /// Sends a non-client mouse message carrying a hit-test code, as the system does.
    /// Presses or releases the primary button, as a real input event.
    ///
    /// A synthesized `WM_NCLBUTTONDOWN` cannot drive the caption: it asks `GetPointerInfo`
    /// which button changed, and a hand-built message carries no pointer to answer about.
    pub fn mouse_button(down: bool) {
        const INPUT_MOUSE: u32 = 0;
        const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
        const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
        let input = Input {
            kind: INPUT_MOUSE,
            _pad: 0,
            mi: MouseInput {
                flags: if down {
                    MOUSEEVENTF_LEFTDOWN
                } else {
                    MOUSEEVENTF_LEFTUP
                },
                ..Default::default()
            },
        };
        unsafe { SendInput(1, &input, size_of::<Input>() as i32) };
    }

    /// Puts the window above everything, at a known place, without taking focus.
    ///
    /// A hover probe launched from a terminal otherwise hovers the terminal, and the
    /// finding then describes the harness rather than the window.
    pub fn place_on_top(window: &Window, x: i32, y: i32, w: i32, h: i32) {
        const HWND_TOPMOST: isize = -1;
        const SWP_NOACTIVATE: u32 = 0x0010;
        unsafe { SetWindowPos(window.hwnd(), HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE) };
    }

    pub fn drop_topmost(window: &Window) {
        const HWND_NOTOPMOST: isize = -2;
        const SWP_NOMOVE_NOSIZE_NOACTIVATE: u32 = 0x0002 | 0x0001 | 0x0010;
        unsafe {
            SetWindowPos(
                window.hwnd(),
                HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE_NOSIZE_NOACTIVATE,
            )
        };
    }

    pub fn cursor_position() -> (i32, i32) {
        let mut point = Point::default();
        unsafe { GetCursorPos(&mut point) };
        (point.x, point.y)
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
    struct Input {
        kind: u32,
        _pad: u32,
        mi: MouseInput,
    }

    /// Moves the pointer with a real input event. It is the user's, so every caller puts
    /// it back.
    ///
    /// `SendInput` and not `SetCursorPos`: the two are not the same instrument. A cursor
    /// warp moves the pointer and lets the window manager notice, where this injects an
    /// event into the input stack — which is the only one of the two that can answer what
    /// the input stack does with it.
    pub fn move_cursor(x: i32, y: i32) {
        const INPUT_MOUSE: u32 = 0;
        const MOUSEEVENTF_MOVE: u32 = 0x0001;
        const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
        const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;
        const SM_XVIRTUALSCREEN: i32 = 76;
        const SM_YVIRTUALSCREEN: i32 = 77;
        const SM_CXVIRTUALSCREEN: i32 = 78;
        const SM_CYVIRTUALSCREEN: i32 = 79;
        unsafe {
            let (vx, vy) = (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
            );
            let (vw, vh) = (
                GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1),
                GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1),
            );
            let input = Input {
                kind: INPUT_MOUSE,
                _pad: 0,
                mi: MouseInput {
                    dx: ((x - vx) as i64 * 65535 / vw as i64) as i32,
                    dy: ((y - vy) as i64 * 65535 / vh as i64) as i32,
                    flags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    ..Default::default()
                },
            };
            SendInput(1, &input, size_of::<Input>() as i32);
        }
    }

    /// Which window is on top at a screen point — the check that says whether a hover
    /// probe measured the window or measured whatever was covering it.
    pub fn is_on_top(window: &Window, x: i32, y: i32) -> bool {
        unsafe { WindowFromPoint(Point { x, y }) == window.hwnd() }
    }

    pub fn is_maximized(window: &Window) -> bool {
        unsafe { IsZoomed(window.hwnd()) != 0 }
    }

    pub fn system_command(window: &Window, command: usize) {
        unsafe { SendMessageW(window.hwnd(), WM_SYSCOMMAND, command, 0) };
    }

    /// The monitor's work area, in screen coordinates.
    pub fn work_area(window: &Window) -> (i32, i32, i32, i32) {
        let mut info = MonitorInfo {
            size: size_of::<MonitorInfo>() as u32,
            ..Default::default()
        };
        unsafe {
            let monitor = MonitorFromWindow(window.hwnd(), MONITOR_DEFAULTTONEAREST);
            GetMonitorInfoW(monitor, &mut info);
        }
        (
            info.work.left,
            info.work.top,
            info.work.right,
            info.work.bottom,
        )
    }

    /// How far the client area sticks out past the monitor's work area, if at all.
    pub fn work_area_overhang(window: &Window) -> (i32, i32, i32, i32) {
        let mut info = MonitorInfo {
            size: size_of::<MonitorInfo>() as u32,
            ..Default::default()
        };
        unsafe {
            let monitor = MonitorFromWindow(window.hwnd(), MONITOR_DEFAULTTONEAREST);
            GetMonitorInfoW(monitor, &mut info);
        }
        let mut client = Rect::default();
        unsafe { GetClientRect(window.hwnd(), &mut client) };
        let (ox, oy) = client_origin(window);
        let work = info.work;
        (
            (work.left - ox).max(0),
            (work.top - oy).max(0),
            (ox + client.right - client.left - work.right).max(0),
            (oy + client.bottom - client.top - work.bottom).max(0),
        )
    }

    pub fn extended_frame(window: &Window) -> ((i32, i32, i32, i32), bool) {
        let mut rect = Rect::default();
        let hr = unsafe {
            DwmGetWindowAttribute(
                window.hwnd(),
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&raw mut rect).cast(),
                size_of::<Rect>() as u32,
            )
        };
        ((rect.left, rect.top, rect.right, rect.bottom), hr >= 0)
    }

    fn name(code: i32) -> &'static str {
        match code {
            0 => "HTNOWHERE",
            1 => "HTCLIENT",
            2 => "HTCAPTION",
            8 => "HTMINBUTTON",
            9 => "HTMAXBUTTON",
            10 => "HTLEFT",
            11 => "HTRIGHT",
            12 => "HTTOP",
            13 => "HTTOPLEFT",
            14 => "HTTOPRIGHT",
            15 => "HTBOTTOM",
            16 => "HTBOTTOMLEFT",
            17 => "HTBOTTOMRIGHT",
            20 => "HTCLOSE",
            other => Box::leak(format!("HT?({other})").into_boxed_str()),
        }
    }
}
