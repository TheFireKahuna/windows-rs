//! Measures whether `DwmDefWindowProc` draws and hit-tests the caption buttons on a window
//! with **no redirection surface**.
//!
//! The documented recipe — extend the frame into the client area, remove the standard frame
//! in `WM_NCCALCSIZE`, pass the non-client messages to `DwmDefWindowProc` first — is
//! written for a window that has a redirection bitmap for DWM to draw into, and this window
//! has none. Where the recipe holds anyway, the caption keeps its drag strip, DWM takes the
//! three window commands, and no legacy mouse message is read.
//!
//! The DWM entry points are declared here and routed from this example's own message
//! handler, which runs ahead of the caption's.
//!
//! ```text
//! cargo run -p windows-window --example dwm_frame
//! cargo run -p windows-window --example dwm_frame -- --hold   # leave it up to look at
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use windows_composition::{Color, Compositor};
use windows_numerics::Vector2;
use windows_window::{CaptionButtons, CaptionHit, CaptionSpec, CornerPreference, Result, Window};

const BAR_H: f32 = 32.0;

#[derive(Copy, Clone, Default, Debug)]
struct Tally {
    /// Non-client messages `DwmDefWindowProc` claimed.
    dwm_handled: u32,
    nc_mouse_move: u32,
    nc_lbutton_down: u32,
    nc_mouse_leave: u32,
}

