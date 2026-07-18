//! Default design tokens for the self-hosted control library — the **WinUI 3
//! Fluent theme defaults**, extracted from the `microsoft/microsoft-ui-xaml`
//! sources (`Common_themeresources_any.xaml` "Default" (dark) and "Light"
//! dictionaries, `CornerRadius_themeresources.xaml`,
//! `TextBlock_themeresources.xaml`, and the per-control theme resource files).
//!
//! The tables are deliberately **application-agnostic**: they carry no product
//! palette and no design decisions beyond what stock WinUI ships. A host
//! application restyles the drawn controls by supplying its own values —
//! [`set_host_tokens`] — exactly as a XAML app would override the Fluent
//! resources. Every color accessor below resolves **per scheme** at call time:
//! the host table for the current [`ColorScheme`] when one was installed, the
//! matching Fluent table otherwise. Non-color metrics (spacing, radii, control
//! geometry, durations, fonts) are structural and stay `const`.
//!
//! Colours are Fluent's own encoding — mostly **ink at a low alpha** composited
//! over the base surface (white ink on the dark table, black on the light), not
//! opaque greys. Each token cites the Fluent resource it mirrors; a few metrics
//! keep this library's drawn-control geometry where it structurally differs
//! from XAML templates (noted inline).
#![allow(dead_code)]

use std::sync::OnceLock;

use crate::style::{current_color_scheme, ColorScheme};
use crate::Color;

// ── Host token injection ─────────────────────────────────────────────────────

/// The color-bearing tokens a host application can override, wholesale. The
/// [`Default`] is the stock Fluent dark table below; hosts struct-update from it
/// (`HostTokens { text: …, ..Default::default() }`) or supply every field.
///
/// The library never interprets these values — an HDR host hands in
/// extended-range linear scRGB (its own luminance anchor included) and the
/// drawn controls simply use them.
#[derive(Copy, Clone, Debug)]
pub struct HostTokens {
    /// The ink basis every wash and stroke derives from ([`w`], [`stroke`], …):
    /// diffuse white on a dark table, black on a light table. Fluent's dark ink
    /// is plain linear `1.0`; an HDR host passes its anchored white so
    /// hairlines sit on its luminance scale.
    pub ink: Color,
    /// `SolidBackgroundFillColorSecondary` — window base.
    pub surface_sunken: Color,
    /// `SolidBackgroundFillColorTertiary` — card / panel surface.
    pub surface: Color,
    /// `SolidBackgroundFillColorQuarternary` — flyout / menu solid.
    pub surface_raised: Color,
    /// Nearest solid to Fluent's hover wash over the surface.
    pub surface_hover: Color,
    /// `TextFillColorPrimary` — also the default foreground of un-styled text.
    pub text: Color,
    /// `TextFillColorSecondary`.
    pub text_secondary: Color,
    /// `TextFillColorTertiary`.
    pub text_tertiary: Color,
    /// `TextFillColorDisabled`.
    pub text_disabled: Color,
    /// `AccentFillColorDefault`.
    pub accent: Color,
    /// `AccentTextFillColorPrimary`.
    pub accent_light: Color,
    /// `AccentFillColorTertiary` (pressed accent).
    pub accent_dark: Color,
    /// `SystemFillColorSuccess`.
    pub ok: Color,
    /// `SystemFillColorCaution`.
    pub warn: Color,
    /// `SystemFillColorCritical` (soft).
    pub bad: Color,
    /// `SystemFillColorCritical`.
    pub danger: Color,
    /// The drawn controls' single disabled dim (Fluent has per-role disabled
    /// colours; this library dims uniformly).
    pub disabled_opacity: f32,
}

/// The stock WinUI 3 Fluent **dark** table (the values documented per-accessor
/// below). `Color::rgb`/`rgba` decode sRGB hex to plain linear — no luminance
/// anchor, by design.
const FLUENT_DARK: HostTokens = HostTokens {
    ink: Color::scrgb(1.0, 1.0, 1.0, 1.0),
    surface_sunken: rgb(0x1c, 0x1c, 0x1c),
    surface: rgb(0x28, 0x28, 0x28),
    surface_raised: rgb(0x2c, 0x2c, 0x2c),
    surface_hover: rgb(0x33, 0x33, 0x33),
    text: rgb(0xff, 0xff, 0xff),
    text_secondary: rgba(0xff, 0xff, 0xff, 0xc5),
    text_tertiary: rgba(0xff, 0xff, 0xff, 0x87),
    text_disabled: rgba(0xff, 0xff, 0xff, 0x5d),
    accent: rgb(0x4c, 0xc2, 0xff),
    // `with_alpha(accent, 0.8)` through the dark wash fit, precomputed so the
    // table stays const now that the public helper resolves per scheme.
    accent_light: rgb(0x99, 0xeb, 0xff),
    accent_dark: Color::scrgb(
        rgb(0x4c, 0xc2, 0xff).r,
        rgb(0x4c, 0xc2, 0xff).g,
        rgb(0x4c, 0xc2, 0xff).b,
        dark_wash_alpha(0.8),
    ),
    ok: rgb(0x6c, 0xcb, 0x5f),
    warn: rgb(0xfc, 0xe1, 0x00),
    bad: rgb(0xff, 0x99, 0xa4),
    danger: rgb(0xff, 0x99, 0xa4),
    disabled_opacity: 0.4,
};

