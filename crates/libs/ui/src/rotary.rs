//! Reads rotary input — Surface Dial and compatible wheels — through `RadialController`.
//!
//! A desktop application reaches the device through `IRadialControllerInterop::CreateForWindow`.
//! It is HWND-based, so neither a `CoreWindow` nor the App SDK is involved. `CreateForWindow`
//! succeeds with **no dial attached**, so holding a controller is not evidence of a device.
//!
//! The dial is a delta source, so it lands on the gesture seam and drives the value path a
//! knob drag uses rather than the pointer seam. A control that declares no
//! [`RotaryDecl`](crate::gesture::RotaryDecl) is not a dial target.
//!
//! `ScreenContactStarted` carries a position, so a dial resting on screen picks its target out
//! of the same flat hit array a finger does. That position is what the turn and the press
//! carry here; the three screen-contact events themselves are not wired, because a contact
//! that arrives and moves without turning or pressing changes nothing the router can act on.

use crate::bindings::*;
use crate::gesture::RotaryDecl;
use crate::input::Service;
use core::cell::{Cell, RefCell};
use std::rc::Rc;
use windows_core::{EventRevoker, Interface, Result};
use windows_scene::Point;

/// What the dial did.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Rotation {
    /// Turned. `degrees` is signed; clockwise is positive.
    Turned {
        degrees: f64,
        /// Where the dial is resting on screen, in client DIPs, if it is on screen at all.
        at: Option<Point>,
    },
    /// Pressed or released. The press carries no contact: `ButtonPressed` and `ButtonReleased`
    /// live on `IRadialController2` and their argument has no screen contact on it.
    Button { pressed: bool },
    /// Pressed and released together, with nothing in between.
    Clicked { at: Option<Point> },
}

/// The queue, the tick gate and the scale, shared with every handler.
#[derive(Default)]
struct Sink {
    queue: RefCell<Vec<Rotation>>,
    /// Asks for a tick as each event arrives rather than waiting on the display clock. The
    /// gate is the window's, shared with the doorbell, so a dial turned during a drag costs no
    /// second post.
    service: Rc<Service>,
    /// The window's DPI scale, which a screen-space contact position is divided by. Shared
    /// rather than copied into each handler, so [`Rotary::rescale`] reaches a contact already
    /// on screen.
    scale: Cell<f32>,
}

impl Sink {
    /// Queues one event and asks for a tick.
    ///
    /// A queue that cannot be borrowed — one re-entering from another callback — takes
    /// nothing, and the tick is still asked for.
    fn push(&self, event: Rotation) {
        if let Ok(mut queue) = self.queue.try_borrow_mut() {
            queue.push(event);
        }
        self.service.now();
    }

    /// Converts a dial's on-screen contact position into client DIPs, so a caller resolves it
    /// through the hit array exactly as it would a finger.
    ///
    /// `None` where there is no contact, the position cannot be read, or the scale is not
    /// positive.
    fn at(&self, contact: Option<RadialControllerScreenContact>) -> Option<Point> {
        let scale = self.scale.get();
        let position = contact?.Position().ok()?;
        (scale > 0.0).then(|| Point {
            x: position.x / scale,
            y: position.y / scale,
        })
    }
}

/// Registers one handler that queues a [`Rotation`] and yields the revoker that unsubscribes
/// it.
macro_rules! raises {
    ($src:expr, $sink:ident, $event:ident, |$args:pat_param| $make:expr) => {{
        // The clone is bound to the caller's own name, so the expression that builds the
        // event reaches the clone the closure owns rather than the one outside it.
        let $sink = $sink.clone();
        $src.$event(move |_, args| {
            let $args = args.as_ref();
            $sink.push($make);
        })?
    }};
}

/// Owns the window's radial controller and the events it raises.
///
/// One per window, held for the window's life. [`Rotary::tune`] restates resolution and
/// haptics per focused target, so the detents match that target's declared step.
pub struct Rotary {
    controller: RadialController,
    sink: Rc<Sink>,
    /// Held for their `Drop`, which revokes.
    _revokers: Vec<EventRevoker>,
}

impl Rotary {
    /// Creates the controller for `window` and registers the handlers that fill its queue.
    ///
    /// # Errors
    ///
    /// The window is closed, the interop factory refuses the window, or an event registration
    /// fails.
    pub fn new(window: &windows_window::Window, service: Rc<Service>) -> Result<Self> {
        let scale = window.scale().ok_or_else(|| {
            windows_core::Error::new(
                windows_core::HRESULT(0x8007_0006_u32 as i32),
                "the window is closed",
            )
        })?;
        let interop = windows_core::factory::<RadialController, IRadialControllerInterop>()?;
        // SAFETY: `hwnd` belongs to `window`, borrowed for the whole call, so the handle
        // cannot be destroyed under it; `RadialController` is the class `CreateForWindow`
        // returns, so the requested interface is one the object implements.
        let controller: RadialController = unsafe { interop.CreateForWindow(window.hwnd())? };
        let sink = Rc::new(Sink {
            scale: Cell::new(scale),
            service,
            ..Sink::default()
        });
        // `ButtonPressed` and `ButtonReleased` live on `IRadialController2` rather than the
        // class's default interface, so they are reached by cast.
        let buttons: IRadialController2 = controller.cast()?;
        let revokers = vec![
            raises!(controller, sink, RotationChanged, |args| Rotation::Turned {
                degrees: args
                    .and_then(|a| a.RotationDeltaInDegrees().ok())
                    .unwrap_or(0.0),
                at: sink.at(args.and_then(|a| a.Contact().ok())),
            }),
            raises!(controller, sink, ButtonClicked, |args| Rotation::Clicked {
                at: sink.at(args.and_then(|a| a.Contact().ok())),
            }),
            raises!(buttons, sink, ButtonPressed, |_| Rotation::Button {
                pressed: true
            }),
            raises!(buttons, sink, ButtonReleased, |_| Rotation::Button {
                pressed: false
            }),
        ];
        Ok(Self {
            controller,
            sink,
            _revokers: revokers,
        })
    }

    /// Takes everything raised since the last call, leaving the queue empty.
    ///
    /// A call that cannot borrow the queue — one re-entering from an event callback — takes
    /// nothing.
    pub fn drain(&self, out: &mut Vec<Rotation>) {
        if let Ok(mut queue) = self.sink.queue.try_borrow_mut() {
            out.append(&mut queue);
        }
    }

    /// Restates the window's scale after a DPI change.
    pub fn rescale(&self, scale: f32) {
        self.sink.scale.set(scale);
    }

    /// Matches the dial's detents to the step `decl` declares, so each detent the user feels
    /// advances the target by one step. Applies to the whole controller until the next call.
    ///
    /// # Errors
    ///
    /// The controller rejects the resolution or the feedback setting.
    pub fn tune(&self, decl: &RotaryDecl) -> Result<()> {
        self.controller
            .SetRotationResolutionInDegrees(decl.resolution_degrees)?;
        self.controller.SetUseAutomaticHapticFeedback(decl.haptics)
    }
}
