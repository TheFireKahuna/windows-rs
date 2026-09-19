//! Application-defined tokens resolve without registering a catalogue or installing a palette.

use std::collections::HashSet;
use windows_text::{FamilyId, FontSpec};
use windows_ui::role::{self, AccentId, Density, Metric, Scope, ScopedToken, TypeRole, WidthClass};

static HEIGHT: ScopedToken<f32> = ScopedToken::new("height", |scope| {
    if scope.width == WidthClass::Narrow {
        190.0
    } else {
        220.0
    }
});
static OTHER_HEIGHT: ScopedToken<f32> = ScopedToken::new("height", |_| 330.0);
static HEADING: ScopedToken<FontSpec> = ScopedToken::new("heading", |scope| {
    FontSpec::new(
        FamilyId(0),
        if scope.density == Density::Compact {
            12.0
        } else {
            16.0
        },
    )
});

#[test]
fn application_tokens_are_total_without_palette_registration() {
    let scope = Scope::root(&NoBuiltins, AccentId(0), Density::Comfortable);
    assert_eq!(
        role::metric(Metric::Custom(&HEIGHT), scope.at_width(WidthClass::Narrow)),
        190.0
    );
    assert_eq!(
        role::metric(Metric::Custom(&HEIGHT), scope.at_width(WidthClass::Wide)),
        220.0
    );
    assert_eq!(
        role::typography(TypeRole::Custom(&HEADING), scope),
        HEADING.resolve(scope)
    );
    let compact = scope.at_density(Density::Compact);
    assert_ne!(
        role::typography(TypeRole::Custom(&HEADING), scope),
        role::typography(TypeRole::Custom(&HEADING), compact)
    );
}

#[test]
fn token_identity_is_the_static_not_the_name_or_resolved_value() {
    let first = Metric::Custom(&HEIGHT);
    let second = Metric::Custom(&OTHER_HEIGHT);
    assert_ne!(first, second);
    assert_eq!(HashSet::from([first, first, second]).len(), 2);
}

struct NoBuiltins;
impl role::Palette for NoBuiltins {
    fn text(&self, _: role::Text, _: Scope) -> windows_color::Radiance { unreachable!() }
    fn fill(&self, _: role::Fill, _: Scope) -> windows_color::Radiance { unreachable!() }
    fn stroke(&self, _: role::Stroke, _: Scope) -> windows_color::Radiance { unreachable!() }
    fn shadow(&self, _: Scope) -> role::Shadow { unreachable!() }
    fn data(&self, _: role::DataRole) -> windows_color::Radiance { unreachable!() }
    fn emission(&self, _: role::Role, _: Scope) -> role::Emission { unreachable!() }
    fn typography(&self, _: TypeRole, _: Scope) -> FontSpec { unreachable!() }
    fn metric(&self, _: Metric, _: Scope) -> f32 { unreachable!() }
    fn content_peak_nits(&self, _: &windows_color::Gamut, _: Scope) -> f32 { unreachable!() }
}
