//! The small shared vocabulary the arena and the widgets both name.

use crate::role::{Fill, Metric, Stroke, Text};

/// How a channel moves when its value changes.
///
/// Per channel and declared by the seed, so two call sites cannot disagree about the same
/// control: a meter level carries momentum and springs, a slider thumb the application
/// writes must be where it was put.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Motion {
    #[default]
    Snap,
    Chrome,
}

/// Whether a node has interaction chrome, and which wash it fades in.
///
/// **Not automatic.** Text, captions, meters, paths and info rows are `None`, which is most
/// of a screen, and they pay nothing. Only a control that can be hovered mints the extra
/// sprite.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum StatePolicy {
    #[default]
    None,
    Wash {
        hover: Wash,
        press: Wash,
    },
}

/// Which derived wash a state fades in.
///
/// A wash, not a different base colour — which is both what the palette already derives
/// (`ink` / `accent_wash`) and the only colour transition this architecture can express: a
/// sprite's colour is an FP16 surface cell, a composition colour brush is 8-bit, and there
/// is no brush the compositor interpolates between two FP16 sources. So a state change is a
/// crossfade, and the thing being faded is a wash.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Wash {
    Ink,
    Accent,
}

/// What a widget names instead of writing a `UiaDecl`.
///
/// Synthesised into one by the lowering, from this plus the slot's own text and the channel
/// bound to it. A hand-written declaration per widget is a chance per widget to disagree
/// with a promise that accessibility derives.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum UiaRole {
    #[default]
    None,
    Text,
    Group,
    Button,
    CheckBox,
    Slider,
    Edit,
    ComboBox,
    List,
    ProgressBar,
    Graph,
}

/// A widget's colour triple, as one row of a const table.
///
/// A variant is a row and never a function. A table does not accrete behaviour: a fifth
/// variant is visibly a fifth row, where a fifth function is not.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RoleSet {
    pub fill: Option<Fill>,
    pub text: Text,
    pub stroke: Option<Stroke>,
}

/// Which interaction state a control's roles are resolved in.
///
/// Hover and press are **not** here as colours. They are the wash's opacity. What is here
/// is the model state that swaps a base role: a selected row and a disabled control.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ModelState {
    #[default]
    Rest,
    Selected,
    Disabled,
}

impl RoleSet {
    /// The same roles, resolved in `state`. One function, not one per widget.
    #[must_use]
    pub const fn in_state(self, state: ModelState) -> Self {
        match state {
            ModelState::Rest => self,
            ModelState::Selected => Self {
                fill: Some(Fill::Selected),
                ..self
            },
            ModelState::Disabled => Self {
                text: Text::Disabled,
                stroke: None,
                ..self
            },
        }
    }
}

/// A widget's own surface: which const table it reads, which row of it, and how round it is.
///
/// The slot carries the **index**, not the sprites, so `.accent()` rewrites one byte and the
/// mount decides what that costs. A variant that drops the stroke therefore mints one sprite
/// fewer rather than minting an invisible one, and the number of visuals on a screen follows
/// from the table instead of from the order the modifiers ran in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Chrome {
    pub roles: &'static [RoleSet],
    pub variant: u8,
    pub radius: Metric,
}

impl Chrome {
    /// The row this chrome selects.
    ///
    /// A variant index is minted by a method on the widget that owns the table, so an
    /// out-of-range one is a defect rather than a case: it is asserted where it can be seen
    /// and clamped in release, because rendering as the last variant beats not rendering.
    #[must_use]
    pub fn roles(self) -> RoleSet {
        debug_assert!(
            (self.variant as usize) < self.roles.len(),
            "variant {} is past the end of this widget's own table",
            self.variant
        );
        let at = (self.variant as usize).min(self.roles.len().saturating_sub(1));
        self.roles[at]
    }
}

