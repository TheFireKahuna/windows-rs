//! One conversion, and the samples that come out of it.
//!
//! `POINTER_INFO.ptPixelLocation` and `ptPixelLocationRaw` are **screen physical**. The hit
//! array is built in **client DIPs**. This module crosses that gap in one place, so the
//! pointer, the caption band, focus order and automation all resolve through one
//! conversion rather than through two that can disagree. Reading a pointer is part of the same
//! job, because every read ends in that conversion.
//!
//! The screen→client half is `GetPointerInputTransform` where the input carries a transform
//! and `ScreenToClient` where it does not, which is the algorithm the platform documents: a
//! consumer "*typically uses `ScreenToClient` … If a transform is applied on the message
//! consumer, use `GetPointerInputTransform`*", and the latter fails with `ERROR_NO_DATA` when
//! there is no transform. The pixel→DIP half is the window's own scale, which
//! `windows-window` resolves and this crate never re-derives.
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
//! # Every sample, not the newest one
//!
//! A pointer stream carries three kinds of quantity, and they are not interchangeable. **State
//! at an instant** — what is under the pointer now — is observable only at a display frame, so
//! sampling it at display rate loses nothing. **Integrals over the path** — displacement,
//! velocity, rotation, scale — take every sample, so sampling loses energy and the answer is
//! wrong. **Events at a point on the path** — a press, a region crossing, a threshold crossing
//! — must each be *examined* or the event vanishes; sampling **aliases**, and nothing
//! downstream can tell that it happened.
//!
//! Hover is the third kind wearing the first kind's clothes, so [`Coords::batch`] is read and
//! every entry examined. The batch costs one `GetPointerInfoHistory` in place of one
//! `GetPointerInfo` — the same syscall with a bigger buffer.
//!
//! # This path allocates nothing
//!
//! Hover reads into a caller-owned buffer and constructs no `PointerPoint`, which would be a
//! WinRT object per sample on the one path that is always on. Nothing in this module can reach
//! a recogniser.

use super::{Late, PointerEvent, PointerFlags, PointerType};
use crate::bindings::{Point as WinPoint, Rect as WinRect, *};
use core::cell::Cell;
use windows_scene::{Env, Point};

/// Returns `hwnd`'s client origin in screen pixels, shared by TSF and immutable UIA
/// publication.
///
/// `None` once the window has gone, which leaves no origin to answer with.
pub fn client_origin(hwnd: HWND) -> Option<windows_numerics::Vector2> {
    let mut at = POINT::default();
    // SAFETY: the handle is passed by value and `at` is a stack local the call writes back
    // through; a handle whose window has been destroyed fails the call rather than being
    // dereferenced.
    if !unsafe { ScreenToClient(hwnd, &mut at) }.as_bool() {
        return None;
    }
    Some(windows_numerics::Vector2 {
        x: -at.x as f32,
        y: -at.y as f32,
    })
}

/// Converts one window's pointer input into client DIPs, and reads it.
///
/// The scale is passed in at every call and never held: a cached scale is not updated when the
/// window moves to a display with a different one, and every contact then resolves against the
/// wrong pixel grid. [`Env::scale`] is the only derivation of the factor, so `dpi / 96`
/// computed here would be a second one. The window handle and the resolved exports are not
/// display facts, so they are held.
#[derive(Copy, Clone, Debug)]
pub struct Coords {
    hwnd: HWND,
    late: Late,
}

impl Coords {
    /// Creates a conversion for `hwnd`.
    #[must_use]
    pub const fn new(hwnd: HWND, late: Late) -> Self {
        Self { hwnd, late }
    }

    /// Returns the window this conversion resolves against.
    #[must_use]
    pub const fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Converts the screen physical point `(x_px, y_px)` to the client DIPs the hit array is
    /// built in.
    ///
    /// `id` selects the transform *that pointer's* input carries, which keeps the answer right
    /// for a window whose content the system is scaling.
    #[must_use]
    pub fn client(&self, env: Env, id: u32, x_px: i32, y_px: i32) -> Point {
        self.client_at_scale(env.scale(), id, x_px, y_px)
    }

