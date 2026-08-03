//! The id authority: minting, generations, the free list. **App half.**
//!
//! One generic index serves every family in the crate, and the model owns every counter.
//! That is what makes id allocation a local operation rather than a handshake: because the
//! patch is a single in-order channel, a slot freed by a destroy can be reused by a create
//! *in the same patch* with no round trip, and the generation is what tells the front half
//! that the reuse was deliberate.

use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

/// A generational index into one family's arena.
///
/// Eight bytes, `Copy`, and phantom-typed so a `GeomId` cannot be handed to a table of
/// ramps. `fn() -> T` rather than `T` as the marker, so the id is `Send + Sync` whatever
/// `T` is — the families it names include front-thread objects, and the *id* crosses the
/// seam even though they never do.
pub struct Id<T> {
    idx: u32,
    generation: u32,
    _family: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// The id nothing was ever minted at. Every arena leaves slot zero unused so this is
    /// unambiguous, which is what lets a parentless node and an absent sibling both be
    /// `NONE` without an `Option` inside every link.
    pub const NONE: Self = Self::raw(0, 0);

    /// The id a fresh [`Ids`] mints first.
    ///
    /// What lets the two halves name a founding object — the root group — without
    /// exchanging its id: the model mints densely from one, so whatever it creates first
    /// *is* this, and the scene can seat its own under the same name. That is an invariant
    /// of the minting arithmetic and not a coincidence, which `a_fresh_authority_mints_the_
    /// first_id` is what holds.
    pub const FIRST: Self = Self::raw(1, 1);

    pub(crate) const fn raw(idx: u32, generation: u32) -> Self {
        Self {
            idx,
            generation,
            _family: PhantomData,
        }
    }

    /// Its dense slot. Every per-id table on both sides of the seam is a `Vec` indexed by
    /// this, which is why no hash map appears on any node path.
    #[must_use]
    pub const fn index(self) -> usize {
        self.idx as usize
    }

    /// Which occupant of that slot it names.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Whether it names nothing.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.idx == 0 && self.generation == 0
    }

    /// Re-tags an id as naming another family, for the one place that is meaningful: a
    /// resource id that has already been matched to its table by the op carrying it.
    pub(crate) const fn cast<U>(self) -> Id<U> {
        Id::raw(self.idx, self.generation)
    }
}

// Derived impls would demand the same bound of `T`, which is wrong here: an id is plain
// data about a slot and says nothing about what lives in it.
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
// Ordered so a per-id array can be sorted and binary-searched — which is how the hit table
// answers "the rect this control was laid out at" without a hash. The order is the slot's,
// then its occupant's; it carries no meaning beyond being total and stable.
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

