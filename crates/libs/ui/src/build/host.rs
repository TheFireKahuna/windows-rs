//! Owns the app thread's one tree, every mint authority, the control table and the pending
//! patch.
//!
//! `Host` is app-thread affine and is the only holder of a `SinkPatch`, which is what keeps a
//! layer that can reach the host from moving a pixel out of turn. Construction borrows these
//! stores through `Ui`; UI bindings defer their first run until that borrow ends. Event
//! handlers and retired callbacks execute outside the borrow.

use super::binding::{HandlerTable, Retired};
use super::control::ControlRow;
use super::hits::{self, HitBuilder};
use super::tree::{self, Geom, Pool, Tree};
use crate::layout::{Align, Anchors, Len, Placed, Probe, Rect, solve};
use crate::role::Scope;
use crate::signal::Cell;
use crate::widget::{Gesturing, Intent, ModelState, ValueRow, What};
use std::cell::RefCell;
use std::rc::Rc;
use windows_numerics::Vector2;
use windows_scene::{
    Anim, Attach, Axes, Bind, CONTROL, Cap, ControlId, DELAY, DashId, DelayId, Easing, Env, Exit,
    GEOM, GeomId, GroupId, Halo, HitDecl, Id, Ids, Ink, Iterations, Join, Mask, NodeId, NodeKind,
    Op, Paint, PathVerb, Prop, RAMP, RampId, RegionId, ResOp, RunId, SinkPatch, Slots, Span,
    Spread, SpriteId, StrokeStyle, TRACKER, TrackerId, TrackerOp, Value,
};

mod rounded;

// ── the rows beside the tree ────────────────────────────────────────────────────────

/// Holds one open overlay's placement rule and where it last landed.
///
/// Resolving a placement needs the solve — the overlay's measured size and its anchor's rect
/// — so the row sits beside the tree. The layer above owns the overlay's lifetime; `Host`
/// owns its geometry.
#[derive(Copy, Clone)]
pub(crate) struct Placement {
    pub root: NodeId,
    pub blocker: Option<ControlId>,
    /// The control this overlay opened from, where it opened from one. What automation
    /// reports as that control's expand-collapse state, which is otherwise a button that
    /// answers "collapsed" with its own menu open beside it.
    pub invoker: Option<ControlId>,
    pub anchor: crate::overlay::Anchor,
    /// The client box this overlay is inset into, resolved: its origin is where the insets
    /// put it, and its extent is written into the root's own authored bounds before each solve.
    pub viewport: Rect,
    pub at: Vector2,
    pub entry: Option<Entrance>,
}

/// Runs an entrance once, then returns the node to ordinary layout placement.
#[derive(Copy, Clone)]
pub(crate) struct Entrance {
    pub node: NodeId,
    pub slide: crate::overlay::Slide,
    /// DIP displacement with an opacity entrance; otherwise `slide.by` is fractional.
    dip_fade: bool,
    delay_ms: u32,
    /// Whether the slide is on the compositor. Set when it starts and read to keep the next
    /// publication from binding the same curve again.
    started: bool,
    done: bool,
}

impl Entrance {
    pub(crate) fn running(self) -> bool {
        !self.done
    }

    pub(crate) fn new(node: NodeId, slide: crate::overlay::Slide) -> Self {
        Self {
            node,
            slide,
            dip_fade: false,
            delay_ms: 0,
            started: false,
            done: false,
        }
    }
}

/// What a node declares rarely, pooled and headed by the `side` column.
///
/// One row for all of them rather than a head column each: an ordinary node names none and
/// pays four bytes, a node that names one pays one row however many it names, and retirement
/// reads one row rather than probing seven tables.
struct Side {
    node: NodeId,
    escape: Option<Rc<dyn Fn()>>,
    probe: Option<Probe>,
    origin: Option<Anchors>,
    /// A pivot stated as a fraction of this node's own solved box, resolved at publication.
    pivot: Option<Vector2>,
    /// The centre the pivot last resolved to, so a box that did not move re-sends nothing.
    /// `NaN` until the first publication.
    centre: Vector2,
    /// A derived sprite's own geometry, which takes no space from its parent.
    visual: Visual,
    geometry: u32,
    scroll: u32,
    region: u32,
    surface: u32,
    rounded: u32,
}

impl Default for Side {
    fn default() -> Self {
        Self {
            node: NodeId::NONE,
            escape: None,
            probe: None,
            origin: None,
            pivot: None,
            centre: Vector2 {
                x: f32::NAN,
                y: f32::NAN,
            },
            visual: Visual::Unplaced,
            geometry: tree::NONE,
            scroll: tree::NONE,
            region: tree::NONE,
            surface: tree::NONE,
            rounded: tree::NONE,
        }
    }
}

/// A derived sprite's own box: stated outright, or inset from the owner's solved box.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub(crate) enum Visual {
    #[default]
    Unplaced,
    Rect(Vector2, Vector2),
    Insets([f32; 4]),
}

/// One attachment to a keyed anchor set, in attachment order.
///
/// One flat list rather than a map per set: an attachment is written once and read once per
/// flush, and publication walks it to drop the entries whose node has unmounted, which is the
/// same pass that would have to check them anyway.
struct Attachment {
    set: Anchors,
    key: u64,
    node: NodeId,
}

// ── the host ────────────────────────────────────────────────────────────────────────

pub struct Host {
    pub(crate) tree: Tree,
    /// Distinct scopes, interned: the `scope` column is an index into this. A window holds a
    /// handful, so the intern is a scan over a short vector at creation and off every walk.
    scopes: Vec<Scope>,
    /// The one `SinkPatch` on this thread.
    pub(crate) pending: SinkPatch,
    pub(crate) env: Env,
    pub(super) window: Cell<Vector2>,
    root: NodeId,
    /// Resolved metric values per width class, filled at theme install, so `Len::resolve` on
    /// the solver's walk is an index and a multiply.
    pub(crate) metrics: [[f32; crate::role::BUILTIN_METRICS]; 3],

    /// One index authority for all five resource families.
    ///
    /// The scene's table keys on family as well as index, so one free list serves them all
    /// and a release is one call rather than a five-arm match on the family parameter.
    res_ids: Ids<GEOM>,
    tracker_ids: Ids<TRACKER>,
    delay_ids: Ids<DELAY>,
    control_ids: Ids<CONTROL>,

    pub(crate) controls: Slots<CONTROL, ControlRow>,
    pub(crate) handlers: HandlerTable,
    /// One row per installed channel writer, chained from the node's `bindings` head.
    pub(crate) binders: super::binding::Binders,
    sides: Pool<Side>,
    rounded: Pool<rounded::Rounded>,
    anchors: Vec<Attachment>,
    pub(crate) overlays: Vec<Placement>,
    entrances: Vec<Entrance>,
    /// The authored stops of every live ramp, so a theme change re-resolves them.
    ramps: Slots<RAMP, (Vec<super::Stop>, Spread)>,
    scratch_stops: Vec<(u16, windows_color::Radiance)>,

    pub(crate) text: super::text::Table,
    pub(crate) appearances: super::theme::Appearances,
    pub(crate) fields: Slots<CONTROL, super::field::Row>,
    /// Field declarations produced since the last fill: application sources, shaped layouts and
    /// the commits the application answered. `fill` moves them onto the seam.
    pub(crate) field_sources: Vec<crate::text_input::Source>,
    pub(crate) field_layouts: Vec<crate::text_input::Layout>,
    pub(crate) field_commits: Vec<crate::text_input::Commit>,
    pub(crate) scrolls: Pool<crate::layout::ScrollRow>,
    pub(crate) regions: Pool<crate::present::RegionRow>,
    pub(crate) geometry: super::geometry::Jobs,

    hits: HitBuilder,
    /// Which control is which window command, and the registry as the last fill sent it:
    /// three id compares per fill are cheaper than a flag every writer has to remember.
    pub(crate) caption: [Option<ControlId>; 3],
    sent_caption: [Option<ControlId>; 3],

