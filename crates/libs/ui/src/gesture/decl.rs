//! What a target declares about the gestures it accepts.
//!
//! Declarations are **front-resident**: no call is made to the application thread to decide
//! whether a gesture applies, because that decision has to be made between a contact
//! arriving and its pixels moving. A control that declares nothing routes press, release and
//! drag through the ordinary paths and costs no recogniser at all.

use crate::bindings::GestureSettings;
use core::time::Duration;
use windows_scene::Point;

/// Everything one target says about how it may be touched.
///
/// `Copy`, and every field a number or a flag: a declaration travels with the widget that
/// made it and is looked up per contact, so it must not own anything.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GestureDecl {
    /// This node's recogniser configuration.
    pub settings: GestureSettings,
    pub hold: HoldTuning,
    /// A knob. Present means single-pointer rotation around a declared centre.
    pub pivot: Option<PivotDecl>,
    pub touch: TouchTargetDecl,
    /// Hand touch to an `InteractionTracker` instead of to a recogniser. A scroll surface
    /// wants this; a knob must not have it.
    pub redirect: bool,
    /// A drag whose meaning depends on its direction.
    pub drag: Option<DragDecl>,
    /// Rotary interest — resolution and step for `RadialController`.
    pub rotary: Option<RotaryDecl>,
}

impl Default for GestureDecl {
    /// A plain tappable target: tap, right-tap, and hold — which is what gives touch users
    /// the context menu mouse users get from the secondary button.
    fn default() -> Self {
        Self {
            settings: GestureSettings::Tap
                | GestureSettings::RightTap
                | GestureSettings::Hold
                | GestureSettings::HoldWithMouse,
            hold: HoldTuning::default(),
            pivot: None,
            touch: TouchTargetDecl::Inflate,
            redirect: false,
            drag: None,
            rotary: None,
        }
    }
}

impl GestureDecl {
    /// A target that only reports taps.
    #[must_use]
    pub fn tap() -> Self {
        Self {
            settings: GestureSettings::Tap,
            ..Self::default()
        }
    }

    /// A one-dimensional value edit — a slider. Translation on one axis, no inertia, because
    /// a value that keeps moving after the finger lifts is a value the user did not choose.
    #[must_use]
    pub fn slider(vertical: bool) -> Self {
        Self {
            settings: if vertical {
                GestureSettings::ManipulationTranslateY
            } else {
                GestureSettings::ManipulationTranslateX
            },
            touch: TouchTargetDecl::None,
            ..Self::default()
        }
    }

    /// A knob: single-pointer rotation about `center`, `radius` DIPs out.
    ///
    /// Rotation needs `ManipulationRotate` **and** a non-zero pivot radius, and the pivot
    /// applies only to single-pointer input. Both are the platform's rules, not ours.
    #[must_use]
    pub fn knob(center: Point, radius: f32) -> Self {
        Self {
            settings: GestureSettings::ManipulationRotate
                | GestureSettings::ManipulationTranslateX
                | GestureSettings::ManipulationTranslateY,
            pivot: Some(PivotDecl { radius, center }),
            touch: TouchTargetDecl::None,
            ..Self::default()
        }
    }

    /// Adds rotary interest, so the dial drives the same value path a drag does.
    #[must_use]
    pub const fn with_rotary(mut self, rotary: RotaryDecl) -> Self {
        self.rotary = Some(rotary);
        self
    }

    /// Adds a two-axis drag policy.
    #[must_use]
    pub const fn with_drag(mut self, drag: DragDecl) -> Self {
        self.drag = Some(drag);
        self
    }

    /// Whether this declaration asks for any manipulation at all, which is what decides
    /// whether a contact needs a recogniser bound to it.
    #[must_use]
    pub fn manipulates(&self) -> bool {
        const ANY: u32 = GestureSettings::ManipulationTranslateX.0
            | GestureSettings::ManipulationTranslateY.0
            | GestureSettings::ManipulationRotate.0
            | GestureSettings::ManipulationScale.0;
        self.settings.0 & ANY != 0
    }
}

/// How long, how far, and with how many contacts a hold is a hold.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HoldTuning {
    pub start_delay: Duration,
    /// DIPs of slack before a hold becomes a drag.
    pub radius: f32,
    pub min_contacts: u32,
    pub max_contacts: u32,
}

impl Default for HoldTuning {
    /// The platform's own feel: a hold is most of a second, and a contact that wanders more
    /// than a finger's width was a drag.
    fn default() -> Self {
        Self {
            start_delay: Duration::from_millis(500),
            radius: 10.0,
            min_contacts: 1,
            max_contacts: 1,
        }
    }
}

/// Single-pointer rotation about a declared centre — which is what a knob is.
///
/// **Both fields must be restated on every `ManipulationUpdated`**, not set once at down:
/// the platform documents them as values to update regularly during the interaction, and a
/// stale centre makes a knob drift under a finger that has not moved.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PivotDecl {
    pub radius: f32,
    pub center: Point,
}

