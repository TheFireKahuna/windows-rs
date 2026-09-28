//! The front thread's half of a control: what a [`Report`] does to pixels, before anything is
//! queued to the application.
//!
//! No intent causes a visual: the pixels move front-side in the tick that saw the event, and the
//! [`Intent`] the application receives is emitted afterwards.
//!
//! Scalar samples retarget retained channels. A reorder snapshots its group's layout slots
//! at threshold crossing; later samples reuse that storage and retarget neighbors only when
//! the insertion index changes.
//!
//! Interaction state is held per window, not per control. The pointer and the keyboard are each
//! one physical thing, so a second hovered control cannot exist to carry a bit.

use super::roles::{FOCUS_OUTSET, ScalarPart, TURN_SWEEP, fraction_of};
#[path = "reorder.rs"]
mod reorder;
pub use reorder::ReorderUpdate;
pub(crate) use reorder::ReorderRow;
use crate::gesture::{DragUpdate, Phase, Recognised};
use crate::input::{KeyKind, Report};
use crate::uia::Action;
use windows_numerics::Vector2;
use windows_present::SubId;
use windows_scene::{
    Affine, Anim, Backends, Bind, CONTROL, ControlId, Env, HitFlags, NodeId, Point, Prop, Result,
    Scene, Slots, SpriteId, Tuning, Value,
};

/// What an arrow, `Home` or `End` does to a scalar's fraction, in steps of its own quantum.
///
/// The end stops are infinite rather than a second shape: the clamp in [`Controls::put`] turns
/// them into the ends.
const KEYS: [(u16, f32); 6] = [
    (0x25, -1.0),
    (0x28, -1.0),
    (0x26, 1.0),
    (0x27, 1.0),
    (0x24, f32::NEG_INFINITY),
    (0x23, f32::INFINITY),
];

/// What a control is, as bits: everything a report needs about it that is not a number or an id.
pub mod flag {
    /// Scope edges emit application hover intents.
    pub const OBSERVES: u8 = 1 << 0;
    /// The application declared a handler for a two-axis drag here.
    ///
    /// A flag and not the handler: the handler is application code and stays on the app thread.
    /// This side needs only to know whether a drag is worth raising, and whether a release ends
    /// one or is a tap.
    pub const DRAGS: u8 = 1 << 1;
    /// A pointer reads the value off its position along the control's own rect.
    pub const SLIDE: u8 = 1 << 2;
    /// A pointer turns the value: a rotation about the control's centre, or a dial detent.
    pub const TURN: u8 = 1 << 3;
    /// The value runs up the screen, which is against the coordinate it is read from.
    pub const VERTICAL: u8 = 1 << 4;
    /// The wash reaches full opacity while its control holds input focus.
    pub const FOCUS_WASH: u8 = 1 << 5;
    /// Disabled controls suppress every transient wash contribution.
    pub const DISABLED: u8 = 1 << 6;
    /// A decided drag lifts its retained subtree into the scene's overlay band.
    pub const DRAG_PREVIEW: u8 = 1 << 7;
    /// Either way a pointer moves a value.
    pub const VALUED: u8 = SLIDE | TURN;
}

/// How a value arrived, which decides both how its pixels move and what the application is told.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum How {
    /// The pointer is carrying the motion.
    Carried,
    /// The value jumped on its own and springs to where it landed.
    Sprung,
    /// The gesture ended here, so the application is told to commit.
    Settled,
}

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
    /// Springs `prop` to `to`.
    ///
    /// One of the shared spring templates, so a state change allocates nothing, and the motion
    /// plays to completion on the compositor with no further frames on any thread of ours. A
    /// retarget mid-ramp continues from where the spring had reached.
    fn spring(&mut self, node: NodeId, prop: Prop, to: Value) -> Result<()> {
        let bind = Bind::Animate(Anim::Spring {
            to,
            tuning: Tuning::Chrome,
            delay_ms: 0,
        });
        self.scene.retarget(node, prop, bind, self.back)
    }

    /// Springs a scalar channel, or writes it plainly where the value is already being carried.
    ///
    /// Two platform facts decide which. `StartAnimation` resets the property's velocity, so a
    /// spring retargeted on every pointer-move pins the property instead of moving it. And an
    /// implicit natural-motion animation never receives its target value automatically, so a
    /// sprung first write on a freshly mounted part runs to zero rather than to the value: a mount
    /// pins its start by arriving as [`How::Carried`].
    fn scalar(&mut self, node: NodeId, prop: Prop, to: f32, how: How) -> Result<()> {
        if how == How::Carried {
            let bind = Bind::Set(Value::Scalar(to));
            self.scene.retarget(node, prop, bind, self.back)
        } else {
            self.spring(node, prop, Value::Scalar(to))
        }
    }
}

/// One control's chrome. Every control has one.
///
/// Every field is a number or an id. Roles, colours and closures stay on the app thread, which is
/// the side that can resolve and call them: the two alphas are resolved at mount, because
/// realizing a colour cell mid-hover would create a surface on the interaction path.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ChromeRow {
    /// The sprite whose opacity hover and press ride, or a `NONE` node for a control with no wash.
    /// Written by every hover and press report; read as the only pixel an ordinary state change
    /// moves.
    pub wash: SpriteId,
    /// The nearest interaction scope's retained reveal target, or `NONE`. Written by hover, press
    /// and focus through the scope; read as the opacity that fades.
    pub reveal: NodeId,
    /// Nearest interaction scope, inherited through mounted children, or `NONE`.
    pub scope: ControlId,
    /// Resolved wash opacities: what a hover shows, and what a press shows over it.
    pub hover: f32,
    pub press: f32,
    pub flags: u8,
}

