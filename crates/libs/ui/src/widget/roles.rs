//! The vocabulary a widget declares, and every variant in the stock set as `const` rows.
//!
//! A variant is a row rather than a function, so the variants a widget has are the length of its
//! table and a variant modifier such as an accent-subtle button selects an index into it.
//!
//! The rest is what a recipe says about a control that is not a colour: how a channel moves, which
//! wash a state fades in, what a pointer means, the span a value runs over, and the mapping that
//! places a part at a fraction. The chrome ladder is here because its rows are [`RoleSet`]s.

use crate::layout::Edge;
use crate::role::{Fill, Metric, Stroke, Text};
use windows_scene::Prop;

// ── colour rows ─────────────────────────────────────────────────────────────────────

/// A widget's colour triple: one row of a table.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RoleSet {
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub text: Text,
}

/// Which model state a control's roles are resolved in.
///
/// Hover and press are not here: they are the wash's opacity. This is the state that swaps a base
/// role — a selected row, a disabled control.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ModelState {
    #[default]
    Rest,
    Selected,
    Disabled,
}

impl RoleSet {
    /// Returns one row, so a table below is one line per variant.
    #[must_use]
    pub const fn new(fill: Option<Fill>, stroke: Option<Stroke>, text: Text) -> Self {
        Self { fill, stroke, text }
    }

    /// Returns these roles resolved in `state`.
    ///
    /// Selected is a solid accent fill under accent text and no stroke, whatever the resting
    /// row states: the resting stroke separates an unfilled surface from the ground behind it,
    /// and kept it would draw the groove's edge around the fill that replaced it. One hue
    /// carries selection across the fill and the label, so a selected option, rail item and
    /// row announce it at one strength.
    #[must_use]
    pub const fn in_state(self, state: ModelState) -> Self {
        match state {
            ModelState::Rest => self,
            ModelState::Selected => Self {
                fill: Some(Fill::Selected),
                stroke: None,
                text: Text::Accent,
            },
            ModelState::Disabled => Self {
                text: Text::Disabled,
                stroke: None,
                ..self
            },
        }
    }
}

/// The variant a widget starts in.
pub const DEFAULT: u8 = 0;
/// The accent-filled row of [`BUTTON`].
pub const ACCENT: u8 = 1;
/// The tinted row of [`BUTTON`]: an accent-subtle fill under accent text.
pub const ACCENT_SUBTLE: u8 = 2;
/// The unfilled row of [`BUTTON`]: no fill and no stroke.
pub const GHOST: u8 = 3;

/// The rows `button`, `icon_button` and `select` read.
///
/// The subtle variant is an unoutlined accent wash. Applications may combine translucent fills
/// and real outlines in their own chrome recipes.
pub const BUTTON: [RoleSet; 4] = [
    RoleSet::new(Some(Fill::Surface), Some(Stroke::Subtle), Text::Primary),
    RoleSet::new(Some(Fill::Accent), None, Text::OnAccent),
    RoleSet::new(Some(Fill::AccentSubtle), None, Text::Accent),
    RoleSet::new(None, None, Text::Secondary),
];

/// The card row of [`SURFACE`]: a hairline, so it reads as a surface rather than a lighter patch.
pub const SURFACE_CARD: u8 = 0;
/// The panel row of [`SURFACE`]: the window's own plane, and no outline.
pub const SURFACE_PANEL: u8 = 1;
/// The flyout row of [`SURFACE`].
pub const SURFACE_FLYOUT: u8 = 2;

/// The rows `card`, `panel` and `flyout` read.
///
/// They differ in stroke. Their fills differ by rung of the surface ladder, which the scope push
/// carries rather than the row.
pub const SURFACE: [RoleSet; 3] = [
    RoleSet::new(Some(Fill::Surface), Some(Stroke::Subtle), Text::Primary),
    RoleSet::new(Some(Fill::Surface), None, Text::Primary),
    RoleSet::new(Some(Fill::Surface), Some(Stroke::Default), Text::Primary),
];

/// The resting row of [`TRACK`]: a groove.
pub const TRACK_OFF: u8 = 0;
/// The filled row of [`TRACK`]: the accent itself, which is what a toggle that is on is.
pub const TRACK_ON: u8 = 1;

/// The rows a track reads: a slider's groove, a toggle's body, a meter's bed.
pub const TRACK: [RoleSet; 2] = [
    RoleSet::new(Some(Fill::Pressed), None, Text::Secondary),
    RoleSet::new(Some(Fill::Accent), None, Text::OnAccent),
];

