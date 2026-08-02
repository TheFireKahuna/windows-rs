//! Reading a pointer, in the two spaces a decision needs.
//!
//! # Predicted, or raw — per decision, not per application
//!
//! The platform already predicts. `ptPixelLocation` is the **predicted** screen position,
//! corrected from the digitizer reading plus pointer motion to compensate for visual lag;
//! `ptPixelLocationRaw` is the unprocessed one. The correction applies to touch — for every
//! other pointer type the two are identical, which is why taking it costs nothing.
//!
//! | Use | Which | Why |
//! |---|---|---|
//! | continuous motion — gesture feed, drag path, manipulation | predicted | it is latency compensation the system already computed; refusing it makes touch drags lag for nothing |
//! | discrete decisions — press target, hover resolve, **any hit test** | raw | an extrapolated point is wrong at contact start and at direction reversals, and a target chosen from one is a mis-click rather than a smoother one |
//! | touch-target sizing | `rcContactRaw` | the honest contact size, not the adjusted one |
//!
//! Both values are in the `POINTER_INFO` already read, so carrying both is free.
//!
//! # This path allocates nothing
//!
//! Hover reads into a stack struct and never constructs a `PointerPoint` — a WinRT object
//! per sample, on the one path that is always on. That confinement is structural rather
//! than careful: nothing in this module can reach the recogniser.

use super::coords::Coords;
use super::doorbell::{PointerFlags, PointerType};
use super::dynamic::Late;
use crate::bindings::*;
use windows_scene::{Env, Point};

/// What a pen reports beyond a position.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Pen {
    /// 0..=1, from the platform's 0..=1024.
    pub pressure: f32,
    /// Degrees from vertical, −90..=90.
    pub tilt_x: f32,
    pub tilt_y: f32,
    /// Barrel rotation in degrees, 0..360.
    pub twist: f32,
    pub inverted: bool,
    pub eraser: bool,
    pub barrel: bool,
}

/// One reading of one pointer.
///
/// `Copy` and free of anything owned: this is what the always-on path traffics in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sample {
    pub id: u32,
    pub ptype: PointerType,
    pub flags: PointerFlags,
    /// Predicted, **client DIPs**. What continuous motion integrates.
    pub at: Point,
    /// Raw, **client DIPs**. What every discrete decision resolves through.
    pub raw: Point,
    /// The honest contact patch, width and height in DIPs. `(0, 0)` where the device reports
    /// none, which is every mouse and most pens.
    pub contact: (f32, f32),
    pub pen: Option<Pen>,
    /// The system tick count the sample was stamped with, in milliseconds.
    pub time: u32,
    /// The high-resolution performance counter the sample was stamped with.
    ///
    /// **Time is data.** Without it, a tick that consumes a frame's worth of samples and a
    /// tick that consumes fifty after a stalled pump are indistinguishable — so every
    /// rate-derived quantity (a velocity, a fling's energy, a dwell) is wrong exactly when
    /// the machine is under load, which is when it is least affordable. Nothing above the
    /// recogniser derives a rate *yet*; carrying it costs a field and removes the trap.
    pub qpc: u64,
}

impl Sample {
    /// The contact's kind, for the one hit authority.
    #[must_use]
    pub const fn kind(&self) -> windows_scene::ContactKind {
        self.ptype.contact()
    }
}

/// Reads pointers, into buffers the caller owns and nothing else.
///
/// # Every sample, not the newest one
///
/// There are three kinds of quantity in a pointer stream and they are not interchangeable:
///
/// * **State at an instant** — what is under the pointer now. Observable only at a display
///   frame, so sampling it at display rate loses nothing.
/// * **Integrals over the path** — displacement, velocity, rotation, scale. Every sample
///   contributes; sampling loses energy and the answer is simply wrong.
/// * **Events at a point on the path** — a press, a region crossing, a threshold crossing.
///   Every sample must be *examined* or the event vanishes; sampling **aliases**, and
///   nothing downstream can tell that it happened.
///
/// Hover is the third kind wearing the first kind's clothes: "what is under the pointer" is
/// state, but *hover* is the accumulated result of enter and leave events, which are
/// boundary crossings on a path. So the batch is read and every entry examined — because a
/// layer may discard **work**, and must not discard **information** a policy above it might
/// need. Frame-bounding the publish discards work. Frame-sampling the input discards
/// information, and no consumer can recover it.
///
/// The cost of doing it properly is one `GetPointerInfoHistory` in place of one
/// `GetPointerInfo` — the same syscall with a bigger buffer — plus a linear scan of the flat
/// hit array per entry, which the memo bounds to a handful of compares.
#[derive(Copy, Clone, Debug)]
pub struct Reader {
    late: Late,
}

impl Reader {
    /// A reader over whatever this build of `user32` turned out to export.
    #[must_use]
    pub const fn new(late: Late) -> Self {
        Self { late }
    }

    /// What the running build could not do.
    #[must_use]
    pub const fn late(&self) -> &Late {
        &self.late
    }

    /// The newest reading of one pointer. **The hover path**, and it allocates nothing.
    ///
    /// `None` once the pointer is retired or its information has aged out, which is not an
    /// error: it is what a contact that ended between the message and the tick looks like.
    #[must_use]
    pub fn newest(&self, id: u32, coords: &Coords, env: Env) -> Option<Sample> {
        let mut info = POINTER_INFO::default();
        // SAFETY: the destination is a stack local of the type the call writes.
        if !unsafe { GetPointerInfo(id, &mut info) }.as_bool() {
            return None;
        }
        Some(self.sample(&info, coords, env))
    }

