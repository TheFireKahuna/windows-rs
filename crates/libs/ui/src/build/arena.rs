//! The build arena: what a widget writes, and where it lives until it is mounted.
//!
//! Arguments evaluate before the call consuming them, so a builder chain is constructed
//! inner-first: children exist before the parent that has to be minted before them.
//! Buffering that here keeps [`El`](super::El) a `Copy` index and allocates nothing after
//! warm-up, where owning children per element or boxing a mount closure costs one
//! allocation per node — and a realized list row is on the fling path.
//!
//! Children are a contiguous chunk, since `.stack(c)` writes the whole list in one push.
//! Everything a modifier appends is an intrusive chain, since modifiers run out of order:
//! `let a = x().grow(); let b = y().grow(); let a = a.width(..)` interleaves two slots, and a
//! span scheme would give `a` one of `b`'s.
//!
//! A `Build::with` body must not call application code, because the arena is borrowed for
//! the length of it. Borrows do not nest on their own, since arguments finish first, and a
//! keyed list's rows and a branch's arms build at reconcile time, outside any borrow.

// `expect` rather than `allow`: the lint fires again once every item here has a consumer.
#![expect(
    dead_code,
    reason = "consumed by the widget seeds; narrow to the specific items when they land"
)]

use crate::gesture::GestureDecl;
use crate::layout::{Len, Over, Preset, Rule};
use crate::role::{Elevation, Role, Text, TypeRole};
use crate::widget::{Chrome, Flow, Interaction, Motion, StatePolicy, TextSource, UiaRole};
use std::cell::RefCell;
use windows_scene::{Bounds, Exit, GeomId, HitFlags, Prop, Value};

/// The end of a chain, and the absence of a slot.
pub(crate) const NIL: u32 = u32::MAX;

/// A contiguous run of child indices.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Chunk {
    pub at: u32,
    pub len: u32,
}

impl Chunk {
    pub(crate) const EMPTY: Self = Self { at: 0, len: 0 };

    pub(crate) fn range(self) -> core::ops::Range<usize> {
        self.at as usize..(self.at + self.len) as usize
    }
}

/// The head and tail of an intrusive chain, so an append is `O(1)` and order is forward.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Link {
    pub head: u32,
    pub tail: u32,
}

impl Default for Link {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Link {
    pub(crate) const EMPTY: Self = Self {
        head: NIL,
        tail: NIL,
    };

    pub(crate) const fn is_empty(self) -> bool {
        self.head == NIL
    }
}

/// What a widget contributes: one node, as plain data.
///
/// `Copy`, so the mount walk can read a slot without borrowing the arena across the calls
/// it makes. Everything that is not `Copy` — a string, a handler, a flyout body — lives in
/// a side buffer this points into.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Slot {
    pub preset: Preset,
    pub over: Link,
    pub seeds: Link,
    pub chans: Link,
    pub acts: Link,
    pub kids: Chunk,
    pub hit: Option<HitSeed>,
    /// The gesture declaration, in the side buffer, or [`NIL`].
    ///
    /// Out of line because a declaration is ninety-six bytes, a handful of nodes per screen
    /// set one, and the walk copies this slot once per node.
    pub gesture: u32,
    pub state: StatePolicy,
    /// The widget's own surface, as a table row rather than as sprites.
    ///
    /// Held unexpanded so a variant modifier rewrites one byte and the mount decides which
    /// sprites the row implies: a ghost mints no fill because its row has none.
    pub chrome: Option<Chrome>,
    /// What a pointer means here, front-side. `None` is every non-value control.
    pub interaction: Option<Interaction>,
    /// Path geometry, in sprite-local DIPs. The one resource a widget names that this crate
    /// did not mint.
    pub geom: Option<GeomId>,
    /// A surface pushes a rung of the ladder for everything inside it.
    pub elevate: Option<Elevation>,
    /// Classifies its own inline size for its subtree.
    pub responsive: Option<Bounds>,
    /// A scroll container. The tracker is minted at mount, because only then is there a
    /// group for it to be sourced from.
    pub scroll: Option<crate::layout::ScrollDecl>,
    /// The automation-id segment. `&'static str`, so nothing is built at mount.
    pub key: Option<&'static str>,
    pub name: Option<&'static str>,
    /// One of the window's own commands. Recorded at mount so the caption band can resolve a
    /// point through the same array everything else does.
    pub caption: Option<windows_window::CaptionButton>,
    pub uia: UiaRole,
    pub exit: Exit,
    /// A structural adapter owns this node's children instead of `kids`.
    pub adapter: Option<u32>,
    /// Whether this node reaches the mount at all.
    ///
    /// A constant `.when(false)` clears it, and a container skips it when collecting. The
    /// slot is left in the arena rather than removed, because the arena is cleared per mount
    /// and never compacted — so an absent node costs high-water mark and **nothing else**:
    /// no visual, no layout participation, no shaped run, no placeholder.
    pub present: bool,
    /// Whether a child was written straight into [`kids`](Self::kids) by explicit placement.
    ///
    /// Read by [`Build::take_kids`], which asserts on it rather than replacing placed
    /// children in silence.
    pub placed: bool,
    /// Declines touch inflation, whether or not a hit entry exists **yet**.
    ///
    /// Its own field and not a flag on [`hit`](Self::hit), so it does not depend on call
    /// order: `.no_inflate().on_click(..)` and the reverse are equivalent. It is folded
    /// in at mount, and only where a hit entry was declared, so declining an inflation never
    /// *creates* a target.
    pub no_inflate: bool,
    /// Where the solve reports this node's box, or `None` for a node no one reads back.
    ///
    /// A [`Cell`](crate::signal::Cell) and not a node id, so the reporting runs one way: the
    /// application never learns a `NodeId` and never reaches the model.
    pub probe: Option<crate::signal::Cell<crate::layout::Placed>>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            preset: Preset::Bare,
            over: Link::EMPTY,
            seeds: Link::EMPTY,
            chans: Link::EMPTY,
            acts: Link::EMPTY,
            kids: Chunk::EMPTY,
            hit: None,
            gesture: NIL,
            state: StatePolicy::None,
            chrome: None,
            interaction: None,
            geom: None,
            elevate: None,
            responsive: None,
            scroll: None,
            key: None,
            name: None,
            caption: None,
            uia: UiaRole::None,
            exit: Exit::None,
            adapter: None,
            present: true,
            placed: false,
            no_inflate: false,
            probe: None,
        }
    }
}

