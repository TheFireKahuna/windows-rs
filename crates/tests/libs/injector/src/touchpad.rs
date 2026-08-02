//! The precision-touchpad stream.
//!
//! **Its coordinates are fractions of the pad, not window DIPs**, and that difference is
//! the device rather than the API. A touchpad is indirect: it has no screen position, and
//! the recogniser that reads it — `PhysicalGestureRecognizer` — reports in units relative
//! to the device, which is exactly what makes a touchpad gesture feel right. A harness that
//! let a caller aim a touchpad contact at a control would be inventing a mapping the system
//! does not have.
//!
//! Two halves, and they arrive by different routes:
//!
//! * **Contacts** are injected like any other synthetic pointer, but in `ptHimetricLocation`
//!   rather than `ptPixelLocation`, because a touchpad device must be created with a
//!   physical size and `SDCO_PHYSICAL_SIZE` moves the sample into device space. This is why
//!   `CreateSyntheticPointerDevice` v1 cannot create a touchpad at all — it has no options
//!   parameter and therefore no way to state a size.
//! * **Global gestures** — the three-, four- and five-finger taps and presses, and the two
//!   inertia messages — are not contacts. They arrive through `InjectTouchpadAction`, and
//!   only on a device created `SDCO_TOUCHPAD_GESTURE_ONLY`, which this stream is. On a pad
//!   that is not gesture-only, the system produces them from physical input instead and
//!   the injected action does nothing.
//!
//! Gesture-only also settles a second thing: injected contacts on this stream can never be
//! recognised as mouse motion or clicks, so a run that drives the touchpad cannot silently
//! be measuring the mouse path.
//!
//! **Touchpad input is never recognised as Tap or Hold**, by the platform's design, and no
//! amount of injection changes that. A test that asserts a tap here is asserting something
//! the device class does not produce.

use crate::bindings::*;
use crate::device::Device;
use crate::{Error, Injector, Rate, Result};

/// The pad this stream declares itself to be, in millimetres.
///
/// Roughly a 13-inch laptop's. It matters because the recogniser reports in units relative
/// to the device, so the same fraction of a larger pad is a longer pan — which is the
/// property that makes touchpad gestures scale with the hardware rather than with the
/// screen.
pub const PAD_MM: (f32, f32) = (105.0, 65.0);

/// One of the global touchpad gestures, or one of the two inertia signals.
///
/// The inertia pair is the only way to observe `WM_STOPINERTIA` and `WM_ENDINERTIA` at the
/// platform floor: both are message *numbers*, the floor's SDK redacts them, and a number
/// cannot be resolved by name the way an export can. Each of these produces exactly one of
/// them, to the window that last reported content inertia.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TouchpadAction {
    /// Three-finger tap.
    ThreeFingerTap,
    /// Three-finger button press.
    ThreeFingerPress,
    /// Three-finger button release.
    ThreeFingerRelease,
    /// Four-finger tap.
    FourFingerTap,
    /// Four-finger button press.
    FourFingerPress,
    /// Four-finger button release.
    FourFingerRelease,
    /// Five-finger tap.
    FiveFingerTap,
    /// Five-finger button press.
    FiveFingerPress,
    /// Five-finger button release.
    FiveFingerRelease,
    /// Asks the tracked inertia window to stop — what a finger landing during a fling does.
    InertiaStop,
    /// Tells the tracked inertia window its inertia has ended.
    InertiaEnd,
}

impl TouchpadAction {
    const fn raw(self) -> TOUCHPAD_ACTION {
        match self {
            Self::ThreeFingerTap => TA_3FINGER_TAP,
            Self::ThreeFingerPress => TA_3FINGER_PRESS,
            Self::ThreeFingerRelease => TA_3FINGER_RELEASE,
            Self::FourFingerTap => TA_4FINGER_TAP,
            Self::FourFingerPress => TA_4FINGER_PRESS,
            Self::FourFingerRelease => TA_4FINGER_RELEASE,
            Self::FiveFingerTap => TA_5FINGER_TAP,
            Self::FiveFingerPress => TA_5FINGER_PRESS,
            Self::FiveFingerRelease => TA_5FINGER_RELEASE,
            Self::InertiaStop => TA_INERTIA_STOP,
            Self::InertiaEnd => TA_INERTIA_END,
        }
    }
}

/// How many contacts the parameter block allows a touchpad, and therefore how many this
/// stream has.
const CONTACTS: u32 = 5;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Phase {
    Absent,
    Down,
    Update,
    Up,
}

#[derive(Copy, Clone)]
struct Slot {
    phase: Phase,
    /// Himetric, relative to the pad's top-left.
    at: (i32, i32),
}

/// A precision touchpad.
pub struct TouchpadStream<'a> {
    injector: &'a mut Injector,
    device: Device,
    slots: [Slot; CONTACTS as usize],
}

impl<'a> TouchpadStream<'a> {
    pub(crate) fn open(injector: &'a mut Injector) -> Result<Self> {
        let device = Device::new(
            &injector.late,
            PT_TOUCHPAD as POINTER_INPUT_TYPE,
            CONTACTS,
            // The parameter block's own rule for a touchpad, not a preference.
            POINTER_FEEDBACK_NONE,
            Some(PAD_MM),
            true,
        )?;
        Ok(Self {
            injector,
            device,
            slots: [Slot {
                phase: Phase::Absent,
                at: (0, 0),
            }; CONTACTS as usize],
        })
    }

