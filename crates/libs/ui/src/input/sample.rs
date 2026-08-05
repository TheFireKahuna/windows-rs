//! Reads a pointer into a plain sample, in the two coordinate spaces a decision needs.
//!
//! # Predicted, or raw — per decision, not per application
//!
//! The platform predicts. `ptPixelLocation` is the **predicted** screen position, corrected
//! from the digitizer reading plus pointer motion to compensate for visual lag;
//! `ptPixelLocationRaw` is the unprocessed one. The correction applies to touch; for every
//! other pointer type the two are identical.
//!
//! | Use | Which | Why |
//! |---|---|---|
//! | continuous motion — gesture feed, drag path, manipulation | predicted | latency compensation the system has already computed; refusing it makes touch drags lag |
//! | discrete decisions — press target, hover resolve, **any hit test** | raw | an extrapolated point is wrong at contact start and at direction reversals, so a target chosen from one is a mis-click |
//! | touch-target sizing | `rcContactRaw` | the unadjusted contact size |
//!
//! Both values are in the `POINTER_INFO` already read, so carrying both costs nothing.
//!
//! # This path allocates nothing
//!
//! Hover reads into a stack struct and constructs no `PointerPoint`, which would be a WinRT
//! object per sample on the one path that is always on. Nothing in this module can reach the
//! recogniser.

use super::coords::Coords;
use super::doorbell::{PointerFlags, PointerType};
use super::dynamic::Late;
use crate::bindings::*;
use windows_scene::{Env, Point};

/// Holds what a pen reports beyond a position.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Pen {
    /// 0..=1, from the platform's 0..=1024.
    pub pressure: f32,
    /// Degrees from vertical, −90..=90.
    pub tilt_x: f32,
    /// Degrees from vertical, −90..=90.
    pub tilt_y: f32,
    /// Barrel rotation in degrees, 0..360.
    pub twist: f32,
    /// The transducer is inverted.
    pub inverted: bool,
    /// The eraser button is down.
    pub eraser: bool,
    /// The barrel button is down.
    pub barrel: bool,
}

/// Holds one reading of one pointer.
///
/// `Copy` and owning nothing, so the always-on path passes it by value and allocates nothing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sample {
    /// The platform pointer id the reading came from.
    pub id: u32,
    /// The device kind that produced the reading.
    pub ptype: PointerType,
    /// The `POINTER_INFO` flags, as read.
    pub flags: PointerFlags,
    /// Predicted, **client DIPs**. What continuous motion integrates.
    pub at: Point,
    /// Raw, **client DIPs**. What every discrete decision resolves through.
    pub raw: Point,
    /// The unadjusted contact patch, width and height in DIPs. `(0, 0)` where the device
    /// reports none, which is every mouse and most pens.
    pub contact: (f32, f32),
    /// Pen detail, `None` for every other pointer type.
    pub pen: Option<Pen>,
    /// The system tick count the sample was stamped with, in milliseconds.
    pub time: u32,
    /// The high-resolution performance counter the sample was stamped with.
    ///
    /// Without it, a tick that consumes a frame's worth of samples and a tick that consumes
    /// fifty after a stalled pump are indistinguishable, so any rate derived from sample
    /// count alone — a velocity, a fling's energy, a dwell — is wrong under load. Zero where
    /// the source carries no counter, which is [`Reader::at_transition`].
    pub qpc: u64,
}

impl Sample {
    /// Returns the contact kind the hit authority resolves with.
    #[must_use]
    pub const fn kind(&self) -> windows_scene::ContactKind {
        self.ptype.contact()
    }
}

/// Reads pointers into buffers the caller owns.
///
/// # Every sample, not the newest one
///
/// A pointer stream carries three kinds of quantity, and they are not interchangeable:
///
/// * **State at an instant** — what is under the pointer now. Observable only at a display
///   frame, so sampling it at display rate loses nothing.
/// * **Integrals over the path** — displacement, velocity, rotation, scale. Every sample
///   contributes, so sampling loses energy and the answer is wrong.
/// * **Events at a point on the path** — a press, a region crossing, a threshold crossing.
///   Every sample must be *examined* or the event vanishes; sampling **aliases**, and
///   nothing downstream can tell that it happened.
///
/// Hover is the third kind wearing the first kind's clothes: what is under the pointer is
/// state, but *hover* is the accumulated result of enter and leave events, which are boundary
/// crossings on a path. So [`batch`](Self::batch) is read and every entry examined.
/// Frame-bounding the publish discards work; frame-sampling the input discards information no
/// consumer can recover.
///
/// The batch costs one `GetPointerInfoHistory` in place of one `GetPointerInfo` — the same
/// syscall with a bigger buffer — plus a linear scan of the flat hit array per entry.
#[derive(Copy, Clone, Debug)]
pub struct Reader {
    late: Late,
}

