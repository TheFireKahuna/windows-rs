//! Native proof of the shipping field driver. `--drive` checks ordinary character input
//! and idle; manual IME/touch/accessibility qualification remains a separate run.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use windows_color::{Ictcp, Radiance};
use windows_composition::Compositor;
use windows_core::Result;
use windows_d2d::Gpu;
use windows_scene::{BackdropSpec, Backends, quant_stop};
use windows_text::{FamilyId, FontLadder, FontSpec};
use windows_ui::driver::{UiRuntime, observe};
use windows_ui::layout::{Len, layer, scroll, stack};
use windows_ui::role::*;
use windows_ui::signal::Cell;
use windows_ui::text_input::InputScope;
use windows_ui::widget::{button, field, label};
use windows_window::Window;

fn main() -> Result<()> {
    let ui = UiRuntime::new(&REFERENCE, AccentId(0), Density::Comfortable);
    let commits = Arc::new(Mutex::new(Vec::<String>::new()));
    let ticks = Arc::new(AtomicU64::new(0));
    observe({
        let ticks = ticks.clone();
        move |seen| {
            if !seen.reports.is_empty() {
                println!("reports {:?}", seen.reports);
            }
            ticks.store(seen.ticks, Ordering::Release);
        }
    });
    let automatic = std::env::args().any(|a| a == "--drive");
    let idle_ok = Arc::new(AtomicU64::new(0));
    let window = Window::new("windows-ui — fields")
        .size_dips(560.0, 600.0)
        .pointer_input()
        .quit_on_close(true)
        .on_message({
            let ticks = ticks.clone();
            let idle_ok = idle_ok.clone();
            let mut pending = automatic;
            move |hwnd, message, wparam, _| {
                if pending && message == 0x18 && wparam != 0 {
                    pending = false;
                    let hwnd = hwnd as usize;
                    let ticks = ticks.clone();
                    let idle_ok = idle_ok.clone();
                    std::thread::spawn(move || {
                        use std::time::Duration;
                        std::thread::sleep(Duration::from_secs(2));
                        unsafe {
                            PostMessageW(hwnd as _, 0x100, 9, 0);
                        }
                        std::thread::sleep(Duration::from_millis(300));
                        for u in "a😀".encode_utf16() {
                            unsafe {
                                PostMessageW(hwnd as _, 0x102, u as usize, 0);
                            }
                        }
                        std::thread::sleep(Duration::from_secs(2));
                        let before = ticks.load(Ordering::Acquire);
                        std::thread::sleep(Duration::from_secs(2));
                        let after = ticks.load(Ordering::Acquire);
                        println!("focused idle input ticks: {}", after - before);
                        idle_ok.store(u64::from(after == before), Ordering::Release);
                        unsafe {
                            PostMessageW(hwnd as _, 0x10, 0, 0);
                        }
                    });
                }
                None
            }
        });
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
        BackdropSpec {
            base: vec![
                (quant_stop(0.0), light(2.1, 0.004, ACCENT_HUE)),
                (quant_stop(1.0), light(2.1, 0.004, ACCENT_HUE)),
            ],
            glows: Vec::new(),
        },
        {
            let commits = commits.clone();
            move |ui, _ctx| {
                let source = Cell::new(String::new());
                stack(ui, |ui| {
                    label(ui, "Text input proof");
                    field(
                        ui,
                        windows_ui::widget::TextSource::Dynamic(Box::new(move |out| {
                            source.with(|s| out.push_str(s))
                        })),
                    )
                    .name("Text")
                    .on_commit(move |text| {
                        commits.lock().unwrap().push(text.into());
                        source.set(text.into());
                    });
                    field(ui, "12.5").scope(InputScope::Number).name("Number");
                    field(ui, "https://newapo.dev")
                        .scope(InputScope::Url)
                        .name("URL");
                    field(ui, "search").scope(InputScope::Search).name("Search");
                    field(ui, "secret")
                        .scope(InputScope::Password)
                        .name("Password");
                    button(ui, "Replace text from model")
                        .on_click(move || source.set("model replacement".into()));
                    scroll(ui, |ui| {
                        stack(ui, |ui| {
                            label(ui, "Scroll to the field below");
                            layer(ui, |_| {}).height(Len::times(Metric::RowH, 12.0));
                            field(ui, "scroll-contained input").name("Scrolled field");
                        });
                    })
                    .grow();
                })
                .gap(Metric::SpaceSm)
                .padding(Metric::SpaceMd)
                .grow();
            }
        },
    )?;
    println!("commits: {:?}", commits.lock().unwrap());
    if automatic {
        assert_eq!(*commits.lock().unwrap(), ["a", "a😀"]);
        assert_eq!(
            idle_ok.load(Ordering::Acquire),
            1,
            "focused idle must settle"
        );
    }
    Ok(())
}
windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd: *mut core::ffi::c_void, message: u32, w: usize, l: isize) -> i32);

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

    /// No light at any rung. A reference palette states an appearance and nothing about
    /// what emits — a glow is the application's claim about its own data.
    fn emission(&self, _role: Role, _scope: Scope) -> Emission {
        Emission::NONE
    }

    fn shadow(&self, _scope: Scope) -> Shadow {
        Shadow {
            sigma: 18.0,
            offset: 14.0,
            light: Radiance::new(0.0, 0.0, 0.0, 0.45),
        }
    }

    fn typography(&self, role: TypeRole, scope: Scope) -> FontSpec {
        let size = match role {
            TypeRole::Custom(token) => return token.resolve(scope),
            TypeRole::Display => 32.0,
            TypeRole::Title => 20.0,
            TypeRole::Body | TypeRole::BodyStrong | TypeRole::Mono => 14.0,
            TypeRole::Caption | TypeRole::Label => 12.0,
            TypeRole::Micro => 10.0,
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
            Metric::Custom(token) => token.resolve(scope),
            Metric::Space3xs => 1.0,
            Metric::Space2xs => 2.0,
            Metric::SpaceXs => 4.0 * tight,
            Metric::SpaceXsSm => 6.0 * tight,
            Metric::SpaceSm => 8.0 * tight,
            Metric::SpaceSmMd => 10.0 * tight,
            Metric::SpaceMd => 12.0 * tight,
            Metric::SpaceMdLg => 16.0 * tight,
            Metric::SpaceLg => 20.0 * tight,
            Metric::SpaceXl => 28.0 * tight,
            Metric::Space2xl => 40.0 * tight,
            Metric::Radius | Metric::RadiusSurface | Metric::RadiusPill => 8.0,
            Metric::RowH => (32.0 * tight).max(24.0),
            Metric::TrackH => 20.0 * tight,
            Metric::BorderW => 1.0,
            Metric::HairlineW => 0.5,
            Metric::CardMinW => 240.0,
            Metric::CardMinH => 160.0,
            Metric::SliderRailH => 5.0 * tight,
            Metric::SliderThumb => 13.0 * tight,
        }
    }

    fn content_peak_nits(&self, _gamut: &windows_color::Gamut, _scope: Scope) -> f32 {
        290.0
    }
}
