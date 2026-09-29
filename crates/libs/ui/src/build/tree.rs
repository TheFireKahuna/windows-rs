//! One app-thread node arena. Columns indexed by a dense generational id, intrusive links.
//!
//! Every layout input is a column a setter writes; the setter marks the node and walks to the
//! first ancestor already marked; the solve clears both marks on the way down. There is no
//! second child list, no lowered copy of a declaration and no per-node layout cache.

use crate::build::text::MeasureKey;
use crate::layout::{Layout, Rect, WidthClass};
use windows_numerics::Vector2;
use windows_scene::{
    Anim, Bind, Clip, ControlId, Corners, Forest, Ids, Links, NO_LINK, NODE, NodeId, Op, Prop,
    SinkPatch, Tuning, Value,
};

/// Absence in a `u32` head column: no row in the pool it heads.
pub(crate) const NONE: u32 = u32::MAX;

pub(crate) type Bits = u32;

pub(crate) const SPRITE: Bits = 1 << 0;
pub(crate) const DERIVED: Bits = 1 << 1;
pub(crate) const HIDDEN: Bits = 1 << 2;
pub(crate) const CLIP: Bits = 1 << 3;
pub(crate) const SCROLL: Bits = 1 << 4;
pub(crate) const SUSPENDED: Bits = 1 << 5;
/// The node's authored position takes it out of its container's flow: a mirror of
/// `Layout::position`, kept beside the bits a sibling walk already reads so that skipping an
/// out-of-flow child costs no read of its layout row.
pub(crate) const FLOATS: Bits = 1 << 6;
pub(crate) const RESPONSIVE: Bits = 1 << 7;
/// This node's own layout input moved.
pub(crate) const MEASURE: Bits = 1 << 8;
/// A descendant's did.
pub(crate) const DESC: Bits = 1 << 9;
/// This node holds a hit declaration; its eight authored flags are in [`DECL`].
pub(crate) const HIT: Bits = 1 << 10;
const CLASS: Bits = 0b11 << 11;
const CLASS_SHIFT: u32 = 11;
const OWN_CLASS: Bits = 0b11 << 13;
const OWN_CLASS_SHIFT: u32 = 13;
/// The eight authored bits of a hit declaration, packed by [`pack_decl`].
///
/// `CLIP` and `SCROLL` are the node's own bits and the entry's id is the `control` column, so
/// a declaration costs no row of its own and no second liveness check.
pub(crate) const DECL: Bits = 0xFF << 15;
pub(crate) const DECL_SHIFT: u32 = 15;
/// How this node announces itself when its content changes: nothing, politely, or by
/// interrupting. Two bits on the node, because a live region is a fact of the element and not
/// of the control that may or may not sit on it.
pub(crate) const LIVE: Bits = 0b11 << 23;
const LIVE_SHIFT: u32 = 23;
/// Announced once the client is idle.
pub(crate) const LIVE_POLITE: u32 = 1;
/// Announced by interrupting.
pub(crate) const LIVE_ASSERTIVE: u32 = 2;
/// Arranged away under a hidden ancestor, so this node's box and its derived sprites' take
/// no pixels. Written by the arrange alone; [`HIDDEN`] is the authored bit.
pub(crate) const SUNK: Bits = 1 << 25;
/// Walk B wrote this node's width this pass, so the boxes under it are not where the last
/// arrange left them even where its own snapped box is. Cleared by the arrange.
pub(crate) const PLACED: Bits = 1 << 26;
pub(crate) const ANIMATE_LAYOUT: Bits = 1 << 27;
pub(crate) const INITIAL: Bits = 1 << 28;
/// The sprite samples glyph coverage whose extent changes with the run resource.
pub(crate) const RUN: Bits = 1 << 29;
pub(crate) const ROUNDED_CLIP: Bits = 1 << 30;
/// The node is attached to a keyed anchor set, so its box moving moves that set.
pub(crate) const ANCHORED: Bits = 1 << 31;

// ── the hit declaration, packed ─────────────────────────────────────────────────────

