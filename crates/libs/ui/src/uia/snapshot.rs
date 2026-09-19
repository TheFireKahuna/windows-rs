//! The published snapshot: one preorder table, one string pool, one live column.
//!
//! A client's call reads an immutable snapshot from automation's own worker thread and never
//! enters the window's message pump, so a provider method cannot block a screen reader and a
//! client walking the tree at idle costs no front-thread wakes.
//!
//! The rows are emitted by the same preorder walk over the arena that fills the hit array, so
//! the two describe one layout and element-from-point scans the same order the pointer does.
//! Navigation is index arithmetic over the one vector rather than a second tree.
//!
//! Strings are UTF-16 because that is what automation returns. Storing them as `str` would
//! cost a transcode on every name query and an offset table for every text range.

use super::roles::{self, Patterns};
use crate::text_input::{Geometry, Selection};
use crate::widget::{Range, UiaRole};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};
use windows_numerics::Vector2;
use windows_scene::{ControlId, NodeId, Point};

/// The index no element sits at: an absent parent, child or sibling.
pub const NONE: u16 = u16::MAX;

/// Structural facts about an element that a click cannot change.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct ColFlags(pub u16);

impl ColFlags {
    /// No flags set.
    pub const NONE: Self = Self(0);
    /// Answers `ExpandCollapse`: the element owns a flyout.
    pub const EXPANDS: Self = Self(1 << 0);
    /// Answers `SelectionItem` even though its role does not imply it.
    pub const SELECTS: Self = Self(1 << 1);
    /// Takes keyboard focus.
    pub const FOCUSABLE: Self = Self(1 << 2);
    /// A live region announced once the client is idle.
    pub const LIVE_POLITE: Self = Self(1 << 3);
    /// A live region that interrupts to announce.
    pub const LIVE_ASSERTIVE: Self = Self(1 << 4);
    /// A popup, which announces itself as a dialog and is read title-first.
    pub const DIALOG: Self = Self(1 << 5);
    /// Carries a number in a range: a row in the range column holds the bounds.
    pub const RANGED: Self = Self(1 << 6);
    /// Publishes its body as a text document.
    pub const BODY: Self = Self(1 << 7);
    /// The body is editable, so it is a field row rather than a run in the pool.
    pub const FIELD: Self = Self(1 << 8);
    /// Took its name from the run before it, which is what `LabeledBy` reports.
    pub const LABELLED: Self = Self(1 << 9);

    /// Returns whether every bit set in `other` is set here.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for ColFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// One published element. 44 bytes, and every field is read by a named automation call.
///
/// | field | who reads it |
/// |---|---|
/// | `id` | `GetRuntimeId`, every raised event, [`Tree::index_of`] |
/// | `box_` | `get_BoundingRectangle`, `ElementProviderFromPoint`, `IsOffscreen` |
/// | `name` | `Name`, and `LabeledBy` through the sibling that donated it |
/// | `parent` | `Navigate(Parent)`, and the parent role `ControlType` resolves against |
/// | `child` | `Navigate(FirstChild)`, `Navigate(LastChild)`, `ISelectionProvider::GetSelection` |
/// | `next` | `Navigate(NextSibling)`, `Navigate(PreviousSibling)` by walking |
/// | `clip` | `IsOffscreen`, and the scan's clip rejection |
/// | `scroll` | `get_BoundingRectangle`, which subtracts that scroll row's offset |
/// | `flags` | `IsKeyboardFocusable`, `IsDialog`, `LiveSetting`, `ExpandCollapseState`, `IsSelected`, `IsPassword`, and which patterns [`Tree::patterns`] adds or clears |
/// | `role` | `ControlType`, `LocalizedControlType`, `IsContentElement`, the pattern mask |
///
/// There is no `last_child` and no `previous_sibling`: both are a walk of one parent's list,
/// which costs four bytes an element to store and is asked for once per client keystroke.
/// Help text, the automation-id segment and a numeric range are sparse side tables rather
/// than columns, because most elements carry none of the three.
///
/// `parent` and `clip` each name an element earlier in the table, because a preorder walk
/// emits an ancestor before its descendants. Every walk upward in this module terminates on
/// that, and [`link`]'s backward pass is correct because of it.
#[derive(Copy, Clone, Default, Debug)]
pub struct Entry {
    pub id: ControlId,
    /// Absolute layout DIPs, unscrolled, as `left, top, right, bottom`.
    pub box_: [f32; 4],
    /// Offset of this element's name in the pool, or zero. The unit before it is its length.
    pub name: u32,
    pub parent: u16,
    pub child: u16,
    pub next: u16,
    /// The nearest clipping ancestor, or [`NONE`].
    pub clip: u16,
    /// Which scroll row holds the offset this element's box resolves through, or [`NONE`].
    pub scroll: u16,
    pub flags: ColFlags,
    pub role: UiaRole,
}

/// One nameable area inside a presentation region.
///
/// A region's contents are a buffer, so nothing in them can be an entry in the hit array.
/// Parts are what makes a band handle nameable, focusable and value-reporting anyway. Their
/// rects are region-local and move whenever the renderer's mapping does, so they are
/// published separately from the tree rather than built into it.
#[derive(Copy, Clone, Debug)]
pub struct Part {
    pub sub: u32,
    pub name: &'static str,
    pub role: UiaRole,
    /// Region-local DIPs, as `left, top, right, bottom`.
    pub rect: [f32; 4],
}

/// One editable document, published separately from its element's accessible name.
#[derive(Clone)]
pub(crate) struct FieldText {
    pub id: ControlId,
    pub revision: u64,
    pub text: Arc<[u16]>,
    pub selection: Selection,
    pub geometry: Option<Arc<Geometry>>,
    pub password: bool,
}

/// Per-entry model state. The bits a click changes and a layout does not.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct State(pub u32);

impl State {
    pub const ENABLED: Self = Self(1 << 0);
    pub const TOGGLED: Self = Self(1 << 1);
    pub const SELECTED: Self = Self(1 << 2);
    pub const EXPANDED: Self = Self(1 << 3);

