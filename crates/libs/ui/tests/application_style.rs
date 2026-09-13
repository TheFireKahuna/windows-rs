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
    assert!(!role::installed());
    let scope = Scope::root(AccentId(0), Density::Comfortable);
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
    assert!(!role::installed());
}

#[test]
fn token_identity_is_the_static_not_the_name_or_resolved_value() {
    let first = Metric::Custom(&HEIGHT);
    let second = Metric::Custom(&OTHER_HEIGHT);
    assert_ne!(first, second);
    assert_eq!(HashSet::from([first, first, second]).len(), 2);
}
