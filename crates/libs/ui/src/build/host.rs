//! Holds the app thread's model, style recipes, and dense control table across mounts.
//!
//! The arena clears after every mount, so state the lowering must be able to redo is kept
//! here: the style recipe per node, since a width class is decided in the solve and moves on
//! resize; the control row per interactive node, since the hover path indexes an array rather
//! than hashing.
//!
//! A [`Host::with`] body must not call application code.
//! [`Effect::new`](crate::signal::Effect::new) runs its closure immediately, so an effect
//! created under the borrow re-enters it. Mount releases the borrow around every effect it
//! installs, and an effect body takes a fresh one. An effect's captures are therefore `Copy`
//! ids, with no `Rc<RefCell<Model>>` to clone per binding.

use crate::gesture::GestureDecl;
use crate::role::{Role, Scope};
use crate::widget::{Chrome, ModelState, TextSource, UiaRole};
use std::cell::RefCell;
use std::rc::Rc;
use windows_numerics::Vector2;
use windows_scene::{
    ControlId, Env, Exit, GroupId, Id, Ids, MeasureIn, MeasureKey, Model, NodeId, Paint, Prop,
    SinkPatch, Slots, SpriteId, Tracker,
};

/// Names the family of mount rows, whose ids are [`MountId`].
///
/// [`Mount`], [`Value`], [`Scroll`] and [`Probe`] are markers rather than the row types
/// themselves: an id belongs to a family, and one family can have more than one store — a
/// control has a row here and a row on the front thread, over one set of ids.
#[derive(Debug)]
pub(crate) struct Mount;
#[derive(Debug)]
pub(crate) struct Value;
#[derive(Debug)]
pub(crate) struct Scroll;
#[derive(Debug)]
pub(crate) struct Probe;

pub(crate) type MountId = Id<Mount>;
pub(crate) type ValueId = Id<Value>;
pub(crate) type ScrollId = Id<Scroll>;
pub(crate) type ProbeId = Id<Probe>;

/// Records one mounted node and every table row it has to release.
pub(crate) struct MountRow {
    pub node: NodeId,
    /// The next row of the same mounted subtree, or [`Id::NONE`] at the end of the chain.
    ///
    /// A chain rather than a `Vec`, so a list row realized during a fling records its rows
    /// without allocating. The link is an id and not a bare index, so every step of a walk is
    /// checked: an index would reach a row without asking whether it is still the row that
    /// was linked.
    pub next: MountId,
    pub control: Option<ControlId>,
    pub text: Option<MeasureKey>,
    /// Heads this node's chain of value rows.
    ///
    /// The unmount walks its own subtree's rows and releases exactly what they name, so
    /// unmounting one row of a long list costs that row rather than a scan of the value and
    /// scroll tables.
    pub values: ValueId,
    pub scroll: Option<ScrollId>,
    pub probe: Option<ProbeId>,
}

/// Holds one interactive node, addressed by the index inside its [`ControlId`].
///
/// The handlers stay on this thread, the only one that may call them;
/// [`front`](Self::front) holds what the front thread needs during the tick that moves a
/// pixel. Nothing crosses that is not a number or an id — the wash opacities are resolved
/// here, at mount, so the interaction path never realizes a colour cell.
pub(crate) struct ControlRow {
    pub node: NodeId,
    /// The parts a model-state change re-paints, addressed by id so the swap needs no
    /// search.
    pub fill: Option<SpriteId>,
    pub label: Option<SpriteId>,
    pub border: Option<SpriteId>,
    /// The front thread's half of this control, kept here as well as sent.
    ///
    /// One copy of the wash ids, the alphas and the travel, so the two sides cannot disagree
    /// about what a control is, and so a solve that changed this control's room re-sends a
    /// corrected row rather than reconstructing one.
    pub front: crate::widget::ChromeRow,
    /// The table row this control's colours come from, so a state change re-reads the same
    /// row rather than remembering what it painted.
    pub chrome: Option<Chrome>,
    pub scope: Scope,
    pub state: ModelState,
    pub click: Option<Box<dyn Fn()>>,
    pub change: Option<Box<dyn Fn(f64)>>,
    pub commit: Option<Box<dyn Fn(f64)>>,
    /// The hover description and the side it opens on.
    ///
    /// `Rc` rather than `Box` for both this and [`flyout`](Self::flyout): building either
    /// body is application code, so the overlay layer clones it out of the host's borrow
    /// before running it. Both stay in the row, since a picker's flyout opens once per press
    /// and not once per lifetime.
    ///
    /// The side is authored rather than derived: which side clears a control's neighbours
    /// depends on the axis its author stacked them on, so a description below a toolbar
    /// button clears its neighbours and the same one below a rail item lands on the next.
    pub tip: Option<(Rc<TextSource>, crate::overlay::Side)>,
    pub flyout: Option<Rc<dyn Fn() -> super::View>>,
    pub uia: UiaRole,
    pub name: Option<&'static str>,
    /// The text this control's subtree laid out, which its accessible name derives from where
    /// [`name`](Self::name) is unset. A control's label is rarely its own sprite, so the
    /// mount walk claims this on the way back up rather than reading it off this row.
    pub text: Option<MeasureKey>,
    /// The automation-id segment. A `&'static str`, so mount builds nothing: the full path is
    /// materialized only when UI Automation asks for it, which is off every hot path.
    pub key: Option<&'static str>,
}