    /// Returns whether every flag in `other` is set.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns this state with the flags in `other` set or cleared according to `on`.
    #[must_use]
    pub const fn with(self, other: Self, on: bool) -> Self {
        Self(if on {
            self.0 | other.0
        } else {
            self.0 & !other.0
        })
    }
}

impl core::ops::BitOr for State {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// What the application thread hands over: the published table as rows, already in preorder,
/// plus the pool and the sparse columns those rows index.
///
/// `Send`, because every row holds ids, numbers and `&'static str`s and every string has
/// already been resolved into the pool on the thread that owns the text table.
#[derive(Default)]
pub struct Snapshot {
    /// Preorder, parents before children. `parent`, `clip` and `scroll` are filled during
    /// the walk; `child` and `next` are filled by [`link`] on adoption.
    pub entries: Vec<Entry>,
    pub blob: Vec<u16>,
    /// Help text, sorted by entry index.
    pub helps: Vec<(u16, u32)>,
    /// The automation-id segment, sorted by entry index.
    pub keys: Vec<(u16, &'static str)>,
    /// The bounds a number moves between, sorted by entry index.
    pub ranges: Vec<(u16, Range)>,
    pub(crate) fields: Vec<FieldText>,
    /// The scroll containers the rows resolve through, deduplicated, in the order
    /// [`Entry::scroll`] indexes them.
    pub scrolls: Vec<NodeId>,
    /// The per-entry initial model state, parallel to [`Snapshot::entries`].
    pub state: Vec<State>,
}

impl Snapshot {
    /// Appends `text` to the pool and returns its offset, or zero for an empty string.
    ///
    /// Each run is written behind its own length, so an [`Entry`] names a string with one
    /// `u32` and the pool answers how long it is.
    ///
    /// # Contract
    ///
    /// `text` must encode to at most `u16::MAX` UTF-16 units, which is the width of the
    /// length the pool writes. A longer run is truncated to that.
    pub fn intern(&mut self, text: &str) -> u32 {
        if text.is_empty() {
            return 0;
        }
        self.blob.push(0);
        let at = self.blob.len() as u32;
        self.blob
            .extend(text.encode_utf16().take(u16::MAX as usize));
        let len = self.blob.len() as u32 - at;
        self.blob[at as usize - 1] = len as u16;
        at
    }