impl Reader {
    /// Creates a reader over the `user32` exports `late` resolved.
    #[must_use]
    pub const fn new(late: Late) -> Self {
        Self { late }
    }

    /// Returns the late-resolved exports this build of `user32` provides.
    #[must_use]
    pub const fn late(&self) -> &Late {
        &self.late
    }

    /// Returns the newest reading of pointer `id`, in client DIPs.
    ///
    /// **The hover path**: it allocates nothing.
    ///
    /// `None` once the pointer is retired or its information has aged out, which is not an
    /// error: it is what a contact that ended between the message and the tick looks like.
    #[must_use]
    pub fn newest(&self, id: u32, coords: &Coords, env: Env) -> Option<Sample> {
        let mut info = POINTER_INFO::default();
        // SAFETY: `info` is a stack local of the type the call writes.
        if !unsafe { GetPointerInfo(id, &mut info) }.as_bool() {
            return None;
        }
        Some(self.sample(&info, coords, env))
    }

    /// Fills `buf` with every coalesced reading of `id`, **oldest first**, and returns how
    /// many it wrote.
    ///
    /// The platform answers most-recent-first and this reverses it, because a path is walked
    /// forwards: a threshold crossed on the way out is a different event from the same
    /// threshold crossed on the way back, and integrating a reversed path is a different
    /// gesture.
    ///
    /// Zero means `buf` is empty, or the pointer is retired or its information has aged out,
    /// which is not an error: it is what a contact that ended between the message and the
    /// tick looks like.
    pub fn batch(&self, id: u32, buf: &mut [POINTER_INFO]) -> usize {
        if buf.is_empty() {
            return 0;
        }
        let mut count = buf.len() as u32;
        // SAFETY: `count` is `buf`'s own length, so the call writes at most as many entries
        // as `buf` holds and reports how many it wrote through the same variable.
        if !unsafe { GetPointerInfoHistory(id, &mut count, buf.as_mut_ptr()) }.as_bool() {
            return 0;
        }
        let count = (count as usize).min(buf.len());
        buf[..count].reverse();
        count
    }

    /// Converts one `POINTER_INFO` reading into a sample, in client DIPs.
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

    /// Builds a sample for a discrete transition from the point the doorbell recorded **at
    /// the message**, rather than from wherever the pointer has since moved to.
    ///
    /// `at` and `raw` are the same point: a press is a discrete decision, so the predicted
    /// position has no meaning for it and the raw one is what a target is chosen from. `qpc`
    /// is zero, because the message carries no performance counter.
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
            // A ring record carries the message's tick count and not a performance counter,
            // which the message does not hold. Zero reads as unstamped.
            qpc: 0,
        }
    }

    /// Reads per-device detail through the accessor `ptype` selects: a contact extent for
    /// touch and touchpad, pen state for a pen, and neither for a mouse.
    ///
    /// A touchpad answers in a `POINTER_TOUCH_INFO` like touch does, through an export the
    /// SDK does not name ([`Late`]). Where that export is absent the contact patch is
    /// unknown and reads as `(0, 0)`, a mouse-sized target for a device that reports as a
    /// cursor anyway.
    fn detail(&self, id: u32, ptype: PointerType, scale: f32) -> ((f32, f32), Option<Pen>) {
        match ptype {
            PointerType::Touch => {
                let mut info = POINTER_TOUCH_INFO::default();
                // SAFETY: `info` is a stack local of the type the call writes, and the
                // pointer reported this type, so the accessor is the one it answers.
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
                // SAFETY: `info` is a stack local of the type the call writes, and the
                // pointer reported this type, so the accessor is the one it answers.
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

/// Returns a contact rectangle's width and height in DIPs.
///
/// The rectangle is screen physical and only its extent crosses; the sample carries the
/// contact's position separately. A `scale` that is not positive returns `(0, 0)`.
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
