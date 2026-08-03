//! The front thread's half of a control: what a [`Report`] does to pixels, before anything
//! is queued to the application.
//!
//! Invariant: **no intent may be the cause of a visual.** For a retained sink that means the
//! pixels move front-side, in the tick that saw the event, and the application hears about
//! it afterwards — so by the time an [`Intent`] exists the visual has already happened. That
//! ordering is what makes the rule structural rather than a thing to remember.
//!
//! The whole path is index arithmetic. A hover is a rect test the router already did, an
//! array index, and one retarget: **no hash lookup, no allocation, no `resolve`, and no
//! app-thread hop.** The wash opacities are resolved at mount, on the app thread, and shipped
//! as numbers, because realizing a new colour cell mid-hover would be a surface creation on
//! the interaction path.

use super::{Interaction, Range, TURN_SPAN, angle_of, detent_delta, fraction_of, offset_of};
use crate::input::Report;
use windows_scene::{
    Anim, Backends, Bind, Control, ControlId, Env, NodeId, Prop, Result, Scene, Slots, SpriteId,
    Tuning, Value,
};

/// The front half's write side.
///
/// One argument because every mover needs all three and none of them means anything without
/// the others — and because `env` is **stated rather than held**, for the reason every entry
/// point into the scene states it: a scene that cached the display could be not told when the
/// window hops one.
pub struct Front<'a> {
    pub scene: &'a mut Scene,
    pub back: &'a Backends,
    pub env: Env,
}

impl Front<'_> {
    fn retarget(&mut self, node: NodeId, prop: Prop, bind: Bind) -> Result<()> {
        self.scene.retarget(node, prop, bind, self.back, self.env)
    }
}

/// One control, as the front thread needs it.
///
/// Everything here is a number or an id. Nothing is a role, a colour or a closure — those
/// stay on the thread that can resolve and call them.
#[derive(Copy, Clone, Debug)]
pub struct ChromeRow {
    pub id: ControlId,
    /// The sprite whose opacity hover and press ride. `None` for a control with no wash.
    pub wash: Option<SpriteId>,
    /// Resolved wash opacities.
    pub hover: f32,
    pub press: f32,
    /// The part a value moves, and the room the last solve measured for it. Both, because
    /// moving it is one multiply and the router must never ask the app thread for geometry.
    pub thumb: Option<NodeId>,
    pub travel: f32,
    /// What a pointer means here. `None` is a press and nothing else.
    pub drive: Option<Interaction>,
    /// Where this control's value stands, `0..=1`.
    ///
    /// Seeded by the mount and advanced here, because a **turned** control has no absolute
    /// position to read off a pointer: a drag reports displacement from its origin and a dial
    /// reports detents, so the front thread is the only place its value can accumulate.
    pub fraction: f32,
}

/// What the application is asked to do, after the pixels have already moved.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Intent {
    pub target: ControlId,
    pub what: What,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum What {
    /// A press and a release on the same control.
    Tapped,
    /// A value while it is being moved.
    Changed(f64),
    /// The value it settled on. A canceled contact commits nothing.
    Committed(f64),
}

/// The front thread's control table and the interaction state over it.
///
/// Dense and generational: a control's index is its slot for the life of its mount, and the
/// generation is what turns a stale report — one about a control that has since unmounted —
/// into a miss rather than into a write to whatever now occupies the slot.
#[derive(Default)]
pub struct Controls {
    /// The front thread's store over the control family the app thread mints. **No `Ids`
    /// beside it**, and that is the whole statement: this side owns no counter, so it can
    /// place a row and never mint one.
    rows: Slots<Control, ChromeRow>,
    hovered: Option<ControlId>,
    pressed: Option<ControlId>,
    /// The **window's** focus ring: one visual, sprung between controls. Focus is singular
    /// by definition, so a ring per control would be eighty visuals to express one fact —
    /// and the glide between them would be a behaviour to implement rather than a
    /// compositor animation to start.
    ring: Option<NodeId>,
    /// Whether the ring is showing. Keyboard focus shows it; a pointer interaction hides
    /// it, which is the input-mode rule and not a widget decision.
    ring_shown: bool,
    /// The control being turned, and the fraction it stood at when the contact landed.
    ///
    /// A turn is a **displacement from where the contact started**, so the origin is what the
    /// answer is relative to. Accumulating per-sample deltas instead would drift by exactly
    /// the samples the recogniser coalesced — and it is also what lets a cancel put the value
    /// back, which is the drag policy's rule rather than this table's.
    grabbed: Option<(ControlId, f32)>,
}

