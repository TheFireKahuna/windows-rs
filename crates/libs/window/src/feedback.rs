use crate::bindings::*;
use windows_core::BOOL;

/// One system-drawn touch or pen feedback visual.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Feedback {
    /// The contact ring drawn under a finger.
    TouchContact,
    TouchTap,
    TouchDoubleTap,
    /// The circle that grows under a held finger, announcing a right-tap.
    TouchPressAndHold,
    TouchRightTap,
    /// The barrel-button indicator on a pen.
    PenBarrel,
    PenTap,
    PenDoubleTap,
    PenPressAndHold,
    PenRightTap,
    GesturePressAndTap,
}

impl Feedback {
    const ALL: [Self; 11] = [
        Self::TouchContact,
        Self::PenBarrel,
        Self::PenTap,
        Self::PenDoubleTap,
        Self::PenPressAndHold,
        Self::PenRightTap,
        Self::TouchTap,
        Self::TouchDoubleTap,
        Self::TouchPressAndHold,
        Self::TouchRightTap,
        Self::GesturePressAndTap,
    ];

    const fn kind(self) -> FEEDBACK_TYPE {
        match self {
            Self::TouchContact => FEEDBACK_TOUCH_CONTACTVISUALIZATION,
            Self::PenBarrel => FEEDBACK_PEN_BARRELVISUALIZATION,
            Self::PenTap => FEEDBACK_PEN_TAP,
            Self::PenDoubleTap => FEEDBACK_PEN_DOUBLETAP,
            Self::PenPressAndHold => FEEDBACK_PEN_PRESSANDHOLD,
            Self::PenRightTap => FEEDBACK_PEN_RIGHTTAP,
            Self::TouchTap => FEEDBACK_TOUCH_TAP,
            Self::TouchDoubleTap => FEEDBACK_TOUCH_DOUBLETAP,
            Self::TouchPressAndHold => FEEDBACK_TOUCH_PRESSANDHOLD,
            Self::TouchRightTap => FEEDBACK_TOUCH_RIGHTTAP,
            Self::GesturePressAndTap => FEEDBACK_GESTURE_PRESSANDTAP,
        }
    }

    /// The system numbers these from one.
    const fn bit(self) -> u16 {
        1 << (self.kind() as u16 - 1)
    }
}

/// Which system-drawn visuals a window keeps.
///
/// The default keeps all of them: a suppressed visual with no replacement drawn is a touch
/// that gives no feedback at all.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FeedbackPolicy {
    suppressed: u16,
}

impl FeedbackPolicy {
    /// Keep everything the system draws.
    pub const SYSTEM: Self = Self { suppressed: 0 };

    /// Suppress one kind, because the application draws its own.
    #[must_use]
    pub const fn without(self, feedback: Feedback) -> Self {
        Self {
            suppressed: self.suppressed | feedback.bit(),
        }
    }

    /// Whether `feedback` is one this window suppresses.
    #[must_use]
    pub const fn suppresses(self, feedback: Feedback) -> bool {
        self.suppressed & feedback.bit() != 0
    }

    pub(crate) fn apply(self, hwnd: HWND) {
        if self.suppressed == 0 {
            return;
        }
        let off: BOOL = false.into();
        for feedback in Feedback::ALL {
            if !self.suppresses(feedback) {
                continue;
            }
            // Only the suppressed kinds are written. Writing `TRUE` for the rest would pin
            // them against a future system default.
            // SAFETY: `hwnd` is live; the configuration is a stack local of the stated size.
            unsafe {
                _ = SetWindowFeedbackSetting(
                    hwnd,
                    feedback.kind(),
                    0,
                    size_of::<BOOL>() as u32,
                    (&raw const off).cast(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_its_own_bit() {
        let mut seen = 0u16;
        for feedback in Feedback::ALL {
            assert_eq!(
                seen & feedback.bit(),
                0,
                "{feedback:?} aliases another kind"
            );
            seen |= feedback.bit();
        }
        assert_eq!(seen.count_ones(), Feedback::ALL.len() as u32);
    }

    #[test]
    fn the_default_keeps_everything() {
        let policy = FeedbackPolicy::default();
        assert_eq!(policy, FeedbackPolicy::SYSTEM);
        assert!(Feedback::ALL.iter().all(|f| !policy.suppresses(*f)));

        let policy = policy.without(Feedback::TouchContact);
        assert!(policy.suppresses(Feedback::TouchContact));
        assert!(!policy.suppresses(Feedback::TouchTap));
    }
}
