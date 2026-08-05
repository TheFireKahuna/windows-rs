//! The touch stream, which the fidelity contracts are asserted on.
//!
//! A touch contact carries its own screen position, so it is exact by construction with no
//! normalization to invert, and it arrives on a device of its own, so no physical device at
//! the machine interferes with it. Every claim that can be made on touch is made on touch;
//! mouse covers what only a mouse produces.
//!
//! This is the one stream that does not go through `InputInjector`. That object's touch is a
//! virtual device, which is brokered, and an unpackaged process cannot create one: the calls
//! succeed and deliver nothing ([`identity`](crate::identity)). `InitializeTouchInjection`
//! and `InjectTouchInput` need no device and deliver unpackaged.
//!
//! Every injected call carries the whole live contact set. A frame that omits a contact still
//! down ends that contact, so a two-finger gesture written as two independent one-finger
//! streams cancels itself on the second call. The stream owns the contact table and assembles
//! the frame, so moving one contact holds the others where they are.
//!
//! The API is two levels for that reason. [`set`](TouchStream::set), [`lift`] and [`abort`]
//! stage a contact, [`frame`] delivers one frame carrying all of them, and the single-contact
//! methods pair the two. A pinch has to be written with the staged level, because both
//! fingers move in one frame or the gesture is two drags.
//!
//! Two limits come from the platform's own contract. Injected touch has exactly six legal
//! flag combinations, and none of them admits `POINTER_FLAG_CONFIDENCE`, so palm rejection is
//! not reachable from an injected stream and a test for it needs a real digitizer. And an
//! `UP` frame must repeat the position of the frame before it, or the injection fails and
//! every live contact is cancelled, which is why [`lift`] takes no point.
//!
//! [`lift`]: TouchStream::lift
//! [`abort`]: TouchStream::abort
//! [`frame`]: TouchStream::frame

use windows_core::WIN32_ERROR;

use crate::bindings::*;
use crate::space::Point;
use crate::{Error, Injector, Rate, Result};

/// The one refusal [`inject`] retries.
const NOT_READY: WIN32_ERROR = WIN32_ERROR(ERROR_NOT_READY as u32);

/// The default width of a contact box, in DIPs.
///
/// Touch-target sizing reads the contact rectangle, so this is the width of a finger rather
/// than of a point: a 2-pixel dot would exercise the inflation rule against a contact no
/// finger makes.
const CONTACT_DIPS: f32 = 8.0;

/// Where one contact is in the frame about to be injected.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Phase {
    /// Not in the frame at all.
    Absent,
    /// Starting in this frame.
    Down,
    /// Continuing.
    Update,
    /// Ending in this frame.
    Up,
    /// Ending in this frame, withdrawn rather than released.
    Cancel,
}

impl Phase {
    const fn flags(self) -> i32 {
        match self {
            Self::Down => POINTER_FLAG_DOWN | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT,
            Self::Update => POINTER_FLAG_UPDATE | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT,
            Self::Up => POINTER_FLAG_UP,
            Self::Cancel => POINTER_FLAG_UP | POINTER_FLAG_CANCELED,
            Self::Absent => POINTER_FLAG_NONE,
        }
    }
}

#[derive(Copy, Clone)]
struct Slot {
    phase: Phase,
    at: (i32, i32),
}

/// A digitizer with a fixed number of contacts, aimed at the injector's space.
pub struct TouchStream<'a> {
    injector: &'a mut Injector,
    slots: Vec<Slot>,
    contact_dips: f32,
    pressure: u32,
}

impl<'a> TouchStream<'a> {
    pub(crate) fn open(injector: &'a mut Injector, contacts: u32) -> Result<Self> {
        // The contact count is process-wide, so it is raised and never lowered: a narrower
        // stream opened later must not take contacts away from what the process declared.
        if contacts > injector.touch_max {
            // SAFETY: a count and a mode, both passed by value; no pointer crosses the
            // boundary.
            unsafe { InitializeTouchInjection(contacts, TOUCH_FEEDBACK_NONE as u32) }
                .ok()
                .map_err(|e| Error::call("InitializeTouchInjection", e))?;
            injector.touch_max = contacts;
        }
        Ok(Self {
            injector,
            slots: vec![
                Slot {
                    phase: Phase::Absent,
                    at: (0, 0),
                };
                contacts as usize
            ],
            contact_dips: CONTACT_DIPS,
            pressure: 512,
        })
    }

