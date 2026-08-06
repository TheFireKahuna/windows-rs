//! Drives scroll and virtualization against a real compositor and prints a census of the run.
//!
//! The run measures three properties that only a live compositor supplies:
//!
//! 1. A fling holds the frame clock open. A tracker reports from another process with no
//!    input behind it, and the queue its callbacks push into keeps the clock running until it
//!    drains. Without that the realization window is read at whatever unrelated wake comes
//!    next, and the fling lands on rows nothing realized.
//! 2. A wheel notch costs the front thread nothing. `PointerWheelConfig` routes the notch to
//!    the tracker, so the position moves with no [`Report::Wheel`] on the front thread.
//! 3. The realized set stays bounded across ten thousand rows. The realized-row count is
//!    reported at rest and at its peak.
//!
//! The run is the shipping one: [`Ui::run`] owns the window, the tick and their order, and
//! the census is an observer over it. A loop written here to watch the tick would be a second
//! implementation of that order, and the numbers it printed would be its own.
//!
//! Scroll with the wheel, drag the thumb, then press Q; the summary prints on the way out.

use std::cell::Cell;
use std::rc::Rc;

use windows_color::{Ictcp, Radiance};
use windows_composition::Compositor;
use windows_core::Result;
use windows_d2d::Gpu;
use windows_scene::{BackdropSpec, Backends, Census, SceneEvent};
use windows_text::{FamilyId, FontLadder, FontSpec};
use windows_ui::build::mount;
use windows_ui::driver::{Ui, observe};
use windows_ui::input::Report;
use windows_ui::layout::{ListSpec, list};
use windows_ui::role::{
    AccentId, DataRole, Density, Fill, Metric, Palette, Polarity, Scope, Stroke, Text, TypeRole,
    WidthClass,
};
use windows_ui::widget::label;
use windows_window::Window;

/// Rows in the list, enough that realizing all of them would be plain in the census.
const ROWS: usize = 10_000;

/// `WM_KEYDOWN`, `WM_MOUSEWHEEL` and `WM_POINTERWHEEL`: the three raw messages this example
/// reads before the doorbell classifies them.
const WM_KEYDOWN: u32 = 0x0100;
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_POINTERWHEEL: u32 = 0x024E;

fn main() -> Result<()> {
    let ui = Ui::install(&REFERENCE, AccentId(0), Density::Comfortable);
    let seen = Rc::new(Seen::default());
    // Raw wheel messages, counted before the doorbell. A notch the compositor took and a
    // notch that never arrived both read as zero front-thread reports; this count separates
    // them.
    let wheel_messages = Rc::new(Cell::new(0u32));

    observe({
        let seen = Rc::clone(&seen);
        move |events, reports, census| seen.tick(events, reports, census)
    });

    let window = Window::new("windows-ui — scroll and virtualization")
        .size_dips(520.0, 640.0)
        .pointer_input()
        .touchpad_capable()
        .quit_on_close(true)
        // Chained ahead of the driver's own, which answers `WM_FRAME` and hands everything
        // else to the doorbell. Returning `None` is what lets both run.
        .on_message({
            let wheel_messages = Rc::clone(&wheel_messages);
            let mut pending_drive = std::env::args().any(|arg| arg == "--drive");
            move |hwnd, message, wparam, _| {
                if message == WM_MOUSEWHEEL || message == WM_POINTERWHEEL {
                    wheel_messages.set(wheel_messages.get() + 1);
                }
                // The first message is where this example learns the handle: the driver owns
                // the window and hands it to nothing here.
                if pending_drive {
                    pending_drive = false;
                    let hwnd = hwnd as usize;
                    std::thread::spawn(move || drive(hwnd));
                }
                if message == WM_KEYDOWN && wparam == b'Q' as usize {
                    windows_window::quit();
                    return Some(0);
                }
                None
            }
        });

    println!(
        "{ROWS} rows, {} DIP each. Wheel over the list, drag the thumb, then Q.\n",
        windows_ui::role::metric(Metric::RowH, ui.root_scope())
    );

    ui.run(
        window,
        || {
            let compositor = Compositor::new()?;
            let gpu = Gpu::for_window()?;
            Backends::new(
                compositor,
                &gpu,
                FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]),
            )
        },
        BackdropSpec::default(),
        |root| {
            mount(
                list(
                    || ListSpec::uniform(ROWS, Metric::RowH),
                    |realized, out| {
                        for run in realized.runs() {
                            out.extend(run.map(|index| (index, index)));
                        }
                    },
                    |index: &usize| label(format!("row {index}")),
                )
                // The root is a full-client stretching column, so the list states its share
                // of the main axis and nothing about the window's extent.
                .grow(),
                root,
            )
        },
    )?;

    seen.report();
    println!("  raw wheel messages           {}", wheel_messages.get());
    Ok(())
}