    /// Empties every row and the pool, keeping every allocation for the next publish.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.blob.clear();
        self.helps.clear();
        self.keys.clear();
        self.ranges.clear();
        self.fields.clear();
        self.scrolls.clear();
        self.state.clear();
    }
}

/// The `f64::NAN` bit pattern, standing for no value. A written value reads back only when it
/// is finite, so this cannot collide with one.
const NO_VALUE: u64 = 0x7ff8_0000_0000_0000;

/// The published tree: one preorder table, one pool, and the live column beside them.
///
/// Immutable but for the live column, and shared by `Arc`.
pub struct Tree {
    entries: Box<[Entry]>,
    pool: Box<[u16]>,
    /// Control id to entry index, sorted for binary search. A client holds an id, and the
    /// table is positional.
    by_id: Box<[(ControlId, u16)]>,
    /// Sparse columns, each sorted by entry index: help text, the automation-id segment, and
    /// the bounds of a number. Each is a minority, and each would otherwise widen every entry.
    helps: Box<[(u16, u32)]>,
    keys: Box<[(u16, &'static str)]>,
    ranges: Box<[(u16, Range)]>,
    /// Sorted by control id, which is what a client's text call holds.
    fields: Box<[FieldText]>,
    /// One word an entry, holding its value as `f64` bits.
    ///
    /// Relaxed throughout: each word stands alone, with no other datum ordered against it, so
    /// a reader takes whichever whole value is current. The tree these words index into is
    /// published under its own release-acquire pair, which is [`Versioned`].
    live: Box<[AtomicU64]>,
    state: Box<[AtomicU32]>,
    /// One packed offset per scroll container, keyed by node. A handful per screen, so the
    /// lookup a write does is a scan and the lookup a read does is an index.
    scrolls: Box<[(NodeId, AtomicU64)]>,
    /// The focused control as a packed generational id. Focus is singular, so it is one word,
    /// and an id rather than an index because an index is only meaningful until the next
    /// republish.
    focus: AtomicU64,
    /// The window's top-left in physical pixels packed as two `f32`s, and `dpi / 96`.
    ///
    /// Automation speaks screen pixels and everything above speaks DIPs; these convert
    /// between them. Both are written on every window move, because a stale origin reports
    /// every control at the wrong place.
    origin: AtomicU64,
    scale: AtomicU32,
}

impl Default for Tree {
    fn default() -> Self {
        Self::empty()
    }
}

impl Tree {
    /// Returns a tree with no elements: what a window holds before anything is published, and
    /// what it publishes while no client is attached.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: Box::default(),
            pool: Box::default(),
            by_id: Box::default(),
            helps: Box::default(),
            keys: Box::default(),
            ranges: Box::default(),
            fields: Box::default(),
            live: Box::default(),
            state: Box::default(),
            scrolls: Box::default(),
            focus: AtomicU64::new(u64::MAX),
            origin: AtomicU64::new(0),
            scale: AtomicU32::new(1.0f32.to_bits()),
        }
    }

    /// Adopts the rows the application thread produced.
    ///
    /// One pass copies the rows and the pool; [`link`] fills the sibling links and
    /// [`adopt_labels`] runs over them. The field rows are sorted here because
    /// [`field`](Self::field) binary-searches them by control id. Runs when layout changed,
    /// never per frame.
    #[must_use]
    pub fn adopt(snapshot: &Snapshot) -> Self {
        let mut entries = snapshot.entries.clone();
        // Links are `u16` and [`NONE`] is the last of them, so the table stops one row short
        // of that: an element sitting at `NONE` would be reachable as every absent link.
        entries.truncate(NONE as usize);
        link(&mut entries);
        adopt_labels(&mut entries);
        let mut by_id: Box<[_]> = entries
            .iter()
            .enumerate()
            .map(|(at, entry)| (entry.id, at as u16))
            .collect();
        by_id.sort_unstable_by_key(|&(id, _)| id);
        let mut fields = snapshot.fields.clone();
        fields.sort_by_key(|row| row.id);
        debug_assert!(
            snapshot.helps.windows(2).all(|pair| pair[0].0 <= pair[1].0)
                && snapshot.keys.windows(2).all(|pair| pair[0].0 <= pair[1].0)
                && snapshot
                    .ranges
                    .windows(2)
                    .all(|pair| pair[0].0 <= pair[1].0),
            "a sparse column reached the adopt out of entry order"
        );
        Self {
            live: entries.iter().map(|_| AtomicU64::new(NO_VALUE)).collect(),
            state: (0..entries.len())
                .map(|at| AtomicU32::new(snapshot.state.get(at).copied().unwrap_or_default().0))
                .collect(),
            entries: entries.into_boxed_slice(),
            pool: snapshot.blob.clone().into_boxed_slice(),
            by_id,
            helps: snapshot.helps.clone().into_boxed_slice(),
            keys: snapshot.keys.clone().into_boxed_slice(),
            ranges: snapshot.ranges.clone().into_boxed_slice(),
            fields: fields.into_boxed_slice(),
            scrolls: snapshot
                .scrolls
                .iter()
                .map(|&node| (node, AtomicU64::new(0)))
                .collect(),
            ..Self::empty()
        }
    }

    /// Returns every published element, in the order a scan reads it.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Returns the entry at `at`, or `None` past the end.
    #[must_use]
    pub fn at(&self, at: u16) -> Option<&Entry> {
        self.entries.get(at as usize)
    }

    /// Returns whether the tree publishes no elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the entry index `id` sits at, or `None` when the tree does not hold it.
    #[must_use]
    pub fn index_of(&self, id: ControlId) -> Option<u16> {
        let found = self.by_id.binary_search_by_key(&id, |&(key, _)| key).ok()?;
        Some(self.by_id[found].1)
    }

    /// Returns the UTF-16 run the pool holds at `at`, or an empty slice for zero.
    #[must_use]
    pub fn text(&self, at: u32) -> &[u16] {
        let Some(&len) = at.checked_sub(1).and_then(|at| self.pool.get(at as usize)) else {
            return &[];
        };
        let at = at as usize;
        self.pool.get(at..at + len as usize).unwrap_or_default()
    }