impl Controls {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adopts the rows a mount produced, or a solve corrected. Called with whatever the app
    /// thread drained alongside its patch.
    ///
    /// A re-adopted row carries corrected **geometry**, never a corrected value: for the same
    /// control the router's own fraction is the newer of the two, so it survives. Without
    /// that, a window resize would snap every knob back to where the app last wrote it.
    ///
    /// And the part is re-driven **here**, because this table is the channel's only writer:
    /// the app thread deliberately does not bind a property the router owns, so a room that
    /// changed reaches the pixels through this multiply and no other.
    ///
    /// # Errors
    ///
    /// A retarget was refused by the compositor.
    pub fn adopt(&mut self, rows: &[ChromeRow], front: &mut Front<'_>) -> Result<()> {
        for &row in rows {
            // The staleness test is the store's, so a row arriving for a control that has
            // since been recycled into this slot is a fresh row rather than a match.
            let held = self.rows.get(row.id).copied();
            let fraction = held.map_or(row.fraction, |old| old.fraction);
            self.rows.place(row.id, ChromeRow { fraction, ..row });
            // Only where the room actually moved, so re-adopting an unchanged row is free
            // and a fresh one starts wherever the mount put it.
            if held.is_some_and(|old| old.travel != row.travel) {
                self.drive(row.id, fraction, front)?;
            }
        }
        Ok(())
    }

    /// Forgets a control. Anything still pointing at it becomes a miss.
    pub fn release(&mut self, id: ControlId) {
        self.rows.take(id);
        if self.hovered == Some(id) {
            self.hovered = None;
        }
        if self.pressed == Some(id) {
            self.pressed = None;
        }
    }

    /// The window's focus ring, minted once by whoever owns the window.
    pub fn set_ring(&mut self, ring: NodeId) {
        self.ring = Some(ring);
    }

    /// Consumes one tick's reports: moves what they moved, and answers what the application
    /// is being asked to do.
    ///
    /// `out` is appended to rather than replaced, so a caller keeps one buffer for the life
    /// of the window and this path allocates nothing.
    ///
    /// # Errors
    ///
    /// A retarget was refused by the compositor.
    pub fn tick(
        &mut self,
        reports: &[Report],
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        for report in reports {
            self.one(report, front, out)?;
        }
        Ok(())
    }

    /// Hover and press for a control the router will never see the pointer over.
    ///
    /// The window's own caption buttons, and nothing else. Once `WM_NCHITTEST` names one, the
    /// pointer over it is the system's: no [`Report`] is produced for it and no `Sample`
    /// exists to build one from. So the two fields the reports would have set are stated
    /// here, and the wash is derived by the same function every other control's is — a
    /// synthesized report carrying an invented position and pointer type would be a fiction
    /// the gesture layer could later read as fact.
    ///
    /// The pointer is one physical thing, so this and the router's own hover are never both
    /// live. Call it only when the window says the band's state moved, not per tick, or a
    /// stale `(None, None)` will put out a hover the router just lit.
    ///
    /// # Errors
    ///
    /// A retarget was refused by the compositor.
    pub fn nonclient(
        &mut self,
        hover: Option<ControlId>,
        pressed: Option<ControlId>,
        front: &mut Front<'_>,
    ) -> Result<()> {
        let (was_hover, was_pressed) = (self.hovered, self.pressed);
        self.hovered = hover;
        self.pressed = pressed;
        for id in [was_hover, was_pressed, hover, pressed].into_iter().flatten() {
            self.wash(id, front)?;
        }
        Ok(())
    }