/// The stock WinUI 3 Fluent **light** table ("Light" resource dictionary; black
/// ink, near-white surface ladder, the accent's dark shades for control fills).
const FLUENT_LIGHT: HostTokens = HostTokens {
    ink: Color::scrgb(0.0, 0.0, 0.0, 1.0),
    // `SolidBackgroundFillColor{Secondary,Tertiary,Quarternary}` light values;
    // hover is the nearest solid to the light hover wash.
    surface_sunken: rgb(0xee, 0xee, 0xee),
    surface: rgb(0xf9, 0xf9, 0xf9),
    surface_raised: rgb(0xff, 0xff, 0xff),
    surface_hover: rgb(0xec, 0xec, 0xec),
    // `TextFillColor*` light — black at Fluent's authored alphas.
    text: rgba(0x00, 0x00, 0x00, 0xe4),
    text_secondary: rgba(0x00, 0x00, 0x00, 0x9e),
    text_tertiary: rgba(0x00, 0x00, 0x00, 0x72),
    text_disabled: rgba(0x00, 0x00, 0x00, 0x5c),
    // Light theme fills with the accent's DARK shades (the mirror of dark
    // filling with the light shades): Dark1 base, Dark2 text/hover.
    accent: rgb(0x00, 0x5f, 0xb8),
    accent_light: rgb(0x00, 0x3e, 0x92),
    accent_dark: Color::scrgb(
        rgb(0x00, 0x5f, 0xb8).r,
        rgb(0x00, 0x5f, 0xb8).g,
        rgb(0x00, 0x5f, 0xb8).b,
        0.8,
    ),
    // `SystemFillColor{Success,Caution,Critical}` light.
    ok: rgb(0x0f, 0x7b, 0x0f),
    warn: rgb(0x9d, 0x5d, 0x00),
    bad: rgb(0xc4, 0x2b, 0x1c),
    danger: rgb(0xc4, 0x2b, 0x1c),
    disabled_opacity: 0.4,
};

impl Default for HostTokens {
    fn default() -> Self {
        FLUENT_DARK
    }
}

/// The installed host tables, if any. Written once before the window exists,
/// read from the UI thread thereafter.
static HOST_DARK: OnceLock<HostTokens> = OnceLock::new();
static HOST_LIGHT: OnceLock<HostTokens> = OnceLock::new();

/// Install the host application's token tables, one per [`ColorScheme`]. Call
/// **once, before the app window is created** — drawn controls resolve tokens
/// at paint time (per the current scheme), but tables swapped mid-run do not
/// repaint existing content. A second call is ignored (first one wins).
pub fn set_host_tokens(dark: HostTokens, light: HostTokens) {
    let _ = HOST_DARK.set(dark);
    let _ = HOST_LIGHT.set(light);
}

/// The active table for the current scheme: the host's, else Fluent.
#[inline]
fn host() -> &'static HostTokens {
    match current_color_scheme() {
        ColorScheme::Dark => HOST_DARK.get().unwrap_or(&FLUENT_DARK),
        ColorScheme::Light => HOST_LIGHT.get().unwrap_or(&FLUENT_LIGHT),
    }
}

// ── A.1 Surfaces (Fluent SolidBackgroundFill ladder) ─────────────────────────
/// `SolidBackgroundFillColorSecondary` (#FF1C1C1C).
pub fn surface_sunken() -> Color {
    host().surface_sunken
}
/// `SolidBackgroundFillColorTertiary` (#FF282828).
pub fn surface() -> Color {
    host().surface
}
/// `SolidBackgroundFillColorQuarternary` (#FF2C2C2C) — also the flyout/menu solid.
pub fn surface_raised() -> Color {
    host().surface_raised
}
/// `SolidBackgroundFillColorQuinary` (#FF333333) — nearest solid to Fluent's
/// hover wash (`SubtleFillColorSecondary` over the surface).
pub fn surface_hover() -> Color {
    host().surface_hover
}

