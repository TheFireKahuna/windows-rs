//! Coordinate conversion for pointer input.
//!
//! `POINTER_INFO.ptPixelLocation` and `ptPixelLocationRaw` are **screen physical**. The hit
//! array is built in **client DIPs**. This module crosses that gap in one place, so the
//! pointer, the wheel, the caption band, focus order and automation all resolve through one
//! conversion rather than through two that can disagree.
//!
//! The screen→client half is `GetPointerInputTransform` where the input carries a transform
//! and `ScreenToClient` where it does not, which is the algorithm the platform documents: a
//! consumer "*typically uses `ScreenToClient` … If a transform is applied on the message
//! consumer, use `GetPointerInputTransform`*", and the latter fails with `ERROR_NO_DATA` when
//! there is no transform. The pixel→DIP half is the window's own scale, which
//! `windows-window` resolves and this crate never re-derives.

use crate::bindings::{Point as WinPoint, Rect as WinRect, *};
use core::cell::Cell;
use windows_scene::{Env, Point};

/// Converts screen physical pixels to client DIPs for one window.
///
/// The scale is passed in at every call and never held: a cached scale is not updated when
/// the window moves to a display with a different one, and every contact then resolves
/// against the wrong pixel grid. The window handle is not a display fact, so it is held.
/// [`Env::scale`] is the only derivation of the factor, so `dpi / 96` computed here would be
/// a second one.
#[derive(Copy, Clone, Debug)]
pub struct Coords {
    hwnd: HWND,
}

impl Coords {
    /// Creates a conversion for `hwnd`.
    #[must_use]
    pub const fn new(hwnd: HWND) -> Self {
        Self { hwnd }
    }

    /// Converts the screen physical point `(x_px, y_px)` to the client DIPs the hit array is
    /// built in.
    ///
    /// `id` selects the transform *that pointer's* input carries, which keeps the answer
    /// right for a window whose content the system is scaling.
    #[must_use]
    pub fn client(&self, env: Env, id: u32, x_px: i32, y_px: i32) -> Point {
        let (x, y) = self
            .transform(id, x_px, y_px)
            .unwrap_or_else(|| self.screen_to_client(x_px, y_px));
        let scale = env.scale();
        Point {
            x: x / scale,
            y: y / scale,
        }
    }

    /// Applies the inverse of the consumer transform to a screen point, in client pixels.
    ///
    /// `None` when the input carries no transform, which is the ordinary case for a
    /// per-monitor-v2 window nothing is scaling.
    fn transform(&self, id: u32, x_px: i32, y_px: i32) -> Option<(f32, f32)> {
        let mut transform = INPUT_TRANSFORM::default();
        // SAFETY: `transform` is a stack local of the type the call writes, and the count of
        // one matches the single entry a non-history reading asks for.
        if !unsafe { GetPointerInputTransform(id, 1, &mut transform) }.as_bool() {
            return None;
        }
        // SAFETY: both arms of the union are views of the same sixteen floats, so the named
        // arm reads initialised data whichever arm the call wrote through.
        let m = unsafe { transform.Anonymous.Anonymous };
        invert_affine(&m, x_px as f32, y_px as f32)
    }

