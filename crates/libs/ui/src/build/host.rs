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

use super::arena::NIL;
use crate::gesture::GestureDecl;
use crate::layout::Over;
use crate::role::{Role, Scope};
use crate::widget::id;
use crate::widget::{Chrome, ModelState, TextSource, UiaRole};
use std::cell::RefCell;
use windows_numerics::Vector2;
use windows_scene::{
    Bounds, ControlId, Env, Exit, GroupId, MeasureIn, MeasureKey, Model, NodeId, Paint, Prop,
    SinkPatch, SpriteId, WidthClass,
};

/// One mounted node's re-lowerable recipe.
///
/// `over` is **contiguous** here even though the arena's is a chain: mount flattens it, and
/// at that point the whole list is known.
pub(crate) struct MountRow {
    pub node: NodeId,
    pub preset: crate::layout::Preset,
    pub over: OverStore,
    /// The scope this node's style was lowered against. Its `width` axis is what a
    /// responsive container rewrites.
    pub scope: Scope,
    /// The next row of the same mounted subtree, or [`NIL`]. A chain and not a `Vec`, so a
    /// list row realized during a fling records its rows without allocating.
    pub next: u32,
    pub control: Option<ControlId>,
    pub text: Option<MeasureKey>,
    /// Everything else this node claimed, held **here** rather than searched for.
    ///
    /// The unmount walks its own subtree's rows and releases exactly what they name. Scanning
    /// the value, responsive and scroll tables instead made unmounting one row of a long list
    /// cost the whole screen, which is the opposite of what a keyed list is for.
    pub values: u32,
    pub responsive: Option<u32>,
    pub scroll: Option<u32>,
    pub generation: u32,
    pub live: bool,
}

/// Up to four overrides inline, and a boxed slice beyond.
///
/// Four covers every widget in the set and almost every call site, so a realized list row
/// allocates nothing for its styles. Beyond four it is one allocation, freed with the node
/// rather than leaked into a slab that never compacts.
#[derive(Clone)]
pub(crate) enum OverStore {
    Inline { count: u8, items: [Over; 4] },
    Spill(Box<[Over]>),
}

impl OverStore {
    /// Taken straight off the arena's chain, and **not** through a `Vec` on the way.
    ///
    /// This is the store the mount was going to build anyway, so collecting into a temporary
    /// first would be an allocation per node per mount — on the path a list row realized
    /// during a fling takes.
    pub(crate) fn collect(items: impl Iterator<Item = Over>) -> Self {
        let mut inline = [Over::Grow; 4];
        let mut count = 0usize;
        let mut spill: Option<Vec<Over>> = None;
        for over in items {
            match &mut spill {
                Some(spilled) => spilled.push(over),
                None if count < inline.len() => {
                    inline[count] = over;
                    count += 1;
                }
                None => {
                    let mut spilled = inline.to_vec();
                    spilled.push(over);
                    spill = Some(spilled);
                }
            }
        }
        match spill {
            Some(spilled) => Self::Spill(spilled.into_boxed_slice()),
            None => Self::Inline {
                count: count as u8,
                items: inline,
            },
        }
    }

    pub(crate) fn as_slice(&self) -> &[Over] {
        match self {
            Self::Inline { count, items } => &items[..*count as usize],
            Self::Spill(items) => items,
        }
    }
}

/// One interactive node, addressed by the index inside its [`ControlId`].
///
/// Split from its front-side half by **what each thread can do with it**: the handlers stay
/// here because only this thread can call them, and everything in [`front`](Self::front) goes
/// there because only that thread is in the tick that has to move a pixel. Nothing crosses
/// that is not a number or an id — the wash opacities are resolved here, at mount, because
/// realizing a colour cell mid-hover would be a surface creation on the interaction path.
#[expect(
    dead_code,
    reason = "the overlay layer reads tip and flyout, and UI Automation reads the rest;               both are written against this row and land next"
)]
pub(crate) struct Control {
    pub generation: u32,
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
    pub tip: Option<TextSource>,
    pub flyout: Option<Box<dyn Fn() -> super::View>>,
    pub uia: UiaRole,
    pub name: Option<&'static str>,
    /// The automation-id segment. `&'static str`, so nothing is built at mount: the path is
    /// materialized only if UI Automation asks, which is rarely and never on a hot path.
    pub key: Option<&'static str>,
    pub live: bool,
}