    /// Returns the help text `at` publishes, which most elements do not carry.
    #[must_use]
    pub fn help(&self, at: u16) -> &[u16] {
        match self.helps.binary_search_by_key(&at, |&(key, _)| key) {
            Ok(found) => self.text(self.helps[found].1),
            Err(_) => &[],
        }
    }

    /// Returns the automation-id segment `at` publishes, widened only where a client asks.
    #[must_use]
    pub fn key(&self, at: u16) -> Option<&'static str> {
        let found = self.keys.binary_search_by_key(&at, |&(key, _)| key).ok()?;
        Some(self.keys[found].1)
    }

    /// Returns the bounds `at` moves between, or `None` where it carries no number.
    #[must_use]
    pub fn range(&self, at: u16) -> Option<Range> {
        let found = self
            .ranges
            .binary_search_by_key(&at, |&(key, _)| key)
            .ok()?;
        Some(self.ranges[found].1)
    }

    /// Returns the editable document `id` publishes, or `None` where it has none.
    pub(crate) fn field(&self, id: ControlId) -> Option<&FieldText> {
        let found = self.fields.binary_search_by_key(&id, |row| row.id).ok()?;
        Some(&self.fields[found])
    }

    /// Returns the patterns the element at `at` answers: its role's, adjusted by the flags it
    /// declared.
    #[must_use]
    pub fn patterns(&self, at: u16) -> Patterns {
        let Some(entry) = self.at(at) else {
            return Patterns::NONE;
        };
        let mut out = roles::row(entry.role).patterns;
        if !entry.flags.has(ColFlags::EXPANDS) {
            out = out.without(Patterns::EXPAND);
        }
        if entry.flags.has(ColFlags::SELECTS) {
            out = out.or(Patterns::SELECTION_ITEM);
        }
        out
    }

    /// Returns the sibling before `at`, by walking its parent's child list.
    ///
    /// The parentless elements are the window's children and their list head is entry zero, so
    /// a top-level element needs no separate case.
    #[must_use]
    pub fn previous(&self, at: u16) -> u16 {
        let parent = self.at(at).map_or(NONE, |entry| entry.parent);
        let mut walk = self.at(parent).map_or(0, |entry| entry.child);
        let mut before = NONE;
        while walk != NONE && walk != at {
            before = walk;
            walk = self.at(walk).map_or(NONE, |entry| entry.next);
        }
        before
    }

    /// Returns the last of `at`'s children, by walking to the end of the list.
    ///
    /// [`NONE`] names the window, whose children are the parentless elements, so the root's
    /// last child is the end of that list.
    #[must_use]
    pub fn last_child(&self, at: u16) -> u16 {
        let mut walk = match self.at(at) {
            Some(entry) => entry.child,
            None if at == NONE && !self.is_empty() => 0,
            None => return NONE,
        };
        let mut last = NONE;
        while walk != NONE {
            last = walk;
            walk = self.at(walk).map_or(NONE, |entry| entry.next);
        }
        last
    }

    /// Returns the entry under `p`, in layout DIPs, or `None` where nothing is.
    ///
    /// Scans back to front and takes the first entry whose box holds the point and whose clip
    /// ancestry admits it. The rows are in the preorder the walk emitted, which is paint
    /// order, so the last match is the topmost — the rule the pointer's own array is scanned
    /// under, over boxes that walk filled at the same moment.
    ///
    /// A contact's touch inflation is not applied: a client asking what is at a screen point
    /// is naming a pixel, not placing a finger.
    #[must_use]
    pub fn hit(&self, p: Point) -> Option<u16> {
        (0..self.entries.len() as u16).rev().find(|&at| {
            let entry = &self.entries[at as usize];
            inside(entry.box_, self.resolve(p, at)) && !self.clipped_out(p, entry.clip)
        })
    }

    /// Returns `p` in the layout space of the entry at `at`.
    ///
    /// Layout places content unscrolled and the compositor applies the offset, so a query
    /// moves the point rather than the rects.
    fn resolve(&self, p: Point, at: u16) -> Point {
        let by = self.scroll(at);
        Point {
            x: p.x + by.x,
            y: p.y + by.y,
        }
    }

    /// Returns whether any clipping ancestor from `clip` upward excludes `p`.
    ///
    /// Terminates because a clipping ancestor sits earlier in the table than what it clips.
    fn clipped_out(&self, p: Point, mut clip: u16) -> bool {
        while let Some(entry) = self.at(clip) {
            if !inside(entry.box_, self.resolve(p, clip)) {
                return true;
            }
            clip = entry.clip;
        }
        false
    }

    /// Returns the box of the entry at `at` with its scroll offset applied, in layout DIPs.
    #[must_use]
    pub fn shifted(&self, at: u16) -> [f32; 4] {
        let Some(entry) = self.at(at) else {
            return [0.0; 4];
        };
        let by = self.scroll(at);
        [
            entry.box_[0] - by.x,
            entry.box_[1] - by.y,
            entry.box_[2] - by.x,
            entry.box_[3] - by.y,
        ]
    }