impl Default for ChromeRow {
    /// A control with nothing to light: no wash, no reveal, no scope, and no flag set.
    fn default() -> Self {
        Self {
            wash: SpriteId(NodeId::NONE),
            reveal: NodeId::NONE,
            scope: ControlId::NONE,
            hover: 0.0,
            press: 0.0,
            flags: 0,
        }
    }
}

/// A control's value half. Placed only where a value moves, which is a handful of the controls a
/// screen carries, so the rest carry the chrome row alone.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ValueRow {
    /// Every node the value moves, and how each maps `0..=1`. A thumb, a value stroke and a
    /// rotating needle are rows of this one array, so the router drives them in one loop and none
    /// of them can disagree about which property carries the value.
    pub parts: [(NodeId, ScalarPart); 4],
    /// Bottom of the range and its width. A fraction becomes the number an intent carries with one
    /// multiply and one add, so this side never holds the range the application authored.
    pub min: f64,
    pub span: f64,
    /// The inset a value part rests at, and the travel the last solve measured beyond it.
    pub rest: f32,
    pub travel: f32,
    /// Where the value stands, `0..=1`. Seeded by the mount and advanced here: a turned control
    /// has no absolute position on the pointer, so its value accumulates on this thread.
    pub fraction: f32,
    /// The increment in application units, or zero for a continuous value.
    pub step: f64,
    /// The application's source revision. A changed one supersedes a gesture standing on this
    /// control; a repeated one preserves an active gesture's fraction.
    pub revision: u64,
}

impl ValueRow {
    fn quantum(&self) -> f32 {
        crate::widget::Range::new(self.min, self.min + self.span)
            .step(self.step)
            .quantum()
    }
}

#[derive(Copy, Clone, Default)]
struct HeldValue {
    row: ValueRow,
    /// Last published fraction, separate from the fraction input advances in `row`.
    source: f32,
}

/// What the application is asked to do, raised after the pixels have already moved.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Intent {
    pub target: ControlId,
    pub what: What,
}

impl Intent {
    /// Returns the intent an activation key raises on the control that holds focus.
    ///
    /// `Enter` and `Right` on a menu row reach the focus scope rather than a control, so the
    /// caller names the target and this states what reaching it means: the tap a pointer would
    /// have raised, so a row opened from the keyboard runs the handler a click runs.
    #[must_use]
    pub const fn invoke_focused(target: ControlId) -> Self {
        Self {
            target,
            what: What::Tapped,
        }
    }
}

/// What an [`Intent`] asks of the application.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum What {
    /// A platform-recognized double tap in target-local DIPs.
    DoubleTapped(Point),
    /// Signed wheel detents delivered to the positional target that declared interest.
    Wheel { notches: f32, horizontal: bool },
    Closed,
    /// Entry or exit of an explicitly observed hover scope.
    Hovered(bool),
    /// A press and a release on the same control.
    Tapped,
    /// Sets the requested disclosure state.
    Expanded(bool),
    Selected(crate::uia::action::SelectionChange),
    TextReveal {
        revision: u64,
        start: u32,
        end: u32,
    },
    /// A value while it is being moved, and the value it settled on when `commit`.
    Scalar {
        value: f64,
        revision: u64,
        commit: bool,
    },
    /// Capture was canceled; discard application preview state.
    Canceled(u64),
    /// A two-axis drag moved. Raised only for a control that declared a handler for one.
    Dragged(DragUpdate),
    /// A two-axis drag ended. `Some` carries the last sample it reported, whose displacement takes
    /// effect; `None` is a contact that was taken away, whose pre-drag value stands.
    DragEnded(Option<DragUpdate>),
    Reordered(ReorderUpdate),
    ReorderEnded(Option<ReorderUpdate>),
    /// A contact finished on one pickable piece of a presented region's pixels.
    ///
    /// Raised by the present layer rather than by these tables: the part is resolved against
    /// pixels the renderer drew, which no control row holds a rect for.
    Part(SubId),
}

