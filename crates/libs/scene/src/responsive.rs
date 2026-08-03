//! The classifying container. **App half.**
//!
//! A card arranges its parts differently at 520, 700 and 900 DIPs of *its own* width, and
//! the same card appears in a full-width row, a narrow column and a detail pane at once. A
//! window-level breakpoint cannot answer that, and threading a width down from every call
//! site is the ceremony an authoring layer exists to delete.
//!
//! What such a card actually needs is a **classification, not a measurement**: padding,
//! gap, type size, radius and control sizes resolve from a density; a flex direction, a
//! grid track list and a hidden part are style variants. Every one of them is a threshold
//! selecting a discrete variant, so the primitive is a class.
//!
//! **Nothing here mounts or unmounts.** Crossing a threshold changes styles and never
//! structure, which is what makes the mechanism safe to evaluate during a resize drag: no
//! owner is dropped, no state is disposed, and a value half-typed into a field inside the
//! narrow arrangement survives the user wobbling across the boundary.

/// How wide a container found itself. Ordered, so a rule can read "at least medium".
///
/// **The default is the *unclassified* class, and it is `Wide`.** A node outside every
/// responsive container has one, a node inside one has it until the first solve resolves
/// otherwise, and a style is lowered at it before any classification exists — so it has to
/// be the same value on both sides of the crate boundary, and the layer above states the
/// same one at the root of its scope. Any other choice makes the first solve of a container
/// that classifies *to* the default produce no transition, and a subtree keeps the styles
/// its mount lowered at a class it is not in.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WidthClass {
    Narrow,
    Medium,
    #[default]
    Wide,
}

/// The two thresholds a container classifies against: `[narrow_max, medium_max]`, in DIPs.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bounds(pub [f32; 2]);

/// How far past a threshold the width must travel before the class follows it back.
///
/// Not for correctness — the classification cannot oscillate, because a container's inline
/// size is an input its parent hands down and nothing inside it can change. It is so a
/// card's density does not strobe while someone drags a window edge across a threshold.
pub const HYSTERESIS_DIPS: f32 = 20.0;

impl Bounds {
    /// The class `width` falls in, with no history to hold it.
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

    /// The class `width` falls in, given the class it was last in.
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
            // A one-DIP wobble either side of the boundary, which is what a drag on a
            // window edge actually delivers.
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