    /// Returns whether a clipping ancestor excludes the element's own box.
    ///
    /// Walks the clip chain rather than testing against the window: an element scrolled out of
    /// a list is offscreen, and one below the fold of a window is not. Terminates for the same
    /// reason [`clipped_out`](Self::clipped_out) does.
    #[must_use]
    pub fn clipped(&self, at: u16) -> bool {
        let me = self.shifted(at);
        let mut clip = self.at(at).map_or(NONE, |entry| entry.clip);
        while let Some(entry) = self.at(clip) {
            let bound = self.shifted(clip);
            if me[0] >= bound[2] || me[1] >= bound[3] || me[2] <= bound[0] || me[3] <= bound[1] {
                return true;
            }
            clip = entry.clip;
        }
        false
    }

    /// Returns the value at `at`, or `None` where none was written, the value is not finite,
    /// or the index is past the end.
    #[must_use]
    pub fn value(&self, at: u16) -> Option<f64> {
        let held = f64::from_bits(self.live.get(at as usize)?.load(Relaxed));
        held.is_finite().then_some(held)
    }

    /// Stores `value` at `at`. An index past the end is ignored.
    pub fn set_value(&self, at: u16, value: f64) {
        if let Some(word) = self.live.get(at as usize) {
            word.store(value.to_bits(), Relaxed);
        }
    }

    /// Returns the offset the element at `at` is scrolled by, or zero where nothing scrolls it.
    #[must_use]
    pub fn scroll(&self, at: u16) -> Vector2 {
        let row = self.at(at).map_or(NONE, |entry| entry.scroll);
        let bits = self
            .scrolls
            .get(row as usize)
            .map_or(0, |(_, word)| word.load(Relaxed));
        unpack(bits)
    }

    /// Stores the offset of the scroll container `node` names. A node the published tree
    /// scrolls nothing by is ignored.
    pub fn set_scroll(&self, node: NodeId, offset: Vector2) {
        if let Some((_, word)) = self.scrolls.iter().find(|(held, _)| *held == node) {
            word.store(pack(offset), Relaxed);
        }
    }

    /// Returns the flags set at `at`, or none where the index is past the end.
    #[must_use]
    pub fn state(&self, at: u16) -> State {
        State(
            self.state
                .get(at as usize)
                .map_or(0, |word| word.load(Relaxed)),
        )
    }

    /// Sets or clears `flag` at `at`, leaving the other flags as they are.
    pub fn set_state(&self, at: u16, flag: State, on: bool) {
        if let Some(word) = self.state.get(at as usize) {
            word.store(State(word.load(Relaxed)).with(flag, on).0, Relaxed);
        }
    }

    /// Returns the focused control as a packed generational id.
    #[must_use]
    pub fn focused(&self) -> u64 {
        self.focus.load(Relaxed)
    }

    /// Stores the focused control's packed generational id.
    pub fn set_focused(&self, id: u64) {
        self.focus.store(id, Relaxed);
    }

    /// Returns the window's client origin in physical pixels and the DIP scale there.
    #[must_use]
    pub fn window(&self) -> (Vector2, f32) {
        (
            unpack(self.origin.load(Relaxed)),
            f32::from_bits(self.scale.load(Relaxed)),
        )
    }

    /// Stores the window's client origin in physical pixels and the DIP scale there.
    pub fn set_window(&self, origin: Vector2, scale: f32) {
        self.origin.store(pack(origin), Relaxed);
        self.scale.store(scale.to_bits(), Relaxed);
    }

