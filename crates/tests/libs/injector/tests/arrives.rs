//! Asserts what this harness claims, against a real window.
//!
//! Behind `--features harness`: it drives the real input stack against a real foreground
//! window and needs an interactive session, so a headless `cargo test --all` compiles the
//! crate and runs none of this.
//!
//! The machine must be idle for the mouse section. There is one system cursor and this
//! harness is not its only source, so a hand on a real mouse adds travel to the drag whose
//! total is asserted. The assertion names which direction the total went wrong in, so a run
//! on a busy desk is not read as a defect in the stack. Touch has a device of its own, which
//! is why the fidelity claim is made there.
//!
//! One test function: `cargo test` runs test functions on separate threads, and there is one
//! input stack per session, so two streams would be one stream and two windows would compete
//! for the point every sample lands on. The sections below would otherwise be separate tests,
//! and they run in order.
//!
//! ```text
//! cargo test -p injector --features harness -- --nocapture
//! ```
#![cfg(feature = "harness")]

use std::cell::RefCell;
use std::rc::Rc;

use injector::{Injector, Point, Rate, Space, drive, zigzag};
use windows_window::Window;

/// One pointer arrival: which device it came from, and every sample it carried.
#[derive(Clone)]
struct Arrival {
    ptype: u32,
    /// Oldest first. One entry for a message the pump kept up with, more for one it did not.
    samples: Vec<(i32, i32)>,
}

const PT_MOUSE: u32 = 4;
const PT_TOUCH: u32 = 2;

/// How far off the straight line the fidelity paths deviate, in DIPs. Large enough that
/// losing one sample cuts a corner the assertion can see, small enough to stay inside a
/// target.
const AMPLITUDE: f32 = 6.0;