/// One family's minting authority: a generation per slot, and the slots free to reuse.
///
/// It holds no payload. What lives in a slot is the caller's business and differs on the
/// two sides of the seam — the model keeps a style, the scene keeps a visual — so what is
/// shared is only the arithmetic that keeps the two agreeing about *which* slot.
///
/// **Liveness is the generation's parity**, and that is what makes the counter monotonic:
/// minting makes it odd, releasing makes it even, and a released slot's next occupant is
/// therefore two apart from the last rather than back where it started. A separate live
/// flag is a second fact that can disagree with the first, and resetting the generation on
/// release hands the *same* id out twice.
///
/// Phantom-typed for the reason [`Id`] is, and it is the half that matters: an authority
/// and the [`Slots`] it mints into are two objects that have to name one family, and
/// nothing else says they do. Untyped, a store could be filled from the wrong counter and
/// the only symptom would be two live rows sharing an index.
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
    /// An authority that has minted nothing.
    #[must_use]
    pub fn new() -> Self {
        // Slot zero is burnt so that `Id::NONE` can never collide with a live id.
        Self {
            generations: vec![0],
            free: Vec::new(),
            _family: PhantomData,
        }
    }

    /// Mints an id, reusing a freed slot in preference to growing.
    pub fn mint(&mut self) -> Id<F> {
        if let Some(idx) = self.free.pop() {
            let generation = &mut self.generations[idx as usize];
            // Even (dead) to odd (live). Saturating rather than wrapping: a slot recycled
            // two billion times would otherwise hand back an id someone may still hold,
            // and pinning it live is the safe end of that.
            *generation = generation.saturating_add(1) | 1;
            return Id::raw(idx, *generation);
        }
        let idx = u32::try_from(self.generations.len()).expect("more than 4 billion live ids");
        self.generations.push(1);
        Id::raw(idx, 1)
    }

    /// Returns a slot to the free list. A second free of the same id is refused rather
    /// than double-listing the slot, which would hand the same index to two live ids.
    pub fn release(&mut self, id: Id<F>) -> bool {
        if !self.is_live(id) {
            return false;
        }
        // Odd (live) to even (dead).
        self.generations[id.index()] = self.generations[id.index()].saturating_add(1) & !1;
        self.free.push(id.idx);
        true
    }

    /// Whether `id` still names its slot's current occupant.
    #[must_use]
    pub fn is_live(&self, id: Id<F>) -> bool {
        !id.is_none()
            && id.generation % 2 == 1
            && self
                .generations
                .get(id.index())
                .is_some_and(|&generation| generation == id.generation)
    }

    /// How many slots exist to be occupied, held and released alike.
    ///
    /// Slot zero is burnt so that [`Id::NONE`] names nothing, so it is not one of them — and
    /// counting it would make "one row occupies one slot" read as two.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.generations.len() - 1
    }

    /// How many ids are live.
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

/// The payload half of [`Ids`]: one `Vec`, indexed by [`Id::index`], validated by the id it
/// was written for.
///
/// `None` **is** vacancy, and it is the only fact about it. A slot that stores its id
/// separately from its payload is two facts that have to agree, which is the arrangement
/// [`Ids`] refuses to keep about liveness — and this is the same slot it is refusing it
/// about.
///
/// Comparing against the stored id *is* the generation check, since [`Id`]'s equality is
/// over both halves. That is what lets one store serve a family the caller mints and one it
/// merely indexes into: a table keyed by someone else's ids validates against their
/// generations without ever holding their [`Ids`].
///
/// Auto-traits are **derived and never implemented**. `Id` is unconditionally `Send + Sync`,
/// so the bound falls entirely on `T` — a store of app-thread handlers is therefore not
/// `Send`, which is part of what makes a patch's `Send` a proof rather than a claim. An
/// `unsafe impl` here "because it is only a `Vec`" would quietly delete that.
/// `T` defaults to `F`, because a family usually holds the thing it is named after — a
/// `Slots<Node>` keeps nodes. Where the two differ it is because one family has two stores,
/// which is the ordinary shape either side of a thread seam: `Slots<Control, Control>` here
/// and `Slots<Control, ChromeRow>` there, over one set of ids.
pub struct Slots<F, T = F> {
    rows: Vec<Option<(Id<F>, T)>>,
}

