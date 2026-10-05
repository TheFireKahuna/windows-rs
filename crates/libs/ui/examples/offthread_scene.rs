//! Probe: a `Windows.UI.Composition` scene owned by a thread that is not the window's.
//!
//! The window and its pump live on the main thread. A second thread creates the dispatcher
//! queue, the compositor and the `DesktopWindowTarget` for the window's content window, and
//! then waits on an event plus its own message queue — no window message and no clock. The
//! probe drives itself from a third thread and reads the answers back off the screen:
//!
//! 1. the target renders — a red square is on screen;
//! 2. a property write made from an event-driven wake publishes — the square turns green;
//! 3. a compositor animation started the same way plays — the square springs right;
//! 4. an `InteractionTracker` owner callback lands on the scene thread — a wheel notch over
//!    the window moves the tracker, which reports `ValuesChanged` there;
//! 5. a resize while the target is foreign-owned does not fault;
//! 6. teardown in the reverse order is clean.
//!
//! The target is on the content window because the system compositor attaches the input sink
//! that routes a wheel to a `VisualInteractionSource` only to a target whose window has
//! `WS_CHILD`, and the source's visual is painted because that routing follows the compositor's
//! hit test, which finds content rather than bounds.
//!
//! Exit code 0 when every check passes, 1 otherwise, with each verdict printed.
//!
//! `--same-thread` builds the identical scene on the window's own thread, as a control.
//! `--manual` waits for a real wheel notch instead of injecting one.
//!
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use windows_composition::{
    Color, Compositor, DispatcherQueueController, RedirectionMode, SourceMode, TrackerEvent,
    VisualInteractionSource, WheelMode,
};
use windows_core::Result;
use windows_numerics::Vector3;
use windows_window::{Event, Window};

const WAIT_OBJECT_0: u32 = 0;
const INFINITE: u32 = u32::MAX;
const QS_ALLINPUT: u32 = 0x04FF;
const MWMO_INPUTAVAILABLE: u32 = 0x0004;

const CMD_NONE: u32 = 0;
const CMD_RECOLOUR: u32 = 1;
const CMD_SPRING: u32 = 2;
const CMD_NUDGE: u32 = 4;
const CMD_STOP: u32 = 3;

/// The square, in physical pixels: where it starts and where the spring takes it.
const SIDE: f32 = 200.0;
const AT: (f32, f32) = (60.0, 60.0);
const SPRUNG_X: f32 = 360.0;

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct MouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra: usize,
}

