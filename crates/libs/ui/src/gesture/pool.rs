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
    /// Whether the contact has lifted and its motion is still being pumped.
    pub inertial: bool,
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
}

/// The recognisers, bound and free.
pub struct RecognizerPool {
    free: Vec<FrontHandle<Recognizer>>,
    ptp_free: Vec<FrontHandle<Recognizer>>,
    /// Keyed by the system's pointer id — the one structure in this crate not keyed by a
    /// dense index this crate minted.
    bound: FxHashMap<u32, Bound>,
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
        self.bound.values().any(|bound| bound.inertial)
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
        let ids: Vec<u32> = self.bound.keys().copied().collect();
        for id in ids {
            self.release(id, abort);
        }
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