    fn one(
        &mut self,
        report: &Report,
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        match *report {
            // More than one of these can arrive from a single service, in the order the
            // pointer crossed them. They are all published, and what a three-millisecond
            // traversal should light is decided here — by a retargeted spring, which
            // swallows a sub-frame excursion at about eight percent of its ramp. That is a
            // better filter than sampling, and it is the one that can still be wrong later.
            Report::HoverChanged { from, to, .. } => {
                self.hovered = to;
                if let Some(from) = from {
                    self.wash(from, front)?;
                }
                if let Some(to) = to {
                    self.wash(to, front)?;
                }
            }
            Report::Pressed { target, .. } => {
                self.pressed = Some(target);
                // Where the value stood when the contact landed: a turn is measured from it.
                self.grabbed = self.rows.get(target).map(|row| (target, row.fraction));
                self.hide_ring(front)?;
                self.wash(target, front)?;
            }
            Report::Released { target, at, .. } => {
                let was = self.pressed.take() == Some(target);
                self.grabbed = None;
                self.wash(target, front)?;
                if !was {
                    return Ok(());
                }
                match self.rows.get(target).and_then(|row| row.drive) {
                    // A press and a release on one control is a tap, whatever happened in
                    // between: the drag policy has already decided that nothing did.
                    None | Some(Interaction::Press) => out.push(Intent {
                        target,
                        what: What::Tapped,
                    }),
                    Some(Interaction::Slide(range)) => {
                        let value = self.slide(target, at, range, front)?;
                        out.push(Intent {
                            target,
                            what: What::Committed(value),
                        });
                    }
                    // Where it was turned to, which is what the front table has been
                    // accumulating — not the bottom of the range.
                    Some(Interaction::Turn(range)) => {
                        let fraction = self.rows.get(target).map_or(0.0, |row| row.fraction);
                        out.push(Intent {
                            target,
                            what: What::Committed(range.at(fraction)),
                        });
                    }
                }
            }
            // **Not a release.** Nothing is committed, the value goes back to what it was
            // before the contact, and the wash goes back to whatever the pointer's current
            // position deserves.
            Report::Canceled { target, .. } => {
                self.pressed = None;
                if let Some((grabbed, fraction)) = self.grabbed.take()
                    && grabbed == target
                {
                    self.drive(target, fraction, front)?;
                }
                self.wash(target, front)?;
            }
            // The thumb moves here, in this tick, before the number is queued.
            Report::Moved { target, sample, .. } => {
                if let Some(Interaction::Slide(range)) = self.rows.get(target).and_then(|r| r.drive)
                {
                    let value = self.slide(target, sample.raw, range, front)?;
                    out.push(Intent {
                        target,
                        what: What::Changed(value),
                    });
                }
            }
            // A knob is dragged rather than slid: its displacement is **from the contact's
            // origin**, so the answer is relative to where the value stood then.
            Report::Dragged { target, update, .. } => {
                if let Some(Interaction::Turn(range)) = self.rows.get(target).and_then(|r| r.drive)
                {
                    let Some((_, from)) = self.grabbed.filter(|&(id, _)| id == target) else {
                        return Ok(());
                    };
                    // Upward is more, and the coordinate grows downward.
                    let value =
                        self.turn(target, from - update.delta.y / TURN_SPAN, range, front)?;
                    out.push(Intent {
                        target,
                        what: What::Changed(value),
                    });
                }
            }
            Report::FocusChanged { to, .. } => self.move_ring(to, front)?,
            // A dial reports **detents**, which are a delta. Treating one as an absolute
            // position sends a single click to an end stop.
            Report::Rotary {
                target: Some(target),
                steps,
                ..
            } => {
                if let Some(Interaction::Turn(range)) = self.rows.get(target).and_then(|r| r.drive)
                {
                    let from = self.rows.get(target).map_or(0.0, |row| row.fraction);
                    let value =
                        self.turn(target, from + detent_delta(range, steps), range, front)?;
                    out.push(Intent {
                        target,
                        what: What::Changed(value),
                    });
                }
            }
            // Exhaustive, and deliberately so: a new `Report` variant must be a compile
            // error here rather than an event this table quietly stops answering. Each of
            // these is somebody else's — the overlay layer's, the text stack's, the
            // recogniser's — and none of them moves a control's own chrome.
            Report::Rotary { target: None, .. }
            | Report::RotaryButton { .. }
            | Report::CaptureLost
            | Report::Buttons { .. }
            | Report::Gesture { .. }
            | Report::Wheel { .. }
            | Report::Key { .. }
            | Report::Escape { .. }
            | Report::Dismiss { .. } => {}
        }
        Ok(())
    }

    /// What a control's wash should be showing right now, from the state this table holds
    /// rather than from the event that just arrived.
    ///
    /// Derived rather than assigned, and that is what makes hover-on-one-control-while-
    /// another-is-pressed expressible — which happens whenever a drag passes under the
    /// pointer, and is the case a shared window-level wash cannot represent at all.
    fn rest_alpha(&self, id: ControlId) -> f32 {
        let Some(row) = self.rows.get(id) else {
            return 0.0;
        };
        if self.pressed == Some(id) {
            row.press
        } else if self.hovered == Some(id) {
            row.hover
        } else {
            0.0
        }
    }

    fn wash(&self, id: ControlId, front: &mut Front<'_>) -> Result<()> {
        let Some(wash) = self.rows.get(id).and_then(|row| row.wash) else {
            return Ok(());
        };
        // A spring, so it plays to completion with **zero front-thread frames** after this
        // one, and so a retarget mid-ramp continues from where it had reached.
        front.retarget(wash.node(), Prop::Opacity, chrome(self.rest_alpha(id)))
    }

