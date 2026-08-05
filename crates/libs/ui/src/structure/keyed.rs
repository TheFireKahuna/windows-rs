//! Keyed reconciliation: the minimal set of moves, and one scope per surviving key.

use crate::signal::Owner;
use core::hash::Hash;
use rustc_hash::FxHashMap;

/// The indices of a longest increasing subsequence of `seq`.
///
/// Everything **not** in the result needs one move; everything in it is already in relative
/// order and must not be touched. `O(n log n)`, by patience sorting with a predecessor
/// chain.
///
/// This is the whole of "minimal moves", and it is why it is worth having: a reconciler
/// without it moves every element after the first change, so a list that gained one row at
/// the top moves every row.
#[must_use]
pub fn compute_lis(seq: &[usize]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut work = Lis::default();
    work.compute(seq, &mut out);
    out
}

/// [`compute_lis`]'s working state, kept so a list reconciling repeatedly does not
/// reallocate it.
#[derive(Default)]
struct Lis {
    /// `tails[l]` indexes the smallest tail of an increasing subsequence of length `l + 1`.
    tails: Vec<usize>,
    /// `prev[i]` is the index preceding `i` in the best subsequence ending at `i`.
    prev: Vec<usize>,
}

const NONE: usize = usize::MAX;

impl Lis {
    fn compute(&mut self, seq: &[usize], out: &mut Vec<usize>) {
        out.clear();
        if seq.is_empty() {
            return;
        }
        self.tails.clear();
        self.prev.clear();
        self.prev.resize(seq.len(), NONE);

        for (i, &value) in seq.iter().enumerate() {
            // The first tail not less than `value`. Strictly increasing, so a tie extends
            // nothing — which is what keeps equal old-positions from being called stable.
            let at = self.tails.partition_point(|&t| seq[t] < value);
            if at > 0 {
                self.prev[i] = self.tails[at - 1];
            }
            if at == self.tails.len() {
                self.tails.push(i);
            } else {
                self.tails[at] = i;
            }
        }

        let mut at = *self.tails.last().expect("a non-empty sequence has a tail");
        loop {
            out.push(at);
            if self.prev[at] == NONE {
                break;
            }
            at = self.prev[at];
        }
        out.reverse();
    }
}

/// What reconciling decided about one position in the new list.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Step {
    /// A key that was not there before. Its view was already built, inside its own scope;
    /// what remains is to place it.
    Insert,
    /// A survivor whose position changed. Its view is **not** rebuilt — this is a
    /// placement change and nothing else.
    Move,
    /// A survivor already in the right place. Rebind its data; do nothing structural.
    Keep,
}

/// A keyed list, and the [`Owner`] behind each of its rows.
///
/// Reconciling to a new item set does four things, in this order:
///
/// 1. **destroys** removed keys, dropping their scopes;
/// 2. **creates** added keys, each in a fresh scope;
/// 3. **reorders** survivors with the minimum number of moves, via [`compute_lis`];
/// 4. **rebinds** every key's data — a survivor's view is not rebuilt.
///
/// Step 4 is what makes recycling free: a row scrolling out and another scrolling in is a
/// move plus a value change, not a destroy plus a create. It is also why a filter keystroke
/// that keeps a card reorders it rather than rebuilding it.
///
/// A row's scope is **detached** from whatever scope is current when `reconcile` runs. It
/// has to be: reconciling from inside an effect would otherwise register every row it ever
/// created as a child of the effect's own scope, and that list would grow for the life of
/// the screen. The rows are this list's, and the list is its parent's.
pub struct Keyed<K: Eq + Hash + Clone> {
    /// The current order.
    keys: Vec<K>,
    rows: FxHashMap<K, Row>,
    /// Bumped per reconcile, and stamped on every key present in the new set. What
    /// separates a survivor from a departure in one pass per side, with no set to build.
    epoch: u32,
    scratch: Scratch,
}

struct Row {
    /// Disposes everything this row's view created. Dropping the row is the unmount.
    _owner: Owner,
    /// This row's index in `keys`.
    at: usize,
    epoch: u32,
}

#[derive(Default)]
struct Scratch {
    /// For each survivor, in **new** order, its index in the old order.
    previous: Vec<usize>,
    /// For each survivor, in new order, its index in the new order.
    positions: Vec<usize>,
    /// Which entries of `previous` the subsequence keeps.
    keep: Vec<usize>,
    lis: Lis,
}

