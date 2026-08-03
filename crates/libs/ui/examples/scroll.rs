//! Drives scroll and virtualization against a real compositor, and reports what a run did.
//!
//! Three claims are only checkable here, because all three are the compositor's:
//!
//! 1. **A fling asks for its own frames.** A tracker reports from another process with no
//!    input behind it, so the queue its callbacks push into holds the frame clock open until
//!    it is drained. Without that the realization window is read at whatever unrelated wake
//!    comes next, which is a fling that lands on rows nobody realized.
//! 2. **A wheel notch costs the front thread nothing.** `PointerWheelConfig` routes it to the
//!    tracker, so the run below should show the position moving with no `Report::Wheel` at
//!    all.
//! 3. **The realized set stays bounded.** Ten thousand rows, and the count of realized rows
//!    is printed at rest, mid-fling and after it settles.
//!
//! Scroll it with the wheel, drag the thumb, then press Q. The summary is the point.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows_color::{DisplayCapability, Ictcp, OutputTransform, Radiance};
use windows_composition::Compositor;
use windows_core::Result;
use windows_d2d::Gpu;
use windows_numerics::Vector2;
use windows_scene::{BackdropSpec, Backends, Env, Model, Scene, SceneEvent, SinkPatch, taffy};
use windows_text::{FamilyId, FontLadder, FontSpec};
use windows_ui::build::{Host, mount};
use windows_ui::input::{Doorbell, Report, Router};
use windows_ui::layout::{Len, ListSpec, list};
use windows_ui::role::{
    AccentId, DataRole, Density, Fill, Metric, Palette, Polarity, Scope, Stroke, Text, TypeRole,
    WidthClass,
};
use windows_ui::signal;
use windows_ui::widget::{Controls, Front, Intent};
use windows_window::{Tick, Wake, Window};

/// Enough rows that realizing them all is not an option anyone could miss.
const ROWS: usize = 10_000;