#[repr(C)]
struct Input {
    kind: u32,
    // `MOUSEINPUT` is the largest union member; padding matches the x64 layout.
    mouse: MouseInput,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn MsgWaitForMultipleObjectsEx(
        count: u32,
        handles: *const *mut core::ffi::c_void,
        milliseconds: u32,
        wake_mask: u32,
        flags: u32,
    ) -> u32;
    fn GetDC(hwnd: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    fn ReleaseDC(hwnd: *mut core::ffi::c_void, hdc: *mut core::ffi::c_void) -> i32;
    fn GetWindowRect(hwnd: *mut core::ffi::c_void, rect: *mut Rect) -> i32;
    fn GetClientRect(hwnd: *mut core::ffi::c_void, rect: *mut Rect) -> i32;
    fn ClientToScreen(hwnd: *mut core::ffi::c_void, point: *mut [i32; 2]) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
    fn SetWindowPos(
        hwnd: *mut core::ffi::c_void,
        after: *mut core::ffi::c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetPixel(hdc: *mut core::ffi::c_void, x: i32, y: i32) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}

/// What the scene thread and the driver share. Plain atomics: this is a probe, not the seam.
struct Shared {
    hwnd: AtomicUsize,
    /// The content window, which the target is bound to and input is delivered to.
    content: AtomicUsize,
    command: AtomicU32,
    wake: Event,
    scene_thread: AtomicU32,
    callback_thread: AtomicU32,
    values_changed: AtomicU32,
    ready: Event,
    /// Set by the scene thread once the tracker's position moved past zero.
    tracker_position: AtomicU64,
    failed: AtomicBool,
    wheel_messages: AtomicU32,
}

use std::sync::atomic::AtomicUsize;

fn main() -> Result<()> {
    let shared = Arc::new(Shared {
        hwnd: AtomicUsize::new(0),
        content: AtomicUsize::new(0),
        command: AtomicU32::new(CMD_NONE),
        wake: Event::auto_reset()?,
        scene_thread: AtomicU32::new(0),
        callback_thread: AtomicU32::new(0),
        values_changed: AtomicU32::new(0),
        ready: Event::auto_reset()?,
        tracker_position: AtomicU64::new(0),
        failed: AtomicBool::new(false),
        wheel_messages: AtomicU32::new(0),
    });

    let window = Window::new("offthread scene probe")
        .size_dips(640.0, 400.0)
        .pointer_input()
        .quit_on_close(true)
        .content_window({
            let shared = Arc::clone(&shared);
            move |_, message, _, _| {
                // `WM_MOUSEWHEEL` and `WM_POINTERWHEEL`: whether the injected notches reached
                // this window at all, so a silent tracker is told apart from a missed wheel.
                if message == 0x020A || message == 0x024E {
                    shared.wheel_messages.fetch_add(1, Ordering::AcqRel);
                }
                None
            }
        })
        .create()?;
    let hwnd = window.hwnd();
    shared.hwnd.store(hwnd as usize, Ordering::Release);
    let content = window.content().expect("the window was built with a content window");
    shared.content.store(content.raw() as usize, Ordering::Release);
    // SAFETY: takes no pointer.
    let window_thread = unsafe { GetCurrentThreadId() };
    println!("window thread {window_thread}");

    // `--same-thread` is the control: the identical scene built on the window's own thread,
    // pumped by the window's loop. What passes there and fails off-thread is a thread rule;
    // what fails in both is the probe's own injection.
    let same_thread = std::env::args().any(|a| a == "--same-thread");
    if same_thread {
        return control(shared, window, window_thread);
    }
    let scene = {
        let shared = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("probe-scene".into())
            .spawn(move || scene_thread(&shared))
            .expect("the scene thread starts")
    };
    // The target exists before the window is shown, as the driver would order it.
    shared.ready.wait(INFINITE);
    window.show();

    let driver = {
        let shared = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("probe-driver".into())
            .spawn(move || drive(&shared, window_thread))
            .expect("the driver starts")
    };

    windows_window::run();

    shared.command.store(CMD_STOP, Ordering::Release);
    shared.wake.signal();
    let scene_ok = scene.join().map(|r| r.is_ok()).unwrap_or(false);
    let checks_ok = driver.join().unwrap_or(false);
    drop(window);
    println!(
        "teardown {}",
        if scene_ok { "clean" } else { "FAILED" }
    );
    std::process::exit(if scene_ok && checks_ok && !shared.failed.load(Ordering::Acquire) {
        0
    } else {
        1
    });
}

/// The scene thread: owns the compositor and the target for a window it did not create.
fn scene_thread(shared: &Shared) -> Result<()> {
    // Whatever happens below, the main thread is released: a failure before `ready` would
    // otherwise park it for good behind a window nobody shows.
    struct Release<'a>(&'a Shared);
    impl Drop for Release<'_> {
        fn drop(&mut self) {
            if std::thread::panicking() {
                self.0.failed.store(true, Ordering::Release);
            }
            self.0.ready.signal();
        }
    }
    let _release = Release(shared);
    let out = scene_body(shared);
    if out.is_err() {
        shared.failed.store(true, Ordering::Release);
    }
    out
}

