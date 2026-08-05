//! The published accessibility tree, and the seeds it is built from.
//!
//! The tree is the hit array, plus four columns and a string blob. Copying the entries into
//! the snapshot lets automation run the same scan over the same data from its own thread,
//! so hit-testing and element-from-point share one implementation and cannot diverge.
//!
//! Strings are UTF-16 because that is what automation returns. Storing them as `str` would
//! cost a transcode on every name query and an offset table for every text range.

use super::live::{Live, State};
use super::roles::{self, Patterns};
use crate::widget::{Range, UiaRole};
use windows_scene::{ControlId, HitEntry, HitFlags, NO_ENTRY, NodeId};

/// A span of the string blob.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Text {
    pub at: u32,
    pub len: u32,
}

impl Text {
    /// Returns whether the span covers no units.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// What kind of value an element carries, and the bounds it moves between.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum Value {
    #[default]
    None,
    /// A number in a range. The number itself is live; only the bounds are structural.
    Range(Range),
    /// A string, whose body is the element's own text.
    Text,
}

/// One element's structural columns, held beside its [`HitEntry`] at the same index.
#[derive(Copy, Clone, Debug)]
pub struct Col {
    /// Index of the enclosing element, or [`NO_ENTRY`] for a child of the fragment root.
    pub parent: u32,
    pub first_child: u32,
    pub next_sibling: u32,
    pub last_child: u32,
    pub name: Text,
    pub help: Text,
    /// The element this one takes its name from, or [`NO_ENTRY`].
    ///
    /// A slider, a toggle and a knob carry no text of their own, so a form row states the
    /// name beside them rather than inside them. Both halves are published: `Name`, which
    /// most clients read directly, and `LabeledBy`, which says where the name came from, so
    /// a reader that navigates to the label does not announce it a second time as the
    /// control's own.
    pub labelled_by: u32,
    /// The automation-id segment, kept as a `&'static str` and widened only when a client
    /// asks for it.
    pub key: Option<&'static str>,
    pub role: UiaRole,
    pub value: Value,
    pub flags: ColFlags,
}

/// Structural facts about an element that a click cannot change.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
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

/// One nameable area inside a presentation region.
///
/// A region's contents are a buffer, so nothing in them can be an entry in the hit array.
/// Parts are what makes a band handle nameable, focusable and value-reporting anyway. Their
/// rects are region-local and move whenever the renderer's mapping does, so they are
/// published separately from the tree rather than built into it.
#[derive(Clone, Debug)]
pub struct Part {
    pub sub: u32,
    pub name: &'static str,
    pub role: UiaRole,
    /// Region-local DIPs.
    pub rect: (f32, f32, f32, f32),
    pub value: Option<f64>,
}

/// What the application thread hands over: everything automation needs that the hit array
/// does not already carry.
///
/// `Send`, because the rows hold ids and `&'static str`s and every string has already been
/// resolved into the blob on the thread that owns the text table.
#[derive(Debug, Default)]
pub struct Seeds {
    pub rows: Vec<Seed>,
    pub blob: Vec<u16>,
}

/// One element's automation facts, joined against the hit array by control id.
#[derive(Copy, Clone, Debug)]
pub struct Seed {
    pub id: ControlId,
    pub role: UiaRole,
    pub name: Text,
    pub help: Text,
    pub key: Option<&'static str>,
    pub value: Value,
    pub flags: ColFlags,
    pub state: State,
}

impl Seeds {
    /// Appends `text` to the blob and returns its span.
    pub fn intern(&mut self, text: &str) -> Text {
        if text.is_empty() {
            return Text::default();
        }
        let at = self.blob.len() as u32;
        self.blob.extend(text.encode_utf16());
        Text {
            at,
            len: self.blob.len() as u32 - at,
        }
    }

    /// Empties the rows and the blob, keeping both allocations for the next publish.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.blob.clear();
    }

    /// Sorts the rows by control id, which is what makes the join in [`Tree::build`] a
    /// binary search per entry rather than a scan.
    pub fn sort(&mut self) {
        self.rows.sort_unstable_by_key(|seed| seed.id);
    }
}

/// The published tree. Immutable but for [`live`](Self::live), and shared by `Arc`.
#[derive(Debug)]
pub struct Tree {
    entries: Box<[HitEntry]>,
    cols: Box<[Col]>,
    blob: Box<[u16]>,
    /// The parentless elements, as one list. Their parent is the window, which is the one
    /// element that is not in the array.
    first_root: u32,
    last_root: u32,
    /// Control id to entry index, sorted for binary search. A client holds an id, and the
    /// array is positional.
    by_id: Box<[(ControlId, u32)]>,
    pub live: Live,
}