// ── where a moving part sits ─────────────────────────────────────────────────────
//
// The mount binds these and the router retargets them, so they live in one place. Two copies
// of this arithmetic would drift the first time a vertical control was added, and the symptom
// would be a thumb that runs the wrong way rather than a compile error.

/// How far a turned control rotates end to end, in radians.
pub const TURN_SWEEP: f32 = core::f32::consts::TAU * 0.75;

/// DIPs of vertical drag for a full sweep. The knob's own sensitivity, so it is a number
/// here rather than a [`Metric`] in the theme — see the scrollbar's dimensions for the same
/// call.
pub const TURN_SPAN: f32 = 200.0;

/// A dial's full sweep, in detents, where the range does not name its own step.
const TURN_DETENTS: f64 = 64.0;

/// Where a part sitting at `fraction` of its travel goes, in the property's own unit.
///
/// Downward is increasing and a value grows upward, so a vertical control at its maximum
/// sits at the **top** of its travel.
#[must_use]
pub fn offset_of(fraction: f32, travel: f32, vertical: bool) -> f32 {
    fraction_of(fraction, vertical) * travel
}

/// The same for a turned part: where `fraction` of a value sits on its sweep, in radians.
///
/// Beside `offset_of` and not beside the knob, because the mount writes this angle and the
/// router retargets it. A second copy would drift, and the symptom would be a knob that
/// jumps the moment it is let go rather than a compile error.
#[must_use]
pub fn angle_of(fraction: f32) -> f32 {
    fraction.clamp(0.0, 1.0) * TURN_SWEEP
}

/// The same mapping, read the other way: where a pointer at `along` of a control's own
/// extent puts its value. Its own inverse, which is why one function serves both.
#[must_use]
pub fn fraction_of(along: f32, vertical: bool) -> f32 {
    let along = along.clamp(0.0, 1.0);
    if vertical { 1.0 - along } else { along }
}

/// What a dial detent moves a value by, as a fraction of its range.
///
/// A **delta**, which is the whole of what a dial reports: treating a detent count as an
/// absolute position sends one click to an end stop.
#[must_use]
pub fn detent_delta(range: Range, steps: f64) -> f32 {
    let span = range.max - range.min;
    if span.abs() < f64::EPSILON {
        return 0.0;
    }
    let per = if range.step > 0.0 {
        range.step
    } else {
        span / TURN_DETENTS
    };
    (steps * per / span) as f32
}

/// What a pointer means to a control, front-side.
///
/// The router owns the pixels of an interaction, so it needs to know what kind of thing it
/// is moving without asking the application. Four cases, and the fourth is the one that
/// carries a number.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Interaction {
    /// A press and a release, and nothing in between.
    Press,
    /// A value read off the pointer's position along the control's own rect.
    Slide(Range),
    /// A value turned: a drag's cross-axis displacement, or a dial detent.
    Turn(Range),
}

/// The span of values a control edits, and how coarsely.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    /// Zero is continuous.
    pub step: f64,
    /// Which way the value grows. A vertical slider grows upward, which is the opposite of
    /// the coordinate it is read from.
    pub vertical: bool,
}

impl Range {
    /// `0..=1`, continuous, horizontal.
    pub const UNIT: Self = Self {
        min: 0.0,
        max: 1.0,
        step: 0.0,
        vertical: false,
    };

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
        let span = self.max - self.min;
        if span.abs() < f64::EPSILON {
            return 0.0;
        }
        (((value - self.min) / span).clamp(0.0, 1.0)) as f32
    }

    /// The value at `fraction` of the way along, snapped to the step.
    #[must_use]
    pub fn at(self, fraction: f32) -> f64 {
        let raw = self.min + f64::from(fraction.clamp(0.0, 1.0)) * (self.max - self.min);
        if self.step <= 0.0 {
            return raw;
        }
        // Snapped from `min` rather than from zero, so a range that does not start on a
        // multiple of the step still lands on values the caller named.
        self.min + ((raw - self.min) / self.step).round() * self.step
    }
}
