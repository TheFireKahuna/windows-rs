//! Mints generational ids: the index type, the per-slot generations, the free list, and the
//! payload store keyed by them. **App half.**
//!
//! One generic index serves every family in the crate and the model owns every counter, so
//! minting is a local operation rather than a handshake. The patch is a single in-order
//! channel, so a slot freed by a destroy is reused by a create *in the same patch* with no
//! round trip, and the generation tells the front half that the reuse was deliberate.

use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

/// Indexes a slot in one family's arena, tagged with the generation naming its occupant.
///
/// Eight bytes, `Copy`, and phantom-typed so a `GeomId` cannot be handed to a table of
/// ramps. The marker is `fn() -> T` rather than `T`, so the id is `Send + Sync` whatever
/// `T` is: the families it names include front-thread objects, and the *id* crosses the
/// seam even though they never do.
pub struct Id<T> {
    idx: u32,
    generation: u32,
    _family: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// The id nothing is ever minted at. Every arena leaves slot zero unoccupied, so a
    /// parentless node and an absent sibling are both `NONE` and no link needs an `Option`
    /// wrapped around it.
    pub const NONE: Self = Self::raw(0, 0);

    /// The id a fresh [`Ids`] mints first.
    ///
    /// [`Ids::mint`] allocates densely from slot one, so whatever a fresh authority creates
    /// first *is* this id. Both halves seat their root group here without exchanging it.
    pub const FIRST: Self = Self::raw(1, 1);

    /// Builds an id from a slot and a generation, consulting no authority.
    pub(crate) const fn raw(idx: u32, generation: u32) -> Self {
        Self {
            idx,
            generation,
            _family: PhantomData,
        }
    }

    /// Returns the dense slot the id names. Every per-id table on both sides of the seam is
    /// a `Vec` indexed by it, so no node path reaches a hash map.
    #[must_use]
    pub const fn index(self) -> usize {
        self.idx as usize
    }

    /// Returns which occupant of that slot the id names.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Returns `true` where the id names nothing.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.idx == 0 && self.generation == 0
    }

    /// Re-tags an id as naming another family. Meaningful only for a resource id the op
    /// carrying it has already matched to its table.
    pub(crate) const fn cast<U>(self) -> Id<U> {
        Id::raw(self.idx, self.generation)
    }
}

// Hand-written rather than derived: a derive would demand the same bound of `T`, and an id
// is plain data about a slot that says nothing about what occupies it.
impl<T> Copy for Id<T> {}
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx && self.generation == other.generation
    }
}
impl<T> Eq for Id<T> {}
// Ordered so a per-id array can be sorted and binary-searched, which is how the hit table
// answers with a control's own rect without a hash. The order is the slot's, then its
// occupant's, and carries no meaning beyond being total and stable.
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (self.idx, self.generation).cmp(&(other.idx, other.generation))
    }
}
impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.idx.hash(state);
        self.generation.hash(state);
    }
}
impl<T> Default for Id<T> {
    fn default() -> Self {
        Self::NONE
    }
}
impl<T> core::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "#{}.{}", self.idx, self.generation)
    }
}

/// Mints one family's ids: a generation per slot, and the slots free to reuse.
///
/// Holds no payload. What occupies a slot differs on the two sides of the seam — the model
/// keeps a style, the scene keeps a visual — so only the arithmetic that keeps the two
/// agreeing about *which* slot is shared.
///
/// Liveness is the generation's parity: minting makes it odd, releasing makes it even, so
/// the counter only rises and a released slot's next occupant is two generations on rather
/// than back where the last one started. A separate live flag would be a second fact that
/// can disagree with the parity, and resetting the generation on release would hand the
/// *same* id out twice.
///
/// Phantom-typed for the reason [`Id`] is: an authority and the [`Slots`] it mints into
/// must name one family, and nothing else in their types says they do. Untyped, a store
/// filled from the wrong counter shows up only as two live rows sharing an index.
pub struct Ids<F> {
    generations: Vec<u32>,
    free: Vec<u32>,
    _family: PhantomData<fn() -> F>,
}

impl<F> Default for Ids<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F> Ids<F> {
    /// Returns an authority that has minted nothing.
    #[must_use]
    pub fn new() -> Self {
        // Slot zero is left occupied by nothing, so `Id::NONE` can never collide with a
        // live id.
        Self {
            generations: vec![0],
            free: Vec::new(),
            _family: PhantomData,
        }
    }

    /// Mints an id, reusing a freed slot in preference to growing.
    ///
    /// # Panics
    ///
    /// Panics where the family has grown past `u32::MAX` slots.
    pub fn mint(&mut self) -> Id<F> {
        if let Some(idx) = self.free.pop() {
            let generation = &mut self.generations[idx as usize];
            // Even (dead) to odd (live). Saturating rather than wrapping: a slot recycled
            // two billion times would otherwise hand back an id someone may still hold, and
            // pinning the generation live is the safe end of that.
            *generation = generation.saturating_add(1) | 1;
            return Id::raw(idx, *generation);
        }
        let idx = u32::try_from(self.generations.len()).expect("more than 4 billion live ids");
        self.generations.push(1);
        Id::raw(idx, 1)
    }