/// One moving part, and the room it has.
///
/// A fraction cannot be lowered at mount, because the room is a **layout output** — the
/// track's extent less the part's own. So the fraction is kept here and multiplied out after
/// the solve, which is the split [`publish_scrolls`](Host::publish_scrolls) already makes for
/// the same reason. It is also what lets the front thread move the same part from the same
/// number without asking this thread for geometry.
pub(crate) struct ValueRow {
    pub generation: u32,
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
    pub row: u32,
    /// The next value row of the same mount row, or [`NIL`].
    pub next: u32,
    pub live: bool,
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

/// A value row's identity.
///
/// Generational for the reason [`ControlId`] is: an effect is disposed by the scope that owns
/// it and a mount is released by the handle that owns it, and nothing orders those two. A
/// write arriving after the release must find **nothing**, not whatever now occupies the
/// slot. The packing is `id`'s, shared with a control's rather than restated.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ValueId(u64);

/// A container that classifies its own inline size for its subtree.
pub(crate) struct Responsive {
    /// Held as a `GroupId` because only a group can be one, and because the model's own
    /// signature says so — there is no way to name a group this crate did not mint.
    pub node: GroupId,
    pub bounds: Bounds,
    pub class: WidthClass,
    /// Every mount index in this container's subtree, recorded once during the walk.
    pub subtree: Vec<u32>,
}

/// The app thread's half of the widget layer.
pub struct Host {
    pub(crate) model: Model,
    pub(crate) env: Env,
    pub(crate) root_scope: Scope,
    pub(crate) mounts: Vec<MountRow>,
    pub(crate) mount_free: Vec<u32>,
    pub(crate) controls: Vec<Control>,
    pub(crate) control_free: Vec<u32>,
    /// Slotted rather than compacted, for the reason every other table here is: the mount row
    /// holds an index, and a `retain` that shifted one would silently point it at a
    /// neighbour.
    pub(crate) responsives: Vec<Option<Responsive>>,
    pub(crate) responsive_free: Vec<u32>,
    /// What each target declared about the gestures it accepts, drained by whoever owns the
    /// router. Front-resident from then on: no call is made to this thread to decide
    /// whether a gesture applies.
    pub(crate) gestures: Vec<(ControlId, GestureDecl)>,
    /// The front-side half of each control minted — or re-measured — since the last drain.
    pub(crate) chrome: Vec<crate::widget::ChromeRow>,
    /// Controls released since the last drain, so the front table forgets them rather than
    /// holding a row that names a destroyed sprite.
    pub(crate) released: Vec<ControlId>,
    /// Moving parts awaiting the travel only a solve can give them.
    pub(crate) values: Vec<ValueRow>,
    pub(crate) value_free: Vec<u32>,
    /// Trackers minted here but **created** on the front thread, because an
    /// `InteractionTracker` is a composition object and its source is a visual.
    pub(crate) trackers: Vec<TrackerSpec>,
    pub(crate) scrolls: Vec<Option<ScrollRow>>,
    pub(crate) scroll_free: Vec<u32>,
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
    /// Installs the app thread's host, and wires the measure seam into the model.
    ///
    /// The measure closure captures **nothing**. It has to be `Send`, and it reaches the
    /// text table through that table's own thread-local — which is also what lets the table
    /// hold laid-out runs, since a run is thread-affine and an `Arc<Mutex<..>>` of one
    /// would not compile.
    pub fn install(mut model: Model, env: Env, root_scope: Scope) {
        model.on_measure(|input: MeasureIn| super::text::measure(input));
        let host = Self {
            model,
            env,
            root_scope,
            mounts: Vec::new(),
            mount_free: Vec::new(),
            controls: Vec::new(),
            control_free: Vec::new(),
            responsives: Vec::new(),
            responsive_free: Vec::new(),
            gestures: Vec::new(),
            chrome: Vec::new(),
            released: Vec::new(),
            values: Vec::new(),
            value_free: Vec::new(),
            trackers: Vec::new(),
            scrolls: Vec::new(),
            scroll_free: Vec::new(),
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

    pub(crate) fn mint_mount(&mut self, row: MountRow) -> u32 {
        let Some(at) = self.mount_free.pop() else {
            self.mounts.push(row);
            return (self.mounts.len() - 1) as u32;
        };
        let slot = &mut self.mounts[at as usize];
        let generation = slot.generation.wrapping_add(1);
        *slot = MountRow { generation, ..row };
        at
    }

    /// Records a responsive container against the mount row that owns it.
    pub(crate) fn mint_responsive(&mut self, row: u32, responsive: Responsive) {
        let at = slot_in(&mut self.responsives, &mut self.responsive_free, responsive);
        if let Some(row) = self.mounts.get_mut(row as usize) {
            row.responsive = Some(at);
        }
    }

    /// The same for a scroll container, whose tracker dies with it.
    pub(crate) fn mint_scroll(&mut self, row: u32, scroll: ScrollRow) {
        let at = slot_in(&mut self.scrolls, &mut self.scroll_free, scroll);
        if let Some(row) = self.mounts.get_mut(row as usize) {
            row.scroll = Some(at);
        }
    }

    /// The scope a node's style was last lowered against, or `None` where it has unmounted.
    ///
    /// The one reader of a live scope: a responsive container rewrites this row and a
    /// surface pushed a rung into it, so anything re-lowering a style asks here rather than
    /// capturing a copy that a class change will leave behind.
    pub(crate) fn mount_scope(&self, row: u32) -> Option<Scope> {
        self.mounts
            .get(row as usize)
            .filter(|row| row.live)
            .map(|row| row.scope)
    }

    /// Mints a control row and its identity.
    ///
    /// The generation half is what makes a **stale intent** — one queued before an unmount
    /// — a miss rather than a call into whatever now occupies the slot.
    pub(crate) fn mint_control(&mut self, control: Control) -> ControlId {
        let at = if let Some(at) = self.control_free.pop() {
            let slot = &mut self.controls[at as usize];
            let generation = slot.generation.wrapping_add(1);
            *slot = Control {
                generation,
                ..control
            };
            at
        } else {
            self.controls.push(control);
            (self.controls.len() - 1) as u32
        };
        ControlId(id::pack(at, self.controls[at as usize].generation))
    }

    /// The control a routed hit or an arriving intent names, or `None` if it is stale.
    pub(crate) fn control(&self, id: ControlId) -> Option<&Control> {
        let (at, generation) = (id::index(id.0), id::generation(id.0));
        self.controls
            .get(at as usize)
            .filter(|c| c.live && c.generation == generation)
    }

    pub(crate) fn control_mut(&mut self, id: ControlId) -> Option<&mut Control> {
        let (at, generation) = (id::index(id.0), id::generation(id.0));
        self.controls
            .get_mut(at as usize)
            .filter(|c| c.live && c.generation == generation)
    }

    fn release_control(&mut self, id: ControlId) {
        let (at, generation) = (id::index(id.0), id::generation(id.0));
        if let Some(slot) = self.controls.get_mut(at as usize)
            && slot.live
            && slot.generation == generation
        {
            slot.live = false;
            // Dropped rather than left: a handler captures application state, and a table
            // that kept them would hold a screen's worth of closures alive behind a
            // generation nobody will ever name again.
            slot.click = None;
            slot.change = None;
            slot.commit = None;
            slot.tip = None;
            slot.flyout = None;
            self.control_free.push(at);
            self.released.push(id);
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
            .get(mount as usize)
            .map_or(NIL, |mount| mount.values);
        let at = if let Some(at) = self.value_free.pop() {
            let slot = &mut self.values[at as usize];
            let generation = slot.generation.wrapping_add(1);
            *slot = ValueRow {
                generation,
                next: head,
                ..row
            };
            at
        } else {
            self.values.push(ValueRow { next: head, ..row });
            (self.values.len() - 1) as u32
        };
        if let Some(mount) = self.mounts.get_mut(mount as usize) {
            mount.values = at;
        }
        ValueId(id::pack(at, self.values[at as usize].generation))
    }

    fn value_mut(&mut self, id: ValueId) -> Option<&mut ValueRow> {
        let (at, generation) = (id::index(id.0), id::generation(id.0));
        self.values
            .get_mut(at as usize)
            .filter(|v| v.live && v.generation == generation)
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
    fn publish_values(&mut self) -> bool {
        let mut moved = false;
        for index in 0..self.values.len() {
            let value = &self.values[index];
            if !value.live || value.unit != crate::build::arena::Unit::Travel {
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
            self.values[index].travel = travel;
            moved = true;
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
        moved
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
        let Some(chrome) = control.chrome else {
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
    pub(crate) fn unmount(&mut self, node: NodeId, exit: Exit, rows: u32) {
        let mut at = rows;
        while at != NIL {
            let Some(row) = self.mounts.get_mut(at as usize) else {
                break;
            };
            let next = row.next;
            row.live = false;
            row.next = NIL;
            let (control, text) = (row.control.take(), row.text.take());
            let values = core::mem::replace(&mut row.values, NIL);
            let (responsive, scroll) = (row.responsive.take(), row.scroll.take());
            self.mount_free.push(at);
            if let Some(id) = control {
                self.release_control(id);
            }
            if let Some(key) = text {
                // Fallibly, for the reason `try_with` gives: this can run while the thread
                // is tearing its locals down, and the runs are going the same way anyway.
                super::text::try_with(|table| table.release(key, &mut self.model));
            }
            // A moving part: no box, no room. Released into the free list rather than
            // removed, because an effect holds its identity and a shifted index would
            // silently become another control's.
            let mut value = values;
            while let Some(row) = self.values.get_mut(value as usize) {
                let next = core::mem::replace(&mut row.next, NIL);
                row.live = false;
                self.value_free.push(value);
                value = next;
            }
            // A responsive container inside the subtree has nothing left to re-lower.
            if let Some(at) = responsive {
                self.responsives[at as usize] = None;
                self.responsive_free.push(at);
            }
            // A tracker outliving its viewport would be a compositor object with nothing to
            // be sourced from, so it is dropped with the row that named it.
            if let Some(at) = scroll
                && let Some(row) = self.scrolls[at as usize].take()
            {
                self.scroll_free.push(at);
                self.model.drop_tracker(row.tracker);
            }
            at = next;
        }
        self.model.destroy(node, exit);
    }

    // ── the width class, computed rather than reported ────────────────────────────

    /// Re-classifies every responsive container against the last solve, and re-lowers the
    /// styles of any subtree whose class moved.
    ///
    /// The class is resolved inside the solve and reported only to a measurement, so a
    /// container with no measured descendant never learns it from below. It does not have
    /// to: the solved rect is public, and classifying it here uses the same [`Bounds`] and
    /// hysteresis. Running at the start of a flush settles one frame behind a live resize,
    /// which is the alternative to solving the tree twice per frame.
    pub(crate) fn reclassify(&mut self) {
        for index in 0..self.responsives.len() {
            let Some((node, bounds, previous)) = self.responsives[index]
                .as_ref()
                .map(|r| (r.node, r.bounds, r.class))
            else {
                continue;
            };
            let width = self.model.solved(node.node()).size.x;
            if width <= 0.0 {
                continue;
            }
            let class = bounds.reclassify(width, previous);
            if class == previous {
                continue;
            }
            let Some(row) = self.responsives[index].as_mut() else {
                continue;
            };
            row.class = class;
            let subtree = core::mem::take(&mut row.subtree);
            for &at in &subtree {
                self.relower(at, class);
            }
            if let Some(row) = self.responsives[index].as_mut() {
                row.subtree = subtree;
            }
        }
    }

    /// Re-lowers one node's style at a new width class.
    ///
    /// Colour is **not** touched, and that is the point: only `metric` and `typography` may
    /// read the width axis, so a class change re-pushes styles and re-measures text and
    /// rebinds zero paints. `Model::style` compares before it pushes, so a class change
    /// that moves no metric emits no op at all.
    fn relower(&mut self, at: u32, class: WidthClass) {
        let Some(row) = self.mounts.get_mut(at as usize) else {
            return;
        };
        if !row.live {
            return;
        }
        row.scope = row.scope.at_width(class);
        let (node, preset, scope, text) = (row.node, row.preset, row.scope, row.text);
        let style = crate::layout::lower(preset, self.mounts[at as usize].over.as_slice(), scope);
        self.model.style(node, &style);
        // The type ramp reads the class, so a run inside a container that reclassified is
        // laid out under a different font — which the run finds out from here rather than
        // from ambient state.
        if let Some(key) = text {
            super::text::with(|table| table.set_scope(key, scope));
        }
    }

    // ── the flush ─────────────────────────────────────────────────────────────────

    /// Solves, publishes the text that solve decided the width of, and hands the patch over.
    ///
    /// The order is the whole of it. A run laid out at the width layout chose has to reach
    /// the *same* patch as that layout, so the solve is separated from the hand-over and the
    /// glyphs are placed in between. Publishing can move a wrapping run's line boxes, which
    /// is why the second solve is not redundant — and it is free when nothing moved, because
    /// a pass that changed nothing solves nothing.
    pub fn flush(&mut self, patch: &mut SinkPatch) {
        let env = self.env;
        self.reclassify();
        self.model.solve(env);
        let moved = super::text::with(|table| table.publish(&mut self.model))
            | self.publish_scrolls()
            | self.publish_values();
        if moved {
            self.model.solve(env);
        }
        self.model.flush(patch, env);
    }

    /// Sets each scroll container's extent and thumb from the box the solve gave it.
    ///
    /// Post-solve for the same reason shaped text is: a tracker's travel is the content's
    /// height less the viewport's, and neither exists until layout has said so. Nothing here
    /// runs per frame — a scroll that is *scrolling* moves entirely compositor-side, and this
    /// only speaks when the extents themselves changed.
    fn publish_scrolls(&mut self) -> bool {
        let mut moved = false;
        for index in 0..self.scrolls.len() {
            let Some(scroll) = self.scrolls[index].as_ref() else {
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
            if let Some(scroll) = self.scrolls[index].as_mut() {
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

/// Puts `row` in a free slot, or on the end. Shared by the two tables whose index a mount row
/// holds, so neither can be compacted out from under one.
fn slot_in<T>(table: &mut Vec<Option<T>>, free: &mut Vec<u32>, row: T) -> u32 {
    if let Some(at) = free.pop() {
        table[at as usize] = Some(row);
        return at;
    }
    table.push(Some(row));
    (table.len() - 1) as u32
}

// The mount chain's terminator is the arena's, so a row that says "no next" and a slot that
// says "no link" cannot drift apart.
const _: [(); 1] = [(); (NIL == u32::MAX) as usize];