    /// Sets how wide a contact reports itself, in DIPs.
    pub fn contact_dips(&mut self, dips: f32) -> &mut Self {
        self.contact_dips = dips;
        self
    }

    /// Sets how hard a contact reports itself, from 0 to 1. Values outside that range clamp.
    pub fn pressure(&mut self, pressure: f32) -> &mut Self {
        self.pressure = (pressure.clamp(0.0, 1.0) * 1024.0).round() as u32;
        self
    }

    // ── Staged ────────────────────────────────────────────────────────────────

    /// Puts a contact somewhere, without injecting.
    ///
    /// A contact that was not down starts; one that was continues. Nothing is delivered until
    /// [`frame`](Self::frame).
    pub fn set(&mut self, index: u32, at: impl Into<Point>) -> Result<&mut Self> {
        let at = self.injector.space.to_px(at);
        let slot = self.slot(index)?;
        slot.phase = match slot.phase {
            Phase::Absent | Phase::Up | Phase::Cancel => Phase::Down,
            live => live,
        };
        slot.at = at;
        Ok(self)
    }

    /// Ends a contact, without injecting.
    pub fn lift(&mut self, index: u32) -> Result<&mut Self> {
        self.ending(index, Phase::Up)
    }

    /// Withdraws a contact, without injecting.
    ///
    /// A cancel is not an up: it aborts the gesture rather than completing it, so no value is
    /// committed. Legacy mouse messages cannot express the distinction, and an injected mouse
    /// stream cannot produce it.
    pub fn abort(&mut self, index: u32) -> Result<&mut Self> {
        self.ending(index, Phase::Cancel)
    }

    /// Delivers one frame carrying every live contact.
    pub fn frame(&mut self) -> Result<&mut Self> {
        let half = (self.contact_dips * self.injector.space.scale() / 2.0).round() as i32;
        let mut frame = Vec::with_capacity(self.slots.len());
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.phase == Phase::Absent {
                continue;
            }
            frame.push(contact(index as u32, slot, half, self.pressure));
        }
        if frame.is_empty() {
            return Ok(self);
        }
        inject(&frame)?;
        for slot in &mut self.slots {
            slot.phase = match slot.phase {
                Phase::Up | Phase::Cancel | Phase::Absent => Phase::Absent,
                _ => Phase::Update,
            };
        }
        Ok(self)
    }

    // ── One contact ───────────────────────────────────────────────────────────

    /// Starts contact 0 at a point.
    pub fn down(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.set(0, at)?.frame()
    }

    /// Moves contact 0.
    pub fn move_to(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.set(0, at)?.frame()
    }

    /// Ends contact 0.
    pub fn up(&mut self) -> Result<&mut Self> {
        self.lift(0)?.frame()
    }

    /// Withdraws contact 0. See [`abort`](Self::abort).
    pub fn cancel(&mut self) -> Result<&mut Self> {
        self.abort(0)?.frame()
    }

    /// Down and up at one point, without moving.
    pub fn tap(&mut self, at: impl Into<Point>) -> Result<&mut Self> {
        self.down(at)?.up()
    }

    /// Walks contact 0 along a path, one frame per point, at `rate`.
    ///
    /// One frame per point: a touch frame is a set of contacts rather than a series in time,
    /// so it carries no field stating when the next frame lands.
    pub fn polyline(&mut self, points: &[Point], rate: Rate) -> Result<&mut Self> {
        for point in points {
            self.set(0, *point)?.frame()?;
            self.injector.pace.wait(rate)?;
        }
        Ok(self)
    }

    /// Waits without injecting, so a contact becomes a hold.
    pub fn hold(&mut self, duration: core::time::Duration) -> Result<&mut Self> {
        self.injector.pace.sleep(duration)?;
        Ok(self)
    }

    // ── Two contacts ──────────────────────────────────────────────────────────

    /// Moves two contacts either side of `centre` from a separation of `from` DIPs to `to`
    /// DIPs, one frame per step.
    ///
    /// The step count is derived from the distance so that no contact moves more than one DIP
    /// between frames, which keeps the path continuous at the resolution the recogniser
    /// measures in. Returns [`Error::Contact`] if the stream was opened with fewer than two
    /// contacts.
    pub fn pinch(
        &mut self,
        centre: impl Into<Point>,
        from: f32,
        to: f32,
        rate: Rate,
    ) -> Result<&mut Self> {
        if self.slots.len() < 2 {
            return Err(Error::Contact {
                index: 1,
                max: self.slots.len() as u32,
            });
        }
        let centre = centre.into();
        let steps = ((to - from).abs() / 2.0).ceil().max(1.0) as u32;
        for step in 0..=steps {
            let span = from + (to - from) * step as f32 / steps as f32;
            self.set(0, Point::new(centre.x - span / 2.0, centre.y))?
                .set(1, Point::new(centre.x + span / 2.0, centre.y))?
                .frame()?;
            self.injector.pace.wait(rate)?;
        }
        Ok(self)
    }

    // ── Internals ─────────────────────────────────────────────────────────────

    fn slot(&mut self, index: u32) -> Result<&mut Slot> {
        let max = self.slots.len() as u32;
        self.slots
            .get_mut(index as usize)
            .ok_or(Error::Contact { index, max })
    }

    fn ending(&mut self, index: u32, phase: Phase) -> Result<&mut Self> {
        let slot = self.slot(index)?;
        if slot.phase == Phase::Absent {
            return Err(Error::NotDown);
        }
        slot.phase = phase;
        Ok(self)
    }
}