/// Counts what the run observed, so the summary reports measurements rather than intent.
///
/// Every field is a [`Cell`], because the observer holds this shared and is handed each tick
/// by reference.
#[derive(Default)]
struct Seen {
    ticks: Cell<u32>,
    values: Cell<u32>,
    inertia: Cell<u32>,
    wheel_reports: Cell<u32>,
    drags: Cell<u32>,
    /// Highest realized-row count seen during the run.
    realized_max: Cell<usize>,
    /// Realized-row count at the last tick that drained no scene events.
    realized_rest: Cell<usize>,
    reached: Cell<f32>,
    /// Trackers alive at the last tick. Zero is a scroll container bound to nothing.
    trackers: Cell<u32>,
}

impl Seen {
    /// Records one tick. Reads its arguments and holds none of them, so the census allocates
    /// nothing on the frame path.
    fn tick(&self, events: &[SceneEvent], reports: &[Report], census: Census) {
        self.ticks.set(self.ticks.get() + 1);
        for event in events {
            match *event {
                SceneEvent::TrackerValues { position, .. } => {
                    self.values.set(self.values.get() + 1);
                    self.reached.set(self.reached.get().max(position.y));
                }
                SceneEvent::InertiaStarting { .. } => self.inertia.set(self.inertia.get() + 1),
                _ => {}
            }
        }
        for report in reports {
            match report {
                Report::Wheel { .. } => self.wheel_reports.set(self.wheel_reports.get() + 1),
                Report::Dragged { .. } => self.drags.set(self.drags.get() + 1),
                _ => {}
            }
        }
        // This window's whole tree is the list, so the visual count is the realized set plus a
        // viewport, a content group and a thumb.
        let realized = census.visuals_live as usize;
        self.realized_max.set(self.realized_max.get().max(realized));
        if events.is_empty() {
            self.realized_rest.set(realized);
        }
        self.trackers.set(census.trackers_live);
    }

    fn report(&self) {
        println!("\n── what the run did ──");
        println!("  front-thread ticks           {}", self.ticks.get());
        println!("  tracker value reports        {}", self.values.get());
        println!("  inertia entries              {}", self.inertia.get());
        println!("  thumb drag samples           {}", self.drags.get());
        println!(
            "  Report::Wheel on the front   {}   (a scroll container's wheel is the \
             compositor's; anything here is a leak)",
            self.wheel_reports.get()
        );
        println!(
            "  realized rows   at rest {}   peak {}   of {ROWS}",
            self.realized_rest.get(),
            self.realized_max.get()
        );
        println!(
            "  furthest position reached    {:.0} DIP",
            self.reached.get()
        );
        println!(
            "  trackers live                {}   (zero is a scroll container bound to nothing)",
            self.trackers.get()
        );
    }
}

// ── a palette, so roles resolve ──────────────────────────────────────────────────
//
// The example's own palette, computed arithmetically rather than tabulated, so every role
// resolves to a value and no gap in it can read as a framework fault.

struct Reference;
static REFERENCE: Reference = Reference;

const SURFACE_NITS: [f32; 4] = [2.1, 3.7, 6.4, 11.1];
const TEXT_NITS: [f32; 4] = [30.0, 96.0, 160.0, 244.0];
const ACCENT_HUE: f32 = 250.0;

fn light(nits: f32, chroma: f32, hue: f32) -> Radiance {
    Ictcp::polar(nits, chroma, hue).to_radiance(1.0)
}

impl Palette for Reference {
    fn text(&self, role: Text, scope: Scope) -> Radiance {
        let rung = |i: usize| match scope.polarity {
            Polarity::Dark => TEXT_NITS[i],
            Polarity::Light => TEXT_NITS[TEXT_NITS.len() - 1 - i],
        };
        match role {
            Text::Disabled => light(rung(0), 0.0, 0.0),
            Text::Tertiary => light(rung(1), 0.0, 0.0),
            Text::Secondary => light(rung(2), 0.0, 0.0),
            Text::Primary | Text::OnAccent => light(rung(3), 0.0, 0.0),
            Text::Accent => light(107.0, 0.06, ACCENT_HUE),
        }
    }

    fn fill(&self, role: Fill, scope: Scope) -> Radiance {
        let base = SURFACE_NITS[scope.elevation as usize];
        match role {
            Fill::Surface => light(base, 0.004, ACCENT_HUE),
            Fill::Hover => light(base * 1.18, 0.004, ACCENT_HUE),
            Fill::Pressed => light(base * 0.86, 0.004, ACCENT_HUE),
            Fill::Selected => light(base * 1.32, 0.010, ACCENT_HUE),
            Fill::Accent => light(72.0, 0.09, ACCENT_HUE),
            Fill::AccentSubtle => light(base * 1.6, 0.03, ACCENT_HUE),
        }
    }

    fn stroke(&self, role: Stroke, scope: Scope) -> Radiance {
        let base = SURFACE_NITS[scope.elevation as usize];
        match role {
            Stroke::Subtle => light(base * 1.5, 0.002, ACCENT_HUE),
            Stroke::Default => light(base * 2.4, 0.002, ACCENT_HUE),
            Stroke::Focus => light(107.0, 0.08, ACCENT_HUE),
            Stroke::Accent => light(72.0, 0.09, ACCENT_HUE),
        }
    }

