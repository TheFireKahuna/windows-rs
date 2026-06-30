//! Design tokens for the self-hosted control library — the scRGB/sRGB chrome
//! palette and the DIP metric scale, mirrored verbatim from the NewAPO parity
//! spec (`gui/docs/DCOMP-CONTROL-PARITY.md` §A and `newapo-ui/src/theme.rs`).
//!
//! Every metric and colour a control paints comes from here, not a raw literal:
//! one surface ladder, one stroke ladder, one accent ramp (derived, not stored),
//! one spacing scale, one radius scale. Colours are authored in 8-bit sRGB
//! ([`Color`]); the paint layer gamma-decodes them to linear scRGB once via
//! `node::linear` before they reach a brush.
//!
//! This is the complete parity token table — a few tokens are not yet consumed
//! by a control but are kept for fidelity and future screens.
#![allow(dead_code)]

use crate::Color;

// ── A.1 Surfaces (opaque — one elevation ladder) ─────────────────────────────
pub const SURFACE_SUNKEN: Color = rgb(0x1c, 0x1c, 0x1c);
pub const SURFACE: Color = rgb(0x28, 0x28, 0x28);
pub const SURFACE_RAISED: Color = rgb(0x30, 0x30, 0x30);
pub const SURFACE_HOVER: Color = rgb(0x34, 0x34, 0x34);

// ── A.2 Strokes (white-alpha, derived via `w`) ───────────────────────────────
pub fn stroke_subtle() -> Color {
    w(0.04)
}
pub fn stroke_divider() -> Color {
    w(0.06)
}
pub fn stroke() -> Color {
    w(0.08)
}
pub fn stroke_strong() -> Color {
    w(0.15)
}

/// White at `alpha` (the pervasive hairline / wash helper).
pub const fn w(alpha: f32) -> Color {
    Color {
        a: (alpha * 255.0) as u8,
        r: 255,
        g: 255,
        b: 255,
    }
}

/// Black at `alpha` (dark insets / scrims / drop shadows).
pub const fn b(alpha: f32) -> Color {
    Color {
        a: (alpha * 255.0) as u8,
        r: 0,
        g: 0,
        b: 0,
    }
}

/// An arbitrary hue at `alpha` (badge washes, fill tints).
pub const fn with_alpha(c: Color, alpha: f32) -> Color {
    Color {
        a: (alpha * 255.0) as u8,
        r: c.r,
        g: c.g,
        b: c.b,
    }
}

// ── A.3 Text (four roles) + disabled ─────────────────────────────────────────
pub const TEXT: Color = rgb(0xff, 0xff, 0xff);
pub const TEXT_SECONDARY: Color = rgb(0xaa, 0xaa, 0xaa);
pub const TEXT_TERTIARY: Color = rgb(0x77, 0x77, 0x77);
pub const TEXT_DISABLED: Color = rgb(0x55, 0x55, 0x55);
pub const DISABLED_OPACITY: f32 = 0.4;

// ── A.4 Accent (one stored hue; ramp derived) ────────────────────────────────
pub const ACCENT: Color = rgb(0x0e, 0xa5, 0xe9);
pub fn accent_light() -> Color {
    rgb(0x38, 0xbd, 0xf8)
}
pub fn accent_dark() -> Color {
    rgb(0x08, 0x91, 0xb2)
}
pub fn accent_glow() -> Color {
    with_alpha(ACCENT, 0.25)
}
pub fn accent_fill() -> Color {
    with_alpha(ACCENT, 0.12)
}
pub fn accent_subtle() -> Color {
    with_alpha(ACCENT, 0.08)
}

// ── A.5 Status ───────────────────────────────────────────────────────────────
pub const OK: Color = rgb(0x34, 0xd3, 0x99);
pub const WARN: Color = rgb(0xf5, 0x9e, 0x0b);
pub const BAD: Color = rgb(0xfb, 0x71, 0x85);
pub const DANGER: Color = rgb(0xef, 0x44, 0x44);

// ── A.6 Spacing (4px grid) ───────────────────────────────────────────────────
pub const SPACE_4: f32 = 4.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_12: f32 = 12.0;
pub const SPACE_16: f32 = 16.0;
pub const SPACE_24: f32 = 24.0;
pub const SPACE_32: f32 = 32.0;

// ── A.7 Radius ───────────────────────────────────────────────────────────────
pub const RADIUS_BADGE: f32 = 3.0;
pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 8.0;
pub const RADIUS_LG: f32 = 12.0;
pub const RADIUS_PILL: f32 = 8.0;

// ── A.8 Control metrics (DIP) ────────────────────────────────────────────────
pub const ROW_H: f32 = 32.0;
pub const ROW_H_SM: f32 = 24.0;
pub const BORDER_W: f32 = 1.0;
pub const SLIDER_TRACK: f32 = 4.0;
pub const SLIDER_THUMB: f32 = 14.0;
pub const NAV_RAIL_W: f32 = SPACE_32 + SPACE_16; // 48

// ── A.9 Durations (seconds — the ink cross-fade rate uses these) ─────────────
pub const DUR_FAST: f32 = 0.150;

// ── A.10 Fonts ───────────────────────────────────────────────────────────────
/// Icon-glyph face. A [`crate::Symbol`]'s integer value is the PUA codepoint.
pub const FONT_ICON: &str = "Segoe Fluent Icons";
pub const FONT_SIZE_MICRO: f32 = 9.0;
pub const FONT_SIZE_SM: f32 = 11.0;
pub const FONT_SIZE_MD: f32 = 13.0;
pub const FONT_SIZE_LG: f32 = 16.0;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { a: 255, r, g, b }
}
