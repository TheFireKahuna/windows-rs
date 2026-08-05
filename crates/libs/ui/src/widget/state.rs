//! The front thread's half of a control: what a [`Report`] does to pixels, before anything
//! is queued to the application.
//!
//! No intent causes a visual: the pixels move front-side in the tick that saw the event, and
//! the [`Intent`] the application receives is emitted afterwards.
//!
//! The path is index arithmetic — a rect test the router already did, an array index, and one
//! retarget. It performs no hash lookup, no allocation, no role resolve and no hop to the app
//! thread. Wash opacities are resolved at mount, on the app thread, and carried here as
//! numbers, because realizing a colour cell mid-hover would create a surface on the
//! interaction path.

use super::{Interaction, Range, TURN_SPAN, angle_of, detent_delta, fraction_of, offset_of};
use crate::input::Report;
use windows_scene::{
    Anim, Backends, Bind, Control, ControlId, Env, NodeId, Prop, Result, Scene, Slots, SpriteId,
    Tuning, Value,
};

/// The front half's write side: the scene, the backends, and the display environment.
///
/// `env` is passed per tick rather than cached on the [`Scene`], so a window moving to another
/// display cannot leave a stale DPI or output transform behind.
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
/// Every field is a number or an id. Roles, colours and closures stay on the app thread,
/// which is the side that can resolve and call them.
#[derive(Copy, Clone, Debug)]
pub struct ChromeRow {
    pub id: ControlId,
    /// The sprite whose opacity hover and press ride. `None` for a control with no wash.
    pub wash: Option<SpriteId>,
    /// Resolved wash opacities.
    pub hover: f32,
    pub press: f32,
    /// The node a value moves, and the travel the last solve measured for it. Holding both
    /// keeps the move to one multiply and keeps the router from asking the app thread for
    /// geometry.
    pub thumb: Option<NodeId>,
    pub travel: f32,
    /// What a pointer means here. `None` is a press and nothing else.
    pub drive: Option<Interaction>,
    /// Where this control's value stands, `0..=1`.
    ///
    /// Seeded by the mount and advanced here. A turned control has no absolute position on
    /// the pointer — a drag reports displacement from its origin and a dial reports detents —
    /// so its value accumulates on the front thread.
    pub fraction: f32,
}

/// What the application is asked to do, raised after the pixels have already moved.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Intent {
    pub target: ControlId,
    pub what: What,
}

/// What an [`Intent`] asks of the application.
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
/// generation turns a report about a control that has since unmounted into a miss rather than
/// a write to whatever now occupies the slot.
#[derive(Default)]
pub struct Controls {
    /// The store over the control id family the app thread mints. This side holds no `Ids`
    /// counter, so it can place a row but never mint an id.
    rows: Slots<Control, ChromeRow>,
    hovered: Option<ControlId>,
    pressed: Option<ControlId>,
    /// The window's focus ring: one visual, sprung between controls. Focus is singular, so
    /// the ring is per window rather than per control, and moving it between two controls is
    /// a compositor animation.
    ring: Option<NodeId>,
    /// Whether the ring is showing. Keyboard focus shows it; a pointer interaction hides it.
    ring_shown: bool,
    /// The control being turned, and the fraction it stood at when the contact landed.
    ///
    /// A turn is a displacement from the contact's origin, so each drag sample is applied to
    /// this fraction rather than accumulated onto the last one — which would drift by the
    /// samples the recogniser coalesced. A cancel restores the same fraction.
    grabbed: Option<(ControlId, f32)>,
}

