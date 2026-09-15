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

use super::{Interaction, Range, TURN_SPAN, detent_delta, fraction_of, offset_of};
use crate::gesture::{DragPhase, DragUpdate};
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
#[derive(Copy, Clone, Debug, Default)]
pub struct ChromeRow {
    pub id: ControlId,
    /// Nearest declared semantic hover scope, inherited through mounted children.
    pub hover_scope: Option<ControlId>,
    /// The sprite whose opacity hover and press ride. `None` for a control with no wash.
    pub wash: Option<SpriteId>,
    /// Resolved wash opacities.
    pub hover: f32,
    pub press: f32,
    /// The node a value moves, the inset it rests at, and the travel the last solve measured
    /// between those insets. Holding all three keeps the move to one multiply and one add,
    /// and keeps the router from asking the app thread for geometry.
    pub scalar_parts: [Option<(NodeId, super::ScalarPart)>; 4],
    pub thumb: Option<NodeId>,
    /// Retained value stroke and its normalized origin.
    pub trail: Option<(NodeId, f32)>,
    pub rest: f32,
    pub travel: f32,
    /// What a pointer means here. `None` is a press and nothing else.
    pub drive: Option<Interaction>,
    /// Whether the application declared a handler for this control's two-axis drag.
    ///
    /// A flag and not the handler: the handler is application code and stays on the app
    /// thread. This side needs only to know whether a drag on this control is worth raising,
    /// and whether a release ends a drag or is a tap.
    pub drags: bool,
    /// Where this control's value stands, `0..=1`.
    ///
    /// Seeded by the mount and advanced here. A turned control has no absolute position on
    /// the pointer — a drag reports displacement from its origin and a dial reports detents —
    /// so its value accumulates on the front thread.
    pub fraction: f32,
    /// Last application-authored fraction. Geometry-only updates repeat it, so adopting
    /// new geometry can preserve a newer pointer value without swallowing an external edit.
    pub source_fraction: f32,
    pub revision: u64,
}

/// The hit target includes the half-thumb gutters; the value range does not.
fn slider_value_at(along: f32, span: f32, travel: f32, range: Range) -> f64 {
    let fraction = if travel > 0.0 {
        fraction_of((along - (span - travel) * 0.5) / travel, range.vertical)
    } else {
        0.0
    };
    range.at(fraction)
}

impl ChromeRow {
    fn adopted(self, previous: Option<Self>) -> Self {
        let fraction = previous
            .filter(|old| old.drive == self.drive && old.revision == self.revision)
            .map_or(self.source_fraction, |old| old.fraction);
        Self { fraction, ..self }
    }
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
    /// Entry or exit of an explicitly observed hover scope.
    Hovered(bool),
    /// A press and a release on the same control.
    Tapped,
    /// A value while it is being moved.
    Scalar {
        value: f64,
        revision: u64,
        commit: bool,
    },
    /// The value it settled on. A canceled contact commits nothing.
    Committed(f64),
    /// Capture was canceled; discard application preview state.
    Canceled(u64),
    /// A two-axis drag moved. Raised only for a control that declared a handler for one.
    Dragged(DragUpdate),
    /// A two-axis drag ended. `commit` is false for a contact that was taken away, whose
    /// pre-drag value stands.
    DragEnded { commit: bool },
}

/// Returns the drag state a sample leaves behind: the control it is on, and whether the
/// gesture has passed its threshold at any point.
///
/// `decided` is **sticky**. A drag that has locked an axis stays locked for the rest of the
/// contact, so a release after it ends the drag rather than being a tap — and the phase of
/// the last sample alone cannot answer that, because a locked drag reports zero displacement
/// on the axis it does not own and can sample as though nothing moved.
///
/// Split out because it is the one decision on this path that touches no pixels: everything
/// else here writes the scene, and a compositor is not available to a test.
fn dragging_after(
    held: Option<(ControlId, bool)>,
    target: ControlId,
    phase: DragPhase,
) -> (ControlId, bool) {
    let was = held.is_some_and(|(id, decided)| id == target && decided);
    (target, was || phase != DragPhase::Undecided)
}