    /// Copies the outgoing tree's live column into this one, by control id.
    ///
    /// An element the outgoing tree did not hold keeps the state it was built with. A
    /// republish is a layout change, which disables no control and moves no slider, so
    /// without this a resize would announce every toggle as reset.
    pub fn carry(&self, from: &Self) {
        for (at, entry) in self.entries.iter().enumerate() {
            let Some(was) = from.index_of(entry.id) else {
                continue;
            };
            self.live[at].store(from.live[was as usize].load(Relaxed), Relaxed);
            self.state[at].store(from.state[was as usize].load(Relaxed), Relaxed);
        }
        for (node, word) in &*self.scrolls {
            if let Some((_, was)) = from.scrolls.iter().find(|(held, _)| held == node) {
                word.store(was.load(Relaxed), Relaxed);
            }
        }
        self.focus.store(from.focused(), Relaxed);
        let (origin, scale) = from.window();
        self.set_window(origin, scale);
    }
}

/// Returns whether `p` is inside `box_`, given as `left, top, right, bottom`.
fn inside(box_: [f32; 4], p: Point) -> bool {
    p.x >= box_[0] && p.x <= box_[2] && p.y >= box_[1] && p.y <= box_[3]
}

/// Returns the two coordinates packed into one word, `x` above `y`.
const fn pack(v: Vector2) -> u64 {
    ((v.x.to_bits() as u64) << 32) | v.y.to_bits() as u64
}

/// Returns the two coordinates [`pack`] wrote into one word.
const fn unpack(bits: u64) -> Vector2 {
    Vector2 {
        x: f32::from_bits((bits >> 32) as u32),
        y: f32::from_bits(bits as u32),
    }
}

/// Fills the child and sibling links from the parent column in one backward pass.
///
/// The pass runs backward so that pushing each element to the front of its parent's list
/// leaves the list in forward order.
///
/// The parentless elements are a sibling list too, whose parent is the window. Leaving them
/// unlinked gives every top-level element after the first no sibling to be reached through, so
/// a client's walk stops at the first one — and an overlay is top-level, which would put every
/// open menu but one out of reach.
fn link(out: &mut [Entry]) {
    let mut roots = NONE;
    // Cleared up front rather than in the pass: a preorder table has `parent < at`, so a parent
    // is visited after its children and would wipe the links they had just written.
    for entry in out.iter_mut() {
        entry.child = NONE;
    }
    for at in (0..out.len() as u16).rev() {
        let parent = out[at as usize].parent;
        out[at as usize].next = if parent == NONE {
            roots
        } else {
            out[parent as usize].child
        };
        if parent == NONE {
            roots = at;
        } else {
            out[parent as usize].child = at;
        }
    }
    // The first published element is parentless, because nothing precedes it to be its
    // ancestor, so the root list head is always entry zero and is not stored.
    debug_assert!(out.is_empty() || roots == 0);
}

/// Gives a control with no text of its own the name of the run immediately before it.
///
/// A form row is `(label("Gain"), slider(..))`: the name is a sibling, not a child, so nothing
/// the slider owns can derive it. [`ColFlags::LABELLED`] records that it was adopted, and
/// `LabeledBy` answers with the sibling the walk finds again.
///
/// The match is narrow — only the immediately preceding sibling, only when it is a static run
/// carrying a name, and only for a control that has none — so it cannot reach across a row,
/// claim a heading two controls up, or overwrite a name an author wrote.
fn adopt_labels(out: &mut [Entry]) {
    for at in 0..out.len() as u16 {
        let me = out[at as usize];
        if me.name != 0 || matches!(me.role, UiaRole::Text | UiaRole::Group) {
            continue;
        }
        let mut walk = if me.parent == NONE {
            0
        } else {
            out[me.parent as usize].child
        };
        let mut before = NONE;
        while walk != NONE && walk != at {
            before = walk;
            walk = out[walk as usize].next;
        }
        let Some(label) = out.get(before as usize).copied() else {
            continue;
        };
        if label.role == UiaRole::Text && label.name != 0 {
            out[at as usize].name = label.name;
            out[at as usize].flags = me.flags | ColFlags::LABELLED;
        }
    }
}

/// A value published under a version, so a reader holding the current one takes no lock.
///
/// A reader compares one acquire load against the version its own copy was taken at and takes
/// the lock only when it moved. A poisoned lock is stepped over rather than propagated: what
/// is behind it is plain data, and a publisher that panicked left either the outgoing value or
/// the incoming one, never a half-built one.
#[derive(Default)]
pub struct Versioned<T> {
    version: AtomicU64,
    held: Mutex<T>,
}

impl<T> Versioned<T> {
    /// Returns the published version.
    pub fn version(&self) -> u64 {
        // acquire: pairs with the release in `write`, so a reader that sees this version sees
        // the value published under it.
        self.version.load(core::sync::atomic::Ordering::Acquire)
    }

    /// Runs `edit` under the lock, then advances the version.
    pub fn write<R>(&self, edit: impl FnOnce(&mut T) -> R) -> R {
        let out = edit(&mut self.held.lock().unwrap_or_else(PoisonError::into_inner));
        // release: pairs with the acquire in `version`, so the edit is in place before the
        // version advertising it becomes visible.
        self.version
            .fetch_add(1, core::sync::atomic::Ordering::Release);
        out
    }