    /// Converts a screen point to client pixels through `ScreenToClient`, the conversion the
    /// platform documents for input carrying no transform.
    fn screen_to_client(&self, x_px: i32, y_px: i32) -> (f32, f32) {
        let mut point = POINT { x: x_px, y: y_px };
        // SAFETY: `hwnd` is live for the call and `point` is a stack local the call writes
        // back through. A failure leaves it holding the screen point, which is wrong by the
        // client origin and the only answer available once the window has gone.
        unsafe {
            _ = ScreenToClient(self.hwnd, &mut point);
        }
        (point.x as f32, point.y as f32)
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

/// States how the WinRT pointer statics' coordinates relate to the hit array's DIPs.
///
/// `PointerPoint.Position` is documented as "*client coordinates, in device-independent
/// pixel*" and the statics "*always use the app context*", so on a window this thread owns
/// the two spaces should be one space. **Measured on 26200 they are not**: a window at 150%
/// reads the statics 15/14 larger than its own DIPs, so the statics divide client pixels by a
/// scale that is not the window's DPI scale. The factor is neither 1 nor the window's scale,
/// so it is measured rather than derived — 7% is four DIPs on a 60-DIP drag threshold, and
/// every hold radius, cross-slide distance and manipulation delta is expressed in it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Unit {
    /// No contact has been read in both spaces yet. The transform is the identity until one
    /// is.
    Unmeasured,
    /// The statics agree with the hit array to within a DIP.
    Identity,
    /// The two spaces differ by this uniform factor, which is what the transform applies.
    Scaled(f32),
}

/// Transforms the WinRT pointer statics' coordinates into the hit array's DIPs.
///
/// The recogniser is handed this and calls it; nothing else does. It puts a hold radius, a
/// cross-slide threshold and a manipulation delta in the units the layout solved in, so a
/// gesture and a hit agree by construction rather than through two conversions kept equal by
/// hand.
#[windows_core::implement(IPointerPointTransform)]
pub struct PointerSpace {
    /// The scalar every coordinate is multiplied by. One scalar states the whole transform:
    /// the difference between a window's own DIPs and whatever the statics call DIPs is a
    /// scale about the client origin, with no rotation or shear between two views of one
    /// client area.
    factor: Cell<f32>,
    /// What [`PointerSpace::calibrate`] settled, and the gate that keeps it settled once.
    unit: Cell<Unit>,
}

impl Default for PointerSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerSpace {
    /// Creates an unmeasured transform, which is the identity until
    /// [`calibrate`](Self::calibrate) settles a factor.
    ///
    /// It takes no scale: the factor is measurably *not* the window's scale, so there is no
    /// scale a caller could usefully supply.
    #[must_use]
    pub fn new() -> Self {
        Self {
            factor: Cell::new(1.0),
            unit: Cell::new(Unit::Unmeasured),
        }
    }

    /// Discards the measurement and returns the transform to the identity. Called on an
    /// environment change.
    ///
    /// The factor is **not** rescaled: it was not derived from the window's scale, so no
    /// arithmetic carries it to a new one. The next contact measures again, which costs one
    /// comparison.
    pub fn forget(&self) {
        self.factor.set(1.0);
        self.unit.set(Unit::Unmeasured);
    }

    /// Returns what calibration settled.
    #[must_use]
    pub fn unit(&self) -> Unit {
        self.unit.get()
    }

    /// Returns the factor every coordinate is multiplied by, which is `1.0` while unmeasured.
    #[must_use]
    pub fn factor(&self) -> f32 {
        self.factor.get()
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
        if self.unit.get() != Unit::Unmeasured {
            return;
        }
        // Far enough from the client origin that a few percent is resolvable: at 60 DIPs a
        // 7% factor is four DIPs, which no rounding closes.
        const FLOOR: f32 = 40.0;
        let ratio = |ours: f32, winrt: f32| -> Option<f32> {
            (winrt.abs() >= FLOOR && ours.abs() >= FLOOR && winrt.is_finite())
                .then(|| ours / winrt)
                .filter(|factor| factor.is_finite() && *factor > 0.0)
        };
        let factor = match (ratio(ours.x, winrt.x), ratio(ours.y, winrt.y)) {
            // Both axes usable: they must agree, or the two reads were of different samples
            // and the ratio is motion rather than conversion.
            (Some(x), Some(y)) if (x - y).abs() <= 0.01 * x.abs().max(y.abs()) => (x + y) * 0.5,
            (Some(_), Some(_)) => return,
            (Some(x), None) => x,
            (None, Some(y)) => y,
            (None, None) => return,
        };
        // Within measurement noise of one, the factor is one: keeping 0.999875 would put a
        // rounding into every coordinate for the rest of the session.
        if (factor - 1.0).abs() < 0.002 {
            self.factor.set(1.0);
            self.unit.set(Unit::Identity);
        } else {
            self.factor.set(factor);
            self.unit.set(Unit::Scaled(factor));
        }
    }
}

impl IPointerPointTransform_Impl for PointerSpace_Impl {
    /// Returns the inverse transform, which the recogniser asks for when it has to undo one.
    ///
    /// A scale's inverse is a scale, so this is the same type holding the reciprocal. A
    /// factor that is not normal — unmeasured, zero, subnormal — inverts to the identity
    /// rather than to an infinity.
    fn Inverse(&self) -> windows_core::Result<IPointerPointTransform> {
        let factor = self.factor.get();
        let inverse = PointerSpace {
            factor: Cell::new(if factor.is_normal() {
                1.0 / factor
            } else {
                1.0
            }),
            unit: Cell::new(self.unit.get()),
        };
        Ok(windows_core::ComObject::new(inverse).into_interface())
    }