fn scene_body(shared: &Shared) -> Result<()> {
    // SAFETY: takes no pointer.
    let me = unsafe { GetCurrentThreadId() };
    shared.scene_thread.store(me, Ordering::Release);

    let _queue = DispatcherQueueController::create_on_current_thread()?;
    let compositor = Compositor::new()?;
    let content = shared.content.load(Ordering::Acquire) as *mut core::ffi::c_void;
    // SAFETY: the handle is live for the probe's whole run; the main thread joins this one
    // before the window drops. Whether a foreign thread may do this is what the probe asks.
    let target = match unsafe { compositor.create_desktop_window_target_for_hwnd(content, false) } {
        Ok(target) => target,
        Err(e) => {
            println!("CHECK target-on-foreign-thread: FAIL ({e})");
            return Err(e);
        }
    };
    println!("CHECK target-on-foreign-thread: created on {me}");

    let root = painted_root(&compositor, shared);
    target.set_root(&root);

    // The scrolled layer: the tracker's position drives its offset through an expression,
    // so a wheel notch that reached the tracker is visible as the square moving up.
    let content = compositor.create_container_visual();
    content.set_size(640.0, 400.0);
    root.children().insert_at_top(&content);
    let square = compositor.create_sprite_visual();
    square.set_size(SIDE, SIDE);
    square.set_offset(AT.0, AT.1, 0.0);
    square.set_brush(&compositor.create_color_brush(Color::rgb(220, 30, 30)));
    content.children().insert_at_top(&square);

    // A tracker owned here, fed by the wheel over the root. The callback thread is the
    // question: it must be this one for the design to hold.
    let source = VisualInteractionSource::for_visual(&root)?;
    // The shipping configuration: the Y axis live, wheel and touchpad redirected to the
    // source, wheel driving Y. A wheel-mode alone routes nothing.
    source.set_axis_modes(SourceMode::Disabled, SourceMode::EnabledWithInertia, SourceMode::Disabled);
    source.set_redirection_mode(RedirectionMode::TouchpadAndWheel);
    source.set_wheel_modes(WheelMode::Disabled, WheelMode::Enabled, WheelMode::Disabled)?;
    let tracker = {
        let shared_ptr: *const Shared = shared;
        // SAFETY: the tracker is dropped at the end of this function, before `shared` can.
        let shared: &'static Shared = unsafe { &*shared_ptr };
        compositor.create_interaction_tracker_with_owner(move |event| {
            if let TrackerEvent::ValuesChanged { position, .. } = event {
                // SAFETY: takes no pointer.
                let on = unsafe { GetCurrentThreadId() };
                shared.callback_thread.store(on, Ordering::Release);
                shared.values_changed.fetch_add(1, Ordering::AcqRel);
                shared
                    .tracker_position
                    .store(position.y.abs().to_bits().into(), Ordering::Release);
            }
        })?
    };
    tracker.add_source(&source)?;
    tracker.set_position_bounds(
        Vector3 { x: 0.0, y: 0.0, z: 0.0 },
        Vector3 { x: 0.0, y: 4000.0, z: 0.0 },
    );
    let follow = compositor.create_expression_animation("Vector3(0, -tracker.Position.Y, 0)");
    follow.set_reference_parameter("tracker", &tracker);
    content.start_animation("Offset", &follow);

    shared.ready.signal();

    // The loop the real scene thread will run: the event and this thread's own queue, no
    // guard timeout, no window message.
    use std::os::windows::io::{AsHandle, AsRawHandle};
    let handles = [shared.wake.as_handle().as_raw_handle()];
    loop {
        // SAFETY: one live handle, a stack array.
        let woke = unsafe {
            MsgWaitForMultipleObjectsEx(
                1,
                handles.as_ptr().cast(),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };
        if woke == WAIT_OBJECT_0 + 1 {
            // Messages for this thread: the dispatcher queue's own work and the compositor's
            // callbacks.
            if !windows_window::pump() {
                break;
            }
            continue;
        }
        match shared.command.swap(CMD_NONE, Ordering::AcqRel) {
            CMD_RECOLOUR => {
                square.set_brush(&compositor.create_color_brush(Color::rgb(30, 200, 60)));
            }
            CMD_SPRING => {
                let spring = compositor.create_spring_vector3_animation();
                spring.set_period(Duration::from_millis(250));
                spring.set_damping_ratio(1.0);
                spring.set_final_value(Vector3 {
                    x: SPRUNG_X,
                    y: AT.1,
                    z: 0.0,
                });
                square.start_animation("Offset", &spring);
            }
            CMD_NUDGE => {
                // A request from this thread: its `ValuesChanged` must come back here
                // whatever the wheel did, which separates callback delivery from routing.
                // Absolute, so the layer rests 120 up however far the wheel scrolled it.
                _ = tracker.try_update_position(
                    Vector3 { x: 0.0, y: 120.0, z: 0.0 },
                    windows_composition::Clamping::Auto,
                    windows_composition::ScaleAnimationPolicy::Keep,
                );
            }
            CMD_STOP => break,
            _ => {}
        }
    }
    drop(tracker);
    drop(source);
    drop(square);
    drop(root);
    drop(target);
    drop(compositor);
    Ok(())
}

/// Drives the checks on a timeline and reads the answers off the screen.
fn drive(shared: &Shared, window_thread: u32) -> bool {
    let hwnd = shared.hwnd.load(Ordering::Acquire) as *mut core::ffi::c_void;
    let mut ok = true;
    let mut check = |name: &str, pass: bool, detail: String| {
        println!("CHECK {name}: {} ({detail})", if pass { "PASS" } else { "FAIL" });
        ok &= pass;
    };

    std::thread::sleep(Duration::from_millis(600));
    let red = sample(hwnd, AT.0 + SIDE / 2.0, AT.1 + SIDE / 2.0);
    check("renders", is(red, (220, 30, 30)), format!("{red:?}"));

    shared.command.store(CMD_RECOLOUR, Ordering::Release);
    shared.wake.signal();
    std::thread::sleep(Duration::from_millis(400));
    let green = sample(hwnd, AT.0 + SIDE / 2.0, AT.1 + SIDE / 2.0);
    check("event-driven write publishes", is(green, (30, 200, 60)), format!("{green:?}"));

    shared.command.store(CMD_SPRING, Ordering::Release);
    shared.wake.signal();
    std::thread::sleep(Duration::from_millis(900));
    let moved = sample(hwnd, SPRUNG_X + SIDE / 2.0, AT.1 + SIDE / 2.0);
    let vacated = sample(hwnd, AT.0 + 10.0, AT.1 + SIDE / 2.0);
    check(
        "spring plays",
        is(moved, (30, 200, 60)) && !is(vacated, (30, 200, 60)),
        format!("at {moved:?}, vacated {vacated:?}"),
    );

    // A wheel notch over the window. The source is on the root, so anywhere inside counts.
    // The square's bottom edge is the witness: a routed notch scrolls it out from under
    // this point.
    let edge = (SPRUNG_X + SIDE / 2.0, AT.1 + SIDE - 15.0);
    let before = sample(hwnd, edge.0, edge.1);
    if manual() {
        wait_for_real_wheel(shared);
    } else {
        let (cx, cy) = screen_point(hwnd, 320.0, 300.0);
        // SAFETY: plain integers.
        unsafe { SetCursorPos(cx, cy) };
        std::thread::sleep(Duration::from_millis(100));
        for _ in 0..3 {
            wheel(-120);
            std::thread::sleep(Duration::from_millis(60));
        }
        std::thread::sleep(Duration::from_millis(700));
    }
    let wheeled = shared.values_changed.load(Ordering::Acquire);
    let messages = shared.wheel_messages.load(Ordering::Acquire);
    let scene = shared.scene_thread.load(Ordering::Acquire);
    let after = sample(hwnd, edge.0, edge.1);
    let scrolled = is(before, (30, 200, 60)) && !is(after, (30, 200, 60));
    check(
        "wheel routes to the tracker",
        scrolled,
        format!(
            "scrolled {scrolled} (edge {before:?} -> {after:?}); {wheeled} callbacks; {messages} wheel messages reached the window"
        ),
    );
    shared.command.store(CMD_NUDGE, Ordering::Release);
    shared.wake.signal();
    std::thread::sleep(Duration::from_millis(500));
    let callbacks = shared.values_changed.load(Ordering::Acquire);
    let on = shared.callback_thread.load(Ordering::Acquire);
    check(
        "tracker callback on scene thread",
        callbacks > wheeled && on == scene,
        format!("{callbacks} callbacks on thread {on}; scene {scene}, window {window_thread}"),
    );

    // Resize while the target is foreign-owned.
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: a stack out-parameter.
    unsafe { GetWindowRect(hwnd, &mut rect) };
    // SAFETY: plain integers; SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE.
    let resized = unsafe {
        SetWindowPos(
            hwnd,
            core::ptr::null_mut(),
            0,
            0,
            rect.right - rect.left + 120,
            rect.bottom - rect.top + 80,
            0x0002 | 0x0004 | 0x0010,
        )
    };
    std::thread::sleep(Duration::from_millis(400));
    // Sampled near the top of the square: the nudge above scrolled the layer up by 120.
    let still = sample(hwnd, SPRUNG_X + SIDE / 2.0, AT.1 + 40.0);
    check(
        "survives resize",
        resized != 0 && is(still, (30, 200, 60)),
        format!("{still:?}"),
    );

    // SAFETY: plain integers; posting the close to the window thread from here is legal.
    unsafe { post_close(hwnd) };
    ok
}

/// Returns the source's visual: a sprite covering the client area with a transparent brush.
///
/// An explicit extent, because an interaction source refuses a visual with no size of its own
/// and a relative adjustment is not one. Painted, because the compositor routes a wheel to a
/// source only over content its hit test finds, and a container paints nothing.
fn painted_root(compositor: &Compositor, shared: &Shared) -> windows_composition::SpriteVisual {
    let mut client = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    let content = shared.content.load(Ordering::Acquire) as *mut core::ffi::c_void;
    // SAFETY: the content window is live for the probe's run; the destination is a local.
    unsafe { GetClientRect(content, &mut client) };
    let root = compositor.create_sprite_visual();
    root.set_brush(&compositor.create_color_brush(Color::rgba(0, 0, 0, 0)));
    root.set_size(client.right as f32, client.bottom as f32);
    root
}

/// Posts `WM_CLOSE`, the same way a worker that must end the process would.
unsafe fn post_close(hwnd: *mut core::ffi::c_void) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn PostMessageW(hwnd: *mut core::ffi::c_void, msg: u32, w: usize, l: isize) -> i32;
    }
    // SAFETY: `PostMessageW` is callable from any thread.
    unsafe { PostMessageW(hwnd, 0x0010, 0, 0) };
}