    fn data(&self, role: DataRole) -> Radiance {
        light(84.0, 0.12, f32::from(role.0) * 31.0 % 360.0)
    }

    fn typography(&self, role: TypeRole, scope: Scope) -> FontSpec {
        let size = match role {
            TypeRole::Display => 32.0,
            TypeRole::Title => 20.0,
            TypeRole::Body | TypeRole::BodyStrong | TypeRole::Mono => 14.0,
            TypeRole::Caption | TypeRole::Label => 12.0,
        };
        let size = match scope.density {
            Density::Comfortable => size,
            Density::Compact => size - 1.0,
        };
        let weight = if matches!(role, TypeRole::Title | TypeRole::BodyStrong) {
            600
        } else {
            400
        };
        FontSpec::new(FamilyId(u16::from(role == TypeRole::Mono)), size).weight(weight)
    }

    fn metric(&self, metric: Metric, scope: Scope) -> f32 {
        let tight = match (scope.density, scope.width) {
            (Density::Compact, WidthClass::Narrow) => 0.75,
            (Density::Compact, _) | (_, WidthClass::Narrow) => 0.875,
            _ => 1.0,
        };
        match metric {
            Metric::SpaceXs => 4.0 * tight,
            Metric::SpaceSm => 8.0 * tight,
            Metric::SpaceMd => 12.0 * tight,
            Metric::SpaceLg => 20.0 * tight,
            Metric::Radius | Metric::RadiusSurface | Metric::RadiusPill => 8.0,
            Metric::RowH => (32.0 * tight).max(24.0),
            Metric::BandSm => 28.0 * tight,
            Metric::BandMd => 44.0 * tight,
            Metric::BandLg => 48.0 * tight,
            Metric::CommandW => 46.0 * tight,
            Metric::BorderW => 1.0,
            Metric::HairlineW => 0.5,
            Metric::CardMinW => 240.0,
            Metric::CardMinH => 160.0,
        }
    }

    fn content_peak_nits(&self) -> f32 {
        290.0
    }
}

// ── driving ─────────────────────────────────────────────────────────────────────

/// Injects a wheel burst at the centre of `hwnd` and then closes it, so the run reports what
/// the compositor did with real input and no hand on the mouse.
fn drive(hwnd: usize) {
    use std::thread::sleep;
    use std::time::Duration;

    sleep(Duration::from_millis(600));
    // SAFETY: `hwnd` names a window in this process, and `rect` is a four-`i32` stack local,
    // which is the extent `GetWindowRect` writes.
    unsafe {
        let _ = raw::SetForegroundWindow(hwnd as *mut core::ffi::c_void);
        let mut rect = [0i32; 4];
        let _ = raw::GetWindowRect(hwnd as *mut core::ffi::c_void, rect.as_mut_ptr().cast());
        let _ = raw::SetCursorPos((rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2);
    }
    sleep(Duration::from_millis(300));
    // Twenty notches down, then a pause long enough for any inertia to settle.
    for _ in 0..20 {
        wheel(-3);
        sleep(Duration::from_millis(50));
    }
    sleep(Duration::from_millis(2500));
    // Posted rather than called: the pump owns the window and this runs on another thread.
    // SAFETY: `hwnd` names a window in this process, and `WM_CLOSE` carries no pointer in
    // either parameter, so the zeros are the whole payload.
    unsafe {
        let _ = raw::PostMessageW(hwnd as *mut core::ffi::c_void, 0x0010, 0, 0);
    }
}

fn wheel(notches: i32) {
    let input = raw::INPUT {
        kind: 0,
        mi: raw::MOUSEINPUT {
            dx: 0,
            dy: 0,
            data: (notches * 120) as u32,
            flags: 0x0800,
            time: 0,
            extra: 0,
        },
        pad: [0; 2],
    };
    // SAFETY: `input` is one fully initialized record, and `cbsize` is its exact size, so the
    // count and stride the call reads with match the allocation.
    unsafe {
        let _ = raw::SendInput(1, &input, size_of::<raw::INPUT>() as i32);
    }
}

/// Declares the input injector and the window calls this example drives itself with.
// The names, field names and layouts are the platform's.
#[expect(clippy::upper_case_acronyms)]
mod raw {
    windows_core::link!("user32.dll" "system" fn SendInput(cinputs: u32, pinputs: *const INPUT, cbsize: i32) -> u32);
    windows_core::link!("user32.dll" "system" fn SetCursorPos(x: i32, y: i32) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn SetForegroundWindow(hwnd: *mut core::ffi::c_void) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn GetWindowRect(hwnd: *mut core::ffi::c_void, lprect: *mut i32) -> windows_core::BOOL);
    windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd: *mut core::ffi::c_void, msg: u32, wparam: usize, lparam: isize) -> windows_core::BOOL);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MOUSEINPUT {
        pub dx: i32,
        pub dy: i32,
        pub data: u32,
        pub flags: u32,
        pub time: u32,
        pub extra: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct INPUT {
        pub kind: u32,
        pub mi: MOUSEINPUT,
        pub pad: [u64; 2],
    }
}