    /// Returns `id`'s slot to the free list, and `true` where it did. A second release of
    /// the same id is refused rather than double-listing the slot, which would hand one
    /// index to two live ids.
    pub fn release(&mut self, id: Id<F>) -> bool {
        if !self.is_live(id) {
            return false;
        }
        // Odd (live) to even (dead).
        self.generations[id.index()] = self.generations[id.index()].saturating_add(1) & !1;
        self.free.push(id.idx);
        true
    }

    /// Returns `true` where `id` still names its slot's current occupant.
    #[must_use]
    pub fn is_live(&self, id: Id<F>) -> bool {
        !id.is_none()
            && id.generation % 2 == 1
            && self
                .generations
                .get(id.index())
                .is_some_and(|&generation| generation == id.generation)
    }

    /// Returns how many slots exist to be occupied, held and released alike.
    ///
    /// Slot zero is left unoccupied so that [`Id::NONE`] names nothing, and it is not
    /// counted here.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.generations.len() - 1
    }

    /// Returns how many ids are live.
    #[must_use]
    pub fn live(&self) -> usize {
        self.slots() - self.free.len()
    }
}

impl<F> core::fmt::Debug for Ids<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ids")
            .field("live", &self.live())
            .field("slots", &self.slots())
            .finish()
    }
}

/// Stores the payload half of [`Ids`]: one `Vec`, indexed by [`Id::index`], each row
/// holding the id it was written for.
///
/// `None` is vacancy and the only record of it. Storing an id apart from its payload would
/// be two facts that have to agree, which is what [`Ids`] avoids about liveness by carrying
/// it in the generation's parity.
///
/// Comparing against the stored id *is* the generation check, since [`Id`] equality covers
/// both halves. One store therefore serves a family the caller mints and one it merely
/// indexes into: a table keyed by someone else's ids validates against their generations
/// without ever holding their [`Ids`].
///
/// Auto-traits are derived and never implemented here. [`Id`] is unconditionally
/// `Send + Sync`, so the bound falls entirely on `T`, and a store of app-thread handlers is
/// therefore not `Send` — which is what a patch's `Send` bound rests on. An `unsafe impl`
/// would remove that check.
///
/// `T` defaults to `F`, since a family usually holds what it is named after: a
/// `Slots<Node>` keeps nodes. The two differ where one family has two stores either side of
/// a thread seam — `Slots<Control, Control>` here and `Slots<Control, ChromeRow>` there,
/// over one set of ids.
pub struct Slots<F, T = F> {
    rows: Vec<Option<(Id<F>, T)>>,
}

impl<F, T> Default for Slots<F, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F, T> Slots<F, T> {
    /// Returns a store holding nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Writes `value` at `id`, growing the store to reach it.
    ///
    /// The one indexing operation in this type, and it follows the resize that makes it
    /// valid. Every other method answers `None` rather than panicking, whatever id it is
    /// handed.
    ///
    /// Returns nothing, and any payload already in the slot is dropped in place. A caller
    /// that wants what was there asks for it with [`take`](Self::take) first; a row owning
    /// event handlers leaks if it is handed back and discarded.
    pub fn place(&mut self, id: Id<F>, value: T) {
        debug_assert!(
            !id.is_none(),
            "slot zero is burnt so that NONE names nothing"
        );
        // A place under a generation older than the slot's occupant installs a row nobody
        // holds an id for and drops a live one on the way. Monotonic rather than equal: a
        // *newer* occupant is ordinary reuse, and a consumer not yet told of the release
        // legitimately adopts one.
        debug_assert!(
            self.rows
                .get(id.index())
                .and_then(Option::as_ref)
                .is_none_or(|(held, _)| held.generation() <= id.generation()),
            "a stale id overwrote a live row"
        );
        let at = id.index();
        if at >= self.rows.len() {
            self.rows.resize_with(at + 1, || None);
        }
        self.rows[at] = Some((id, value));
    }

    /// Mints an id from `ids`, places `value` under it, and returns the id.
    ///
    /// The authority is a parameter rather than a field, so a store keyed by ids another
    /// layer mints holds no counter it could mint from. A field of `Ids<F>` beside a store
    /// means that layer mints the family; its absence means it does not.
    ///
    /// Minting and placing in one call leaves no id minted but never placed, which would be
    /// a slot the authority holds live with no row in it and nothing to reuse it.
    pub fn insert(&mut self, ids: &mut Ids<F>, value: T) -> Id<F> {
        let id = ids.mint();
        self.place(id, value);
        id
    }

    /// Takes the row `id` names and releases its slot. Both happen, or neither does.
    pub fn remove(&mut self, ids: &mut Ids<F>, id: Id<F>) -> Option<T> {
        let value = self.take(id)?;
        // Only once the row is out. Releasing a slot whose payload is still in it would let
        // the next mint hand that index to a second live id.
        ids.release(id);
        Some(value)
    }