impl Tree {
    /// Returns a tree with no elements: what a window holds before anything is published,
    /// and what it publishes while no client is attached.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: Box::default(),
            cols: Box::default(),
            blob: Box::default(),
            first_root: NO_ENTRY,
            last_root: NO_ENTRY,
            by_id: Box::default(),
            live: Live::new(0, core::iter::empty()),
        }
    }

    /// Joins the adopted hit array against the seeds the application thread produced.
    ///
    /// One pass selects and copies the entries that carry a seed, one fills the sibling
    /// links, and the string blob is cloned whole. Runs when layout changed, never per
    /// frame.
    #[must_use]
    pub fn build(entries: &[HitEntry], seeds: &Seeds) -> Self {
        // `kept[i]` is the index an original entry landed at, or `NO_ENTRY`. Ancestry is
        // stated in the original index space and has to be rewritten into this one. An
        // element whose parent has no seed reparents to the nearest ancestor that does,
        // which makes a decorative wrapper invisible rather than a gap in the tree.
        let mut kept = vec![NO_ENTRY; entries.len()];
        let mut rows = Vec::new();
        let mut out = Vec::new();
        for (at, entry) in entries.iter().enumerate() {
            if !entry.flags.contains(HitFlags::UIA) {
                continue;
            }
            let Ok(found) = seeds
                .rows
                .binary_search_by_key(&entry.id, |seed| seed.id)
                .map(|found| &seeds.rows[found])
            else {
                continue;
            };
            if found.role == UiaRole::None {
                continue;
            }
            kept[at] = out.len() as u32;
            out.push(*entry);
            rows.push(*found);
        }

        let mut cols: Box<[Col]> = rows
            .iter()
            .zip(&out)
            .map(|(seed, entry)| Col {
                parent: nearest(&kept, entries, entry.parent),
                first_child: NO_ENTRY,
                next_sibling: NO_ENTRY,
                last_child: NO_ENTRY,
                name: seed.name,
                help: seed.help,
                labelled_by: NO_ENTRY,
                key: seed.key,
                role: seed.role,
                value: seed.value,
                flags: seed.flags,
            })
            .collect();
        let (first_root, last_root) = link(&mut cols);
        adopt_labels(&mut cols);

        let by_id = sorted_index(&out);
        // Keyed by the nodes descendants resolve through, which is not the same set as the
        // entries that scroll: a container's own entry names its ancestor's offset, because
        // the builder fills `scroll_src` before pushing the container onto its own stack.
        // Keying on the containers would leave every lookup finding nothing, and a scan
        // over scrolled content would answer as if nothing had scrolled.
        let mut sources: Vec<NodeId> = out
            .iter()
            .map(|entry| entry.scroll_src)
            .filter(|node| !node.is_none())
            .collect();
        sources.sort_unstable();
        sources.dedup();
        let live = Live::new(out.len(), sources.into_iter());
        for (at, seed) in rows.iter().enumerate() {
            live.set_state(at, seed.state, true);
            live.set_state(at, State(!seed.state.0), false);
        }

        Self {
            entries: out.into_boxed_slice(),
            cols,
            blob: seeds.blob.clone().into_boxed_slice(),
            first_root,
            last_root,
            by_id,
            live,
        }
    }

    /// Returns the number of published elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the tree publishes no elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the first and last elements the window itself parents.
    #[must_use]
    pub const fn roots(&self) -> (u32, u32) {
        (self.first_root, self.last_root)
    }

    /// Returns the published hit array, in the order a scan reads it.
    #[must_use]
    pub fn entries(&self) -> &[HitEntry] {
        &self.entries
    }

    /// Returns the entry at `at`, or `None` past the end.
    #[must_use]
    pub fn entry(&self, at: usize) -> Option<&HitEntry> {
        self.entries.get(at)
    }

    /// Returns the columns for the element at `at`, or `None` past the end.
    #[must_use]
    pub fn col(&self, at: usize) -> Option<&Col> {
        self.cols.get(at)
    }

    /// Returns the entry index `id` sits at, or `None` when the tree does not hold it.
    #[must_use]
    pub fn index_of(&self, id: ControlId) -> Option<usize> {
        let at = self.by_id.binary_search_by_key(&id, |&(key, _)| key).ok()?;
        Some(self.by_id[at].1 as usize)
    }

    /// Returns the UTF-16 units a span names, or an empty slice for a span past the blob.
    #[must_use]
    pub fn text(&self, span: Text) -> &[u16] {
        let at = span.at as usize;
        self.blob
            .get(at..at + span.len as usize)
            .unwrap_or_default()
    }

    /// Returns the patterns the element at `at` answers: its role's, adjusted by the flags
    /// it declared.
    #[must_use]
    pub fn patterns(&self, at: usize) -> Patterns {
        let Some(col) = self.cols.get(at) else {
            return Patterns::NONE;
        };
        let mut patterns = roles::row(col.role).patterns;
        if !col.flags.has(ColFlags::EXPANDS) {
            patterns = patterns.without(Patterns::EXPAND);
        }
        if col.flags.has(ColFlags::SELECTS) {
            patterns = patterns.or(Patterns::SELECTION_ITEM);
        }
        patterns
    }

    /// Returns every element both trees hold, as its index in `from` paired with its index
    /// here. This is the mapping that carries the live half forward across a republish.
    pub fn remap<'a>(&'a self, from: &'a Self) -> impl Iterator<Item = (usize, usize)> + 'a {
        self.by_id
            .iter()
            .filter_map(move |&(id, to)| Some((from.index_of(id)?, to as usize)))
    }
}

