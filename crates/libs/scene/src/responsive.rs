//! The classifying container. **App half.**
//!
//! A container classifies its own inline width into a [`WidthClass`], so one card can be
//! arranged differently at 520, 700 and 900 DIPs, and can appear in a full-width row, a
//! narrow column and a detail pane at the same time. A window-level breakpoint cannot
//! express that.
//!
//! The output is a classification and not a measurement: padding, gap, type size, radius and
//! control sizes resolve from a density, and a flex direction, a grid track list or a hidden
//! part are style variants. Each is a threshold selecting a discrete variant.
//!
//! Crossing a threshold changes styles and never structure. Nothing here mounts or unmounts,
//! so a resize drag drops no owner, disposes no state, and leaves a value half-typed into a
//! field standing while the width wobbles across a boundary.

/// How wide a container classified itself. Ordered, so a rule can read "at least medium".
///
/// [`WidthClass::Wide`] is the unclassified class: a node outside every responsive container
/// carries it, a node inside one carries it until the first solve resolves otherwise, and a
/// style lowered before any classification exists is lowered at it. The layer above must
/// state the same class at the root of its scope, because a subtree that mounts at one
/// default and solves to another produces no transition and keeps the styles its mount
/// lowered.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WidthClass {
    Narrow,
    Medium,
    #[default]
    Wide,
}

impl WidthClass {
    /// Every class, narrowest first.
    pub const ALL: [Self; 3] = [Self::Narrow, Self::Medium, Self::Wide];

    /// Returns every class narrower than this one, narrowest first.
    #[must_use]
    pub fn below(self) -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().filter(move |class| *class < self)
    }
}

/// The two thresholds a container classifies against: `[narrow_max, medium_max]`, in DIPs.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bounds(pub [f32; 2]);

/// How far past a threshold the width must travel before the class follows it.
///
/// The band keeps a card's density from strobing while a window edge is dragged across a
/// threshold. The classification cannot oscillate on its own: a container's inline size is
/// an input its parent hands down, and nothing inside the container changes it.
pub const HYSTERESIS_DIPS: f32 = 20.0;

impl Bounds {
    /// Returns the class `width` falls in, with no previous class to hold it. A non-finite
    /// `width` classifies as [`WidthClass::Narrow`].
    #[must_use]
    pub fn classify(self, width: f32) -> WidthClass {
        let [narrow, medium] = self.0;
        if !width.is_finite() || width <= narrow {
            WidthClass::Narrow
        } else if width <= medium {
            WidthClass::Medium
        } else {
            WidthClass::Wide
        }
    }

    /// Returns the class `width` falls in, given the class it was last in.
    ///
    /// The band is applied in the direction of travel: widening has to clear the threshold
    /// by the band before the class rises, and narrowing has to fall below it by the band
    /// before it drops. So a width parked on a boundary keeps whichever class it arrived
    /// with, and a sweep across and back changes class exactly once in each direction.
    #[must_use]
    pub fn reclassify(self, width: f32, previous: WidthClass) -> WidthClass {
        let fresh = self.classify(width);
        if fresh == previous {
            return previous;
        }
        let [narrow, medium] = self.0;
        let threshold = if fresh > previous {
            // Rising: the threshold just cleared is the top of the class being left.
            match previous {
                WidthClass::Narrow => narrow,
                _ => medium,
            }
        } else {
            // Falling: the threshold just fallen below is the top of the class being
            // entered.
            match fresh {
                WidthClass::Narrow => narrow,
                _ => medium,
            }
        };
        if (width - threshold).abs() < HYSTERESIS_DIPS {
            previous
        } else {
            fresh
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Bounds = Bounds([600.0, 1000.0]);

    #[test]
    fn a_cold_classification_reads_the_thresholds() {
        assert_eq!(BOUNDS.classify(480.0), WidthClass::Narrow);
        assert_eq!(BOUNDS.classify(800.0), WidthClass::Medium);
        assert_eq!(BOUNDS.classify(1400.0), WidthClass::Wide);
        assert_eq!(BOUNDS.classify(f32::NAN), WidthClass::Narrow);
    }

    #[test]
    fn a_sweep_across_a_threshold_and_back_changes_class_once_each_way() {
        // One DIP at a time, as a live resize drag delivers it.
        let mut class = WidthClass::Narrow;
        let mut changes = 0;
        for w in 500..=700 {
            let next = BOUNDS.reclassify(w as f32, class);
            if next != class {
                changes += 1;
                class = next;
            }
        }
        assert_eq!(changes, 1, "widening should cross exactly once");
        assert_eq!(class, WidthClass::Medium);

        for w in (500..=700).rev() {
            let next = BOUNDS.reclassify(w as f32, class);
            if next != class {
                changes += 1;
                class = next;
            }
        }
        assert_eq!(changes, 2, "narrowing should cross exactly once");
        assert_eq!(class, WidthClass::Narrow);
    }

    #[test]
    fn a_width_parked_on_a_threshold_does_not_strobe() {
        let mut class = WidthClass::Narrow;
        for step in 0..64 {
            // A one-DIP wobble either side of the boundary, as a drag on a window edge
            // delivers it.
            let width = 600.0 + if step % 2 == 0 { 1.0 } else { -1.0 };
            let next = BOUNDS.reclassify(width, class);
            assert_eq!(next, class, "wobbling at the boundary changed the class");
            class = next;
        }
    }

    #[test]
    fn the_band_is_left_behind_once_the_width_clears_it() {
        let class = BOUNDS.reclassify(600.0 + HYSTERESIS_DIPS + 1.0, WidthClass::Narrow);
        assert_eq!(class, WidthClass::Medium);
        let back = BOUNDS.reclassify(600.0 - HYSTERESIS_DIPS - 1.0, WidthClass::Medium);
        assert_eq!(back, WidthClass::Narrow);
    }
}