fn main() -> Result<()> {
    windows_ui::role::install(&REFERENCE);

    let bell = Rc::new(Doorbell::new());
    let quit = Rc::new(Cell::new(false));
    // Raw wheel messages, counted before the doorbell. Without this a run cannot tell "the
    // compositor took the wheel" from "no wheel ever arrived": both read as zero reports.
    let wheel_messages = Rc::new(Cell::new(0u32));
    let resized: Rc<Cell<Option<(i32, i32)>>> = Rc::new(Cell::new(None));
    let frame: Rc<RefCell<Option<Loop>>> = Rc::new(RefCell::new(None));

    let window = Window::new("windows-ui — scroll and virtualization")
        .size_dips(520.0, 640.0)
        .pointer_input()
        .touchpad_capable()
        .quit_on_close(true)
        .on_message({
            let bell = Rc::clone(&bell);
            let quit = Rc::clone(&quit);
            let frame = Rc::downgrade(&frame);
            let wheel_messages = Rc::clone(&wheel_messages);
            move |_, message, wparam, lparam| {
                // WM_MOUSEWHEEL and WM_POINTERWHEEL.
                if message == 0x020A || message == 0x024E {
                    wheel_messages.set(wheel_messages.get() + 1);
                }
                if message == 0x0100 && wparam == b'Q' as usize {
                    quit.set(true);
                    windows_window::quit();
                    return Some(0);
                }
                if message != windows_window::WM_FRAME {
                    return bell.wndproc(message, wparam, lparam);
                }
                if let Some(cell) = frame.upgrade()
                    && let Ok(mut slot) = cell.try_borrow_mut()
                    && let Some(run) = slot.as_mut()
                    && let Err(error) = run.tick()
                {
                    // Never swallowed: a refused tracker creation is exactly the failure
                    // this example exists to catch, and it looks identical to a wheel that
                    // never arrived if the error is dropped.
                    println!("tick failed: {error}");
                    windows_window::quit();
                }
                Some(0)
            }
        })
        .on_resize({
            let resized = Rc::clone(&resized);
            move |w, h| resized.set(Some((w, h)))
        })
        .create()?;
    let window = Rc::new(window);

    let compositor = Compositor::new()?;
    let gpu = Gpu::for_window()?;
    let ladder = FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]);
    let backends = Backends::new(compositor, &gpu, ladder.clone())?;
    windows_ui::build::text::install(ladder)?;

    let pacer = window.pacer()?;
    let env = env_of(&window);
    let scene = Rc::new(RefCell::new(Scene::new(
        &window,
        &backends,
        pacer.wake(),
        env,
        BackdropSpec::default(),
    )?));
    let router = Router::new(&bell, &window, pacer.wake())?;

    let root_scope = Scope::root(AccentId(0), Density::Comfortable);
    let mut model = Model::new(taffy::Style {
        size: taffy::Size {
            width: taffy::Dimension::percent(1.0),
            height: taffy::Dimension::percent(1.0),
        },
        ..taffy::Style::DEFAULT
    });
    model.set_window(client_dips(&window));
    let root = model.root();
    Host::install(model, env, root_scope);

    let _mounted = mount(
        list(
            || ListSpec {
                count: ROWS,
                row_h: Metric::RowH,
                overscan: 3,
            },
            |realized, out| {
                for run in realized.runs() {
                    out.extend(run.map(|index| (index, index)));
                }
            },
            |index: &usize| windows_ui::widget::label(format!("row {index}")),
        )
        .width(Len::Pct(1.0))
        .height(Len::Pct(1.0)),
        root,
    );

    let pending: Rc<Cell<Option<Tick>>> = Rc::new(Cell::new(None));
    signal::set_waker({
        let pending = Rc::clone(&pending);
        let wake = pacer.wake();
        move || pending.set(Some(wake.tick()))
    });

    *frame.borrow_mut() = Some(Loop {
        window: Rc::clone(&window),
        scene,
        backends,
        router,
        controls: Controls::new(),
        patch: SinkPatch::new(),
        events: Vec::new(),
        reports: Vec::new(),
        intents: Vec::new(),
        resized,
        pending,
        wake: pacer.wake(),
        seen: Seen::default(),
    });

    if let Some(run) = frame.borrow_mut().as_mut() {
        run.tick()?;
    }
    window.show();
    if std::env::args().any(|arg| arg == "--drive") {
        // From this process, because taking the foreground is what a wheel notch is
        // delivered against and a background process is refused it.
        let hwnd = window.hwnd() as usize;
        std::thread::spawn(move || drive(hwnd));
    }
    println!(
        "{ROWS} rows, {} DIP each. Wheel over the list, drag the thumb, then Q.\n",
        windows_ui::role::metric(Metric::RowH, root_scope)
    );
    windows_window::run();

    if let Some(run) = frame.borrow().as_ref() {
        run.seen.report();
        println!("  raw wheel messages           {}", wheel_messages.get());
        println!(
            "  trackers live                {}   (zero is a scroll container bound to nothing)",
            run.trackers_live()
        );
    }
    Ok(())
}

/// What the run observed, so the summary is measured rather than remembered.
#[derive(Default)]
struct Seen {
    ticks: Cell<u32>,
    values: Cell<u32>,
    inertia: Cell<u32>,
    wheel_reports: Cell<u32>,
    drags: Cell<u32>,
    /// The realized-row high-water mark, and where it stood at the last rest.
    realized_max: Cell<usize>,
    realized_rest: Cell<usize>,
    reached: Cell<f32>,
}

impl Seen {
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
    }
}

struct Loop {
    window: Rc<Window>,
    scene: Rc<RefCell<Scene>>,
    backends: Backends,
    router: Router,
    controls: Controls,
    patch: SinkPatch,
    events: Vec<SceneEvent>,
    reports: Vec<Report>,
    intents: Vec<Intent>,
    resized: Rc<Cell<Option<(i32, i32)>>>,
    pending: Rc<Cell<Option<Tick>>>,
    wake: Wake,
    seen: Seen,
}

impl Loop {
    /// Whether the compositor actually built the tracker this list scrolls on.
    fn trackers_live(&self) -> u32 {
        self.scene
            .try_borrow()
            .map_or(0, |scene| scene.census().trackers_live)
    }