/// Holds one moving part's fraction and the room it moves in.
///
/// The room is a layout output — the track's extent less the part's own — so a fraction
/// cannot be lowered at mount. It is kept here and multiplied out after the solve, by
/// [`publish_values`](Host::publish_values). The same number lets the front thread move the
/// part without asking this thread for geometry.
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
    /// Whether the router moves this part rather than this thread.
    ///
    /// The property has two possible writers and this field picks one. When it is set this
    /// thread never binds the property: a solve that changed the room re-sends the room, and
    /// the router re-drives the part from the fraction it holds, which is the newer of the
    /// two.
    pub front_driven: bool,
    pub row: MountId,
    /// The next value row of the same mount row, or [`Id::NONE`].
    pub next: ValueId,
}

impl ValueRow {
    /// Returns this row's fraction converted to the property's own unit.
    ///
    /// Reaches the same two conversions the front thread's driving path does, so a slid part
    /// and a turned one agree on which way their value runs.
    fn number(&self) -> f32 {
        match self.unit {
            crate::build::arena::Unit::Turn => crate::widget::angle_of(self.fraction),
            _ => crate::widget::offset_of(self.fraction, self.travel, self.vertical),
        }
    }
}

/// Holds one open overlay's placement rule and where it last landed.
///
/// Resolving a placement needs the solve — the overlay's measured size and its anchor's rect
/// — so the row sits beside the model rather than on the overlay layer, as [`ScrollRow`]
/// does. The layer above owns the overlay's lifetime; [`Host`] owns its geometry and is the
/// only writer of the model.
pub(crate) struct Placement {
    pub root: GroupId,
    pub anchor: crate::overlay::Anchor,
    /// What was last published, so a pass that moved nothing emits nothing.
    pub at: Vector2,
}

/// Owns the model and the tables the app thread's half of the widget layer builds into.
pub struct Host {
    pub(crate) model: Model,
    pub(crate) env: Env,
    pub(crate) root_scope: Scope,
    /// Mints mount-row ids.
    ///
    /// An `Ids` sits beside a store only where this thread owns that family's counter; a
    /// store keyed by ids minted elsewhere — the recipe table, the front thread's chrome —
    /// carries none.
    pub(crate) mount_ids: Ids<Mount>,
    pub(crate) mounts: Slots<Mount, MountRow>,
    pub(crate) control_ids: Ids<windows_scene::Control>,
    pub(crate) controls: Slots<windows_scene::Control, ControlRow>,
    /// What each target declared about the gestures it accepts, drained by the owner of the
    /// router. The declaration lives on the front thread from then on, so deciding whether a
    /// gesture applies needs no call into this thread.
    pub(crate) gestures: Vec<(ControlId, GestureDecl)>,
    /// The front-side half of each control minted — or re-measured — since the last drain.
    pub(crate) chrome: Vec<crate::widget::ChromeRow>,
    /// Model-state changes since the last drain, for automation.
    pub(crate) states: Vec<(ControlId, ModelState)>,
    /// Whether the set of elements has changed since the last accessible-tree publish.
    ///
    /// A `Cell`, so the side that has published clears it through a shared reference rather
    /// than a mutable one.
    pub(crate) uia_stale: std::cell::Cell<bool>,
    /// Controls released since the last drain, so the front table forgets them rather than
    /// holding a row that names a destroyed sprite.
    pub(crate) released: Vec<ControlId>,
    /// Moving parts awaiting the travel only a solve can give them.
    pub(crate) value_ids: Ids<Value>,
    pub(crate) values: Slots<Value, ValueRow>,
    /// Trackers named here and created on the front thread, since an `InteractionTracker` is
    /// a composition object sourced from a visual.
    pub(crate) trackers: Vec<TrackerSpec>,
    pub(crate) scroll_ids: Ids<Scroll>,
    pub(crate) scrolls: Slots<Scroll, ScrollRow>,
    /// Nodes an application asked for the solved box of. Empty on most screens.
    pub(crate) probe_ids: Ids<Probe>,
    pub(crate) probes: Slots<Probe, ProbeRow>,
    /// Which control is which window command, for the caption band to resolve a point
    /// through. Filled at mount by [`El::caption`](super::El::caption).
    pub(crate) caption: crate::caption::Registry,
    /// Open overlays, in the order they opened. A stack rather than a slotted table: overlays
    /// nest — a submenu sits above its menu and cannot outlive it — so closing one takes
    /// everything above it, and an index stays valid for exactly as long as that holds.
    pub(crate) overlays: Vec<Placement>,
}