/// What a node declares to the one hit array.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct HitSeed {
    pub flags: HitFlags,
    /// `None` for the DPI-derived default.
    pub inflate: Option<Len>,
}

/// One sprite, named by **role** and never by colour.
///
/// `resolve` is called inside the mount walk, so neither `Radiance` nor `Paint` is reachable
/// from a widget.
#[derive(Copy, Clone, Debug)]
pub(crate) struct SpriteSeed {
    pub mask: MaskSeed,
    pub role: Role,
    /// Which interaction slot this sprite's colour re-resolves through.
    pub part: Part,
    pub next: u32,
}

/// Which of a control's parts a sprite is, so interaction can re-resolve exactly the ones
/// that change and leave the rest alone.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// Not interaction-sensitive.
    Static,
    Fill,
    Label,
    Border,
    /// The part a value moves: a slider's thumb, a toggle's knob, a meter's level.
    Thumb,
    /// The wash a hover or a press fades in. Minted by the lowering, never by a widget.
    Wash,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum MaskSeed {
    /// A rounded rect. `None` is square.
    Box { radius: Option<Len> },
    /// A rounded rect whose radius is already resolved, for the one case where it is
    /// derived rather than named: a chrome fill sits a hairline inside its own border, so
    /// its radius is the surface's less that hairline and is not a [`Metric`] anybody owns.
    Radius { dips: f32 },
    /// One shaped run, from the text side buffer at this index.
    Run { text: u32 },
    /// The slot's own geometry, filled or outlined.
    Shape { stroke: Option<Len> },
    /// The paint's own alpha is the shape.
    Bare,
}

/// What unit a channel's number is in, and therefore who has to finish it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Unit {
    /// The property's own. Written as read.
    Direct,
    /// `0..=1` of the room a moving part has along its track.
    ///
    /// A fraction cannot be lowered at mount, because the room is a **layout output**: the
    /// track's extent less the part's own. The mount records the fraction and the post-solve
    /// step multiplies it out, which also lets the front thread move the same part from the
    /// same number without asking this thread for geometry.
    Travel,
    /// `0..=1` of a turned part's sweep, which is a constant.
    ///
    /// Not `Direct` even though the sweep needs nothing from layout: this opens the same
    /// value row a slid part does, so one arbiter decides whether this thread or the router
    /// moves it.
    Turn,
}

