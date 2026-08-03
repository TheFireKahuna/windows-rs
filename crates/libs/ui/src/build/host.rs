//! What survives a mount: the model, the recipes, and the dense control table.
//!
//! The arena clears after every mount, so anything the lowering must be able to *redo* lives
//! here — the style recipe per node, because a width class is decided in the solve and moves
//! on resize; the control row per interactive node, because the hover path must be an array
//! index rather than a hash lookup.
//!
//! A `Host::with` body must not call application code. [`Effect::new`](crate::signal::Effect::new)
//! runs its closure immediately, so an effect created under the borrow would re-enter it:
//! mount releases the borrow around every effect it installs, and an effect body takes a
//! fresh one. That is also what keeps an effect's captures to `Copy` ids, with no
//! `Rc<RefCell<Model>>` to clone per binding.

use crate::gesture::GestureDecl;
use crate::role::{Role, Scope};
use crate::widget::{Chrome, ModelState, TextSource, UiaRole};
use std::cell::RefCell;
use std::rc::Rc;
use windows_numerics::Vector2;
use windows_scene::{
    ControlId, Env, Exit, GroupId, Id, Ids, MeasureIn, MeasureKey, Model, NodeId, Paint, Prop,
    SinkPatch, Slots, SpriteId,
};

/// The families this layer mints, and the ids that name their rows.
///
/// Markers rather than the row types themselves, for the reason `windows-scene` uses them:
/// the family is what an id belongs to, and one family can have more than one store — a
/// control has a row here and a row on the front thread, over one set of ids.
#[derive(Debug)]
pub(crate) struct Mount;
#[derive(Debug)]
pub(crate) struct Value;
#[derive(Debug)]
pub(crate) struct Scroll;

pub(crate) type MountId = Id<Mount>;
pub(crate) type ValueId = Id<Value>;
pub(crate) type ScrollId = Id<Scroll>;

/// One mounted node, and everything it has to release.
pub(crate) struct MountRow {
    pub node: NodeId,
    /// The next row of the same mounted subtree, or [`Id::NONE`]. A chain and not a `Vec`, so
    /// a list row realized during a fling records its rows without allocating.
    ///
    /// A link is an **id** and not a bare index, which is what keeps a walk on the checked
    /// path: an index would reach a row without asking whether it is still the row that was
    /// linked. `Id::NONE` is the terminator, which is what it exists for.
    pub next: MountId,
    pub control: Option<ControlId>,
    pub text: Option<MeasureKey>,
    /// Everything else this node claimed, held **here** rather than searched for.
    ///
    /// The unmount walks its own subtree's rows and releases exactly what they name. Scanning
    /// the value and scroll tables instead made unmounting one row of a long list cost the
    /// whole screen, which is the opposite of what a keyed list is for.
    pub values: ValueId,
    pub scroll: Option<ScrollId>,
}

/// One interactive node, addressed by the index inside its [`ControlId`].
///
/// Split from its front-side half by **what each thread can do with it**: the handlers stay
/// here because only this thread can call them, and everything in [`front`](Self::front) goes
/// there because only that thread is in the tick that has to move a pixel. Nothing crosses
/// that is not a number or an id — the wash opacities are resolved here, at mount, because
/// realizing a colour cell mid-hover would be a surface creation on the interaction path.
pub(crate) struct ControlRow {
    pub node: NodeId,
    /// The parts a model-state change re-paints. Held as ids so the swap is two ops and no
    /// search.
    pub fill: Option<SpriteId>,
    pub label: Option<SpriteId>,
    pub border: Option<SpriteId>,
    /// The front thread's own half of this control, held rather than only sent.
    ///
    /// One copy of the wash ids, the alphas and the travel, so the two sides cannot disagree
    /// about what a control is — and so a solve that changed a control's room can re-send a
    /// corrected row instead of reconstructing one.
    pub front: crate::widget::ChromeRow,
    /// The table row this control's colours come from, so a state change re-reads the same
    /// row rather than remembering what it painted.
    pub chrome: Option<Chrome>,
    pub scope: Scope,
    pub state: ModelState,
    pub click: Option<Box<dyn Fn()>>,
    pub change: Option<Box<dyn Fn(f64)>>,
    pub commit: Option<Box<dyn Fn(f64)>>,
    /// Both are `Rc` rather than `Box` because the overlay layer has to take them *out* of
    /// the host's borrow before running them: building a flyout's body is application code,
    /// and it cannot run while the host is borrowed. They stay in the row, because a
    /// picker's flyout opens once per press and not once per lifetime.
    pub tip: Option<Rc<TextSource>>,
    pub flyout: Option<Rc<dyn Fn() -> super::View>>,
    pub uia: UiaRole,
    pub name: Option<&'static str>,
    /// The text this control's own subtree laid out, which is what its accessible name
    /// derives from where it was not given one. A control's label is almost never its own
    /// sprite, so this is claimed on the way back up rather than read off its own row.
    pub text: Option<MeasureKey>,
    /// The automation-id segment. `&'static str`, so nothing is built at mount: the path is
    /// materialized only if UI Automation asks, which is rarely and never on a hot path.
    pub key: Option<&'static str>,
}