impl Drop for TouchStream<'_> {
    /// Lifts anything the caller left down.
    ///
    /// Without this the system believes those contacts are still on the digitizer, and every
    /// later test in the process inherits them.
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

/// Delivers one frame, retrying `ERROR_NOT_READY` and no other failure.
///
/// Two frames landing inside the same tenth of a millisecond are refused with
/// `ERROR_NOT_READY`, and the documented response is to send the same frame again: the
/// injection has not happened, so the retry repeats a call that delivered nothing rather than
/// adding a sample. Every other failure is returned.
fn inject(frame: &[POINTER_TOUCH_INFO]) -> Result<()> {
    const ATTEMPTS: u32 = 8;
    for attempt in 1..=ATTEMPTS {
        // SAFETY: `frame` is a slice, so it is a contiguous run of initialized records, and
        // the count passed is its own length.
        if unsafe { InjectTouchInput(frame.len() as u32, frame.as_ptr()) }.as_bool() {
            return Ok(());
        }
        let error = windows_core::Error::from_thread();
        if attempt == ATTEMPTS || WIN32_ERROR::from_error(&error) != Some(NOT_READY) {
            return Err(Error::call("InjectTouchInput", error));
        }
        std::thread::yield_now();
    }
    unreachable!("the loop returns on every attempt")
}

fn contact(id: u32, slot: &Slot, half: i32, pressure: u32) -> POINTER_TOUCH_INFO {
    let (x, y) = slot.at;
    POINTER_TOUCH_INFO {
        pointerInfo: POINTER_INFO {
            pointerType: PT_TOUCH as POINTER_INPUT_TYPE,
            pointerId: id,
            pointerFlags: slot.phase.flags() as POINTER_FLAGS,
            ptPixelLocation: POINT { x, y },
            ..Default::default()
        },
        touchFlags: TOUCH_FLAG_NONE as TOUCH_FLAGS,
        touchMask: (TOUCH_MASK_CONTACTAREA | TOUCH_MASK_ORIENTATION | TOUCH_MASK_PRESSURE)
            as TOUCH_MASK,
        rcContact: RECT {
            left: x - half,
            top: y - half,
            right: x + half,
            bottom: y + half,
        },
        orientation: 90,
        pressure,
        ..Default::default()
    }
}