// ── A.2 Strokes (white-alpha, derived via `w`) ───────────────────────────────
/// `SubtleFillColorTertiary` (#0AFFFFFF) — the faintest separation.
pub fn stroke_subtle() -> Color {
    w(0x0a as f32 / 255.0)
}
/// `DividerStrokeColorDefault` (#15FFFFFF).
pub fn stroke_divider() -> Color {
    w(0x15 as f32 / 255.0)
}
/// `ControlStrokeColorDefault` (#12FFFFFF) — the standard control border.
pub fn stroke() -> Color {
    w(0x12 as f32 / 255.0)
}
/// `ControlStrongStrokeColorDefault` (#8BFFFFFF) — toggle/checkbox rest border.
pub fn stroke_strong() -> Color {
    w(0x8b as f32 / 255.0)
}

/// sRGB-authored → linear-blend wash alpha over the **dark** base. Fluent's
/// translucent tokens (`#0AFFFFFF`-style washes) assume gamma-space
/// compositing; this pipeline blends in linear light, where the same alpha
/// lands ~4× hotter over a dark surface. The cubic reproduces the authored
/// appearance over the dark base (fit against `#282828`, within ~1% across
/// `0..=1`).
const fn dark_wash_alpha(a: f32) -> f32 {
    let out = a * (0.2113 + 0.5905 * a + 0.1984 * a * a);
    if out > 1.0 {
        1.0
    } else {
        out
    }
}

/// The light-base counterpart: a black-ink wash authored in sRGB also lands
/// hotter in linear over a light surface (a 6% authored scrim reads ~13%).
/// Cubic fit of the exact gamma-blend conversion against the light card
/// surface (`#f9f9f9`), within ~1% across `0..=1` — the fit is insensitive to
/// the exact near-white base, so it matches the app-side light cubic.
const fn light_wash_alpha(a: f32) -> f32 {
    let out = a * (2.265 - 1.516 * a + 0.251 * a * a);
    if out > 1.0 {
        1.0
    } else {
        out
    }
}

/// sRGB-authored → linear-blend wash alpha for the current scheme's ink over
/// its own surface ladder.
pub(crate) fn wash_alpha(a: f32) -> f32 {
    match current_color_scheme() {
        ColorScheme::Dark => dark_wash_alpha(a),
        ColorScheme::Light => light_wash_alpha(a),
    }
}

/// The scheme's ink at `alpha` (the pervasive hairline / wash helper): the host
/// table's [`HostTokens::ink`] — white on dark, black on light. `alpha` is the
/// sRGB-authored opacity — converted by [`wash_alpha`] for the linear blend.
pub fn w(alpha: f32) -> Color {
    let ink = host().ink;
    Color::scrgb(ink.r, ink.g, ink.b, wash_alpha(alpha))
}

/// Black at `alpha` (insets / scrims / drop shadows — shadows stay dark on both
/// schemes). Linear black is `0.0`.
pub const fn b(alpha: f32) -> Color {
    Color::scrgb(0.0, 0.0, 0.0, alpha)
}

/// An arbitrary hue at `alpha` (badge washes, fill tints) — keeps `c`'s linear RGB,
/// overrides the opacity. `alpha` is sRGB-authored ([`wash_alpha`]).
pub fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::scrgb(c.r, c.g, c.b, wash_alpha(alpha))
}

// ── A.3 Text (Fluent TextFillColor ramp — white-alpha, not opaque greys) ─────
/// `TextFillColorPrimary` (#FFFFFFFF) — also the default foreground for text
/// with no explicit style.
pub fn text() -> Color {
    host().text
}
/// `TextFillColorSecondary` (#C5FFFFFF).
pub fn text_secondary() -> Color {
    host().text_secondary
}
/// `TextFillColorTertiary` (#87FFFFFF).
pub fn text_tertiary() -> Color {
    host().text_tertiary
}
/// `TextFillColorDisabled` (#5DFFFFFF).
pub fn text_disabled() -> Color {
    host().text_disabled
}
/// Fluent has no global disabled-opacity multiplier (each role has a dedicated
/// disabled colour); kept as the drawn controls' single disabled dim.
pub fn disabled_opacity() -> f32 {
    host().disabled_opacity
}

