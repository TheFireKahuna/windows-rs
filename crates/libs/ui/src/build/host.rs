//! Owns the app thread's retained model, layout declarations, text and controls.
//!
//! Construction borrows these stores through `Ui`. UI bindings defer their first run
//! until that borrow ends and capture checked IDs. Event handlers and retired callbacks
//! execute outside the host borrow. Layout uses borrowed services over the same stores.

use crate::gesture::GestureDecl;
use crate::role::Scope;
use crate::seam::{Down, RegionOp, ScrollOp};
use crate::widget::{ModelState, TextSource, UiaRole};
use std::cell::RefCell;
use std::rc::Rc;
use windows_numerics::Vector2;
use windows_present::Extent;
use windows_scene::{
    ControlId, Env, Exit, GroupId, Id, Ids, MeasureKey, Model, NodeId, Prop, SinkPatch, Slots,
    SpriteId, Tracker,
};

pub(crate) type ScrollId = NodeId;

/// Records one mounted node and every table row it has to release.
pub(crate) struct MountRow {
    pub(super) channels: u64,
    pub(super) no_inflate: bool,
    pub(super) bindings: Option<usize>,
    pub escape: Option<Rc<dyn Fn()>>,
    pub popup: bool,
    pub control: Option<ControlId>,
    pub text: Option<MeasureKey>,
    pub paints: NodeId,
}

impl MountRow {
    pub(crate) fn new() -> Self {
        Self {
            bindings: None,
            channels: 0,
            no_inflate: false,
            escape: None,
            popup: false,
            control: None,
            text: None,
            paints: NodeId::NONE,
        }
    }
}