/// The front thread's control tables and the interaction state over them.
///
/// Dense and generational: a control's index is its slot for the life of its mount, and the
/// generation turns a report about a control that has since unmounted into a miss rather than a
/// write to whatever now occupies the slot. Every singleton below is `ControlId::NONE` when it
/// names nothing, so retiring a control is one comparison against each.
#[derive(Default)]
pub struct Controls {
    /// Target of the preceding recognized single tap; a double tap cannot cross owners.
    tapped: ControlId,
    /// The stores over the control id family the app thread mints. This side holds no `Ids`
    /// counter, so it can place a row but never mint an id.
    chrome: Slots<CONTROL, ChromeRow>,
    values: Slots<CONTROL, HeldValue>,
    hovered: ControlId,
    pressed: ControlId,
    focused: ControlId,
    /// Input focus survives pointer presses that hide the external keyboard ring.
    input_focus: ControlId,
    /// The observed scope the application was last told about, so a crossing between two children
    /// of one scope is not an edge.
    observed: ControlId,
    /// The control being turned, and the fraction it stood at when the contact landed.
    ///
    /// A turn is a displacement from the contact's origin, so each sample applies to this rather
    /// than accumulating onto the last one, which would drift by the samples the recogniser
    /// coalesced. A cancel restores it.
    grabbed: ControlId,
    grab_at: f32,
    /// The control a declared two-axis drag runs on, the last sample it raised, and whether it
    /// ever passed the threshold.
    ///
    /// `decided` is sticky. Below the threshold a drag has no axis and no meaning, so a nudge
    /// while clicking is a click; once locked it stays locked, so a release ends the drag rather
    /// than being a tap. The phase of the last sample alone cannot answer that, because a locked
    /// drag reports zero displacement on the axis it does not own and can sample as though nothing
    /// moved.
    dragged: ControlId,
    drag_last: Option<DragUpdate>,
    decided: bool,
    /// A gesture the app thread superseded by editing the value under it, raised on the next tick
    /// because adoption has no intent buffer.
    superseded: ControlId,
    superseded_at: u64,
    /// The scopes whose reveal targets are up, deduplicated in fixed storage.
    revealed: [ControlId; 3],
    translations: Vec<(ControlId, NodeId, windows_scene::Translation)>,
    previews: Vec<(ControlId, NodeId)>,
    preview_release: Option<u64>,
    reorders: reorder::Reorders,
    translation_changed: bool,
    /// The window's one focus ring, sprung between controls. Focus is singular, so the ring is per
    /// window rather than per control, and the glide between two controls is a compositor
    /// animation rather than a behaviour this crate runs.
    ring: NodeId,
    ring_shown: bool,
    viewport: Option<Vector2>,
}

impl Controls {
    /// Returns an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the window's focus ring visual, minted once by the window's owner.
    pub fn set_ring(&mut self, ring: NodeId) {
        self.ring = ring;
    }

    /// Keeps the focus outline inside the client area when the window changes size.
    pub(crate) fn set_viewport(&mut self, size: Vector2, front: &mut Front<'_>) -> Result<()> {
        self.viewport = Some(size);
        self.move_ring(front)
    }

    /// Adopts the rows a mount produced or a solve corrected, as drained by the app thread
    /// alongside its patch.
    ///
    /// `released` is the same patch's retirement list. Publication excludes those ids; adoption
    /// retires them before input can reach a destroyed visual.
    ///
    /// An unchanged source preserves the pointer's fraction through layout. An idle control
    /// adopts a changed source even within the same revision. A changed revision adopts the
    /// application's value and supersedes a gesture standing on it, so another control or a
    /// document load reaches this control through the same front-side writer as a pointer.
    /// Mounted press-only controls spring to changed values. Mounts, geometry corrections,
    /// sliders and rotary controls adopt their positions immediately.
    ///
    /// Geometry that moved is re-driven and re-bound here. These tables are the only writer of the
    /// properties the router owns, so changed geometry reaches the pixels through this call and no
    /// other.
    ///
    /// # Errors
    ///
    /// A retarget was refused by the compositor.
    pub fn adopt(
        &mut self,
        chrome: &[(ControlId, ChromeRow)],
        values: &[(ControlId, ValueRow)],
        released: &[ControlId],
        front: &mut Front<'_>,
    ) -> Result<()> {
        // Retirement is part of adoption and not an optional driver step: a late report can never
        // reach a visual this same patch destroyed.
        for &id in released {
            if id == self.dragged { front.scene.end_drag_preview(); }
            self.release(id);
        }
        for &(id, row) in chrome {
            debug_assert!(!released.contains(&id), "a retired row crossed publication");
            self.chrome.place(id, row);
            self.wash(id, front)?;
        }
        for &(id, mut row) in values {
            let mounted = self.values.get(id).is_some();
            let held = self.value(id);
            let source = row.fraction;
            if self.values.get(id).is_some_and(|v| {
                held.revision == row.revision && (self.grabbed == id || v.source == source)
            }) {
                row.fraction = held.fraction;
            } else if self.grabbed == id && held.revision != row.revision {
                self.grabbed = ControlId::NONE;
                (self.superseded, self.superseded_at) = (id, row.revision);
            }
            let moved = (held.rest, held.travel, held.parts) != (row.rest, row.travel, row.parts);
            self.values.place(id, HeldValue { row, source });
            if moved {
                self.bind_followers(row, front)?;
            }
            if moved || held.fraction != row.fraction {
                let how = if mounted && !moved && self.flags(id) & flag::VALUED == 0 {
                    How::Sprung
                } else {
                    How::Carried
                };
                self.drive(id, row.fraction, how, front)?;
            }
        }
        self.move_ring(front)?;
        self.reveals(front)
    }

    /// Installs shared target geometry after the scene applies the structural patch.
    pub fn adopt_translations(
        &mut self,
        rows: &[(ControlId, NodeId, windows_scene::Translation)],
        released: &[ControlId],
        front: &mut Front<'_>,
    ) -> Result<()> {
        for &id in released {
            front.scene.remove_translation(id);
        }
        for (id, node, state) in rows {
            if let Some(row) = self.translations.iter_mut().find(|r| r.0 == *id) {
                *row = (*id, *node, state.clone());
            } else {
                self.translations.push((*id, *node, state.clone()));
            }
        }
        for (id, _, state) in &self.translations {
            front.scene.install_translation(*id, state);
        }
        self.translate(front)
    }

    pub fn take_translation_changed(&mut self) -> bool {
        core::mem::take(&mut self.translation_changed)
    }