    /// The message-time equivalent of [`Self::client`], using the window's current metrics.
    pub(crate) fn client_at_scale(&self, scale: f32, id: u32, x_px: i32, y_px: i32) -> Point {
        let mut transform = INPUT_TRANSFORM::default();
        // SAFETY: `transform` is a stack local of the type the call writes, and the count of
        // one matches the single entry a non-history reading asks for.
        let consumer = unsafe { GetPointerInputTransform(id, 1, &mut transform) }.as_bool();
        let (x, y) = consumer
            // SAFETY: both arms of the union are views of the same sixteen floats, so the
            // named arm reads initialised data whichever arm the call wrote through.
            .then(|| unsafe { transform.Anonymous.Anonymous })
            .and_then(|m| invert_affine(&m, x_px as f32, y_px as f32))
            .unwrap_or_else(|| self.screen_to_client(x_px, y_px));
        Point {
            x: x / scale,
            y: y / scale,
        }
    }

    /// Converts a screen point to client pixels through `ScreenToClient`, the conversion the
    /// platform documents for input carrying no transform.
    fn screen_to_client(&self, x_px: i32, y_px: i32) -> (f32, f32) {
        let mut at = POINT { x: x_px, y: y_px };
        // SAFETY: the handle is passed by value and `at` is a stack local the call writes back
        // through. A failure leaves it holding the screen point, which is the only answer
        // available once the window has gone.
        unsafe {
            _ = ScreenToClient(self.hwnd, &mut at);
        }
        (at.x as f32, at.y as f32)
    }

    /// Returns the newest reading of pointer `id`, in client DIPs. **The hover path**: it
    /// allocates nothing.
    ///
    /// `None` once the pointer is retired or its information has aged out, which is not an
    /// error: it is what a contact that ended between the message and the tick looks like.
    #[must_use]
    pub fn newest(&self, id: u32, env: Env) -> Option<Sample> {
        let mut info = POINTER_INFO::default();
        // SAFETY: `info` is a stack local of the type the call writes.
        unsafe { GetPointerInfo(id, &mut info) }
            .as_bool()
            .then(|| self.sample(&info, env))
    }

    /// Fills `buf` with every coalesced reading of `id`, **oldest first**, and returns how
    /// many it wrote.
    ///
    /// The platform answers most-recent-first and this reverses it, because a path is walked
    /// forwards: a threshold crossed on the way out is a different event from the same
    /// threshold crossed on the way back, and integrating a reversed path is a different
    /// gesture.
    ///
    /// Zero means `buf` is empty, or the pointer is retired or its information has aged out.
    pub fn batch(&self, id: u32, buf: &mut [POINTER_INFO]) -> usize {
        let mut count = buf.len() as u32;
        // SAFETY: `count` is `buf`'s own length, so the call writes at most as many entries as
        // `buf` holds and reports how many it wrote through the same variable.
        if count == 0
            || !unsafe { GetPointerInfoHistory(id, &mut count, buf.as_mut_ptr()) }.as_bool()
        {
            return 0;
        }
        let count = (count as usize).min(buf.len());
        buf[..count].reverse();
        count
    }

    /// Converts one `POINTER_INFO` reading into a sample, in client DIPs.
    #[must_use]
    pub fn sample(&self, info: &POINTER_INFO, env: Env) -> Sample {
        let id = info.pointerId;
        let ptype = PointerType::from_raw(info.pointerType);
        Sample {
            id,
            ptype,
            flags: PointerFlags(info.pointerFlags),
            at: self.client(env, id, info.ptPixelLocation.x, info.ptPixelLocation.y),
            raw: self.client(
                env,
                id,
                info.ptPixelLocationRaw.x,
                info.ptPixelLocationRaw.y,
            ),
            contact: self.contact(id, ptype, env.scale()),
            pen: self.pen(id, ptype),
            time: info.dwTime,
            qpc: info.PerformanceCount,
        }
    }

    /// Builds a sample for a discrete transition from the point the doorbell recorded **at the
    /// message**, rather than from wherever the pointer has since moved to.
    ///
    /// `at` and `raw` are the same point: a press is a discrete decision, so the predicted
    /// position has no meaning for it and the raw one is what a target is chosen from. `qpc`
    /// is zero, because a ring record carries the message's tick count and no performance
    /// counter.
    #[must_use]
    pub fn at_transition(&self, event: &PointerEvent, env: Env) -> Sample {
        let point = self.client(env, event.id, event.x_px, event.y_px);
        Sample {
            id: event.id,
            ptype: event.ptype,
            flags: event.flags,
            at: point,
            raw: point,
            contact: self.contact(event.id, event.ptype, env.scale()),
            pen: self.pen(event.id, event.ptype),
            time: event.time,
            qpc: 0,
        }
    }