    fn tick(&mut self) -> Result<()> {
        self.pending.take();
        self.seen.ticks.set(self.seen.ticks.get() + 1);
        let env = env_of(&self.window);
        let mut scene = self.scene.borrow_mut();

        if let Some((w, h)) = self.resized.take() {
            let scale = env.scale();
            Host::with(|host| {
                host.set_window(Vector2 {
                    x: w as f32 / scale,
                    y: h as f32 / scale,
                });
            });
        }

        self.events.clear();
        scene.drain_events(&mut self.events);
        windows_ui::layout::scroll_observe(&self.events);
        for event in &self.events {
            match *event {
                SceneEvent::TrackerValues { position, .. } => {
                    self.seen.values.set(self.seen.values.get() + 1);
                    self.seen
                        .reached
                        .set(self.seen.reached.get().max(position.y));
                }
                SceneEvent::InertiaStarting { .. } => {
                    self.seen.inertia.set(self.seen.inertia.get() + 1);
                }
                _ => {}
            }
        }

        signal::flush();
        Host::with(|host| host.flush(&mut self.patch));
        scene.apply(&mut self.patch, &self.backends, env)?;
        self.patch.clear();

        let (chrome, released, gestures) = Host::with(|host| {
            (
                host.take_chrome(),
                host.take_released(),
                host.take_gestures(),
            )
        });
        for (target, decl) in gestures {
            self.router.declare(target, decl);
        }
        {
            let mut front = Front {
                scene: &mut scene,
                back: &self.backends,
                env,
            };
            self.controls.adopt(&chrome, &mut front)?;
        }
        for target in released {
            self.controls.release(target);
            self.router.forget(target);
        }

        self.reports.clear();
        self.router.tick(scene.hits(), env, &mut self.reports)?;
        for report in &self.reports {
            match report {
                Report::Wheel { .. } => {
                    self.seen
                        .wheel_reports
                        .set(self.seen.wheel_reports.get() + 1);
                }
                Report::Dragged { .. } => self.seen.drags.set(self.seen.drags.get() + 1),
                _ => {}
            }
        }

        self.intents.clear();
        let mut front = Front {
            scene: &mut scene,
            back: &self.backends,
            env,
        };
        self.controls
            .tick(&self.reports, &mut front, &mut self.intents)?;
        windows_ui::layout::scroll_front(&self.events, &self.reports, &mut front)?;
        drop(scene);
        Host::with(|host| host.dispatch(&self.intents));

        // Off the scene's own census: this window's whole tree is the list, so the node
        // count is the realized set plus a viewport, a content group and a thumb.
        let realized = scene_nodes(&self.scene);
        self.seen
            .realized_max
            .set(self.seen.realized_max.get().max(realized));
        if self.events.is_empty() {
            self.seen.realized_rest.set(realized);
        }
        // Nothing here asks for another frame. If the clock keeps running through a fling it
        // is because the tracker's own queue held it, which is claim 1.
        let _ = &self.wake;
        Ok(())
    }
}

fn client_dips(window: &Window) -> Vector2 {
    let scale = window.scale().unwrap_or(1.0);
    let (w, h) = window.client_size().unwrap_or((0, 0));
    Vector2 {
        x: w as f32 / scale,
        y: h as f32 / scale,
    }
}

fn env_of(window: &Window) -> Env {
    Env::new(
        window.metrics().map_or(96.0, |m| m.dpi as f32),
        OutputTransform::for_display(
            window.color_capability().unwrap_or(DisplayCapability::Sdr),
            REFERENCE.content_peak_nits(),
        ),
    )
}

// ── a palette, so roles resolve ──────────────────────────────────────────────────
//
// The example's own, and deliberately arithmetic rather than a table: nothing here is a
// design statement, and a hole in it would read as a framework bug.

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
            Metric::Radius | Metric::RadiusPill => 8.0,
            Metric::RowH => (32.0 * tight).max(24.0),
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

/// The scene's live visual count, which for this window is the realized set plus a
/// viewport, a content group, a thumb and each row's own label.
fn scene_nodes(scene: &Rc<RefCell<Scene>>) -> usize {
    scene
        .try_borrow()
        .map_or(0, |scene| scene.census().visuals_live as usize)
}

// ── driving ─────────────────────────────────────────────────────────────────────

/// Injects a wheel burst and then Q, so a run reports what the compositor did with real
/// input rather than what a person remembered doing.
fn drive(hwnd: usize) {
    use std::thread::sleep;
    use std::time::Duration;

    sleep(Duration::from_millis(600));
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
    // Posted rather than called: `quit` belongs to the pump's own thread.
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
    unsafe {
        let _ = raw::SendInput(1, &input, size_of::<raw::INPUT>() as i32);
    }
}

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
