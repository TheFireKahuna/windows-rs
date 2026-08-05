//! Every variant in the widget set, as `const` rows of [`RoleSet`].
//!
//! A variant is a row rather than a function, so the variants a widget has are the length of
//! its table and a variant modifier such as `.accent_subtle()` selects an index into it.

use super::RoleSet;
use crate::role::{Fill, Stroke, Text};

/// The variant a widget starts in.
pub const DEFAULT: u8 = 0;
/// The accent-filled row of [`BUTTON`].
pub const ACCENT: u8 = 1;
/// The tinted row of [`BUTTON`]: an accent-subtle fill under accent text and stroke.
pub const ACCENT_SUBTLE: u8 = 2;
/// The unfilled row of [`BUTTON`]: no fill and no stroke.
pub const GHOST: u8 = 3;

/// The rows `button`, `icon_button` and `select` read.
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

/// The rows `card`, `panel` and `flyout` read.
///
/// They differ in stroke. Their fills differ by rung of the surface ladder, which the
/// [`Scope`](crate::role::Scope) push carries rather than the row.
pub const SURFACE: &[RoleSet] = &[
    // A card: a hairline, so it reads as a surface rather than as a lighter patch.
    RoleSet {
        fill: Some(Fill::Surface),
        text: Text::Primary,
        stroke: Some(Stroke::Subtle),
    },
    // A panel: the window's own plane, and no outline.
    RoleSet {
        fill: Some(Fill::Surface),
        text: Text::Primary,
        stroke: None,
    },
    // A flyout: detached, so its edge is stated more firmly than a card's.
    RoleSet {
        fill: Some(Fill::Surface),
        text: Text::Primary,
        stroke: Some(Stroke::Default),
    },
];

/// The card row of [`SURFACE`].
pub const SURFACE_CARD: u8 = 0;
/// The panel row of [`SURFACE`].
pub const SURFACE_PANEL: u8 = 1;
/// The flyout row of [`SURFACE`].
pub const SURFACE_FLYOUT: u8 = 2;

/// The rows a track reads: a slider's groove, a toggle's body, a meter's bed.
///
/// [`TRACK_ON`] is the accent fill itself, which is what a toggle that is on is.
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

/// The resting row of [`TRACK`]: a groove.
pub const TRACK_OFF: u8 = 0;
/// The filled row of [`TRACK`]: the accent itself.
pub const TRACK_ON: u8 = 1;

/// The single row a text-editable field reads. Focus is drawn by the focus ring rather than
/// by a variant.
pub const FIELD: &[RoleSet] = &[RoleSet {
    fill: Some(Fill::Surface),
    text: Text::Primary,
    stroke: Some(Stroke::Default),
}];

/// The single row one option of a segmented picker reads. Selection is
/// [`ModelState`](super::ModelState) rather than a row, since any control can be selected.
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
    /// An index past the end of its table is clamped by
    /// [`Chrome::roles`](super::super::Chrome::roles) into whichever row is last, which
    /// renders the control as some other variant.
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

    /// A ghost row carries no fill and no stroke, so the mount gives it neither sprite.
    ///
    /// The row decides the sprite count; there is no branch at mount that could.
    #[test]
    fn a_ghost_variant_mints_no_surface() {
        let ghost = BUTTON[GHOST as usize];
        assert!(ghost.fill.is_none() && ghost.stroke.is_none());
    }
}