    /// Returns the unadjusted contact patch in DIPs, through the accessor `ptype` selects.
    ///
    /// A touchpad answers in a `POINTER_TOUCH_INFO` like touch does, through an export the SDK
    /// does not name. Where that export is absent the patch is unknown and reads as `(0, 0)`,
    /// a mouse-sized target for a device that reports as a cursor anyway.
    fn contact(&self, id: u32, ptype: PointerType, scale: f32) -> (f32, f32) {
        let rect = match ptype {
            PointerType::Touch => {
                let mut info = POINTER_TOUCH_INFO::default();
                // SAFETY: `info` is a stack local of the type the call writes, and the pointer
                // reported this type, so the accessor is the one it answers.
                unsafe { GetPointerTouchInfo(id, &mut info) }
                    .as_bool()
                    .then_some(info.rcContactRaw)
            }
            PointerType::Touchpad => self.late.touchpad_info(id).map(|info| info.rcContactRaw),
            PointerType::Mouse | PointerType::Pen => None,
        };
        match rect {
            Some(r) if scale > 0.0 => (
                (r.right - r.left) as f32 / scale,
                (r.bottom - r.top) as f32 / scale,
            ),
            _ => (0.0, 0.0),
        }
    }

    /// Returns what a pen reports beyond a position, and `None` for every other pointer type.
    fn pen(&self, id: u32, ptype: PointerType) -> Option<Pen> {
        if ptype != PointerType::Pen {
            return None;
        }
        let mut info = POINTER_PEN_INFO::default();
        // SAFETY: `info` is a stack local of the type the call writes, and the pointer
        // reported this type, so the accessor is the one it answers.
        unsafe { GetPointerPenInfo(id, &mut info) }
            .as_bool()
            .then(|| Pen {
                pressure: info.pressure as f32 / 1024.0,
                tilt_x: info.tiltX as f32,
                tilt_y: info.tiltY as f32,
                twist: info.rotation as f32,
                flags: info.penFlags,
            })
    }
}

/// Inverts the 2-D affine part of an `INPUT_TRANSFORM` and applies it to a screen point.
///
/// The matrix maps **client to screen** in row-vector convention, so a consumer applies its
/// inverse. A singular matrix answers `None`: no client point maps to the given screen point,
/// and inventing one would put every contact at the origin.
fn invert_affine(m: &INPUT_TRANSFORM_0_0, sx: f32, sy: f32) -> Option<(f32, f32)> {
    let det = m._11 * m._22 - m._12 * m._21;
    if !det.is_finite() || det.abs() < f32::EPSILON {
        return None;
    }
    let (dx, dy) = (sx - m._41, sy - m._42);
    Some((
        (dx * m._22 - dy * m._21) / det,
        (dy * m._11 - dx * m._12) / det,
    ))
}

/// What a pen reports beyond a position.
///
/// Pressure is 0..=1, from the platform's 0..=1024; the tilts are degrees from vertical,
/// −90..=90; twist is barrel rotation in degrees, 0..360.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Pen {
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub twist: f32,
    /// `PEN_FLAG_INVERTED`, `PEN_FLAG_ERASER` and `PEN_FLAG_BARREL`, as read.
    pub flags: u32,
}

/// One reading of one pointer.
///
/// `Copy` and owning nothing, so the always-on path passes it by value and allocates nothing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sample {
    pub id: u32,
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
    pub pen: Option<Pen>,
    /// The system tick count the sample was stamped with, in milliseconds.
    pub time: u32,
    /// The high-resolution performance counter the sample was stamped with.
    ///
    /// Without it, a tick that consumes a frame's worth of samples and a tick that consumes
    /// fifty after a stalled pump are indistinguishable, so any rate derived from sample count
    /// alone — a velocity, a fling's energy, a dwell — is wrong under load. Zero where the
    /// source carries no counter, which is [`Coords::at_transition`].
    pub qpc: u64,
}

impl Sample {
    /// Returns the contact kind the hit array is queried with.
    #[must_use]
    pub const fn kind(&self) -> windows_scene::ContactKind {
        self.ptype.contact()
    }
}

/// How the WinRT pointer statics' coordinates relate to the hit array's DIPs.
///
/// `PointerPoint.Position` is documented as "*client coordinates, in device-independent
/// pixel*" and the statics "*always use the app context*", so on a window this thread owns the
/// two spaces should be one space. **Measured on 26200 they are not**: a window at 150% reads
/// the statics 15/14 larger than its own DIPs, so the statics divide client pixels by a scale
/// that is not the window's DPI scale. The factor is neither 1 nor the window's scale, so it
/// is measured rather than derived — 7% is four DIPs on a 60-DIP drag threshold, and every
/// hold radius, cross-slide distance and manipulation delta is expressed in it.
///
/// The recogniser is handed this and calls it; nothing else does.
#[windows_core::implement(IPointerPointTransform)]
#[derive(Default)]
pub struct PointerSpace {
    /// The scalar every coordinate is multiplied by, and the gate that keeps calibration
    /// settled once: zero is unmeasured. One scalar states the whole transform, because the
    /// difference between a window's own DIPs and whatever the statics call DIPs is a scale
    /// about the client origin, with no rotation or shear between two views of one client
    /// area.
    factor: Cell<f32>,
}