/// Names a tracker for the front thread to create.
#[derive(Copy, Clone, Debug)]
pub struct TrackerSpec {
    pub id: windows_scene::TrackerId<windows_scene::Observed>,
    pub viewport: GroupId,
    pub axes: windows_scene::Axes,
}

pub(crate) use crate::layout::{ProbeRow, ScrollRow};

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

/// Why the host could not be reached. Each cause has its own message and its own fix.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Access {
    /// Nothing was installed.
    NoHost,
    /// A [`Host::with`] body reached back in.
    Reentrant,
    /// The thread is tearing its locals down, which leaves [`Host::try_with`] nothing to do.
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
    /// Installs the app thread's host and wires the measure and restyle seams into the model.
    ///
    /// Both closures are `Send` and capture nothing: each reaches its table through that
    /// table's own thread-local, which is what lets the text table hold laid-out runs, since
    /// a run is thread-affine and an `Arc<Mutex<..>>` of one would not compile. Neither may
    /// reach the host, whose borrow the solve is already inside.
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
            probe_ids: Ids::new(),
            probes: Slots::new(),
            caption: crate::caption::Registry::default(),
            overlays: Vec::new(),
        };
        HOST.with(|slot| *slot.borrow_mut() = Some(host));
    }

    /// Runs `f` against the thread's host.
    ///
    /// `f` must not call application code: it runs under the host's borrow, and an
    /// [`Effect`](crate::signal::Effect) created there runs its closure immediately and
    /// re-enters that borrow.
    ///
    /// # Panics
    ///
    /// Panics if no host is installed, and separately if `f` re-enters the host. The message
    /// names which of the two happened, because the two have opposite fixes.
    pub fn with<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        match Self::access(f) {
            Ok(out) => out,
            Err(why) => panic!("{}", why.message()),
        }
    }

    /// Runs `f` against the thread's host, answering `None` where there is no host to reach.
    ///
    /// For callers that run inside a `Drop`: a [`Mount`](super::Mount) is dropped by the
    /// scope that owned it, and at thread teardown that scope is itself a thread-local being
    /// destroyed. Reaching a thread-local during its own destruction phase fails, and a panic
    /// inside a `Drop` aborts the process. A host that is already gone leaves nothing to
    /// release, so this answers `None`.
    ///
    /// Re-entry is asserted in debug builds rather than ignored: dropping a mount from inside
    /// a [`Host::with`] body leaks the whole subtree.
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

    /// Returns whether a host is installed on this thread.
    #[must_use]
    pub fn installed() -> bool {
        HOST.with(|slot| slot.borrow().is_some())
    }

    /// Takes the gesture declarations mounted since the last drain, for the router to install.
    pub fn take_gestures(&mut self) -> Vec<(ControlId, GestureDecl)> {
        core::mem::take(&mut self.gestures)
    }

    /// Takes the front-side rows minted since the last drain, to be adopted alongside the
    /// patch.
    ///
    /// A row carries only numbers and ids, so it crosses as plain `Send` data. The handlers
    /// stay in the control row on this thread, the only one that may call them.
    pub fn take_chrome(&mut self) -> Vec<crate::widget::ChromeRow> {
        core::mem::take(&mut self.chrome)
    }

    /// Fills `out` with the automation facts layout does not already carry.
    ///
    /// Every seed is derived rather than declared: a widget names a role and the rest follows
    /// from it — the name is the control's own laid-out text unless it was given one, the
    /// value is the channel it already binds, and the patterns follow from the role.
    ///
    /// Clears `out`, then emits one sorted row per control that has a role. Strings are
    /// interned here, on the thread that owns the text table, so what crosses to the front is
    /// plain `Send` data.
    pub fn uia_seeds(&self, out: &mut crate::uia::Seeds) {
        use crate::uia::{ColFlags, Seed, State, Value};

        out.clear();
        for (id, control) in self.controls.iter() {
            if control.uia == UiaRole::None {
                continue;
            }
            let name = match control.name {
                Some(explicit) => out.intern(explicit),
                // Interned rather than borrowed, so an explicit name and a derived one — which
                // is not `'static` — take one path.
                None => control
                    .text
                    .and_then(|key| super::text::with(|table| table.str_of(key).map(str::to_owned)))
                    .map_or_else(Default::default, |text| out.intern(&text)),
            };
            // A tooltip becomes the element's `HelpText`. Read untracked: this runs inside a
            // flush, and subscribing whatever effect is on the stack would rebuild a screen
            // when a tip changed.
            let help = control
                .tip
                .as_ref()
                .map_or_else(Default::default, |(tip, _)| {
                    let mut text = String::new();
                    crate::signal::untracked(|| tip.append(&mut text));
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
                // A static run publishes its own body as a text document, which is what a
                // screen reader reads a read-only selectable surface through.
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
    /// A drain rather than a tree republish: a toggle does not move the set of elements, so
    /// announcing one costs no tree allocation and tells no client that the screen's
    /// structure changed.
    pub fn take_states(&mut self) -> Vec<(ControlId, ModelState)> {
        core::mem::take(&mut self.states)
    }

    /// Returns whether the accessible tree needs rebuilding.
    ///
    /// Set when a control is minted or released, which is when the set of elements changes,
    /// and again when a published name changes. Everything else a client can observe — a
    /// value, a state, focus, a scroll offset — reaches it without a rebuild.
    pub fn uia_stale(&self) -> bool {
        self.uia_stale.get()
    }

    /// Clears the stale flag, for the caller that has just republished the tree.
    pub fn uia_published(&self) {
        self.uia_stale.set(false);
    }

    /// Marks the accessible tree stale without minting a control.
    ///
    /// A name is copied into the published blob, so a label that re-reads its text leaves the
    /// tree holding the old string until it is republished.
    pub(crate) fn uia_restale(&self) {
        self.uia_stale.set(true);
    }

    /// Takes the controls released since the last drain, for the front table to forget.
    ///
    /// The generational id already makes a stale report a miss; draining is what bounds the
    /// front table, which would otherwise keep a row per control that ever existed.
    pub fn take_released(&mut self) -> Vec<ControlId> {
        core::mem::take(&mut self.released)
    }

    /// Calls the handler each intent names.
    ///
    /// An intent queued before its control unmounted is skipped: the generation half of the
    /// id does not match the slot's, so the lookup is a bounds-checked index that finds
    /// nothing rather than a call into whatever occupies that slot.
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

    /// Runs `f` against the first scroll container `pick` accepts.
    ///
    /// A linear scan rather than a map: a screen has a handful of scroll surfaces, and this
    /// is walked from every tracker report of every fling.
    fn scroll_where(&mut self, pick: impl Fn(&ScrollRow) -> bool, f: impl FnOnce(&mut ScrollRow)) {
        for at in self.scrolls.positions() {
            let Some(id) = self.scrolls.id_at(at) else {
                continue;
            };
            if self.scrolls.get(id).is_some_and(&pick)
                && let Some(row) = self.scrolls.get_mut(id)
            {
                f(row);
                return;
            }
        }
    }

    /// Runs `f` against the container a tracker report belongs to, keyed by the raw id a
    /// [`SceneEvent`](windows_scene::SceneEvent) carries.
    pub(crate) fn scroll_by_tracker(
        &mut self,
        tracker: Id<Tracker>,
        f: impl FnOnce(&mut ScrollRow),
    ) {
        self.scroll_where(|row| row.tracker.id() == tracker, f);
    }

    /// Runs `f` against the container whose viewport carries `control`, which is what a hover
    /// names.
    pub(crate) fn scroll_by_control(&mut self, control: ControlId, f: impl FnOnce(&mut ScrollRow)) {
        self.scroll_where(|row| row.control == Some(control), f);
    }

    /// Runs `f` against the container whose thumb is grabbed by `control`.
    pub(crate) fn scroll_by_grab(&mut self, control: ControlId, f: impl FnOnce(&mut ScrollRow)) {
        self.scroll_where(|row| row.grab == Some(control), f);
    }

    /// Records a probed node against the mount row that owns it, so it is released when that
    /// subtree unmounts rather than left reporting a destroyed node.
    pub(crate) fn mint_probe(&mut self, row: MountId, probe: ProbeRow) {
        let at = self.probes.insert(&mut self.probe_ids, probe);
        if let Some(row) = self.mounts.get_mut(row) {
            row.probe = Some(at);
        }
    }

    /// Publishes the solved box of every probed node whose box moved.
    ///
    /// Writing a cell marks the graph and raises a frame request; it runs no effect and no
    /// memo, so it cannot re-enter the host's borrow. A reader of the value runs on the next
    /// tick, which is the one-tick lag a probe reports.
    ///
    /// A cell whose owner has been disposed is skipped rather than written, because
    /// `Cell::set` panics on a disposed handle. The two halves of a probe die at different
    /// moments — the cell with the scope that made it, the row with the mount walk that
    /// recorded it — so neither drop order is depended on.
    fn publish_probes(&mut self) {
        for at in self.probes.positions() {
            let Some(id) = self.probes.id_at(at) else {
                continue;
            };
            let Some(probe) = self.probes.get(id) else {
                continue;
            };
            let (node, cell) = (probe.node, probe.cell);
            let now = crate::layout::Placed::from(self.model.solved(node));
            // `set` gates on equality, which is what keeps a probe off the per-frame path: a
            // solve that moved nothing wakes nothing derived from this cell.
            if cell.alive() {
                cell.set(now);
            }
        }
    }

    /// Records a scroll container against the mount row that owns it, so its tracker is
    /// dropped when that row unmounts.
    pub(crate) fn mint_scroll(&mut self, row: MountId, scroll: ScrollRow) {
        let at = self.scrolls.insert(&mut self.scroll_ids, scroll);
        if let Some(row) = self.mounts.get_mut(row) {
            row.scroll = Some(at);
        }
    }

    /// Mints a control row, returns its id, and marks the accessible tree stale.
    ///
    /// The generation half of the id makes an intent queued before an unmount a miss rather
    /// than a call into whatever now occupies the slot.
    pub(crate) fn mint_control(&mut self, control: ControlRow) -> ControlId {
        self.uia_stale.set(true);
        self.controls.insert(&mut self.control_ids, control)
    }

    /// Returns the control `id` names, or `None` where the id is stale.
    pub(crate) fn control(&self, id: ControlId) -> Option<&ControlRow> {
        self.controls.get(id)
    }

    pub(crate) fn control_mut(&mut self, id: ControlId) -> Option<&mut ControlRow> {
        self.controls.get_mut(id)
    }

    /// Releases a control, dropping the handlers it captured.
    ///
    /// Queues the id for [`take_released`](Self::take_released), which is what bounds the
    /// front table; a stale report there is already a miss through the generational id.
    fn release_control(&mut self, id: ControlId) {
        if self.controls.remove(&mut self.control_ids, id).is_some() {
            self.released.push(id);
            self.uia_stale.set(true);
        }
    }

    // ── values ────────────────────────────────────────────────────────────────────

    /// Opens a value row for a moving part, before the control that owns it is known.
    ///
    /// Threads the row onto its mount row's chain, so the unmount releases it without
    /// searching the table.
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

    /// Names the control a moving part belongs to, the track it runs in, and which thread
    /// moves it. Called when that control mounts, which is after the part's own row opened.
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

    /// Records a fraction, clamped to `0..=1`, and binds the property that finishes it: the
    /// travel the last solve gave a slid part, or the constant sweep of a turned one.
    ///
    /// The only place on this thread that turns a fraction into a property. A part the router
    /// drives records the fraction and binds nothing, so that channel keeps one writer.
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
    /// Runs after the solve, as shaped text and scroll extents do: the room is the track's
    /// box less the part's own, and neither exists until layout has said so. The correction
    /// snaps rather than springs, because a window resize moves geometry and not a value, so
    /// no thumb on the screen animates.
    ///
    /// A part the router drives is corrected by sending it the new room rather than by
    /// binding the property, which keeps one writer on that channel; the front side re-drives
    /// from the fraction it holds, which is the newer of the two. A turned part has a
    /// constant sweep and no room, and is skipped.
    fn publish_values(&mut self) {
        // Walked by position and re-resolved through the id at each step: the body reaches
        // into the model, a second field of `self`, so no borrow of the table is held across
        // the walk. A position yields an id, so every row access stays checked.
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
            // Exact compare: `travel` is recomputed from the same two rects, so anything that
            // moved at all is a different float and a tolerance would hide small real moves.
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

    /// Puts a control into a model state, re-painting exactly the parts that state changes.
    ///
    /// `None` returns the control to rest. Selection and disablement are discrete paint swaps
    /// at event rate rather than washes, so they go through the model rather than a retarget,
    /// and they read the same chrome row the mount painted from.
    pub(crate) fn set_state(&mut self, id: ControlId, state: Option<ModelState>) {
        let state = state.unwrap_or(ModelState::Rest);
        let Some(control) = self.control(id) else {
            return;
        };
        if control.state == state {
            return;
        }
        let chrome = control.chrome;
        // Recorded whether or not there is anything to repaint: a control with no chrome row
        // still has an automation peer that reports checked, selected or unavailable.
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

    /// Destroys a mounted subtree and releases every table row it claimed.
    ///
    /// One destroy call, which cascades on the far side. Every other release walks the chain
    /// the mount threaded and touches only what those rows name, so the cost is proportional
    /// to the subtree and unmounting one row of a long list is cheap.
    pub(crate) fn unmount(&mut self, node: NodeId, exit: Exit, rows: MountId) {
        let mut at = rows;
        while let Some(row) = self.mounts.remove(&mut self.mount_ids, at) {
            // Fallible: this can run while the thread tears its locals down, and the style
            // table is a thread-local going the same way.
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
            if let Some(probe) = row.probe {
                self.probes.remove(&mut self.probe_ids, probe);
            }
            // A tracker is sourced from its viewport's visual, so it is dropped with the row
            // that named it.
            if let Some(scroll) = row
                .scroll
                .and_then(|at| self.scrolls.remove(&mut self.scroll_ids, at))
            {
                self.model.drop_tracker(scroll.tracker);
                // The thumb's control is minted beside the tracker rather than by the mount
                // walk, so releasing it here is what keeps its id from outliving the sprite
                // it names.
                if let Some(grab) = scroll.grab {
                    self.release_control(grab);
                }
            }
            at = row.next;
        }
        self.model.destroy(node, exit);
    }

    // ── the flush ─────────────────────────────────────────────────────────────────

    /// Solves, settles what only a solve can decide, and writes the patch.
    ///
    /// Three steps, each re-solving only what the one before it moved. A pass that changed
    /// nothing re-solves nothing, so the second and third solves are free in the steady
    /// state.
    ///
    /// 1. Solve. The width class resolves inside the solve, and the styles it implies are
    ///    re-lowered through the restyle seam before layout runs on them, so a container that
    ///    crossed a threshold needs no correcting pass here.
    /// 2. Publish geometry. Shaped runs, scroll extents and value travel are all functions of
    ///    solved boxes, so they cannot be stated before one. Publishing can move a wrapping
    ///    run's line boxes and a thumb's style, which the second solve takes up.
    /// 3. Place overlays. After the publishes rather than beside them: a menu's width is its
    ///    labels', and the labels are placed in step 2. Placement moves an overlay and never
    ///    resizes one, so the third solve computes the sizes the second did and the sequence
    ///    terminates.
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

    /// Publishes everything whose value is a function of the solve, and returns whether any
    /// of it moved a box.
    fn publish_geometry(&mut self) -> bool {
        let text = super::text::with(|table| table.publish(&mut self.model));
        let scrolls = self.publish_scrolls();
        // Values bind compositor properties and dirty no layout, so they are published here
        // for ordering and contribute nothing to whether a re-solve is owed.
        self.publish_values();
        // Probes contribute nothing either: what they write is read on the next tick, so
        // counting one would make a solve react to a solve and the sequence in `flush` would
        // have no reason to terminate.
        self.publish_probes();
        text | scrolls
    }

    // ── overlays ──────────────────────────────────────────────────────────────────

    /// Mints a parentless overlay root and opens a slot on it. The one caller of
    /// `Model::orphan_group` outside `windows-scene`.
    ///
    /// A parentless root is invisible to a parent walk and is reached by the disposal walk
    /// instead, which reads the slot array. Minting and opening in one call is what puts
    /// every such root in that array: no un-opened one exists.
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

    /// Mints the control id a blocker entry is named by.
    ///
    /// A [`ControlId`] from the same minting authority as every other control, since the hit
    /// array, the focus ring and automation all key on that space. The row carries no
    /// handlers, no chrome and no automation role: the router answers a press on a blocker
    /// from the hit flag alone, and focus cannot rest on it.
    pub(crate) fn mint_blocker(&mut self) -> ControlId {
        self.mint_control(ControlRow {
            node: NodeId::NONE,
            fill: None,
            label: None,
            border: None,
            // Nothing to light and nothing to move: a blocker is a rect in the hit array, and
            // the front table's hover and press paths find no wash and no thumb here.
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

    /// Returns a clone of the control's flyout body, or `None` where it declared none.
    ///
    /// Cloned rather than borrowed: building the body is application code and must run after
    /// the host's borrow is released. The row keeps its own handle, since a picker's flyout
    /// opens once per press and not once per lifetime.
    pub(crate) fn flyout_of(&self, target: ControlId) -> Option<Rc<dyn Fn() -> super::View>> {
        self.control(target).and_then(|c| c.flyout.clone())
    }

    /// Returns a clone of the control's hover description and the side it opens on.
    ///
    /// Cloned for the same reason as [`flyout_of`](Self::flyout_of). Both come back in one
    /// read, since a description with no side cannot be placed.
    pub(crate) fn tip_of(
        &self,
        target: ControlId,
    ) -> Option<(Rc<TextSource>, crate::overlay::Side)> {
        self.control(target).and_then(|c| c.tip.clone())
    }

    /// Returns the control's explicit accessible name, which a menu's type-ahead matches on.
    pub(crate) fn name_of(&self, target: ControlId) -> Option<&'static str> {
        self.control(target).and_then(|control| control.name)
    }

    /// Pushes a placement row for the overlay opening at `depth`.
    ///
    /// Pushed rather than slotted: overlays nest, so an index stays valid exactly as long as
    /// everything above it is still open. `depth` is the caller's own stack depth, and the
    /// two stacks are pushed and truncated together rather than storing an index the position
    /// already carries.
    ///
    /// # Panics
    ///
    /// Debug builds panic if `depth` is not the current number of placement rows.
    pub(crate) fn open_overlay_placement(&mut self, depth: u32, placement: Placement) {
        debug_assert_eq!(
            self.overlays.len(),
            depth as usize,
            "placement rows drifted"
        );
        self.overlays.push(placement);
    }

    /// Drops every placement row from index `at` upward, so closing a menu takes its
    /// submenus.
    pub(crate) fn release_overlays_from(&mut self, at: u32) {
        self.overlays.truncate(at as usize);
    }

    /// Resolves every open overlay's offset against the solve that just ran, and returns
    /// whether any of them moved.
    ///
    /// An overlay moves when it opens, when its anchor moves and when the window resizes
    /// under it, so this is not a per-frame cost.
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
                    // Whether it stays open is the overlay layer's decision, and moving it to
                    // the origin first would pre-empt that.
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

    /// Sets each scroll container's tracker bounds and thumb from the box the solve gave it,
    /// and returns whether any of that moved a box.
    ///
    /// Runs after the solve, as shaped text does: a tracker's travel is the content's height
    /// less the viewport's, and neither exists until layout has said so. A scroll in progress
    /// moves entirely compositor-side, so this writes only when the extents themselves
    /// changed and is not a per-frame cost.
    fn publish_scrolls(&mut self) -> bool {
        // The trackers this mount named, created here rather than at mount: a
        // `VisualInteractionSource` takes its hit region from the viewport's size at the
        // moment it is created, and the solve above is what gave the viewport one. Created at
        // mount it hit-tests nothing, reports success, and the surface silently ignores every
        // wheel notch for the life of the window.
        //
        // A viewport with no area is not ready, and its spec stays pending. A scroll
        // container inside a hidden subtree is laid out at zero — `hide_when` and `when` are
        // both `Display::None` rather than an unmount — so the solve above gives it nothing
        // to be sourced from. The retry costs one `solved` read per pending spec on a list
        // that is empty in the steady state, and it lands on the flush that reveals the
        // subtree, since revealing it is a style change.
        let mut pending = core::mem::take(&mut self.trackers);
        pending.retain(|spec| {
            let size = self.model.solved(spec.viewport.node()).size;
            if size.x <= 0.0 || size.y <= 0.0 {
                return true;
            }
            self.model.create_tracker(spec.id, spec.viewport, spec.axes);
            false
        });
        self.trackers = pending;
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
            let (content, state, rail, grab) =
                (scroll.content, scroll.state, scroll.rail, scroll.grab);
            let box_ = self.model.solved(viewport).size;
            // A viewport with no area has not been laid out — a hidden subtree solves at zero
            // — and publishing from that zero would record `last` as sent while the bounds
            // went to a tracker that does not exist yet. The equality gate would then never
            // send them again. An unmeasured container publishes nothing and remembers
            // nothing, so the flush that gives it a box is the one that publishes.
            if box_.x <= 0.0 || box_.y <= 0.0 {
                continue;
            }
            let viewport_h = box_.y;
            // The realization window is a fraction of the viewport height, which a
            // virtualized list cannot compute for itself.
            state.resized(viewport_h);
            let geom = crate::layout::thumb_geom(viewport_h, self.model.solved(content).size.y);
            if geom == last {
                continue;
            }
            if let Some(scroll) = self.scrolls.get_mut(id) {
                scroll.last = geom;
            }
            moved = true;
            // The position may travel outside these bounds during a manipulation or inertia;
            // that overpan is the bounce.
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
                // The thumb rides the same tracker as the content, so it follows with no
                // front-thread work. The re-bind is needed because the ratio it rides at is a
                // function of the extents that just changed.
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
            // The rail is a strip over the right edge of the content, so it is a hit target
            // only while there is something to scroll. Left on, it takes every press on the
            // right edge of a surface that does not scroll, and a button sitting there
            // cannot be clicked.
            if let (Some(rail), Some(grab)) = (rail, grab) {
                self.model.hit(
                    rail.node(),
                    geom.overflow.then(|| crate::layout::grab_hit(grab)),
                );
            }
        }
        moved
    }

    /// Re-sends the coverage of every live text run.
    ///
    /// Answers [`SceneEvent::DeviceRebuilt`](windows_scene::SceneEvent::DeviceRebuilt) and
    /// [`ScaleChanged`](windows_scene::SceneEvent::ScaleChanged), which the ordinary publish
    /// cannot: neither event moves a DIP, so the width gate that makes publishing cheap
    /// reports nothing moved for exactly the case where every raster is wrong.
    ///
    /// Costs one re-emit per live run, so it belongs on those two events and nowhere else.
    pub fn reemit_text(&mut self) {
        super::text::with(|table| table.reemit(&mut self.model));
    }

    /// Sets the window's size in DIPs, from the window's own resize message.
    pub fn set_window(&mut self, size: Vector2) {
        self.model.set_window(size);
    }

    /// Sets the pixel grid everything is snapped to and rasterized for.
    pub fn set_env(&mut self, env: Env) {
        self.env = env;
    }

    /// Returns the model. The mount walk is its only caller, which keeps every `Model` call
    /// in one module.
    pub(crate) fn model(&mut self) -> &mut Model {
        &mut self.model
    }
}

/// Re-paints one part, or leaves it alone where either the sprite or the role is absent.
///
/// A part whose state carries no role keeps the colour it had: there is no paint that clears
/// a sprite.
fn paint(model: &mut Model, id: Option<SpriteId>, role: Option<Role>, scope: Scope) {
    if let (Some(id), Some(role)) = (id, role) {
        model.paint(id, Paint::Solid(crate::role::resolve(role, scope)));
    }
}