impl<K: Eq + Hash + Clone> Default for Keyed<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash + Clone> Keyed<K> {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            rows: FxHashMap::default(),
            epoch: 0,
            scratch: Scratch::default(),
        }
    }

    /// The current order.
    #[must_use]
    pub fn keys(&self) -> &[K] {
        &self.keys
    }

    /// How many rows are live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Drops every row's scope, last first. What unmounting a list does.
    pub fn clear(&mut self, mut remove: impl FnMut(&K)) {
        while let Some(key) = self.keys.pop() {
            remove(&key);
            self.rows.remove(&key);
        }
    }

    /// Reconciles to `next`.
    ///
    /// - `key` projects an item to its identity. A **borrow** and not a value, so a key that
    ///   is not `Copy` costs nothing to read four times, and so the identity may live inside
    ///   the item rather than beside it. Where the item *is* its own key, this is `|item|
    ///   item`.
    /// - `remove` is called for every departing key, before anything is built.
    /// - `build` is called for every arriving key, **inside that key's own scope**, so
    ///   everything it creates is disposed when the key later leaves.
    /// - `place` is called once per key in `next`, front to back, with that key's [`Step`]
    ///   and the index in `next` of the key it follows — `None` at the head. Front to back
    ///   is what makes the predecessor already correct when a step is applied.
    ///
    /// Allocates nothing once the scratch has grown, so a list reconciling per keystroke
    /// costs the callbacks and the hash lookups and nothing else.
    pub fn reconcile<T>(
        &mut self,
        next: &[T],
        key: impl Fn(&T) -> &K,
        mut remove: impl FnMut(&K),
        mut build: impl FnMut(&K, &T),
        mut place: impl FnMut(&K, &T, Step, Option<usize>),
    ) {
        let Self {
            keys,
            rows,
            epoch,
            scratch,
        } = self;
        *epoch = epoch.wrapping_add(1);
        let epoch = *epoch;

        // ── 1. destroy ──────────────────────────────────────────────────────────
        // Stamp every key that survives, then sweep the old order for what was not
        // stamped. One pass per side, and no set to allocate.
        for item in next {
            if let Some(row) = rows.get_mut(key(item)) {
                row.epoch = epoch;
            }
        }
        for key in keys.drain(..) {
            if rows.get(&key).is_some_and(|row| row.epoch != epoch) {
                remove(&key);
                // Dropping the row drops its `Owner`, which disposes everything the row's
                // view created, in reverse creation order.
                rows.remove(&key);
            }
        }

        // ── 2. create, and record where each survivor came from ─────────────────
        scratch.previous.clear();
        scratch.positions.clear();
        for (position, item) in next.iter().enumerate() {
            let key = key(item);
            if let Some(row) = rows.get(key) {
                scratch.previous.push(row.at);
                scratch.positions.push(position);
            } else {
                // Detached: this row belongs to the list, not to whatever scope is running
                // the reconcile. See the type's own note.
                let (owner, ()) = Owner::detached(|| Owner::scope(|| build(key, item)));
                rows.insert(
                    key.clone(),
                    Row {
                        _owner: owner,
                        at: position,
                        epoch,
                    },
                );
            }
        }

        debug_assert_eq!(
            rows.len(),
            next.len(),
            "a keyed list's keys must be unique: a repeat collapses two rows into one"
        );

        // ── 3. the minimal move set ─────────────────────────────────────────────
        // The subsequence indexes `previous`, which is in new order, so the walk below
        // consumes it in order alongside `positions`.
        scratch.lis.compute(&scratch.previous, &mut scratch.keep);

        // ── 4. place, front to back ─────────────────────────────────────────────
        let mut keep = scratch.keep.iter().copied().peekable();
        let mut survivor = 0;
        for (position, item) in next.iter().enumerate() {
            let key = key(item);
            let after = position.checked_sub(1);
            // `positions` lists the survivors in new order, so a position that is not the
            // next one in it was built in step 2. Reading survivorship from the scratch
            // rather than from the row is what keeps a freshly built row — which is also
            // stamped with this epoch — out of the subsequence.
            let step = if scratch.positions.get(survivor) == Some(&position) {
                let stable = keep.peek() == Some(&survivor);
                if stable {
                    keep.next();
                }
                survivor += 1;
                if stable { Step::Keep } else { Step::Move }
            } else {
                Step::Insert
            };
            if let Some(row) = rows.get_mut(key) {
                row.at = position;
            }
            place(key, item, step, after);
        }

        keys.extend(next.iter().map(|item| key(item).clone()));
    }
}