    /// Fills `buf` with every coalesced reading of `id`, **oldest first**, and answers how
    /// many it wrote.
    ///
    /// The platform answers most-recent-first; this reverses it, because a path is walked
    /// forwards — a threshold crossed on the way out is a different event from the same
    /// threshold crossed on the way back, and integrating a reversed path is a different
    /// gesture.
    ///
    /// Zero means the pointer is retired or its information has aged out, which is not an
    /// error: it is what a contact that ended between the message and the tick looks like.
    pub fn batch(&self, id: u32, buf: &mut [POINTER_INFO]) -> usize {
        if buf.is_empty() {
            return 0;
        }
        let mut count = buf.len() as u32;
        // SAFETY: `count` states the buffer's length and the buffer is at least that long;
        // the call writes at most `count` entries and reports how many it wrote.
        if !unsafe { GetPointerInfoHistory(id, &mut count, buf.as_mut_ptr()) }.as_bool() {
            return 0;
        }
        let count = (count as usize).min(buf.len());
        buf[..count].reverse();
        count
    }

    /// One history entry as a sample.
    #[must_use]
    pub fn sample(&self, info: &POINTER_INFO, coords: &Coords, env: Env) -> Sample {
        let ptype = PointerType::from_raw(info.pointerType);
        let id = info.pointerId;
        let (contact, pen) = self.detail(id, ptype, env.scale());
        Sample {
            id,
            ptype,
            flags: PointerFlags(info.pointerFlags),
            at: coords.client(env, id, info.ptPixelLocation.x, info.ptPixelLocation.y),
            raw: coords.client(
                env,
                id,
                info.ptPixelLocationRaw.x,
                info.ptPixelLocationRaw.y,
            ),
            contact,
            pen,
            time: info.dwTime,
            qpc: info.PerformanceCount,
        }
    }

    /// A discrete transition as a sample, from the point the doorbell recorded **at the
    /// message** rather than from wherever the pointer has since moved to.
    ///
    /// `at` and `raw` are the same point, and that is not a shortcut: a press is a discrete
    /// decision, so the predicted position has no meaning for it, and the raw one is what a
    /// target is chosen from.
    #[must_use]
    pub fn at_transition(&self, event: &super::PointerEvent, coords: &Coords, env: Env) -> Sample {
        let point = coords.client(env, event.id, event.x_px, event.y_px);
        let (contact, pen) = self.detail(event.id, event.ptype, env.scale());
        Sample {
            id: event.id,
            ptype: event.ptype,
            flags: event.flags,
            at: point,
            raw: point,
            contact,
            pen,
            time: event.time,
            // A ring record carries the message's tick count and not its performance
            // counter: the doorbell reads one `POINTER_INFO` per transition and the counter
            // is not in the message. Zero reads as "unstamped", which is what it is.
            qpc: 0,
        }
    }

    /// Per-device detail, from the accessor `GetPointerType` selected.
    ///
    /// A touchpad answers in a `POINTER_TOUCH_INFO` like touch does, through an export the
    /// SDK does not name ([`Late`]). Where that export is absent the contact patch is simply
    /// unknown, which reads as `(0, 0)` — a mouse-sized target, which is the conservative
    /// answer for a device that reports as a cursor anyway.
    fn detail(&self, id: u32, ptype: PointerType, scale: f32) -> ((f32, f32), Option<Pen>) {
        match ptype {
            PointerType::Touch => {
                let mut info = POINTER_TOUCH_INFO::default();
                // SAFETY: the destination is a stack local of the type the call writes, and
                // the pointer type was just read.
                if unsafe { GetPointerTouchInfo(id, &mut info) }.as_bool() {
                    return (extent(&info.rcContactRaw, scale), None);
                }
                ((0.0, 0.0), None)
            }
            PointerType::Touchpad => match self.late.touchpad_info(id) {
                Some(info) => (extent(&info.rcContactRaw, scale), None),
                None => ((0.0, 0.0), None),
            },
            PointerType::Pen => {
                let mut info = POINTER_PEN_INFO::default();
                // SAFETY: as above.
                if unsafe { GetPointerPenInfo(id, &mut info) }.as_bool() {
                    return (
                        (0.0, 0.0),
                        Some(Pen {
                            pressure: info.pressure as f32 / 1024.0,
                            tilt_x: info.tiltX as f32,
                            tilt_y: info.tiltY as f32,
                            twist: info.rotation as f32,
                            inverted: info.penFlags & PEN_FLAG_INVERTED as u32 != 0,
                            eraser: info.penFlags & PEN_FLAG_ERASER as u32 != 0,
                            barrel: info.penFlags & PEN_FLAG_BARREL as u32 != 0,
                        }),
                    );
                }
                ((0.0, 0.0), None)
            }
            PointerType::Mouse => ((0.0, 0.0), None),
        }
    }
}

/// A contact rectangle's extent in DIPs. Screen physical in, so only the size crosses — a
/// contact patch has no origin worth carrying.
fn extent(rect: &RECT, scale: f32) -> (f32, f32) {
    if scale <= 0.0 {
        return (0.0, 0.0);
    }
    (
        (rect.right - rect.left) as f32 / scale,
        (rect.bottom - rect.top) as f32 / scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_contact_patch_crosses_as_an_extent_in_dips() {
        let rect = RECT {
            left: 100,
            top: 200,
            right: 130,
            bottom: 245,
        };
        assert_eq!(extent(&rect, 1.5), (20.0, 30.0));
        // A scale that has not been resolved yet cannot divide.
        assert_eq!(extent(&rect, 0.0), (0.0, 0.0));
    }
}
