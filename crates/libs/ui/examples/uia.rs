//! Drives UI Automation against a real window, so an automation client can walk the tree.
//!
//! No tree is written out here. The mounted controls declare their own roles, names and
//! values; the driver's tick publishes the rows that come out of that, and a client reads
//! them from automation's own worker thread without entering the window's pump.
//!
//! An attached client exercises what no unit test reaches: that `WM_GETOBJECT` returns a
//! provider `UiaReturnRawElementProvider` accepts, that the fragment root is reachable, that
//! the control types read as intended, and that a command sent by a client is applied on the
//! thread that owns the model, through the same handler a tap runs.
//!
//! ```text
//! cargo run -p windows-ui --example uia
//! ```
//!
//! Attach Accessibility Insights or Inspect and walk the tree. Invoking Mute, toggling Bypass
//! and setting Gain each run the widget's own handler, and every resulting value change prints
//! here.
use windows_color::{Ictcp, Radiance};
use windows_composition::Compositor;
use windows_core::Result;
use windows_d2d::Gpu;
use windows_scene::{BackdropSpec, Backends};
use windows_text::{FamilyId, FontLadder, FontSpec};
use windows_ui::driver::UiRuntime;
use windows_ui::layout::{Align, row, stack};
use windows_ui::role::*;
use windows_ui::signal::{Cell, Effect};
use windows_ui::widget::{Range, SliderStyle, button, label, meter, slider, title, toggle};
use windows_window::Window;

/// The bounds Gain moves between, which the slider publishes as its range.
const GAIN_RANGE: Range = Range::new(-60.0, 12.0);

fn main() -> Result<()> {
    let ui = UiRuntime::new(&REFERENCE, AccentId(0), Density::Comfortable);
    let window = Window::new("windows-ui — UI Automation")
        .size_dips(496.0, 360.0)
        .pointer_input()
        .quit_on_close(true);

    println!("attach Accessibility Insights or Inspect and walk the tree");
    println!("invoke Mute, toggle Bypass, set Gain — from the client or the window\n");

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
        |ui, _ctx| {
            let muted = Cell::new(false);
            let bypassed = Cell::new(false);
            let gain = Cell::new(0.0f64);

            // The model's only readers. Each runs once on creation, so the run opens with the
            // state a client is about to read, and again whenever a tap or an automation
            // command moves it.
            Effect::new(move || println!("Mute -> {}", muted.get()));
            Effect::new(move || println!("Bypass -> {}", bypassed.get()));
            Effect::new(move || println!("Gain -> {:.1} dB", gain.get()));

            stack(ui, |ui| {
                title(ui, "Output");
                row(ui, |ui| {
                    button(ui, "Mute")
                        .name("Mute")
                        .selected(muted)
                        .on_click(move || muted.set(!muted.get()));
                    toggle(ui, bypassed).name("Bypass");
                    label(ui, "Bypass");
                })
                .gap(Metric::SpaceMd)
                .align(Align::Center);
                row(ui, |ui| {
                    label(ui, "Gain");
                    slider(ui, gain, GAIN_RANGE, SliderStyle::default())
                        .name("Gain")
                        .grow();
                })
                .gap(Metric::SpaceMd)
                .align(Align::Center);
                // A value-reporting element that takes no gesture, so a client reads it and
                // can do nothing to it.
                meter(ui, move || {
                    let span = (GAIN_RANGE.max - GAIN_RANGE.min) as f32;
                    ((gain.get() - GAIN_RANGE.min) as f32 / span).clamp(0.0, 1.0)
                })
                .name("Level")
                .height(Metric::TrackH);
            })
            .gap(Metric::SpaceLg)
            .padding(Metric::SpaceLg)
            .grow();
        },
    )
}

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
            Fill::Hover => self.text(Text::Primary, scope).with_alpha(0.012),
            Fill::Pressed => self.text(Text::Primary, scope).with_alpha(0.008),
            Fill::Sunken => light(base * 0.86, 0.004, ACCENT_HUE),
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

    /// Returns no light past any silhouette, at any rung.
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