fn main() -> Result<()> {
    let hold = std::env::args().any(|a| a == "--hold");
    let tally = Rc::new(Cell::new(Tally::default()));
    let extended = Rc::new(Cell::new(false));
    // Re-applied on every activation, so an arm switches margins by setting this and poking
    // the window rather than by rebuilding it.
    let margins = Rc::new(Cell::new((-1, -1, -1, -1)));

    let window = Window::new("windows-window — DWM custom frame")
        .size(900, 600)
        .no_redirection_bitmap()
        .pointer_input()
        .on_message({
            let tally = Rc::clone(&tally);
            let extended = Rc::clone(&extended);
            let margins = Rc::clone(&margins);
            move |hwnd, message, wparam, lparam| {
                let mut counts = tally.get();
                match message {
                    dwm::WM_NCMOUSEMOVE => counts.nc_mouse_move += 1,
                    dwm::WM_NCLBUTTONDOWN => counts.nc_lbutton_down += 1,
                    dwm::WM_NCMOUSELEAVE => counts.nc_mouse_leave += 1,
                    // `WM_ACTIVATE` rather than `WM_CREATE`, so a window that opens
                    // maximized extends its frame through the same path.
                    dwm::WM_ACTIVATE => {
                        let hr = dwm::extend_frame(hwnd, margins.get());
                        extended.set(hr >= 0);
                    }
                    _ => {}
                }

                // DWM sees every non-client message first. It answers only over its own
                // caption buttons and declines elsewhere, which leaves the drag strip and
                // the resize edges to the caption.
                if matches!(
                    message,
                    dwm::WM_NCHITTEST
                        | dwm::WM_NCMOUSEMOVE
                        | dwm::WM_NCMOUSELEAVE
                        | dwm::WM_NCLBUTTONDOWN
                        | dwm::WM_NCLBUTTONUP
                ) && let Some(result) = dwm::def_window_proc(hwnd, message, wparam, lparam)
                {
                    counts.dwm_handled += 1;
                    tally.set(counts);
                    return Some(result);
                }
                tally.set(counts);
                None
            }
        })
        .custom_caption(CaptionSpec {
            height: Some(BAR_H),
            corners: CornerPreference::Round,
            // The caption declares no buttons of its own; DWM supplies them or nothing does.
            buttons: CaptionButtons {
                minimize: false,
                maximize: false,
                close: false,
            },
        })
        .create()?;

    // Drag or client only: every window command belongs to DWM here.
    window
        .on_caption_hit(|_, y| {
            if y < BAR_H {
                CaptionHit::Drag
            } else {
                CaptionHit::Client
            }
        })
        .expect("a window with a caption of its own");

    // Opaque content covering the whole window, so a capture shows whether DWM's buttons
    // are drawn over what the application composes.
    let compositor = Compositor::new()?;
    // SAFETY: `window` owns the handle and destroys it only on drop, so the target is bound
    // to a live window, on the thread that created it.
    let target = unsafe { compositor.create_desktop_window_target_for_hwnd(window.hwnd(), false)? };
    let root = compositor.create_container_visual();
    root.set_relative_size_adjustment(Vector2 { x: 1.0, y: 1.0 });
    let body = compositor.create_sprite_visual();
    body.set_relative_size_adjustment(Vector2 { x: 1.0, y: 1.0 });
    body.set_brush(&compositor.create_color_brush(Color::rgb(24, 26, 32)));
    root.children().insert_at_top(&body);
    // A lighter strip where the application's own bar would be, so the capture shows
    // whether DWM's buttons land on top of it.
    let strip = compositor.create_sprite_visual();
    strip.set_relative_size_adjustment(Vector2 { x: 1.0, y: 0.0 });
    strip.set_size(0.0, band_px(&window));
    strip.set_brush(&compositor.create_color_brush(Color::rgb(64, 92, 160)));
    root.children().insert_at_top(&strip);
    target.set_root(&root);

    dwm::place(&window, 100, 100, 900, 600);
    settle(200);
    dwm::click(&window, 400, 500);
    settle(300);
    dwm::drop_topmost(&window);
    settle(200);

    let metrics = window.metrics().expect("an open window");
    let rect = dwm::window_rect(&window);
    println!(
        "  window {rect:?}, dpi {}, frame extended: {}",
        metrics.dpi,
        extended.get()
    );

    // ── 1. does DWM report caption buttons at all? ───────────────────────────────────
    let bounds = dwm::caption_button_bounds(&window);
    let has_bounds = bounds.is_some_and(|b| b.2 > b.0 && b.3 > b.1);
    println!("  DWMWA_CAPTION_BUTTON_BOUNDS: {bounds:?}");

    // ── 2. does DwmDefWindowProc claim the hit test over them? ───────────────────────
    //
    // Asked directly rather than through a sent message, so the answer is DWM's alone and
    // not something the caption could have produced.
    println!("\n  DwmDefWindowProc(WM_NCHITTEST) across the top edge");
    let y = rect.1 + metrics.px(BAR_H) / 2;
    let mut claimed = Vec::new();
    for fraction in [0.10, 0.30, 0.50, 0.70, 0.85, 0.90, 0.94, 0.97, 0.99] {
        let x = rect.0 + ((rect.2 - rect.0) as f32 * fraction) as i32;
        let answer = dwm::hit_test(&window, x, y);
        if let Some(code) = answer {
            claimed.push((x, code));
        }
        println!(
            "    x {:>5} ({:>4.0}% across)  ->  {}",
            x,
            fraction * 100.0,
            match answer {
                Some(code) => format!("DWM claims it, {}", ht_name(code)),
                None => "declined".to_owned(),
            }
        );
    }

    // ── 3. with the frame extended, hover where DWM says its buttons are ─────────────
    let mut flyout = None;
    let mut hover_counts = Tally::default();
    if let Some((bl, bt, br, bb)) = bounds {
        // The bounds are relative to the window's top-left.
        let centre = (rect.0 + (bl + br) / 2, rect.1 + (bt + bb) / 2);
        // The bounds cover all three buttons, which run minimize, maximize, close from the
        // left, so the probe aims into the right half of the cluster.
        let button_w = (br - bl) / 3;
        let maximize = (centre.0 + button_w / 2, centre.1);
        println!("\n  hovering DWM's maximize button at {maximize:?}");
        dwm::move_cursor(rect.0 + 300, rect.1 + 400);
        settle(250);
        tally.set(Tally::default());
        for step in 0..6 {
            dwm::move_cursor(maximize.0 - step, maximize.1);
            settle(60);
        }
        settle(1800);
        hover_counts = tally.get();
        flyout = dwm::snap_flyout();
        println!(
            "    DWM handled {} non-client messages; NCMOUSEMOVE {}, flyout {:?}",
            hover_counts.dwm_handled, hover_counts.nc_mouse_move, flyout
        );
        dwm::move_cursor(rect.0 + 300, rect.1 + 400);
        settle(300);
    }

    println!("\n  answers");
    answer(
        "DWM reports caption-button bounds on a window with no redirection surface",
        has_bounds,
        format!("{bounds:?}"),
    );
    answer(
        "DwmDefWindowProc claims the hit test over them",
        !claimed.is_empty(),
        format!(
            "claimed at {} of 9 probe points: {claimed:?}",
            claimed.len()
        ),
    );
    answer(
        "DWM handles the non-client messages while the pointer is on a button",
        hover_counts.dwm_handled > 0,
        format!(
            "handled {}, of which our own reads were NCMOUSEMOVE {}",
            hover_counts.dwm_handled, hover_counts.nc_mouse_move
        ),
    );
    answer(
        "the Snap Layouts flyout opens over DWM's own maximize button",
        flyout.is_some(),
        format!("{flyout:?}"),
    );

    if hold {
        println!("\n  --hold: window left up for 12 s — look for the three caption buttons.");
        let deadline = 1200;
        for _ in 0..deadline {
            windows_window::pump();
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}

fn answer(claim: &str, held: bool, note: String) {
    println!(
        "    [{}] {claim}\n         {note}",
        if held { "yes" } else { "NO " }
    );
}

/// Returns the caption band's height in physical pixels, for the stand-in strip.
fn band_px(window: &Window) -> f32 {
    window.metrics().expect("an open window").px(BAR_H) as f32
}

/// Pumps for `ms` milliseconds, so the window services the input just injected.
fn settle(ms: u64) {
    for _ in 0..(ms / 10).max(1) {
        windows_window::pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn ht_name(code: isize) -> String {
    match code {
        0 => "HTNOWHERE".into(),
        1 => "HTCLIENT".into(),
        2 => "HTCAPTION".into(),
        8 => "HTMINBUTTON".into(),
        9 => "HTMAXBUTTON".into(),
        20 => "HTCLOSE".into(),
        other => format!("HT?({other})"),
    }
}

/// The DWM and user32 entry points this example calls, declared here because none of them
/// is in the crate's binding filter.
mod dwm {
    use windows_window::Window;

    pub const WM_NCHITTEST: u32 = 0x0084;
    pub const WM_NCMOUSEMOVE: u32 = 0x00A0;
    pub const WM_NCLBUTTONDOWN: u32 = 0x00A1;
    pub const WM_NCLBUTTONUP: u32 = 0x00A2;
    pub const WM_NCMOUSELEAVE: u32 = 0x02A2;
    pub const WM_ACTIVATE: u32 = 0x0006;

    const DWMWA_CAPTION_BUTTON_BOUNDS: u32 = 5;

    type Hwnd = *mut core::ffi::c_void;

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
    struct Margins {
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
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

    #[link(name = "dwmapi", kind = "raw-dylib")]
    unsafe extern "system" {
        fn DwmDefWindowProc(
            hwnd: Hwnd,
            msg: u32,
            wparam: usize,
            lparam: isize,
            result: *mut isize,
        ) -> i32;
        fn DwmExtendFrameIntoClientArea(hwnd: Hwnd, margins: *const Margins) -> i32;
        fn DwmGetWindowAttribute(hwnd: Hwnd, attribute: u32, value: *mut Rect, size: u32) -> i32;
    }

    #[link(name = "user32", kind = "raw-dylib")]
    unsafe extern "system" {
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn SetWindowPos(
            hwnd: Hwnd,
            after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn GetSystemMetrics(index: i32) -> i32;
        fn EnumWindows(
            callback: unsafe extern "system" fn(Hwnd, isize) -> i32,
            param: isize,
        ) -> i32;
        fn GetClassNameW(hwnd: Hwnd, buffer: *mut u16, len: i32) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
    }

    /// Returns `Some(result)` when DWM handled the message itself, and `None` when it
    /// declined and the caller still owns the message.
    pub fn def_window_proc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> Option<isize> {
        let mut result = 0isize;
        // SAFETY: `DwmDefWindowProc` accepts any handle value, forwards `wparam` and
        // `lparam` to the window procedure it stands in for, and writes only through
        // `result`, a live stack local.
        let handled = unsafe { DwmDefWindowProc(hwnd, msg, wparam, lparam, &mut result) };
        (handled != 0).then_some(result)
    }

    /// Extends the frame into the client area by `margins`, given as left, right, top,
    /// bottom.
    ///
    /// All four fields at `-1` is the sheet-of-glass form; all four at `0` extends nothing.
    pub fn extend_frame(hwnd: Hwnd, margins: (i32, i32, i32, i32)) -> i32 {
        let margins = Margins {
            left: margins.0,
            right: margins.1,
            top: margins.2,
            bottom: margins.3,
        };
        // SAFETY: `DwmExtendFrameIntoClientArea` accepts any handle value and reads
        // `margins`, a live local with the `MARGINS` layout, for the duration of the call.
        unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) }
    }

    /// Returns where DWM says its own caption buttons are, relative to the window's
    /// top-left, or `None` where it reports nothing.
    pub fn caption_button_bounds(window: &Window) -> Option<(i32, i32, i32, i32)> {
        let mut rect = Rect::default();
        // SAFETY: `DWMWA_CAPTION_BUTTON_BOUNDS` reports a `RECT`, and the size passed is
        // `size_of::<Rect>()`, which is what bounds the write into `rect`.
        let hr = unsafe {
            DwmGetWindowAttribute(
                window.hwnd(),
                DWMWA_CAPTION_BUTTON_BOUNDS,
                &mut rect,
                size_of::<Rect>() as u32,
            )
        };
        (hr >= 0).then_some((rect.left, rect.top, rect.right, rect.bottom))
    }

    /// Returns DWM's own hit-test answer for a screen point, or `None` where it declines
    /// the point.
    pub fn hit_test(window: &Window, x: i32, y: i32) -> Option<isize> {
        let lparam = ((y as u32 as isize) << 16) | (x as u32 as isize & 0xffff);
        def_window_proc(window.hwnd(), WM_NCHITTEST, 0, lparam)
    }

    /// Returns the window's outer rectangle in screen coordinates.
    pub fn window_rect(window: &Window) -> (i32, i32, i32, i32) {
        let mut rect = Rect::default();
        // SAFETY: `GetWindowRect` accepts any handle value and writes only through `rect`,
        // a live stack local of the size it expects.
        unsafe { GetWindowRect(window.hwnd(), &mut rect) };
        (rect.left, rect.top, rect.right, rect.bottom)
    }

    /// Places the window at a screen position in the topmost band, without activating it.
    pub fn place(window: &Window, x: i32, y: i32, w: i32, h: i32) {
        // SAFETY: `SetWindowPos` accepts any handle value and takes only integers besides
        // it.
        unsafe { SetWindowPos(window.hwnd(), -1, x, y, w, h, 0x0010) };
    }

    /// Leaves the topmost band, keeping the window's position and size.
    pub fn drop_topmost(window: &Window) {
        // SAFETY: `SetWindowPos` accepts any handle value and takes only integers besides
        // it.
        unsafe { SetWindowPos(window.hwnd(), -2, 0, 0, 0, 0, 0x0002 | 0x0001 | 0x0010) };
    }

    fn send(input: Input) {
        // SAFETY: `SendInput` reads `size_of::<Input>()` bytes from `input`, a live local
        // whose `#[repr(C)]` layout is the `INPUT` it expects, and the size passed says so.
        unsafe { SendInput(1, &input, size_of::<Input>() as i32) };
    }

    /// Moves the pointer to a screen point with an injected input event.
    pub fn move_cursor(x: i32, y: i32) {
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
            mi: MouseInput {
                dx: ((x - vx) as i64 * 65535 / vw as i64) as i32,
                dy: ((y - vy) as i64 * 65535 / vh as i64) as i32,
                flags: 0x0001 | 0x8000 | 0x4000,
                ..Default::default()
            },
        });
    }

    /// Clicks a point given relative to the window's top-left.
    ///
    /// `SetForegroundWindow` is refused to a process that owns no recent input, so a click
    /// is how this one becomes foreground.
    pub fn click(window: &Window, x: i32, y: i32) {
        let rect = window_rect(window);
        move_cursor(rect.0 + x, rect.1 + y);
        for flags in [0x0002u32, 0x0004] {
            send(Input {
                kind: 0,
                _pad: 0,
                mi: MouseInput {
                    flags,
                    ..Default::default()
                },
            });
        }
    }

    use std::sync::Mutex;
    static FOUND: Mutex<Option<String>> = Mutex::new(None);

    /// Returns the class name of the Snap Layouts flyout while it is open, matching the two
    /// names the shell gives that separate window.
    pub fn snap_flyout() -> Option<String> {
        /// # Safety
        ///
        /// `hwnd` must be a window handle `EnumWindows` supplied for the enumeration in
        /// progress.
        unsafe extern "system" fn visit(hwnd: Hwnd, _: isize) -> i32 {
            // SAFETY: `hwnd` names a live window for the duration of the callback, and
            // `GetClassNameW` writes at most `buffer.len()` code units into `buffer`.
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
                    *FOUND.lock().expect("no panic holds this lock") = Some(class);
                    return 0;
                }
            }
            1
        }
        *FOUND.lock().expect("no panic holds this lock") = None;
        // SAFETY: `visit` has the signature `EnumWindows` calls, and every handle it passes
        // is one this enumeration produced.
        unsafe { EnumWindows(visit, 0) };
        FOUND.lock().expect("no panic holds this lock").clone()
    }
}