impl Unit {
    /// Whether a channel in this unit carries a **value** — something a pointer can move —
    /// rather than a property the application simply sets.
    pub(crate) const fn is_value(self) -> bool {
        matches!(self, Self::Travel | Self::Turn)
    }
}

/// A property awaiting the reactive lowering.
pub(crate) struct ChanSeed {
    pub prop: Prop,
    pub motion: Motion,
    pub unit: Unit,
    /// `Option` so mount can **move** a boxed reader into the effect that owns it from
    /// then on, rather than cloning one or borrowing the arena for the effect's life.
    pub source: Option<ChanSource>,
    pub next: u32,
}

/// A channel's value, and whether reading it can ever answer differently.
///
/// A constant carries its value inline and produces no graph node, no `Effect` and no
/// allocation, so static content costs its sprites and nothing else. Decided here rather
/// than once per widget.
pub(crate) enum ChanSource {
    Const(Value),
    Dynamic(Box<dyn Fn() -> Value>),
}

/// A shaped-text declaration.
pub(crate) struct TextSeed {
    pub source: Option<TextSource>,
    pub ramp: TypeRole,
    /// `None` takes the enclosing widget's own chrome row, so a variant that changes the
    /// text colour does not have to reach into the text seed to say so.
    pub ink: Option<Text>,
    pub flow: Flow,
}

/// Something the application asked to happen, held until mount moves it into the host's
/// dense table.
///
/// Handlers never cross the thread seam: they live in an app-thread table and reach the
/// front thread as a presence bit in [`HitFlags`], which is what keeps `SinkPatch: Send`.
pub(crate) enum Act {
    Click(Box<dyn Fn()>),
    ChangeF64(Box<dyn Fn(f64)>),
    CommitF64(Box<dyn Fn(f64)>),
    Tip(TextSource, crate::overlay::Side),
    /// `Rc` from the start, because the row it lands in holds one: the overlay layer has to
    /// take the body *out* of the host's borrow before running it. Boxing here and
    /// converting at mount would allocate twice for the same closure.
    Flyout(std::rc::Rc<dyn Fn() -> super::El>),
    /// Presence that varies, as `Display::None` rather than as an unmount — so a subtree
    /// whose state the user is in the middle of survives the condition flipping.
    HideWhen(Box<dyn Fn() -> bool>),
    /// Overrides that follow a value, written into the lowering's buffer.
    ///
    /// A **style** and not a channel: a size layout has to see must go through the solve,
    /// where binding it as a channel would move the node and leave everything below it where
    /// it was.
    ///
    /// Writing into a buffer rather than returning one override is what lets a column
    /// template be bound — `ClearColumns` and a track each — and what lets a node carry two
    /// of these without them taking turns.
    Restyle(Box<dyn Fn(&mut Vec<Over>)>),
    /// Disabled is model state: it swaps base roles and drops the hit flags.
    DisabledWhen(Box<dyn Fn() -> bool>),
    /// Selection is model state too — a discrete paint swap at event rate, not a wash.
    SelectedWhen(Box<dyn Fn() -> bool>),
}

pub(crate) struct ActSeed {
    pub act: Option<Act>,
    pub next: u32,
}

/// A structural adapter, which owns its node's children and produces them at reconcile
/// time rather than at build time.
///
/// Boxed once per adapter and not per row. The seam where `each`, `when` and `switch` reach
/// `Keyed` and `Branch`. The closure runs **outside** any arena borrow, so a row it builds
/// is free to call application code.
pub(crate) struct Adapter {
    /// `FnOnce`, because it is handed the site once and installs whatever drives it from
    /// then on. Nothing calls back into the walk.
    pub install: Option<Box<dyn FnOnce(super::Site)>>,
}