/// How much a finger's slack is allowed to grow this target.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum TouchTargetDecl {
    /// Grow to the platform's ~9 mm guidance and no further.
    #[default]
    Inflate,
    /// Never grow. A dense meter or a curve node field, where inflation would make adjacent
    /// targets indistinguishable.
    None,
    /// Grow by exactly this many DIPs on each side.
    Explicit(f32),
}

impl TouchTargetDecl {
    /// What the hit array should be told, given the node's own laid-out size.
    #[must_use]
    pub fn inflation(self) -> Option<f32> {
        match self {
            Self::Inflate => None,
            Self::None => Some(0.0),
            Self::Explicit(dips) => Some(dips),
        }
    }
}

/// Which axes a two-axis drag may mean something on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DragAxes {
    Vertical,
    Horizontal,
    #[default]
    Both,
}

/// Whether the first axis past the threshold owns the drag.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum AxisLock {
    /// The first axis past the threshold owns the drag for its whole duration. **A drag that
    /// changes meaning mid-flight is a drag the user cannot aim.**
    #[default]
    FirstPast,
    /// No lock: both axes stay live. For a free pan, never for a drag whose axis selects a
    /// meaning.
    None,
}

/// When a drag's value takes effect.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Commit {
    /// The value lands on release, and a canceled contact restores what it was.
    #[default]
    OnRelease,
    /// The value follows the contact.
    Live,
}

/// A drag whose meaning depends on its direction.
///
/// Reordering a processor chain and rescoping a processor to different channels are the same
/// physical gesture on the same object, separated by axis. One policy, or every consumer
/// invents its own and they disagree.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DragDecl {
    pub axes: DragAxes,
    /// DIPs before any meaning is assigned. **Nothing is decided below it**, so a nudge
    /// while clicking is a click.
    pub threshold: f32,
    pub lock: AxisLock,
    pub commit: Commit,
}

impl Default for DragDecl {
    fn default() -> Self {
        Self {
            axes: DragAxes::Both,
            threshold: 6.0,
            lock: AxisLock::FirstPast,
            commit: Commit::OnRelease,
        }
    }
}

impl DragDecl {
    /// Vertical is order, horizontal is scope: the chain's own drag.
    #[must_use]
    pub fn reorder() -> Self {
        Self::default()
    }
}

/// Rotary interest — what the dial does to this target.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RotaryDecl {
    /// Degrees of dial rotation per detent. The dial's haptics are driven from this, so it
    /// is the step the user *feels* as well as the one they get.
    pub resolution_degrees: f64,
    /// How much the value moves per detent, in the target's own units.
    pub step: f64,
    /// Whether the dial should click at each detent.
    pub haptics: bool,
}

impl Default for RotaryDecl {
    fn default() -> Self {
        Self {
            resolution_degrees: 10.0,
            step: 1.0,
            haptics: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_declaration_gives_touch_the_context_menu_mouse_gets_free() {
        let decl = GestureDecl::default();
        assert!(decl.settings.contains(GestureSettings::RightTap));
        assert!(decl.settings.contains(GestureSettings::Hold));
        // Press-and-hold raises RightTapped, so context-menu routing gains touch support
        // with no extra design.
        assert!(!decl.manipulates());
    }

    #[test]
    fn a_slider_manipulates_on_one_axis_and_never_inflates() {
        let decl = GestureDecl::slider(false);
        assert!(decl.manipulates());
        assert!(
            decl.settings
                .contains(GestureSettings::ManipulationTranslateX)
        );
        assert!(
            !decl
                .settings
                .contains(GestureSettings::ManipulationTranslateY)
        );
        assert_eq!(decl.touch.inflation(), Some(0.0));
    }

    #[test]
    fn a_knob_carries_a_non_zero_pivot_because_rotation_needs_one() {
        let decl = GestureDecl::knob(Point { x: 20.0, y: 20.0 }, 20.0);
        assert!(decl.settings.contains(GestureSettings::ManipulationRotate));
        let pivot = decl.pivot.expect("a knob declares a pivot");
        assert!(
            pivot.radius > 0.0,
            "rotation is not supported at radius zero"
        );
    }

    #[test]
    fn inflation_is_the_defaults_absence_rather_than_a_number() {
        assert_eq!(TouchTargetDecl::Inflate.inflation(), None);
        assert_eq!(TouchTargetDecl::None.inflation(), Some(0.0));
        assert_eq!(TouchTargetDecl::Explicit(4.5).inflation(), Some(4.5));
    }

    #[test]
    fn the_default_hold_is_the_platforms_own_feel() {
        // Most of a second, and a contact that wanders more than a finger's width was a drag.
        let hold = HoldTuning::default();
        assert_eq!(hold.start_delay, Duration::from_millis(500));
        assert!((hold.radius - 10.0).abs() < 1e-6);
        assert_eq!((hold.min_contacts, hold.max_contacts), (1, 1));
    }
}