    /// Controls whose front row may have changed since the last fill.
    ///
    /// A row is declared across several setters after it is minted, so what crosses is the row as
    /// it stands at the fill, not as it stood at the mint. Duplicates are removed there.
    chrome_touched: Vec<ControlId>,
    pub(crate) values: Vec<(ControlId, ValueRow)>,
    pub(crate) gestures: Vec<(ControlId, crate::gesture::GestureDecl)>,
    pub(crate) focus_ops: Vec<crate::seam::FocusOp>,
    pub(crate) released: Vec<ControlId>,
    /// What a presentation region declares about its own pixels, for automation to join with
    /// the renderer's geometry. One row per region, written as the region is declared.
    pub(crate) peers: Vec<crate::uia::RegionPeer>,
    pub(crate) region_ops: Vec<crate::seam::RegionOp>,
    pub(crate) scroll_ops: Vec<crate::seam::ScrollOp>,
    pub(crate) popups: Vec<crate::overlay::Request>,
    pub(crate) uia_stale: core::cell::Cell<bool>,
    /// The id paths the last publish used, held for their allocations.
    uia_seen: Vec<(String, u32)>,
    census: crate::seam::AppCensus,
    /// Values whose drop runs application code, held until the borrow ends.
    pub(crate) retired: Vec<Retired>,
    /// The queued channel writes, keyed by `(node, prop)`.
    pub(crate) queued: Vec<(NodeId, Prop, Bind)>,
    scratch: Vec<NodeId>,
    scratch_text: String,
    /// Root lists a retired mount handed back, so a warm mount reuses one rather than
    /// allocating its own.
    root_pool: Vec<Vec<NodeId>>,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

/// Why the host could not be reached. Each cause has its own message and its own fix.
pub enum Unreachable {
    NotInstalled,
    Reentered,
    TearingDown,
}

impl Host {
    /// Installs one app-thread host with its own text and style stores.
    pub fn install(env: Env, root_scope: Scope) {
        HOST.with(|slot| {
            let mut host = Self {
                tree: Tree::default(),
                scopes: vec![root_scope],
                pending: SinkPatch::default(),
                env,
                window: Cell::new(Vector2 { x: 0.0, y: 0.0 }),
                root: NodeId::NONE,
                metrics: [[0.0; crate::role::BUILTIN_METRICS]; 3],
                res_ids: Ids::default(),
                tracker_ids: Ids::default(),
                delay_ids: Ids::default(),
                control_ids: Ids::default(),
                controls: Slots::default(),
                handlers: HandlerTable::default(),
                binders: Pool::default(),
                sides: Pool::default(),
                rounded: Pool::default(),
                anchors: Vec::new(),
                overlays: Vec::new(),
                entrances: Vec::new(),
                ramps: Slots::default(),
                scratch_stops: Vec::new(),
                text: super::text::Table::default(),
                appearances: super::theme::Appearances::default(),
                fields: Slots::default(),
                field_sources: Vec::new(),
                field_layouts: Vec::new(),
                field_commits: Vec::new(),
                scrolls: Pool::default(),
                regions: Pool::default(),
                geometry: super::geometry::Jobs::default(),
                hits: HitBuilder::default(),
                caption: [None; 3],
                sent_caption: [None; 3],
                chrome_touched: Vec::new(),
                values: Vec::new(),
                gestures: Vec::new(),
                focus_ops: Vec::new(),
                released: Vec::new(),
                peers: Vec::new(),
                uia_seen: Vec::new(),
                region_ops: Vec::new(),
                scroll_ops: Vec::new(),
                popups: Vec::new(),
                uia_stale: core::cell::Cell::new(false),
                census: crate::seam::AppCensus::default(),
                retired: Vec::new(),
                queued: Vec::new(),
                scratch: Vec::new(),
                scratch_text: String::new(),
                root_pool: Vec::new(),
            };
            host.root = host.tree.mint(0);
            host.tree.c.layout[host.root.index()] = crate::layout::Layout::window();
            host.pending.push(Op::New {
                id: host.root,
                kind: NodeKind::Group,
                parent: Attach::Window,
                after: None,
            });
            // The solve reads lengths through this table, so it is filled before the first one
            // runs rather than by the first theme change.
            host.fill_metrics(root_scope);
            *slot.borrow_mut() = Some(host);
        });
    }

    /// Runs `f` against the thread's host.
    ///
    /// `f` must not call application code: it runs under the host's borrow, and an `Effect`
    /// created there runs its closure immediately and re-enters that borrow.
    ///
    /// # Panics
    ///
    /// Panics if no host is installed, and separately if `f` re-enters. The message names
    /// which of the two happened, because the two have opposite fixes.
    pub fn with<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        match Self::enter(f) {
            Ok(r) => r,
            Err(Unreachable::Reentered) => panic!("a Host::with body reached back into the host"),
            Err(_) => panic!("no host is installed on this thread"),
        }
    }

    /// Runs `f` against the thread's host, answering `None` where there is none to reach.
    ///
    /// For callers that run inside a `Drop`: reaching a thread-local during its own
    /// destruction phase fails, and a panic inside a `Drop` aborts the process. A host that is
    /// already gone leaves nothing to release. Re-entry is asserted in debug builds rather
    /// than ignored: dropping a mount inside a `Host::with` body leaks the whole subtree.
    pub fn try_with<R>(f: impl FnOnce(&mut Self) -> R) -> Option<R> {
        match Self::enter(f) {
            Ok(r) => Some(r),
            Err(Unreachable::Reentered) => {
                // Not while unwinding: a second panic inside a `Drop` aborts the process and hides
                // the first, which is the one worth reading.
                debug_assert!(
                    std::thread::panicking(),
                    "a mount was dropped inside a Host::with body"
                );
                None
            }
            Err(_) => None,
        }
    }

    fn enter<R>(f: impl FnOnce(&mut Self) -> R) -> Result<R, Unreachable> {
        HOST.try_with(|slot| {
            let mut borrow = slot.try_borrow_mut().map_err(|_| Unreachable::Reentered)?;
            let host = borrow.as_mut().ok_or(Unreachable::NotInstalled)?;
            let r = f(host);
            // Dropping a retired value runs whatever the application captured, so the borrow
            // is released before the drop rather than during it.
            let retired = core::mem::take(&mut host.retired);
            drop(borrow);
            drop(retired);
            Ok(r)
        })
        .unwrap_or(Err(Unreachable::TearingDown))
    }

    pub fn installed() -> bool {
        HOST.try_with(|slot| slot.borrow().is_some())
            .unwrap_or(false)
    }

    /// The window's client extent in DIPs, written only from window resize input.
    pub fn window_size() -> Cell<Vector2> {
        Self::with(|h| h.window)
    }