/// One moving part, and the room it has.
///
/// A fraction cannot be lowered at mount, because the room is a **layout output** — the
/// track's extent less the part's own. So the fraction is kept here and multiplied out after
/// the solve, which is the split [`publish_scrolls`](Host::publish_scrolls) already makes for
/// the same reason. It is also what lets the front thread move the same part from the same
/// number without asking this thread for geometry.
pub(crate) struct ValueRow {
    /// The part that moves.
    pub node: NodeId,
    /// The box it moves in — the enclosing control, filled in when that control mounts.
    pub track: NodeId,
    pub control: Option<ControlId>,
    /// Which unit the fraction is finished in: along a track, or around a sweep.
    pub unit: crate::build::arena::Unit,
    pub prop: Prop,
    pub motion: crate::widget::Motion,
    pub vertical: bool,
    /// The last fraction anybody published, whether the app's channel or the router's.
    pub fraction: f32,
    /// The travel it was last published against, so a solve that moved nothing emits
    /// nothing.
    pub travel: f32,
    /// Whether the **router** moves this part.
    ///
    /// The one arbiter of a channel with two possible writers. When it is set this thread
    /// never binds the property: a solve that changed the room re-sends the room, and the
    /// router re-drives the part from the fraction it already holds — which is the newer of
    /// the two. Without that split, correcting a resize would fight a drag.
    pub front_driven: bool,
    pub row: MountId,
    /// The next value row of the same mount row, or [`Id::NONE`].
    pub next: ValueId,
}

impl ValueRow {
    /// This row's fraction, in the property's own unit.
    ///
    /// The app-thread twin of the router's `drive`, and both reach the same two functions —
    /// so a slid part and a turned one cannot disagree about which way their value runs.
    fn number(&self) -> f32 {
        match self.unit {
            crate::build::arena::Unit::Turn => crate::widget::angle_of(self.fraction),
            _ => crate::widget::offset_of(self.fraction, self.travel, self.vertical),
        }
    }
}

/// One open overlay's placement rule, and where it last landed.
///
/// Here rather than on the overlay layer's own row for the reason [`Responsive`] and
/// [`ScrollRow`] are: resolving it needs the solve — the overlay's measured size and its
/// anchor's rect — and this is what holds the solve. The layer above owns the overlay's
/// *lifetime*; this owns its *geometry*, and only this writes the model.
pub(crate) struct Placement {
    pub root: GroupId,
    pub anchor: crate::overlay::Anchor,
    /// What was last published, so a pass that moved nothing emits nothing.
    pub at: Vector2,
}

/// The app thread's half of the widget layer.
pub struct Host {
    pub(crate) model: Model,
    pub(crate) env: Env,
    pub(crate) root_scope: Scope,
    /// The four families this thread mints, each an authority beside the store it fills.
    ///
    /// Held apart rather than fused into one table type, because a store keyed by ids
    /// somebody *else* mints — the recipe table, the front thread's chrome — must have no
    /// authority at all. Where a field of `Ids` sits beside a store, this layer owns the
    /// counter; where it does not, it does not.
    pub(crate) mount_ids: Ids<Mount>,
    pub(crate) mounts: Slots<Mount, MountRow>,
    pub(crate) control_ids: Ids<windows_scene::Control>,
    pub(crate) controls: Slots<windows_scene::Control, ControlRow>,
    /// What each target declared about the gestures it accepts, drained by whoever owns the
    /// router. Front-resident from then on: no call is made to this thread to decide
    /// whether a gesture applies.
    pub(crate) gestures: Vec<(ControlId, GestureDecl)>,
    /// The front-side half of each control minted — or re-measured — since the last drain.
    pub(crate) chrome: Vec<crate::widget::ChromeRow>,
    /// Model-state changes since the last drain, for automation.
    pub(crate) states: Vec<(ControlId, ModelState)>,
    /// Whether the set of elements has moved since the last accessible-tree publish.
    ///
    /// A `Cell`, because clearing it is the front side saying "I have published" rather
    /// than this thread mutating itself, and the two happen either side of a drain.
    pub(crate) uia_stale: std::cell::Cell<bool>,
    /// Controls released since the last drain, so the front table forgets them rather than
    /// holding a row that names a destroyed sprite.
    pub(crate) released: Vec<ControlId>,
    /// Moving parts awaiting the travel only a solve can give them.
    pub(crate) value_ids: Ids<Value>,
    pub(crate) values: Slots<Value, ValueRow>,
    /// Trackers minted here but **created** on the front thread, because an
    /// `InteractionTracker` is a composition object and its source is a visual.
    pub(crate) trackers: Vec<TrackerSpec>,
    pub(crate) scroll_ids: Ids<Scroll>,
    pub(crate) scrolls: Slots<Scroll, ScrollRow>,
    /// Which control is which window command, for the caption band to resolve a point
    /// through. Filled at mount by [`El::caption`](super::El::caption).
    pub(crate) caption: crate::caption::Registry,
    /// Open overlays, in the order they opened. **A stack, not a slotted table**: overlays
    /// genuinely nest — a submenu is above its menu and cannot outlive it — so closing one
    /// takes everything above it, and an index is stable for exactly as long as that holds.
    pub(crate) overlays: Vec<Placement>,
}