fn wheel(delta: i32) {
    let input = Input {
        kind: 0, // INPUT_MOUSE
        mouse: MouseInput {
            dx: 0,
            dy: 0,
            mouse_data: delta as u32,
            flags: 0x0800, // MOUSEEVENTF_WHEEL
            time: 0,
            extra: 0,
        },
    };
    // SAFETY: one record of the declared size.
    unsafe { SendInput(1, &input, size_of::<Input>() as i32) };
}

/// Converts a client point to screen pixels. Composition visuals are stated in physical
/// pixels on a desktop target, so no DPI scaling applies.
fn screen_point(hwnd: *mut core::ffi::c_void, x: f32, y: f32) -> (i32, i32) {
    let mut point = [x as i32, y as i32];
    // SAFETY: a stack out-parameter.
    unsafe { ClientToScreen(hwnd, &mut point) };
    (point[0], point[1])
}

/// Reads the composed pixel under a client point, in physical pixels.
fn sample(hwnd: *mut core::ffi::c_void, x: f32, y: f32) -> (u8, u8, u8) {
    let (sx, sy) = screen_point(hwnd, x, y);
    // SAFETY: the screen DC, released below.
    unsafe {
        let dc = GetDC(core::ptr::null_mut());
        let bgr = GetPixel(dc, sx, sy);
        ReleaseDC(core::ptr::null_mut(), dc);
        ((bgr & 0xFF) as u8, ((bgr >> 8) & 0xFF) as u8, ((bgr >> 16) & 0xFF) as u8)
    }
}