/// The thread's build arena. Every buffer pooled: cleared after each mount, capacity kept.
#[derive(Default)]
pub(crate) struct Build {
    pub nodes: Vec<Slot>,
    pub kids: Vec<u32>,
    /// Children being collected for the containers under construction.
    ///
    /// A **stack**, because containers nest, and the arena's own, because a temporary per
    /// container would allocate once for `card().stack((..))` and once for every container
    /// inside it, on every mount. A container marks the depth on the way in and moves
    /// everything above that mark into [`kids`](Self::kids) on the way out, so the run it
    /// records is contiguous and the buffer keeps its capacity.
    pub pending: Vec<u32>,
    pub over: Vec<OverSeed>,
    pub seeds: Vec<SpriteSeed>,
    pub chans: Vec<ChanSeed>,
    pub acts: Vec<ActSeed>,
    pub texts: Vec<TextSeed>,
    pub adapters: Vec<Adapter>,
    /// Gesture declarations, out of line. See [`Slot::gesture`].
    pub gestures: Vec<GestureDecl>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct OverSeed {
    pub rule: Rule,
    pub next: u32,
}

thread_local! {
    /// The arena a builder chain writes into.
    static CURRENT: RefCell<Build> = RefCell::new(Build::default());
    /// Arenas a nested mount stood in with, kept so nesting allocates nothing.
    static SPARE: RefCell<Vec<Build>> = const { RefCell::new(Vec::new()) };
}

impl Build {
    /// Runs `f` against the thread's arena.
    ///
    /// **`f` must not call application code.** The arena is borrowed for the length of the
    /// call, so anything that builds from inside `f` panics on the borrow.
    pub(crate) fn with<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        CURRENT.with(|b| f(&mut b.borrow_mut()))
    }

    /// Empties every buffer, keeping the allocations.
    pub(crate) fn clear(&mut self) {
        self.nodes.clear();
        self.kids.clear();
        self.pending.clear();
        self.over.clear();
        self.seeds.clear();
        self.chans.clear();
        self.acts.clear();
        self.texts.clear();
        self.adapters.clear();
        self.gestures.clear();
    }

    /// Edits this node's gesture declaration, minting a default one where there is none.
    ///
    /// Read-modify-write, because `.drag(..)` refines whatever `.gesture(..)` already said
    /// and the two may run in either order.
    pub(crate) fn gesture_mut(&mut self, at: u32, f: impl FnOnce(&mut GestureDecl)) {
        let entry = self.nodes[at as usize].gesture;
        if entry == NIL {
            let entry = self.gestures.len() as u32;
            self.gestures.push(GestureDecl::default());
            self.nodes[at as usize].gesture = entry;
            f(&mut self.gestures[entry as usize]);
            return;
        }
        f(&mut self.gestures[entry as usize]);
    }

    /// Returns the declaration a [`Slot::gesture`] entry names. [`NIL`] answers `None`, since
    /// the buffer is never that long.
    pub(crate) fn gesture(&self, entry: u32) -> Option<GestureDecl> {
        self.gestures.get(entry as usize).copied()
    }

    pub(crate) fn push_slot(&mut self, slot: Slot) -> u32 {
        let at = self.nodes.len() as u32;
        self.nodes.push(slot);
        at
    }

    pub(crate) fn push_over(&mut self, at: u32, rule: Rule) {
        let entry = self.over.len() as u32;
        self.over.push(OverSeed { rule, next: NIL });
        let link = &mut self.nodes[at as usize].over;
        if link.is_empty() {
            *link = Link {
                head: entry,
                tail: entry,
            };
        } else {
            self.over[link.tail as usize].next = entry;
            link.tail = entry;
        }
    }

    pub(crate) fn push_seed(&mut self, at: u32, mut seed: SpriteSeed) {
        seed.next = NIL;
        let entry = self.seeds.len() as u32;
        self.seeds.push(seed);
        let link = &mut self.nodes[at as usize].seeds;
        if link.is_empty() {
            *link = Link {
                head: entry,
                tail: entry,
            };
        } else {
            self.seeds[link.tail as usize].next = entry;
            link.tail = entry;
        }
    }

    pub(crate) fn push_chan(
        &mut self,
        at: u32,
        prop: Prop,
        motion: Motion,
        unit: Unit,
        source: ChanSource,
    ) {
        let entry = self.chans.len() as u32;
        self.chans.push(ChanSeed {
            prop,
            motion,
            unit,
            source: Some(source),
            next: NIL,
        });
        let link = &mut self.nodes[at as usize].chans;
        if link.is_empty() {
            *link = Link {
                head: entry,
                tail: entry,
            };
        } else {
            self.chans[link.tail as usize].next = entry;
            link.tail = entry;
        }
    }

    pub(crate) fn push_act(&mut self, at: u32, act: Act) {
        let entry = self.acts.len() as u32;
        self.acts.push(ActSeed {
            act: Some(act),
            next: NIL,
        });
        let link = &mut self.nodes[at as usize].acts;
        if link.is_empty() {
            *link = Link {
                head: entry,
                tail: entry,
            };
        } else {
            self.acts[link.tail as usize].next = entry;
            link.tail = entry;
        }
    }

