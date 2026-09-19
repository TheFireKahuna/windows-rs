//! A turned control rotates about its own box, in the space its samples are fed in.
//!
//! The pivot a knob rotates about carries no numbers in its declaration: the input thread
//! resolves it from the hit array per contact. Whether that resolution is correct rests on
//! one platform fact that reads the same either way — the space `SetPivotCenter` wants the
//! centre in — and a centre in the wrong space rotates about the wrong point rather than
//! failing. So this drives a real contact on an arc through the real recogniser and reads
//! back what the platform reported.
//!
//! Run on an interactive desktop with
//! `cargo test -p windows-ui --test knob_pivot -- --ignored --test-threads=1`.

use std::rc::Rc;
use std::time::{Duration, Instant};

use injector::{Injector, Point, Rate, Space};
use windows_color::{DisplayCapability, OutputTransform};
use windows_scene::{ControlId, Env, HitEntry, HitFlags, HitTable, NO_ENTRY, NodeId};
use windows_ui::gesture::{GestureDecl, Recognised, pivot_of};
use windows_ui::input::{Doorbell, Report, Router};
use windows_window::Window;

/// The knob's box in client DIPs: 80 x 80, so the resolved pivot is centred at (180, 140)
/// with a radius of 40.
const BOX: (f32, f32, f32, f32) = (140.0, 100.0, 220.0, 180.0);
const CENTRE: (f32, f32) = (180.0, 140.0);
/// The contact orbits inside the pivot, so every sample is on the control and the press that
/// resolved the target is the one the whole arc stays on.
const ORBIT: f32 = 30.0;

#[test]
#[ignore = "injects touch into a foreground window; requires an interactive desktop"]
fn a_contact_on_an_arc_turns_a_knob_about_its_own_box() {
    let bell = Rc::new(Doorbell::new());
    let window = Window::new("windows-ui — knob pivot")
        .size_dips(480.0, 320.0)
        .pointer_input()
        .quit_on_close(false)
        .on_message({
            let bell = Rc::clone(&bell);
            move |_, message, wparam, lparam| bell.wndproc(message, wparam, lparam)
        })
        .create()
        .expect("create a pointer-enabled window");
    let pacer = window.pacer().expect("a window can be paced");
    let mut router = Router::new(&bell, &window, pacer.wake()).expect("the window is open");
    window.show().expect("show the window");
    // SAFETY: the handle belongs to the live window held for this whole test, and
    // `SetForegroundWindow` takes it by value and writes through no pointer.
    unsafe {
        SetForegroundWindow(window.hwnd());
    }
    pump_for(Duration::from_millis(200));

    let target = ControlId::FIRST;
    let hits = table(target);
    router.declare(target, GestureDecl::knob());
    let pivot = pivot_of(&hits, target, GestureDecl::knob()).expect("a turned control pivots");
    assert_eq!((pivot.center.x, pivot.center.y), CENTRE);
    assert!((pivot.radius - 40.0).abs() < 1.0e-6);

    let env = Env::new(
        window.metrics().expect("window metrics").dpi as f32,
        OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
    );
    let space = Space::for_window(window.hwnd()).expect("the window's own space");
    let mut reports = Vec::new();
    router.tick(&hits, env, &mut reports).expect("a first tick");

    let path = arc(48);
    let driving = injector::drive(space, move |injector: &mut Injector| {
        let mut touch = injector.touch(1)?;
        touch.down(path[0])?;
        touch.polyline(&path[1..], Rate::PerMs(4))?;
        touch.up()?;
        Ok(())
    });
    // The router consumes no native message, so the window is pumped while the drive runs and
    // ticked between pumps: a manipulation is integrated across ticks, and the cumulative
    // rotation is only whole once the contact has lifted.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut turned = 0.0_f32;
    let mut updates = 0_u32;
    while Instant::now() < deadline {
        windows_window::pump();
        reports.clear();
        router.tick(&hits, env, &mut reports).expect("a tick");
        for report in &reports {
            if let Report::Gesture {
                event:
                    Recognised::ManipulationUpdated { cumulative, .. }
                    | Recognised::ManipulationCompleted { cumulative, .. },
                ..
            } = report
            {
                turned = cumulative.rotation;
                updates += 1;
            }
        }
        if driving.is_finished() && updates > 0 {
            break;
        }
        std::thread::yield_now();
    }
    assert!(driving.is_finished(), "touch injection did not finish");
    driving
        .join()
        .expect("the drive thread")
        .expect("the drive");

    assert!(updates > 0, "the recogniser reported no manipulation");
    // Positive is clockwise, and this arc runs clockwise. The magnitude is not the arc's own
    // ninety degrees: single-pointer rotation converts tangential travel over the pivot
    // radius, so a 30 DIP orbit inside a 40 DIP pivot reports about 31, and the bound is wide
    // because the platform integrates a path the injector placed sample by sample. What the
    // bound does separate is the centre's space — the same arc about a centre displaced by a
    // thousand DIPs reports two hundredths of a degree.
    assert!(
        (10.0..90.0).contains(&turned),
        "a quarter turn clockwise about {CENTRE:?} reported {turned} degrees over {updates} \
         updates, so the pivot centre is not in the client DIPs the samples are fed in"
    );
}

/// Returns a hit array holding one turned control at [`BOX`].
fn table(target: ControlId) -> HitTable {
    let mut table = HitTable::default();
    table.replace(
        &[HitEntry {
            x0: BOX.0,
            y0: BOX.1,
            x1: BOX.2,
            y1: BOX.3,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: NO_ENTRY,
            flags: HitFlags::INTERACTIVE | HitFlags::GESTURE,
            scroll_src: NodeId::NONE,
            id: target,
        }],
        &[(target, 0)],
    );
    table
}

/// Returns a quarter turn clockwise about [`CENTRE`], from directly above it to directly
/// right of it — clockwise as the screen runs, where y grows downward.
fn arc(steps: usize) -> Vec<Point> {
    (0..=steps)
        .map(|step| {
            let angle = core::f32::consts::FRAC_PI_2 * ((step as f32 / steps as f32) - 1.0);
            Point {
                x: CENTRE.0 + ORBIT * angle.cos(),
                y: CENTRE.1 + ORBIT * angle.sin(),
            }
        })
        .collect()
}

fn pump_for(duration: Duration) {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        windows_window::pump();
        std::thread::yield_now();
    }
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SetForegroundWindow(hwnd: *mut core::ffi::c_void) -> i32;
}