/// `HitFlags::INTERACTIVE`, the one authored bit below `SCROLL`.
const DECL_LOW: u32 = 0b1;
/// `GESTURE`, `WHEEL`, `UIA`, `TEXT` and `NO_INFLATE`: the five between `SCROLL` and `CLIP`.
const DECL_MID: u32 = 0b0111_1100;
/// `BLOCKER` and `UNSCROLLED`, the two above `CLIP`.
const DECL_HIGH: u32 = 0b0011_0000_0000;

/// Compresses a declaration's flags into the eight bits [`DECL`] holds.
///
/// `HitFlags` defines ten bits and two of them — `SCROLL` and `CLIP` — are the node's own
/// [`SCROLL`] and [`CLIP`], so the eight that are authored are not contiguous and a bare
/// shift would drop `BLOCKER` and `UNSCROLLED` off the top.
pub(crate) const fn pack_decl(flags: u32) -> Bits {
    (flags & DECL_LOW) | ((flags & DECL_MID) >> 1) | ((flags & DECL_HIGH) >> 2)
}

/// Returns the declaration flags a node's `flags` word holds in [`DECL`], the inverse of
/// [`pack_decl`].
pub(crate) const fn unpack_decl(flags: Bits) -> u32 {
    let bits = (flags & DECL) >> DECL_SHIFT;
    (bits & 0b1) | ((bits & 0b0011_1110) << 1) | ((bits & 0b1100_0000) << 2)
}

// ── a dense pool behind a `u32` head ────────────────────────────────────────────────

/// Rows addressed by a `u32` head column, with a free list.
///
/// A vacant row is `None` rather than a sentinel value, so a head that names a freed row
/// reads absence instead of the previous occupant.
pub(crate) struct Pool<T> {
    rows: Vec<Option<T>>,
    free: Vec<u32>,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self { rows: Vec::new(), free: Vec::new() }
    }
}

impl<T> Pool<T> {
    /// Places `value` and returns the index the head column holds.
    pub(crate) fn place(&mut self, value: T) -> u32 {
        match self.free.pop() {
            Some(at) => {
                self.rows[at as usize] = Some(value);
                at
            }
            None => {
                self.rows.push(Some(value));
                self.rows.len() as u32 - 1
            }
        }
    }

    /// Vacates `at` and returns what it held.
    pub(crate) fn free(&mut self, at: u32) -> Option<T> {
        let row = self.rows.get_mut(at as usize)?.take()?;
        self.free.push(at);
        Some(row)
    }

    pub(crate) fn get(&self, at: u32) -> Option<&T> {
        self.rows.get(at as usize)?.as_ref()
    }

    pub(crate) fn get_mut(&mut self, at: u32) -> Option<&mut T> {
        self.rows.get_mut(at as usize)?.as_mut()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(at, row)| row.as_ref().map(|row| (at as u32, row)))
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &mut T)> {
        self.rows
            .iter_mut()
            .enumerate()
            .filter_map(|(at, row)| row.as_mut().map(|row| (at as u32, row)))
    }

    /// How many rows are placed.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.rows.len() - self.free.len()
    }

    /// How many slots the pool spans, placed and vacant.
    ///
    /// What a walk that also writes the tree counts over, since it cannot hold an iterator
    /// borrowed from the pool across that write.
    pub(crate) fn slots(&self) -> u32 {
        self.rows.len() as u32
    }
}

impl<T> core::ops::Index<u32> for Pool<T> {
    type Output = T;

    fn index(&self, at: u32) -> &T {
        self.get(at).expect("a head column names a placed row")
    }
}

impl<T> core::ops::IndexMut<u32> for Pool<T> {
    fn index_mut(&mut self, at: u32) -> &mut T {
        self.get_mut(at).expect("a head column names a placed row")
    }
}

// ── what the solve and the encode hold ──────────────────────────────────────────────

/// What the solve wrote for one node.
///
/// `rect` is the snapped box, and `local` and `size` are what the wire carries for it: the
/// box's origin against the parent's box and the box's extent. `at` and `at_h` are the
/// unsnapped origin and height walk C arranged the box from, kept so that a translate
/// re-snaps from the same numbers a fresh arrange would and lands on the same pixels.
///
/// `pair` is the shrink contract's two answers per axis, `[min_w, nat_w, min_h, nat_h]`, and
/// `at_w` the width the block pair was answered at, which is what lets walk B return without
/// descending when the width handed down has not moved.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub(crate) struct Geom {
    pub local: Vector2,
    pub size: Vector2,
    pub rect: Rect,
    pub pair: [f32; 4],
    pub at_w: f32,
    pub at: Vector2,
    pub at_h: f32,
}