/// Returns the nearest ancestor of `parent` that survived the select pass, in the new index
/// space, or [`NO_ENTRY`] when none did.
fn nearest(kept: &[u32], entries: &[HitEntry], mut parent: u32) -> u32 {
    let mut guard = entries.len();
    while parent != NO_ENTRY && guard > 0 {
        let at = parent as usize;
        if kept.get(at).is_some_and(|&index| index != NO_ENTRY) {
            return kept[at];
        }
        parent = entries.get(at).map_or(NO_ENTRY, |entry| entry.parent);
        guard -= 1;
    }
    NO_ENTRY
}

/// Fills the sibling and child links from the parent column in one backward pass, and
/// returns the first and last parentless element.
///
/// The pass runs backward so that pushing each element to the front of its parent's list
/// leaves the list in forward order.
///
/// The parentless elements are a sibling list too, whose parent is the window. Leaving them
/// unlinked gives every top-level element after the first no sibling to be reached through,
/// so a client's walk stops at the first one — and an overlay is top-level, which would put
/// every open menu but one out of reach.
fn link(cols: &mut [Col]) -> (u32, u32) {
    let (mut first_root, mut last_root) = (NO_ENTRY, NO_ENTRY);
    for at in (0..cols.len()).rev() {
        let parent = cols[at].parent;
        let (head, tail) = if parent == NO_ENTRY {
            (first_root, last_root)
        } else {
            let up = &cols[parent as usize];
            (up.first_child, up.last_child)
        };
        cols[at].next_sibling = head;
        let tail = if head == NO_ENTRY { at as u32 } else { tail };
        if parent == NO_ENTRY {
            (first_root, last_root) = (at as u32, tail);
        } else {
            let up = &mut cols[parent as usize];
            (up.first_child, up.last_child) = (at as u32, tail);
        }
    }
    (first_root, last_root)
}

/// Gives a control with no text of its own the name of the run immediately before it, and
/// records that run in `labelled_by`.
///
/// A form row is `(label("Gain"), slider(..))`: the name is a sibling, not a child, so
/// nothing the slider owns can derive it.
///
/// The match is narrow — only the immediately preceding sibling, only when it is a static
/// run carrying a name, and only for a control that has none — so it cannot reach across a
/// row, claim a heading two controls up, or overwrite a name an author wrote.
fn adopt_labels(cols: &mut [Col]) {
    for at in 0..cols.len() {
        let col = cols[at];
        if !col.name.is_empty() || col.role == UiaRole::Text || col.role == UiaRole::Group {
            continue;
        }
        // The sibling before it, walked from the parent's list: the columns carry only the
        // forward links, so this is the one direction that costs a walk.
        let mut previous = NO_ENTRY;
        let mut next = match cols.get(col.parent as usize) {
            Some(up) => up.first_child,
            None => (0..cols.len())
                .find(|&index| cols[index].parent == NO_ENTRY)
                .map_or(NO_ENTRY, |index| index as u32),
        };
        while next != NO_ENTRY && next as usize != at {
            previous = next;
            next = cols[next as usize].next_sibling;
        }
        let Some(label) = cols.get(previous as usize) else {
            continue;
        };
        if label.role == UiaRole::Text && !label.name.is_empty() {
            cols[at].name = label.name;
            cols[at].labelled_by = previous;
        }
    }
}

