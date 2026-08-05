//! Where a point lands: the conversion from client DIPs to screen pixels, and the paths a
//! drive is built from.
//!
//! Every stream except the touchpad states its points in the target window's client DIPs,
//! which is the space a laid-out target occupies and the space an assertion is written in.
//! The conversion to screen pixels happens here, once, so no test does its own.

use crate::bindings::*;
use crate::{Error, Result};

/// A point in the target window's client area, in DIPs.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Point {
    /// Distance right of the client origin.
    pub x: f32,
    /// Distance below the client origin.
    pub y: f32,
}

impl Point {
    /// Returns the client-DIP point `(x, y)`.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<(f32, f32)> for Point {
    fn from((x, y): (f32, f32)) -> Self {
        Self { x, y }
    }
}

impl From<Point> for (f32, f32) {
    fn from(point: Point) -> Self {
        (point.x, point.y)
    }
}

/// Returns a straight path of `samples` points from `from` to `to`, `from` excluded.
///
/// The start is excluded because the contact is already there: a drag begins with a press at
/// `from`, and repeating it would inject a sample that travels nothing — which for a mouse is
/// not input at all.
#[must_use]
pub fn line(from: impl Into<Point>, to: impl Into<Point>, samples: usize) -> Vec<Point> {
    let (from, to) = (from.into(), to.into());
    (1..=samples)
        .map(|step| {
            let t = step as f32 / samples as f32;
            Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
        })
        .collect()
}

/// Returns a saw-toothed path from `from` to `to`, `from` excluded, deviating `amplitude`
/// DIPs to alternating sides of the straight line between them.
///
/// A drag-fidelity assertion is driven with this rather than [`line`]: the length of a
/// polyline through collinear points does not change when one of them is removed, so a
/// straight drive of 40 samples measures the same total whether the stack saw all forty or
/// only the two ends. Every point here is a corner, so losing one cuts it and shortens the
/// integral.
#[must_use]
pub fn zigzag(
    from: impl Into<Point>,
    to: impl Into<Point>,
    samples: usize,
    amplitude: f32,
) -> Vec<Point> {
    let (from, to) = (from.into(), to.into());
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    // The unit normal to the run, so the deviation is across the path whichever way it points.
    let (nx, ny) = if length > 0.0 {
        (-dy / length, dx / length)
    } else {
        (0.0, 0.0)
    };
    (1..=samples)
        .map(|step| {
            let t = step as f32 / samples as f32;
            // The last sample lands on the line, so the path ends where the caller said.
            let side = if step == samples {
                0.0
            } else if step % 2 == 0 {
                amplitude
            } else {
                -amplitude
            };
            Point::new(from.x + dx * t + nx * side, from.y + dy * t + ny * side)
        })
        .collect()
}

/// The conversion from client DIPs to screen physical pixels: an origin and a scale.
///
/// Both are read from the window once and then held for the streams opened against them,
/// because re-reading them would cost two syscalls per sample on a path that has to be exact
/// about timing. A window that moves or changes DPI restates them through
/// [`Injector::retarget`](crate::Injector::retarget); until then the held pair is what every
/// sample is placed against.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Space {
    origin_px: (i32, i32),
    scale: f32,
}

impl Space {
    /// Reads `hwnd`'s client origin and scale.
    // `hwnd` is a raw pointer by binding convention and is never dereferenced here. Both
    // calls below validate the handle themselves — `ClientToScreen` returns `FALSE` and
    // `GetDpiForWindow` returns zero — and both answers are handled, so this function has no
    // precondition an `unsafe fn` would state.
    #[expect(clippy::not_unsafe_ptr_arg_deref)]
    pub fn for_window(hwnd: *mut core::ffi::c_void) -> Result<Self> {
        let mut point = POINT::default();
        // SAFETY: the call validates `hwnd` itself and returns `FALSE` for a handle that is
        // not a window; `point` is a stack local it writes back through.
        unsafe { ClientToScreen(hwnd, &mut point) }
            .ok()
            .map_err(|e| Error::call("ClientToScreen", e))?;
        // SAFETY: the call validates `hwnd` itself and returns zero for a handle that is not
        // a window; nothing is dereferenced on this side.
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        // Zero means the handle was not a window. Defaulting to 96 would place every sample
        // on the wrong grid, which reads as a hit-test bug in the stack under test.
        if dpi == 0 {
            return Err(Error::call(
                "GetDpiForWindow",
                windows_core::Error::from_thread(),
            ));
        }
        Ok(Self {
            origin_px: (point.x, point.y),
            scale: dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32,
        })
    }

    /// Returns a space with an explicit origin and scale, for a target that is not a window
    /// of this process.
    #[must_use]
    pub const fn new(origin_px: (i32, i32), scale: f32) -> Self {
        Self { origin_px, scale }
    }

    /// Returns the client origin, in screen physical pixels.
    #[must_use]
    pub const fn origin_px(&self) -> (i32, i32) {
        self.origin_px
    }

    /// Returns the factor converting DIPs to physical pixels.
    #[must_use]
    pub const fn scale(&self) -> f32 {
        self.scale
    }

    /// Returns the length of the path `points` describes as this space places them, in DIPs.
    ///
    /// This is the quantised length, not the ideal one: a sample is placed on a whole screen
    /// pixel, so a path stated in DIPs is quantised before it is injected. Measured on a 150%
    /// display, a 30-segment zigzag loses about 8 DIPs of its 393 that way, eight times the
    /// margin a fidelity assertion is sensitive to — so comparing arrivals against the ideal
    /// length reports a dropped sample on every run.
    #[must_use]
    pub fn placed_length(&self, points: &[Point]) -> f32 {
        let placed: Vec<(i32, i32)> = points.iter().map(|point| self.to_px(*point)).collect();
        placed
            .windows(2)
            .map(|pair| ((pair[1].0 - pair[0].0) as f32).hypot((pair[1].1 - pair[0].1) as f32))
            .sum::<f32>()
            / self.scale
    }

    /// Returns the screen physical pixel a client point lands on.
    ///
    /// Rounded, not truncated: truncation biases every sample half a pixel toward the origin,
    /// which on a 150% display is a third of a DIP applied consistently in one direction and
    /// reads as a systematic offset in the stack under test.
    #[must_use]
    pub fn to_px(&self, point: impl Into<Point>) -> (i32, i32) {
        let point = point.into();
        (
            self.origin_px.0 + (point.x * self.scale).round() as i32,
            self.origin_px.1 + (point.y * self.scale).round() as i32,
        )
    }
}