/// The single row a text-editable field reads. Focus is drawn by the window's ring rather than by
/// a variant.
pub const FIELD: [RoleSet; 1] = [RoleSet::new(
    Some(Fill::Pressed),
    Some(Stroke::Default),
    Text::Primary,
)];

/// The single row one option of a segmented picker reads. Selection is a [`ModelState`] rather
/// than a row, since any control can be selected.
///
/// Tertiary at rest: the options a picker is not on name the alternatives to the one it is on, and
/// set at the strength of ordinary secondary text they read as three live values.
pub const OPTION: [RoleSet; 1] = [RoleSet::new(None, None, Text::Tertiary)];

/// The single row a groove reads: a segmented picker's track, and no outline.
///
/// [`Fill::Pressed`] is the sunken rung of the surface ladder, which is what a groove is — the
/// same value a control resolves while it is held down, resolved here as a resting surface rather
/// than as a state.
pub const GROOVE: [RoleSet; 1] = [RoleSet::new(Some(Fill::Pressed), None, Text::Secondary)];

// ── what a recipe declares ──────────────────────────────────────────────────────────

/// How a channel moves when its value changes.
///
/// Declared per channel by the recipe, so two call sites cannot disagree about one control: a
/// meter level springs, and a slider thumb the application writes lands where it was put.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    /// The channel lands on the new value with no animation.
    #[default]
    Snap,
    /// The channel springs to the new value.
    Chrome,
}

/// Which derived wash a state fades in.
///
/// A state change is a crossfade of a wash over the base colour rather than an interpolation
/// towards a second base colour: a sprite's colour is an FP16 surface cell, a composition colour
/// brush is 8-bit, and no brush interpolates between two FP16 sources.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Wash {
    /// The scope's foreground, at the state's opacity.
    Ink,
    /// The scope's accent fill, at the state's opacity.
    Accent,
}

/// What a widget names instead of writing an automation declaration.
///
/// The lowering synthesises the declaration from this, the slot's own text, and the channel bound
/// to it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum UiaRole {
    #[default]
    None,
    Text,
    Group,
    Button,
    CheckBox,
    /// One of a set. Reports `SelectionItem` rather than `Toggle`, which is the distinction a
    /// screen reader announces as "3 of 5" instead of "checked".
    RadioButton,
    Slider,
    Edit,
    ComboBox,
    List,
    /// A menu, and the container its items are announced under. Raised as opened and closed by the
    /// overlay layer.
    Menu,
    ProgressBar,
    Graph,
    /// A hover description. Raised as opened by the overlay layer, and doubling as its
    /// target's help text.
    ToolTip,
}

/// A resolved component recipe: the three rows a control can show, and its shape.
///
/// The ladder is resolved once, at construction, so a state change is an index rather than a
/// resolution. A recipe that wants its own selected or disabled row states it with [`Self::when`]
/// rather than letting a later pass patch one in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Chrome {
    rows: [RoleSet; 3],
    pub radius: Metric,
    /// The flush edge has square corners and no border.
    pub attached: Option<Edge>,
}

impl Chrome {
    /// Returns the recipe `roles` resolves to in every model state.
    #[must_use]
    pub const fn new(roles: RoleSet, radius: Metric) -> Self {
        Self {
            rows: [
                roles,
                roles.in_state(ModelState::Selected),
                roles.in_state(ModelState::Disabled),
            ],
            radius,
            attached: None,
        }
    }

    /// Returns this recipe with `state` showing `roles` rather than the derived row.
    #[must_use]
    pub const fn when(mut self, state: ModelState, roles: RoleSet) -> Self {
        self.rows[state as usize] = roles;
        self
    }

    /// Returns the roles this chrome shows in `state`.
    #[must_use]
    pub const fn in_state(self, state: ModelState) -> RoleSet {
        self.rows[state as usize]
    }
}

/// What a pointer means to a control.
///
/// The front thread moves the pixels of an interaction, so the kind is named by the recipe rather
/// than asked of the application. Two of the three carry the range the value runs over, which the
/// app thread keeps: the front thread is given the bottom, the width and the quantum instead.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Interaction {
    /// A press and a release, and nothing in between.
    Press,
    /// A value read off the pointer's position along the control's own rect.
    Slide(Range),
    /// A value turned: a single-pointer rotation about the control's centre, or a dial detent.
    Turn(Range),
}