    /// Installs only the controls that declared a retained drag preview.
    pub fn adopt_previews(&mut self, rows: &[(ControlId, NodeId)]) {
        for &(id, node) in rows {
            if let Some(row) = self.previews.iter_mut().find(|row| row.0 == id) {
                row.1 = node;
            } else {
                self.previews.push((id, node));
            }
        }
    }

    /// Takes the released preview identity to accompany its completed drag intent.
    pub fn take_preview_release(&mut self) -> Option<u64> {
        self.preview_release.take()
    }

    pub(crate) fn adopt_reorders(&mut self, rows: &[ReorderRow], released: &[ControlId]) {
        self.reorders.adopt(rows, released);
    }

    pub(crate) fn validate_reorder(&mut self, front: &mut Front<'_>) -> Result<()> {
        self.translation_changed |= self.reorders.validate(front)?;
        Ok(())
    }

    pub(crate) fn finish_reorder(&mut self, epoch: u64, front: &mut Front<'_>) -> Result<()> {
        self.translation_changed |= self.reorders.finish(epoch, front)?;
        Ok(())
    }

    fn translate(&mut self, front: &mut Front<'_>) -> Result<()> {
        let mut changed = false;
        for (id, node, state) in &self.translations {
            if self.reorders.contains(*id) { continue; }
            let active = [self.hovered, self.pressed, self.focused].into_iter().any(|source| {
                !source.is_none() && front.scene.hits().entry(source).is_some()
                    && front.scene.hits().in_translation(*id, source)
            });
            if state.set_active(active) {
                let to = state.get();
                let by = state.target();
                for (prop, value, extent) in [
                    (Prop::TranslationX, to.x, by.x), (Prop::TranslationY, to.y, by.y),
                ] {
                    if extent != 0.0 {
                        front.spring(*node, prop, Value::Scalar(value))?;
                    }
                }
                changed = true;
            }
        }
        self.translation_changed |= changed;
        if changed { self.move_ring(front)?; }
        Ok(())
    }

    /// Binds trim and opacity followers to the thumb's compositor offset.
    fn bind_followers(&self, row: ValueRow, front: &mut Front<'_>) -> Result<()> {
        let Some((source, vertical)) = row.parts.iter().find_map(|(node, part)| match part {
            ScalarPart::Thumb { vertical } => Some((*node, *vertical)),
            _ => None,
        }) else {
            return Ok(());
        };
        for (node, part) in row.parts {
            let targets = match part {
                ScalarPart::Trail { from } => [
                    Some((Prop::TrimStart, [0.0, from])),
                    Some((Prop::TrimEnd, [from, 1.0])),
                ],
                ScalarPart::Fade => [Some((Prop::Opacity, [0.0, 1.0])), None],
                _ => continue,
            };
            let m = if row.travel > 0.0 {
                1.0 / row.travel
            } else {
                0.0
            };
            let mut affine = Affine {
                m,
                c: -row.rest * m,
            };
            if part == ScalarPart::Fade && vertical {
                affine = Affine {
                    m: -m,
                    c: 1.0 + row.rest * m,
                };
            }
            for (prop, clamp) in targets.into_iter().flatten() {
                let bind = Bind::FollowOffset {
                    source,
                    vertical,
                    affine,
                    clamp,
                };
                front.scene.retarget(node, prop, bind, front.back)?;
            }
        }
        Ok(())
    }

    /// Forgets a control. Anything still pointing at it becomes a miss.
    pub fn release(&mut self, id: ControlId) {
        self.previews.retain(|row| row.0 != id);
        self.translations.retain(|(owner, _, state)| {
            if *owner != id { return true; }
            state.set_active(false);
            false
        });
        self.chrome.take(id);
        self.values.take(id);
        for slot in [
            &mut self.tapped,
            &mut self.hovered,
            &mut self.pressed,
            &mut self.focused,
            &mut self.input_focus,
            &mut self.observed,
            &mut self.grabbed,
            &mut self.dragged,
            &mut self.superseded,
        ]
        .into_iter()
        .chain(self.revealed.iter_mut())
        {
            if *slot == id {
                *slot = ControlId::NONE;
            }
        }
    }

