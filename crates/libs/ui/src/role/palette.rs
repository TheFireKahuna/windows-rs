//! The installed palette, and the total functions that resolve a role, a metric or a type
//! rung through it.

use super::{DataRole, Emission, Fill, Metric, Role, Scope, Stroke, Text, TypeRole};
use windows_color::{Gamut, Radiance};
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
pub trait Palette: core::any::Any + Send + Sync + 'static {
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
    /// Returns how much light `role` spends outside the silhouette it paints.
    ///
    /// The whole [`Role`] rather than one of the four sub-enums, because emission crosses
    /// them: a processor kind's colour emits wherever it is painted, and the accent under a
    /// call to action emits as a fill.
    ///
    /// Total like the rest: a role that carries no light answers [`Emission::NONE`] rather
    /// than nothing. May not read `scope.width`, for the reason the colour methods may not.
    fn emission(&self, role: Role, scope: Scope) -> Emission;

    /// Returns the font a rung of the type ramp resolves to in `scope`.
    fn typography(&self, role: TypeRole, scope: Scope) -> FontSpec;
    /// Returns a detached surface's shadow. Width is pinned as for surface colours.
    fn shadow(&self, scope: Scope) -> super::Shadow;
    /// Returns a scalar the palette owns, in DIPs unless the name says otherwise.
    fn metric(&self, metric: Metric, scope: Scope) -> f32;

    /// Returns the brightest channel this palette authors in `gamut`'s primaries, in
    /// cd/m².
    ///
    /// The output transform's shoulder is built to reach this value, and anything authored
    /// above it clips. Taken in the primaries of the display the transform is for, because
    /// a channel's height depends on the primaries it is expressed in.
    fn content_peak_nits(&self, gamut: &Gamut) -> f32;
}

/// Returns the light `role` resolves to in `scope`.
///
/// Total: every pair has a value. The result is authored light — scene-referred, absolute
/// cd/m² — and no display transform has run on it.
///
#[must_use]
pub fn resolve(role: Role, scope: Scope) -> Radiance {
    let scope = scope.for_paint();
    let palette = scope.palette.0;
    match role {
        Role::Text(text) => palette.text(text, scope),
        Role::Fill(fill) => palette.fill(fill, scope),
        Role::Stroke(stroke) => palette.stroke(stroke, scope),
        Role::Data(data) => palette.data(data),
        Role::Custom(token) => token.resolve(scope).0,
    }
}

/// Returns how much light `role` spends past its own silhouette.
///
/// The emissive tier's one reader. A surface that draws its own pixels — a presentation
/// region, which has no [`Scope`] on the thread it draws on — reads this where it *can*
/// resolve a scope and pins the answer, the way it already pins a type rung.
///
#[must_use]
pub fn emission(role: Role, scope: Scope) -> Emission {
    let scope = scope.for_paint();
    match role {
        Role::Custom(token) => token.resolve(scope).1,
        _ => scope.palette.0.emission(role, scope),
    }
}

/// Returns the font for a rung of the type ramp, resolved through the same scope the colours
/// use.
///
#[must_use]
pub fn typography(role: TypeRole, scope: Scope) -> FontSpec {
    match role {
        TypeRole::Custom(token) => token.resolve(scope),
        _ => scope.palette.0.typography(role, scope),
    }
}

/// Returns a spacing, radius, row height or border width, in DIPs.
///
#[must_use]
pub fn metric(metric: Metric, scope: Scope) -> f32 {
    match metric {
        Metric::Custom(token) => token.resolve(scope),
        _ => scope.palette.0.metric(metric, scope),
    }
}

/// Returns the brightest value the palette authors in `gamut`'s primaries, in cd/m².
///
/// Scope-free: it is a property of the authored table and of the display's primaries rather
/// than of any site that uses it, and it is what the output transform's shoulder is built to
/// reach. Read from the palette rather than passed in, so the transform a window builds and
/// the values the palette authors answer to one peak.
///
#[must_use]
pub fn content_peak_nits(gamut: &Gamut, scope: Scope) -> f32 {
    scope.palette.0.content_peak_nits(gamut)
}

/// Returns the light a chromatic role resolves to.
///
/// No [`Scope`]: a data role is chromatic and shared between polarities, so it is the one
/// role that resolves the same everywhere. That is what lets a gradient be minted where there
/// is no scope to resolve against — a resource, rather than a sprite inside a tree.
///
#[must_use]
pub fn data(role: DataRole, scope: Scope) -> Radiance {
    scope.palette.0.data(role)
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

/// Returns the palette's detached-surface shadow, with the paint width pinned.
#[must_use]
pub fn shadow(scope: Scope) -> super::Shadow {
    scope.palette.0.shadow(scope.for_paint())
}