#[test]
fn injected_input_arrives_as_pointer_input() {
    let arrivals: Rc<RefCell<Vec<Arrival>>> = Rc::new(RefCell::new(Vec::new()));
    let legacy = Rc::new(std::cell::Cell::new(0u32));
    let unconsumed = Rc::new(std::cell::Cell::new(0u32));

    let window = Window::new("injector — arrives")
        .size_dips(720.0, 420.0)
        .pointer_input()
        .touchpad_capable()
        .on_message({
            let arrivals = Rc::clone(&arrivals);
            let legacy = Rc::clone(&legacy);
            let unconsumed = Rc::clone(&unconsumed);
            move |_, message, wparam, _| {
                if observe::is_legacy(message) {
                    legacy.set(legacy.get() + 1);
                    return None;
                }
                if !observe::is_pointer(message) {
                    return None;
                }
                let Some(arrival) = observe::pointer((wparam & 0xFFFF) as u32) else {
                    // A pointer message whose pointer cannot be read falls through to
                    // `DefWindowProc`, which promotes it into a legacy mouse message.
                    unconsumed.set(unconsumed.get() + 1);
                    return None;
                };
                arrivals.borrow_mut().push(arrival);
                Some(0)
            }
        })
        .create()
        .expect("creating the window");
    _ = window.show();
    // Injected absolute input lands on whatever window is at that screen point, which is a
    // z-order question rather than a focus one. Without this, a run under a terminal that
    // happens to overlap the target reads zero arrivals and looks exactly like a stack that
    // delivered nothing.
    observe::foreground(window.hwnd());
    settle(&window, 400);

    let space = Space::for_window(window.hwnd()).expect("aiming at the window");
    println!(
        "{:#?}",
        Injector::new(space).expect("an injector").capability()
    );

    // ── Touch arrives, and carries every sample it was given ──────────────────
    //
    // The fidelity claim is made on touch rather than mouse: a touch contact carries its own
    // position and its own device, so it is exact by construction and no physical device at
    // the machine interferes with it.
    // One drive, start to finish, with nothing pre-positioned: a stream lifts its contacts
    // when it drops, so a separate "finger down" step leaves nothing behind and the measured
    // path would start one sample later than the length it is compared against.
    const TOUCH_FROM: Point = Point::new(150.0, 120.0);
    let path = zigzag(TOUCH_FROM, Point::new(150.0, 300.0), 30, AMPLITUDE);
    let asked = space.placed_length(&drive_from(TOUCH_FROM, &path));
    arrivals.borrow_mut().clear();
    run(&window, space, move |injector: &mut Injector| {
        let mut touch = injector.touch(2)?;
        touch
            .down(TOUCH_FROM)?
            .polyline(&path, Rate::PerMs(2))?
            .up()?;
        Ok(())
    });

    let touch = drained(&arrivals, PT_TOUCH);
    assert!(
        !touch.is_empty(),
        "an injected touch drag produced no PT_TOUCH input at all"
    );
    // Even against a pumping window the platform coalesces touch frames, so the count
    // asserted on is the sample history rather than the message count.
    let samples: usize = touch.iter().map(|arrival| arrival.samples.len()).sum();
    assert!(
        samples > touch.len(),
        "{} messages carried {samples} samples, so nothing was coalesced and the batch this \
         design rests on was never exercised",
        touch.len()
    );
    // Every point of the drive is a corner, so a lost sample cuts one and shortens the
    // integral.
    let carried = integral(&touch, space.scale());
    assert!(
        (carried - asked).abs() < 1.0,
        "a touch drive of {asked:.1} DIPs arrived carrying {carried:.1} across {} messages \
         and {samples} samples",
        touch.len()
    );

    // ── The mouse places its samples exactly ──────────────────────────────────
    //
    // Opening the stream verifies the desktop's absolute-coordinate mapping and fails rather
    // than correcting, so reaching this line is the claim.
    const MOUSE_FROM: Point = Point::new(100.0, 60.0);
    // The cursor is placed on the start point before the count begins, because a mouse
    // contact starts where the pointer already is: without this the drag's first sample is
    // the jump from wherever the cursor happened to be.
    run(&window, space, |injector: &mut Injector| {
        injector.mouse()?.move_to(MOUSE_FROM)?;
        Ok(())
    });
    arrivals.borrow_mut().clear();

    let path = zigzag(MOUSE_FROM, Point::new(400.0, 60.0), 40, AMPLITUDE);
    let asked_mouse = space.placed_length(&drive_from(MOUSE_FROM, &path));
    run(&window, space, move |injector: &mut Injector| {
        let mut mouse = injector.mouse()?;
        mouse
            .down(MOUSE_FROM)?
            .polyline(&path, Rate::PerMs(2))?
            .up()?;
        Ok(())
    });

    let drag = drained(&arrivals, PT_MOUSE);
    assert!(
        !drag.is_empty(),
        "an injected mouse drag produced no PT_MOUSE input at all"
    );
    // The two directions are different findings, and the messages below say which. Short
    // means samples were dropped, the only cause that shortens a path whose every point is a
    // corner. Long cannot come from the stack, which has no way to invent travel, so it is
    // another input source moving the same cursor.
    let carried = integral(&drag, space.scale());
    assert!(
        carried >= asked_mouse - 1.0,
        "a drive of {asked_mouse:.1} DIPs arrived carrying only {carried:.1} across {} \
         messages: samples were dropped between the injection and the window",
        drag.len()
    );
    assert!(
        carried <= asked_mouse + 1.0,
        "a drive of {asked_mouse:.1} DIPs arrived carrying {carried:.1} across {} messages, \
         which is more travel than was injected — something else moved the cursor during the \
         run. Injection tests need an idle machine",
        drag.len()
    );
    println!("touch drive asked {asked:.1} DIPs; mouse drive asked {asked_mouse:.1}");

    // ── No legacy mouse message is ever handled ───────────────────────────────
    //
    // The assertion is that every pointer message was consumed, not that no legacy message
    // arrives: the system's cursor resynchronisation posts those on its own account, and no
    // application prevents it. A pointer message that falls through is one `DefWindowProc`
    // promotes into a legacy one.
    assert_eq!(
        unconsumed.get(),
        0,
        "{} pointer messages fell through to DefWindowProc",
        unconsumed.get()
    );
    println!(
        "legacy mouse messages observed: {} — all unhandled, none promoted from a pointer \
         message",
        legacy.get()
    );

    // ── A virtual input device is refused, and says why ───────────────────────
    //
    // Pen and touchpad contacts need `inputInjectionBrokered`, which is declared in a package
    // manifest, so an unpackaged test process cannot have it. The streams refuse rather than
    // run, because every call on that path returns success and delivers nothing.
    let mut injector = Injector::new(space).expect("an injector");
    if !injector.capability().packaged {
        let refused = injector.pen().err().map(|error| error.to_string());
        assert!(
            refused.is_some_and(|why| why.contains("inputInjectionBrokered")),
            "an unpackaged process opened a pen stream, which means either this process gained \
             package identity or the refusal stopped naming the capability"
        );
    }
}