/// A tracker this thread named and the other thread has to build.
#[derive(Copy, Clone, Debug)]
pub struct TrackerSpec {
    pub id: windows_scene::TrackerId<windows_scene::Observed>,
    pub viewport: GroupId,
    pub axes: windows_scene::Axes,
}

/// One scroll container, as the post-solve step needs it.
pub(crate) struct ScrollRow {
    pub tracker: windows_scene::TrackerId<windows_scene::Observed>,
    pub viewport: NodeId,
    pub content: NodeId,
    pub thumb: Option<SpriteId>,
    /// What was last published, so a solve that moved nothing emits nothing.
    pub last: crate::layout::ThumbGeom,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

/// Why the host could not be reached. Three causes with three different fixes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Access {
    /// Nothing was installed.
    NoHost,
    /// A `Host::with` body reached back in.
    Reentrant,
    /// The thread is tearing its locals down. Not an error at all in the one place it
    /// happens.
    Gone,
}

impl Access {
    const fn message(self) -> &'static str {
        match self {
            Self::NoHost => {
                "a host must be installed before anything mounts: call \
                 windows_ui::build::Host::install once at start-up"
            }
            Self::Reentrant => {
                "a Host::with body reached back into the host: it must not call application \
                 code, and an effect it creates runs immediately"
            }
            Self::Gone => "the host's thread is being torn down",
        }
    }
}

impl Host {
    /// Installs the app thread's host, and wires the two solve-time seams into the model.
    ///
    /// Both closures capture **nothing**. They have to be `Send`, and each reaches its table
    /// through that table's own thread-local — which is also what lets the text table hold
    /// laid-out runs, since a run is thread-affine and an `Arc<Mutex<..>>` of one would not
    /// compile. Reaching the *host* from either would re-enter a borrow the solve is inside.
    pub fn install(mut model: Model, env: Env, root_scope: Scope) {
        model.on_measure(|input: MeasureIn| super::text::measure(input));
        model.on_restyle(|node, class| super::style::restyle(node, class));
        let host = Self {
            model,
            env,
            root_scope,
            mount_ids: Ids::new(),
            mounts: Slots::new(),
            control_ids: Ids::new(),
            controls: Slots::new(),
            gestures: Vec::new(),
            chrome: Vec::new(),
            states: Vec::new(),
            uia_stale: std::cell::Cell::new(true),
            released: Vec::new(),
            value_ids: Ids::new(),
            values: Slots::new(),
            trackers: Vec::new(),
            scroll_ids: Ids::new(),
            scrolls: Slots::new(),
            caption: crate::caption::Registry::default(),
            overlays: Vec::new(),
        };
        HOST.with(|slot| *slot.borrow_mut() = Some(host));
    }