// ── A.4 Accent (Windows 11 default ramp; dark theme uses the LIGHT shades) ───
/// `AccentFillColorDefault` = `SystemAccentColorLight2` (#FF4CC2FF, the Windows 11
/// default-blue ramp) — dark-theme control fills never use the base accent.
pub fn accent() -> Color {
    host().accent
}
/// `AccentTextFillColorPrimary` = `SystemAccentColorLight3` (#FF99EBFF).
pub fn accent_light() -> Color {
    host().accent_light
}
/// `AccentFillColorTertiary` — Fluent's pressed accent is Light2 at 0.8 opacity.
pub fn accent_dark() -> Color {
    host().accent_dark
}
/// Accent washes (no Fluent equivalent — this library's drawn selection/glow
/// affordances; derived from the active accent so a host override tints them).
pub fn accent_glow() -> Color {
    with_alpha(accent(), 0.25)
}
pub fn accent_fill() -> Color {
    with_alpha(accent(), 0.12)
}
pub fn accent_subtle() -> Color {
    with_alpha(accent(), 0.08)
}

// ── A.5 Status (Fluent SystemFillColor roles) ────────────────────────────────
/// `SystemFillColorSuccess` (#FF6CCB5F).
pub fn ok() -> Color {
    host().ok
}
/// `SystemFillColorCaution` (#FFFCE100).
pub fn warn() -> Color {
    host().warn
}
/// `SystemFillColorCritical` (#FFFF99A4) — Fluent has a single critical role.
pub fn bad() -> Color {
    host().bad
}
/// `SystemFillColorCritical` (#FFFF99A4).
pub fn danger() -> Color {
    host().danger
}

// ── A.6 Spacing (Fluent's documented 4-epx grid — convention, not resources) ─
pub const SPACE_4: f32 = 4.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_12: f32 = 12.0;
pub const SPACE_16: f32 = 16.0;
pub const SPACE_24: f32 = 24.0;
pub const SPACE_32: f32 = 32.0;

// ── A.7 Radius ───────────────────────────────────────────────────────────────
/// No Fluent token (badges in Fluent are fully round); drawn-control geometry.
pub const RADIUS_BADGE: f32 = 3.0;
/// `ControlCornerRadius` (4).
pub const RADIUS_SM: f32 = 4.0;
/// `OverlayCornerRadius` (8) — flyouts, menus, dialogs.
pub const RADIUS_MD: f32 = 8.0;
/// No Fluent token; drawn-control geometry.
pub const RADIUS_LG: f32 = 12.0;
/// No Fluent token (Fluent pills are fully round, height/2); drawn geometry.
pub const RADIUS_PILL: f32 = 8.0;

// ── A.8 Control metrics (DIP) ────────────────────────────────────────────────
/// `TextControlThemeMinHeight` / `ComboBoxMinHeight` (32).
pub const ROW_H: f32 = 32.0;
/// Compact rows (Fluent narrow menu-item padding lands ≈24).
pub const ROW_H_SM: f32 = 24.0;
/// Standard control `BorderThickness` (1).
pub const BORDER_W: f32 = 1.0;
/// `SliderTrackThemeHeight` (4).
pub const SLIDER_TRACK: f32 = 4.0;
/// This library draws a single-circle thumb (Fluent's is an 18-epx ring + 12-epx
/// accent dot); 14 is the nearest single-circle visual weight.
pub const SLIDER_THUMB: f32 = 14.0;
/// `NavigationView.CompactPaneLength` default (48).
pub const NAV_RAIL_W: f32 = SPACE_32 + SPACE_16; // 48

// ── A.9 Durations (seconds) ──────────────────────────────────────────────────
/// `ControlFastAnimationDuration` (167 ms).
pub const DUR_FAST: f32 = 0.167;

// ── A.10 Fonts (Fluent type ramp) ────────────────────────────────────────────
/// Icon-glyph face (`SymbolThemeFontFamily`). A [`crate::Symbol`]'s integer value
/// is the PUA codepoint.
pub const FONT_ICON: &str = "Segoe Fluent Icons";
/// Below the Fluent ramp (no equivalent; smallest drawn-control annotations).
pub const FONT_SIZE_MICRO: f32 = 9.0;
/// `CaptionTextBlockFontSize` (12).
pub const FONT_SIZE_SM: f32 = 12.0;
/// `BodyTextBlockFontSize` (14).
pub const FONT_SIZE_MD: f32 = 14.0;
/// `SubtitleTextBlockFontSize` (20).
pub const FONT_SIZE_LG: f32 = 20.0;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::rgb(r, g, b)
}

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::rgba(r, g, b, a)
}
