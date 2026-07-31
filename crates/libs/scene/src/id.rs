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
#[derive(Debug)]
pub struct Ids {
    generations: Vec<u32>,
    free: Vec<u32>,
}

impl Default for Ids {
    fn default() -> Self {
        Self::new()
    }
}

impl Ids {
    /// An authority that has minted nothing.
    #[must_use]
    pub fn new() -> Self {
        // Slot zero is burnt so that `Id::NONE` can never collide with a live id.
        Self {
            generations: vec![0],
            free: Vec::new(),
        }
    }

    /// Mints an id, reusing a freed slot in preference to growing.
    pub fn mint<T>(&mut self) -> Id<T> {
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
    pub fn release<T>(&mut self, id: Id<T>) -> bool {
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
    pub fn is_live<T>(&self, id: Id<T>) -> bool {
        !id.is_none()
            && id.generation % 2 == 1
            && self
                .generations
                .get(id.index())
                .is_some_and(|&generation| generation == id.generation)
    }

    /// How many slots have ever existed, which is the length every per-id table grows to.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.generations.len()
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
