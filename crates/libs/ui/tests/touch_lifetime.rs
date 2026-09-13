//! Retired touch pointers must still complete through the deferred service tick.
//!
//! Run on an interactive desktop with
//! `cargo test -p windows-ui --test touch_lifetime -- --ignored --test-threads=1`.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use injector::{Injector, Point, Space};
use windows_color::{DisplayCapability, OutputTransform};
use windows_scene::{ControlId, Env, HitEntry, HitFlags, HitTable, Ids, NO_ENTRY, NodeId};
use windows_ui::gesture::{GestureDecl, Recognised};
use windows_ui::input::{Doorbell, Report, Router};
use windows_window::Window;

#[test]
#[ignore = "injects touch into a foreground window; requires an interactive desktop"]
fn retired_taps_and_cancels_drain_in_order_and_release_the_frame_request() {
    let bell = Rc::new(Doorbell::new());
    let left = Rc::new(Cell::new(0));
    let window = Window::new("windows-ui — touch lifetime")
        .size_dips(480.0, 320.0)
        .pointer_input()
        .quit_on_close(false)
        .on_message({
            let bell = Rc::clone(&bell);
            let left = Rc::clone(&left);
            move |_, message, wparam, lparam| {
                if message == 0x024A {
                    left.set(left.get() + 1);
                }
                bell.wndproc(message, wparam, lparam)
            }
        })
        .create()
        .expect("create a pointer-enabled window");
    let pacer = window.pacer().unwrap();
    let wake = pacer.wake();
    let mut router = Router::new(&bell, &window, wake.clone()).unwrap();
    window.show().unwrap();
    // SAFETY: the handle belongs to the live window held for this entire test.
    unsafe {
        SetForegroundWindow(window.hwnd());
    }
    pump_for(Duration::from_millis(200));

    let target = Ids::<windows_scene::Control>::new().mint();
    let mut hits = HitTable::default();
    hits.replace(&[HitEntry {
        x0: 40.0,
        y0: 40.0,
        x1: 400.0,
        y1: 260.0,
        touch_inflate: 0.0,
        clip_parent: NO_ENTRY,
        parent: NO_ENTRY,
        flags: HitFlags::INTERACTIVE | HitFlags::GESTURE,
        scroll_src: NodeId::NONE,
        id: target,
    }]);
    router.declare(target, GestureDecl::tap());
    let env = Env::new(
        window.metrics().unwrap().dpi as f32,
        OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
    );
    let space = Space::for_window(window.hwnd()).unwrap();
    let mut reports = Vec::new();
    router.tick(&hits, env, &mut reports).unwrap();

    for canceled in [false, true, false, false] {
        reports.clear();
        let before = left.get();
        let driving = injector::drive(space, move |injector: &mut Injector| {
            let mut touch = injector.touch(1)?;
            touch.down(Point { x: 120.0, y: 140.0 })?;
            if canceled {
                touch.cancel()?;
            } else {
                touch.up()?;
            }
            Ok(())
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        // The window consumes every native message, including LEAVE, while the router
        // deliberately consumes none. An id lookup at the later tick is therefore too late.
        while (!driving.is_finished() || left.get() == before) && Instant::now() < deadline {
            windows_window::pump();
            std::thread::yield_now();
        }
        assert!(driving.is_finished(), "touch injection did not finish");
        driving.join().unwrap().expect("inject a complete contact");
        assert!(
            left.get() > before,
            "the pointer must retire before the service tick"
        );
        pump_for(Duration::from_millis(50));
        router
            .tick(&hits, env, &mut reports)
            .expect("consume the retained transition points");

        assert_contact(&reports, target, canceled);
        reports.clear();
        router.tick(&hits, env, &mut reports).unwrap();
        assert!(
            reports.is_empty(),
            "an ended contact reported again: {reports:?}"
        );
        assert!(bell.idle(), "the contact left pending input");
        assert_eq!(
            wake.requesters(),
            0,
            "the contact left the frame clock running"
        );
        assert_eq!(bell.health().dropped, 0);
    }
}

fn assert_contact(reports: &[Report], target: ControlId, canceled: bool) {
    for report in reports {
        if let Report::Gesture {
            event: Recognised::Tapped { at, .. },
            ..
        } = report
        {
            assert!(
                (at.x - 120.0).abs() < 1.0 && (at.y - 140.0).abs() < 1.0,
                "the retained point lost its client-DIP transform: {at:?}"
            );
        }
    }
    let presses: Vec<_> = reports
        .iter()
        .filter_map(|report| match report {
            Report::Pressed {
                target: to,
                contact,
                ..
            } if *to == target => Some(*contact),
            _ => None,
        })
        .collect();
    assert_eq!(presses.len(), 1, "one press: {reports:?}");
    let id = presses[0];
    let releases = reports
        .iter()
        .filter(|r| {
            matches!(r,
        Report::Released { contact, .. } if *contact == id)
        })
        .count();
    let cancels = reports
        .iter()
        .filter(|r| {
            matches!(r,
        Report::Canceled { contact, .. } if *contact == id)
        })
        .count();
    let taps = reports
        .iter()
        .filter(|r| {
            matches!(r,
        Report::Gesture { contact, event: Recognised::Tapped { .. }, .. } if *contact == id)
        })
        .count();
    assert_eq!(
        (releases, cancels, taps),
        if canceled { (0, 1, 0) } else { (1, 0, 1) },
        "{reports:?}"
    );
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