    /// Applies one tick's reports: moves the pixels they move, and appends the intents they raise
    /// to `out`.
    ///
    /// Per-frame path: `out` is appended to rather than replaced, so a caller holding one buffer
    /// for the life of the window allocates nothing here.
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
        if !self.superseded.is_none() {
            let target = core::mem::replace(&mut self.superseded, ControlId::NONE);
            out.push(Intent {
                target,
                what: What::Canceled(self.superseded_at),
            });
        }
        // A control that stopped being interactive under a live contact never sends a release.
        let dropped = !self.pressed.is_none()
            && front
                .scene
                .hits()
                .entry(self.pressed)
                .is_none_or(|entry| !entry.flags.contains(HitFlags::INTERACTIVE));
        if dropped {
            self.end(self.pressed, None, front, out)?;
        }
        for report in reports {
            self.one(report, front, out)?;
        }
        self.reveals(front)
    }

    /// Applies the actions an automation client asked for.
    ///
    /// A set value lands on the same writer a pointer reaches, so clamping and snapping cannot
    /// differ between them.
    ///
    /// # Errors
    ///
    /// A retarget was refused by the compositor.
    pub(crate) fn automation(
        &mut self,
        actions: &[Action],
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        for &action in actions {
            match action {
                Action::SetValue(target, value) => {
                    let row = self.value(target);
                    if row.span != 0.0 {
                        self.settle(target, ((value - row.min) / row.span) as f32, front, out)?;
                    }
                }
                Action::Invoke(target) | Action::Toggle(target)
                    if self.chrome.get(target).is_some() =>
                {
                    out.push(Intent {
                        target,
                        what: What::Tapped,
                    });
                }
                Action::Expand(target, expanded) if self.chrome.get(target).is_some() => {
                    out.push(Intent {
                        target,
                        what: What::Expanded(expanded),
                    });
                }
                Action::Select(target, change) if self.chrome.get(target).is_some() => {
                    out.push(Intent {
                        target,
                        what: What::Selected(change),
                    });
                }
                Action::CloseWindow(target) => out.push(Intent {
                    target,
                    what: What::Closed,
                }),
                Action::RevealText(target, revision, start, end, _) => {
                    out.push(Intent {
                        target,
                        what: What::TextReveal {
                            revision,
                            start,
                            end,
                        },
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Sets hover and press for a control the router never sees the pointer over.
    ///
    /// The window's own caption buttons, and nothing else: once `WM_NCHITTEST` names one, the
    /// system owns its pointer stream, so no [`Report`] and no sample exist for it. The two fields
    /// a report would have set are written here directly, and the wash is derived by the same path
    /// every other control's is.
    ///
    /// Call this only when the window reports that the caption band's state moved, never per tick:
    /// a stale pair of nothings clears a hover the router has just lit. The pointer is one physical
    /// thing, so this hover and the router's are never both live.
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
        let was = [self.hovered, self.pressed];
        self.hovered = hover.unwrap_or(ControlId::NONE);
        self.pressed = pressed.unwrap_or(ControlId::NONE);
        for id in [was[0], was[1], self.hovered, self.pressed] {
            self.wash(id, front)?;
        }
        self.reveals(front)
    }

    fn one(&mut self, report: &Report, front: &mut Front<'_>, out: &mut Vec<Intent>) -> Result<()> {
        match *report {
            // One tick can publish several, in the order the pointer crossed them. Each is
            // applied; a sub-frame traversal is absorbed by the spring, which reaches about eight
            // percent of its ramp before the next retarget replaces it.
            Report::HoverChanged { from, to, .. } => {
                let was = from.unwrap_or(ControlId::NONE);
                self.hovered = self.live(to);
                self.observe(out);
                for id in [was, self.hovered] {
                    self.wash(id, front)?;
                }
            }
            Report::FocusChanged { to, .. } => {
                self.tapped = ControlId::NONE;
                let was = self.input_focus;
                self.focused = self.live(to);
                self.input_focus = self.focused;
                for id in [was, self.input_focus] {
                    if self.flags(id) & flag::FOCUS_WASH != 0 {
                        self.wash(id, front)?;
                    }
                }
                self.move_ring(front)?;
            }
            Report::Pressed { target, sample, .. } => {
                if self.tapped != target { self.tapped = ControlId::NONE; }
                if self.chrome.get(target).is_none() {
                    return Ok(());
                }
                self.translation_changed |= self.reorders.reset(front)?;
                front.scene.end_drag_preview();
                self.focused = ControlId::NONE;
                self.pressed = target;
                // Where the value stood when the contact landed: a turn is measured from it, and
                // a cancel puts it back. A control carrying no value grabs nothing, so its
                // canceled contact restores nothing and reports no number.
                (self.grabbed, self.grab_at) = if self.flags(target) & flag::VALUED != 0 {
                    (target, self.value(target).fraction)
                } else {
                    (ControlId::NONE, 0.0)
                };
                // A fresh contact starts undecided, whatever the last one ended as.
                (self.dragged, self.decided, self.drag_last) = (ControlId::NONE, false, None);
                self.hide_ring(front)?;
                self.wash(target, front)?;
                self.slide(target, sample.raw, How::Sprung, front, out)?;
            }
            // The thumb moves here, in this tick, before the number is queued.
            Report::Moved { target, sample, .. } if self.pressed == target => {
                self.slide(target, sample.raw, How::Carried, front, out)?;
            }
            // A turned control is a single-pointer rotation about its own centre, so its value
            // arrives as the recogniser's cumulative rotation rather than as a displacement. The
            // platform reports degrees, positive clockwise, and clockwise is more.
            Report::Gesture {
                target,
                event: Recognised::ManipulationUpdated { cumulative, .. },
                ..
            } if self.flags(target) & flag::TURN != 0 && self.grabbed == target => {
                let turned = cumulative.rotation.to_radians() / TURN_SWEEP;
                self.put(target, self.grab_at + turned, How::Carried, front, out)?;
            }
            Report::Dragged { target, update, .. } if self.flags(target) & flag::DRAGS != 0 => {
                if self.pressed != target { return Ok(()); }
                if self.flags(target) & flag::DRAG_PREVIEW != 0 {
                    if update.decided {
                        if let Some(&(_, node)) = self.previews.iter().find(|row| row.0 == target) {
                            front.scene.begin_drag_preview(node, front.back);
                            if self.reorders.contains(target) { self.reorders.begin(target, front)?; }
                        }
                    }
                    front.scene.move_drag_preview(Vector2 { x: update.delta.x, y: update.delta.y });
                }
                self.decided =
                    (self.dragged == target && self.decided) || update.phase != Phase::Undecided;
                (self.dragged, self.drag_last) = (target, Some(update));
                if self.reorders.contains(target) {
                    let (reorder, changed) = self.reorders.moved(update.at, update.decided, front)?;
                    self.translation_changed |= changed;
                    self.translation_changed |= self.reorders.follow_preview(Vector2::new(update.delta.x, update.delta.y));
                    if let Some(update) = reorder {
                        out.push(Intent { target, what: What::Reordered(update) });
                    }
                    return Ok(());
                }
                out.push(Intent {
                    target,
                    what: What::Dragged(update),
                });
            }
            Report::Released { target, at, .. } => self.end(target, Some(at), front, out)?,
            Report::Canceled { target, .. } => {
                self.tapped = ControlId::NONE;
                self.end(target, None, front, out)?;
            }
            Report::CaptureLost | Report::Dismiss { .. } => self.tapped = ControlId::NONE,
            Report::Gesture { target, event: Recognised::Tapped { at, count }, .. } =>
            {
                let hit = (at.x.is_finite() && at.y.is_finite()
                    && self.chrome.get(target).is_some_and(|row| row.flags & flag::DISABLED == 0))
                    .then(|| front.scene.hits().hit(at, windows_scene::ContactKind::Mouse))
                    .flatten().filter(|hit| hit.id == target);
                let previous = self.tapped;
                self.tapped = if count == 1 && hit.is_some() { target } else { ControlId::NONE };
                if count == 2 && previous == target {
                    if let Some(hit) = hit {
                        out.push(Intent { target, what: What::DoubleTapped(hit.local) });
                    }
                }
            }
            Report::Wheel { target: Some(target), notches, horizontal, .. }
                if notches.is_finite() && notches != 0.0 && self.dragged.is_none()
                    && self.chrome.get(target).is_some_and(|row| row.flags & flag::DISABLED == 0)
                    && front.scene.hits().entry(target).is_some_and(|hit| {
                        hit.flags.contains(HitFlags::WHEEL)
                    }) =>
            {
                out.push(Intent { target, what: What::Wheel { notches, horizontal } });
            }
            // A dial reports detents, which are a delta: a step count applied as an absolute
            // position would send one click to an end stop.
            Report::Rotary {
                target: Some(target),
                steps,
                ..
            } if self.flags(target) & flag::TURN != 0 => {
                let row = self.value(target);
                self.settle(
                    target,
                    row.fraction + steps as f32 * row.quantum(),
                    front,
                    out,
                )?;
            }
            Report::Key {
                target: Some(target),
                event,
            } if self.value(target).span > 0.0
                && event.kind == KeyKind::Down
                && !event.mods.ctrl
                && !event.mods.alt =>
            {
                if let Some(&(_, steps)) = KEYS.iter().find(|(vk, _)| *vk == event.key) {
                    let row = self.value(target);
                    self.settle(target, row.fraction + steps * row.quantum(), front, out)?;
                }
            }
            // Listed rather than matched with a wildcard, so a new `Report` variant fails to
            // compile here. None of these moves a control's chrome: they belong to the overlay
            // layer, the text stack, the scroll front and the recogniser.
            Report::Redirect { .. }
            | Report::Moved { .. }
            | Report::Buttons { .. }
            | Report::Gesture { .. }
            | Report::Dragged { .. }
            | Report::Wheel { .. }
            | Report::Key { .. }
            | Report::Escape { .. }
            | Report::Rotary { .. }
            | Report::RotaryButton { .. } => {}
        }
        Ok(())
    }

    /// Ends the contact on `target`, at the point it lifted, or nowhere when it was taken away.
    ///
    /// A release and a cancel are the same unwind with two differences — a cancel puts the value
    /// back where the contact found it and commits nothing — so they share the order in which the
    /// press, the grab and the drag are let go, and the wash is re-derived once for both.
    ///
    /// A scroll rail belongs to the scroll front and carries no chrome row. Its release must not
    /// become a menu choice or an application click.
    fn end(
        &mut self,
        target: ControlId,
        at: Option<Point>,
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        if self.pressed != target || self.chrome.get(target).is_none() {
            return Ok(());
        }
        self.pressed = ControlId::NONE;
        let grabbed = core::mem::replace(&mut self.grabbed, ControlId::NONE) == target;
        let dragged = core::mem::replace(&mut self.dragged, ControlId::NONE) == target;
        if dragged && self.decided && at.is_some() {
            self.preview_release = front.scene.drag_preview_epoch();
        } else {
            front.scene.end_drag_preview();
        }
        self.wash(target, front)?;
        // A decided drag ends here and is not also a tap: the two are the same contact, and
        // raising both would run the click handler at the end of every reorder. A canceled one
        // ends the same way carrying nothing, which is the whole of what a drag handler is told.
        if dragged && self.decided {
            if self.reorders.contains(target) {
                let update = if let Some(at) = at {
                    let (_, changed) = self.reorders.moved(at, false, front)?;
                    self.translation_changed |= changed;
                    self.reorders.released()
                } else {
                    self.translation_changed |= self.reorders.reset(front)?;
                    None
                };
                out.push(Intent { target, what: What::ReorderEnded(update) });
                return Ok(());
            }
            out.push(Intent {
                target,
                what: What::DragEnded(at.and(self.drag_last)),
            });
            return Ok(());
        }
        let Some(at) = at else {
            if grabbed {
                self.put(target, self.grab_at, How::Sprung, front, out)?;
            }
            out.push(Intent {
                target,
                what: What::Canceled(self.value(target).revision),
            });
            return Ok(());
        };
        match self.flags(target) & flag::VALUED {
            // A control that carries no value: a press and a release on it is a tap whatever moved
            // in between.
            0 => out.push(Intent {
                target,
                what: What::Tapped,
            }),
            flag::SLIDE => self.slide(target, at, How::Settled, front, out)?,
            // The fraction this table accumulated during the turn, not the range's floor.
            _ => self.put(
                target,
                self.value(target).fraction,
                How::Settled,
                front,
                out,
            )?,
        }
        Ok(())
    }

    /// Moves a value that arrived as a discrete step — a detent, an arrow key, an automation set —
    /// and reports it as moved and then settled, which is the whole of that gesture.
    fn settle(
        &mut self,
        id: ControlId,
        to: f32,
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        self.put(id, to, How::Sprung, front, out)?;
        self.put(id, to, How::Settled, front, out)
    }

    /// Clamps and snaps `to`, moves every part that follows it, and reports the value it stands
    /// for.
    ///
    /// The one path a fraction takes to become both a pixel and a number, so a pointer, a key, a
    /// dial and an automation client cannot disagree about clamping, snapping, which property
    /// carries the value or which way it runs.
    fn put(
        &mut self,
        id: ControlId,
        to: f32,
        how: How,
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        let row = self.value(id);
        let to = to.clamp(0.0, 1.0);
        let value = if row.step > 0.0 {
            row.min + (f64::from(to) * row.span / row.step).round() * row.step
        } else {
            row.min + row.span * f64::from(to)
        }
        .clamp(row.min, row.min + row.span);
        let to = if row.span > 0.0 {
            ((value - row.min) / row.span) as f32
        } else {
            to
        };
        self.drive(id, to, how, front)?;
        out.push(Intent {
            target: id,
            what: What::Scalar {
                value,
                revision: row.revision,
                commit: how == How::Settled,
            },
        });
        Ok(())
    }

    /// Moves a slid control to the pointer's position along its own rect and reports the value.
    ///
    /// The rect is the hit-array entry the router already resolved through, looked up by id, so
    /// nothing here measures or asks the app thread for geometry. Where the control has no entry
    /// the held fraction stands and nothing moves.
    fn slide(
        &mut self,
        id: ControlId,
        at: Point,
        how: How,
        front: &mut Front<'_>,
        out: &mut Vec<Intent>,
    ) -> Result<()> {
        let flags = self.flags(id);
        if flags & flag::SLIDE == 0 {
            return Ok(());
        }
        let row = self.value(id);
        let vertical = flags & flag::VERTICAL != 0;
        let Some(entry) = front.scene.hits().entry(id).copied() else {
            return self.put(id, row.fraction, how, front, out);
        };
        let (along, span) = if vertical {
            (at.y - entry.y0, entry.y1 - entry.y0)
        } else {
            (at.x - entry.x0, entry.x1 - entry.x0)
        };
        // The hit target includes the half-thumb gutters; the value range does not. A control with
        // no thumb has no gutters, so its travel is the whole span.
        let travel = if row.travel > 0.0 { row.travel } else { span };
        let over = ((along - (span - travel) * 0.5) / travel).clamp(0.0, 1.0);
        self.put(id, fraction_of(over, vertical), how, front, out)
    }

    /// Records `fraction` and moves every part of the control that follows it.
    ///
    /// A control with no parts is left alone: whatever it shows follows the application's own
    /// channel, whose writer is the app thread.
    fn drive(
        &mut self,
        id: ControlId,
        fraction: f32,
        how: How,
        front: &mut Front<'_>,
    ) -> Result<()> {
        let Some(row) = self.values.get_mut(id) else {
            return Ok(());
        };
        row.row.fraction = fraction;
        let row = row.row;
        for (node, part) in row.parts {
            for (prop, to) in part
                .channels(fraction, row.rest, row.travel)
                .into_iter()
                .flatten()
            {
                front.scalar(node, prop, to, how)?;
            }
        }
        Ok(())
    }

    /// Retargets `id`'s wash to the opacity this table's own state implies.
    ///
    /// Derived per control rather than from the event that arrived, so one control can be hovered
    /// while another is pressed — the state a drag passing under the pointer produces.
    fn wash(&self, id: ControlId, front: &mut Front<'_>) -> Result<()> {
        if let Some((sprite, opacity)) = self.wash_target(id) {
            front.spring(sprite.0, Prop::Opacity, Value::Scalar(opacity))?;
        }
        Ok(())
    }

    fn wash_target(&self, id: ControlId) -> Option<(SpriteId, f32)> {
        let row = self.chrome_of(id);
        if row.wash.0.is_none() {
            return None;
        }
        let to = if row.flags & flag::DISABLED != 0 {
            0.0
        } else if self.input_focus == id && row.flags & flag::FOCUS_WASH != 0 {
            1.0
        } else if self.pressed == id {
            row.press
        } else if self.hovered == id {
            row.hover
        } else {
            0.0
        };
        Some((row.wash, to))
    }

    /// Raises scope entry and exit for the scopes that asked for them.
    fn observe(&mut self, out: &mut Vec<Intent>) {
        let scope = self.chrome_of(self.hovered).scope;
        let next = if self.flags(scope) & flag::OBSERVES != 0 {
            scope
        } else {
            ControlId::NONE
        };
        if next == self.observed {
            return;
        }
        for (target, entered) in [(self.observed, false), (next, true)] {
            if !target.is_none() {
                out.push(Intent {
                    target,
                    what: What::Hovered(entered),
                });
            }
        }
        self.observed = next;
    }

    /// Fades the reveal target of every active scope, and only where that changed.
    ///
    /// Crossing two children of one scope leaves it active, so the fade is not restarted. The fade
    /// is a spring rather than a two-keyframe hold, because step easing takes a segment's end
    /// value immediately and would jump at the start of the hold rather than at its end.
    fn reveals(&mut self, front: &mut Front<'_>) -> Result<()> {
        let mut next = [ControlId::NONE; 3];
        for (slot, source) in [self.hovered, self.pressed, self.focused]
            .into_iter()
            .enumerate()
        {
            let scope = self.chrome_of(source).scope;
            if !self.chrome_of(scope).reveal.is_none() && !next.contains(&scope) {
                next[slot] = scope;
            }
        }
        for (these, others, to) in [(self.revealed, next, 0.0), (next, self.revealed, 1.0)] {
            for id in these {
                let node = self.chrome_of(id).reveal;
                if !node.is_none() && !others.contains(&id) {
                    front.spring(node, Prop::Opacity, Value::Scalar(to))?;
                }
            }
        }
        self.revealed = next;
        self.translate(front)
    }

    /// Springs the window's one ring onto the focused control, or takes it down.
    fn move_ring(&mut self, front: &mut Front<'_>) -> Result<()> {
        let Some(entry) = front.scene.hits().entry(self.focused).copied() else {
            return self.hide_ring(front);
        };
        if self.ring.is_none() {
            return Ok(());
        }
        front.scene.raise_overlay(self.ring);
        let scroll = if entry.flags.contains(HitFlags::UNSCROLLED) {
            Vector2::default()
        } else {
            front.scene.hits().offset(entry.scroll_src)
        };
        let shift = front.scene.hits().translation(self.focused);
        let at = Vector2 {
            x: entry.x0 - scroll.x + shift.x - FOCUS_OUTSET,
            y: entry.y0 - scroll.y + shift.y - FOCUS_OUTSET,
        };
        let size = Vector2 {
            x: entry.x1 - entry.x0 + 2.0 * FOCUS_OUTSET,
            y: entry.y1 - entry.y0 + 2.0 * FOCUS_OUTSET,
        };
        let Some((at, size)) = focus_box(at, size, self.viewport) else {
            return self.hide_ring(front);
        };
        if self.ring_shown {
            front.spring(self.ring, Prop::Offset, Value::Vec2(at))?;
            front.spring(self.ring, Prop::Size, Value::Vec2(size))?;
        } else {
            // The outline's nine-grid needs a nonzero box before its first reveal.
            for (prop, value) in [(Prop::Offset, at), (Prop::Size, size)] {
                front
                    .scene
                    .retarget(self.ring, prop, Bind::Set(Value::Vec2(value)), front.back)?;
            }
            self.ring_shown = true;
            front.spring(self.ring, Prop::Opacity, Value::Scalar(1.0))?;
        }
        Ok(())
    }

    fn hide_ring(&mut self, front: &mut Front<'_>) -> Result<()> {
        if self.ring.is_none() || !self.ring_shown {
            return Ok(());
        }
        self.ring_shown = false;
        front.spring(self.ring, Prop::Opacity, Value::Scalar(0.0))
    }

    /// Returns `id` where it still has a row, and `NONE` otherwise.
    fn live(&self, id: Option<ControlId>) -> ControlId {
        id.filter(|id| self.chrome.get(*id).is_some())
            .unwrap_or(ControlId::NONE)
    }

    fn chrome_of(&self, id: ControlId) -> ChromeRow {
        self.chrome.get(id).copied().unwrap_or_default()
    }

    fn flags(&self, id: ControlId) -> u8 {
        self.chrome_of(id).flags
    }

    /// Returns the value half of `id`, or an all-zero row for a control no value moves.
    ///
    /// A zero row has zero span, so a value read off it is the range floor and moves no part.
    fn value(&self, id: ControlId) -> ValueRow {
        self.values.get(id).map(|v| v.row).unwrap_or_default()
    }
}

fn focus_box(at: Vector2, size: Vector2, viewport: Option<Vector2>) -> Option<(Vector2, Vector2)> {
    let Some(viewport) = viewport else { return Some((at, size)); };
    let end = Vector2::new((at.x + size.x).min(viewport.x), (at.y + size.y).min(viewport.y));
    let start = Vector2::new(at.x.max(0.0), at.y.max(0.0));
    (end.x > start.x && end.y > start.y).then_some((start, end - start))
}

#[cfg(test)]
#[path = "scalar_tests.rs"]
mod scalar_tests;

#[cfg(test)]
#[path = "reveal_tests.rs"]
mod reveal_tests;

#[cfg(test)]
#[path = "wheel_tests.rs"]
mod wheel_tests;

#[cfg(test)]
#[path = "double_tap_tests.rs"]
mod double_tap_tests;

#[cfg(test)]
#[path = "focus_bounds_tests.rs"]
mod focus_bounds_tests;
