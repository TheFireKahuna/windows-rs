//! Recogniser lifetime: a pool.
//!
//! A recogniser is **bound to a `(contact, target)` pair on down**, configured from that
//! target's declaration, and returned to the pool on up, cancel or inertia end. Neither a
//! recogniser per element nor one constructed per gesture is used: the first does not scale
//! past a few dozen targets, and the second churns.
//!
//! Two free lists, not one: a precision-touchpad contact needs the physical recogniser and
//! the two are different types, so one mixed list would hand out the wrong kind.

use super::decl::GestureDecl;
use super::drag::Drag;
use super::recognizer::{Events, Recognizer};
use crate::FrontHandle;
use crate::input::PointerType;
use rustc_hash::FxHashMap;
use windows_core::Result;
use windows_scene::{ControlId, Point};

/// What one contact is doing, and to what.
pub struct Bound {
    pub target: ControlId,
    pub decl: GestureDecl,
    /// The contact's down point, in client DIPs, **raw** — a press target is a discrete
    /// decision and an extrapolated origin makes every later delta wrong by the same amount.
    pub origin: Point,
    /// The two-axis policy, where the target declared one.
    pub drag: Option<Drag>,
    /// Whether a manipulation has actually begun, so a contact that never passed the
    /// recogniser's own threshold is not reported as one that did.
    pub manipulating: bool,
    /// Whether the contact has lifted and its motion is still being pumped. Written only by
    /// [`RecognizerPool::set_inertial`], which is what keeps the pool's inertial index exact.
    inertial: bool,
    /// Whether the contact arrived without the digitizer's confidence. Such a contact
    /// **never starts a gesture** — nothing is fed to its recogniser — and that is the whole
    /// of palm rejection on this stack.
    pub rejected: bool,
    recognizer: FrontHandle<Recognizer>,
}

impl Bound {
    /// Returns the recogniser this contact is bound to.
    #[must_use]
    pub fn recognizer(&self) -> &Recognizer {
        &self.recognizer
    }

    /// Returns whether the contact has lifted and its motion is still being pumped.
    #[must_use]
    pub const fn is_inertial(&self) -> bool {
        self.inertial
    }
}

/// The recognisers, bound and free.
pub struct RecognizerPool {
    free: Vec<FrontHandle<Recognizer>>,
    ptp_free: Vec<FrontHandle<Recognizer>>,
    /// Keyed by the system's pointer id — the one structure in this crate not keyed by a
    /// dense index this crate minted.
    bound: FxHashMap<u32, Bound>,
    /// The ids whose binding is inertial. Exact at every mutation of that flag, so the tick
    /// that pumps inertia neither scans `bound` nor allocates.
    inertial: Vec<u32>,
    /// Ids held across a pass that releases bindings, so the borrow of `bound` ends before
    /// the first release. Kept for its capacity.
    scratch: Vec<u32>,
    events: Events,
    minted: u32,
}

impl Default for RecognizerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl RecognizerPool {
    /// Returns an empty pool. Nothing is minted until a contact needs a recogniser.
    #[must_use]
    pub fn new() -> Self {
        Self {
            free: Vec::new(),
            ptp_free: Vec::new(),
            bound: FxHashMap::default(),
            inertial: Vec::new(),
            scratch: Vec::new(),
            events: Events::new(),
            minted: 0,
        }
    }

    /// Returns the queue every bound recogniser raises into, drained by the router after
    /// each feed.
    #[must_use]
    pub fn events(&self) -> &Events {
        &self.events
    }

    /// Returns how many recognisers have been constructed. A count that keeps rising means
    /// contacts are not being released.
    #[must_use]
    pub const fn minted(&self) -> u32 {
        self.minted
    }

    /// Returns how many contacts are bound.
    #[must_use]
    pub fn live(&self) -> usize {
        self.bound.len()
    }