/// What a declared two-axis drag reports to the application.
///
/// One enum rather than a handler per phase: a drag is a sequence with exactly one end, and
/// two callbacks would let a caller register the moves and forget the release.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Dragging {
    /// The contact moved. The update carries the phase, the displacement **projected onto
    /// the locked axis**, and whether this sample is the one that decided that axis.
    Moved(DragUpdate),
    /// The contact lifted: the drag's value takes effect.
    Committed,
    /// The contact was taken away: nothing takes effect, and what stood before the drag
    /// stands.
    Canceled,
}

/// The front thread's control table and the interaction state over it.
///
/// Dense and generational: a control's index is its slot for the life of its mount, and the
/// generation turns a report about a control that has since unmounted into a miss rather than
/// a write to whatever now occupies the slot.
#[derive(Default)]
pub struct Controls {
    canceled: Option<(ControlId, u64)>,

    /// The store over the control id family the app thread mints. This side holds no `Ids`
    /// counter, so it can place a row but never mint an id.
    rows: Slots<Control, ChromeRow>,
    hovered: Option<ControlId>,
    observed_hover: Option<ControlId>,
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
    /// The control a declared two-axis drag is running on, and whether it ever passed the
    /// threshold.
    ///
    /// The flag is what separates a release that ends a drag from one that is a tap: below
    /// the threshold a drag has no axis and no meaning, so a nudge while clicking is a click.
    dragged: Option<(ControlId, bool)>,
}

impl Controls {
    /// Returns an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adopts the rows a mount produced or a solve corrected, as drained by the app thread
    /// alongside its patch.
    /// `released` is the same patch's retirement list. Publication excludes those ids from
    /// `rows`; adoption retires them before input can reach a destroyed visual.
    ///
    /// A geometry-only update preserves the pointer's fraction. A changed source fraction
    /// adopts the application's value, so another control or a document load reaches this
    /// control through the same front-side writer as a pointer.
    ///
    /// A row whose travel moved is re-driven here. This table is the only writer of the
    /// properties the router owns, so changed geometry reaches the pixels through this call
    /// and no other.
    ///
    /// # Errors
    ///
    /// A retarget was refused by the compositor.
    pub fn adopt(
        &mut self,
        rows: &[ChromeRow],
        released: &[ControlId],
        front: &mut Front<'_>,
    ) -> Result<()> {
        // Retirement is part of adoption, not an optional driver step. Late reports can
        // never reach the visuals destroyed by this same patch.
        for &id in released {
            self.release(id);
        }
        for &row in rows {
            debug_assert!(
                !released.contains(&row.id),
                "a retired chrome row crossed the publication seam"
            );
            // `Slots` compares the generation, so a row for a control whose slot has since
            // been recycled misses and is placed fresh.
            let held = self.rows.get(row.id).copied();
            let superseded =
                held.is_some_and(|old| old.drive != row.drive || old.revision != row.revision);
            if superseded && self.pressed == Some(row.id) {
                self.canceled = Some((row.id, row.revision));
                self.pressed = None;
                self.grabbed = None;
                self.dragged = None;
                self.wash(row.id, front)?;
            }
            let row = row.adopted(held);
            if held.is_none_or(|old| {
                (old.trail, old.thumb, old.rest, old.travel)
                    != (row.trail, row.thumb, row.rest, row.travel)
            }) {
                if let (Some((trail, origin)), Some(source), Some(Interaction::Slide(range))) =
                    (row.trail, row.thumb, row.drive)
                {
                    let m = if row.travel > 0.0 {
                        1.0 / row.travel
                    } else {
                        0.0
                    };
                    for (prop, clamp) in [
                        (Prop::TrimStart, [0.0, origin]),
                        (Prop::TrimEnd, [origin, 1.0]),
                    ] {
                        front.retarget(
                            trail,
                            prop,
                            Bind::FollowOffset {
                                source,
                                vertical: range.vertical,
                                affine: windows_scene::Affine {
                                    m,
                                    c: -row.rest * m,
                                },
                                clamp,
                            },
                        )?;
                    }
                }
            }
            self.rows.place(row.id, row);
            if held.is_none_or(|old| {
                (
                    old.rest,
                    old.travel,
                    old.fraction,
                    old.scalar_parts,
                    old.drive,
                ) != (
                    row.rest,
                    row.travel,
                    row.fraction,
                    row.scalar_parts,
                    row.drive,
                )
            }) {
                self.drive(
                    row.id,
                    row.fraction,
                    held.is_none_or(|old| old.fraction == row.fraction),
                    front,
                )?;
            }
        }
        Ok(())
    }