    pub(crate) fn push_text(&mut self, seed: TextSeed) -> u32 {
        let at = self.texts.len() as u32;
        self.texts.push(seed);
        at
    }

    pub(crate) fn push_adapter(&mut self, adapter: Adapter) -> u32 {
        let at = self.adapters.len() as u32;
        self.adapters.push(adapter);
        at
    }

    /// Returns where the pending stack stands. A container takes this on the way in.
    pub(crate) fn mark(&self) -> u32 {
        self.pending.len() as u32
    }

    /// Moves everything pushed since `mark` into this node's child list.
    ///
    /// A second call replaces the first, which is what makes `.stack(a).row(b)` mean the
    /// second and not both. The replaced run is left where it was: the arena is cleared per
    /// mount rather than compacted, so an abandoned list costs high-water mark and nothing
    /// else.
    pub(crate) fn take_kids(&mut self, at: u32, mark: u32) {
        // Placement writes straight into the child list, so a class chosen afterwards
        // replaces it, and the only symptom is a cell that is not on screen. Replacing a
        // previous *class*'s run is wanted and stays silent; replacing placed children is a
        // call-order mistake.
        debug_assert!(
            !self.nodes[at as usize].placed,
            "choose this container's layout class before placing children into it"
        );
        let start = self.kids.len() as u32;
        let len = self.pending.len() as u32 - mark;
        self.kids.extend_from_slice(&self.pending[mark as usize..]);
        self.pending.truncate(mark as usize);
        self.nodes[at as usize].kids = Chunk { at: start, len };
    }

    /// Appends one more child to a list already written.
    ///
    /// The explicit-placement escape, and the one case where a node gains a child after its
    /// run was recorded. Copied **within** the buffer rather than through a temporary, so an
    /// eight-cell grid is eight appends and no allocation.
    pub(crate) fn push_kid(&mut self, at: u32, kid: u32) {
        if !self.nodes[kid as usize].present {
            return;
        }
        let existing = self.nodes[at as usize].kids;
        let start = self.kids.len() as u32;
        self.kids.extend_from_within(existing.range());
        self.kids.push(kid);
        let slot = &mut self.nodes[at as usize];
        slot.placed = true;
        slot.kids = Chunk {
            at: start,
            len: existing.len + 1,
        };
    }

    /// Returns how many sprites a slot declared. A count and not a collection: `walk` needs
    /// the number before it needs the seeds, and collecting them to find out would allocate.
    pub(crate) fn seed_count(&self, link: Link) -> usize {
        self.chain_seeds(link).count()
    }

    pub(crate) fn chain_over(&self, link: Link) -> ChainIter<'_, OverSeed> {
        ChainIter {
            buffer: &self.over,
            at: link.head,
            next: |s| s.next,
        }
    }

    pub(crate) fn chain_seeds(&self, link: Link) -> ChainIter<'_, SpriteSeed> {
        ChainIter {
            buffer: &self.seeds,
            at: link.head,
            next: |s| s.next,
        }
    }

    /// Takes the thread's arena, leaving a pooled empty one in its place.
    ///
    /// Mount holds the built arena locally for the whole walk, so anything that *builds*
    /// while it runs — a keyed list reconciling inside an effect the walk installs — writes
    /// into the fresh one rather than corrupting this one. Both come from the same pool, so
    /// nesting costs no allocation in the steady state.
    pub(crate) fn take() -> Self {
        CURRENT.with(|slot| core::mem::replace(&mut *slot.borrow_mut(), Self::spare()))
    }

    /// Hands a taken arena back, cleared, and parks whatever stood in for it.
    pub(crate) fn restore(mut self) {
        self.clear();
        let stand_in = CURRENT.with(|slot| core::mem::replace(&mut *slot.borrow_mut(), self));
        Self::park(stand_in);
    }

    fn spare() -> Self {
        SPARE
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default()
    }

    fn park(mut arena: Self) {
        arena.clear();
        SPARE.with(|pool| pool.borrow_mut().push(arena));
    }
}

/// Walks an intrusive chain forward.
///
/// `Copy`, so a caller can walk the same chain twice — count, then fill — without asking the
/// arena for it again.
#[derive(Copy, Clone)]
pub(crate) struct ChainIter<'a, T> {
    buffer: &'a [T],
    at: u32,
    next: fn(&T) -> u32,
}

impl<'a, T> Iterator for ChainIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if self.at == NIL {
            return None;
        }
        let item = &self.buffer[self.at as usize];
        self.at = (self.next)(item);
        Some(item)
    }
}