/// Holds one interactive node, addressed by the index inside its [`ControlId`].
///
/// The handlers stay on this thread, the only one that may call them;
/// [`front`](Self::front) holds what the front thread needs during the tick that moves a
/// pixel. Nothing crosses that is not a number or an id — the wash opacities are resolved
/// here, at mount, so the interaction path never realizes a colour cell.
pub(crate) struct ControlRow {
    pub(super) dirty: bool,
    pub(super) hit: Option<windows_scene::HitDecl>,
    pub(super) selected: bool,
    pub(super) disabled: bool,
    pub source_epoch: u64,
    pub validation: Option<&'static str>,
    pub accepted_fraction: Option<f32>,
    pub node: NodeId,
    /// Concrete chrome parts needed by the native value driver.
    pub fill: Option<SpriteId>,
    pub border: Option<SpriteId>,
    /// The front thread's half of this control, kept here as well as sent.
    ///
    /// One copy of the wash ids, the alphas and the travel, so the two sides cannot disagree
    /// about what a control is, and so a solve that changed this control's room re-sends a
    /// corrected row rather than reconstructing one.
    pub front: crate::widget::ChromeRow,
    /// The table row this control's colours come from, so a state change re-reads the same
    /// row rather than remembering what it painted.
    pub scope: Scope,
    pub state: ModelState,
    pub click: Option<Rc<dyn Fn()>>,
    pub hovered: Option<crate::signal::Cell<bool>>,
    pub change: Option<Rc<dyn Fn(f64)>>,
    pub cancel: Option<Rc<dyn Fn()>>,
    pub commit: Option<Rc<dyn Fn(f64)>>,
    /// The two-axis drag's handler, where the application declared one.
    pub drag: Option<Rc<dyn Fn(crate::widget::Dragging)>>,
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
    pub flyout: Option<Rc<dyn Fn(&mut super::Ui<'_>)>>,
    pub uia: UiaRole,
    pub name: Option<std::borrow::Cow<'static, str>>,
    /// The first text registered by this control's children supplies its accessible name
    /// where [`name`](Self::name) is unset. Nested controls register their own text.
    pub text: Option<MeasureKey>,
    /// The automation-id segment. A `&'static str`, so mount builds nothing: the full path is
    /// materialized only when UI Automation asks for it, which is off every hot path.
    pub key: Option<&'static str>,
}

impl ControlRow {
    pub(crate) fn new(node: NodeId, scope: Scope) -> Self {
        ControlRow {
            dirty: false,
            hit: None,
            selected: false,
            disabled: false,
            source_epoch: 0,
            validation: None,
            accepted_fraction: None,
            node,
            fill: None,
            border: None,
            // Nothing to light and nothing to move: a blocker is a rect in the hit array, and
            // the front table's hover and press paths find no wash and no thumb here.
            front: crate::widget::ChromeRow::default(),
            scope,
            state: ModelState::Rest,
            click: None,
            hovered: None,
            change: None,
            cancel: None,
            commit: None,
            drag: None,
            tip: None,
            flyout: None,
            uia: UiaRole::None,
            name: None,
            text: None,
            key: None,
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
    pub viewport: Option<[crate::layout::Len; 4]>,
    pub bounds: Option<windows_scene::Rect>,
    pub entry: Option<OverlayEntry>,
    /// What was last published, so a pass that moved nothing emits nothing.
    pub at: Vector2,
}

pub(crate) struct OverlayEntry {
    node: NodeId,
    slide: crate::overlay::Slide,
    started: bool,
    finished: bool,
    window: Vector2,
    rect: Option<windows_scene::Rect>,
}

/// Owns the model and the tables the app thread's half of the widget layer builds into.
pub struct Host {
    pub(super) channels: Vec<(NodeId, NodeId, Prop, windows_scene::Bind)>,
    pub(super) root_pool: Vec<Vec<NodeId>>,
    pub(super) bindings: super::binding::Bindings,
    pub(super) retired: Vec<super::binding::Retired>,
    pub(crate) text: super::text::Table,
    pub(crate) styles: super::style::Styles,
    pub(crate) identity: u64,
    pub(crate) ramps: Slots<windows_scene::Ramp, (Vec<super::Stop>, windows_scene::Spread)>,
    pub(super) ramp_stops: Vec<(f32, windows_color::Radiance)>,
    pub(super) ramp_pool: Vec<Vec<super::Stop>>,
    pub(crate) model: Model,
    pub(super) window_size: crate::signal::Cell<Vector2>,
    _window_owner: crate::signal::Owner,
    pub(crate) popup_requests: Vec<crate::overlay::Request>,
    pub(crate) env: Env,
    pub(crate) root_scope: Scope,
    pub(super) geometry_jobs: Slots<windows_scene::Node, super::geometry::Row>,
    pub(crate) appearances: Slots<windows_scene::Node, super::theme::Appearance>,
    pub(crate) surfaces: Slots<windows_scene::Node, super::theme::Surface>,
    pub(crate) surfaces_dirty: bool,
    pub(crate) theme_update: Option<(Scope, windows_scene::BackdropSpec)>,
    pub(crate) theme_backdrop: Option<windows_scene::BackdropSpec>,
    /// Logical creation membership, keyed by the node's existing generation.
    pub(crate) mounts: Slots<windows_scene::Node, MountRow>,
    pub(crate) control_ids: Ids<windows_scene::Control>,
    pub(crate) controls: Slots<windows_scene::Control, ControlRow>,
    pub(crate) fields: Slots<windows_scene::Control, super::field::Row>,
    pub(crate) field_sources: Vec<crate::text_input::Source>,
    pub(crate) field_layouts: Vec<crate::text_input::Layout>,
    pub(crate) field_commits: Vec<crate::text_input::Commit>,
    /// What each target declared about the gestures it accepts, drained by the owner of the
    /// router. The declaration lives on the front thread from then on, so deciding whether a
    /// gesture applies needs no call into this thread.
    pub(crate) gestures: Vec<(ControlId, GestureDecl)>,
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
    /// Trackers named here and created on the front thread, since an `InteractionTracker` is
    /// a composition object sourced from a visual.
    pub(crate) trackers: Vec<TrackerSpec>,
    pub(crate) scrolls: Slots<windows_scene::Node, ScrollRow>,
    /// Nodes an application asked for the solved box of. Empty on most screens.
    pub(crate) probes: Slots<windows_scene::Node, crate::signal::Cell<crate::layout::Placed>>,
    pub(crate) regions: Slots<windows_scene::Node, crate::present::RegionRow>,
    /// Which control is which window command, for the caption band to resolve a point
    /// through. Filled during creation by [`Element::caption`](super::Element::caption).
    pub(crate) caption: crate::caption::Registry,
    /// The registry as [`fill`](Self::fill) last sent it, so an unchanged one is not resent.
    ///
    /// Compared rather than flagged at each declaration: three id compares per fill are
    /// cheaper than a flag every writer has to remember to set.
    caption_sent: crate::caption::Registry,
    /// Region edits this flush produced, drained by [`fill`](Self::fill).
    pending_regions: Vec<RegionOp>,
    /// Scroll-container edits this flush produced, drained by [`fill`](Self::fill).
    pending_scrolls: Vec<ScrollOp>,
    /// What this host's seam has done, carried out by every [`fill`](Self::fill).
    census: crate::seam::AppCensus,
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
    pub content: NodeId,
    pub axes: windows_scene::Axes,
}

pub(crate) use crate::layout::ScrollRow;

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

static NEXT_RUNTIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Host {
    /// Installs one app-thread host with its own text and style stores.
    pub fn install(model: Model, env: Env, root_scope: Scope) {
        crate::signal::assert_writable();
        let (window_owner, window_size) =
            crate::signal::Owner::scope(|| crate::signal::Cell::new(model.window()));
        let host = Self {
            root_pool: Vec::new(),
            bindings: super::binding::Bindings::default(),
            retired: Vec::new(),
            text: super::text::Table::default(),
            styles: super::style::Styles::default(),
            identity: NEXT_RUNTIME.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ramps: Slots::new(),
            ramp_stops: Vec::new(),
            ramp_pool: Vec::new(),
            window_size,
            channels: Vec::new(),
            _window_owner: window_owner,
            popup_requests: Vec::new(),
            model,
            env,
            root_scope,
            geometry_jobs: Slots::new(),
            appearances: Slots::new(),
            surfaces: Slots::new(),
            surfaces_dirty: false,
            theme_update: None,
            theme_backdrop: None,
            mounts: Slots::new(),
            control_ids: Ids::new(),
            controls: Slots::new(),
            fields: Slots::new(),
            field_sources: Vec::new(),
            field_layouts: Vec::new(),
            field_commits: Vec::new(),
            gestures: Vec::new(),
            states: Vec::new(),
            uia_stale: std::cell::Cell::new(true),
            released: Vec::new(),
            trackers: Vec::new(),
            scrolls: Slots::new(),
            probes: Slots::new(),
            regions: Slots::new(),
            caption: crate::caption::Registry::default(),
            caption_sent: crate::caption::Registry::default(),
            pending_regions: Vec::new(),
            pending_scrolls: Vec::new(),
            census: crate::seam::AppCensus::default(),
            overlays: Vec::new(),
        };
        let previous = HOST.with(|slot| slot.borrow_mut().replace(host));
        drop(previous);
    }

    /// Schedules a node-owned binding without entering the host during construction.
    pub(crate) fn binding(
        &mut self,
        node: NodeId,
        update: impl FnMut() + 'static,
    ) -> crate::signal::Effect {
        let effect = self.binding_effect(node, update);
        self.own_binding(node, None, effect);
        effect
    }

    pub(super) fn binding_effect(
        &self,
        node: NodeId,
        mut update: impl FnMut() + 'static,
    ) -> crate::signal::Effect {
        let runtime = self.identity;
        crate::signal::Effect::deferred(move || {
            let live = Self::try_with(|h| runtime == h.identity && h.mounts.get(node).is_some());
            if live == Some(true) {
                update();
            }
        })
    }

    /// Installs the font ladder shared with the renderer on this runtime's text store.
    pub fn install_text(fonts: windows_text::FontLadder) -> windows_core::Result<()> {
        Self::with(|host| host.text.install(fonts))
    }

    /// The window's client extent in DIPs, written only from window resize input.
    /// Structural window presentations may read it; container layouts use responsive rules.
    #[must_use]
    #[cfg(any(test, feature = "test-support"))]
    pub fn window_size() -> crate::signal::Cell<Vector2> {
        Self::with(|host| host.window_size)
    }

    pub(crate) fn request_popup(&mut self, request: crate::overlay::Request) {
        let key = request.key();
        if let Some(pending) = self.popup_requests.iter_mut().find(|r| r.key() == key) {
            if let crate::overlay::Request::Show { body, closed, .. } =
                core::mem::replace(pending, request)
            {
                self.retired.push(super::binding::Retired::Flyout(body));
                self.retired.push(super::binding::Retired::Click(closed));
            }
        } else {
            self.popup_requests.push(request);
        }
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
        crate::signal::assert_writable();
        Self::with_output(f)
    }

    // Only narrow geometry/paint publishers may enter while a draw callback is read-only.
    pub(super) fn with_output<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        match Self::access(f) {
            Ok(out) => {
                Self::drop_retired();
                out
            }
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
        crate::signal::assert_writable();
        match Self::access(f) {
            Ok(out) => {
                Self::drop_retired();
                Some(out)
            }
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

    fn drop_retired() {
        while let Ok(Some(retired)) = Self::access(|host| host.retired.pop()) {
            retired.release();
        }
    }

    /// Returns whether a host is installed on this thread.
    #[must_use]
    #[cfg(any(test, feature = "test-support"))]
    pub fn installed() -> bool {
        HOST.with(|slot| slot.borrow().is_some())
    }

    /// Moves everything this side produced since the last fill into `down`.
    ///
    /// Called straight after [`flush`](Self::flush), which is what fills the region and
    /// scroll edits; the chrome rows, gesture declarations and released ids accumulate from
    /// creation and retirement as well. Every row carries only numbers and ids, so what
    /// crosses is plain `Send` data and the handlers stay on this thread, the only one that
    /// may call them.
    ///
    /// Appends rather than moves the buffers, so both sides keep their capacity and a fill
    /// on a `Down` the consumer has not drained adds to that batch rather than replacing it.
    ///
    /// The caption registry is sent only when it differs from what the last fill sent: the
    /// three ids change when a title bar mounts and at no other time.
    pub(crate) fn fill(&mut self, down: &mut Down) {
        for (_, control) in self.controls.iter_mut() {
            control.accepted_fraction = None;
            if core::mem::take(&mut control.dirty) {
                down.chrome.push(control.front);
            }
        }
        down.field_sources.append(&mut self.field_sources);
        down.field_layouts.append(&mut self.field_layouts);
        down.field_commits.append(&mut self.field_commits);
        down.gestures.append(&mut self.gestures);
        down.released.append(&mut self.released);
        // A backpressured batch can contain a mount and its later unmount. Publish only
        // live declarations: the scene has already destroyed retired nodes before adoption.
        if !down.released.is_empty() {
            down.chrome
                .retain(|row| self.controls.get(row.id).is_some());
            down.gestures
                .retain(|(id, _)| self.controls.get(*id).is_some());
            down.field_sources
                .retain(|row| self.controls.get(row.id).is_some());
            down.field_layouts
                .retain(|row| self.controls.get(row.id).is_some());
            down.field_commits
                .retain(|row| self.controls.get(row.id).is_some());
        }
        down.regions.append(&mut self.pending_regions);
        if let Some(theme) = self.theme_update.take() {
            down.theme = Some(theme);
        }
        down.scrolls.append(&mut self.pending_scrolls);
        if self.caption != self.caption_sent {
            self.caption_sent = self.caption;
            down.caption = Some(self.caption.into());
        }
        down.census = self.census;
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
            let name = match control.name.as_deref() {
                Some(explicit) => out.intern(explicit),
                // Interned rather than borrowed, so an explicit name and a derived one — which
                // is not `'static` — take one path.
                None if self.fields.get(id).is_some() => Default::default(),
                None => control
                    .text
                    .and_then(|key| self.text.str_of(key))
                    .map_or_else(Default::default, |text| out.intern(text)),
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
            let help = control
                .validation
                .map_or(help, |message| out.intern(message));
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
                (UiaRole::Edit, _) if self.fields.get(id).is_some() => Value::EditableText,
                _ => Value::None,
            };
            let mut flags = ColFlags::NONE;
            if control.flyout.is_some() {
                flags = flags | ColFlags::EXPANDS;
            }
            if control.click.is_some()
                || control.front.drive.is_some()
                || self.fields.get(id).is_some()
            {
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
        for (id, row) in self.fields.iter() {
            let password = row.scope == crate::text_input::InputScope::Password;
            out.fields.push(crate::uia::tree::FieldText {
                id,
                revision: row.revision,
                text: if password {
                    std::sync::Arc::from([])
                } else {
                    row.text.clone()
                },
                selection: if password {
                    Default::default()
                } else {
                    row.selection
                },
                geometry: if password { None } else { row.geometry.clone() },
                password,
            });
        }
        out.sort();
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

    /// Calls the handler each intent names.
    ///
    /// An intent queued before its control unmounted is skipped: the generation half of the
    /// id does not match the slot's, so the lookup is a bounds-checked index that finds
    /// nothing rather than a call into whatever occupies that slot.
    pub fn dispatch(intents: &[crate::widget::Intent]) {
        use crate::widget::{Dragging, What};
        enum Call {
            Click(Rc<dyn Fn()>),
            Value(Rc<dyn Fn(f64)>, f64),
            Drag(Rc<dyn Fn(Dragging)>, Dragging),
        }
        for intent in intents {
            let call = Self::with(|host| {
                let control = host.control_mut(intent.target)?;
                match intent.what {
                    What::Hovered(value) => {
                        if let Some(cell) = control.hovered.filter(|cell| cell.alive()) {
                            cell.set(value);
                        }
                        None
                    }
                    What::Tapped => control.click.clone().map(Call::Click),
                    What::Scalar {
                        value,
                        revision,
                        commit,
                    } if control.front.revision == revision => {
                        if let Some(
                            crate::widget::Interaction::Turn(range)
                            | crate::widget::Interaction::Slide(range),
                        ) = control.front.drive
                        {
                            control.accepted_fraction = Some(range.fraction(value));
                        }
                        (if commit {
                            &control.commit
                        } else {
                            &control.change
                        })
                        .clone()
                        .map(|f| Call::Value(f, value))
                    }
                    What::Canceled(revision) if revision == control.front.revision => {
                        control.cancel.clone().map(Call::Click)
                    }
                    What::Scalar { .. } | What::Canceled(_) => None,
                    What::Committed(value) => control.commit.clone().map(|f| Call::Value(f, value)),
                    What::Dragged(update) => control
                        .drag
                        .clone()
                        .map(|f| Call::Drag(f, Dragging::Moved(update))),
                    What::DragEnded { commit } => control.drag.clone().map(|f| {
                        Call::Drag(
                            f,
                            if commit {
                                Dragging::Committed
                            } else {
                                Dragging::Canceled
                            },
                        )
                    }),
                }
            });
            match call {
                Some(Call::Click(f)) => f(),
                Some(Call::Value(f, value)) => f(value),
                Some(Call::Drag(f, value)) => f(value),
                None => {}
            }
        }
    }
    // ── identity ──────────────────────────────────────────────────────────────────

    pub(crate) fn set_escape(&mut self, row: NodeId, f: Rc<dyn Fn()>) {
        if let Some(row) = self.mounts.get_mut(row)
            && let Some(previous) = row.escape.replace(f)
        {
            self.retired.push(super::binding::Retired::Escape(previous));
        }
    }

    /// Takes a callable copy so invoking application code never borrows this host.
    pub(crate) fn escape_handler(&self) -> Option<Rc<dyn Fn()>> {
        self.mounts.iter().find_map(|(node, row)| {
            let size = self.model.solved(node).size;
            (size.x > 0.0 && size.y > 0.0)
                .then(|| row.escape.clone())
                .flatten()
        })
    }

    /// Runs `f` against the first scroll container `pick` accepts.
    ///
    /// A linear scan rather than a map: a screen has a handful of scroll surfaces, and this
    /// is walked from every tracker report of every fling.
    fn scroll_where(&mut self, pick: impl Fn(&ScrollRow) -> bool, f: impl FnOnce(&mut ScrollRow)) {
        for (_, row) in self.scrolls.iter_mut() {
            if pick(row) {
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

    /// Publishes the solved box of every probed node whose box moved.
    ///
    /// Writing a cell marks the graph and raises a frame request; it runs no effect and no
    /// memo, so it cannot re-enter the host's borrow. A reader of the value runs on the next
    /// tick, which is the one-tick lag a probe reports.
    ///
    /// A cell whose owner has been disposed is skipped rather than written, because
    /// `Cell::set` panics on a disposed handle. The two halves of a probe die at different
    /// moments — the cell with its signal scope, the row with its retained node — so neither
    /// drop order is depended on.
    fn publish_probes(&mut self) {
        for (node, cell) in self.probes.iter() {
            let mut now = crate::layout::Placed::from(self.model.solved(node));
            now.scope = self
                .styles
                .get(node)
                .map(|recipe| recipe.scope.at_width(now.class));
            // `set` gates on equality, which is what keeps a probe off the per-frame path: a
            // solve that moved nothing wakes nothing derived from this cell.
            if cell.alive() {
                cell.set(now);
            }
        }
    }

    /// Returns the control the first mounted region occupies. What a test names its target
    /// with; the id is otherwise never handed out.
    #[cfg(test)]
    pub(crate) fn first_region_control(&self) -> Option<ControlId> {
        self.regions.iter().find_map(|(_, row)| row.control)
    }

    /// Returns how many regions are mounted and how many of them satisfy `f`. What a test
    /// asks about the region table, which is otherwise private to the flush.
    #[cfg(test)]
    pub(crate) fn regions_count(
        &self,
        f: impl Fn(&crate::present::RegionRow) -> bool,
    ) -> (usize, usize) {
        self.regions.iter().fold((0, 0), |(all, some), (_, row)| {
            (all + 1, some + usize::from(f(row)))
        })
    }

    /// Emits a mount for every region that has a box and no buffers, and a resize for every
    /// one whose box moved.
    ///
    /// Publishes nothing back into the solve: an extent is read from a solved box and never
    /// stated into one, so this cannot make [`flush`](Self::flush)'s sequence fail to
    /// terminate. It contributes nothing to whether a re-solve is owed, for that reason.
    ///
    /// A box with no area is not ready. A region inside a subtree `when` or `hide_below` has
    /// made `Display::None` is laid out at zero, and buffers allocated against that would be
    /// one texel across for the life of the window — the extent gate is what defers the
    /// mount to the flush that reveals the subtree, since revealing it is a style change.
    fn publish_regions(&mut self) {
        let dpi = self.env.dpi();
        for (node, row) in self.regions.iter_mut() {
            let size = self.model.solved(node).size;
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            let extent = Extent::new(size.x, size.y, dpi);
            let op = match row.build.take() {
                Some(build) => {
                    row.extent = Some(extent);
                    RegionOp::Mount {
                        key: row.key,
                        sink: row.sink,
                        control: row.control,
                        live: row.live.clone(),
                        extent,
                        queue: row.queue,
                        build,
                        theme: row.theme.clone(),
                    }
                }
                // The recorded extent is the only account of whether the box moved, so a
                // solve that moved nothing emits nothing.
                None if row.extent == Some(extent) => continue,
                None => {
                    row.extent = Some(extent);
                    RegionOp::Resize {
                        key: row.key,
                        extent,
                    }
                }
            };
            self.pending_regions.push(op);
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
        if let Some(mut field) = self.fields.take(id) {
            if let Some(callback) = field.callback.take() {
                self.retired.push(super::binding::Retired::Text(callback));
            }
        }
        self.field_sources.retain(|s| s.id != id);
        self.field_layouts.retain(|s| s.id != id);
        self.field_commits.retain(|s| s.id != id);
        if let Some(control) = self.controls.remove(&mut self.control_ids, id) {
            if let Some(cell) = control.hovered.filter(|cell| cell.alive()) {
                cell.set(false);
            }
            self.released.push(id);
            self.uia_stale.set(true);
            control.retire(&mut self.retired);
        }
    }

    // ── values ────────────────────────────────────────────────────────────────────

    pub(crate) fn publish_fraction(&mut self, id: ControlId, fraction: f32, epoch: u64) {
        if let Some(control) = self.control_mut(id) {
            if control.source_epoch != epoch
                || (control.front.source_fraction != fraction
                    && control.accepted_fraction != Some(fraction))
            {
                control.front.revision = control.front.revision.wrapping_add(1);
                control.source_epoch = epoch;
            }
            control.front.fraction = fraction;
            control.front.source_fraction = fraction;
            control.dirty = true;
        }
    }

    /// Publishes travel from solved boxes. Scene owns every value-driven property.
    fn publish_values(&mut self) {
        for (_, control) in self.controls.iter_mut() {
            let Some(node) = control.front.thumb else {
                continue;
            };
            let vertical = match control.front.drive {
                Some(crate::widget::Interaction::Slide(range)) => range.vertical,
                Some(crate::widget::Interaction::Press) => false,
                _ => continue,
            };
            let axis = |v: Vector2| if vertical { v.y } else { v.x };
            let rest = axis(self.model.solved(node).local);
            let travel = (axis(self.model.solved(control.node).size)
                - rest * 2.0
                - axis(self.model.solved(node).size))
            .max(0.0);
            if (rest, travel) != (control.front.rest, control.front.travel) {
                control.front.rest = rest;
                control.front.travel = travel;
                control.dirty = true;
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
        let Some(control) = self.controls.get_mut(id) else {
            return;
        };
        if control.state == state {
            return;
        }
        if state == ModelState::Disabled {
            control.front.revision = control.front.revision.wrapping_add(1);
            control.dirty = true;
        }
        // Recorded whether or not there is anything to repaint: a control with no chrome row
        // still has an automation peer that reports checked, selected or unavailable.
        self.states.push((id, state));
        control.state = state;
        self.repaint_control(id);
    }

    // ── unmount ───────────────────────────────────────────────────────────────────

    fn first_owned_text(&self, node: NodeId) -> Option<MeasureKey> {
        (0..self.model.child_count(node)).find_map(|index| {
            let child = self.model.child(node, index);
            let row = self.mounts.get(child)?;
            if row.control.is_some() {
                return None;
            }
            row.text.or_else(|| self.first_owned_text(child))
        })
    }

    fn detach_control_part(&mut self, node: NodeId, text: Option<MeasureKey>) {
        let mut parent = self.model.parent(node);
        while let Some(at) = parent {
            if let Some(owner) = self.mounts.get(at).and_then(|row| row.control) {
                if let Some(scope) = self
                    .controls
                    .get(owner)
                    .and_then(|row| row.front.hover_scope)
                    && let Some(row) = self.controls.get_mut(scope)
                    && row.front.reveal == node
                {
                    row.front.reveal = NodeId::NONE;
                    row.dirty = true;
                }
                let replacement = text
                    .filter(|key| self.controls.get(owner).unwrap().text == Some(*key))
                    .map(|_| self.first_owned_text(at));
                let control = self.controls.get_mut(owner).unwrap();
                if control.front.thumb == Some(node) {
                    control.front.thumb = None;
                    control.dirty = true;
                }
                if control.front.trail.is_some_and(|(id, _)| id == node) {
                    control.front.trail = None;
                    control.dirty = true;
                }
                for part in &mut control.front.scalar_parts {
                    if part.is_some_and(|(id, _)| id == node) {
                        *part = None;
                        control.dirty = true;
                    }
                }
                if let Some(text) = replacement {
                    control.text = text;
                    self.uia_stale.set(true);
                }
                break;
            }
            parent = self.model.parent(at);
        }
    }

    /// Retires descendants, including reactive branches, before any callback can run.
    pub(super) fn retire_tree(&mut self, at: NodeId) {
        for index in (0..self.model.child_count(at)).rev() {
            let child = self.model.child(at, index);
            self.retire_tree(child);
        }
        if let Some(row) = self.mounts.take(at) {
            self.detach_control_part(at, row.text);
            self.retire_bindings(row.bindings);
            if let Some(escape) = row.escape {
                self.retired.push(super::binding::Retired::Escape(escape));
            }
            self.geometry_jobs.take(at);
            self.surfaces.take(at);
            if row.popup {
                self.request_popup(crate::overlay::Request::Close(at));
            }
            self.styles.take(at);
            if let Some(id) = row.control {
                self.release_control(id);
            }
            if let Some(key) = row.text {
                self.text.release(key, &mut self.model);
            }
            let mut paint = row.paints;
            while let Some(row) = self.appearances.take(paint) {
                self.styles.take(paint);
                paint = row.next;
            }
            if let Some(cell) = self.probes.take(at)
                && cell.alive()
            {
                cell.set(crate::layout::Placed::default());
            }
            // The drop is emitted first and the sink is released after: the region owns the
            // surface handle behind the brush this side is painting with, so the unmount
            // that closes it must be asked for before the claim on the sink goes.
            if let Some(mut region) = self.regions.take(at) {
                self.pending_regions
                    .push(RegionOp::Drop { key: region.key });
                self.model.release(region.sink);
                if let Some(build) = region.build.take() {
                    self.retired.push(super::binding::Retired::Region(build));
                }
            }
            // A tracker is sourced from its viewport's visual, so it is dropped with the row
            // that named it.
            if let Some(scroll) = self.scrolls.take(at) {
                self.pending_scrolls.push(ScrollOp::Drop(at));
                self.model.drop_tracker(scroll.tracker);
                // The thumb's control belongs to this scroll record, so releasing it here
                // keeps its id from outliving the sprite it names.
                if let Some(grab) = scroll.grab {
                    self.release_control(grab);
                }
            }
        }
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
    /// Finally, release the Host borrow to draw dirty paths from settled local boxes,
    /// then emit their geometry and the layout together in the scene batch.
    fn solve(&mut self) {
        let mut measure = |input| self.text.measure(input);
        let mut restyle = |node, class| self.styles.lower(node, class);
        self.model.solve(
            self.env,
            &mut windows_scene::LayoutServices {
                measure: Some(&mut measure),
                restyle: Some(&mut restyle),
            },
        );
    }

    pub fn flush(patch: &mut SinkPatch) {
        crate::signal::flush();
        Self::with(|h| {
            h.census.flushes += 1;
            h.publish_surfaces();
            h.size_overlay_viewports();
            h.solve();
            if h.publish_geometry() {
                h.solve();
            }
            if h.place_overlays() {
                h.solve();
            }
            h.publish_masks();
            h.publish_fields();
            h.publish_overlay_entries();
            h.publish_probes();
            for (node, job) in h.geometry_jobs.iter_mut() {
                let solved = h.model.solved(node);
                let local = (
                    solved.size,
                    h.styles.get(node).unwrap().scope.at_width(solved.class),
                );
                if job.local != Some(local) {
                    job.local = Some(local);
                    if let Some(effect) = job.effect {
                        effect.schedule();
                    }
                    if let Some(pivot) = job.pivot {
                        h.model.bind(
                            node,
                            Prop::Center,
                            windows_scene::Bind::Set(windows_scene::Value::Vec2(Vector2::new(
                                local.0.x * pivot.x,
                                local.0.y * pivot.y,
                            ))),
                        );
                    }
                }
            }
        });
        crate::signal::flush_geometry();
        Self::with(|h| {
            h.publish_channels();
            let mut measure = |input| h.text.measure(input);
            let mut restyle = |node, class| h.styles.lower(node, class);
            h.model.flush(
                patch,
                h.env,
                &mut windows_scene::LayoutServices {
                    measure: Some(&mut measure),
                    restyle: Some(&mut restyle),
                },
            );
        });
    }

    /// Publishes everything whose value is a function of the solve, and returns whether any
    /// of it moved a box.
    fn publish_geometry(&mut self) -> bool {
        let text = self.text.publish(&mut self.model, &self.styles);
        let scrolls = self.publish_scrolls();
        // Values bind compositor properties and dirty no layout, so they are published here
        // for ordering and contribute nothing to whether a re-solve is owed.
        self.publish_values();
        // Probes publish after overlay placement in `flush`, when absolute rects are final.
        // Neither do regions: an extent goes out to the present thread and nothing comes
        // back into the solve.
        self.publish_regions();
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
        self.model.destroy(root.node(), Exit::None);
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
        self.mint_control(ControlRow::new(NodeId::NONE, self.root_scope))
    }

    /// Returns a clone of the control's flyout body, or `None` where it declared none.
    ///
    /// Cloned rather than borrowed: building the body is application code and must run after
    /// the host's borrow is released. The row keeps its own handle, since a picker's flyout
    /// opens once per press and not once per lifetime.
    pub(crate) fn flyout_of(&self, target: ControlId) -> Option<Rc<dyn Fn(&mut super::Ui<'_>)>> {
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
    pub(crate) fn name_of(&self, target: ControlId) -> Option<&str> {
        self.control(target).and_then(|control| control.name.as_deref())
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
    pub(crate) fn open_overlay_entry(
        &mut self,
        depth: u32,
        node: NodeId,
        slide: crate::overlay::Slide,
    ) {
        self.model.suspend_input(node, true);
        self.overlays[depth as usize].entry = Some(OverlayEntry {
            node,
            slide,
            started: false,
            finished: false,
            window: self.model.window(),
            rect: None,
        });
    }

    pub(crate) fn complete_overlay_entry(&mut self, node: NodeId) {
        for placement in &mut self.overlays {
            if let Some(entry) = &mut placement.entry
                && entry.node == node
                && entry.started
            {
                entry.finished = true;
                self.model.suspend_input(node, false);
            }
        }
    }

    // Only the first solved box starts motion. Changed geometry snaps in one event-rate write.
    fn publish_overlay_entries(&mut self) {
        use windows_scene::{Bind, Easing, Iterations, Prop, Value};
        let window = self.model.window();
        for placement in &mut self.overlays {
            let Some(entry) = &mut placement.entry else {
                continue;
            };
            if entry.finished {
                continue;
            }
            let solved = self.model.solved(entry.node);
            if solved.size.x <= 0.0 || solved.size.y <= 0.0 {
                continue;
            }
            if window != entry.window || entry.rect.is_some_and(|rect| rect != solved.rect) {
                self.model.bind(
                    entry.node,
                    Prop::Offset,
                    Bind::Set(Value::Vec2(solved.local)),
                );
                self.model.suspend_input(entry.node, false);
                entry.finished = true;
            } else if !entry.started {
                let from = Vector2 {
                    x: solved.local.x + entry.slide.by.x * solved.size.x,
                    y: solved.local.y + entry.slide.by.y * solved.size.y,
                };
                let anim = self.model.frames(
                    &[
                        (0.0, Value::Vec2(from), Easing::Linear),
                        (1.0, Value::Vec2(solved.local), entry.slide.easing),
                    ],
                    entry.slide.ms,
                    Iterations::Count(1),
                );
                self.model
                    .bind(entry.node, Prop::Offset, Bind::Animate(anim));
                entry.started = true;
                entry.rect = Some(solved.rect);
            }
        }
    }

    fn size_overlay_viewports(&mut self) {
        let window = self.model.window();
        for placement in &mut self.overlays {
            let Some(insets) = placement.viewport else {
                continue;
            };
            let [left, top, right, bottom] =
                insets.map(|len| len.dips(self.root_scope).unwrap_or(0.0).max(0.0));
            let bounds = windows_scene::Rect::new(
                left,
                top,
                (window.x - right).max(left),
                (window.y - bottom).max(top),
            );
            if placement.bounds == Some(bounds) {
                continue;
            }
            placement.bounds = Some(bounds);
            self.model.style(
                placement.root.node(),
                &crate::layout::viewport_style(Vector2 {
                    x: bounds.x1 - bounds.x0,
                    y: bounds.y1 - bounds.y0,
                }),
            );
        }
    }

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
                AnchorTo::Window => self.overlays[index]
                    .bounds
                    .unwrap_or(windows_scene::Rect::new(0.0, 0.0, window.x, window.y)),
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
            if self.model.input_suspended(spec.viewport.node()) {
                return true;
            }
            if size.x <= 0.0 || size.y <= 0.0 {
                return true;
            }
            self.model.create_tracker(spec.id, spec.viewport, spec.axes);
            // A binding to a tracker that does not exist is discarded by the scene.
            // Publish it with creation, including when a hidden viewport is first shown.
            self.model.bind(
                spec.content,
                Prop::OffsetY,
                windows_scene::Bind::Track {
                    tracker: spec.id,
                    axis: windows_scene::TrackerAxis::PositionY,
                    affine: windows_scene::Affine::CONTENT,
                },
            );
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
            let (added, describe) = (scroll.front_added, scroll.describe(id));
            let box_ = self.model.solved(viewport).size;
            // A viewport with no area has not been laid out — a hidden subtree solves at zero
            // — and publishing from that zero would record `last` as sent while the bounds
            // went to a tracker that does not exist yet. The equality gate would then never
            // send them again. An unmeasured container publishes nothing and remembers
            // nothing, so the flush that gives it a box is the one that publishes.
            if box_.x <= 0.0 || box_.y <= 0.0 || self.model.input_suspended(viewport) {
                continue;
            }
            let viewport_h = box_.y;
            // The realization window is a fraction of the viewport height, which a
            // virtualized list cannot compute for itself.
            if let Some(state) = state {
                state.resized(viewport_h);
            }
            let geom = crate::layout::thumb_geom(viewport_h, self.model.solved(content).size.y);
            // Before the gate below, because a container whose content fits publishes the
            // same geometry it was minted with and would otherwise never reach the front
            // table at all — leaving its thumb's reveal with nothing to act on.
            if !added {
                if let Some(scroll) = self.scrolls.get_mut(id) {
                    scroll.front_added = true;
                }
                self.pending_scrolls.push(ScrollOp::Add(describe));
            }
            if geom == last {
                continue;
            }
            if let Some(scroll) = self.scrolls.get_mut(id) {
                scroll.last = geom;
            }
            self.pending_scrolls.push(ScrollOp::Geom { id, geom });
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
                self.model.visual_rect(
                    thumb,
                    Vector2::new(crate::layout::THUMB_MARGIN, 0.0),
                    Vector2::new(crate::layout::THUMB_W, geom.thumb_h),
                );
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
        self.text.reemit(&mut self.model);
    }

    /// Sets the window's size in DIPs, from the window's own resize message.
    pub fn set_window(&mut self, size: Vector2) {
        self.window_size.set(size);
        self.model.set_window(size);
    }

    /// Sets the pixel grid everything is snapped to and rasterized for.
    pub fn set_env(&mut self, env: Env) {
        self.env = env;
    }

    /// Returns the retained model and scene output writer.
    pub(crate) fn model(&mut self) -> &mut Model {
        &mut self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::tests::fixture;
    use crate::seam::Down;

    #[test]
    fn retirement_wins_when_mount_and_unmount_share_a_backpressured_batch() {
        let mut patch = fixture();
        let mut down = Down::default();
        let root = Host::with(|h| h.model().root());
        let held = crate::build::Ui::mount_at(root, None, crate::build::root_scope(), None, |ui| {
            crate::layout::stack(ui, |ui| {
                crate::widget::button(ui, "Gain");
                crate::widget::field(ui, "name");
            });
        });
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        assert!(!down.chrome.is_empty() && !down.field_sources.is_empty());
        let old: Vec<_> = down.chrome.iter().map(|row| row.id).collect();
        drop(held);
        let _replacement =
            crate::build::Ui::mount_at(root, None, crate::build::root_scope(), None, |ui| {
                crate::widget::button(ui, "replacement");
            });
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        assert!(old.iter().all(|id| down.released.contains(id)));
        assert!(
            !down.chrome.is_empty(),
            "the replacement generation survives"
        );
        assert!(down.chrome.iter().all(|row| !old.contains(&row.id)));
        assert!(down.gestures.iter().all(|(id, _)| !old.contains(id)));
        assert!(down.field_sources.is_empty() && down.field_layouts.is_empty());
    }

    /// A fill hands every buffer over and leaves the host with none of it.
    ///
    /// Both halves matter. A fill that copied rather than moved would re-send every control
    /// on the next tick, and the front table would adopt a row per tick for the life of the
    /// window.
    #[test]
    fn a_fill_hands_over_every_buffer_and_empties_the_host() {
        let mut patch = fixture();
        let held = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                crate::widget::button(ui, "press me");
            },
        );
        Host::flush(&mut patch);

        let mut down = Down::default();
        Host::with(|h| h.fill(&mut down));
        assert!(!down.chrome.is_empty(), "the control minted no front row");
        assert_eq!(down.gestures.len(), 1, "a button declares one gesture");
        assert!(down.released.is_empty(), "nothing has unmounted yet");
        // A delta, because the fixture has already flushed: what the census counts is every
        // flush this host has run, not every flush since the last fill.
        let flushes = down.census.flushes;

        let mut second = Down::default();
        Host::with(|h| h.fill(&mut second));
        assert!(
            second.chrome.is_empty() && second.gestures.is_empty(),
            "the host kept what it had already handed over"
        );

        drop(held);
        Host::flush(&mut patch);
        let mut third = Down::default();
        Host::with(|h| h.fill(&mut third));
        assert_eq!(
            third.released.len(),
            1,
            "the unmounted control was not released"
        );
        assert_eq!(
            third.census.flushes,
            flushes + 1,
            "the flush went uncounted"
        );
    }

    /// A fill appends, so a batch the consumer has not drained grows rather than being
    /// replaced, and the buffer keeps its capacity.
    #[test]
    fn a_fill_appends_to_an_undrained_batch() {
        let mut patch = fixture();
        let _held = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                crate::widget::button(ui, "one");
            },
        );
        Host::flush(&mut patch);
        let mut down = Down::default();
        Host::with(|h| h.fill(&mut down));
        let first = down.gestures.len();

        let _second = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                crate::widget::button(ui, "two");
            },
        );
        Host::flush(&mut patch);
        Host::with(|h| h.fill(&mut down));
        assert_eq!(
            down.gestures.len(),
            first + 1,
            "the second mount replaced the first batch instead of joining it"
        );
    }

    /// The window commands cross once, on the fill after the bar mounted, and not again.
    #[test]
    fn the_caption_registry_crosses_only_when_it_changes() {
        let mut patch = fixture();
        let _held = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                crate::layout::row(ui, |ui| {
                    crate::widget::button(ui, "\u{2715}")
                        .caption(windows_window::CaptionButton::Close);
                });
            },
        );
        Host::flush(&mut patch);

        let mut down = Down::default();
        Host::with(|h| h.fill(&mut down));
        let ids = down.caption.expect("the bar declared a command");
        assert!(
            ids.iter().any(Option::is_some),
            "no command reached the seam"
        );

        down.clear();
        Host::flush(&mut patch);
        Host::with(|h| h.fill(&mut down));
        assert_eq!(down.caption, None, "an unchanged registry was resent");
    }
}
