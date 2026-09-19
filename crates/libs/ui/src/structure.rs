//! Structure: the two places a tree changes shape, and what owns each piece while it does.
//!
//! A view function runs once, at mount, and installs [`Effect`](crate::signal::Effect)s that
//! update sinks thereafter. There is no re-render and so no hook order: a cell is created once
//! per scope, at whatever point in the code creates it, including inside a conditional.
//! Structure therefore changes only where the shape genuinely changes, and there are two such
//! places.
//!
//! | | shape | mechanism |
//! |---|---|---|
//! | [`Keyed`] | a list whose items are inserted, removed and reordered | key delta + a longest increasing subsequence |
//! | [`Branch`] | a subtree that is present or absent, or one of several | an [`Owner`] that exists or does not |
//!
//! Both retain each build result beside its [`Owner`], so disposing structure is disposing
//! scopes, exactly as it is for values. Neither knows what a widget is: the widget layer
//! supplies the callbacks that turn a step into nodes, and it is the only layer that does.
//! `each`, `when` and `switch` are these two mechanisms bound to a view type, and that type
//! belongs to the widget layer.

#[cfg(test)]
mod tests;

use core::hash::Hash;

use rustc_hash::FxHashMap;

use crate::signal::Owner;

/// No slot: an arriving position before its row is built, and the end of a predecessor chain.
const NONE: u32 = u32::MAX;

/// A subtree keyed by which arm is showing.
///
/// Both conditional forms are this one mechanism, differing only in what they key on:
///
/// - a condition is `Branch<bool>`: [`set`](Self::set) with `Some(true)` builds, with `None`
///   tears down. Absence contributes nothing — no node, no layout participation, no
///   placeholder.
/// - navigation is `Branch<Route>`: the scope is dropped and rebuilt on a key change, so a
///   screen's state is gone once its arm is torn down. State that must outlive the arm lives in
///   a cell owned by a scope above the branch, which the call site decides by where it creates
///   the cell.
///
/// The arm's scope is detached from whatever scope is running the update, as a keyed list's
/// rows are: a branch driven from an effect would otherwise register every arm it ever built as
/// a child of that effect's scope, and that list would grow for the life of the screen.
pub struct Branch<K: PartialEq, V = ()> {
    /// The result before its scope: the declaration order is the disposal order, so an arm's
    /// nodes are released before the scope that owns what they hold.
    arm: Option<(K, V, Owner)>,
}

impl<K: PartialEq, V> Branch<K, V> {
    /// Creates a branch showing nothing.
    pub fn new() -> Self {
        Self { arm: None }
    }

    /// Returns the key of the showing arm.
    pub fn key(&self) -> Option<&K> {
        self.arm.as_ref().map(|(key, _, _)| key)
    }

    /// Returns whether an arm is showing.
    pub fn is_open(&self) -> bool {
        self.arm.is_some()
    }

    /// Retains the result of building inside a detached scope. A repeated key keeps both the
    /// result and its scope. A replacement drops the result, then its scope, before
    /// constructing the incoming arm.
    pub fn set(&mut self, key: Option<K>, build: impl FnOnce(&K) -> V) {
        if self.key() == key.as_ref() {
            return;
        }
        self.arm = None;
        let Some(key) = key else { return };
        let (owner, value) = Owner::detached(|| Owner::scope(|| build(&key)));
        self.arm = Some((key, value, owner));
    }

    /// Drops the retained result and its scope, showing nothing.
    pub fn close(&mut self) {
        self.arm = None;
    }
}

/// What reconciling decided about one position in the new list.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Step {
    /// A key that was not there before. Its view was already built, inside its own scope; what
    /// remains is to place it.
    Insert,
    /// A survivor whose position changed. Its view is **not** rebuilt — this is a placement
    /// change and nothing else.
    Move,
    /// A survivor already in the right place. Rebind its data; do nothing structural.
    Keep,
}

/// One row: its key, what its view built, and the scope that disposes it.
struct Row<K, V> {
    /// Cloned when the key arrives and never again: a survivor's key is not touched by a
    /// reconcile that keeps it.
    key: K,
    /// Before the owner: the retained result is released while the scope that owns what it
    /// holds is still alive.
    value: V,
    #[expect(dead_code, reason = "held for its Drop, which disposes the row's scope")]
    owner: Owner,
    /// The pass that last saw this key, so a survivor is told from a departure in one pass per
    /// side with no set to allocate.
    stamp: u32,
    /// This row's index in the current order, read as the *old* index by the next reconcile.
    position: u32,
}

