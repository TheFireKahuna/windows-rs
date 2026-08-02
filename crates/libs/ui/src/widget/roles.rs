//! Every variant in the widget set, as `const` rows.
//!
//! This is the exact place a widget set grows a cross-product, so it is the place where a
//! variant must be a **row and not a function**. Tables do not accrete behaviour: a fifth
//! variant is visibly a fifth row, where a fifth function is a fifth body with its own
//! conditionals, and by the time anyone counts there are sixty-six of them.
//!
//! Reading a row is the whole of `.accent_subtle()`.

use super::RoleSet;
use crate::role::{Fill, Stroke, Text};

/// The variant a widget starts in. Named, because `[0]` at four call sites is four chances
/// to mean a different row.
pub const DEFAULT: u8 = 0;
pub const ACCENT: u8 = 1;
pub const ACCENT_SUBTLE: u8 = 2;
pub const GHOST: u8 = 3;

/// `button`, `icon_button` and `select`, which is a button that opens something.
pub const BUTTON: &[RoleSet] = &[
    RoleSet {
        fill: Some(Fill::Surface),
        text: Text::Primary,
        stroke: Some(Stroke::Subtle),
    },
    RoleSet {
        fill: Some(Fill::Accent),
        text: Text::OnAccent,
        stroke: None,
    },
    RoleSet {
        fill: Some(Fill::AccentSubtle),
        text: Text::Accent,
        stroke: Some(Stroke::Accent),
    },
    RoleSet {
        fill: None,
        text: Text::Secondary,
        stroke: None,
    },
];

/// A card, a panel and a flyout differ in elevation and stroke, not in variant — the rung
/// is a [`Scope`](crate::role::Scope) push and carries the fill with it.
pub const SURFACE: &[RoleSet] = &[
    // A card: a hairline, so it reads as a surface rather than as a lighter patch.
    RoleSet {
        fill: Some(Fill::Surface),
        text: Text::Primary,
        stroke: Some(Stroke::Subtle),
    },
    // A panel: the window's own plane, so an outline would be drawing a box around nothing.
    RoleSet {
        fill: Some(Fill::Surface),
        text: Text::Primary,
        stroke: None,
    },
    // A flyout: detached, so it states its own edge more firmly than a card does.
    RoleSet {
        fill: Some(Fill::Surface),
        text: Text::Primary,
        stroke: Some(Stroke::Default),
    },
];

pub const SURFACE_CARD: u8 = 0;
pub const SURFACE_PANEL: u8 = 1;
pub const SURFACE_FLYOUT: u8 = 2;

/// A track a value runs along: a slider's groove, a toggle's body, a meter's bed.
///
/// One row, and the second exists only because a toggle that is on is the accent itself
/// rather than a groove with accent in it.
pub const TRACK: &[RoleSet] = &[
    RoleSet {
        fill: Some(Fill::Surface),
        text: Text::Secondary,
        stroke: Some(Stroke::Subtle),
    },
    RoleSet {
        fill: Some(Fill::Accent),
        text: Text::OnAccent,
        stroke: None,
    },
];

pub const TRACK_OFF: u8 = 0;
pub const TRACK_ON: u8 = 1;

/// A text-editable field. Its resting state is a groove; the accent arrives with focus,
/// which is the focus ring's business and not a variant.
pub const FIELD: &[RoleSet] = &[RoleSet {
    fill: Some(Fill::Surface),
    text: Text::Primary,
    stroke: Some(Stroke::Default),
}];

/// One option of a segmented picker. Selection is [`ModelState`](super::ModelState) rather
/// than a row, because it is model state that any control can be in and not something only
/// this one has.
pub const OPTION: &[RoleSet] = &[RoleSet {
    fill: None,
    text: Text::Secondary,
    stroke: None,
}];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every named index addresses a row that exists.
    ///
    /// The failure this closes is a variant method minting an index its table is too short
    /// for, which [`Chrome::roles`](super::super::Chrome::roles) would clamp into whichever
    /// row happened to be last — a control that silently renders as a different variant.
    #[test]
    fn every_named_variant_indexes_its_own_table() {
        for at in [DEFAULT, ACCENT, ACCENT_SUBTLE, GHOST] {
            assert!((at as usize) < BUTTON.len());
        }
        for at in [SURFACE_CARD, SURFACE_PANEL, SURFACE_FLYOUT] {
            assert!((at as usize) < SURFACE.len());
        }
        for at in [TRACK_OFF, TRACK_ON] {
            assert!((at as usize) < TRACK.len());
        }
    }

    /// A ghost has no fill and no stroke, so it costs the sprites it does not have.
    ///
    /// This is what makes the table load-bearing rather than decorative: the row decides
    /// the visual count, not a branch at mount.
    #[test]
    fn a_ghost_variant_mints_no_surface() {
        let ghost = BUTTON[GHOST as usize];
        assert!(ghost.fill.is_none() && ghost.stroke.is_none());
    }
}