/// What the last encode put on the wire for one node.
#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) struct Published {
    pub local: Vector2,
    pub size: Vector2,
    pub bounded: bool,
    /// The box was published arranged away under a hidden ancestor, as a zero box at the
    /// origin that no animation may start from.
    pub sunk: bool,
}

impl Published {
    /// A minted slot, so a recycled one never compares equal to its previous occupant.
    const MINTED: Self = Self {
        local: Vector2 { x: f32::NAN, y: f32::NAN },
        size: Vector2 { x: f32::NAN, y: f32::NAN },
        bounded: false,
        sunk: false,
    };
}

/// Declares the columns once, so minting and recycling a slot cannot disagree about them.
macro_rules! columns {
    ($($name:ident: $ty:ty = $empty:expr,)*) => {
        #[derive(Default)]
        pub(crate) struct Columns { $(pub $name: Vec<$ty>,)* }

        impl Columns {
            /// Seats row `at` at every column's empty value, growing where it is new.
            fn seat(&mut self, at: usize) {
                $(
                    if at < self.$name.len() {
                        self.$name[at] = $empty;
                    } else {
                        self.$name.resize(at + 1, $empty);
                    }
                )*
            }
        }
    };
}

columns! {
    links: Links = Links::default(),
    flags: Bits = 0,
    channels: u64 = 0,
    driven: u64 = 0,
    bindings: u32 = NONE,
    side: u32 = NONE,
    inflate: f32 = f32::NAN,
    scope: u32 = 0,
    control: ControlId = ControlId::NONE,
    text: MeasureKey = MeasureKey::NONE,
    paints: NodeId = NodeId::NONE,
    layout: Layout = Layout::DEFAULT,
    geom: Geom = Geom::default(),
    published: Published = Published::MINTED,
    scoped: u32 = 0,
    lead: u32 = NO_LINK,
}

/// The arena: one mint authority, one column set, one dirty frontier.
#[derive(Default)]
pub(crate) struct Tree {
    pub ids: Ids<NODE>,
    pub c: Columns,
    /// Nodes the publication moved, as full ids: a slot minted and destroyed in one
    /// flush passes an index check and fails a generation one, which is the case the encode
    /// has to skip.
    pub touched: Vec<NodeId>,
    /// Nodes whose box, width class or visibility changed, for the passes after the solve.
    ///
    /// The change set every per-row pass reads instead of scanning its whole table: a row is
    /// revisited where its node is named here, where its own inputs changed (each pass keeps
    /// that list itself), or on a sweep. May name a node twice, or a node since destroyed;
    /// every reader checks liveness and every pass is idempotent. The host drains what every
    /// pass has read at the end of a flush.
    pub moved: Vec<NodeId>,
    /// A setter under a detached root has no ancestor to mark, so it marks this instead.
    pub roots_dirty: bool,
    /// A hover flag, a mount or an unmount rebuilds the array and solves nothing.
    pub hits_dirty: bool,
    /// Whether each node's subtree put anything in the hit array on its last build, by node
    /// index. A box that moves under a node that bore nothing cannot change the array, so
    /// only a move where this holds, or past its end, rebuilds it.
    pub bears: Vec<bool>,
    /// A laid-out box moved since the last hit build: what the automation tree, which
    /// reads every box and not only the hit-bearing ones, goes stale on.
    pub boxes_moved: bool,
    /// The window extent changed since the last flush, so this flush's bounds follow the
    /// window 1:1: every layout write is a plain set, including under `ANIMATE_LAYOUT`.
    pub window_resized: bool,
    /// The stamp [`Tree::layout_scope`] answers are memoized under. See [`Tree::fresh_scopes`].
    epoch: u32,
    /// The ancestry one scope lookup climbed before it met an answer, kept for its capacity.
    climbed: Vec<u32>,
    /// How many nodes the scope lookups have climbed through, for a test to bound.
    #[cfg(test)]
    pub climbs: u32,
}