/// A keyed list, and the [`Owner`] behind each of its rows.
///
/// Reconciling to a new item set proceeds in four steps, in this order:
///
/// 1. **destroys** removed keys, dropping their scopes;
/// 2. **creates** added keys, each in a fresh scope;
/// 3. **reorders** survivors with the minimum number of moves;
/// 4. **rebinds** every key's data — a survivor's view is not rebuilt.
///
/// Step 4 is what makes recycling free: a row scrolling out and another scrolling in is a move
/// plus a value change, not a destroy plus a create, and a filter keystroke that keeps a card
/// reorders it rather than rebuilding it.
///
/// A row lives in a slot it keeps for its whole life, and the order is a permutation of slot
/// indices, so a reorder moves `u32`s and the key index is written only when a key arrives or
/// leaves. A row's scope is detached from whatever scope is current when
/// [`reconcile`](Self::reconcile) runs: reconciling from inside an effect would otherwise
/// register every row it ever created as a child of the effect's own scope. A row belongs to
/// this list, and this list to its parent scope.
pub struct Keyed<K: Eq + Hash + Clone, V = ()> {
    rows: Vec<Option<Row<K, V>>>,
    free: Vec<u32>,
    at: FxHashMap<K, u32>,
    /// The current order, as slot indices.
    order: Vec<u32>,
    stamp: u32,
    scratch: Scratch,
}

/// One reconcile's working state, kept so a list reconciling repeatedly does not reallocate it.
#[derive(Default)]
struct Scratch {
    /// The order being built.
    next: Vec<u32>,
    /// Each survivor's position in the old order, in new order.
    from: Vec<u32>,
    /// Each survivor's position in the new order.
    seat: Vec<u32>,
    /// `tails[l]` indexes the smallest tail of an increasing subsequence of length `l + 1`.
    tails: Vec<u32>,
    /// `prev[i]` is the index preceding `i` in the best subsequence ending at `i`.
    prev: Vec<u32>,
    /// Which survivors the subsequence keeps, and so which need no move.
    keep: Vec<bool>,
}