/// A dial's full sweep, in detents, where a range names no step of its own.
const DETENTS: f64 = 64.0;

/// How far a turned control rotates end to end, in radians.
///
/// One constant for both halves of a turn: the needle's own sweep, and the rotation a contact has
/// to carry to cross it.
pub const TURN_SWEEP: f32 = core::f32::consts::TAU * 0.75;

/// The span of values a control edits, and how coarsely.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    /// Zero is continuous.
    pub step: f64,
    /// Which way the value grows. A vertical slider grows upward, which is the opposite of the
    /// coordinate it is read from.
    pub vertical: bool,
}

impl Range {
    /// `0..=1`, continuous, horizontal.
    pub const UNIT: Self = Self::new(0.0, 1.0);

    /// A closed range, continuous, horizontal.
    #[must_use]
    pub const fn new(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            step: 0.0,
            vertical: false,
        }
    }

    /// The same range in steps of `step`.
    #[must_use]
    pub const fn step(self, step: f64) -> Self {
        Self { step, ..self }
    }

    /// The same range read along the vertical axis.
    #[must_use]
    pub const fn vertical(self) -> Self {
        Self {
            vertical: true,
            ..self
        }
    }

    /// Where `value` sits in this range, as `0..=1`.
    #[must_use]
    pub fn fraction(self, value: f64) -> f32 {
        (((value - self.min) / (self.max - self.min)) as f32).clamp(0.0, 1.0)
    }

    /// The quantum a fraction snaps to, as a fraction.
    ///
    /// Where the range names no step, a full sweep is sixty-four detents, so a dial turns at a
    /// usable rate whatever it edits.
    #[must_use]
    pub fn quantum(self) -> f32 {
        let span = self.max - self.min;
        if self.step > 0.0 && span > 0.0 {
            (self.step / span) as f32
        } else {
            (1.0 / DETENTS) as f32
        }
    }
}

/// Returns the value fraction for a pointer at `along` of a control's own extent.
///
/// The coordinate grows downward and a value grows upward, so the mapping is its own inverse and
/// [`offset_of`] shares it.
#[must_use]
pub fn fraction_of(along: f32, vertical: bool) -> f32 {
    if vertical { 1.0 - along } else { along }
}

/// Returns the offset of a part sitting at `fraction` of `travel`, in DIPs.
#[must_use]
pub fn offset_of(fraction: f32, travel: f32, vertical: bool) -> f32 {
    fraction_of(fraction, vertical) * travel
}

/// A bounded scalar mapping, declared by a component and driven on the front thread.
///
/// The first three are what a decorative descendant declares. The last two are minted by the stock
/// scalar recipes, because they read the control's own solved travel, which no author knows at
/// construction.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum ScalarPart {
    /// No part: what an empty slot of a control's part array holds.
    #[default]
    None,
    Rotation {
        from: f32,
        to: f32,
    },
    TrimEnd,
    Offset {
        vertical: bool,
        from: f32,
        to: f32,
    },
    /// A thumb over the control's own solved travel.
    Thumb {
        vertical: bool,
    },
    /// A value stroke trimmed between a fixed normalized origin and the fraction.
    ///
    /// The source and the axis are read off the control's own thumb part, so the two cannot
    /// disagree about which offset the stroke follows.
    Trail {
        from: f32,
    },
}

impl ScalarPart {
    /// Returns the properties this part writes at `fraction`, and the value of each.
    ///
    /// `rest` and `travel` are the control's own solved geometry, which the parts mapping over the
    /// whole rail read and the parts stating their own endpoints ignore.
    pub(crate) fn channels(
        self,
        fraction: f32,
        rest: f32,
        travel: f32,
    ) -> [Option<(Prop, f32)>; 2] {
        let lerp = |from: f32, to: f32| from + (to - from) * fraction;
        let axis = |vertical| {
            if vertical {
                Prop::OffsetY
            } else {
                Prop::OffsetX
            }
        };
        match self {
            Self::None => [None, None],
            Self::Rotation { from, to } => [Some((Prop::RotationAngle, lerp(from, to))), None],
            Self::TrimEnd => [Some((Prop::TrimEnd, fraction)), None],
            Self::Offset { vertical, from, to } => [Some((axis(vertical), lerp(from, to))), None],
            Self::Thumb { vertical } => [
                Some((axis(vertical), rest + offset_of(fraction, travel, vertical))),
                None,
            ],
            // Driven by a compositor binding onto the thumb's offset, not by a write from here.
            Self::Trail { .. } => [None, None],
        }
    }
}

