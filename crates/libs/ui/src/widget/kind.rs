//! The vocabulary the arena and the widgets both name: motion, state policy, colour rows,
//! automation roles, value ranges, and the arithmetic that places a moving part.

use crate::role::{Fill, Metric, Stroke, Text};

/// How a channel moves when its value changes.
///
/// Declared per channel by the seed, so two call sites cannot disagree about one control: a
/// meter level springs, and a slider thumb the application writes lands where it was put.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Motion {
    /// The channel lands on the new value with no animation.
    #[default]
    Snap,
    /// The channel springs to the new value.
    Chrome,
}

/// Whether a node has interaction chrome, and which wash it fades in.
///
/// Only a control that can be hovered mints the extra sprite. Text, captions, meters, paths
/// and info rows are `None` and mint none.
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
/// A state change is a crossfade of a wash over the base colour rather than an interpolation
/// towards a second base colour: a sprite's colour is an FP16 surface cell, a composition
/// colour brush is 8-bit, and no brush interpolates between two FP16 sources. The palette
/// derives both washes, through [`ink`](crate::role::ink) and
/// [`accent_wash`](crate::role::accent_wash).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Wash {
    /// The scope's foreground, at the state's opacity.
    Ink,
    /// The scope's accent fill, at the state's opacity.
    Accent,
}

/// What a widget names instead of writing a `UiaDecl`.
///
/// The lowering synthesises the declaration from this, the slot's own text, and the channel
/// bound to it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum UiaRole {
    #[default]
    None,
    Text,
    Group,
    Button,
    CheckBox,
    /// One of a set. Reports `SelectionItem` rather than `Toggle`, which is the distinction
    /// a screen reader announces as "3 of 5" instead of "checked".
    RadioButton,
    Slider,
    Edit,
    ComboBox,
    List,
    /// A menu, and the container its items are announced under. Raised as `MenuOpened` and
    /// `MenuClosed` by the overlay layer.
    Menu,
    ProgressBar,
    Graph,
}

/// A widget's colour triple: one row of a `const` table.
///
/// A variant is a row rather than a function, so the variants a widget has are the length of
/// its table.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RoleSet {
    pub fill: Option<Fill>,
    pub text: Text,
    pub stroke: Option<Stroke>,
}

/// Which model state a control's roles are resolved in.
///
/// Hover and press are not here: they are the wash's opacity. This is the state that swaps a
/// base role — a selected row, a disabled control.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ModelState {
    #[default]
    Rest,
    Selected,
    Disabled,
}

impl RoleSet {
    /// Returns these roles resolved in `state`.
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

/// A widget's own surface: which `const` table it reads, which row of it, and how round it is.
///
/// The slot carries the row index rather than the sprites, so a variant modifier rewrites one
/// byte and the mount reads the sprite count off the row. A row with no stroke mints one
/// sprite fewer rather than an invisible one.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Chrome {
    pub roles: &'static [RoleSet],
    pub variant: u8,
    pub radius: Metric,
}

impl Chrome {
    /// Returns the row this chrome selects.
    ///
    /// A variant index is minted by a method on the widget that owns the table, so an
    /// out-of-range index is a defect rather than a case. It clamps to the last row in
    /// release, which renders the control as some other variant.
    ///
    /// # Panics
    ///
    /// In a debug build, if `variant` is past the end of `roles`.
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
// The mount binds these properties and the router retargets them, so both sides reach the
// same arithmetic from here.

/// How far a turned control rotates end to end, in radians.
pub const TURN_SWEEP: f32 = core::f32::consts::TAU * 0.75;

/// DIPs of vertical drag for a full sweep. A knob's sensitivity is an interaction constant
/// rather than a themed [`Metric`].
pub const TURN_SPAN: f32 = 200.0;

/// A dial's full sweep, in detents, where the range does not name its own step.
const TURN_DETENTS: f64 = 64.0;

/// Returns the offset of a part sitting at `fraction` of `travel`, in DIPs.
///
/// The coordinate grows downward and a value grows upward, so a vertical control at its
/// maximum sits at the top of its travel.
#[must_use]
pub fn offset_of(fraction: f32, travel: f32, vertical: bool) -> f32 {
    fraction_of(fraction, vertical) * travel
}

/// Returns the angle a turned part sitting at `fraction` of [`TURN_SWEEP`] takes, in radians.
///
/// The mount writes this angle and the router retargets it, so both reach the same function
/// and a committed value cannot land the part where a live drag did not.
#[must_use]
pub fn angle_of(fraction: f32) -> f32 {
    fraction.clamp(0.0, 1.0) * TURN_SWEEP
}

/// Returns the value fraction for a pointer at `along` of a control's own extent, clamped to
/// `0..=1`. The mapping is its own inverse, so [`offset_of`] shares it.
#[must_use]
pub fn fraction_of(along: f32, vertical: bool) -> f32 {
    let along = along.clamp(0.0, 1.0);
    if vertical { 1.0 - along } else { along }
}

/// Returns what `steps` detents move a value by, as a fraction of `range`.
///
/// The result is a delta to add to the fraction a control already stands at. Where `range`
/// names no step, a full sweep is 64 detents.
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
/// The front thread moves the pixels of an interaction, so the kind is named here rather than
/// asked of the application. Two of the three carry the range the value runs over.
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