    /// The window's client extent, read without re-entering the host.
    pub(crate) fn window_extent(&self) -> Vector2 {
        self.window.get()
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn set_window(&mut self, size: Vector2) {
        self.tree.window_resized |= self.window.get() != size;
        self.window.set(size);
        let root = self.root;
        self.tree.author(root, |l| {
            l.width = Len::dip(size.x);
            l.height = Len::dip(size.y);
        });
    }

    /// Sets the pixel grid everything is snapped to and rasterized for.
    pub fn set_env(&mut self, env: Env) {
        self.env = env;
        let root = self.root;
        self.tree.mark(root);
    }

    pub(crate) fn scope_of(&self, node: NodeId) -> Scope {
        self.scopes[self.tree.c.scope[node.index()] as usize]
    }

    pub(crate) fn scope_at(&self, at: u32) -> Scope {
        self.scopes[at as usize]
    }

    /// The window's own scope, which every ramp and every derived paint resolves through.
    pub(crate) fn root_scope(&self) -> Scope {
        self.scopes[0]
    }

    /// Interns `scope`, so the column holds an index rather than a whole scope per node.
    pub(crate) fn intern(&mut self, scope: Scope) -> u32 {
        match self.scopes.iter().position(|&s| s == scope) {
            Some(at) => at as u32,
            None => {
                self.scopes.push(scope);
                self.scopes.len() as u32 - 1
            }
        }
    }

    /// Takes a cleared root list for a declaration transaction to fill.
    pub(crate) fn take_roots(&mut self) -> Vec<NodeId> {
        self.root_pool.pop().unwrap_or_default()
    }

    /// Takes a retired mount's list back, cleared and with its capacity.
    pub(crate) fn give_roots(&mut self, mut roots: Vec<NodeId>) {
        roots.clear();
        self.root_pool.push(roots);
    }

    /// Replaces the root scope every other scope is rebased onto.
    pub(crate) fn rebase_scopes(&mut self, root: Scope) {
        self.scopes[0] = root;
        for at in 1..self.scopes.len() {
            self.scopes[at] = self.scopes[at].in_theme(root);
        }
    }

    // ── emit ────────────────────────────────────────────────────────────────────────
    //
    // Every method here appends to `pending` and holds no state of its own. They are the
    // whole of what this side may say to the scene.

    pub(crate) fn group(&mut self, parent: GroupId, after: Option<NodeId>) -> GroupId {
        GroupId(self.mint_under(parent.0, after, NodeKind::Group, 0))
    }

    pub(crate) fn sprite(&mut self, parent: GroupId, after: Option<NodeId>) -> SpriteId {
        SpriteId(self.mint_under(parent.0, after, NodeKind::Sprite, tree::SPRITE))
    }

    /// Mints a node under `parent` carrying `bits` from before it is linked, because whether a
    /// link is a layout input depends on them.
    fn mint_under(
        &mut self,
        parent: NodeId,
        after: Option<NodeId>,
        kind: NodeKind,
        bits: tree::Bits,
    ) -> NodeId {
        let id = self.tree.mint(self.tree.c.scope[parent.index()]);
        self.tree.c.flags[id.index()] |= bits;
        self.tree.link(id, parent, after);
        // A fresh slot's pair is zero and the link marked only the parent, so a node given no
        // setter of its own would be measured from that zero.
        self.tree.mark(id);
        self.pending.push(Op::New {
            id,
            kind,
            parent: Attach::Node(parent),
            after,
        });
        id
    }

    /// Mints a group with no parent, for an overlay's own root.
    pub(crate) fn overlay_root(&mut self, scope: u32) -> GroupId {
        let id = self.tree.mint(scope);
        self.tree.roots_dirty = true;
        self.pending.push(Op::New {
            id,
            kind: NodeKind::Group,
            parent: Attach::Overlay,
            after: None,
        });
        GroupId(id)
    }

    /// Gives an overlay's slot root the element its contents are announced inside.
    ///
    /// Declares no hit entry: an overlay is positioned, not pressed, and the rows inside it
    /// carry their own. Only the control column is written, which is what the automation walk
    /// reads.
    pub(crate) fn name_overlay(
        &mut self,
        root: NodeId,
        scope: u32,
        kind: crate::overlay::Kind,
        invoker: Option<ControlId>,
        name: Option<&'static str>,
    ) {
        use crate::overlay::Kind;
        let scope = self.scope_at(scope);
        // The control it opened from, so a reader announces "Channel scope menu" rather than
        // an unnamed container. A menu is the one place a control's name belongs to two
        // elements, because the menu is what that control turned into.
        let named = name.map(std::borrow::Cow::Borrowed).or_else(|| {
            let row = self.control(invoker?)?;
            row.name.clone().or_else(|| {
                self.text
                    .str_of(row.text?)
                    .map(|text| std::borrow::Cow::Owned(text.to_owned()))
            })
        });
        let combo = invoker
            .and_then(|id| self.control(id))
            .is_some_and(|row| row.uia == crate::widget::UiaRole::ComboBox);
        let id = self.mint_control(ControlRow::blank(root, scope));
        self.tree.c.control[root.index()] = id;
        if let Some(row) = self.control_mut(id) {
            row.uia = match kind {
                Kind::Flyout if combo => crate::widget::UiaRole::List,
                Kind::Flyout => crate::widget::UiaRole::Menu,
                Kind::Popup => crate::widget::UiaRole::Group,
                Kind::Tooltip => crate::widget::UiaRole::ToolTip,
            };
            row.overlay = Some(kind);
            row.name = named;
        }
    }

    /// Mints a sprite whose geometry is its own rather than the solve's: a text line tile, a
    /// wash, a scroll thumb. It takes no space from its parent and is skipped by the walks.
    pub(crate) fn visual(&mut self, parent: GroupId, after: Option<NodeId>) -> SpriteId {
        let bits = tree::SPRITE | tree::DERIVED;
        SpriteId(self.mint_under(parent.0, after, NodeKind::Sprite, bits))
    }

    /// Places a derived sprite at its own box.
    ///
    /// Written into the geometry column here as well as recorded on the side row: the box is
    /// complete, so a sprite a publisher places after the solve crosses in the same flush,
    /// and the visuals pass re-reads the row only to take a hidden one off the screen.
    pub(crate) fn visual_rect(&mut self, id: SpriteId, offset: Vector2, size: Vector2) {
        self.side_mut(id.0).visual = Visual::Rect(offset, size);
        if self.tree.c.flags[id.0.index()] & tree::HIDDEN == 0 {
            self.tree.c.geom[id.0.index()] = Geom {
                local: offset,
                size,
                ..Geom::default()
            };
            self.tree.touch(id.0);
        }
        self.tree.mark(id.0);
    }

    pub(crate) fn visual_insets(&mut self, id: SpriteId, insets: [f32; 4]) {
        self.side_mut(id.0).visual = Visual::Insets(insets);
        self.tree.mark(id.0);
    }

    pub(crate) fn place(&mut self, id: NodeId, parent: GroupId, after: Option<NodeId>) {
        self.tree.link(id, parent.0, after);
        self.pending.push(Op::Move {
            id,
            parent: Attach::Node(parent.0),
            after,
        });
    }

    /// Destroys a subtree, handing the scene the box and the clip chain its ghost is mounted
    /// and sized from.
    ///
    /// The origin and the bounds are read here because this side already holds them: the
    /// scene half would have to walk the doomed node's ancestry to recover either, and it is
    /// walking to destroy it at the same moment.
    pub(crate) fn destroy(&mut self, id: NodeId, exit: Exit) {
        let origin = self.tree.c.geom[id.index()].rect;
        let bounds = self.clip_bounds(id);
        self.pending.push(Op::Drop {
            id,
            exit,
            origin: Vector2 {
                x: origin.x0,
                y: origin.y0,
            },
            bounds,
        });
    }

    /// The tightest clip rect `id` sits inside, in absolute DIPs, or `None` where nothing
    /// above it clips.
    fn clip_bounds(&self, id: NodeId) -> Option<[f32; 4]> {
        let mut at = self.tree.parent(id);
        let mut held: Option<[f32; 4]> = None;
        while !at.is_none() {
            if self.tree.c.flags[at.index()] & tree::CLIP != 0 {
                let rect = self.tree.c.geom[at.index()].rect;
                held = Some(match held {
                    Some([l, t, r, b]) => [
                        l.max(rect.x0),
                        t.max(rect.y0),
                        r.min(rect.x1),
                        b.min(rect.y1),
                    ],
                    None => [rect.x0, rect.y0, rect.x1, rect.y1],
                });
            }
            at = self.tree.parent(at);
        }
        held
    }

    pub(crate) fn hide(&mut self, id: NodeId, hidden: bool) {
        self.tree.set_flag(id, tree::HIDDEN, hidden);
    }

    pub(crate) fn suspend_input(&mut self, id: NodeId, suspended: bool) {
        self.tree.set_flag(id, tree::SUSPENDED, suspended);
        self.uia_stale.set(true);
    }

    /// Records or clears what this node declares about the hit array.
    ///
    /// Additive in flags is the caller's business: this writes the declaration it is given,
    /// and clearing it is `None`.
    pub(crate) fn hit(&mut self, id: NodeId, decl: Option<HitDecl>) {
        if !self.tree.is_live(id) {
            return;
        }
        let at = id.index();
        let mut flags = self.tree.c.flags[at] & !(tree::HIT | tree::DECL);
        let (mut control, mut inflate) = (self.tree.c.control[at], self.tree.c.inflate[at]);
        if let Some(decl) = decl {
            flags |= tree::HIT | (tree::pack_decl(decl.flags.bits()) << tree::DECL_SHIFT);
            control = decl.id;
            inflate = decl.touch_inflate.unwrap_or(f32::NAN);
        }
        // Idempotent: a publisher restating the declaration it made last flush rebuilds
        // nothing. The inflation compares bitwise because its absent value is NaN.
        if flags == self.tree.c.flags[at]
            && control == self.tree.c.control[at]
            && inflate.to_bits() == self.tree.c.inflate[at].to_bits()
        {
            return;
        }
        self.tree.c.flags[at] = flags;
        self.tree.c.control[at] = control;
        self.tree.c.inflate[at] = inflate;
        self.tree.hits_dirty = true;
    }

    pub(crate) fn mask(&mut self, id: SpriteId, mask: Mask) {
        self.pending.push(Op::Mask { id, mask });
    }

    /// Declares a sprite's colour and the halo it casts.
    ///
    /// One method and not two: the halo rides `Op::Paint`, because the compositor derives it
    /// from the brush already bound, so a sprite has no halo to declare before it has a
    /// paint and re-declaring either restates both.
    pub(crate) fn paint(&mut self, id: SpriteId, paint: Paint, halo: Option<Halo>) {
        self.pending.push(Op::Paint { id, paint, halo });
    }

    pub(crate) fn bind(&mut self, id: NodeId, prop: Prop, bind: Bind) {
        let bit = 1 << prop as u32;
        self.tree.c.channels[id.index()] |= bit;
        // A tracked or followed channel is driven for as long as the binding stands, and
        // writing the composite that contains it disconnects it: setting `Offset` replaces
        // whatever animates `Offset.Y`. Recorded so the encode leaves that channel alone.
        match bind {
            Bind::Track { .. } | Bind::FollowOffset { .. } => {
                self.tree.c.driven[id.index()] |= bit;
            }
            Bind::Stop => self.tree.c.driven[id.index()] &= !bit,
            Bind::Set(_) | Bind::Animate(_) => {}
        }
        self.pending.push(Op::Bind { id, prop, bind });
    }

    /// Declares a stroke, minting a dash pattern where one is given.
    pub(crate) fn stroke(
        &mut self,
        width: f32,
        cap: Cap,
        join: Join,
        dashes: &[f32],
    ) -> StrokeStyle {
        let dash = if dashes.is_empty() {
            DashId::NONE
        } else {
            let id: DashId = self.mint_res();
            let runs = self.pending.push_floats(dashes);
            self.pending.push(Op::Res {
                id: id.erased(),
                op: ResOp::Dash { runs },
            });
            id
        };
        StrokeStyle {
            width,
            cap,
            join,
            dash,
        }
    }

    pub(crate) fn frames(&mut self, frames: &[(f32, Value, Easing)]) -> Span {
        self.pending.push_frames(frames)
    }

    /// Mints one resource id. The family rides the type.
    fn mint_res<const F: u8>(&mut self) -> Id<F> {
        let id = self.res_ids.mint();
        Id::raw(id.index() as u32, id.generation())
    }

    pub(crate) fn geometry(&mut self, verbs: &[PathVerb]) -> GeomId {
        let id = self.mint_res();
        self.set_geometry(id, verbs);
        id
    }

    pub(crate) fn set_geometry(&mut self, id: GeomId, verbs: &[PathVerb]) {
        let verbs = self.pending.push_verbs(verbs);
        self.pending.push(Op::Res {
            id: id.erased(),
            op: ResOp::Geom { verbs },
        });
    }

    pub(crate) fn ramp(&mut self, stops: &[super::Stop], spread: Spread) -> RampId {
        let id = self.mint_res();
        self.set_ramp(id, stops, spread);
        id
    }

    /// Resolves a ramp's stops through the root scope and keeps the authored roles, so a
    /// theme change re-resolves the same ramp rather than leaving it at the old palette.
    pub(crate) fn set_ramp(&mut self, id: RampId, stops: &[super::Stop], spread: Spread) {
        let scope = self.root_scope();
        self.scratch_stops.clear();
        self.scratch_stops.extend(stops.iter().map(|stop| {
            let light = crate::role::resolve(stop.role, scope);
            (
                windows_scene::quant_stop(stop.at),
                light.with_alpha(light.a * stop.strength),
            )
        }));
        let span = self.pending.push_stops(&self.scratch_stops);
        self.pending.push(Op::Res {
            id: id.erased(),
            op: ResOp::Ramp {
                stops: span,
                spread,
            },
        });
        self.ramps.place(id, (stops.to_vec(), spread));
    }

    /// Re-resolves every live ramp against the current root scope.
    #[cold]
    pub(crate) fn relight_ramps(&mut self) {
        let ids: Vec<RampId> = self.ramps.iter().map(|(id, _)| id).collect();
        for id in ids {
            let Some((stops, spread)) = self.ramps.get(id) else {
                continue;
            };
            let (stops, spread) = (stops.clone(), *spread);
            self.set_ramp(id, &stops, spread);
        }
    }

    pub(crate) fn run(&mut self, segs: Span, ink: Ink) -> RunId {
        let id = self.mint_res();
        self.set_run(id, segs, ink);
        id
    }

    pub(crate) fn set_run(&mut self, id: RunId, segs: Span, ink: Ink) {
        self.pending.push(Op::Res {
            id: id.erased(),
            op: ResOp::Run { segs, ink },
        });
    }

    pub(crate) fn region(&mut self) -> RegionId {
        let id: RegionId = self.mint_res();
        self.pending.push(Op::Res {
            id: id.erased(),
            op: ResOp::Region,
        });
        id
    }

    pub(crate) fn release<const F: u8>(&mut self, id: Id<F>) {
        self.res_ids
            .release(Id::raw(id.index() as u32, id.generation()));
        self.pending.push(Op::Res {
            id: id.erased(),
            op: ResOp::Drop,
        });
    }

    /// Starts a timed reveal, reported back as `SceneEvent::DelayElapsed`. The wait is a
    /// compositor animation inside a scoped batch, so no thread holds a clock for it.
    pub(crate) fn delay(&mut self, ms: u32) -> DelayId {
        let id = self.delay_ids.mint();
        self.pending.push(Op::Delay { id, ms: Some(ms) });
        id
    }

    pub(crate) fn cancel_delay(&mut self, id: DelayId) {
        self.delay_ids.release(id);
        self.pending.push(Op::Delay { id, ms: None });
    }

    pub(crate) fn delay_elapsed(&mut self, id: DelayId) {
        self.delay_ids.release(id);
    }

    pub(crate) fn tracker_id<O>(&mut self) -> TrackerId<O> {
        TrackerId::new(self.tracker_ids.mint())
    }

    pub(crate) fn create_tracker<O>(&mut self, id: TrackerId<O>, viewport: GroupId, axes: Axes) {
        self.pending.push(Op::Tracker {
            id: id.erased(),
            op: TrackerOp::Create {
                viewport,
                axes,
                owned: true,
            },
        });
    }

    pub(crate) fn tracker_bounds<O>(&mut self, id: TrackerId<O>, min: Vector2, max: Vector2) {
        self.pending.push(Op::Tracker {
            id: id.erased(),
            op: TrackerOp::Bounds { min, max },
        });
    }

    pub(crate) fn drop_tracker<O>(&mut self, id: TrackerId<O>) {
        let erased = id.erased();
        self.tracker_ids.release(erased.id());
        self.pending.push(Op::Tracker {
            id: erased,
            op: TrackerOp::Drop,
        });
    }

    // ── side rows ───────────────────────────────────────────────────────────────────

    fn side_mut(&mut self, node: NodeId) -> &mut Side {
        let head = self.tree.c.side[node.index()];
        let at = if head == tree::NONE {
            let at = self.sides.place(Side {
                node,
                ..Side::default()
            });
            self.tree.c.side[node.index()] = at;
            at
        } else {
            head
        };
        &mut self.sides[at]
    }

    fn side(&self, node: NodeId) -> Option<&Side> {
        let head = self.tree.c.side[node.index()];
        (head != tree::NONE).then(|| &self.sides[head])
    }

    pub(crate) fn set_escape(&mut self, node: NodeId, f: Rc<dyn Fn()>) {
        self.side_mut(node).escape = Some(f);
    }

    /// Takes a callable copy so invoking application code never borrows this host.
    pub(crate) fn escape_handler(&self) -> Option<Rc<dyn Fn()>> {
        self.overlays
            .iter()
            .rev()
            .find_map(|p| self.side(p.root)?.escape.clone())
    }

    pub(crate) fn set_probe(&mut self, node: NodeId, probe: Probe) {
        self.side_mut(node).probe = Some(probe);
    }

    pub(crate) fn set_anchor_origin(&mut self, node: NodeId, set: Anchors) {
        self.side_mut(node).origin = Some(set);
    }

    pub(crate) fn attach_anchor(&mut self, node: NodeId, set: Anchors, key: u64) {
        self.anchors.push(Attachment { set, key, node });
    }

    pub(crate) fn set_relative_pivot(&mut self, node: NodeId, fraction: Vector2) {
        self.side_mut(node).pivot = Some(fraction);
        self.tree.mark(node);
    }

    pub(crate) fn set_geometry_job(&mut self, node: NodeId, at: u32) {
        self.side_mut(node).geometry = at;
    }

    pub(crate) fn set_scroll_row(&mut self, node: NodeId, at: u32) {
        self.side_mut(node).scroll = at;
    }

    pub(crate) fn set_region_row(&mut self, node: NodeId, at: u32) {
        self.side_mut(node).region = at;
    }

    /// The region slot this node paints, or [`RegionId::NONE`].
    pub(crate) fn region_sink(&self, node: NodeId) -> RegionId {
        let at = self.side(node).map_or(tree::NONE, |side| side.region);
        self.regions.get(at).map_or(RegionId::NONE, |row| row.sink)
    }

    /// The client extent an overlay is laid out inside, from the insets its spec named.
    ///
    /// Resolved here rather than carried as lengths, because the row the solve reads holds the
    /// number: the insets are stated against the window and the window is not a containing
    /// block the solve has yet walked.
    pub(crate) fn overlay_viewport(&self, insets: Option<[Len; 4]>) -> Rect {
        let window = self.window.get();
        let Some([left, top, right, bottom]) = insets else {
            return Rect {
                x0: 0.0,
                y0: 0.0,
                x1: window.x,
                y1: window.y,
            };
        };
        let (class, scope) = (self.tree.class(self.root), self.root_scope());
        let dip = |len: Len, basis: f32| {
            len.resolve(&self.metrics, class, scope, basis, self.env.scale())
                .unwrap_or(0.0)
        };
        let (x0, y0) = (dip(left, window.x), dip(top, window.y));
        Rect {
            x0,
            y0,
            x1: x0.max(window.x - dip(right, window.x)),
            y1: y0.max(window.y - dip(bottom, window.y)),
        }
    }

    pub(crate) fn set_surface_row(&mut self, node: NodeId, at: u32) {
        self.side_mut(node).surface = at;
    }

    pub(crate) fn surface_row(&self, node: NodeId) -> u32 {
        self.side(node).map_or(tree::NONE, |side| side.surface)
    }

    /// What the solve wrote for one node.
    pub(crate) fn geom(&self, node: NodeId) -> Geom {
        self.tree.c.geom[node.index()]
    }

    /// The control this node declared, or [`ControlId::NONE`].
    pub(crate) fn control_of(&self, node: NodeId) -> ControlId {
        self.tree.c.control[node.index()]
    }

    // ── controls ────────────────────────────────────────────────────────────────────

    /// Mints a control row and marks the accessible tree stale.
    ///
    /// The generation half of the id makes an intent queued before an unmount a miss rather
    /// than a call into whatever now occupies the slot.
    pub(crate) fn mint_control(&mut self, row: ControlRow) -> ControlId {
        let id = self.control_ids.mint();
        self.chrome_touched.push(id);
        self.controls.place(id, row);
        self.uia_stale.set(true);
        id
    }

    pub(crate) fn control(&self, id: ControlId) -> Option<&ControlRow> {
        self.controls.get(id)
    }

    pub(crate) fn control_mut(&mut self, id: ControlId) -> Option<&mut ControlRow> {
        // The one mutable way in, so it is where a row is recorded as possibly changed. A run
        // of setters on one control costs one entry.
        if self.chrome_touched.last() != Some(&id) {
            self.chrome_touched.push(id);
        }
        self.controls.get_mut(id)
    }

    /// Releases a control, dropping the handlers it captured outside this borrow.
    ///
    /// Queues the id for the next fill, which is what bounds the front table; a stale report
    /// there is already a miss through the generational id.
    pub(crate) fn release_control(&mut self, id: ControlId) {
        let Some(row) = self.controls.take(id) else {
            return;
        };
        self.control_ids.release(id);
        self.handlers.vacate(row.handlers, &mut self.retired);
        self.fields.take(id);
        for slot in &mut self.caption {
            if *slot == Some(id) {
                *slot = None;
            }
        }
        self.released.push(id);
        self.uia_stale.set(true);
    }

    /// Mints the control id a blocker entry is named by. Nothing to light and nothing to
    /// move: a blocker is a rect in the hit array.
    pub(crate) fn mint_blocker(&mut self) -> ControlId {
        let scope = self.root_scope();
        self.mint_control(ControlRow::blank(NodeId::NONE, scope))
    }

    /// Puts a control into a model state, re-painting exactly the parts that state changes.
    ///
    /// Recorded whether or not there is anything to repaint: a control with no chrome row
    /// still has an automation peer that reports checked, selected or unavailable.
    pub(crate) fn set_state(&mut self, id: ControlId, state: ModelState, on: bool) {
        let Some(row) = self.control_mut(id) else {
            return;
        };
        let held = match state {
            ModelState::Selected => &mut row.selected,
            ModelState::Disabled => &mut row.disabled,
            ModelState::Rest => return,
        };
        if *held == on {
            return;
        }
        *held = on;
        let next = if row.disabled {
            ModelState::Disabled
        } else if row.selected {
            ModelState::Selected
        } else {
            ModelState::Rest
        };
        let repaint = row.state != next;
        row.state = next;
        if state == ModelState::Selected
            && row.tab_stop.is_none()
            && matches!(
                row.uia,
                crate::widget::UiaRole::RadioButton | crate::widget::UiaRole::TabItem
            )
        {
            self.focus_ops
                .push(crate::seam::FocusOp::TabIndex(id, if on { 0 } else { -1 }));
        }
        self.uia_stale.set(true);
        if repaint {
            self.repaint_control(id);
        }
    }

    /// Runs `edit` on the automation peer of the region `id` names, adding a row on first use.
    ///
    /// One row per region and a handful of regions per screen, so the lookup is a scan.
    pub(crate) fn region_peer(
        &mut self,
        id: ControlId,
        edit: impl FnOnce(&mut crate::uia::RegionPeer),
    ) {
        let at = self
            .peers
            .iter()
            .position(|peer| peer.id == id)
            .unwrap_or_else(|| {
                self.peers.push(crate::uia::RegionPeer {
                    updates: None,
                    id,
                    geometry: std::sync::Arc::new(windows_present::RegionParts::new()),
                    parts: Vec::new(),
                    values: None,
                    value: None,
                });
                self.peers.len() - 1
            });
        edit(&mut self.peers[at]);
    }

    pub(crate) fn publish_fraction(
        &mut self,
        id: ControlId,
        fraction: f32,
        number: f64,
        epoch: u64,
    ) {
        let Some(row) = self.control_mut(id) else {
            return;
        };
        row.number = Some(number);
        let value = row.value.get_or_insert_with(ValueRow::default);
        value.fraction = fraction;
        value.revision = epoch;
        let (value, live) = (*value, row.live);
        self.values.push((id, value));
        // A cell whose owner has been disposed is skipped: `Cell::set` panics on a disposed
        // handle, and a control's two halves die at different moments.
        if let Some(cell) = live.filter(|cell| cell.alive()) {
            cell.set(Some(number));
        }
    }

    /// Calls the handler each intent names.
    ///
    /// An intent queued before its control unmounted is skipped: the generation half of the
    /// id does not match the slot's. The handler is cloned out before it runs, since running
    /// it is application code and must not hold the borrow.
    pub fn dispatch(intents: &[Intent]) {
        for intent in intents {
            let Some(call) = Self::with(|h| h.handler_for(intent)) else {
                continue;
            };
            call();
            if matches!(intent.what, What::Selected(_)) {
                crate::signal::flush();
            }
        }
    }

    /// Finds the adjacent enabled radio or tab in its owning selection group.
    pub(crate) fn choice_neighbor(&self, target: ControlId, key: u16) -> Option<ControlId> {
        use crate::widget::UiaRole;
        if !matches!(key, 0x23..=0x28) {
            return None;
        }
        let row = self.control(target)?;
        if !matches!(row.uia, UiaRole::RadioButton | UiaRole::TabItem) || row.disabled {
            return None;
        }
        let mut owner = self.tree.parent(row.node);
        while !owner.is_none() {
            if self
                .control(self.tree.c.control[owner.index()])
                .is_some_and(|row| {
                    row.selection.is_some() || matches!(row.uia, UiaRole::List | UiaRole::Tab)
                })
            {
                break;
            }
            owner = self.tree.parent(owner);
        }
        if owner.is_none() {
            return None;
        }
        let (mut first, mut last, mut before, mut after) = (None, None, None, None);
        let mut found = false;
        self.visit_choices(owner, row.uia, &mut |id| {
            first = first.or(Some(id));
            if id == target {
                before = last;
                found = true;
            } else if found {
                after = after.or(Some(id));
            }
            last = Some(id);
        });
        match key {
            0x25 | 0x26 => before.or(last),
            0x27 | 0x28 => after.or(first),
            0x24 => first,
            0x23 => last,
            _ => None,
        }
    }

    fn visit_choices(
        &self,
        node: NodeId,
        role: crate::widget::UiaRole,
        visit: &mut impl FnMut(ControlId),
    ) {
        for child in self.tree.children(node) {
            if self.tree.c.flags[child.index()] & (tree::HIDDEN | tree::SUSPENDED | tree::SUNK) != 0
            {
                continue;
            }
            let id = self.tree.c.control[child.index()];
            if let Some(row) = self.control(id) {
                if row.uia == role {
                    if !row.disabled {
                        visit(id);
                    }
                    continue;
                }
                if row.selection.is_some() {
                    continue;
                }
            }
            self.visit_choices(child, role, visit);
        }
    }

    fn handler_for(&mut self, intent: &Intent) -> Option<Box<dyn FnOnce()>> {
        let row = self.control(intent.target)?;
        let handlers = self.handlers.get(row.handlers);
        match intent.what {
            What::Closed => None,
            What::TextReveal {
                revision,
                start,
                end,
            } => {
                self.field_reveal(intent.target, revision, start, end);
                None
            }
            // A hover has no handler: it writes the cell the control observes with, and the
            // graph runs whatever reads it on the next pass.
            What::Hovered(on) => {
                if let Some(cell) = row.hovered.filter(|cell| cell.alive()) {
                    cell.set(on);
                }
                None
            }
            What::Tapped => {
                let handlers = handlers?;
                if let Some(call) = handlers.click.clone() {
                    Some(Box::new(move || call()))
                } else if let Some(call) = handlers.select.clone() {
                    Some(Box::new(move || call(true)))
                } else {
                    let call = handlers.expand.clone()?;
                    let expanded = !row.expanded;
                    Some(Box::new(move || call(expanded)))
                }
            }
            What::Expanded(expanded) => {
                let call = handlers?.expand.clone()?;
                Some(Box::new(move || call(expanded)))
            }
            What::Selected(change) => {
                use crate::uia::action::SelectionChange;
                use crate::widget::UiaRole;
                let selected = change != SelectionChange::Remove;
                if row.disabled || row.selected == selected {
                    return None;
                }
                let mut owner = self.tree.parent(row.node);
                while !owner.is_none() {
                    if let Some(group) = self.control(self.tree.c.control[owner.index()]) {
                        if let Some(required) = group.selection.or_else(|| {
                            matches!(group.uia, UiaRole::ComboBox | UiaRole::List | UiaRole::Tab)
                                .then_some(true)
                        }) {
                            if !selected && required {
                                return None;
                            }
                            if change == SelectionChange::Add {
                                let mut occupied = false;
                                self.visit_choices(owner, row.uia, &mut |id| {
                                    occupied |= self.control(id).is_some_and(|row| row.selected);
                                });
                                if occupied {
                                    return None;
                                }
                            }
                            break;
                        }
                    }
                    owner = self.tree.parent(owner);
                }
                let handlers = handlers?;
                if let Some(call) = handlers.select.clone() {
                    Some(Box::new(move || call(selected)))
                } else if selected {
                    let call = handlers.click.clone()?;
                    Some(Box::new(move || call()))
                } else {
                    None
                }
            }
            What::Scalar { value, commit, .. } => {
                let call = handlers?.scalar.clone()?;
                let gesturing = if commit {
                    Gesturing::Committed(value)
                } else {
                    Gesturing::Moved(value)
                };
                Some(Box::new(move || call(gesturing)))
            }
            What::Canceled(_) => {
                let call = handlers?.scalar.clone()?;
                Some(Box::new(move || call(Gesturing::Canceled)))
            }
            What::Dragged(update) => {
                let call = handlers?.drag.clone()?;
                Some(Box::new(move || call(Gesturing::Moved(update))))
            }
            // A canceled decided drag reports the end and nothing else: what stood before the
            // gesture stands.
            What::DragEnded(update) => {
                let call = handlers?.drag.clone()?;
                let gesturing = update.map_or(Gesturing::Canceled, Gesturing::Committed);
                Some(Box::new(move || call(gesturing)))
            }
            // A presented region reports the part a gesture finished on. It reaches the same
            // handler a committed value does, carrying the part's index.
            What::Part(part) => {
                let call = handlers?.scalar.clone()?;
                Some(Box::new(move || {
                    call(Gesturing::Committed(f64::from(part.0)))
                }))
            }
        }
    }

    // ── unmount ─────────────────────────────────────────────────────────────────────

    /// Retires descendants, including reactive branches, before any callback can run.
    ///
    /// Two-phase: every root is walked first, then destroyed, because destroying clears the
    /// links the walk reads.
    pub(crate) fn retire_tree(&mut self, roots: &[NodeId]) {
        let mut gathered = core::mem::take(&mut self.scratch);
        gathered.clear();
        for &root in roots {
            if self.tree.is_live(root) {
                self.tree.gather(root, &mut gathered);
            }
        }
        for at in 0..gathered.len() {
            self.retire_node(gathered[at]);
        }
        for at in 0..gathered.len() {
            self.tree.release(gathered[at]);
        }
        self.entrances.retain(|entry| self.tree.is_live(entry.node));
        gathered.clear();
        self.scratch = gathered;
    }

    fn retire_node(&mut self, node: NodeId) {
        let at = node.index();
        self.binding_release(self.tree.c.bindings[at]);
        self.tree.c.bindings[at] = tree::NONE;
        let control = self.tree.c.control[at];
        if !control.is_none() {
            self.release_control(control);
        }
        let key = self.tree.c.text[at];
        if key != super::text::MeasureKey::NONE {
            self.release_text(key);
            self.tree.c.text[at] = super::text::MeasureKey::NONE;
        }
        let mut paint = self.tree.c.paints[at];
        while !paint.is_none() {
            paint = self.appearances.release(paint, &mut self.pending);
        }
        self.tree.c.paints[at] = NodeId::NONE;
        let head = self.tree.c.side[at];
        if head != tree::NONE {
            self.retire_side(head);
            self.tree.c.side[at] = tree::NONE;
        }
    }

    fn retire_side(&mut self, at: u32) {
        let Some(side) = self.sides.free(at) else {
            return;
        };
        if side.rounded != tree::NONE {
            self.rounded.free(side.rounded);
        }
        if let Some(escape) = side.escape {
            self.retired.push(Retired::new(escape));
        }
        // A retired node has no box, and a probe reads a zero box wherever its node has none.
        if let Some(probe) = side.probe.filter(|probe| probe.cell().alive()) {
            probe.cell().set(Placed::default());
        }
        self.geometry.release(side.geometry, &mut self.retired);
        if side.region != tree::NONE {
            // The drop is emitted first and the sink released after: the region owns the
            // surface handle behind the brush this side is painting with, so the unmount that
            // closes it must be asked for before the claim on the sink goes.
            if let Some(row) = self.regions.free(side.region) {
                self.region_ops
                    .push(crate::seam::RegionOp::Drop { sink: row.sink });
                self.release(row.sink);
            }
        }
        if side.scroll != tree::NONE {
            // A tracker is sourced from its viewport's visual, so it is dropped with the row
            // that named it, and so is the rail's control: a mount can disappear before its
            // first solve, so a deferred creation is retired too.
            if let Some(row) = self.scrolls.free(side.scroll) {
                self.scroll_ops.push(crate::seam::ScrollOp::Drop {
                    viewport: row.front.viewport,
                });
                self.drop_tracker(row.front.tracker);
                self.release_control(row.front.grab);
            }
        }
        if side.surface != tree::NONE {
            self.appearances
                .release_surface(side.surface, &mut self.pending);
        }
    }

    // ── the flush ───────────────────────────────────────────────────────────────────

    /// Solves once, publishes what only a solve can decide, and writes the patch.
    ///
    /// One solve: nothing a publisher writes is a layout input, which the debug assertion
    /// below states. The host's borrow is released before geometry effects drain, because a
    /// draw callback is application code.
    pub fn flush(patch: &mut SinkPatch) {
        crate::signal::flush();
        Self::with(|h| {
            h.publish_surfaces();
            h.size_overlay_viewports();
            h.solve();
            // A derived sprite's box is its owner's less the insets, so it is written from the
            // solve before anything reads a sprite's box: the encode below, and the masks.
            h.publish_visuals();
            // Ahead of the publishers: a tracker's source takes its hit region from the
            // viewport's size when it is created, so the solved boxes reach the patch before
            // any op that reads one. The encode after them emits only what they moved.
            h.tree.encode(&mut h.pending);
            h.publish_rounded_clips();
            h.publish_text();
            h.publish_scrolls();
            h.place_overlays();
            h.publish_values();
            h.publish_anchors();
            h.publish_regions();
            h.publish_masks();
            h.publish_fields();
            h.publish_overlay_entries();
            h.publish_probes();
            h.publish_pivots();
            h.schedule_geometry();
            // The predicate walks the flags column, so it is built only where it is asserted.
            #[cfg(debug_assertions)]
            debug_assert!(
                h.tree.unsettled().is_none(),
                "a publisher wrote a layout input on {:?}; a second solve is a bug",
                h.tree.unsettled()
            );
        });
        crate::signal::flush_geometry();
        Self::with(|h| {
            h.publish_channels();
            h.tree.encode(&mut h.pending);
            h.build_hits(None);
            for op in h.pending.ops() {
                if let Op::New { id, .. } = *op {
                    if h.tree.is_live(id) {
                        h.tree.c.flags[id.index()] &= !tree::INITIAL;
                    }
                }
            }
            h.tree.window_resized = false;
            h.pending.env = Some(h.env);
            h.census.flushes += 1;
            core::mem::swap(&mut h.pending, patch);
        });
    }

    /// Solves the window root and each detached overlay root, descending only where a
    /// descendant is dirty.
    fn solve(&mut self) {
        solve::solve_root(self, self.root);
        for at in 0..self.overlays.len() {
            let root = self.overlays[at].root;
            solve::solve_root(self, root);
        }
        self.tree.roots_dirty = false;
    }

    /// Writes each overlay root's authored extent from the window it is inset into. A setter
    /// like any other, so the solve below reads it with everything else.
    fn size_overlay_viewports(&mut self) {
        for at in 0..self.overlays.len() {
            let (root, viewport) = (self.overlays[at].root, self.overlays[at].viewport);
            // Anchored to the window, the root is the box the spec inset, and the side and
            // alignment it seats on become the root's own: a drawer states its width as a
            // share of that box and sits against the edge it was asked for. Anchored to a
            // control or a point, the root is its content, bounded by that box, and the
            // anchor seats the whole root.
            let anchor = self.overlays[at].anchor;
            let fills = matches!(anchor.to, crate::overlay::AnchorTo::Window);
            let (w, h) = (viewport.width(), viewport.height());
            self.tree.author(root, |l| {
                if fills {
                    l.width = Len::dip(w);
                    l.height = Len::dip(h);
                    (l.align, l.justify) = seat(anchor.side, anchor.align);
                }
                l.max_width = Len::dip(w);
                l.max_height = Len::dip(h);
            });
        }
    }

    /// Schedules the drawing jobs whose source box moved, without holding two borrows.
    fn schedule_geometry(&mut self) {
        let mut jobs = core::mem::take(&mut self.geometry);
        jobs.schedule(&self.tree);
        self.geometry = jobs;
    }

    /// Gives every derived sprite its own box and touches it for the encode.
    fn publish_visuals(&mut self) {
        for at in 0..self.sides.slots() {
            let Some(side) = self.sides.get(at) else {
                continue;
            };
            let (node, visual) = (side.node, side.visual);
            let hidden = self.tree.c.flags[node.index()] & (tree::HIDDEN | tree::SUNK) != 0;
            let geom = match visual {
                Visual::Unplaced => continue,
                // Hidden is no box, by its own bit or an ancestor's: the walks never see a
                // derived sprite, so this is where it stops taking pixels.
                _ if hidden => Geom::default(),
                Visual::Rect(local, size) => Geom {
                    local,
                    size,
                    ..Geom::default()
                },
                Visual::Insets([l, t, r, b]) => {
                    let owner = self.tree.parent(node);
                    let box_ = self.tree.c.geom[owner.index()].size;
                    // An owner narrower than its insets leaves no box, not a negative one.
                    Geom {
                        local: Vector2 { x: l, y: t },
                        size: Vector2 {
                            x: (box_.x - l - r).max(0.0),
                            y: (box_.y - t - b).max(0.0),
                        },
                        ..Geom::default()
                    }
                }
            };
            self.tree.c.geom[node.index()] = geom;
            self.tree.touch(node);
        }
    }

    /// The rect an overlay's anchor names, in absolute DIPs.
    fn anchor_rect(&self, to: crate::overlay::AnchorTo, viewport: Rect) -> Option<Rect> {
        match to {
            crate::overlay::AnchorTo::Control(id) => {
                let node = self.control(id)?.node;
                self.tree
                    .is_live(node)
                    .then(|| self.tree.c.geom[node.index()].rect)
            }
            crate::overlay::AnchorTo::Point(at) => Some(Rect {
                x0: at.x,
                y0: at.y,
                x1: at.x,
                y1: at.y,
            }),
            crate::overlay::AnchorTo::Window => Some(viewport),
        }
    }

    /// Resolves every open overlay's offset against the solve that just ran.
    ///
    /// An overlay moves when it opens, when its anchor moves and when the window resizes
    /// under it, so this is not a per-frame cost. Placement translates and never constrains,
    /// so a moved root re-publishes its subtree's rects without re-measuring anything.
    fn place_overlays(&mut self) {
        let window = self.window.get();
        for at in 0..self.overlays.len() {
            let held = self.overlays[at];
            let size = self.tree.c.geom[held.root.index()].size;
            // Declared but not yet measured. Placing a zero box would seat it at the anchor's
            // corner and then move it a pass later, which reads as a flash.
            if size.x == 0.0 || size.y == 0.0 {
                continue;
            }
            // An anchor that has unmounted leaves the overlay exactly where it is. Whether it
            // stays open is the overlay layer's decision, and moving it to the origin first
            // would pre-empt that.
            let Some(against) = self.anchor_rect(held.anchor.to, held.viewport) else {
                continue;
            };
            let to = crate::overlay::place(size, against, held.anchor, window);
            if to != held.at {
                self.overlays[at].at = to;
                solve::shift(self, held.root, to);
            }
        }
    }

    /// Publishes the solved box of every probed node whose box moved.
    ///
    /// Writing a cell marks the graph and raises a frame request; it runs no effect and no
    /// memo, so it cannot re-enter this borrow. `set` gates on equality, which is what keeps
    /// a probe off the per-frame path. A cell whose owner has been disposed is skipped,
    /// because `Cell::set` panics on a disposed handle and the two halves of a probe die at
    /// different moments.
    fn publish_probes(&mut self) {
        for at in 0..self.sides.slots() {
            let Some(side) = self.sides.get(at) else {
                continue;
            };
            let (node, probe) = (side.node, side.probe);
            let Some(probe) = probe.filter(|probe| probe.cell().alive()) else {
                continue;
            };
            let geom = self.tree.c.geom[node.index()];
            probe.cell().set(Placed {
                rect: geom.rect,
                size: geom.size,
                local: geom.local,
                scope: Some(self.scope_of(node).at_width(self.tree.class(node))),
            });
        }
    }

    /// Publishes each anchor set's keyed boxes, in its origin container's own space.
    ///
    /// Unmounted attachments leave here rather than through a removal hook: an id carries a
    /// generation, so the liveness check this pass already needs is also what bounds the list.
    fn publish_anchors(&mut self) {
        let tree = &self.tree;
        self.anchors.retain(|a| tree.is_live(a.node));
        for at in 0..self.sides.slots() {
            let Some(side) = self.sides.get(at) else {
                continue;
            };
            let Some(set) = side.origin else { continue };
            let origin = self.tree.c.geom[side.node.index()];
            // At the class the origin was solved in, so a reader converting a box to metric
            // units divides by the number the solve multiplied by.
            let scope = self
                .scope_of(side.node)
                .at_width(self.tree.class(side.node));
            let (tree, anchors) = (&self.tree, &self.anchors);
            let boxes = || {
                anchors
                    .iter()
                    .filter(|a| a.set == set)
                    .map(|a| (a.key, tree.c.geom[a.node.index()].rect.rebased(origin.rect)))
            };
            // An update wakes every reader, so a solve that moved nothing writes nothing.
            let held = crate::signal::untracked(|| {
                set.cell().with(|table| {
                    table.size == origin.size
                        && table.published() == Some(scope)
                        && table.iter().map(|a| (a.key, a.rect)).eq(boxes())
                })
            });
            if held {
                continue;
            }
            set.cell().update(|table| {
                table.clear();
                for (key, rect) in boxes() {
                    table.push(key, rect);
                }
                table.set_origin(origin.size, scope);
            });
        }
    }

    /// Emits a mount for every region that has a box and no buffers, and a resize for every
    /// one whose box moved.
    fn publish_regions(&mut self) {
        let mut out = core::mem::take(&mut self.region_ops);
        crate::present::emit(self, &mut out);
        self.region_ops = out;
    }

    /// Publishes each value control's travel from the boxes the solve gave it.
    ///
    /// A part the router drives is corrected by shipping it the new room, never by writing
    /// its property: writing it would snap the part back to where the application last wrote
    /// it, mid-gesture.
    fn publish_values(&mut self) {
        let Self {
            controls,
            tree,
            values,
            ..
        } = self;
        for (id, row) in controls.iter_mut() {
            let (node, flags) = (row.node, row.front.flags);
            let Some(value) = row.value.as_mut() else {
                continue;
            };
            // The axis the value runs along is the control's own, not the part's: every part
            // of one control reads the same travel.
            let vertical = flags & crate::widget::flag::VERTICAL != 0;
            let along = |v: Vector2| if vertical { v.y } else { v.x };
            // The thumb's solved offset is the inset it rests at, mirrored at the far end, and
            // what is left of the control once the thumb itself is taken out is its travel.
            let thumb = value.parts.iter().find_map(|&(part, kind)| {
                matches!(kind, crate::widget::ScalarPart::Thumb { .. })
                    .then(|| tree.c.geom[part.index()])
            });
            let (rest, own) = thumb.map_or((0.0, 0.0), |g| (along(g.local), along(g.size)));
            let travel = (along(tree.c.geom[node.index()].size) - rest * 2.0 - own).max(0.0);
            if (rest, travel) == (value.rest, value.travel) {
                continue;
            }
            (value.rest, value.travel) = (rest, travel);
            values.push((id, *value));
        }
    }

    pub(crate) fn enter_slide(&mut self, node: NodeId, slide: crate::overlay::Slide) {
        self.suspend_input(node, true);
        self.entrances.push(Entrance::new(node, slide));
    }

    pub(crate) fn enter_from(&mut self, node: NodeId, slide: crate::overlay::Slide, delay_ms: u32) {
        assert!(!self.entrances.iter().any(|entry| entry.node == node), "one entrance per node");
        let channels = &mut self.tree.c.channels[node.index()];
        let owned = (1 << Prop::Opacity as u32) | (1 << Prop::AnchorX as u32) | (1 << Prop::AnchorY as u32);
        assert!(*channels & owned == 0, "an entrance requires unclaimed opacity and anchor channels");
        *channels |= owned;
        self.suspend_input(node, true);
        self.entrances.push(Entrance { dip_fade: true, delay_ms, ..Entrance::new(node, slide) });
    }

    /// Publishes conditional and overlay entrances through the same geometry policy.
    fn publish_overlay_entries(&mut self) {
        for at in 0..self.overlays.len() {
            if let Some(entry) = self.overlays[at].entry {
                self.overlays[at].entry = Some(self.publish_entry(entry));
            }
        }
        for at in 0..self.entrances.len() {
            self.entrances[at] = self.publish_entry(self.entrances[at]);
        }
        self.entrances.retain(|entry| !entry.done);
    }

    fn publish_entry(&mut self, mut entry: Entrance) -> Entrance {
        if entry.done {
            return entry;
        }
        let geom = self.tree.c.geom[entry.node.index()];
        if geom.size.x <= 0.0 || geom.size.y <= 0.0 {
            return entry;
        }
        if !entry.started {
            let duration_ms = entry.slide.ms + entry.delay_ms;
            let start = if entry.delay_ms == 0 { 0.0 } else { entry.delay_ms as f32 / duration_ms as f32 };
            let by = if entry.dip_fade {
                Vector2::new(entry.slide.by.x / geom.size.x, entry.slide.by.y / geom.size.y)
            } else {
                entry.slide.by
            };
            for (prop, by) in [(Prop::AnchorX, by.x), (Prop::AnchorY, by.y)] {
                if by == 0.0 {
                    continue;
                }
                let keys = [
                    (0.0, Value::Scalar(-by), Easing::Linear),
                    (start, Value::Scalar(-by), Easing::Linear),
                    (1.0, Value::Scalar(0.0), entry.slide.easing),
                ];
                let frames = self.frames(&keys[usize::from(entry.delay_ms == 0)..]);
                self.bind(entry.node, prop, Bind::Animate(Anim::Frames {
                    frames,
                    duration_ms,
                    iterations: Iterations::Count(1),
                }));
            }
            if entry.dip_fade {
                let keys = [
                    (0.0, Value::Scalar(0.0), Easing::Linear),
                    (start, Value::Scalar(0.0), Easing::Linear),
                    (1.0, Value::Scalar(1.0), entry.slide.easing),
                ];
                let frames = self.frames(&keys[usize::from(entry.delay_ms == 0)..]);
                self.bind(entry.node, Prop::Opacity, Bind::Animate(Anim::Frames {
                    frames,
                    duration_ms,
                    iterations: Iterations::Count(1),
                }));
            }
            entry.started = true;
            if entry.slide.by == Vector2::zero() {
                entry.done = true;
                self.suspend_input(entry.node, false);
            }
        }
        entry
    }

    /// Ends the entrance the compositor has just finished playing, releasing its hold on input.
    ///
    /// Answers for the node the report names and only while its slide is on the compositor, so
    /// a completion for a channel this row never animated moves nothing.
    pub(crate) fn complete_overlay_entry(&mut self, node: NodeId) {
        if self.entrances.iter().any(|entry| entry.node == node && entry.started) {
            self.entrances.retain(|entry| entry.node != node);
            self.suspend_input(node, false);
        }
        for at in 0..self.overlays.len() {
            let Some(entry) = self.overlays[at].entry else {
                continue;
            };
            if entry.node != node || !entry.started || entry.done {
                continue;
            }
            self.overlays[at].entry = Some(Entrance {
                done: true,
                ..entry
            });
            self.suspend_input(node, false);
        }
    }

    /// Writes the centre of every node that stated one as a fraction of its own box.
    fn publish_pivots(&mut self) {
        for at in 0..self.sides.slots() {
            let Some(side) = self.sides.get(at) else {
                continue;
            };
            let (node, pivot) = (side.node, side.pivot);
            let Some(pivot) = pivot else { continue };
            let size = self.tree.c.geom[node.index()].size;
            let centre = Vector2 {
                x: size.x * pivot.x,
                y: size.y * pivot.y,
            };
            // Bitwise, so the unsent `NaN` never compares equal and a real centre always does.
            let sent = side.centre;
            if (centre.x.to_bits(), centre.y.to_bits()) == (sent.x.to_bits(), sent.y.to_bits()) {
                continue;
            }
            if let Some(side) = self.sides.get_mut(at) {
                side.centre = centre;
            }
            self.bind(node, Prop::Center, Bind::Set(Value::Vec2(centre)));
        }
    }

    // ── the hit array and the accessible tree ───────────────────────────────────────

    /// Rebuilds the array in paint order, straight into the patch's own buffer, and the
    /// accessible tree beside it where `uia` is given.
    ///
    /// One walk for both, so the entries and the array describe the same layout by
    /// construction rather than by an ordering rule. Slot roots append after the window
    /// subtree, in the order they opened, each light-dismissing overlay preceded by its
    /// full-window blocker: the array is the z-order and the scan takes the first hit from
    /// the back.
    pub(crate) fn build_hits(&mut self, uia: Option<&mut crate::uia::Snapshot>) {
        if !self.tree.hits_dirty && uia.is_none() {
            return;
        }
        if self.tree.hits_dirty {
            self.uia_stale.set(true);
        }
        let root = self.root;
        let window = self.tree.c.geom[root.index()].size;
        let Self {
            tree,
            controls,
            text,
            handlers,
            fields,
            hits,
            pending,
            overlays,
            scratch_text,
            ..
        } = self;
        let walk = hits::Walk {
            tree,
            controls,
            text,
            handlers,
            fields,
            overlays,
        };
        let mut out = hits::Out {
            hits,
            patch: pending,
            uia,
            scratch: scratch_text,
        };
        hits::begin(&mut out);
        hits::walk(&walk, &mut out, root, 0);
        for placement in overlays.iter() {
            if let Some(id) = placement.blocker {
                hits::blocker(&mut out, id, (window.x, window.y));
            }
            hits::walk(&walk, &mut out, placement.root, 0);
        }
        // Sorted on the way out, so `HitTable::replace` is two copies and never a sort.
        out.patch.index_mut().sort_unstable_by_key(|&(id, _)| id);
        let patch = out.patch;
        let (entries, index) = (patch.hits_span(), patch.index_span());
        patch.push(Op::Hits { entries, index });
        self.tree.hits_dirty = false;
    }

    /// Fills `out` with the automation tree, in one preorder walk over the arena.
    ///
    /// A node whose control carries no role is skipped and its children reparent past it.
    /// Strings are interned here, on the thread that owns the text table, so what crosses is
    /// plain `Send` data.
    pub fn uia_entries(&mut self, out: &mut crate::uia::Snapshot) {
        self.build_hits(Some(out));
        // The extents the solve settled on, taken here rather than in the walk: the walk sees
        // the viewport, and how far its content reaches is the scroll table's to say.
        for view in &mut out.scrolls {
            let Some(row) = (0..self.scrolls.slots())
                .filter_map(|at| self.scrolls.get(at))
                .find(|row| row.node == view.node)
            else {
                continue;
            };
            view.view = self.tree.c.geom[view.node.index()].size;
            view.content = self.tree.c.geom[row.content.index()].size;
        }
        crate::uia::derive_keys(out, &mut self.uia_seen);
    }

    // ── the fill ────────────────────────────────────────────────────────────────────

    /// Moves everything this side produced since the last fill into `down`.
    ///
    /// Appends rather than moves the buffers, so both sides keep their capacity and a fill on
    /// a `Down` the consumer has not drained adds to that batch rather than replacing it. The
    /// caption registry crosses only when it differs from what the last fill sent: the three
    /// ids change when a title bar mounts and at no other time.
    pub(crate) fn fill(&mut self, down: &mut crate::seam::Down) {
        self.chrome_touched.sort_unstable();
        self.chrome_touched.dedup();
        for id in self.chrome_touched.drain(..) {
            if let Some(row) = self.controls.get(id) {
                down.chrome.push((id, row.front));
            }
        }
        down.values.append(&mut self.values);
        down.regions.append(&mut self.region_ops);
        down.scrolls.append(&mut self.scroll_ops);
        down.declared.gestures.append(&mut self.gestures);
        down.declared.focus.append(&mut self.focus_ops);
        down.fields.sources.append(&mut self.field_sources);
        down.fields.layouts.append(&mut self.field_layouts);
        down.fields.commits.append(&mut self.field_commits);
        down.declared.released.append(&mut self.released);
        down.declared.peers.append(&mut self.peers);
        if self.caption != self.sent_caption {
            self.sent_caption = self.caption;
            down.declared.caption = self.caption;
        }
        down.declared.census = core::mem::take(&mut self.census);
        // A theme change is a transaction of its own: the scene takes the backdrop in the batch
        // that repaints against the new scope, so it crosses with that batch or not at all.
        if let Some(theme) = super::theme::take_theme() {
            down.theme = Some(theme);
        }
        // A backpressured batch can contain a mount and its later unmount. Publish only live
        // declarations: the scene has already destroyed retired nodes before adoption.
        if !down.declared.released.is_empty() {
            let controls = &self.controls;
            let live = |id: ControlId| controls.get(id).is_some();
            down.chrome.retain(|&(id, _)| live(id));
            down.values.retain(|&(id, _)| live(id));
            down.declared.gestures.retain(|&(id, _)| live(id));
            down.declared
                .focus
                .retain(|op| !matches!(op, crate::seam::FocusOp::TabIndex(id, _) if !live(*id)));
            down.fields.sources.retain(|row| live(row.id));
            down.fields.layouts.retain(|row| live(row.id));
            down.fields.commits.retain(|row| live(row.id));
        }
    }

    /// Returns whether the accessible tree needs rebuilding.
    ///
    /// Set when a control is minted or released, which is when the set of elements changes,
    /// and again when a published name changes. A value, a state, focus or a scroll offset
    /// reaches a client without one.
    pub fn uia_stale(&self) -> bool {
        self.uia_stale.get()
    }

    pub fn uia_published(&self) {
        self.uia_stale.set(false);
    }
}

#[cfg(test)]
#[path = "entry_tests.rs"]
mod entry_tests;

/// Fails to compile if the host ever gains a way to be sent. Only `Host` holds a `SinkPatch`,
/// and `Host` is app-thread affine, which is what stops any layer above from moving a pixel
/// out of the pass.
const _: () = {
    const fn assert_not_send<T: ?Sized>() {}
    assert_not_send::<Host>();
};

// ── what a fixture names a node by ──────────────────────────────────────────────────
//
// Production code reads a column by index and never counts; a test asks what a transaction
// left live to prove that a retire freed it.

#[cfg(test)]
impl Host {
    /// How many nodes are live, the window root included.
    pub(crate) fn live_nodes(&self) -> usize {
        self.tree.ids.live()
    }

    /// How many side rows are placed. A node that declares none of the rare things pays
    /// four bytes for the absence, so this is what a test asks whether one was freed with.
    pub(crate) fn side_rows(&self) -> usize {
        self.sides.len()
    }
}

/// Returns the inline and block alignment a window-anchored root places its content with:
/// the side it seats on, and where along that side it lines up.
fn seat(side: crate::overlay::Side, along: crate::overlay::Align) -> (Align, Align) {
    use crate::overlay::Side;
    let along = match along {
        crate::overlay::Align::Start => Align::Start,
        crate::overlay::Align::Center => Align::Center,
        crate::overlay::Align::End => Align::End,
    };
    match side {
        Side::Left => (Align::Start, along),
        Side::Right => (Align::End, along),
        Side::Top => (along, Align::Start),
        Side::Bottom => (along, Align::End),
        Side::Center => (Align::Center, Align::Center),
    }
}
