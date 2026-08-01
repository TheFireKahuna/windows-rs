//! The total function, and the one place the application's palette is reached.

use super::{DataRole, Fill, Metric, Role, Scope, Stroke, Text, TypeRole};
use std::sync::OnceLock;
use windows_color::Radiance;
use windows_text::FontSpec;

/// What the application supplies. This crate never interprets a value it returns.
///
/// Every method is **total**: it returns a value rather than an `Option`, which is what
/// makes "missing token" unrepresentable instead of merely unlikely. A palette that has
/// not decided what some pair means still has to answer, and answering deliberately is
/// cheaper than a fallback chain nobody can see.
///
/// `Send + Sync` because a palette is a lookup table over authored constants and the
/// present thread resolves data roles for a region's own drawing. Anything a palette needs
/// to *compute* — an accent ramp, a wash — is a function over a base rather than a stored
/// shade, so there is nothing here to synchronize.
pub trait Palette: Send + Sync + 'static {
    fn text(&self, role: Text, scope: Scope) -> Radiance;
    fn fill(&self, role: Fill, scope: Scope) -> Radiance;
    fn stroke(&self, role: Stroke, scope: Scope) -> Radiance;
    /// No [`Scope`]: a data role is chromatic and shared between polarities.
    fn data(&self, role: DataRole) -> Radiance;
    fn typography(&self, role: TypeRole, scope: Scope) -> FontSpec;
    fn metric(&self, metric: Metric, scope: Scope) -> f32;

    /// The brightest channel this palette authors, in cd/m².
    ///
    /// The application's **mastering statement**, and the one number the output transform
    /// needs from the palette: it is a promise that nothing authored exceeds it, and
    /// authoring above it clips.
    fn content_peak_nits(&self) -> f32;
}

/// The installed palette. Written once; every resolve is one acquire load and a branch.
static PALETTE: OnceLock<&'static dyn Palette> = OnceLock::new();

/// Installs the application's palette. Once, before anything resolves.
///
/// Takes a `&'static` rather than a boxed value so that any leak is the caller's decision
/// and visible at the call site — a palette is a table of authored constants and is
/// expected to be a `static` or a `LazyLock`, not something built per window.
///
/// # Panics
///
/// If a different palette is already installed. Two palettes mean two answers from a
/// function whose whole value is being total, which is worse than either.
pub fn install(palette: &'static dyn Palette) {
    if PALETTE.set(palette).is_err() {
        assert!(
            core::ptr::addr_eq(
                core::ptr::from_ref(palette),
                core::ptr::from_ref(*current())
            ),
            "a palette is already installed"
        );
    }
}

/// Whether a palette has been installed. What a diagnostic asks; nothing else needs it.
#[must_use]
pub fn installed() -> bool {
    PALETTE.get().is_some()
}

fn current() -> &'static &'static dyn Palette {
    PALETTE.get().expect(
        "a palette must be installed before a role resolves: call \
         windows_ui::role::install once at start-up",
    )
}

/// The one function that turns a role into light. Total, and it returns authored light —
/// there is no way to ask this layer for anything a display could accept.
#[must_use]
pub fn resolve(role: Role, scope: Scope) -> Radiance {
    let palette = *current();
    match role {
        Role::Text(text) => palette.text(text, scope),
        Role::Fill(fill) => palette.fill(fill, scope),
        Role::Stroke(stroke) => palette.stroke(stroke, scope),
        Role::Data(data) => palette.data(data),
    }
}

/// The type ramp, resolved through the same scope the colours use.
#[must_use]
pub fn typography(role: TypeRole, scope: Scope) -> FontSpec {
    current().typography(role, scope)
}

/// A spacing, radius, row height or border width, in DIPs.
#[must_use]
pub fn metric(metric: Metric, scope: Scope) -> f32 {
    current().metric(metric, scope)
}

// ── washes: derived, never stored ───────────────────────────────────────────────
//
// A stroke, a scrim and a hover tint are the same colour at a fraction of opacity. Deriving
// them from one place is what stops a palette growing a stored constant per shade — and a
// token named after the component that wanted it is the bloat smell.

/// The polarity-flipping **foreground** wash: hairlines, dividers, hover tints.
///
/// The ink of the scope it is used in, at `alpha`. It flips with polarity for free, because
/// the foreground it is derived from already did.
#[must_use]
pub fn ink(alpha: f32, scope: Scope) -> Radiance {
    resolve(Role::Text(Text::Primary), scope).with_alpha(alpha)
}

/// The polarity-flipping **background** wash: scrims behind a modal, overlays over content.
///
/// The window's own base surface at `alpha`, resolved at [`Elevation::Base`](super::Elevation::Base)
/// rather than at the caller's rung — a scrim belongs to the window it dims, not to the
/// card that raised it.
#[must_use]
pub fn veil(alpha: f32, scope: Scope) -> Radiance {
    resolve(
        Role::Fill(Fill::Surface),
        scope.elevate(super::Elevation::Base),
    )
    .with_alpha(alpha)
}

/// The accent at a fraction: a selection tint, a subtle accent fill, a focus glow.
#[must_use]
pub fn accent_wash(alpha: f32, scope: Scope) -> Radiance {
    resolve(Role::Fill(Fill::Accent), scope).with_alpha(alpha)
}