/// Returns each entry's control id paired with its index, sorted by id for binary search.
fn sorted_index(entries: &[HitEntry]) -> Box<[(ControlId, u32)]> {
    let mut index: Box<[(ControlId, u32)]> = entries
        .iter()
        .enumerate()
        .map(|(at, entry)| (entry.id, at as u32))
        .collect();
    index.sort_unstable_by_key(|&(id, _)| id);
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mints `count` ids through the authority the stack uses, so they are dense from one
    /// and generational.
    fn ids(count: usize) -> Vec<ControlId> {
        let mut authority = windows_scene::Ids::<windows_scene::Control>::new();
        (0..count).map(|_| authority.mint()).collect()
    }

    fn entry(id: ControlId, parent: u32, uia: bool) -> HitEntry {
        HitEntry {
            x0: 0.0,
            y0: 0.0,
            x1: 10.0,
            y1: 10.0,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent,
            flags: if uia {
                HitFlags::INTERACTIVE | HitFlags::UIA
            } else {
                HitFlags::INTERACTIVE
            },
            scroll_src: NodeId::NONE,
            id,
        }
    }

    fn seeds(ids: &[ControlId]) -> Seeds {
        let mut seeds = Seeds::default();
        for &id in ids {
            let name = seeds.intern("row");
            seeds.rows.push(Seed {
                id,
                role: UiaRole::Button,
                name,
                help: Text::default(),
                key: None,
                value: Value::None,
                flags: ColFlags::NONE,
                state: State::ENABLED,
            });
        }
        seeds.sort();
        seeds
    }

    #[test]
    fn an_element_without_a_peer_is_skipped_and_its_children_reparent_past_it() {
        let id = ids(3);
        // 0: group (peer) · 1: a bare wrapper (no peer) · 2: its child (peer)
        let entries = [
            entry(id[0], NO_ENTRY, true),
            entry(id[1], 0, false),
            entry(id[2], 1, true),
        ];
        let tree = Tree::build(&entries, &seeds(&[id[0], id[2]]));

        assert_eq!(tree.len(), 2, "the wrapper contributes no element");
        assert_eq!(tree.col(0).unwrap().parent, NO_ENTRY);
        assert_eq!(
            tree.col(1).unwrap().parent,
            0,
            "the child reaches the nearest ancestor that has a peer"
        );
        assert_eq!(tree.col(0).unwrap().first_child, 1);
        assert_eq!(tree.col(0).unwrap().last_child, 1);
    }

    #[test]
    fn siblings_link_in_forward_order() {
        let id = ids(4);
        let entries = [
            entry(id[0], NO_ENTRY, true),
            entry(id[1], 0, true),
            entry(id[2], 0, true),
            entry(id[3], 0, true),
        ];
        let tree = Tree::build(&entries, &seeds(&id));

        let mut walk = Vec::new();
        let mut at = tree.col(0).unwrap().first_child;
        while at != NO_ENTRY {
            walk.push(at);
            at = tree.col(at as usize).unwrap().next_sibling;
        }
        assert_eq!(walk, [1, 2, 3], "paint order is the order they are read in");
        assert_eq!(tree.col(0).unwrap().last_child, 3);
    }

    /// Walks the parentless elements as one sibling list: a client reaches the second
    /// top-level element through the first's `next_sibling`, and an unlinked list would
    /// stop the walk at the first — putting every open overlay but one out of reach.
    #[test]
    fn the_parentless_elements_are_a_sibling_list_and_not_a_set_of_orphans() {
        let id = ids(4);
        let entries = [
            entry(id[0], NO_ENTRY, true),
            entry(id[1], 0, true),
            // Two more top-level elements: a second panel, and an overlay above it.
            entry(id[2], NO_ENTRY, true),
            entry(id[3], NO_ENTRY, true),
        ];
        let tree = Tree::build(&entries, &seeds(&id));

        let (first, last) = tree.roots();
        assert_eq!((first, last), (0, 3));
        let mut walk = vec![first];
        while let Some(&at) = walk.last() {
            let next = tree.col(at as usize).unwrap().next_sibling;
            if next == NO_ENTRY {
                break;
            }
            walk.push(next);
        }
        assert_eq!(walk, [0, 2, 3], "every top-level element is reachable");
        assert_eq!(
            tree.col(1).unwrap().next_sibling,
            NO_ENTRY,
            "and a child is not in that list"
        );
    }

    #[test]
    fn a_name_survives_the_round_trip_through_the_blob() {
        let id = ids(1);
        let entries = [entry(id[0], NO_ENTRY, true)];
        let tree = Tree::build(&entries, &seeds(&id));
        let name = tree.text(tree.col(0).unwrap().name);
        assert_eq!(String::from_utf16_lossy(name), "row");
    }

    #[test]
    fn an_entry_with_no_seed_is_not_published() {
        let id = ids(2);
        let entries = [entry(id[0], NO_ENTRY, true), entry(id[1], NO_ENTRY, true)];
        let tree = Tree::build(&entries, &seeds(&id[..1]));
        assert_eq!(tree.len(), 1, "a peer needs both a flag and a seed");
    }
}
