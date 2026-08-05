//! The installed palette, and the total functions that resolve a role, a metric or a type
//! rung through it.

use super::{DataRole, Fill, Metric, Role, Scope, Stroke, Text, TypeRole};
use std::sync::OnceLock;
use windows_color::Radiance;
use windows_text::FontSpec;

/// What the application supplies. This crate never interprets a value it returns.
///
/// Every method is total: it returns a value rather than an `Option`, so a missing token is
/// unrepresentable. A palette that has not decided what a `(role, scope)` pair means still
/// answers.
///
/// `Send + Sync`: a palette is a lookup table over authored constants, and the present thread
/// resolves data roles for a region's own drawing. A derived shade — an accent ramp, a wash —
/// is a function over a base rather than stored state, so there is nothing to synchronize.
///
/// # The three colour methods may not read `scope.width`
///
/// Only [`metric`](Self::metric) and [`typography`](Self::typography) may. A width class
/// is resolved inside the solve and changes whenever a window crosses a threshold, so a
/// colour that depended on it would make a resize invalidate every rasterized cell in the
/// subtree. [`Scope::for_paint`](super::Scope::for_paint) pins the axis on the way in, so a
/// palette reading `scope.width` in a colour method still cannot produce a width-dependent
/// colour.
pub trait Palette: Send + Sync + 'static {
    /// Returns the light a foreground role resolves to in `scope`.
    fn text(&self, role: Text, scope: Scope) -> Radiance;
    /// Returns the light a surface role resolves to in `scope`.
    fn fill(&self, role: Fill, scope: Scope) -> Radiance;
    /// Returns the light a line role resolves to in `scope`.
    fn stroke(&self, role: Stroke, scope: Scope) -> Radiance;
    /// Returns the light an application-defined chromatic role resolves to.
    ///
    /// No [`Scope`]: a data role is chromatic and shared between polarities.
    fn data(&self, role: DataRole) -> Radiance;
    /// Returns the font a rung of the type ramp resolves to in `scope`.
    fn typography(&self, role: TypeRole, scope: Scope) -> FontSpec;
    /// Returns a scalar the palette owns, in DIPs unless the name says otherwise.
    fn metric(&self, metric: Metric, scope: Scope) -> f32;

    /// Returns the brightest channel this palette authors, in cd/m².
    ///
    /// The output transform's shoulder is built to reach this value, and anything authored
    /// above it clips.
    fn content_peak_nits(&self) -> f32;
}

/// The installed palette. Written once; every resolve is one acquire load and a branch.
static PALETTE: OnceLock<&'static dyn Palette> = OnceLock::new();

/// Installs the application's palette. Call it once, before any role resolves.
///
/// Installing the same palette again succeeds and changes nothing.
///
/// Takes a `&'static` rather than a boxed value, so a palette is a `static` or a `LazyLock`
/// and any leak is the caller's own decision at the call site.
///
/// # Panics
///
/// If a different palette is already installed: two palettes would mean two answers from a
/// resolution whose contract is to be total.
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

/// Returns whether a palette has been installed. What a diagnostic asks.
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

/// Returns the light `role` resolves to in `scope`.
///
/// Total: every pair has a value. The result is authored light — scene-referred, absolute
/// cd/m² — and no display transform has run on it.
///
/// # Panics
///
/// If no palette has been installed.
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

/// Returns the font for a rung of the type ramp, resolved through the same scope the colours
/// use.
///
/// # Panics
///
/// If no palette has been installed.
#[must_use]
pub fn typography(role: TypeRole, scope: Scope) -> FontSpec {
    current().typography(role, scope)
}

/// Returns a spacing, radius, row height or border width, in DIPs.
///
/// # Panics
///
/// If no palette has been installed.
#[must_use]
pub fn metric(metric: Metric, scope: Scope) -> f32 {
    current().metric(metric, scope)
}

/// Returns the brightest value the palette authors, in cd/m².
///
/// Scope-free: it is a property of the authored table rather than of any site that uses it,
/// and it is what the output transform's shoulder is built to reach. Read from the palette
/// rather than passed in, so the transform a window builds and the values the palette authors
/// answer to one peak.
///
/// # Panics
///
/// If no palette has been installed.
#[must_use]
pub fn content_peak_nits() -> f32 {
    current().content_peak_nits()
}

// ── washes: derived, never stored ───────────────────────────────────────────────
//
// A hairline, a scrim and a hover tint are one resolved colour at a fraction of opacity.
// Each is derived here, so a palette stores no per-shade constant.

/// Returns the foreground wash: [`Text::Primary`] in `scope`, at `alpha`.
///
/// Hairlines, dividers and hover tints. It follows polarity because the foreground it is
/// derived from does.
#[must_use]
pub fn ink(alpha: f32, scope: Scope) -> Radiance {
    resolve(Role::Text(Text::Primary), scope).with_alpha(alpha)
}

/// Returns the background wash: the window's own base surface at `alpha`.
///
/// Scrims behind a modal, and overlays over content. Resolved at
/// [`Elevation::Base`](super::Elevation::Base) rather than at the caller's rung, because a
/// scrim belongs to the window it dims and not to the card that raised it.
#[must_use]
pub fn veil(alpha: f32, scope: Scope) -> Radiance {
    resolve(
        Role::Fill(Fill::Surface),
        scope.elevate(super::Elevation::Base),
    )
    .with_alpha(alpha)
}

/// Returns the accent fill in `scope` at `alpha`: a selection tint, a subtle accent fill, a
/// focus glow.
#[must_use]
pub fn accent_wash(alpha: f32, scope: Scope) -> Radiance {
    resolve(Role::Fill(Fill::Accent), scope).with_alpha(alpha)
}