impl<K: Eq + Hash + Clone, V> Keyed<K, V> {
    /// Creates an empty list.
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            free: Vec::new(),
            at: FxHashMap::default(),
            order: Vec::new(),
            stamp: 0,
            scratch: Scratch::default(),
        }
    }

    /// Reconciles to `next`.
    ///
    /// - `key` projects an item to its identity. It returns a borrow rather than a value, so a
    ///   key that is not `Copy` costs nothing to read twice and the identity may live inside
    ///   the item rather than beside it. Where the item is its own key, this is `|item| item`.
    /// - `build` is called for every arriving item, **inside that key's own scope**, so its
    ///   returned value and everything it creates are disposed when the key leaves.
    /// - `place` receives the retained value once per item, front to back, with its [`Step`].
    ///   Front to back is what makes the predecessor already correct when a step is applied.
    ///
    /// The keys of `next` must be unique; a repeat collapses two rows into one, and a debug
    /// build asserts it.
    ///
    /// Allocates nothing once the scratch has grown, so a list reconciling per keystroke costs
    /// the callbacks and the hash lookups and nothing else.
    pub fn reconcile<T>(
        &mut self,
        next: &[T],
        key: impl Fn(&T) -> &K,
        mut build: impl FnMut(&T) -> V,
        mut place: impl FnMut(&V, Step),
    ) {
        self.stamp = self.stamp.wrapping_add(1);
        let stamp = self.stamp;
        let mut order = core::mem::take(&mut self.scratch.next);
        order.clear();
        self.scratch.from.clear();
        self.scratch.seat.clear();

        // Stamp every key that survives, recording where it came from, and leave an empty slot
        // at every position whose key is new.
        for (position, item) in next.iter().enumerate() {
            let slot = match self.at.get(key(item)).copied() {
                Some(slot) => {
                    let row = self.rows[slot as usize]
                        .as_mut()
                        .expect("an indexed row is live");
                    debug_assert!(row.stamp != stamp, "the keys of `next` must be unique");
                    row.stamp = stamp;
                    self.scratch.from.push(row.position);
                    self.scratch.seat.push(position as u32);
                    slot
                }
                None => NONE,
            };
            order.push(slot);
        }

        // Sweep the old order for what was not stamped. Dropping the row drops its `Owner`,
        // which disposes everything the row's view created.
        while let Some(slot) = self.order.pop() {
            if self.rows[slot as usize]
                .as_ref()
                .is_some_and(|row| row.stamp != stamp)
            {
                let row = self.rows[slot as usize]
                    .take()
                    .expect("a listed row is live");
                self.at.remove(&row.key);
                self.free.push(slot);
            }
        }

        // Build what arrived, after the departures and before any placement.
        for (position, item) in next.iter().enumerate() {
            if order[position] != NONE {
                continue;
            }
            // Detached: this row belongs to the list, not to whatever scope is running the
            // reconcile.
            let (owner, value) = Owner::detached(|| Owner::scope(|| build(item)));
            let key = key(item).clone();
            let row = Row {
                key: key.clone(),
                value,
                owner,
                stamp,
                position: position as u32,
            };
            let slot = match self.free.pop() {
                Some(slot) => {
                    self.rows[slot as usize] = Some(row);
                    slot
                }
                None => {
                    self.rows.push(Some(row));
                    self.rows.len() as u32 - 1
                }
            };
            let duplicate = self.at.insert(key, slot).is_some();
            debug_assert!(!duplicate, "the keys of `next` must be unique");
            order[position] = slot;
        }

        lis(&mut self.scratch);

        // `seat` lists the survivors in new order, so a position that is not the next one in it
        // was built above. Reading survivorship from the scratch rather than from the row is
        // what keeps a freshly built row — which carries this stamp too — out of the
        // subsequence.
        let mut survivor = 0;
        for (position, &slot) in order.iter().enumerate() {
            let step = match self.scratch.seat.get(survivor) {
                Some(&at) if at as usize == position => {
                    survivor += 1;
                    if self.scratch.keep[survivor - 1] {
                        Step::Keep
                    } else {
                        Step::Move
                    }
                }
                _ => Step::Insert,
            };
            let row = self.rows[slot as usize]
                .as_mut()
                .expect("a placed row is live");
            row.position = position as u32;
            place(&row.value, step);
        }

        core::mem::swap(&mut self.order, &mut order);
        self.scratch.next = order;
    }
}

/// Marks a longest increasing subsequence of the survivors' old positions in `keep`, over
/// scratch the caller keeps.
///
/// Everything unmarked needs one move; everything marked is already in relative order and is
/// left alone. `O(n log n)`, by patience sorting with a predecessor chain. This is what makes
/// the move set minimal: a reconciler without it moves every element after the first change, so
/// a list that gained one row at the top moves every row.
fn lis(s: &mut Scratch) {
    let seq = &s.from;
    s.tails.clear();
    s.prev.clear();
    s.prev.resize(seq.len(), NONE);
    s.keep.clear();
    s.keep.resize(seq.len(), false);
    for (i, &value) in seq.iter().enumerate() {
        // The first tail not less than `value`. Strictly increasing, so a tie extends nothing
        // and two equal old positions are never both counted as stable.
        let at = s.tails.partition_point(|&t| seq[t as usize] < value);
        if at > 0 {
            s.prev[i] = s.tails[at - 1];
        }
        if at == s.tails.len() {
            s.tails.push(i as u32);
        } else {
            s.tails[at] = i as u32;
        }
    }
    let mut last = s.tails.last().copied();
    while let Some(i) = last {
        s.keep[i as usize] = true;
        last = (s.prev[i as usize] != NONE).then_some(s.prev[i as usize]);
    }
}

/// Returns the indices of a longest increasing subsequence of `seq`.
///
/// Allocates the returned `Vec`; [`Keyed::reconcile`] runs the same computation over pooled
/// scratch instead.
pub fn compute_lis(seq: &[usize]) -> Vec<usize> {
    let mut scratch = Scratch {
        from: seq.iter().map(|&i| i as u32).collect(),
        ..Scratch::default()
    };
    lis(&mut scratch);
    (0..seq.len()).filter(|&i| scratch.keep[i]).collect()
}