/// The whole path the window will see: where the contact starts, then every sample.
fn drive_from(start: Point, path: &[Point]) -> Vec<Point> {
    let mut whole = Vec::with_capacity(path.len() + 1);
    whole.push(start);
    whole.extend_from_slice(path);
    whole
}

/// Drains everything that arrived, keeping only arrivals from device `ptype`.
fn drained(arrivals: &Rc<RefCell<Vec<Arrival>>>, ptype: u32) -> Vec<Arrival> {
    arrivals
        .borrow_mut()
        .drain(..)
        .filter(|arrival| arrival.ptype == ptype)
        .collect()
}

/// Returns the length of the path the arrivals describe, in DIPs, over every sample they
/// carried.
fn integral(arrivals: &[Arrival], scale: f32) -> f32 {
    let samples: Vec<(i32, i32)> = arrivals
        .iter()
        .flat_map(|arrival| arrival.samples.iter().copied())
        .collect();
    samples
        .windows(2)
        .map(|pair| ((pair[1].0 - pair[0].0) as f32).hypot((pair[1].1 - pair[0].1) as f32))
        .sum::<f32>()
        / scale
}

/// Drives on a thread of its own while this one pumps, then settles.
///
/// The pump runs during the drive: a window that is not reading its queue lets the platform
/// coalesce what arrives, and for mouse that loses the samples outright. See
/// [`injector::drive`].
fn run<F>(window: &Window, space: Space, what: F)
where
    F: FnOnce(&mut Injector) -> injector::Result<()> + Send + 'static,
{
    let driving = drive(space, what);
    while !driving.is_finished() && window.is_open() {
        windows_window::pump();
        std::thread::yield_now();
    }
    driving
        .join()
        .expect("the drive thread panicked")
        .expect("the drive");
    settle(window, 300);
}

/// Pumps for roughly `ms`, so the window sees what was injected before it is asked about it.
fn settle(window: &Window, ms: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < deadline && window.is_open() {
        windows_window::pump();
        std::thread::yield_now();
    }
}

/// The reading side: the message constants and pointer accessors the injector does not carry.
///
/// The injector's binding filter names no message constant and no pointer accessor, so this
/// file declares its own.
// The names are the platform's, so they are spelled the platform's way.
#[expect(non_snake_case)]
mod observe {
    use super::Arrival;

    windows_core::link!("user32.dll" "system" fn GetPointerInfo(pointerid : u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn GetPointerInfoHistory(pointerid : u32, entriescount : *mut u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn SetForegroundWindow(hwnd : *mut core::ffi::c_void) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn SetWindowPos(hwnd : *mut core::ffi::c_void, after : *mut core::ffi::c_void, x : i32, y : i32, cx : i32, cy : i32, flags : u32) -> windows_core::BOOL);

    /// The prefix of `POINTER_INFO` this file reads.
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

    /// Returns the device and the raw samples behind a pointer message.
    ///
    /// Raw, not predicted: the predicted position is an extrapolation the system added, so
    /// what was injected is compared against what was reported.
    pub fn pointer(id: u32) -> Option<Arrival> {
        let mut info = POINTER_INFO::default();
        // SAFETY: the destination is a stack local the call writes back through.
        unsafe { GetPointerInfo(id, &mut info) }
            .as_bool()
            .then(|| Arrival {
                ptype: info.pointerType,
                samples: match history(id).as_slice() {
                    // A pointer whose history cannot be read still carried this one sample.
                    [] => vec![(info.ptPixelLocationRaw[0], info.ptPixelLocationRaw[1])],
                    read => read.to_vec(),
                },
            })
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

    pub fn foreground(hwnd: *mut core::ffi::c_void) {
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
}