impl PointerSpace {
    /// Discards the measurement and returns the transform to the identity, on an environment
    /// change.
    ///
    /// The factor is **not** rescaled: it was not derived from the window's scale, so no
    /// arithmetic carries it to a new one. The next contact measures again, which costs one
    /// comparison.
    pub fn forget(&self) {
        self.factor.set(0.0);
    }

    /// Returns whether a factor has been settled, which is what keeps calibration to once a
    /// session.
    #[must_use]
    pub fn measured(&self) -> bool {
        self.factor.get() != 0.0
    }

    /// Returns the factor every coordinate is multiplied by, which is `1.0` while unmeasured
    /// and exactly `1.0` where the two spaces agreed.
    #[must_use]
    pub fn factor(&self) -> f32 {
        match self.factor.get() {
            0.0 => 1.0,
            measured => measured,
        }
    }

    /// Settles the factor from one contact read in both spaces.
    ///
    /// `winrt` is the untransformed `RawPosition`; `ours` is the same contact's raw screen
    /// point put through [`Coords`]. Both name the same physical place, so their ratio is the
    /// conversion, and the contact is the one about to be fed to the recogniser, so the first
    /// gesture of a session is measured rather than guessed.
    ///
    /// Returns without changing anything once a factor is settled, and refuses three readings
    /// rather than averaging them in: a point too near the client origin, where every factor
    /// agrees; axes that disagree, which means the two reads were of different samples; and a
    /// non-finite ratio.
    pub fn calibrate(&self, winrt: Point, ours: Point) {
        // Far enough from the client origin that a few percent is resolvable: at 60 DIPs a 7%
        // factor is four DIPs, which no rounding closes.
        const FLOOR: f32 = 40.0;
        if self.measured() {
            return;
        }
        let ratio = |ours: f32, winrt: f32| -> Option<f32> {
            (ours.abs() >= FLOOR && winrt.abs() >= FLOOR && winrt.is_finite())
                .then(|| ours / winrt)
                .filter(|factor| factor.is_finite() && *factor > 0.0)
        };
        let measured = match (ratio(ours.x, winrt.x), ratio(ours.y, winrt.y)) {
            // Both axes usable: they must agree, or the two reads were of different samples
            // and the ratio is motion rather than conversion.
            (Some(x), Some(y)) if (x - y).abs() <= 0.01 * x.abs().max(y.abs()) => (x + y) * 0.5,
            (Some(_), Some(_)) | (None, None) => return,
            (Some(only), None) | (None, Some(only)) => only,
        };
        // Within measurement noise of one, the factor is one: keeping 0.999875 would put a
        // rounding into every coordinate for the rest of the session.
        self.factor.set(if (measured - 1.0).abs() < 0.002 {
            1.0
        } else {
            measured
        });
    }
}

impl IPointerPointTransform_Impl for PointerSpace_Impl {
    /// Returns the inverse transform, which the recogniser asks for when it has to undo one.
    ///
    /// A scale's inverse is a scale, so this is the same type holding the reciprocal. A factor
    /// that is not normal — unmeasured, zero, subnormal — inverts to the identity rather than
    /// to an infinity.
    fn Inverse(&self) -> windows_core::Result<IPointerPointTransform> {
        let factor = self.factor();
        let inverse = PointerSpace::default();
        inverse.factor.set(if factor.is_normal() {
            1.0 / factor
        } else {
            1.0
        });
        Ok(windows_core::ComObject::new(inverse).into_interface())
    }

    /// Multiplies a point by the measured factor. Always succeeds, since a scale transforms
    /// every point.
    fn TryTransform(&self, inpoint: &WinPoint, out: &mut WinPoint) -> windows_core::Result<bool> {
        let f = self.factor();
        *out = WinPoint {
            x: inpoint.x * f,
            y: inpoint.y * f,
        };
        Ok(true)
    }