impl<F, T> Default for Slots<F, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F, T> Slots<F, T> {
    /// A store holding nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Writes `value` at `id`, growing to reach it.
    ///
    /// The **one** indexing operation in this type, and it comes immediately after the
    /// resize that makes it valid. Everything else answers `None` rather than panicking,
    /// whatever id it is handed.
    ///
    /// It returns nothing on purpose. Handing back the displaced payload offers a caller a
    /// value it can drop on the floor — and for a row that owns event handlers, dropping it
    /// on the floor is exactly the leak the release discipline exists to prevent. A caller
    /// that wants what was there asks for it with [`take`](Self::take) first.
    pub fn place(&mut self, id: Id<F>, value: T) {
        debug_assert!(
            !id.is_none(),
            "slot zero is burnt so that NONE names nothing"
        );
        // A place under a generation older than the slot's occupant installs a row nobody
        // holds an id for and drops a live one on the way. Monotonic rather than equal:
        // a *newer* occupant is ordinary reuse, and a consumer that has not yet been told
        // of the release will legitimately adopt one.
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

    /// Mints an id from `ids` and places `value` under it.
    ///
    /// The authority is passed rather than owned, because a store keyed by ids somebody else
    /// mints must not have one at all — a table indexed by the model's nodes owns no counter,
    /// and one it could reach for is one it could mint from. Taking it here is what keeps
    /// that difference visible in the declaration: a field of `Ids<F>` beside a store means
    /// this layer mints the family, and its absence means it does not.
    ///
    /// Minting and placing in one call is what makes "minted but never placed" unsayable.
    /// Apart, that leaves a slot the authority believes is live with no row in it, which is
    /// never reused and has no symptom.
    pub fn insert(&mut self, ids: &mut Ids<F>, value: T) -> Id<F> {
        let id = ids.mint();
        self.place(id, value);
        id
    }

    /// Takes the row `id` names and releases its slot. Both halves, or neither.
    pub fn remove(&mut self, ids: &mut Ids<F>, id: Id<F>) -> Option<T> {
        let value = self.take(id)?;
        // Only once the row is out. Releasing a slot whose payload is still in it would let
        // the next mint hand that index to a second live id.
        ids.release(id);
        Some(value)
    }

    /// The payload `id` names, or `None` where the slot has been released or reused since.
    #[must_use]
    pub fn get(&self, id: Id<F>) -> Option<&T> {
        // `NONE` is the terminator every link in the layer above uses, so it is read far
        // more often than it is written. Answering it here rather than relying on slot zero
        // being burnt is what makes a chain walk safe by construction: a release build with
        // the `place` assertion compiled out cannot turn a terminator into a payload.
        if id.is_none() {
            return None;
        }
        match self.rows.get(id.index())? {
            Some((held, value)) if *held == id => Some(value),
            _ => None,
        }
    }

    /// The same, mutably.
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

    /// Vacates the slot and hands the row back.
    ///
    /// The caller drops it, which releases everything it owned; a row that needs more than
    /// dropping — a compositor object, a foreign handle — does it with the row in hand.
    /// There is deliberately no second removal that keeps the payload: a caller wanting to
    /// reuse an expensive body parks it in a pool of its own, which is what the build arena
    /// already does with its buffers.
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

    /// Every position the store has ever grown to.
    ///
    /// A concrete `Range` and not an iterator, so a walk can reach back into whatever
    /// encloses the store while it runs — which is the whole reason a pass over the table
    /// is written by position rather than by iteration.
    #[must_use]
    pub fn positions(&self) -> core::ops::Range<u32> {
        0..self.rows.len() as u32
    }

    /// The id living at a bare position, if one does.
    ///
    /// It yields an **id and never a payload**, so a walk by position still reaches its rows
    /// through the checked path. That is the difference between a cursor and a hole in the
    /// staleness rule.
    #[must_use]
    pub fn id_at(&self, at: u32) -> Option<Id<F>> {
        self.rows.get(at as usize)?.as_ref().map(|(id, _)| *id)
    }

    /// Every row held, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<F>, &T)> {
        self.rows
            .iter()
            .filter_map(|slot| slot.as_ref().map(|(id, value)| (*id, value)))
    }

    /// Every row held, mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<F>, &mut T)> {
        self.rows
            .iter_mut()
            .filter_map(|slot| slot.as_mut().map(|(id, value)| (*id, value)))
    }

    /// How many rows are held. Not the slot count: a released slot is not a row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.iter().filter(|slot| slot.is_some()).count()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(Option::is_none)
    }

    /// How many slots exist, held and released alike. What a reuse claim is measured
    /// against.
    ///
    /// Slot zero is never occupied — it is what makes [`Id::NONE`] name nothing — so it is
    /// not counted, and the answer agrees with [`Ids::slots`] over the same family.
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
        // Both halves seat their root at `FIRST` with no handshake, so this is the
        // invariant that keeps them naming the same node.
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
