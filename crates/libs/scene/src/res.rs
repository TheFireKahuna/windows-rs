//! Shared resources: the four id-keyed tables. **Front half.**
//!
//! The first of the two storage machines. Which one a thing lands in is one question:
//! **is the key an identity the model minted, or a value derived from something
//! unbounded?** These are the identities, so they need no hash, no quantization and no
//! eviction. See [`Cache`](crate::Cache) for the other.
//!
//! **Re-rasterizing re-points the object every sprite holds; it never replaces it.** A
//! geometry is re-pathed, a brush is re-surfaced. That makes "one resource, many sprites,
//! all moving together" a property of the object, and it is why an entry here is a bare
//! composition object.

use crate::id::Id;
use crate::node::Slots;
use crate::sink::{Holding, RegionId, ResId};
use windows_composition::{CompositionPathGeometry, CompositionSurfaceBrush};

/// A shared resource, and the two independent claims on it.
///
/// **Two claims and not one**: the model disclaims when its declaration goes away, a sprite
/// releases when it is destroyed or re-declared, and neither happens first. The entry lives
/// until both are gone, so a resource can neither outlive its last holder nor be pulled out
/// from under one.
#[derive(Debug)]
pub(crate) struct Res<T> {
    pub(crate) value: T,
    /// Sprites currently painting with it.
    rc: u32,
    /// Whether the model still declares it.
    claimed: bool,
}

impl<T> Res<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value,
            rc: 0,
            claimed: true,
        }
    }

    const fn unheld(&self) -> bool {
        self.rc == 0 && !self.claimed
    }
}

/// One resource family.
pub(crate) type ResTable<T> = Slots<Res<T>>;

impl<T> ResTable<T> {
    pub(crate) fn value<F>(&self, id: Id<F>) -> Option<&T> {
        self.get(id).map(|res| &res.value)
    }

    fn retain<F>(&mut self, id: Id<F>) {
        if let Some(res) = self.get_mut(id) {
            res.rc += 1;
        }
    }

    /// Gives up a sprite's hold, dropping the entry if that was the last claim on it.
    fn release<F>(&mut self, id: Id<F>) {
        if let Some(res) = self.get_mut(id) {
            res.rc = res.rc.saturating_sub(1);
        }
        self.collect(id);
    }

    /// Gives up the *model's* claim. The declaration is gone; the sprites may not be.
    fn disclaim<F>(&mut self, id: Id<F>) {
        if let Some(res) = self.get_mut(id) {
            res.claimed = false;
        }
        self.collect(id);
    }

    fn collect<F>(&mut self, id: Id<F>) {
        if self.get(id).is_some_and(Res::unheld) {
            self.remove(id);
        }
    }
}

/// Every shared resource this scene holds.
#[derive(Default)]
pub(crate) struct Resources {
    pub(crate) geoms: ResTable<CompositionPathGeometry>,
    pub(crate) ramps: ResTable<CompositionSurfaceBrush>,
    pub(crate) runs: ResTable<CompositionSurfaceBrush>,
    /// `None` until the producer hands over its buffer, which arrives out of band as the
    /// one kernel handle that legitimately crosses from the present thread.
    pub(crate) regions: ResTable<Option<CompositionSurfaceBrush>>,
}

/// Dispatches one verb to whichever table holds the family.
///
/// The **one** place a family maps to a table, which is what makes a resource's lifetime
/// independent of the declaration that produced it.
macro_rules! dispatch {
    ($self:ident, $verb:ident, $holding:expr) => {
        match $holding {
            Holding::Geom(id) => $self.geoms.$verb(id),
            Holding::Ramp(id) => $self.ramps.$verb(id),
            Holding::Run(id) => $self.runs.$verb(id),
            Holding::Region(id) => $self.regions.$verb(id),
        }
    };
}

impl Resources {
    pub(crate) fn retain(&mut self, holding: Option<Holding>) {
        if let Some(holding) = holding {
            dispatch!(self, retain, holding);
        }
    }

    pub(crate) fn release(&mut self, holding: Option<Holding>) {
        if let Some(holding) = holding {
            dispatch!(self, release, holding);
        }
    }

    /// Drops the model's claim on `id`, whichever family it turns out to name.
    ///
    /// All four: a [`ResId`] names no family and exactly one table can be holding it.
    pub(crate) fn disclaim(&mut self, id: ResId) {
        self.geoms.disclaim(id.cast::<crate::sink::Geom>());
        self.ramps.disclaim(id.cast::<crate::sink::Ramp>());
        self.runs.disclaim(id.cast::<crate::sink::Run>());
        self.regions.disclaim(id.cast::<crate::sink::Region>());
    }

    /// The buffer a region is currently pointed at, if the producer has handed one over.
    pub(crate) fn region(&self, id: RegionId) -> Option<&CompositionSurfaceBrush> {
        self.regions.value(id).and_then(Option::as_ref)
    }
}

impl ResTable<CompositionSurfaceBrush> {
    /// Re-points at a freshly drawn surface, minting the brush the first time.
    pub(crate) fn point(
        &mut self,
        id: Id<impl Sized>,
        surface: &impl windows_composition::Surface,
        fresh: impl FnOnce() -> CompositionSurfaceBrush,
    ) {
        match self.get_mut(id) {
            Some(res) => res.value.set_surface(surface),
            None => self.insert(id, Res::new(fresh())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resource_outlives_the_model_but_not_its_last_sprite() {
        let mut table: ResTable<u8> = ResTable::default();
        let id = Id::<()>::raw(1, 1);
        table.insert(id, Res::new(7));

        table.retain(id);
        table.retain(id);
        // The declaration goes first. Two sprites are still painting with it, so it has to
        // survive — this is the case an unconditional remove gets wrong.
        table.disclaim(id);
        assert!(
            table.get(id).is_some(),
            "disclaiming dropped a held resource"
        );

        table.release(id);
        assert!(table.get(id).is_some(), "one sprite still holds it");
        table.release(id);
        assert!(
            table.get(id).is_none(),
            "the last holder left and it stayed"
        );
    }

    #[test]
    fn a_resource_no_sprite_ever_took_goes_with_the_declaration() {
        let mut table: ResTable<u8> = ResTable::default();
        let id = Id::<()>::raw(1, 1);
        table.insert(id, Res::new(7));
        table.disclaim(id);
        assert!(table.get(id).is_none());
    }

    #[test]
    fn releasing_more_than_was_taken_cannot_drop_a_declared_resource() {
        let mut table: ResTable<u8> = ResTable::default();
        let id = Id::<()>::raw(1, 1);
        table.insert(id, Res::new(7));
        // An app-side bug must not be able to pull a resource out from under a declaration
        // that is still standing.
        table.release(id);
        table.release(id);
        assert!(table.get(id).is_some());
    }
}