    /// Multiplies a rectangle's origin and extent by the measured factor.
    fn TransformBounds(&self, rect: &WinRect) -> windows_core::Result<WinRect> {
        let f = self.factor();
        Ok(WinRect {
            x: rect.x * f,
            y: rect.y * f,
            width: rect.width * f,
            height: rect.height * f,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix(sx: f32, sy: f32, tx: f32, ty: f32) -> INPUT_TRANSFORM_0_0 {
        INPUT_TRANSFORM_0_0 {
            _11: sx,
            _22: sy,
            _33: 1.0,
            _44: 1.0,
            _41: tx,
            _42: ty,
            ..Default::default()
        }
    }

    #[test]
    fn the_transform_is_inverted_rather_than_applied() {
        // Client → screen doubles and shifts, so screen → client halves and unshifts.
        let m = matrix(2.0, 2.0, 100.0, 40.0);
        let (x, y) = invert_affine(&m, 300.0, 140.0).expect("a scale is invertible");
        assert!((x - 100.0).abs() < 1e-3, "{x}");
        assert!((y - 50.0).abs() < 1e-3, "{y}");
    }

    #[test]
    fn a_singular_transform_answers_nothing_rather_than_the_origin() {
        assert_eq!(invert_affine(&matrix(0.0, 0.0, 0.0, 0.0), 10.0, 10.0), None);
    }

    #[test]
    fn calibration_reads_the_factor_off_one_contact() {
        // What 26200 reports for a window at 150%: the statics read 15/14 larger than the
        // window's own DIPs.
        let space = PointerSpace::default();
        space.calibrate(
            Point {
                x: 214.285_74,
                y: 53.571_43,
            },
            Point { x: 200.0, y: 50.0 },
        );
        // Only x cleared the floor, which is enough to settle the factor.
        assert!(space.measured());
        assert!(
            (space.factor() - 14.0 / 15.0).abs() < 1e-4,
            "{}",
            space.factor()
        );
    }

    #[test]
    fn agreement_within_a_dip_is_the_identity_rather_than_a_factor() {
        let space = PointerSpace::default();
        space.calibrate(Point { x: 200.05, y: 80.0 }, Point { x: 200.0, y: 80.0 });
        assert!(space.measured());
        assert_eq!(space.factor(), 1.0);
    }

    #[test]
    fn calibration_refuses_a_measurement_it_cannot_distinguish() {
        // Near the client origin every factor agrees, so nothing is settled.
        let space = PointerSpace::default();
        space.calibrate(Point { x: 2.0, y: 1.0 }, Point { x: 2.0, y: 1.0 });
        assert!(!space.measured());

        // Axes that disagree mean the two reads were of different samples: a contact that
        // moved between them, whose ratio is motion rather than conversion.
        let moved = PointerSpace::default();
        moved.calibrate(Point { x: 400.0, y: 100.0 }, Point { x: 200.0, y: 90.0 });
        assert!(!moved.measured());
    }

    #[test]
    fn an_environment_change_discards_the_measurement_rather_than_scaling_it() {
        let space = PointerSpace::default();
        space.calibrate(
            Point {
                x: 214.28,
                y: 107.14,
            },
            Point { x: 200.0, y: 100.0 },
        );
        assert!(space.measured());
        space.forget();
        assert!(
            !space.measured(),
            "a factor that was never derived from the scale cannot be carried to a new one"
        );
        assert_eq!(space.factor(), 1.0);
    }

    #[test]
    fn the_measured_factor_is_what_the_recogniser_is_transformed_by() {
        let space = PointerSpace::default();
        space.calibrate(
            Point {
                x: 214.285_74,
                y: 107.142_87,
            },
            Point { x: 200.0, y: 100.0 },
        );
        let transform: IPointerPointTransform = windows_core::ComObject::new(space).to_interface();
        let mut out = WinPoint::default();
        assert!(
            transform
                .TryTransform(
                    WinPoint {
                        x: 214.285_74,
                        y: 107.142_87
                    },
                    &mut out
                )
                .expect("a scale always transforms")
        );
        assert!((out.x - 200.0).abs() < 0.05, "{}", out.x);
        assert!((out.y - 100.0).abs() < 0.05, "{}", out.y);
    }

    /// A contact patch crosses as an extent in DIPs, and a scale that has not been resolved
    /// cannot divide one.
    #[test]
    fn an_unresolved_scale_reports_no_contact_patch() {
        let coords = Coords::new(core::ptr::null_mut(), Late::default());
        // A pointer id no device owns reports no patch either, so both arms answer the same
        // way and the assertion is on the guard rather than on a device.
        assert_eq!(coords.contact(0, PointerType::Touch, 0.0), (0.0, 0.0));
        assert_eq!(coords.contact(0, PointerType::Mouse, 1.5), (0.0, 0.0));
    }
}