    /// Returns the payload `id` names, or `None` where the slot has been vacated or reused
    /// since.
    #[must_use]
    pub fn get(&self, id: Id<F>) -> Option<&T> {
        // `NONE` terminates every link in the layer above, so a chain walk reaches this far
        // constantly. Answering it here rather than relying on slot zero staying unoccupied
        // means a release build, with `place`'s assertion compiled out, still cannot turn a
        // terminator into a payload.
        if id.is_none() {
            return None;
        }
        match self.rows.get(id.index())? {
            Some((held, value)) if *held == id => Some(value),
            _ => None,
        }
    }

    /// Returns the payload `id` names, mutably.
    #[must_use]
    pub fn get_mut(&mut self, id: Id<F>) -> Option<&mut T> {
        if id.is_none() {
            return None;
        }
        match self.rows.get_mut(id.index())? {
            Some((held, value)) if *held == id => Some(value),
            _ => None,
        }
    }

    /// Vacates the slot and returns the row, leaving the slot itself unreleased.
    ///
    /// The caller drops the row, which releases everything it owned; a row needing more than
    /// a drop — a compositor object, a foreign handle — is torn down with the row in hand.
    /// No variant keeps the payload in place: a caller reusing an expensive body pools it
    /// itself, as the build arena does with its buffers.
    pub fn take(&mut self, id: Id<F>) -> Option<T> {
        if id.is_none() {
            return None;
        }
        let slot = self.rows.get_mut(id.index())?;
        match slot {
            Some((held, _)) if *held == id => slot.take().map(|(_, value)| value),
            _ => None,
        }
    }

    /// Returns every position the store has grown to.
    ///
    /// A concrete `Range` and not an iterator over the store, so a walk holds no borrow on
    /// it and can reach back into whatever encloses the store while it runs.
    #[must_use]
    pub fn positions(&self) -> core::ops::Range<u32> {
        0..self.rows.len() as u32
    }

    /// Returns the id occupying position `at`, or `None` where nothing does.
    ///
    /// Yields an id and never a payload, so a walk by position still reaches its rows
    /// through [`get`](Self::get) and its generation check.
    #[must_use]
    pub fn id_at(&self, at: u32) -> Option<Id<F>> {
        self.rows.get(at as usize)?.as_ref().map(|(id, _)| *id)
    }

    /// Returns every row held, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<F>, &T)> {
        self.rows
            .iter()
            .filter_map(|slot| slot.as_ref().map(|(id, value)| (*id, value)))
    }

    /// Returns every row held, mutably, in slot order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<F>, &mut T)> {
        self.rows
            .iter_mut()
            .filter_map(|slot| slot.as_mut().map(|(id, value)| (*id, value)))
    }

    /// Returns how many rows are held, counting them on every call. Not the slot count: a
    /// vacated slot is not a row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.iter().filter(|slot| slot.is_some()).count()
    }

    /// Returns `true` where no row is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(Option::is_none)
    }

    /// Returns how many slots exist, held and vacated alike. What a reuse rate is measured
    /// against.
    ///
    /// Slot zero is never occupied, which is what makes [`Id::NONE`] name nothing, so it is
    /// not counted and the answer agrees with [`Ids::slots`] over the same family.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.rows.len().saturating_sub(1)
    }
}

impl<F, T: core::fmt::Debug> core::fmt::Debug for Slots<F, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Thing;

    #[test]
    fn a_fresh_authority_mints_the_first_id() {
        // Both halves seat their root at `FIRST` without exchanging it, so minting has to
        // start there for them to name the same node.
        let mut ids = Ids::new();
        let first: Id<Thing> = ids.mint();
        assert_eq!(first, Id::FIRST);
    }

    #[test]
    fn no_live_id_is_ever_none() {
        let mut ids = Ids::new();
        for _ in 0..8 {
            let id: Id<Thing> = ids.mint();
            assert!(!id.is_none());
            assert!(ids.is_live(id));
        }
        assert!(!ids.is_live(Id::<Thing>::NONE));
    }

    #[test]
    fn a_reused_slot_is_a_different_id() {
        let mut ids = Ids::new();
        let first: Id<Thing> = ids.mint();
        assert!(ids.release(first));
        let second: Id<Thing> = ids.mint();
        assert_eq!(first.index(), second.index(), "the slot should be reused");
        assert_ne!(first, second, "but not under the same id");
        assert!(!ids.is_live(first));
        assert!(ids.is_live(second));
    }

    #[test]
    fn a_double_release_does_not_hand_one_slot_to_two_ids() {
        let mut ids = Ids::new();
        let id: Id<Thing> = ids.mint();
        assert!(ids.release(id));
        assert!(!ids.release(id));
        let a: Id<Thing> = ids.mint();
        let b: Id<Thing> = ids.mint();
        assert_ne!(a.index(), b.index());
    }
}
