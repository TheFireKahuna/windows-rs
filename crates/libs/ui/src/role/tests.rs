//! What the role layer claims, checked against a palette that answers by construction
//! rather than from a table.
//!
//! The point of this palette is that it cannot have a hole: every method computes from the
//! role and the scope, so a failure here is the *mechanism* — a scope that did not reach a
//! resolve, a wash that read the wrong rung — rather than a token someone forgot.

use super::*;
use windows_color::{Ictcp, Radiance};
use windows_text::{FamilyId, FontSpec};

struct Reference;

/// A luminance per elevation rung, in cd/m². Ascending, so a card reads above its panel.
const SURFACE_NITS: [f32; 4] = [2.1, 3.7, 6.4, 11.1];
/// Text, disabled through primary.
const TEXT_NITS: [f32; 4] = [30.0, 96.0, 160.0, 244.0];
const ACCENT_HUE: f32 = 250.0;

fn light(nits: f32, chroma: f32, hue: f32) -> Radiance {
    Ictcp::polar(nits, chroma, hue).to_radiance(1.0)
}

impl Palette for Reference {
    fn text(&self, role: Text, scope: Scope) -> Radiance {
        // Polarity inverts the ladder; that is the whole of what polarity means here.
        let rung = |i: usize| match scope.polarity {
            Polarity::Dark => TEXT_NITS[i],
            Polarity::Light => TEXT_NITS[TEXT_NITS.len() - 1 - i],
        };
        match role {
            Text::Disabled => light(rung(0), 0.0, 0.0),
            Text::Tertiary => light(rung(1), 0.0, 0.0),
            Text::Secondary => light(rung(2), 0.0, 0.0),
            Text::Primary => light(rung(3), 0.0, 0.0),
            Text::Accent => light(107.0, 0.06, ACCENT_HUE),
            Text::OnAccent => light(rung(3), 0.0, 0.0),
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
        // Chromatic and shared: the hue is the role, and nothing about it reads a scope.
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
        let weight = match role {
            TypeRole::Title | TypeRole::BodyStrong => 600,
            _ => 400,
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
            Metric::Radius => 8.0,
            Metric::RadiusPill => 8.0,
            // Never below the touch floor, whatever the density says.
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

static REFERENCE: Reference = Reference;

/// Installs once for the whole test binary. `install` is idempotent for the same palette,
/// so every test may call it and tests may run in parallel.
///
/// Shared with the lowering's own tests rather than duplicated there: the palette is a
/// process-wide `OnceLock`, so a second one would not install and the module that lost the
/// race would silently assert against the other's answers.
pub(crate) fn palette() {
    install(&REFERENCE);
}

/// Every scope the axes can produce. Five axes, and the product is what "total" is a claim
/// about.
fn every_scope() -> impl Iterator<Item = Scope> {
    const ELEVATIONS: [Elevation; 4] = [
        Elevation::Base,
        Elevation::Raised,
        Elevation::Overlay,
        Elevation::Flyout,
    ];
    const POLARITIES: [Polarity; 2] = [Polarity::Dark, Polarity::Light];
    const DENSITIES: [Density; 2] = [Density::Comfortable, Density::Compact];
    const WIDTHS: [WidthClass; 3] = [WidthClass::Narrow, WidthClass::Medium, WidthClass::Wide];

    ELEVATIONS.into_iter().flat_map(move |elevation| {
        POLARITIES.into_iter().flat_map(move |polarity| {
            DENSITIES.into_iter().flat_map(move |density| {
                WIDTHS.into_iter().map(move |width| Scope {
                    elevation,
                    polarity,
                    accent: AccentId(0),
                    density,
                    width,
                })
            })
        })
    })
}

fn every_role() -> impl Iterator<Item = Role> {
    const TEXTS: [Text; 6] = [
        Text::Primary,
        Text::Secondary,
        Text::Tertiary,
        Text::Disabled,
        Text::Accent,
        Text::OnAccent,
    ];
    const FILLS: [Fill; 6] = [
        Fill::Surface,
        Fill::Hover,
        Fill::Pressed,
        Fill::Selected,
        Fill::Accent,
        Fill::AccentSubtle,
    ];
    const STROKES: [Stroke; 4] = [
        Stroke::Subtle,
        Stroke::Default,
        Stroke::Focus,
        Stroke::Accent,
    ];
    TEXTS
        .into_iter()
        .map(Role::Text)
        .chain(FILLS.into_iter().map(Role::Fill))
        .chain(STROKES.into_iter().map(Role::Stroke))
        .chain((0..8).map(|band| Role::Data(DataRole(band))))
}

#[test]
fn every_role_in_every_scope_resolves_to_finite_light() {
    palette();
    for scope in every_scope() {
        for role in every_role() {
            let light = resolve(role, scope);
            assert!(
                light.r.is_finite() && light.g.is_finite() && light.b.is_finite(),
                "{role:?} in {scope:?} resolved to {light:?}"
            );
            assert!(
                light.a > 0.0,
                "an opaque role resolved transparent: {role:?} in {scope:?}"
            );
        }
    }
}

#[test]
fn a_data_role_carries_no_polarity() {
    palette();
    let dark = Scope::root(AccentId(0), Density::Comfortable);
    let mut light_scope = dark;
    light_scope.polarity = Polarity::Light;
    for band in 0..8 {
        let role = Role::Data(DataRole(band));
        assert_eq!(
            resolve(role, dark),
            resolve(role, light_scope),
            "band {band} changed with polarity"
        );
    }
}

#[test]
fn a_chrome_role_does_carry_polarity() {
    palette();
    let dark = Scope::root(AccentId(0), Density::Comfortable);
    let mut light_scope = dark;
    light_scope.polarity = Polarity::Light;
    let role = Role::Text(Text::Primary);
    assert_ne!(resolve(role, dark), resolve(role, light_scope));
}

#[test]
fn elevating_a_scope_changes_the_surface_and_nothing_else_about_the_call() {
    palette();
    let base = Scope::root(AccentId(0), Density::Comfortable);
    let raised = base.elevate(Elevation::Raised);
    assert_ne!(
        resolve(Role::Fill(Fill::Surface), base),
        resolve(Role::Fill(Fill::Surface), raised),
        "a card must read above the panel it sits on"
    );
    // The role the widget names did not change; only the scope it resolved against did.
    assert_eq!(base.polarity, raised.polarity);
    assert_eq!(base.density, raised.density);
    assert_eq!(base.width, raised.width);
}

#[test]
fn an_interaction_state_is_the_same_role_re_resolved() {
    // Hover, pressed and selected are not extra colour parameters. The application never
    // writes one; the scene ramps between two resolutions of the same scope.
    palette();
    let scope = Scope::root(AccentId(0), Density::Comfortable).elevate(Elevation::Raised);
    let rest = resolve(Role::Fill(Fill::Surface), scope);
    let hover = resolve(Role::Fill(Fill::Hover), scope);
    assert_ne!(rest, hover);
    assert!(hover.peak_nits() > rest.peak_nits());
}

#[test]
fn a_wash_is_derived_from_a_role_rather_than_stored() {
    palette();
    let scope = Scope::root(AccentId(0), Density::Comfortable).elevate(Elevation::Flyout);

    let ink = ink(0.25, scope);
    let text = resolve(Role::Text(Text::Primary), scope);
    assert_eq!((ink.r, ink.g, ink.b), (text.r, text.g, text.b));
    assert!((ink.a - 0.25).abs() < 1e-6);

    // A scrim belongs to the window it dims, not to the flyout that raised it, so it
    // resolves at the base rung however deep the caller is.
    let veil = veil(0.5, scope);
    let base = resolve(
        Role::Fill(Fill::Surface),
        Scope::root(AccentId(0), Density::Comfortable),
    );
    assert_eq!((veil.r, veil.g, veil.b), (base.r, base.g, base.b));

    let wash = accent_wash(0.1, scope);
    let accent = resolve(Role::Fill(Fill::Accent), scope);
    assert_eq!((wash.r, wash.g, wash.b), (accent.r, accent.g, accent.b));
}

#[test]
fn density_and_width_are_separate_axes_and_both_reach_a_metric() {
    palette();
    let root = Scope::root(AccentId(0), Density::Comfortable);
    let comfortable_wide = metric(Metric::SpaceMd, root);
    let compact_wide = metric(Metric::SpaceMd, root.at_density(Density::Compact));
    let comfortable_narrow = metric(Metric::SpaceMd, root.at_width(WidthClass::Narrow));
    let compact_narrow = metric(
        Metric::SpaceMd,
        root.at_density(Density::Compact)
            .at_width(WidthClass::Narrow),
    );

    assert!(
        compact_wide < comfortable_wide,
        "density must reach a metric"
    );
    assert!(comfortable_narrow < comfortable_wide, "and so must width");
    assert!(
        compact_narrow < compact_wide && compact_narrow < comfortable_narrow,
        "and the two must compose in one place rather than at every call site"
    );
}

#[test]
fn a_row_height_never_drops_below_the_touch_floor() {
    palette();
    for scope in every_scope() {
        assert!(
            metric(Metric::RowH, scope) >= 24.0,
            "a compact narrow row went under the floor in {scope:?}"
        );
    }
}

#[test]
fn a_pill_radius_is_a_real_value_and_not_a_sentinel() {
    // A composition corner radius caps at half the box, so a pill authored as some large
    // number renders as a football on a tall box rather than as a stadium.
    palette();
    for scope in every_scope() {
        let pill = metric(Metric::RadiusPill, scope);
        assert!(
            pill > 0.0 && pill <= 32.0,
            "a pill radius of {pill} is a sentinel"
        );
    }
}

#[test]
fn the_type_ramp_resolves_through_the_same_scope_the_colours_use() {
    palette();
    let root = Scope::root(AccentId(0), Density::Comfortable);
    let body = typography(TypeRole::Body, root);
    let compact = typography(TypeRole::Body, root.at_density(Density::Compact));
    assert!(compact.size < body.size);
    assert!(typography(TypeRole::Title, root).weight > body.weight);
    assert!(typography(TypeRole::Display, root).size > typography(TypeRole::Title, root).size);
}

#[test]
fn a_polarity_flip_is_reported_only_when_it_moved() {
    // Step one of four. The caller has to do the other three, and the return value is what
    // tells it whether it needs to.
    let start = polarity();
    assert!(
        !set_polarity(start),
        "a flip to the current polarity is not a flip"
    );
    assert!(set_polarity(match start {
        Polarity::Dark => Polarity::Light,
        Polarity::Light => Polarity::Dark,
    }));
    assert!(set_polarity(start), "and back");
}