impl Controls {
    /// Returns an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adopts the rows a mount produced or a solve corrected, as drained by the app thread
    /// alongside its patch.
    ///
    /// A re-adopted row carries corrected geometry, never a corrected value: where the table
    /// already holds the control, its own fraction is kept, so a window resize does not snap
    /// a knob back to where the application last wrote it.
    ///
    /// A row whose travel moved is re-driven here. This table is the only writer of the
    /// properties the router owns, so changed geometry reaches the pixels through this call
    /// and no other.
    ///
    /// # Errors
    ///
    /// A retarget was refused by the compositor.
    pub fn adopt(&mut self, rows: &[ChromeRow], front: &mut Front<'_>) -> Result<()> {
        for &row in rows {
            // `Slots` compares the generation, so a row for a control whose slot has since
            // been recycled misses and is placed fresh.
            let held = self.rows.get(row.id).copied();
            let fraction = held.map_or(row.fraction, |old| old.fraction);
            self.rows.place(row.id, ChromeRow { fraction, ..row });
            // Only where travel moved: an unchanged row costs no retarget, and a fresh one
            // keeps the position the mount gave it.
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

    /// Records the window's focus ring visual, minted once by the window's owner.
    pub fn set_ring(&mut self, ring: NodeId) {
        self.ring = Some(ring);
    }

    /// Applies one tick's reports: moves the pixels they move, and appends the intents they
    /// raise to `out`.
    ///
    /// Per-frame path: `out` is appended to rather than replaced, so a caller holding one
    /// buffer for the life of the window allocates nothing here.
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

    /// Sets hover and press for a control the router never sees the pointer over.
    ///
    /// The window's own caption buttons, and nothing else: once `WM_NCHITTEST` names one, the
    /// system owns its pointer stream, so no [`Report`] and no `Sample` exist for it. The two
    /// fields a report would have set are written here directly, and the wash is derived by
    /// the same path every other control's is.
    ///
    /// Call this only when the window reports that the caption band's state moved, never per
    /// tick: a stale `(None, None)` clears a hover the router has just lit. The pointer is
    /// one physical thing, so this hover and the router's are never both live.
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
        for id in [was_hover, was_pressed, hover, pressed]
            .into_iter()
            .flatten()
        {
            self.wash(id, front)?;
        }
        Ok(())
    }

    fn one(&mut self, report: &Report, front: &mut Front<'_>, out: &mut Vec<Intent>) -> Result<()> {
        match *report {
            // A single service can publish several of these, in the order the pointer
            // crossed them. Each is applied; a sub-frame traversal is absorbed by the
            // spring, which reaches about eight percent of its ramp before the next
            // retarget replaces it.
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
                    // For a control that carries no value, a press and a release on it is a
                    // tap whatever moved in between.
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
                    // The fraction this table accumulated during the turn, not the bottom
                    // of the range.
                    Some(Interaction::Turn(range)) => {
                        let fraction = self.rows.get(target).map_or(0.0, |row| row.fraction);
                        out.push(Intent {
                            target,
                            what: What::Committed(range.at(fraction)),
                        });
                    }
                }
            }
            // A cancel is not a release: nothing is committed, the value returns to where it
            // stood before the contact, and the wash is re-derived from this table's state.
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
            // A knob is dragged rather than slid: the update carries displacement from the
            // contact's origin, so it applies to the fraction held in `grabbed`.
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
            // A dial reports detents, which are a delta: a step count applied as an
            // absolute position would send one click to an end stop.
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
            // Listed rather than matched with `_`, so a new `Report` variant fails to
            // compile here. None of these moves a control's chrome: they belong to the
            // overlay layer, the text stack and the recogniser.
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

    /// Returns the wash opacity `id` should be showing, derived from the state this table
    /// holds rather than from the event that just arrived.
    ///
    /// Because it is derived per control, one control can be hovered while another is
    /// pressed — the state a drag passing under the pointer produces.
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
        // A spring: it plays to completion with no further front-thread frames, and a
        // retarget mid-ramp continues from where it had reached.
        front.retarget(wash.node(), Prop::Opacity, chrome(self.rest_alpha(id)))
    }

    /// Moves a control's part to `fraction` and records where it now stands.
    ///
    /// The one place a fraction becomes a property on this thread. A slide, a knob drag and
    /// a dial detent all reach it, through [`offset_of`] and [`angle_of`], so none of them
    /// can disagree about which property carries the value or which way it runs.
    ///
    /// A control with no thumb or no [`Interaction`] is left alone: its part follows the
    /// application's own channel, whose writer is the app thread.
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
            // A press carries no value: a toggle's knob follows the application's own
            // channel, so this table does not write it.
            Interaction::Press => Ok(()),
            // A turned part rotates through the constant sweep; a slid one travels the
            // extent the last solve measured for it.
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

    /// Returns the value at the pointer's position along the control's own rect, having
    /// moved the part to match.
    ///
    /// The rect is the hit-array entry the router already resolved through, looked up by id,
    /// so nothing here measures or asks the app thread for geometry. Where the control has
    /// no entry, the held fraction is returned and nothing moves.
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

    /// Clamps `fraction`, moves the part, and returns the value, for a control whose input
    /// is a delta: a knob drag or a dial detent.
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
        // Sprung on offset and size, so the glide between two controls runs on the
        // compositor and costs no further front-thread frames.
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

/// Returns the shared chrome spring bound to `to`, so starting a state transition allocates
/// nothing.
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