/// A readable scalar and the application epoch that owns it. Change the epoch on source
/// replacement.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ScalarValue {
    pub value: f64,
    pub epoch: u64,
}

/// What a gesture reports to the application, whatever the gesture moves.
///
/// One enum rather than a handler per phase: a gesture is a sequence with exactly one end, and
/// separate callbacks would let a caller register the moves and forget the release. A scalar's
/// payload is its value; a declared two-axis drag's is the sample it reported.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Gesturing<T> {
    /// The gesture moved. A drag's update carries the phase, the displacement **projected onto the
    /// locked axis**, and whether this sample is the one that decided that axis.
    Moved(T),
    /// The contact lifted: what it carries takes effect.
    Committed(T),
    /// The contact was taken away: nothing takes effect, and what stood before the gesture stands.
    Canceled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_variant_indexes_its_own_table() {
        for (table, index) in [
            (&BUTTON[..], DEFAULT),
            (&BUTTON[..], ACCENT),
            (&BUTTON[..], ACCENT_SUBTLE),
            (&BUTTON[..], GHOST),
            (&SURFACE[..], SURFACE_CARD),
            (&SURFACE[..], SURFACE_PANEL),
            (&SURFACE[..], SURFACE_FLYOUT),
            (&TRACK[..], TRACK_OFF),
            (&TRACK[..], TRACK_ON),
        ] {
            assert!(
                (index as usize) < table.len(),
                "a named variant must index its own table"
            );
        }
    }

    #[test]
    fn a_ghost_variant_mints_no_surface() {
        let ghost = BUTTON[GHOST as usize];
        assert!(ghost.fill.is_none() && ghost.stroke.is_none());
        // Selection is the one state that fills, and a surface shows it only once declared
        // selectable.
        for state in [ModelState::Rest, ModelState::Disabled] {
            assert!(ghost.in_state(state).fill.is_none());
        }
        assert_eq!(ghost.in_state(ModelState::Selected).fill, Some(Fill::Selected));
    }

    #[test]
    fn a_chrome_resolves_its_ladder_once_and_an_override_replaces_a_row() {
        let base = BUTTON[DEFAULT as usize];
        let chrome = Chrome::new(base, Metric::Radius);
        assert_eq!(chrome.in_state(ModelState::Rest), base);
        assert_eq!(
            chrome.in_state(ModelState::Disabled).text,
            Text::Disabled,
            "the derived row is the one the ladder resolved at construction"
        );
        let accent = BUTTON[ACCENT as usize];
        assert_eq!(
            Chrome::new(base, Metric::Radius)
                .when(ModelState::Selected, accent)
                .in_state(ModelState::Selected),
            accent
        );
    }

    #[test]
    fn a_range_without_a_step_still_turns_at_a_usable_rate() {
        assert_eq!(Range::new(-24.0, 24.0).step(4.8).quantum(), 0.1);
        assert_eq!(Range::UNIT.quantum(), 1.0 / 64.0);
        // A degenerate range answers a quantum rather than a division by zero.
        assert!(Range::new(1.0, 1.0).quantum().is_finite());
    }

    #[test]
    fn a_vertical_part_runs_against_the_coordinate_it_is_read_from() {
        assert_eq!(fraction_of(0.25, false), 0.25);
        assert_eq!(fraction_of(0.25, true), 0.75);
        assert_eq!(offset_of(0.25, 100.0, false), 25.0);
        assert_eq!(offset_of(0.25, 100.0, true), 75.0);
    }

    #[test]
    fn a_trail_writes_nothing_and_a_thumb_writes_one_axis() {
        assert_eq!(
            ScalarPart::Trail { from: 0.5 }.channels(0.25, 4.0, 100.0),
            [None, None]
        );
        assert_eq!(
            ScalarPart::Thumb { vertical: false }.channels(0.25, 4.0, 100.0),
            [Some((Prop::OffsetX, 29.0)), None]
        );
        assert_eq!(
            ScalarPart::Thumb { vertical: true }.channels(0.25, 4.0, 100.0),
            [Some((Prop::OffsetY, 79.0)), None]
        );
    }
}
