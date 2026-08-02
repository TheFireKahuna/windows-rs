//! Where a point lands.
//!
//! Every stream except the touchpad states its points in the target window's **client
//! DIPs**, because that is the space a laid-out target is in and therefore the space an
//! assertion is written in. A harness that took screen pixels would make every test do the
//! conversion, and a test that does its own conversion is a test that can disagree with the
//! window about where it aimed.

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
    /// A point, in client DIPs.
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

/// A straight path of `samples` points from `from` to `to`, `from` excluded.
///
/// Here rather than in each test because a drag assertion is written on the **integral**,
/// and an integral is only comparable against a path whose length every caller computes the
/// same way. The start is excluded because the contact is already there — a drag begins
/// with a press at `from`, and repeating it would inject a sample that travels nothing and,
/// for a mouse, is not input at all.
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

/// A saw-toothed path from `from` to `to`, `from` excluded, deviating `amplitude` DIPs to
/// alternating sides of the straight line between them.
///
/// **This, not [`line`], is what a drag-fidelity assertion must be driven with**, and the
/// reason is a flaw a straight path hides completely: the length of a polyline through
/// collinear points does not change when one of them is removed. A drive of 40 collinear
/// samples measures the same total whether the stack saw all forty or only the two ends, so a
/// straight path proves the endpoints were placed correctly and proves nothing at all about
/// dropped samples. Every point here is a corner, so losing any one of them cuts it and
/// shortens the integral.
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
/// Both are read from the window at a stated moment and then held for the streams opened
/// against them. That is deliberate — reading them per sample would be two syscalls per
/// sample on the path whose whole purpose is to be exact about timing — and it is why
/// [`Injector::retarget`](crate::Injector::retarget) exists: a window that moves or changes
/// DPI restates them, rather than the harness silently drifting from the window it aims at.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Space {
    origin_px: (i32, i32),
    scale: f32,
}

impl Space {
    /// The client area of `hwnd`.
    // A window handle is a raw pointer only by binding convention; it is never dereferenced
    // here. Both calls below validate it themselves and say so — `ClientToScreen` answers
    // `FALSE` and `GetDpiForWindow` answers zero — and both answers are handled, so an
    // `unsafe fn` would be asking every test to promise something the platform already
    // checks.
    #[expect(clippy::not_unsafe_ptr_arg_deref)]
    pub fn for_window(hwnd: *mut core::ffi::c_void) -> Result<Self> {
        let mut point = POINT::default();
        // SAFETY: `hwnd` is the caller's live window and the point is a stack local the
        // call writes back through.
        unsafe { ClientToScreen(hwnd, &mut point) }
            .ok()
            .map_err(|e| Error::call("ClientToScreen", e))?;
        // SAFETY: `hwnd` is live; the call reads it and returns a number.
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        // A zero answer means the handle was not a window. Treating it as 96 would put
        // every sample on the wrong grid and look exactly like a hit-test bug.
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

    /// An explicit origin and scale, for a target that is not a window of this process.
    #[must_use]
    pub const fn new(origin_px: (i32, i32), scale: f32) -> Self {
        Self { origin_px, scale }
    }

    /// The client origin, in screen physical pixels.
    #[must_use]
    pub const fn origin_px(&self) -> (i32, i32) {
        self.origin_px
    }

    /// DIPs to physical pixels.
    #[must_use]
    pub const fn scale(&self) -> f32 {
        self.scale
    }

    /// The length of the path these points describe **as this space will place them**.
    ///
    /// Not the ideal length, and the difference is not rounding noise a tolerance should
    /// absorb: a sample is placed on a whole screen pixel, so a path stated in DIPs is
    /// quantised before it is injected and its real length is the quantised one. Measured on a
    /// 150% display, a 30-segment zigzag loses about 8 DIPs of its 393 that way — eight times
    /// the margin a fidelity assertion wants to be sensitive to. Comparing what arrived
    /// against the *ideal* length therefore reports a dropped sample on every run.
    #[must_use]
    pub fn placed_length(&self, points: &[Point]) -> f32 {
        let placed: Vec<(i32, i32)> = points.iter().map(|point| self.to_px(*point)).collect();
        placed
            .windows(2)
            .map(|pair| ((pair[1].0 - pair[0].0) as f32).hypot((pair[1].1 - pair[0].1) as f32))
            .sum::<f32>()
            / self.scale
    }

    /// The screen physical pixel a client point lands on.
    ///
    /// Rounded, not truncated: truncation biases every sample half a pixel toward the
    /// origin, which on a 150% display is a third of a DIP applied consistently in one
    /// direction — the shape of an error that looks like a systematic offset in the stack
    /// under test.
    #[must_use]
    pub fn to_px(&self, point: impl Into<Point>) -> (i32, i32) {
        let point = point.into();
        (
            self.origin_px.0 + (point.x * self.scale).round() as i32,
            self.origin_px.1 + (point.y * self.scale).round() as i32,
        )
    }
}