    /// Multiplies a point by the measured factor. Always succeeds, since a scale transforms
    /// every point.
    fn TryTransform(
        &self,
        inpoint: &WinPoint,
        outpoint: &mut WinPoint,
    ) -> windows_core::Result<bool> {
        let factor = self.factor.get();
        *outpoint = WinPoint {
            x: inpoint.x * factor,
            y: inpoint.y * factor,
        };
        Ok(true)
    }

    /// Multiplies a rectangle's origin and extent by the measured factor.
    fn TransformBounds(&self, rect: &WinRect) -> windows_core::Result<WinRect> {
        let factor = self.factor.get();
        Ok(WinRect {
            x: rect.x * factor,
            y: rect.y * factor,
            width: rect.width * factor,
            height: rect.height * factor,
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
        let space = PointerSpace::new();
        space.calibrate(
            Point {
                x: 214.285_74,
                y: 53.571_43,
            },
            Point { x: 200.0, y: 50.0 },
        );
        // Only x cleared the floor, which is enough to settle the factor.
        match space.unit() {
            Unit::Scaled(factor) => assert!((factor - 14.0 / 15.0).abs() < 1e-4, "{factor}"),
            other => panic!("a 7% difference read as {other:?}"),
        }
    }

    #[test]
    fn agreement_within_a_dip_is_the_identity_rather_than_a_factor() {
        let space = PointerSpace::new();
        space.calibrate(Point { x: 200.05, y: 80.0 }, Point { x: 200.0, y: 80.0 });
        assert_eq!(space.unit(), Unit::Identity);
        assert_eq!(space.factor(), 1.0);
    }

    #[test]
    fn calibration_refuses_a_measurement_it_cannot_distinguish() {
        // Near the client origin every factor agrees, so nothing is settled.
        let space = PointerSpace::new();
        space.calibrate(Point { x: 2.0, y: 1.0 }, Point { x: 2.0, y: 1.0 });
        assert_eq!(space.unit(), Unit::Unmeasured);

        // Axes that disagree mean the two reads were of different samples: a contact that
        // moved between them, whose ratio is motion rather than conversion.
        let moved = PointerSpace::new();
        moved.calibrate(Point { x: 400.0, y: 100.0 }, Point { x: 200.0, y: 90.0 });
        assert_eq!(moved.unit(), Unit::Unmeasured);
    }

    #[test]
    fn an_environment_change_discards_the_measurement_rather_than_scaling_it() {
        let space = PointerSpace::new();
        space.calibrate(
            Point {
                x: 214.28,
                y: 107.14,
            },
            Point { x: 200.0, y: 100.0 },
        );
        assert!(matches!(space.unit(), Unit::Scaled(_)));
        space.forget();
        assert_eq!(
            space.unit(),
            Unit::Unmeasured,
            "a factor that was never derived from the scale cannot be carried to a new one"
        );
        assert_eq!(space.factor(), 1.0);
    }

    #[test]
    fn the_measured_factor_is_what_the_recogniser_is_transformed_by() {
        let space = PointerSpace::new();
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
}
