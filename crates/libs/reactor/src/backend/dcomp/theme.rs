//! Default design tokens for the self-hosted control library — the **WinUI 3
//! Fluent dark-theme defaults**, extracted from the `microsoft/microsoft-ui-xaml`
//! sources (`Common_themeresources_any.xaml` "Default" dictionary,
//! `CornerRadius_themeresources.xaml`, `TextBlock_themeresources.xaml`, and the
//! per-control theme resource files).
//!
//! This table is deliberately **application-agnostic**: it carries no product
//! palette and no design decisions beyond what stock WinUI ships. A host
//! application restyles the drawn controls by supplying its own values (theme
//! injection), exactly as a XAML app would override the Fluent resources.
//!
//! Colours are Fluent's own encoding — mostly **white at a low alpha** composited
//! over the base surface (`#202020`), not opaque greys. Each token cites the
//! Fluent resource it mirrors; a few metrics keep this library's drawn-control
//! geometry where it structurally differs from XAML templates (noted inline).
#![allow(dead_code)]

use crate::Color;

// ── A.1 Surfaces (Fluent SolidBackgroundFill ladder) ─────────────────────────
/// `SolidBackgroundFillColorSecondary` (#FF1C1C1C).
pub const SURFACE_SUNKEN: Color = rgb(0x1c, 0x1c, 0x1c);
/// `SolidBackgroundFillColorTertiary` (#FF282828).
pub const SURFACE: Color = rgb(0x28, 0x28, 0x28);
/// `SolidBackgroundFillColorQuarternary` (#FF2C2C2C) — also the flyout/menu solid.
pub const SURFACE_RAISED: Color = rgb(0x2c, 0x2c, 0x2c);
/// `SolidBackgroundFillColorQuinary` (#FF333333) — nearest solid to Fluent's
/// hover wash (`SubtleFillColorSecondary` over the surface).
pub const SURFACE_HOVER: Color = rgb(0x33, 0x33, 0x33);

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

/// White at `alpha` (the pervasive hairline / wash helper). White is gamma-invariant,
/// so linear white is `1.0`; `alpha` is a linear opacity fraction.
pub const fn w(alpha: f32) -> Color {
    Color::scrgb(1.0, 1.0, 1.0, alpha)
}

/// Black at `alpha` (dark insets / scrims / drop shadows). Linear black is `0.0`.
pub const fn b(alpha: f32) -> Color {
    Color::scrgb(0.0, 0.0, 0.0, alpha)
}

/// An arbitrary hue at `alpha` (badge washes, fill tints) — keeps `c`'s linear RGB,
/// overrides the opacity.
pub const fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::scrgb(c.r, c.g, c.b, alpha)
}

// ── A.3 Text (Fluent TextFillColor ramp — white-alpha, not opaque greys) ─────
/// `TextFillColorPrimary` (#FFFFFFFF).
pub const TEXT: Color = rgb(0xff, 0xff, 0xff);
/// `TextFillColorSecondary` (#C5FFFFFF).
pub const TEXT_SECONDARY: Color = rgba(0xff, 0xff, 0xff, 0xc5);
/// `TextFillColorTertiary` (#87FFFFFF).
pub const TEXT_TERTIARY: Color = rgba(0xff, 0xff, 0xff, 0x87);
/// `TextFillColorDisabled` (#5DFFFFFF).
pub const TEXT_DISABLED: Color = rgba(0xff, 0xff, 0xff, 0x5d);
/// Fluent has no global disabled-opacity multiplier (each role has a dedicated
/// disabled colour); kept as the drawn controls' single disabled dim.
pub const DISABLED_OPACITY: f32 = 0.4;

// ── A.4 Accent (Windows 11 default ramp; dark theme uses the LIGHT shades) ───
/// `AccentFillColorDefault` = `SystemAccentColorLight2` (#FF4CC2FF, the Windows 11
/// default-blue ramp) — dark-theme control fills never use the base accent.
pub const ACCENT: Color = rgb(0x4c, 0xc2, 0xff);
/// `AccentTextFillColorPrimary` = `SystemAccentColorLight3` (#FF99EBFF).
pub fn accent_light() -> Color {
    rgb(0x99, 0xeb, 0xff)
}
/// `AccentFillColorTertiary` — Fluent's pressed accent is Light2 at 0.8 opacity.
pub fn accent_dark() -> Color {
    with_alpha(ACCENT, 0.8)
}
/// Accent washes (no Fluent equivalent — this library's drawn selection/glow
/// affordances; a host theme typically overrides these).
pub fn accent_glow() -> Color {
    with_alpha(ACCENT, 0.25)
}
pub fn accent_fill() -> Color {
    with_alpha(ACCENT, 0.12)
}
pub fn accent_subtle() -> Color {
    with_alpha(ACCENT, 0.08)
}

// ── A.5 Status (Fluent SystemFillColor roles) ────────────────────────────────
/// `SystemFillColorSuccess` (#FF6CCB5F).
pub const OK: Color = rgb(0x6c, 0xcb, 0x5f);
/// `SystemFillColorCaution` (#FFFCE100).
pub const WARN: Color = rgb(0xfc, 0xe1, 0x00);
/// `SystemFillColorCritical` (#FFFF99A4) — Fluent has a single critical role.
pub const BAD: Color = rgb(0xff, 0x99, 0xa4);
/// `SystemFillColorCritical` (#FFFF99A4).
pub const DANGER: Color = rgb(0xff, 0x99, 0xa4);

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
