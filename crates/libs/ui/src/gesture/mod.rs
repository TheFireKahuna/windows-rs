//! Gesture recognition, and the seam it crosses on.
//!
//! Two recognisers, one downstream. Both emit the same `Windows.UI.Input` event argument
//! types, so the sink is one code path regardless of device.
//!
//! # The declarative seam
//!
//! Gesture interest is declared per node and is **front-resident**: no call is made to the
//! application thread to decide whether a gesture applies, because that decision has to be
//! made between a contact arriving and its pixels moving. Recognised gestures then cross the
//! seam as [`Intent`]s — plain data, *after* the pixels have already moved.
//!
//! # The allocation rule, which is load-bearing
//!
//! `PointerPoint` is a WinRT object **per sample**. Therefore **hover never touches the
//! recogniser path**: it stays on the raw Win32 history with zero allocation, and WinRT
//! points are constructed only between down and up, for the one contact being manipulated.
//! That keeps the zero-allocation rule intact where it matters — the always-on path — and
//! confines allocation to a bounded, user-initiated interval. The confinement is structural:
//! nothing in [`input::sample`](crate::input::Sample) can reach this module.

mod decl;
mod drag;
mod pool;
mod recognizer;

pub use decl::{
    AxisLock, Commit, DragAxes, DragDecl, GestureDecl, HoldTuning, PivotDecl, RotaryDecl,
    TouchTargetDecl,
};
pub use drag::{Axis, Drag, DragPhase, DragUpdate};
pub use pool::{Bound, RecognizerPool};
pub use recognizer::{Events, Manip, Recognised, Recognizer, Velocities};

use windows_scene::{ControlId, Point};

/// A post-visual notification. **NO INTENT MAY BE THE CAUSE OF A VISUAL.**
///
/// An intent is what the application thread learns, on its own schedule, about something
/// that has *already* happened on screen. A slider snaps its thumb inside the front-side
/// handler and *then* queues [`Intent::Value`] — which is what makes the app-thread hop
/// harmless, because a busy application thread can no longer stall a gesture.
///
/// Constructed by the widget layer from a [`Report`](crate::input::Report), never by the
/// router: the router reports what the input was, and only a widget knows what it meant.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Intent {
    Tapped {
        id: ControlId,
    },
    RightTapped {
        id: ControlId,
        at: Point,
    },
    Held {
        id: ControlId,
        at: Point,
    },
    /// A slider or knob **has already moved its thumb**.
    Value {
        id: ControlId,
        v: f64,
    },
    Manipulated {
        id: ControlId,
        delta: Manip,
    },
    /// Coalesced: "your gesture has news". For a consumer that re-reads state rather than
    /// integrating a stream of deltas.
    Gesture {
        id: ControlId,
    },
    Scrolled {
        id: ControlId,
        y: f32,
    },
}