    /// Runs `read` under the lock.
    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.held.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl Versioned<Arc<Tree>> {
    /// Returns the current tree, refreshing `cached` only when the version has moved.
    ///
    /// A reader keeps its own `Arc` rather than copying out of a slot the writer cycles, so
    /// there is no cutover window for a reader to land in.
    pub fn tree(&self, cached: &core::cell::RefCell<Option<(u64, Arc<Tree>)>>) -> Arc<Tree> {
        let version = self.version();
        let mut cached = cached.borrow_mut();
        if let Some((seen, tree)) = cached.as_ref()
            && *seen == version
        {
            return Arc::clone(tree);
        }
        let tree = self.read(Arc::clone);
        *cached = Some((version, Arc::clone(&tree)));
        tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(count: usize) -> Vec<ControlId> {
        let mut authority = windows_scene::Ids::<{ windows_scene::CONTROL }>::default();
        (0..count).map(|_| authority.mint()).collect()
    }

    /// Appends one element under `parent` and returns its index.
    fn row(
        out: &mut Snapshot,
        id: ControlId,
        parent: u16,
        role: UiaRole,
        name: &str,
        box_: [f32; 4],
    ) -> u16 {
        let at = out.entries.len() as u16;
        let name = out.intern(name);
        out.entries.push(Entry {
            id,
            box_,
            name,
            parent,
            child: NONE,
            next: NONE,
            clip: NONE,
            scroll: NONE,
            flags: ColFlags::FOCUSABLE,
            role,
        });
        out.state.push(State::ENABLED);
        at
    }

    /// Returns a snapshot of `count` elements: a parentless group and the rest its children.
    fn fan(count: usize) -> (Snapshot, Vec<ControlId>) {
        let id = ids(count);
        let mut out = Snapshot::default();
        row(&mut out, id[0], NONE, UiaRole::Group, "root", [0.0; 4]);
        for &child in &id[1..] {
            row(&mut out, child, 0, UiaRole::Button, "row", [0.0; 4]);
        }
        (out, id)
    }

    #[test]
    fn siblings_link_in_forward_order() {
        let (snapshot, _) = fan(4);
        let tree = Tree::adopt(&snapshot);

        let mut walk = Vec::new();
        let mut at = tree.at(0).unwrap().child;
        while at != NONE {
            walk.push(at);
            at = tree.at(at).unwrap().next;
        }
        assert_eq!(walk, [1, 2, 3], "paint order is the order they are read in");
        assert_eq!(tree.last_child(0), 3);
        assert_eq!(tree.previous(2), 1);
    }

    /// Walks the parentless elements as one sibling list: a client reaches the second
    /// top-level element through the first's `next`, and an unlinked list would stop the walk
    /// at the first — putting every open overlay but one out of reach.
    #[test]
    fn the_parentless_elements_are_a_sibling_list_and_not_a_set_of_orphans() {
        let id = ids(4);
        let mut snapshot = Snapshot::default();
        row(
            &mut snapshot,
            id[0],
            NONE,
            UiaRole::Group,
            "panel",
            [0.0; 4],
        );
        row(&mut snapshot, id[1], 0, UiaRole::Button, "row", [0.0; 4]);
        // A second panel, and an overlay above it.
        row(
            &mut snapshot,
            id[2],
            NONE,
            UiaRole::Group,
            "second",
            [0.0; 4],
        );
        row(&mut snapshot, id[3], NONE, UiaRole::Menu, "menu", [0.0; 4]);
        let tree = Tree::adopt(&snapshot);

        let mut walk = vec![0u16];
        while let Some(&at) = walk.last() {
            let next = tree.at(at).unwrap().next;
            if next == NONE {
                break;
            }
            walk.push(next);
        }
        assert_eq!(walk, [0, 2, 3], "every top-level element is reachable");
        assert_eq!(
            tree.last_child(NONE),
            3,
            "and the root's last child is the last of them"
        );
        assert_eq!(
            tree.at(1).unwrap().next,
            NONE,
            "a child is not in that list"
        );
    }

    #[test]
    fn a_name_survives_the_round_trip_through_the_pool() {
        let (snapshot, _) = fan(2);
        let tree = Tree::adopt(&snapshot);
        let name = |at: u16| String::from_utf16_lossy(tree.text(tree.at(at).unwrap().name));
        assert_eq!(name(0), "root");
        assert_eq!(name(1), "row");
        assert!(tree.text(0).is_empty(), "zero names no run");
    }

    /// A form row is a label and a control side by side, so the control's name is a sibling
    /// rather than anything it owns.
    #[test]
    fn a_label_is_adopted_from_the_run_before_it() {
        let id = ids(4);
        let mut snapshot = Snapshot::default();
        row(&mut snapshot, id[0], NONE, UiaRole::Group, "", [0.0; 4]);
        row(&mut snapshot, id[1], 0, UiaRole::Text, "Gain", [0.0; 4]);
        row(&mut snapshot, id[2], 0, UiaRole::Slider, "", [0.0; 4]);
        row(&mut snapshot, id[3], 0, UiaRole::Button, "Reset", [0.0; 4]);
        let tree = Tree::adopt(&snapshot);

        let name = |at: u16| String::from_utf16_lossy(tree.text(tree.at(at).unwrap().name));
        assert_eq!(name(2), "Gain", "the slider takes the run before it");
        assert!(tree.at(2).unwrap().flags.has(ColFlags::LABELLED));
        assert_eq!(name(3), "Reset", "and a control with its own name keeps it");
        assert!(!tree.at(3).unwrap().flags.has(ColFlags::LABELLED));
    }

    #[test]
    fn an_unwritten_value_is_absent_rather_than_zero() {
        let (snapshot, _) = fan(2);
        let tree = Tree::adopt(&snapshot);
        assert_eq!(tree.value(0), None, "zero is a value a slider can hold");
        tree.set_value(0, -14.5);
        assert_eq!(tree.value(0), Some(-14.5));
        assert_eq!(tree.value(9), None, "and an index past the end is absent");
    }

    #[test]
    fn a_packed_offset_survives_the_round_trip() {
        let (mut snapshot, _) = fan(2);
        snapshot.scrolls.push(NodeId::FIRST);
        snapshot.entries[1].scroll = 0;
        let tree = Tree::adopt(&snapshot);
        tree.set_scroll(NodeId::FIRST, Vector2 { x: -3.5, y: 128.25 });
        let back = tree.scroll(1);
        assert_eq!((back.x, back.y), (-3.5, 128.25));
        assert_eq!(tree.scroll(0).y, 0.0, "an element outside it is unscrolled");
    }

    #[test]
    fn carrying_forward_moves_state_to_its_new_index() {
        let (was, id) = fan(3);
        let old = Tree::adopt(&was);
        old.set_value(1, 7.0);
        old.set_state(1, State::TOGGLED, true);
        old.set_focused(42);
        old.set_window(Vector2 { x: 4.0, y: 8.0 }, 1.5);

        // The same controls, with a new element ahead of them, so their indices move.
        let mut now = Snapshot::default();
        let extra = ids(1)[0];
        row(&mut now, extra, NONE, UiaRole::Text, "output", [0.0; 4]);
        row(&mut now, id[0], NONE, UiaRole::Group, "root", [0.0; 4]);
        row(&mut now, id[1], 1, UiaRole::Button, "row", [0.0; 4]);
        let new = Tree::adopt(&now);
        new.carry(&old);

        assert_eq!(new.value(2), Some(7.0), "a resize is not a reset");
        assert!(new.state(2).has(State::TOGGLED));
        assert_eq!(new.value(0), None, "and what was not held is left alone");
        assert_eq!(new.focused(), 42);
        assert_eq!(new.window().1, 1.5);
    }

    /// The scan takes the last entry in the table that holds the point, which is the topmost,
    /// and a clipping ancestor removes what it excludes.
    #[test]
    fn a_scan_takes_the_topmost_entry_a_clip_admits() {
        let id = ids(3);
        let mut snapshot = Snapshot::default();
        row(
            &mut snapshot,
            id[0],
            NONE,
            UiaRole::Group,
            "list",
            [0.0, 0.0, 100.0, 100.0],
        );
        row(
            &mut snapshot,
            id[1],
            0,
            UiaRole::Button,
            "row",
            [0.0, 0.0, 200.0, 200.0],
        );
        row(
            &mut snapshot,
            id[2],
            0,
            UiaRole::Button,
            "over",
            [10.0, 10.0, 40.0, 40.0],
        );
        snapshot.entries[1].clip = 0;
        snapshot.entries[2].clip = 0;
        let tree = Tree::adopt(&snapshot);

        assert_eq!(
            tree.hit(Point { x: 20.0, y: 20.0 }),
            Some(2),
            "the topmost wins"
        );
        assert_eq!(tree.hit(Point { x: 80.0, y: 80.0 }), Some(1));
        assert_eq!(
            tree.hit(Point { x: 150.0, y: 150.0 }),
            None,
            "past the clip the row is gone, and so is the clip itself"
        );
    }

    /// A scrolled element is found where it is drawn: the offset moves the point, and the
    /// entry's own box never moves.
    #[test]
    fn a_scrolled_row_is_found_where_it_is_drawn() {
        let id = ids(2);
        let mut snapshot = Snapshot::default();
        row(
            &mut snapshot,
            id[0],
            NONE,
            UiaRole::List,
            "presets",
            [0.0, 0.0, 100.0, 100.0],
        );
        row(
            &mut snapshot,
            id[1],
            0,
            UiaRole::Button,
            "vocal",
            [0.0, 200.0, 100.0, 240.0],
        );
        snapshot.entries[1].clip = 0;
        snapshot.entries[1].scroll = 0;
        snapshot.scrolls.push(NodeId::FIRST);
        let tree = Tree::adopt(&snapshot);

        assert_eq!(
            tree.hit(Point { x: 50.0, y: 20.0 }),
            Some(0),
            "unscrolled the row is below the fold"
        );
        tree.set_scroll(NodeId::FIRST, Vector2 { x: 0.0, y: 200.0 });
        assert_eq!(tree.hit(Point { x: 50.0, y: 20.0 }), Some(1));
    }

    #[test]
    fn a_reader_sees_a_publish_and_reuses_its_own_reference_until_then() {
        let cache = core::cell::RefCell::new(None);
        let slot = Versioned::<Arc<Tree>>::default();
        let first = slot.tree(&cache);
        assert!(Arc::ptr_eq(&first, &slot.tree(&cache)));

        slot.write(|held| *held = Arc::new(Tree::empty()));
        assert!(
            !Arc::ptr_eq(&first, &slot.tree(&cache)),
            "a publish is observed"
        );
    }
}