    /// Binds a contact to a target, configured from that target's declaration.
    ///
    /// `rejected` says the contact arrived without the digitizer's confidence. It is still
    /// bound — so that its up and its cancel are accounted for — but nothing is fed to the
    /// recogniser, so it can never start a gesture.
    ///
    /// # Errors
    ///
    /// A recogniser could not be constructed, or the platform refused the configuration
    /// `decl` asks for.
    pub fn bind(
        &mut self,
        id: u32,
        ptype: PointerType,
        target: ControlId,
        decl: GestureDecl,
        origin: Point,
        rejected: bool,
    ) -> Result<&mut Bound> {
        let recognizer = self.take(ptype)?;
        recognizer.configure(&decl)?;
        let drag = decl.drag.map(|drag| Drag::new(drag, origin));
        Ok(self.bound.entry(id).or_insert(Bound {
            target,
            decl,
            origin,
            drag,
            manipulating: false,
            inertial: false,
            rejected,
            recognizer,
        }))
    }

    /// Returns what a contact is bound to, or `None` if it is not bound.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&Bound> {
        self.bound.get(&id)
    }

    /// Returns what a contact is bound to, mutably, or `None` if it is not bound.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Bound> {
        self.bound.get_mut(&id)
    }

    /// Returns whether any contact is bound to `target`.
    #[must_use]
    pub fn holds(&self, target: ControlId) -> bool {
        self.bound.values().any(|bound| bound.target == target)
    }

    /// Returns every bound contact, for the tick that pumps inertia.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &mut Bound)> {
        self.bound.iter_mut().map(|(id, bound)| (*id, bound))
    }

    /// Returns whether anything is still in inertia, which is what keeps a frame requested.
    #[must_use]
    pub fn any_inertial(&self) -> bool {
        !self.inertial.is_empty()
    }

    /// Sets whether `id`'s motion is still being pumped, and keeps the inertial index exact.
    ///
    /// The sole writer of [`Bound::is_inertial`]. Does nothing if `id` is not bound, and
    /// nothing if the flag already reads `on`, so a repeated set cannot double-enter the
    /// index.
    pub(crate) fn set_inertial(&mut self, id: u32, on: bool) {
        let Some(bound) = self.bound.get_mut(&id) else {
            return;
        };
        if bound.inertial == on {
            return;
        }
        bound.inertial = on;
        if on {
            self.inertial.push(id);
        } else {
            self.drop_inertial(id);
        }
    }

    /// Copies the ids still in inertia into `out`, replacing what it held.
    ///
    /// `out` keeps its capacity, so a caller that hands back the same buffer each tick
    /// allocates only on the first one.
    pub(crate) fn inertial_into(&self, out: &mut Vec<u32>) {
        out.clear();
        out.extend_from_slice(&self.inertial);
    }

    /// Collects every contact bound to `target` into `out`, replacing what it held.
    ///
    /// `out` keeps its capacity. Costs one pass over the bound contacts.
    pub(crate) fn bound_to(&self, target: ControlId, out: &mut Vec<u32>) {
        out.clear();
        out.extend(
            self.bound
                .iter()
                .filter(|(_, bound)| bound.target == target)
                .map(|(id, _)| *id),
        );
    }

    /// Drops `id` from the inertial index, if it is there.
    fn drop_inertial(&mut self, id: u32) {
        if let Some(at) = self.inertial.iter().position(|&held| held == id) {
            self.inertial.swap_remove(at);
        }
    }

    /// Ends a contact and returns its recogniser to the pool.
    ///
    /// `abort` is the difference between an up and a cancel: an aborted contact's queued
    /// events are discarded, so a gesture the user withdrew cannot be delivered after the
    /// fact. Both call `CompleteGesture`, because both end the recogniser's interest.
    pub fn release(&mut self, id: u32, abort: bool) {
        let Some(bound) = self.bound.remove(&id) else {
            return;
        };
        if bound.inertial {
            self.drop_inertial(id);
        }
        // A failure here is a recogniser that is already finished, which is exactly the
        // state being asked for.
        _ = bound.recognizer.complete();
        if abort {
            self.events.clear();
        }
        let free = if bound.recognizer.is_physical() {
            &mut self.ptp_free
        } else {
            &mut self.free
        };
        free.push(bound.recognizer);
    }

    /// Ends every contact. What a lost capture and a window losing focus both do.
    pub fn release_all(&mut self, abort: bool) {
        let mut ids = core::mem::take(&mut self.scratch);
        ids.extend(self.bound.keys().copied());
        for &id in &ids {
            self.release(id, abort);
        }
        ids.clear();
        self.scratch = ids;
    }

    /// Returns a free recogniser of the kind `ptype` needs, minting one if the list is
    /// empty.
    fn take(&mut self, ptype: PointerType) -> Result<FrontHandle<Recognizer>> {
        let physical = ptype.is_touchpad();
        let free = if physical {
            &mut self.ptp_free
        } else {
            &mut self.free
        };
        if let Some(recognizer) = free.pop() {
            return Ok(recognizer);
        }
        self.minted += 1;
        let recognizer = if physical {
            Recognizer::physical(&self.events)?
        } else {
            Recognizer::gesture(&self.events)?
        };
        Ok(FrontHandle::new(recognizer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Binds one mouse contact and returns its id.
    fn bind(pool: &mut RecognizerPool, id: u32) {
        pool.bind(
            id,
            PointerType::Mouse,
            ControlId::FIRST,
            GestureDecl::default(),
            Point { x: 0.0, y: 0.0 },
            false,
        )
        .expect("a recogniser could not be constructed");
    }

    /// Returns what the index holds, through the copy the router tick takes.
    fn index(pool: &RecognizerPool) -> Vec<u32> {
        let mut out = Vec::new();
        pool.inertial_into(&mut out);
        out
    }

    #[test]
    fn a_fresh_binding_is_not_inertial() {
        let mut pool = RecognizerPool::new();
        bind(&mut pool, 7);
        assert!(!pool.any_inertial());
        assert!(index(&pool).is_empty());
    }

    #[test]
    fn setting_the_flag_enters_the_index_and_clearing_it_leaves() {
        let mut pool = RecognizerPool::new();
        bind(&mut pool, 7);
        pool.set_inertial(7, true);
        assert!(pool.any_inertial());
        assert!(pool.get(7).expect("binding lost").is_inertial());
        assert_eq!(index(&pool), vec![7]);
        pool.set_inertial(7, false);
        assert!(!pool.any_inertial());
        assert!(index(&pool).is_empty());
    }

    #[test]
    fn setting_the_flag_twice_enters_the_index_once() {
        let mut pool = RecognizerPool::new();
        bind(&mut pool, 7);
        pool.set_inertial(7, true);
        pool.set_inertial(7, true);
        assert_eq!(index(&pool), vec![7]);
    }

    #[test]
    fn releasing_an_inertial_contact_leaves_the_index() {
        let mut pool = RecognizerPool::new();
        bind(&mut pool, 7);
        pool.set_inertial(7, true);
        pool.release(7, false);
        assert!(!pool.any_inertial());
        assert!(index(&pool).is_empty());
    }

    #[test]
    fn releasing_one_of_two_leaves_the_other_pumping() {
        let mut pool = RecognizerPool::new();
        bind(&mut pool, 7);
        bind(&mut pool, 8);
        pool.set_inertial(7, true);
        pool.set_inertial(8, true);
        pool.release(7, false);
        assert!(pool.any_inertial());
        assert_eq!(index(&pool), vec![8]);
    }

    #[test]
    fn releasing_everything_empties_the_index() {
        let mut pool = RecognizerPool::new();
        bind(&mut pool, 7);
        bind(&mut pool, 8);
        pool.set_inertial(8, true);
        pool.release_all(false);
        assert_eq!(pool.live(), 0);
        assert!(!pool.any_inertial());
        assert!(index(&pool).is_empty());
    }

    #[test]
    fn an_unbound_id_cannot_enter_the_index() {
        let mut pool = RecognizerPool::new();
        pool.set_inertial(7, true);
        assert!(!pool.any_inertial());
    }

    #[test]
    fn the_contacts_on_a_target_are_collected_without_the_others() {
        let mut pool = RecognizerPool::new();
        bind(&mut pool, 7);
        let mut out = Vec::new();
        pool.bound_to(ControlId::FIRST, &mut out);
        assert_eq!(out, vec![7]);
        pool.bound_to(ControlId::NONE, &mut out);
        assert!(out.is_empty());
    }
}