/// Whether a sampled colour is the authored one, within the compositor's 8-bit rounding.
fn is(sampled: (u8, u8, u8), want: (u8, u8, u8)) -> bool {
    let near = |a: u8, b: u8| a.abs_diff(b) <= 6;
    near(sampled.0, want.0) && near(sampled.1, want.1) && near(sampled.2, want.2)
}

/// The same-thread control. Builds the tracker on the window thread and runs only the wheel
/// check against it.
fn control(shared: Arc<Shared>, window: Window, window_thread: u32) -> Result<()> {
    // The window's creation already gave this thread its dispatcher queue.
    let compositor = Compositor::new()?;
    let content = shared.content.load(Ordering::Acquire) as *mut core::ffi::c_void;
    // SAFETY: the content window is live and owned by this thread.
    let target = unsafe { compositor.create_desktop_window_target_for_hwnd(content, false)? };
    let root = painted_root(&compositor, &shared);
    target.set_root(&root);
    // The scrolled layer: the tracker's position drives its offset through an expression,
    // so a wheel notch that reached the tracker is visible as the square moving up.
    let content = compositor.create_container_visual();
    content.set_size(640.0, 400.0);
    root.children().insert_at_top(&content);
    let square = compositor.create_sprite_visual();
    square.set_size(SIDE, SIDE);
    square.set_offset(AT.0, AT.1, 0.0);
    square.set_brush(&compositor.create_color_brush(Color::rgb(220, 30, 30)));
    content.children().insert_at_top(&square);
    let source = VisualInteractionSource::for_visual(&root)?;
    // The shipping configuration: the Y axis live, wheel and touchpad redirected to the
    // source, wheel driving Y. A wheel-mode alone routes nothing.
    source.set_axis_modes(SourceMode::Disabled, SourceMode::EnabledWithInertia, SourceMode::Disabled);
    source.set_redirection_mode(RedirectionMode::TouchpadAndWheel);
    source.set_wheel_modes(WheelMode::Disabled, WheelMode::Enabled, WheelMode::Disabled)?;
    let tracker = {
        let shared = Arc::clone(&shared);
        compositor.create_interaction_tracker_with_owner(move |event| {
            if let TrackerEvent::ValuesChanged { .. } = event {
                // SAFETY: takes no pointer.
                let on = unsafe { GetCurrentThreadId() };
                shared.callback_thread.store(on, Ordering::Release);
                shared.values_changed.fetch_add(1, Ordering::AcqRel);
            }
        })?
    };
    tracker.add_source(&source)?;
    tracker.set_position_bounds(
        Vector3 { x: 0.0, y: 0.0, z: 0.0 },
        Vector3 { x: 0.0, y: 4000.0, z: 0.0 },
    );
    let follow = compositor.create_expression_animation("Vector3(0, -tracker.Position.Y, 0)");
    follow.set_reference_parameter("tracker", &tracker);
    content.start_animation("Offset", &follow);
    shared.scene_thread.store(window_thread, Ordering::Release);
    window.show();
    let driver = {
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || {
            let hwnd = shared.hwnd.load(Ordering::Acquire) as *mut core::ffi::c_void;
            std::thread::sleep(Duration::from_millis(600));
            let edge = (AT.0 + SIDE / 2.0, AT.1 + SIDE - 15.0);
            let before = sample(hwnd, edge.0, edge.1);
            if manual() {
                wait_for_real_wheel(&shared);
            } else {
                let (cx, cy) = screen_point(hwnd, 320.0, 300.0);
                // SAFETY: plain integers.
                unsafe { SetCursorPos(cx, cy) };
                std::thread::sleep(Duration::from_millis(100));
                for _ in 0..3 {
                    wheel(-120);
                    std::thread::sleep(Duration::from_millis(60));
                }
                std::thread::sleep(Duration::from_millis(700));
            }
            let wheeled = shared.values_changed.load(Ordering::Acquire);
            let messages = shared.wheel_messages.load(Ordering::Acquire);
            let after = sample(hwnd, edge.0, edge.1);
            let scrolled = is(before, (220, 30, 30)) && !is(after, (220, 30, 30));
            println!(
                "CHECK control wheel routes to the tracker (same thread): {} (scrolled {scrolled}, edge {before:?} -> {after:?}; {wheeled} callbacks; {messages} wheel messages reached the window)",
                if scrolled { "PASS" } else { "FAIL" }
            );
            // SAFETY: posting to the window thread from here is legal.
            unsafe { post_close(hwnd) };
            scrolled
        })
    };
    windows_window::run();
    let ok = driver.join().unwrap_or(false);
    drop(tracker);
    drop(source);
    drop(square);
    drop(root);
    drop(target);
    drop(compositor);
    drop(window);
    std::process::exit(if ok { 0 } else { 1 });
}

/// Under `--manual`, waits for a real wheel notch over the window instead of injecting one:
/// injected wheel input is not redirected to a tracker on any thread, so only a device can
/// answer the routing question. Returns once a callback arrived or after 20 s.
fn wait_for_real_wheel(shared: &Shared) {
    println!("MANUAL: roll the mouse wheel over the window (20 s)...");
    for _ in 0..80 {
        std::thread::sleep(Duration::from_millis(250));
        if shared.values_changed.load(Ordering::Acquire) > 0 {
            std::thread::sleep(Duration::from_millis(300));
            return;
        }
    }
}

fn manual() -> bool {
    std::env::args().any(|a| a == "--manual")
}