    /// Injects one global gesture, or one inertia signal.
    pub fn action(&mut self, action: TouchpadAction) -> Result<&mut Self> {
        self.injector
            .late
            .inject_action(self.device.handle(), action.raw())?;
        Ok(self)
    }

    /// Puts a contact somewhere on the pad, without injecting.
    ///
    /// `at` is a fraction of the pad's surface, origin top-left, so `(0.5, 0.5)` is its
    /// centre and `(0.0, 0.0)` its corner. Values outside 0–1 are refused rather than
    /// clamped: a contact off the edge of the pad is a mistake in the test, not an input
    /// the device could produce.
    pub fn set(&mut self, index: u32, at: (f32, f32)) -> Result<&mut Self> {
        let (u, v) = at;
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return Err(Error::Contact {
                index,
                max: CONTACTS,
            });
        }
        let at = (
            (u * PAD_MM.0 * 100.0).round() as i32,
            (v * PAD_MM.1 * 100.0).round() as i32,
        );
        let slot = self.slot(index)?;
        slot.phase = match slot.phase {
            Phase::Absent | Phase::Up => Phase::Down,
            live => live,
        };
        slot.at = at;
        Ok(self)
    }

    /// Ends a contact, without injecting.
    pub fn lift(&mut self, index: u32) -> Result<&mut Self> {
        let slot = self.slot(index)?;
        if slot.phase == Phase::Absent {
            return Err(Error::NotDown);
        }
        slot.phase = Phase::Up;
        Ok(self)
    }

    /// Delivers one frame carrying every live contact.
    pub fn frame(&mut self) -> Result<&mut Self> {
        let mut frame = Vec::with_capacity(self.slots.len());
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.phase == Phase::Absent {
                continue;
            }
            frame.push(contact(index as u32, slot));
        }
        if frame.is_empty() {
            return Ok(self);
        }
        self.device.inject(&frame)?;
        for slot in &mut self.slots {
            slot.phase = match slot.phase {
                Phase::Up | Phase::Absent => Phase::Absent,
                _ => Phase::Update,
            };
        }
        Ok(self)
    }

    /// Pans `fingers` contacts together, from one fraction of the pad to another, at
    /// `rate`.
    ///
    /// Two fingers is the pan the system reads as scroll; three and four are the global
    /// gestures, which arrive as [`action`](Self::action) rather than as contacts.
    pub fn pan(
        &mut self,
        fingers: u32,
        from: (f32, f32),
        to: (f32, f32),
        steps: u32,
        rate: Rate,
    ) -> Result<&mut Self> {
        if fingers == 0 || fingers > CONTACTS {
            return Err(Error::Contact {
                index: fingers,
                max: CONTACTS,
            });
        }
        let steps = steps.max(1);
        // Fingers sit side by side across the pad, a tenth of its width apart, which is
        // about what a hand does and keeps them inside the surface for any start point the
        // caller can state.
        let spread = 0.1;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
            for finger in 0..fingers {
                let offset = (finger as f32 - (fingers - 1) as f32 / 2.0) * spread;
                self.set(finger, ((x + offset).clamp(0.0, 1.0), y))?;
            }
            self.frame()?;
            self.injector.pace.wait(rate)?;
        }
        Ok(self)
    }

    /// Waits without injecting.
    pub fn hold(&mut self, duration: core::time::Duration) -> Result<&mut Self> {
        self.injector.pace.sleep(duration)?;
        Ok(self)
    }

    fn slot(&mut self, index: u32) -> Result<&mut Slot> {
        self.slots.get_mut(index as usize).ok_or(Error::Contact {
            index,
            max: CONTACTS,
        })
    }
}

impl Drop for TouchpadStream<'_> {
    /// Lifts anything the caller left down.
    fn drop(&mut self) {
        let live: Vec<u32> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.phase != Phase::Absent)
            .map(|(index, _)| index as u32)
            .collect();
        if live.is_empty() {
            return;
        }
        for index in live {
            _ = self.lift(index);
        }
        _ = self.frame();
    }
}

fn contact(id: u32, slot: &Slot) -> POINTER_TYPE_INFO {
    let flags = match slot.phase {
        Phase::Down => POINTER_FLAG_DOWN | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT,
        Phase::Update => POINTER_FLAG_UPDATE | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT,
        Phase::Up => POINTER_FLAG_UP,
        Phase::Absent => POINTER_FLAG_NONE,
    };
    POINTER_TYPE_INFO {
        r#type: PT_TOUCHPAD as POINTER_INPUT_TYPE,
        Anonymous: POINTER_TYPE_INFO_0 {
            // A touchpad contact answers in a `POINTER_TOUCH_INFO`, not a structure of its
            // own: the extended fields are identical for touch and touchpad, which is the
            // same reason `GetPointerTouchpadInfo` reads one.
            touchInfo: POINTER_TOUCH_INFO {
                pointerInfo: POINTER_INFO {
                    pointerType: PT_TOUCHPAD as POINTER_INPUT_TYPE,
                    pointerId: id,
                    pointerFlags: flags as POINTER_FLAGS,
                    // Device space, because the device was created with a physical size.
                    // `ptPixelLocation` is not read at all for such a device, and filling
                    // it in would be stating a screen position a touchpad does not have.
                    ptHimetricLocation: POINT {
                        x: slot.at.0,
                        y: slot.at.1,
                    },
                    ..Default::default()
                },
                touchFlags: TOUCH_FLAG_NONE as TOUCH_FLAGS,
                touchMask: TOUCH_MASK_PRESSURE as TOUCH_MASK,
                pressure: 512,
                ..Default::default()
            },
        },
    }
}