    /// Runs `f` against the thread's host.
    ///
    /// **`f` must not call application code.** See the module rule.
    ///
    /// # Panics
    ///
    /// If no host is installed, or if `f` re-enters — and it says **which**. The two have
    /// opposite fixes, and one message for both sends the reader to the wrong one.
    pub fn with<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        match Self::access(f) {
            Ok(out) => out,
            Err(why) => panic!("{}", why.message()),
        }
    }

    /// The same, answering `None` where there is nothing to reach rather than panicking.
    ///
    /// For the one caller that cannot panic: a [`Mount`](super::Mount) is dropped by
    /// whatever scope owned it, and at thread teardown that scope is itself a thread-local
    /// being destroyed. Reaching a thread-local **during** its own destruction phase is an
    /// error rather than a value, and `with` answers it by panicking — inside a `Drop`,
    /// which aborts the process. So this asks the fallible way and treats a host that is
    /// already gone as nothing left to release, which it is.
    ///
    /// Re-entry is a different matter and is asserted: dropping a mount from inside a
    /// `Host::with` body would leak the whole subtree, and silence is the one outcome that
    /// makes that impossible to find.
    pub fn try_with<R>(f: impl FnOnce(&mut Self) -> R) -> Option<R> {
        match Self::access(f) {
            Ok(out) => Some(out),
            Err(why) => {
                debug_assert!(why != Access::Reentrant, "{}", why.message());
                None
            }
        }
    }

    fn access<R>(f: impl FnOnce(&mut Self) -> R) -> Result<R, Access> {
        HOST.try_with(|slot| {
            let mut slot = slot.try_borrow_mut().map_err(|_| Access::Reentrant)?;
            slot.as_mut().map(f).ok_or(Access::NoHost)
        })
        .unwrap_or(Err(Access::Gone))
    }

    /// Whether a host is installed. What a diagnostic asks; nothing else needs it.
    #[must_use]
    pub fn installed() -> bool {
        HOST.with(|slot| slot.borrow().is_some())
    }

    /// Takes what has mounted since the last drain, for the router to declare.
    pub fn take_gestures(&mut self) -> Vec<(ControlId, GestureDecl)> {
        core::mem::take(&mut self.gestures)
    }

    /// Takes the front-side rows minted since the last drain, to be adopted alongside the
    /// patch.
    ///
    /// The two halves of a control are split by **what each thread can do with them**: the
    /// handlers stay here because only this thread can call them, and the opacities go there
    /// because only that thread is in the tick that has to move a pixel. Nothing crosses that
    /// is not a number or an id, which is what keeps the patch's `Send` proof intact.
    pub fn take_chrome(&mut self) -> Vec<crate::widget::ChromeRow> {
        core::mem::take(&mut self.chrome)
    }

    /// Fills `out` with what automation needs and layout does not already carry.
    ///
    /// **Synthesised, never declared.** A widget names a role and the lowering derives the
    /// rest: the name is the widget's own text unless it was given one, the value is the
    /// channel it already binds, and the patterns follow from the role. Nineteen
    /// hand-written declarations would be nineteen chances to disagree with a promise that
    /// accessibility derives.
    ///
    /// Walks the mount rows rather than the control table, because a control's own text
    /// and its control row are two facts about one row and this is where they meet. The
    /// strings are resolved *here*, on the thread that owns the text table, so what
    /// crosses to the front is plain `Send` data.
    pub fn uia_seeds(&self, out: &mut crate::uia::Seeds) {
        use crate::uia::{ColFlags, Seed, State, Value};

        out.clear();
        for (id, control) in self.controls.iter() {
            if control.uia == UiaRole::None {
                continue;
            }
            let name = match control.name {
                Some(explicit) => out.intern(explicit),
                // A `&'static str` is interned rather than borrowed because a derived name
                // is not one, and one path for both beats two.
                None => control
                    .text
                    .and_then(|key| super::text::with(|table| table.str_of(key).map(str::to_owned)))
                    .map_or_else(Default::default, |text| out.intern(&text)),
            };
            // A tooltip is the element's `HelpText`, which is the same fact stated to a
            // different sense. Read untracked: this runs inside a flush, and subscribing
            // whatever effect is on the stack would rebuild a screen when a tip changed.
            let help = control.tip.as_ref().map_or_else(Default::default, |tip| {
                let text = crate::signal::untracked(|| tip.read(str::to_owned));
                out.intern(&text)
            });
            let value = match (control.uia, control.front.drive) {
                (
                    _,
                    Some(
                        crate::widget::Interaction::Slide(range)
                        | crate::widget::Interaction::Turn(range),
                    ),
                ) => Value::Range(range),
                // A static run publishes its own body as a text document, which is what
                // makes the read-only selectable surface readable at all.
                (UiaRole::Text, _) => Value::Text,
                _ => Value::None,
            };
            let mut flags = ColFlags::NONE;
            if control.flyout.is_some() {
                flags = flags | ColFlags::EXPANDS;
            }
            if control.click.is_some() || control.front.drive.is_some() {
                flags = flags | ColFlags::FOCUSABLE;
            }
            let mut state = State::default();
            if control.state != ModelState::Disabled {
                state = state | State::ENABLED;
            }
            if control.state == ModelState::Selected {
                state = state | State::SELECTED;
            }
            out.rows.push(Seed {
                id,
                role: control.uia,
                name,
                help,
                key: control.key,
                value,
                flags,
                state,
            });
        }
        out.sort();
    }

    /// Takes the model-state changes since the last drain, for automation to announce.
    ///
    /// A drain rather than a republish: a toggle is not a layout change, and rebuilding the
    /// whole tree to say one box is now checked would put an allocation on every click and
    /// tell a client the screen's structure changed when it did not.
    pub fn take_states(&mut self) -> Vec<(ControlId, ModelState)> {
        core::mem::take(&mut self.states)
    }

    /// Whether the accessible tree needs rebuilding.
    ///
    /// Set when a control is minted or released, which is exactly when the *set* of
    /// elements moves. Everything else a client can observe — a value, a state, focus, a
    /// scroll offset — reaches it without one.
    pub fn uia_stale(&self) -> bool {
        self.uia_stale.get()
    }

    /// Clears the flag, for the caller that has just republished.
    pub fn uia_published(&self) {
        self.uia_stale.set(false);
    }

    /// Marks the accessible tree stale without minting anything.
    ///
    /// What a changed string does: a name is copied into the published blob, so a label
    /// that re-reads leaves the tree holding the old one.
    pub(crate) fn uia_restale(&self) {
        self.uia_stale.set(true);
    }

    /// Takes the controls released since the last drain, for the front table to forget.
    ///
    /// The generational id already makes a stale report a miss, so this is not what keeps
    /// the front side *correct* — it is what keeps it **bounded**: without it a screen that
    /// churns leaves a row per control that ever existed.
    pub fn take_released(&mut self) -> Vec<ControlId> {
        core::mem::take(&mut self.released)
    }

    /// Takes the trackers minted since the last drain, for the front thread to create.
    pub fn take_trackers(&mut self) -> Vec<TrackerSpec> {
        core::mem::take(&mut self.trackers)
    }

    /// Calls what an intent asks for.
    ///
    /// A stale intent — one queued before the control unmounted — is a **miss**: the
    /// generation half of the id no longer matches, so this is a bounds-checked index that
    /// finds nothing rather than a call into whatever now occupies the slot.
    pub fn dispatch(&mut self, intents: &[crate::widget::Intent]) {
        for intent in intents {
            let Some(control) = self.control(intent.target) else {
                continue;
            };
            match intent.what {
                crate::widget::What::Tapped => {
                    if let Some(click) = control.click.as_ref() {
                        click();
                    }
                }
                crate::widget::What::Changed(v) => {
                    if let Some(change) = control.change.as_ref() {
                        change(v);
                    }
                }
                crate::widget::What::Committed(v) => {
                    if let Some(commit) = control.commit.as_ref() {
                        commit(v);
                    }
                }
            }
        }
    }

    // ── identity ──────────────────────────────────────────────────────────────────

    pub(crate) fn mint_mount(&mut self, row: MountRow) -> MountId {
        self.mounts.insert(&mut self.mount_ids, row)
    }

    /// Records a scroll container against the mount row that owns it, whose tracker dies
    /// with it.
    pub(crate) fn mint_scroll(&mut self, row: MountId, scroll: ScrollRow) {
        let at = self.scrolls.insert(&mut self.scroll_ids, scroll);
        if let Some(row) = self.mounts.get_mut(row) {
            row.scroll = Some(at);
        }
    }

    /// Mints a control row and its identity.
    ///
    /// The generation half is what makes a **stale intent** — one queued before an unmount
    /// — a miss rather than a call into whatever now occupies the slot.
    pub(crate) fn mint_control(&mut self, control: ControlRow) -> ControlId {
        self.uia_stale.set(true);
        self.controls.insert(&mut self.control_ids, control)
    }

    /// The control a routed hit or an arriving intent names, or `None` if it is stale.
    pub(crate) fn control(&self, id: ControlId) -> Option<&ControlRow> {
        self.controls.get(id)
    }

    pub(crate) fn control_mut(&mut self, id: ControlId) -> Option<&mut ControlRow> {
        self.controls.get_mut(id)
    }

    /// Releases a control, dropping the handlers it captured with it.
    ///
    /// The front table is told separately: the generational id already makes a stale report
    /// a miss, so this is not what keeps that side *correct* — it is what keeps it bounded.
    fn release_control(&mut self, id: ControlId) {
        if self.controls.remove(&mut self.control_ids, id).is_some() {
            self.released.push(id);
            self.uia_stale.set(true);
        }
    }

    // ── values ────────────────────────────────────────────────────────────────────

    /// Opens a value row for a moving part, before the control that owns it is known.
    ///
    /// Threaded onto its mount row's chain in the same breath, so the unmount releases it
    /// without searching the table.
    pub(crate) fn mint_value(&mut self, row: ValueRow) -> ValueId {
        let mount = row.row;
        let head = self
            .mounts
            .get(mount)
            .map_or(ValueId::NONE, |mount| mount.values);
        let id = self
            .values
            .insert(&mut self.value_ids, ValueRow { next: head, ..row });
        if let Some(mount) = self.mounts.get_mut(mount) {
            mount.values = id;
        }
        id
    }

    fn value_mut(&mut self, id: ValueId) -> Option<&mut ValueRow> {
        self.values.get_mut(id)
    }

    /// Names the control a moving part belongs to, and which thread moves it. Known only
    /// once that control mounts, which is after the part itself did.
    pub(crate) fn own_value(
        &mut self,
        id: ValueId,
        control: ControlId,
        track: NodeId,
        front_driven: bool,
    ) {
        if let Some(value) = self.value_mut(id) {
            value.control = Some(control);
            value.track = track;
            value.front_driven = front_driven;
        }
    }

    /// Publishes a fraction through whatever finishes it: the travel the last solve gave a
    /// slid part, or the constant sweep of a turned one.
    ///
    /// **The one place a `0..=1` becomes a property on this thread**, matching the one place
    /// it becomes one on the other. A part the router drives is skipped outright — it is not
    /// this thread's to write, and writing it would be the two halves fighting over one
    /// channel.
    pub(crate) fn set_fraction(&mut self, id: ValueId, fraction: f32) {
        let Some(value) = self.value_mut(id) else {
            return;
        };
        value.fraction = fraction.clamp(0.0, 1.0);
        if value.front_driven {
            return;
        }
        let (node, prop, motion) = (value.node, value.prop, value.motion);
        let number = value.number();
        self.bind_number(node, prop, motion, number);
    }

    fn bind_number(
        &mut self,
        node: NodeId,
        prop: Prop,
        motion: crate::widget::Motion,
        number: f32,
    ) {
        let value = windows_scene::Value::Scalar(number);
        self.model.bind(
            node,
            prop,
            match motion {
                crate::widget::Motion::Snap => windows_scene::Bind::Set(value),
                crate::widget::Motion::Chrome => {
                    windows_scene::Bind::Animate(windows_scene::Anim::Spring {
                        to: value,
                        tuning: windows_scene::Tuning::Chrome,
                        delay_ms: 0,
                    })
                }
            },
        );
    }

    /// Re-multiplies every slid part against the room the solve just measured for it.
    ///
    /// Post-solve for the reason shaped text and scroll extents are: the room is the track's
    /// box less the part's own, and neither exists until layout has said so. It **snaps**
    /// rather than springs — this is a geometry correction, not a value change, and a window
    /// resize must not make every thumb on the screen animate.
    ///
    /// A part the router drives is corrected by **shipping it the new room**, never by
    /// binding the property: the front side re-drives from the fraction it holds, which is
    /// the newer of the two. That is what keeps one writer per channel with the app and the
    /// router both interested in the same one. A turned part has no room and appears here at
    /// all only because its sweep is constant, so it is left alone entirely.
    fn publish_values(&mut self) {
        // By position, and re-resolved through the id each step: the body reaches back into
        // the model, which is a second field of `self`, so it cannot hold a borrow of the
        // table across the walk. What a position yields is an **id**, so every touch of a
        // row is still checked.
        for at in self.values.positions() {
            let Some(id) = self.values.id_at(at) else {
                continue;
            };
            let Some(value) = self.values.get(id) else {
                continue;
            };
            if value.unit != crate::build::arena::Unit::Travel {
                continue;
            }
            let (node, track, vertical) = (value.node, value.track, value.vertical);
            let axis = |v: Vector2| if vertical { v.y } else { v.x };
            let travel =
                (axis(self.model.solved(track).size) - axis(self.model.solved(node).size)).max(0.0);
            // Exact: `travel` is recomputed from the same two rects, so anything that moved
            // at all is a different float and a tolerance would only hide small real moves.
            if travel == value.travel {
                continue;
            }
            let (prop, fraction, control, front_driven) = (
                value.prop,
                value.fraction,
                value.control,
                value.front_driven,
            );
            if let Some(value) = self.values.get_mut(id) {
                value.travel = travel;
            }
            if !front_driven {
                self.bind_number(
                    node,
                    prop,
                    crate::widget::Motion::Snap,
                    crate::widget::offset_of(fraction, travel, vertical),
                );
            }
            if let Some(id) = control
                && let Some(control) = self.control_mut(id)
            {
                control.front.travel = travel;
                let front = control.front;
                self.chrome.push(front);
            }
        }
    }

    // ── model state ───────────────────────────────────────────────────────────────

    /// Puts a control into a model state, re-resolving exactly the parts it changes.
    ///
    /// `None` is a return to rest. Selection and disablement are **not** washes: they are
    /// discrete paint swaps at event rate, so they go through the model rather than through
    /// a retarget, and they read the same table row the mount painted from.
    pub(crate) fn set_state(&mut self, id: ControlId, state: Option<ModelState>) {
        let state = state.unwrap_or(ModelState::Rest);
        let Some(control) = self.control(id) else {
            return;
        };
        if control.state == state {
            return;
        }
        let chrome = control.chrome;
        // Recorded whether or not there is anything to repaint: what a state change means
        // to a screen reader — checked, selected, unavailable — is independent of whether
        // the control draws a difference, and a control with no chrome still has a peer.
        self.states.push((id, state));
        let Some(control) = self.control(id) else {
            return;
        };
        let Some(chrome) = chrome else {
            // Nothing to swap: a control with no chrome row has no base paint of its own.
            if let Some(control) = self.control_mut(id) {
                control.state = state;
            }
            return;
        };
        let roles = chrome.roles().in_state(state);
        let scope = control.scope.for_paint();
        let (fill, label, border) = (control.fill, control.label, control.border);
        if let Some(control) = self.control_mut(id) {
            control.state = state;
        }
        paint(&mut self.model, fill, roles.fill.map(Role::Fill), scope);
        paint(&mut self.model, label, Some(Role::Text(roles.text)), scope);
        paint(
            &mut self.model,
            border,
            roles.stroke.map(Role::Stroke),
            scope,
        );
    }

    // ── unmount ───────────────────────────────────────────────────────────────────

    /// Destroys a mounted subtree and releases every row it claimed.
    ///
    /// One destroy op, because it cascades on the far side. Everything else is walked through
    /// the chain the mount threaded, and each row releases what it *names* — so this is
    /// proportional to the subtree and touches nothing else, which is what makes unmounting
    /// one row of a long list cheap.
    pub(crate) fn unmount(&mut self, node: NodeId, exit: Exit, rows: MountId) {
        let mut at = rows;
        while let Some(row) = self.mounts.remove(&mut self.mount_ids, at) {
            // Fallibly, for the reason `try_with` gives: this can run while the thread is
            // tearing its locals down, and the tables are going the same way anyway.
            super::style::try_with(|table| {
                table.take(row.node);
            });
            if let Some(id) = row.control {
                self.release_control(id);
            }
            if let Some(key) = row.text {
                super::text::try_with(|table| table.release(key, &mut self.model));
            }
            let mut value = row.values;
            while let Some(row) = self.values.remove(&mut self.value_ids, value) {
                value = row.next;
            }
            // A tracker outliving its viewport would be a compositor object with nothing to
            // be sourced from, so it is dropped with the row that named it.
            if let Some(scroll) = row
                .scroll
                .and_then(|at| self.scrolls.remove(&mut self.scroll_ids, at))
            {
                self.model.drop_tracker(scroll.tracker);
            }
            at = row.next;
        }
        self.model.destroy(node, exit);
    }

    // ── the flush ─────────────────────────────────────────────────────────────────

    /// Solves, settles what only a solve can decide, and hands the patch over.
    ///
    /// Three phases, each solving only what the one before it moved. A pass that changed
    /// nothing solves nothing, so the second and third are free in the steady state.
    ///
    /// 1. **Solve.** The width class resolves *inside* this, and the styles it implies are
    ///    re-lowered through the restyle seam before layout runs on them — so a container
    ///    that crossed a threshold needs no correcting pass above.
    /// 2. **Publish geometry.** Shaped runs, scroll extents and value travel are all
    ///    functions of solved boxes, so they cannot be stated before one. Publishing can
    ///    move a wrapping run's line boxes and a thumb's style, which is what the re-solve
    ///    is for.
    /// 3. **Place overlays.** After the publishes and not beside them: a menu's width is its
    ///    labels', and the labels are placed in phase 2. This terminates rather than
    ///    iterating, because placement moves an overlay and never resizes one — the third
    ///    solve computes the same size the second did.
    pub fn flush(&mut self, patch: &mut SinkPatch) {
        let env = self.env;
        self.model.solve(env);
        if self.publish_geometry() {
            self.model.solve(env);
        }
        if self.place_overlays() {
            self.model.solve(env);
        }
        self.model.flush(patch, env);
    }

    /// Everything whose value is a function of the solve, and whether any of it moved a box.
    fn publish_geometry(&mut self) -> bool {
        let text = super::text::with(|table| table.publish(&mut self.model));
        let scrolls = self.publish_scrolls();
        // Values bind compositor properties and dirty no layout, so they are published here
        // for ordering and contribute nothing to whether a re-solve is owed.
        self.publish_values();
        text | scrolls
    }

    // ── overlays ──────────────────────────────────────────────────────────────────

    /// **The one call site of `Model::orphan_group` outside `windows-scene`.**
    ///
    /// A parentless root is invisible to a parent walk, so it has to be reachable by the
    /// disposal walk instead — and it is reachable exactly because opening it is what puts
    /// it in the array that walk reads. Minting and opening in one call is what keeps that
    /// true: there is no moment at which an un-opened one exists.
    pub(crate) fn open_overlay_slot(&mut self, blocker: Option<ControlId>) -> GroupId {
        let root = self.model.orphan_group();
        self.model.open_slot(root, blocker)
    }

    /// Removes a slot root from the array and releases its blocker's row. The subtree is
    /// destroyed by the mount going out of scope, which is where its exit transition is.
    pub(crate) fn close_overlay_slot(&mut self, root: GroupId, blocker: Option<ControlId>) {
        self.model.close_slot(root);
        if let Some(blocker) = blocker {
            self.release_control(blocker);
        }
    }

    /// Mints the control a blocker entry is named by.
    ///
    /// A real [`ControlId`] from the one minting authority, because the hit array, the focus
    /// ring and automation all key on that space and a second minter would collide with it.
    /// It carries no handlers, no chrome and no automation role: a blocker's whole behaviour
    /// is to route a press, which the router answers from the flag alone, and it is
    /// deliberately not a place focus can rest.
    pub(crate) fn mint_blocker(&mut self) -> ControlId {
        self.mint_control(ControlRow {
            node: NodeId::NONE,
            fill: None,
            label: None,
            border: None,
            // Nothing to light and nothing to move: a blocker is a rect in the array, and
            // the front table's hover and press paths both find nothing to do with it.
            front: crate::widget::ChromeRow {
                id: ControlId::default(),
                wash: None,
                hover: 0.0,
                press: 0.0,
                thumb: None,
                travel: 0.0,
                drive: None,
                fraction: 0.0,
            },
            chrome: None,
            scope: self.root_scope,
            state: ModelState::Rest,
            click: None,
            change: None,
            commit: None,
            tip: None,
            flyout: None,
            uia: UiaRole::None,
            name: None,
            text: None,
            key: None,
        })
    }

    /// A control's declared flyout body, shared out rather than borrowed.
    ///
    /// Building it is application code, so the host's borrow has to be released before it
    /// runs — and it stays in the row, because a picker's flyout is opened once per press
    /// and not once per lifetime.
    pub(crate) fn flyout_of(&self, target: ControlId) -> Option<Rc<dyn Fn() -> super::View>> {
        self.control(target).and_then(|c| c.flyout.clone())
    }

    /// A control's declared hover description, shared out for the same reason.
    pub(crate) fn tip_of(&self, target: ControlId) -> Option<Rc<TextSource>> {
        self.control(target).and_then(|c| c.tip.clone())
    }

    /// A control's accessible name, which is what a menu's type-ahead matches on.
    pub(crate) fn name_of(&self, target: ControlId) -> Option<&'static str> {
        self.control(target).and_then(|control| control.name)
    }

    /// Opens a placement row for the overlay opening at `depth`.
    ///
    /// Pushed rather than slotted: the stack discipline is the nesting rule, and an index is
    /// stable exactly as long as everything above it is still open. `depth` is the caller's
    /// own stack depth, asserted rather than returned — these are two stacks pushed and
    /// truncated together, and the assertion is what says so instead of a stored index that
    /// would only ever repeat what the position already knows.
    pub(crate) fn open_overlay_placement(&mut self, depth: u32, placement: Placement) {
        debug_assert_eq!(
            self.overlays.len(),
            depth as usize,
            "placement rows drifted"
        );
        self.overlays.push(placement);
    }

    /// Drops every placement row from `at` upward. Closing a menu takes its submenus.
    pub(crate) fn release_overlays_from(&mut self, at: u32) {
        self.overlays.truncate(at as usize);
    }

    /// Resolves every open overlay's offset against the solve that just ran, and answers
    /// whether any of them moved.
    ///
    /// Nothing here runs per frame: an overlay moves when it opens, when its anchor moves,
    /// and when the window resizes under it.
    fn place_overlays(&mut self) -> bool {
        use crate::overlay::{AnchorTo, place};
        let window = self.model.window();
        let mut moved = false;
        for index in 0..self.overlays.len() {
            let (root, anchor, last) = {
                let placement = &self.overlays[index];
                (placement.root, placement.anchor, placement.at)
            };
            let size = self.model.solved(root.node()).size;
            if size.x <= 0.0 || size.y <= 0.0 {
                // Declared but not yet measured. Placing a zero box would seat it at the
                // anchor's corner and then move it a pass later, which reads as a flash.
                continue;
            }
            let against = match anchor.to {
                AnchorTo::Control(id) => {
                    // An anchor that has unmounted leaves the overlay exactly where it is.
                    // Whether it should still be open is the layer above's decision, and
                    // moving it to the origin first would answer that question wrongly.
                    let Some(control) = self.control(id) else {
                        continue;
                    };
                    self.model.solved(control.node).rect
                }
                AnchorTo::Point(at) => windows_scene::Rect::new(at.x, at.y, at.x, at.y),
                AnchorTo::Window => windows_scene::Rect::new(0.0, 0.0, window.x, window.y),
            };
            let at = place(size, against, anchor, window);
            if at == last {
                continue;
            }
            self.overlays[index].at = at;
            moved |= self.model.place_slot(root, at);
        }
        moved
    }

    /// Sets each scroll container's extent and thumb from the box the solve gave it.
    ///
    /// Post-solve for the same reason shaped text is: a tracker's travel is the content's
    /// height less the viewport's, and neither exists until layout has said so. Nothing here
    /// runs per frame — a scroll that is *scrolling* moves entirely compositor-side, and this
    /// only speaks when the extents themselves changed.
    fn publish_scrolls(&mut self) -> bool {
        let mut moved = false;
        for at in self.scrolls.positions() {
            let Some(id) = self.scrolls.id_at(at) else {
                continue;
            };
            let Some(scroll) = self.scrolls.get(id) else {
                continue;
            };
            let (tracker, viewport, thumb, last) =
                (scroll.tracker, scroll.viewport, scroll.thumb, scroll.last);
            let content = scroll.content;
            let viewport_h = self.model.solved(viewport).size.y;
            let geom = crate::layout::thumb_geom(viewport_h, self.model.solved(content).size.y);
            if geom == last {
                continue;
            }
            if let Some(scroll) = self.scrolls.get_mut(id) {
                scroll.last = geom;
            }
            moved = true;
            // The position may travel outside these during a manipulation or inertia. That
            // overpan is the bounce, and it is wanted.
            self.model.tracker_bounds(
                tracker,
                Vector2 { x: 0.0, y: 0.0 },
                Vector2 {
                    x: 0.0,
                    y: geom.max_scroll,
                },
            );
            if let Some(thumb) = thumb {
                self.model
                    .style(thumb.node(), &crate::layout::thumb_style(geom));
                // The thumb rides the **same tracker** as the content, so it follows with no
                // front-thread work at all — and a re-bind is needed only because the ratio
                // it rides at is a function of extents that just changed.
                let m = if geom.max_scroll > 0.0 {
                    geom.travel / geom.max_scroll
                } else {
                    0.0
                };
                self.model.bind(
                    thumb.node(),
                    Prop::OffsetY,
                    windows_scene::Bind::Track {
                        tracker,
                        axis: windows_scene::TrackerAxis::PositionY,
                        affine: windows_scene::Affine {
                            m,
                            c: crate::layout::THUMB_MARGIN,
                        },
                    },
                );
            }
        }
        moved
    }

    /// The window's size in DIPs. Crosses upward, from the window's own resize message.
    pub fn set_window(&mut self, size: Vector2) {
        self.model.set_window(size);
    }

    /// The pixel grid everything is snapped to and rasterized for.
    pub fn set_env(&mut self, env: Env) {
        self.env = env;
    }

    /// The model, for the one caller that is allowed to reach it.
    pub(crate) fn model(&mut self) -> &mut Model {
        &mut self.model
    }
}

/// Re-paints one part, or leaves it alone where the row has no such role.
///
/// A part whose role went away keeps the colour it had rather than being cleared: the
/// alternative is a sprite painted with nothing, and there is no such paint.
fn paint(model: &mut Model, id: Option<SpriteId>, role: Option<Role>, scope: Scope) {
    if let (Some(id), Some(role)) = (id, role) {
        model.paint(id, Paint::Solid(crate::role::resolve(role, scope)));
    }
}
