//! Draws one resonant response twice, with and without the smooth stroke edge, for a
//! capture to compare pixel by pixel.
//!
//! ```text
//! cargo run -p windows-ui --example stroke_edge
//! ```
//!
//! The upper plot strokes with exact coverage; the lower one filters the edge through the
//! smooth-stroke graph. Both draw the same geometry at the same width in the same role.
use windows_color::{Ictcp, Radiance};
use windows_composition::Compositor;
use windows_core::Result;
use windows_d2d::Gpu;
use windows_numerics::Vector2;
use windows_scene::{BackdropSpec, Backends, PathVerb, Spread};
use windows_text::{FamilyId, FontLadder, FontSpec};
use windows_ui::build::{Stop, Ui};
use windows_ui::driver::UiRuntime;
use windows_ui::layout::{Len, stack};
use windows_ui::role::*;
use windows_window::Window;

/// Samples per plot. Enough for one per physical pixel across the plot at 2x.
const SAMPLES: usize = 2048;

fn main() -> Result<()> {
    let ui = UiRuntime::new(&REFERENCE, AccentId(0), Density::Comfortable);
    let window = Window::new("windows-ui — stroke edge")
        .size_dips(900.0, 560.0)
        .quit_on_close(true);
    ui.run(
        window,
        || {
            let compositor = Compositor::new()?;
            let gpu = Gpu::for_window()?;
            Backends::new(compositor, &gpu, FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]))
        },
        // The dark plot ground the equalizer draws on: a bright stroke over a dark field is
        // where the coverage filter shows.
        BackdropSpec {
            base: vec![(0, light(2.1, 0.004, ACCENT_HUE)), (u16::MAX, light(2.1, 0.004, ACCENT_HUE))],
            ..BackdropSpec::default()
        },
        |ui, _ctx| {
            stack(ui, |ui| {
                curve(ui, false);
                curve(ui, true);
            })
            .gap(Metric::SpaceLg)
            .padding(Metric::SpaceLg)
            .grow();
        },
    )
}

/// The stroke width in DIPs: `STROKE_EDGE_WIDTH`, or 1.5.
fn width() -> f32 {
    std::env::var("STROKE_EDGE_WIDTH").ok().and_then(|w| w.parse().ok()).unwrap_or(1.5)
}

/// One plot: a peak and a shelf over a log axis, the shapes an equalizer draws.
fn curve(ui: &mut Ui<'_>, smooth: bool) {
    // `STROKE_EDGE_RAMP` paints through a feathered ramp, as the plots' tapered response does.
    let full = |at| Stop { at, role: DataRole(5).into(), strength: 1.0 };
    let ramp = std::env::var_os("STROKE_EDGE_RAMP").map(|_| {
        ui.ramp(&[full(0.0), full(1.0)], Spread::HorizontalFeathered { edge: 0.02, inset: 0.002 })
    });
    let path = ui
        .path_with(SAMPLES + 2, |i, verbs| {
            let size = i.size;
            if size.x <= 0.0 || size.y <= 0.0 {
                return;
            }
            let n = ((size.x * i.scale).ceil() as usize + 1).min(SAMPLES);
            for k in 0..n {
                let t = k as f32 / (n - 1) as f32;
                let peak = 9.0 / (1.0 + ((t - 0.35) / 0.03).powi(2));
                let shelf = 4.0 / (1.0 + (-(t - 0.75) / 0.04).exp());
                let dip = -6.0 / (1.0 + ((t - 0.55) / 0.06).powi(2));
                let db = peak + shelf + dip;
                let to = Vector2::new(t * size.x, size.y * (0.5 - db / 30.0));
                verbs.push(if k == 0 { PathVerb::Move { to, filled: false } } else { PathVerb::Line(to) });
            }
            verbs.push(PathVerb::End { closed: false });
        })
        .height(Len::dip(240.0));
    let path = match ramp {
        Some(ramp) => path.stroke_ramp(ramp, Len::dip(width())),
        None => path.stroke(DataRole(5), Len::dip(width())),
    };
    if smooth {
        path.smooth();
    }
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

    /// `STROKE_EDGE_NITS` sets the data role's light, so a stroke can be drawn above paper white.
    fn data(&self, role: DataRole) -> Radiance {
        let nits = std::env::var("STROKE_EDGE_NITS").ok().and_then(|n| n.parse().ok()).unwrap_or(84.0);
        light(nits, 0.12, f32::from(role.0) * 31.0 % 360.0)
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