    /// Forgets a control. Anything still pointing at it becomes a miss.
    pub fn release(&mut self, id: ControlId) {
        self.rows.take(id);
        if self.canceled.is_some_and(|(target, _)| target == id) {
            self.canceled = None;
        }
        if self.hovered == Some(id) {
            self.hovered = None;
        }
        if self.pressed == Some(id) {
            self.pressed = None;
        }
        if self.observed_hover == Some(id) {
            self.observed_hover = None;
        }
        if self.grabbed.is_some_and(|(target, _)| target == id) {
            self.grabbed = None;
        }
        if self.dragged.is_some_and(|(target, _)| target == id) {
            self.dragged = None;
        }
    }

    fn observe_hover(&mut self, target: Option<ControlId>, out: &mut Vec<Intent>) {
        let next = target
            .and_then(|id| self.rows.get(id)?.hover_scope)
            .filter(|id| self.rows.get(*id).is_some());
        if self.observed_hover == next {
            return;
        }
        for (target, value) in [(self.observed_hover, false), (next, true)] {
            if let Some(target) = target {
                out.push(Intent {
                    target,
                    what: What::Hovered(value),
                });
            }
        }
        self.observed_hover = next;
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
    pub(crate) fn automation(
        &mut self,
        actions: &[crate::uia::Action],
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        use crate::uia::Action;
        for &action in actions {
            match action {
                Action::SetValue(target, value) => {
                    if let Some(Interaction::Slide(range) | Interaction::Turn(range)) =
                        self.rows.get(target).and_then(|row| row.drive)
                    {
                        let value = range.at(range.fraction(value));
                        self.drive(target, range.fraction(value), true, front)?;
                        self.scalar_event(target, value, false, out);
                        self.scalar_event(target, value, true, out);
                    }
                }
                Action::Invoke(target)
                | Action::Toggle(target)
                | Action::Select(target)
                | Action::Expand(target, _) => {
                    if self.rows.get(target).is_some() {
                        out.push(Intent {
                            target,
                            what: What::Tapped,
                        });
                    }
                }
                Action::Focus(_) | Action::Reveal(_) => {}
            }
        }
        Ok(())
    }

    pub fn tick(
        &mut self,
        reports: &[Report],
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        if let Some((target, revision)) = self.canceled.take() {
            out.push(Intent {
                target,
                what: What::Canceled(revision),
            });
        }
        if let Some(target) = self.pressed.filter(|id| {
            front
                .scene
                .hits()
                .entry(*id)
                .is_none_or(|entry| !entry.flags.contains(windows_scene::HitFlags::INTERACTIVE))
        }) {
            self.one(&Report::Canceled { target, contact: 0 }, front, out)?;
        }
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

    fn scalar_event(&self, target: ControlId, value: f64, commit: bool, out: &mut Vec<Intent>) {
        if let Some(row) = self.rows.get(target) {
            out.push(Intent {
                target,
                what: What::Scalar {
                    value,
                    revision: row.revision,
                    commit,
                },
            });
        }
    }

    fn one(&mut self, report: &Report, front: &mut Front<'_>, out: &mut Vec<Intent>) -> Result<()> {
        match *report {
            Report::Redirect { .. } => {}
            // A single service can publish several of these, in the order the pointer
            // crossed them. Each is applied; a sub-frame traversal is absorbed by the
            // spring, which reaches about eight percent of its ramp before the next
            // retarget replaces it.
            Report::HoverChanged { from, to, .. } => {
                self.hovered = to.filter(|id| self.rows.get(*id).is_some());
                self.observe_hover(to, out);
                if let Some(from) = from {
                    self.wash(from, front)?;
                }
                if let Some(to) = to {
                    self.wash(to, front)?;
                }
            }
            Report::Pressed { target, sample, .. } => {
                if self.rows.get(target).is_none() {
                    return Ok(());
                }
                self.pressed = Some(target);
                // Where the value stood when the contact landed: a turn is measured from it.
                self.grabbed = self.rows.get(target).map(|row| (target, row.fraction));
                // A fresh contact starts undecided, whatever the last one ended as.
                self.dragged = None;
                self.hide_ring(front)?;
                self.wash(target, front)?;
                if let Some(Interaction::Slide(range)) = self.rows.get(target).and_then(|r| r.drive)
                {
                    let value = self.slide(target, sample.raw, range, false, front)?;
                    self.scalar_event(target, value, false, out);
                }
            }
            Report::Released { target, at, .. } => {
                // Scroll rails belong to the scroll front and carry no chrome row. Their
                // release must not become a menu choice or an application click.
                if self.pressed != Some(target) || self.rows.get(target).is_none() {
                    return Ok(());
                }
                self.pressed = None;
                self.grabbed = None;
                self.wash(target, front)?;
                // A drag that passed the threshold ends here and is not also a tap: the two
                // are the same contact, and raising both would run the click handler at the
                // end of every reorder.
                if let Some((_, decided)) = self.dragged.take().filter(|&(id, _)| id == target)
                    && decided
                {
                    out.push(Intent {
                        target,
                        what: What::DragEnded { commit: true },
                    });
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
                        let value = self.slide(target, at, range, false, front)?;
                        self.scalar_event(target, value, true, out);
                    }
                    // The fraction this table accumulated during the turn, not the bottom
                    // of the range.
                    Some(Interaction::Turn(range)) => {
                        let fraction = self.rows.get(target).map_or(0.0, |row| row.fraction);
                        self.scalar_event(target, range.at(fraction), true, out);
                    }
                }
            }
            // A cancel is not a release: nothing is committed, the value returns to where it
            // stood before the contact, and the wash is re-derived from this table's state.
            Report::Canceled { target, .. } => {
                if self.pressed != Some(target) {
                    return Ok(());
                }
                self.pressed = None;
                if let Some((grabbed, fraction)) = self.grabbed.take()
                    && grabbed == target
                {
                    self.drive(target, fraction, false, front)?;
                    if let Some(Interaction::Slide(range) | Interaction::Turn(range)) =
                        self.rows.get(target).and_then(|row| row.drive)
                    {
                        self.scalar_event(target, range.at(fraction), false, out);
                    }
                }
                out.push(Intent {
                    target,
                    what: What::Canceled(self.rows.get(target).map_or(0, |row| row.revision)),
                });
                if let Some((_, decided)) = self.dragged.take().filter(|&(id, _)| id == target)
                    && decided
                {
                    out.push(Intent {
                        target,
                        what: What::DragEnded { commit: false },
                    });
                }
                self.wash(target, front)?;
            }
            // The thumb moves here, in this tick, before the number is queued.
            Report::Moved { target, sample, .. } => {
                if self.pressed != Some(target) {
                    return Ok(());
                }
                if let Some(Interaction::Slide(range)) = self.rows.get(target).and_then(|r| r.drive)
                {
                    let value = self.slide(target, sample.raw, range, true, front)?;
                    self.scalar_event(target, value, false, out);
                }
            }
            // A knob is dragged rather than slid: the update carries displacement from the
            // contact's origin, so it applies to the fraction held in `grabbed`.
            Report::Dragged { target, update, .. } => {
                // A control the application declared a drag handler for gets the update as
                // it stands. Nothing here moves a pixel for it: what a two-axis drag displaces
                // is the application's own subject — a row's position in a list, a scope over
                // channels — which this table holds no geometry for.
                if self.rows.get(target).is_some_and(|r| r.drags) {
                    self.dragged = Some(dragging_after(self.dragged, target, update.phase));
                    out.push(Intent {
                        target,
                        what: What::Dragged(update),
                    });
                }
                if let Some(Interaction::Turn(range)) = self.rows.get(target).and_then(|r| r.drive)
                {
                    let Some((_, from)) = self.grabbed.filter(|&(id, _)| id == target) else {
                        return Ok(());
                    };
                    // Upward is more, and the coordinate grows downward.
                    let value =
                        self.turn(target, from - update.delta.y / TURN_SPAN, range, front)?;
                    self.scalar_event(target, value, false, out);
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
                    self.scalar_event(target, value, false, out);
                    self.scalar_event(target, value, true, out);
                }
            }
            Report::Key {
                target: Some(target),
                event,
            } => {
                if event.kind == crate::input::KeyKind::Down && !event.mods.ctrl && !event.mods.alt
                {
                    if let Some(Interaction::Slide(range) | Interaction::Turn(range)) =
                        self.rows.get(target).and_then(|r| r.drive)
                    {
                        let from = self.rows.get(target).unwrap().fraction;
                        let fraction = match event.key {
                            0x25 | 0x28 => Some(from - detent_delta(range, 1.0)),
                            0x26 | 0x27 => Some(from + detent_delta(range, 1.0)),
                            0x24 => Some(0.0),
                            0x23 => Some(1.0),
                            _ => None,
                        };
                        if let Some(fraction) = fraction {
                            let value = self.turn(target, fraction, range, front)?;
                            self.scalar_event(target, value, false, out);
                            self.scalar_event(target, value, true, out);
                        }
                    }
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
            | Report::Key { target: None, .. }
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
    fn drive(
        &mut self,
        id: ControlId,
        fraction: f32,
        snap: bool,
        front: &mut Front<'_>,
    ) -> Result<()> {
        let Some(row) = self.rows.get_mut(id) else {
            return Ok(());
        };
        row.fraction = fraction.clamp(0.0, 1.0);
        let motion = |v| {
            if snap {
                Bind::Set(Value::Scalar(v))
            } else {
                chrome(v)
            }
        };
        for (node, part) in row.scalar_parts.into_iter().flatten() {
            front.retarget(node, part.property(), motion(part.at(row.fraction)))?;
        }
        let (fraction, thumb, rest, travel, drive) =
            (row.fraction, row.thumb, row.rest, row.travel, row.drive);
        let (Some(thumb), Some(drive)) = (thumb, drive) else {
            return Ok(());
        };
        match drive {
            // A press carries no value: a toggle's knob follows the application's own
            // channel, so this table does not write it.
            Interaction::Press => {
                front.retarget(thumb, Prop::OffsetX, motion(rest + fraction * travel))
            }
            // A turned part rotates through the constant sweep; a slid one travels the
            // extent the last solve measured for it.
            Interaction::Turn(_) => Ok(()),
            Interaction::Slide(range) => front.retarget(
                thumb,
                if range.vertical {
                    Prop::OffsetY
                } else {
                    Prop::OffsetX
                },
                motion(rest + offset_of(fraction, travel, range.vertical)),
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
        snap: bool,
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
        let travel = self.rows.get(id).map_or(0.0, |row| {
            if row.thumb.is_some() {
                row.travel
            } else {
                span
            }
        });
        let value = slider_value_at(along, span, travel, range);
        self.drive(id, range.fraction(value), snap, front)?;
        Ok(value)
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
        let value = range.at(fraction);
        self.drive(id, range.fraction(value), false, front)?;
        Ok(value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gesture::Axis;

    #[test]
    fn unmount_retires_chrome_and_every_held_reference() {
        use crate::build::{Host, mount, tests::fixture};
        let mut patch = fixture();
        let held = mount(
            crate::widget::button("Gain"),
            Host::with(|h| h.model().root()),
        );
        let mut down = crate::seam::Down::default();
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        let row = down
            .chrome
            .iter()
            .find(|row| row.wash.is_some())
            .copied()
            .unwrap();
        let mut controls = Controls::new();
        controls.rows.place(row.id, row);
        controls.hovered = Some(row.id);
        controls.observed_hover = Some(row.id);
        controls.pressed = Some(row.id);
        controls.grabbed = Some((row.id, 0.5));
        controls.dragged = Some((row.id, true));
        down.clear();
        drop(held);
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        assert!(down.released.contains(&row.id));
        for &id in &down.released {
            controls.release(id);
        }
        assert!(
            controls.rows.get(row.id).is_none(),
            "late reports cannot find a destroyed wash"
        );
        assert!(controls.hovered.is_none() && controls.observed_hover.is_none());
        assert!(
            controls.pressed.is_none() && controls.grabbed.is_none() && controls.dragged.is_none()
        );
    }

    #[test]
    fn native_adoption_ignores_late_input_after_a_control_unmounts() -> Result<()> {
        use crate::build::{Host, mount, tests::fixture};
        use windows_window::{Apartment, Window, ensure_dispatcher_queue};
        ensure_dispatcher_queue(Apartment::Asta)?;
        let _ = fixture();
        let (env, scope) = Host::with(|h| (h.env, h.root_scope));
        let mut model = windows_scene::Model::new(crate::layout::root());
        model.set_window(windows_numerics::Vector2 { x: 800.0, y: 600.0 });
        Host::install(model, env, scope);
        let window = Window::new("control lifetime regression")
            .size_dips(800.0, 600.0)
            .create()?;
        let back = Backends::new(
            windows_composition::Compositor::new()?,
            &windows_d2d::Gpu::for_window()?,
            windows_text::FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]),
        )?;
        Host::install_text(back.ladder().clone())?;
        let mut scene = Scene::new_at(
            window.handle(),
            &back,
            env,
            windows_scene::BackdropSpec::default(),
        )?;
        let mut controls = Controls::new();
        let mut down = crate::seam::Down::default();
        let root = Host::with(|h| h.model().root());
        let held = mount(crate::widget::button("Gain"), root);
        Host::flush(&mut down.patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        let id = down
            .chrome
            .iter()
            .find(|row| row.wash.is_some())
            .unwrap()
            .id;
        scene.apply(&mut down.patch, &back, env)?;
        let mut front = Front {
            scene: &mut scene,
            back: &back,
            env,
        };
        controls.adopt(&down.chrome, &down.released, &mut front)?;
        let mut intents = Vec::new();
        controls.tick(
            &[Report::HoverChanged {
                from: None,
                to: Some(id),
                at: Default::default(),
                qpc: 0,
            }],
            &mut front,
            &mut intents,
        )?;
        down.clear();
        drop(held);
        let _replacement = mount(crate::widget::button("replacement"), root);
        Host::flush(&mut down.patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        front.scene.apply(&mut down.patch, &back, env)?;
        controls.adopt(&down.chrome, &down.released, &mut front)?;
        intents.clear();
        let replacement = down
            .chrome
            .iter()
            .find(|row| row.wash.is_some())
            .unwrap()
            .id;
        controls.pressed = Some(replacement);
        controls.grabbed = Some((replacement, 0.5));
        controls.dragged = Some((replacement, true));
        controls.tick(
            &[
                Report::HoverChanged {
                    from: Some(id),
                    to: None,
                    at: Default::default(),
                    qpc: 0,
                },
                Report::Canceled {
                    target: id,
                    contact: 1,
                },
                Report::Released {
                    target: id,
                    contact: 1,
                    at: Default::default(),
                },
            ],
            &mut front,
            &mut intents,
        )?;
        controls.automation(&[crate::uia::Action::Invoke(id)], &mut front, &mut intents)?;
        assert!(
            intents.is_empty(),
            "retired generations cannot move visuals or invoke callbacks"
        );
        assert!(controls.rows.get(id).is_none());
        assert_eq!(controls.pressed, Some(replacement));
        assert_eq!(controls.grabbed, Some((replacement, 0.5)));
        assert_eq!(controls.dragged, Some((replacement, true)));
        Ok(())
    }

    /// Two distinct control ids, minted rather than constructed: a slot and a generation are
    /// the arena's to assign and this crate cannot spell one.
    fn two() -> (ControlId, ControlId) {
        let mut ids = windows_scene::Ids::<Control>::new();
        (ids.mint(), ids.mint())
    }

    #[test]
    fn semantic_hover_reports_scope_edges_and_ignores_child_crossings() {
        let mut ids = windows_scene::Ids::<Control>::new();
        let [scope, a, b, other, ordinary] = core::array::from_fn(|_| ids.mint());
        let mut controls = Controls::default();
        for (id, hover_scope) in [
            (scope, Some(scope)),
            (a, Some(scope)),
            (b, Some(scope)),
            (other, Some(other)),
            (ordinary, None),
        ] {
            controls.rows.place(
                id,
                ChromeRow {
                    id,
                    hover_scope,
                    wash: None,
                    hover: 0.0,
                    press: 0.0,
                    scalar_parts: [None; 4],
                    thumb: None,
                    trail: None,
                    rest: 0.0,
                    travel: 0.0,
                    drive: None,
                    drags: false,
                    fraction: 0.0,
                    source_fraction: 0.0,
                    revision: 0,
                },
            );
        }
        let mut out = Vec::with_capacity(2);
        controls.observe_hover(Some(a), &mut out);
        assert_eq!(
            out,
            [Intent {
                target: scope,
                what: What::Hovered(true)
            }]
        );
        out.clear();
        for _ in 0..1000 {
            controls.observe_hover(Some(b), &mut out);
            controls.observe_hover(Some(a), &mut out);
        }
        assert!(out.is_empty(), "child crossings must stay scene-side");
        controls.release(a);
        controls.observe_hover(Some(b), &mut out);
        assert!(
            out.is_empty(),
            "replacing a hovered child preserves the scope"
        );
        controls.observe_hover(Some(other), &mut out);
        assert_eq!(
            out,
            [
                Intent {
                    target: scope,
                    what: What::Hovered(false)
                },
                Intent {
                    target: other,
                    what: What::Hovered(true)
                }
            ]
        );
        out.clear();
        controls.observe_hover(Some(ordinary), &mut out);
        assert_eq!(
            out,
            [Intent {
                target: other,
                what: What::Hovered(false)
            }]
        );
        out.clear();
        controls.observe_hover(None, &mut out);
        controls.observe_hover(Some(ordinary), &mut out);
        assert!(out.is_empty());
        controls.observe_hover(Some(b), &mut out);
        out.clear();
        controls.release(scope);
        controls.observe_hover(Some(b), &mut out);
        assert!(
            out.is_empty(),
            "a released scope cannot receive another event"
        );
        assert_eq!(out.capacity(), 2);
    }

    #[test]
    fn slider_pointer_maps_the_visible_rail_and_snaps_the_reported_value() {
        let horizontal = Range::new(-24.0, 24.0).step(0.1);
        assert_eq!(slider_value_at(13.0, 126.0, 100.0, horizontal), -24.0);
        assert_eq!(slider_value_at(63.0, 126.0, 100.0, horizontal), 0.0);
        assert_eq!(slider_value_at(113.0, 126.0, 100.0, horizontal), 24.0);
        assert_eq!(slider_value_at(-20.0, 126.0, 100.0, horizontal), -24.0);
        assert_eq!(slider_value_at(140.0, 126.0, 100.0, horizontal), 24.0);
        assert!((slider_value_at(70.0, 126.0, 100.0, horizontal) - 3.4).abs() < 1e-10);
        let vertical = Range {
            vertical: true,
            ..horizontal
        };
        assert_eq!(slider_value_at(13.0, 126.0, 100.0, vertical), 24.0);
        assert_eq!(slider_value_at(113.0, 126.0, 100.0, vertical), -24.0);
        assert!(slider_value_at(0.0, 0.0, 0.0, horizontal).is_finite());
    }

    #[test]
    fn source_edits_and_geometry_updates_have_distinct_value_ownership() {
        let (id, _) = two();
        let source = ChromeRow {
            hover_scope: None,
            id,
            wash: None,
            hover: 0.0,
            press: 0.0,
            scalar_parts: [None; 4],
            thumb: None,
            trail: None,
            rest: 0.0,
            travel: 100.0,
            drive: Some(Interaction::Slide(Range::UNIT)),
            drags: false,
            fraction: 0.25,
            source_fraction: 0.25,
            revision: 0,
        };
        assert_eq!(source.adopted(None).fraction, 0.25);
        let dragged = ChromeRow {
            fraction: 0.75,
            ..source
        };
        let resized = ChromeRow {
            travel: 200.0,
            ..source
        }
        .adopted(Some(dragged));
        assert_eq!((resized.fraction, resized.travel), (0.75, 200.0));
        let edited = ChromeRow {
            source_fraction: 0.5,
            revision: 1,
            ..source
        }
        .adopted(Some(resized));
        assert_eq!(edited.fraction, 0.5);
    }

    #[test]
    fn a_drag_below_the_threshold_is_still_a_click() {
        let (a, _) = two();
        assert_eq!(dragging_after(None, a, DragPhase::Undecided), (a, false));
    }

    #[test]
    fn a_decided_drag_stays_decided_through_the_rest_of_the_contact() {
        let (a, _) = two();
        let after = dragging_after(None, a, DragPhase::Locked(Axis::Vertical));
        assert_eq!(after, (a, true));
        // A locked drag reports zero on the axis it does not own, so a sample can look
        // undecided; the lock is never revisited and neither is this.
        assert_eq!(
            dragging_after(Some(after), a, DragPhase::Undecided),
            (a, true)
        );
    }

    #[test]
    fn a_contact_on_another_control_starts_undecided() {
        let (a, b) = two();
        let after = dragging_after(None, a, DragPhase::Locked(Axis::Horizontal));
        assert_eq!(
            dragging_after(Some(after), b, DragPhase::Undecided),
            (b, false),
            "one control's lock does not carry to the next"
        );
    }
}

#[cfg(test)]
#[path = "scalar_tests.rs"]
mod scalar_tests;