    /// Moves a control's part to `fraction`, and records where it now stands.
    ///
    /// **The one place a fraction becomes a property on this thread**, matching the one place
    /// it becomes one on the other and reaching the same two functions. A slide, a knob drag
    /// and a dial detent all land here, so none of them can disagree about which property
    /// carries the value or which way it runs.
    ///
    /// A control this table does not *drive* is left alone: its part follows the
    /// application's own channel, and the app thread is the writer. Refusing here rather than
    /// at each caller is what makes "one writer per channel" hold by construction.
    fn drive(&mut self, id: ControlId, fraction: f32, front: &mut Front<'_>) -> Result<()> {
        let Some(row) = self.rows.get_mut(id) else {
            return Ok(());
        };
        row.fraction = fraction.clamp(0.0, 1.0);
        let (fraction, thumb, travel, drive) = (row.fraction, row.thumb, row.travel, row.drive);
        let (Some(thumb), Some(drive)) = (thumb, drive) else {
            return Ok(());
        };
        match drive {
            // A press has no value, so nothing here moves: a toggle's knob follows the
            // application's own channel and this table must not write it.
            Interaction::Press => Ok(()),
            // A turned part rotates through its own sweep, which is a constant; a slid one
            // travels the room the last solve measured for it.
            Interaction::Turn(_) => {
                front.retarget(thumb, Prop::RotationAngle, chrome(angle_of(fraction)))
            }
            Interaction::Slide(range) => front.retarget(
                thumb,
                if range.vertical {
                    Prop::OffsetY
                } else {
                    Prop::OffsetX
                },
                chrome(offset_of(fraction, travel, range.vertical)),
            ),
        }
    }

    /// Reads a value off the pointer's position along the control's own rect, moves the
    /// part, and answers the number.
    ///
    /// The rect comes from the hit array, which already has it: the entry the router resolved
    /// through *is* the control's box, so nothing here asks the app thread for geometry and
    /// nothing measures. It is looked up by **id**, which the array indexes — a scan would put
    /// the whole screen on the path of every pointer sample.
    fn slide(
        &mut self,
        id: ControlId,
        at: windows_scene::Point,
        range: Range,
        front: &mut Front<'_>,
    ) -> Result<f64> {
        let Some(entry) = front.scene.hits().entry(id).copied() else {
            return Ok(range.at(self.rows.get(id).map_or(0.0, |row| row.fraction)));
        };
        let (along, span) = if range.vertical {
            (at.y - entry.y0, entry.y1 - entry.y0)
        } else {
            (at.x - entry.x0, entry.x1 - entry.x0)
        };
        let fraction = if span > 0.0 {
            fraction_of(along / span, range.vertical)
        } else {
            0.0
        };
        self.drive(id, fraction, front)?;
        Ok(range.at(fraction))
    }

    /// The same, for a control whose input is a **delta**: a knob drag or a dial detent.
    fn turn(
        &mut self,
        id: ControlId,
        fraction: f32,
        range: Range,
        front: &mut Front<'_>,
    ) -> Result<f64> {
        let fraction = fraction.clamp(0.0, 1.0);
        self.drive(id, fraction, front)?;
        Ok(range.at(fraction))
    }

    // ── the window's focus ring ───────────────────────────────────────────────────

    fn move_ring(&mut self, to: Option<ControlId>, front: &mut Front<'_>) -> Result<()> {
        let Some(ring) = self.ring else {
            return Ok(());
        };
        let Some(entry) = to.and_then(|id| front.scene.hits().entry(id)).copied() else {
            return self.hide_ring(front);
        };
        let offset = windows_numerics::Vector2 {
            x: entry.x0,
            y: entry.y0,
        };
        let size = windows_numerics::Vector2 {
            x: entry.x1 - entry.x0,
            y: entry.y1 - entry.y0,
        };
        // Sprung on offset and size, so the glide between two controls is a compositor
        // animation this design gets for free rather than a behaviour it implements.
        front.retarget(ring, Prop::Offset, spring(Value::Vec2(offset)))?;
        front.retarget(ring, Prop::Size, spring(Value::Vec2(size)))?;
        if !self.ring_shown {
            self.ring_shown = true;
            front.retarget(ring, Prop::Opacity, chrome(1.0))?;
        }
        Ok(())
    }

    fn hide_ring(&mut self, front: &mut Front<'_>) -> Result<()> {
        let Some(ring) = self.ring.filter(|_| self.ring_shown) else {
            return Ok(());
        };
        self.ring_shown = false;
        front.retarget(ring, Prop::Opacity, chrome(0.0))
    }
}

/// One of the shared spring templates, so starting a state transition allocates nothing.
const fn spring(to: Value) -> Bind {
    Bind::Animate(Anim::Spring {
        to,
        tuning: Tuning::Chrome,
        delay_ms: 0,
    })
}

const fn chrome(to: f32) -> Bind {
    spring(Value::Scalar(to))
}
