//! The classifying container.
//!
//! A container classifies its own inline width into a [`WidthClass`], so one card is arranged
//! differently at 520, 700 and 900 DIPs, and appears in a full-width row, a narrow column and
//! a detail pane at the same time. A window-level breakpoint cannot express that.
//!
//! The output is a classification and not a measurement: padding, gap, type size, radius and
//! control sizes resolve from a density, and a track list or a hidden part is a field that
//! resolves against the class. Crossing a threshold changes values and never structure, so a
//! resize drag drops no owner and leaves a half-typed field standing.

/// How wide a container classified itself. Ordered, so a rule reads "at least medium".
///
/// [`WidthClass::Wide`] is the unclassified class: a node outside every responsive container
/// carries it, and a node inside one carries it until the first solve resolves otherwise.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WidthClass {
    Narrow,
    Medium,
    #[default]
    Wide,
}

impl WidthClass {
    /// Every class, narrowest first. The order the host's metric table is indexed in.
    pub const ALL: [Self; 3] = [Self::Narrow, Self::Medium, Self::Wide];

    /// Returns every class narrower than this one, narrowest first.
    pub fn below(self) -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().filter(move |class| *class < self)
    }

    /// Packs into the two class bits of a node's flag word.
    ///
    /// [`WidthClass::Wide`] packs to zero, so a minted node whose flags are zero reads as the
    /// unclassified class rather than as the narrowest one.
    #[must_use]
    pub const fn bits(self) -> u32 {
        match self {
            Self::Wide => 0,
            Self::Narrow => 1,
            Self::Medium => 2,
        }
    }

    /// Returns the class `bits` packs, which [`WidthClass::bits`] produced.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        match bits {
            1 => Self::Narrow,
            2 => Self::Medium,
            _ => Self::Wide,
        }
    }
}

/// How far past a threshold the width must travel before the class follows it.
///
/// The band keeps a card's density from strobing while a window edge is dragged across a
/// threshold. The classification cannot oscillate on its own: a container's inline size is an
/// input its parent hands down, and nothing inside the container changes it.
pub const HYSTERESIS_DIPS: f32 = 20.0;

/// The two thresholds a container classifies against: `[narrow_max, medium_max]`, in DIPs.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bounds(pub [f32; 2]);

impl Bounds {
    /// Returns the class `width` falls in, with no previous class to hold it.
    ///
    /// A non-finite `width` classifies as [`WidthClass::Narrow`].
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
    /// The band applies in the direction of travel: widening clears the threshold by the band
    /// before the class rises, and narrowing falls below it by the band before it drops. So a
    /// width parked on a boundary keeps whichever class it arrived with, and a sweep across
    /// and back changes class exactly once in each direction.
    ///
    /// The band applies to both thresholds rather than to the nearest one, because a resize
    /// can skip a whole class and holding the previous class near the destination's boundary
    /// would retain one whose own boundary was crossed hundreds of DIPs ago.
    #[must_use]
    pub fn reclassify(self, width: f32, previous: WidthClass) -> WidthClass {
        let fresh = self.classify(width);
        if fresh == previous {
            return previous;
        }
        let [narrow, medium] = self.0;
        if fresh > previous {
            if width >= medium + HYSTERESIS_DIPS {
                WidthClass::Wide
            } else if width >= narrow + HYSTERESIS_DIPS {
                WidthClass::Medium
            } else {
                previous
            }
        } else if !width.is_finite() || width <= narrow - HYSTERESIS_DIPS {
            WidthClass::Narrow
        } else if width <= medium - HYSTERESIS_DIPS {
            WidthClass::Medium
        } else {
            previous
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Bounds = Bounds([600.0, 1000.0]);

    #[test]
    fn jumps_hold_only_the_class_adjacent_to_a_boundary() {
        for width in [590.0, 600.0, 610.0] {
            assert_eq!(
                BOUNDS.reclassify(width, WidthClass::Wide),
                WidthClass::Medium
            );
        }
        for width in [990.0, 1000.0, 1010.0] {
            assert_eq!(
                BOUNDS.reclassify(width, WidthClass::Narrow),
                WidthClass::Medium
            );
        }
        assert_eq!(
            BOUNDS.reclassify(580.0, WidthClass::Wide),
            WidthClass::Narrow
        );
        assert_eq!(
            BOUNDS.reclassify(1020.0, WidthClass::Narrow),
            WidthClass::Wide
        );
    }

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

    #[test]
    fn the_unclassified_class_packs_to_a_zero_flag_word() {
        assert_eq!(WidthClass::from_bits(0), WidthClass::default());
        for class in WidthClass::ALL {
            assert_eq!(WidthClass::from_bits(class.bits()), class);
            assert!(class.bits() < 4, "a class packs into two bits");
        }
    }
}