impl Forest for Tree {
    fn links(&self, at: u32) -> Links {
        self.c.links.get(at as usize).copied().unwrap_or_default()
    }

    fn links_mut(&mut self, at: u32) -> &mut Links {
        &mut self.c.links[at as usize]
    }

    fn id_at(&self, at: u32) -> NodeId {
        self.ids.id_at(at)
    }
}

impl Tree {
    /// Mints a node under `scope`, seating every column at its empty value.
    pub fn mint(&mut self, scope: u32) -> NodeId {
        let id = self.ids.mint();
        self.c.seat(id.index());
        self.c.flags[id.index()] |= INITIAL;
        self.c.scope[id.index()] = scope;
        self.hits_dirty = true;
        id
    }

    pub fn is_live(&self, id: NodeId) -> bool {
        self.ids.is_live(id)
    }

    /// The id at a link index, or [`NodeId::NONE`] for [`NO_LINK`].
    fn id_of(&self, at: u32) -> NodeId {
        if at == NO_LINK { NodeId::NONE } else { self.ids.id_at(at) }
    }

    pub fn parent(&self, id: NodeId) -> NodeId {
        self.id_of(self.links(id.index() as u32).parent)
    }

    /// This node's children, bottom to top: paint order, and the order the hit array is
    /// scanned in.
    pub fn children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        windows_scene::children(self, id.index() as u32)
    }

    /// Splices `id` above `after` under `parent`, dirtying both arrangements it changed.
    ///
    /// The splice cuts `id` out of whatever held it first, so a move changes two parents'
    /// child lists and both have to be marked. Marking a parentless one is a no-op.
    pub fn link(&mut self, id: NodeId, parent: NodeId, after: Option<NodeId>) {
        let held = self.parent(id);
        self.cut_lead(id);
        windows_scene::link(
            self,
            id.index() as u32,
            parent.index() as u32,
            after.map(|a| a.index() as u32),
        );
        self.join_lead(id);
        // A derived sprite takes no room, so gaining one is not a layout input of its parent.
        if self.c.flags[id.index()] & DERIVED == 0 {
            self.mark(held);
            self.mark(parent);
        }
        self.hits_dirty = true;
    }

    pub fn unlink(&mut self, id: NodeId) {
        let parent = self.parent(id);
        self.cut_lead(id);
        windows_scene::unlink(self, id.index() as u32);
        self.mark(parent);
        self.hits_dirty = true;
    }

    /// The link index of `n`'s first child the layout walks visit, or [`NO_LINK`].
    ///
    /// A node's chrome sits at the bottom of its child list, beneath its content, so every
    /// walk that starts a container's children would otherwise step over the same derived
    /// sprites again. This is the head past them, kept by [`Tree::link`] and [`Tree::unlink`]
    /// in constant time for every splice that does not cross a run of derived siblings.
    pub fn lead(&self, n: NodeId) -> u32 {
        let at = self.c.lead[n.index()];
        debug_assert_eq!(at, self.first_laid_out(self.links(n.index() as u32).first));
        at
    }

    /// The first non-derived node at or after link index `at`.
    fn first_laid_out(&self, mut at: u32) -> u32 {
        while at != NO_LINK && self.c.flags[at as usize] & DERIVED != 0 {
            at = self.c.links[at as usize].next;
        }
        at
    }

    /// Moves a parent's lead past `id` before `id` leaves that parent's list.
    fn cut_lead(&mut self, id: NodeId) {
        let at = id.index() as u32;
        let links = self.c.links[at as usize];
        if links.parent != NO_LINK && self.c.lead[links.parent as usize] == at {
            self.c.lead[links.parent as usize] = self.first_laid_out(links.next);
        }
    }

    /// Makes `id` its new parent's lead where it landed ahead of the old one.
    ///
    /// A derived sprite never leads. A laid-out node leads exactly when no laid-out sibling
    /// sits beneath it, which the walk down its `prev` links answers at the first one it
    /// meets.
    fn join_lead(&mut self, id: NodeId) {
        let at = id.index() as u32;
        let links = self.c.links[at as usize];
        if links.parent == NO_LINK || self.c.flags[at as usize] & DERIVED != 0 {
            return;
        }
        let mut below = links.prev;
        while below != NO_LINK && self.c.flags[below as usize] & DERIVED != 0 {
            below = self.c.links[below as usize].prev;
        }
        if below == NO_LINK {
            self.c.lead[links.parent as usize] = at;
        }
    }

    /// Appends `root`'s subtree to `out` in reverse preorder, children before their parent.
    ///
    /// Gathering is separate from releasing because releasing clears the links this walk
    /// reads: a retirement walks every root first, then destroys.
    pub fn gather(&self, root: NodeId, out: &mut Vec<NodeId>) {
        let from = out.len();
        out.push(root);
        let mut at = from;
        while at < out.len() {
            let node = out[at];
            out.extend(self.children(node));
            at += 1;
        }
        out[from..].reverse();
    }

    /// Releases one gathered node's slot. Its side rows are the caller's to free first.
    pub fn release(&mut self, id: NodeId) {
        self.unlink(id);
        self.ids.release(id);
    }

    /// Marks `n`'s own layout input changed and its ancestry as carrying a dirty descendant.
    pub fn mark(&mut self, n: NodeId) {
        if !self.ids.is_live(n) {
            return;
        }
        let flags = &mut self.c.flags[n.index()];
        // The walks skip a derived sprite, so a mark on one would never clear.
        if *flags & (MEASURE | DERIVED) != 0 {
            return;
        }
        *flags |= MEASURE;
        let parent = self.c.links[n.index()].parent;
        self.mark_desc(parent);
    }

    /// Walks to the first ancestor already carrying [`DESC`].
    ///
    /// The bit is monotone within a pass, so a builder chain of eight setters on one node
    /// walks its ancestry once.
    #[cold]
    fn mark_desc(&mut self, mut at: u32) {
        while at != NO_LINK {
            let flags = &mut self.c.flags[at as usize];
            if *flags & DESC != 0 {
                return;
            }
            *flags |= DESC;
            at = self.c.links[at as usize].parent;
        }
        self.roots_dirty = true;
    }

    /// Marks a responsive container's whole subtree, stopping at a nested responsive
    /// container, which classifies its own.
    #[cold]
    pub fn mark_subtree(&mut self, n: NodeId, stack: &mut Vec<NodeId>) {
        stack.push(n);
        while let Some(at) = stack.pop() {
            self.mark(at);
            for child in self.children(at) {
                if self.c.flags[child.index()] & RESPONSIVE == 0 {
                    stack.push(child);
                }
            }
        }
    }

    /// Marks `n` and every node under it, responsive containers included.
    ///
    /// Preorder over the links with no stack: down the first child, along the next sibling,
    /// and up the parent until the walk is back at `n`.
    #[cold]
    pub fn mark_shown(&mut self, n: NodeId) {
        let root = n.index() as u32;
        let mut at = root;
        loop {
            self.mark(self.id_at(at));
            let first = self.links(at).first;
            if first != NO_LINK {
                at = first;
                continue;
            }
            loop {
                if at == root {
                    return;
                }
                let links = self.links(at);
                if links.next != NO_LINK {
                    at = links.next;
                    break;
                }
                at = links.parent;
            }
        }
    }

    pub fn class(&self, n: NodeId) -> WidthClass {
        WidthClass::from_bits((self.c.flags[n.index()] & CLASS) >> CLASS_SHIFT)
    }

    pub fn set_class(&mut self, n: NodeId, class: WidthClass) {
        let flags = &mut self.c.flags[n.index()];
        let next = (*flags & !CLASS) | (class.bits() << CLASS_SHIFT);
        if next != *flags {
            *flags = next;
            // A mask, a run and a probe each resolve at the class, and a box can keep its
            // extent across a flip.
            self.moved.push(n);
        }
    }

    pub fn own_class(&self, n: NodeId) -> WidthClass {
        WidthClass::from_bits((self.c.flags[n.index()] & OWN_CLASS) >> OWN_CLASS_SHIFT)
    }

    pub fn set_own_class(&mut self, n: NodeId, class: WidthClass) {
        let flags = &mut self.c.flags[n.index()];
        *flags = (*flags & !OWN_CLASS) | (class.bits() << OWN_CLASS_SHIFT);
    }

    /// Sets or clears one flag and marks the node. The path every non-layout setter takes.
    pub fn set_flag(&mut self, n: NodeId, bit: Bits, on: bool) {
        if !self.ids.is_live(n) {
            return;
        }
        let held = self.c.flags[n.index()];
        let next = if on { held | bit } else { held & !bit };
        if next != held {
            self.c.flags[n.index()] = next;
            // `SUSPENDED` is read by the hit walk alone. A derived sprite takes no room and
            // the walks skip it, so none of its flags is a layout input: a publisher may hide
            // one after the solve without asking for another.
            if bit & SUSPENDED == 0 && held & DERIVED == 0 {
                // A hidden subtree was arranged away without being measured, so showing it
                // takes every node in it again, not only the one whose bit moved.
                if bit & HIDDEN != 0 && !on {
                    self.mark_shown(n);
                } else {
                    self.mark(n);
                }
            }
            // The hit walk returns at a derived sprite, so no bit on one reaches the array.
            if held & next & DERIVED == 0 {
                self.hits_dirty = true;
            } else if bit & HIDDEN != 0 {
                // Marks nothing, so the visuals pass learns of it here.
                self.moved.push(n);
            }
        }
    }

    /// Edits `n`'s authored layout and marks it. The path every layout setter takes.
    pub fn author(&mut self, n: NodeId, write: impl FnOnce(&mut Layout)) {
        if !self.ids.is_live(n) {
            return;
        }
        write(&mut self.c.layout[n.index()]);
        self.sync_floats(n);
        self.mark(n);
    }

    /// Mirrors `n`'s authored position into [`FLOATS`]. Every writer of the layout row
    /// calls it after the write.
    fn sync_floats(&mut self, n: NodeId) {
        let floats = !self.c.layout[n.index()].position.in_flow();
        let flags = &mut self.c.flags[n.index()];
        *flags = if floats { *flags | FLOATS } else { *flags & !FLOATS };
    }

    /// Edits `n`'s authored layout and marks it only where the edit changed it.
    ///
    /// For a writer that restates a derived input every flush: [`Tree::author`] would mark
    /// the node, and the solve would re-place its children, whether or not anything moved.
    pub fn restate(&mut self, n: NodeId, write: impl FnOnce(&mut Layout)) {
        if !self.ids.is_live(n) {
            return;
        }
        let held = self.c.layout[n.index()];
        write(&mut self.c.layout[n.index()]);
        if self.c.layout[n.index()] != held {
            self.sync_floats(n);
            self.mark(n);
        }
    }

    /// How this node announces a change to its content, off a flag word a walk already read.
    pub fn live_bits(&self, flags: Bits) -> u32 {
        (flags & LIVE) >> LIVE_SHIFT
    }

    /// States how this node announces a change to its content.
    pub fn set_live(&mut self, n: NodeId, live: u32) {
        let flags = &mut self.c.flags[n.index()];
        *flags = (*flags & !LIVE) | ((live << LIVE_SHIFT) & LIVE);
        self.hits_dirty = true;
    }

    /// Mirrors a preset row's own bits into the node's flag word.
    ///
    /// The four the row declares are read off the node by the solve, the encode and the hit
    /// walk, so creation is where they cross and a preset written over an earlier one replaces
    /// them rather than adding to them.
    pub fn adopt_layout_bits(&mut self, n: NodeId) {
        let stated = self.c.layout[n.index()].preset.node_flags();
        let kept = self.c.flags[n.index()] & !(HIDDEN | CLIP | SCROLL | RESPONSIVE);
        self.c.flags[n.index()] = kept | stated;
        self.sync_floats(n);
        self.hits_dirty = true;
    }

    /// Records that the publication wrote `n`'s box, for the encode to read.
    ///
    /// A box written where it already was on the wire is not recorded: the encode would emit
    /// nothing for it, and a publisher that restates every box it owns each flush would
    /// otherwise grow the list by every node it owns rather than by the ones that moved.
    pub fn touch(&mut self, n: NodeId) {
        if self.stale(n) {
            self.touched.push(n);
        }
    }

    /// Returns whether `n`'s box, clip or sunk state differs from what the encode last put on
    /// the wire: everything [`Tree::encode`] would emit an op or a hit rebuild for.
    ///
    /// Every write of a box is followed by [`Tree::touch`], and only the encode writes the
    /// published column, so a node this answers `false` for at its touch cannot become
    /// stale before the encode reads it without being touched again.
    pub fn stale(&self, n: NodeId) -> bool {
        let i = n.index();
        let (now, was, flags) = (&self.c.geom[i], &self.c.published[i], self.c.flags[i]);
        now.local != was.local
            || now.size != was.size
            || (flags & CLIP != 0) != was.bounded
            || (flags & SUNK != 0) != was.sunk
    }

    /// Returns the first live node that still claims its input is unsolved, with its flags.
    #[cfg(debug_assertions)]
    pub fn unsettled(&self) -> Option<(NodeId, Bits)> {
        // The node whose own input moved is the one to name; an ancestor carrying only `DESC`
        // is reported where no such node is left.
        let first = |bits: Bits| {
            self.c.flags.iter().enumerate().find_map(|(at, &flags)| {
                let id = self.ids.id_at(at as u32);
                (flags & bits != 0 && self.ids.is_live(id)).then_some((id, flags))
            })
        };
        first(MEASURE).or_else(|| first(DESC))
    }

    /// Starts a fresh generation of [`Tree::layout_scope`] answers.
    ///
    /// Called at the head of each pass that asks, because between passes a node can be
    /// relinked or an ancestor can gain or lose [`ANIMATE_LAYOUT`], and nothing on those
    /// paths should pay to invalidate a memo. Within one pass neither happens, so each answer
    /// holds for the rest of it.
    pub(crate) fn fresh_scopes(&mut self) {
        self.epoch += 1;
        // The stamp is the epoch shifted past the answer bit. At the top of the range every
        // stamp is forgotten at once, so a wrapped epoch never matches a stale one.
        if self.epoch >= 1 << 31 {
            self.c.scoped.fill(0);
            self.epoch = 1;
        }
    }

    /// Returns whether `id` or any ancestor declares [`ANIMATE_LAYOUT`].
    ///
    /// Memoized per node for the current [`Tree::fresh_scopes`] generation: a lookup climbs
    /// only until it meets a node already answered this pass, and stamps every node it
    /// climbed on the way back down. Over a pass the climbs visit each ancestor once in
    /// total, so asking for every touched node costs the touched nodes plus their distinct
    /// ancestry rather than the touched count times the depth.
    pub(crate) fn layout_scope(&mut self, id: NodeId) -> bool {
        debug_assert!(self.epoch != 0, "fresh_scopes opens every pass that asks");
        let stamp = self.epoch << 1;
        let mut climbed = core::mem::take(&mut self.climbed);
        let mut at = id.index() as u32;
        let inside = loop {
            if at == NO_LINK {
                break false;
            }
            let seen = self.c.scoped[at as usize];
            if seen & !1 == stamp {
                break seen & 1 != 0;
            }
            climbed.push(at);
            #[cfg(test)]
            {
                self.climbs += 1;
            }
            if self.c.flags[at as usize] & ANIMATE_LAYOUT != 0 {
                break true;
            }
            at = self.c.links[at as usize].parent;
        };
        for &at in &climbed {
            self.c.scoped[at as usize] = stamp | inside as u32;
        }
        climbed.clear();
        self.climbed = climbed;
        inside
    }

    /// Returns whether a box that moved under a layout scope springs rather than snaps,
    /// given the scope answer the caller already holds.
    fn springs(&self, id: NodeId, scoped: bool) -> bool {
        let was = self.c.published[id.index()];
        scoped
            && was.size.x.is_finite()
            && !was.sunk
            && self.c.flags[id.index()] & (SUNK | INITIAL) == 0
            && !self.window_resized
    }

    /// Returns whether a move of `id`'s box springs. Opens no generation of its own: the
    /// caller's pass has called [`Tree::fresh_scopes`].
    pub(crate) fn animates_layout(&mut self, id: NodeId) -> bool {
        let scoped = self.layout_scope(id);
        self.springs(id, scoped)
    }

    /// Emits the geometry the publication moved and takes the published column to now.
    ///
    /// Field-wise rather than one row: a value writer owns `OffsetX` on a driven part, and
    /// re-sending an unchanged offset would snap a thumb back to where the application last
    /// wrote it. A node gathered and then destroyed in the same flush fails the liveness
    /// check and emits nothing; the scene's cascade on its parent's drop covers the rest.
    ///
    /// A node touched twice, or touched and then moved back, is not stale by the time it is
    /// read and costs one comparison. The scope of the rest is asked once each, through the
    /// memo, so the pass is linear in what moved.
    pub fn encode(&mut self, patch: &mut SinkPatch) {
        if self.touched.is_empty() {
            return;
        }
        self.fresh_scopes();
        for at in 0..self.touched.len() {
            let id = self.touched[at];
            if !self.ids.is_live(id) || !self.stale(id) {
                continue;
            }
            let now = self.c.geom[id.index()];
            let was = self.c.published[id.index()];
            let flags = self.c.flags[id.index()];
            let bounded = flags & CLIP != 0;
            let live_clip = self.layout_scope(id);
            let sunk = flags & SUNK != 0;
            let animated = self.springs(id, live_clip);
            let text = flags & RUN != 0 || self.c.text[id.index()] != MeasureKey::NONE;
            let write = |prop, value| Op::Bind {
                id,
                prop,
                bind: if animated && !(prop == Prop::Size && text) {
                    Bind::Animate(Anim::Spring { to: value, tuning: Tuning::Layout, delay_ms: 0 })
                } else {
                    Bind::Set(value)
                },
            };
            // The array holds absolute rects, so a box that moved leaves it stale where anything
            // under it is in the array. The hit walk skips a derived sprite and everything under
            // it, so one moving leaves the array exactly as it was: a meter fill or a text tile
            // restated every frame rebuilds nothing, and neither does a label whose subtree
            // declares no target.
            if (now.local != was.local || now.size != was.size) && flags & DERIVED == 0 {
                self.boxes_moved = true;
                if self.bears.get(id.index()).copied().unwrap_or(true) {
                    self.hits_dirty = true;
                }
            }
            if now.local != was.local {
                // A driven channel is not the layout's to write. `Prop::Offset` carries both
                // axes, and setting it replaces whatever animates `Offset.Y` — which is the
                // tracker expression a scroll container's content and its thumb ride. The
                // axis nothing drives is still the layout's, and goes out on its own channel.
                let driven = self.c.driven[id.index()];
                let held = |prop: Prop| driven & (1 << prop as u32) != 0;
                match (held(Prop::OffsetX), held(Prop::OffsetY)) {
                    (false, false) => patch.push(write(Prop::Offset, Value::Vec2(now.local))),
                    (false, true) => {
                        patch.push(write(Prop::OffsetX, Value::Scalar(now.local.x)));
                    }
                    (true, false) => {
                        patch.push(write(Prop::OffsetY, Value::Scalar(now.local.y)));
                    }
                    (true, true) => {}
                }
            }
            if now.size != was.size {
                patch.push(write(Prop::Size, Value::Vec2(now.size)));
            }
            // A clip is declared, not diffed, scene-side, and declaring the absence of one
            // mints a side row on every node that never had one.
            if flags & ROUNDED_CLIP == 0
                && (bounded != was.bounded || (bounded && !live_clip && now.size != was.size)) {
                let clip = if bounded && live_clip {
                    Clip::Bounds
                } else if bounded {
                    Clip::Rect {
                        l: 0.0,
                        t: 0.0,
                        r: now.size.x,
                        b: now.size.y,
                        radius: Corners::default(),
                    }
                } else {
                    Clip::None
                };
                patch.push(Op::Clip { id, clip });
            }
            self.c.published[id.index()] =
                Published { local: now.local, size: now.size, bounded, sunk };
        }
        self.touched.clear();
    }
}
