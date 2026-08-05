//! Shared resources: the four id-keyed tables. **Front half.**
//!
//! A resource is keyed by an identity the model minted, so these tables need no hash, no
//! quantization and no eviction. A value derived from something unbounded is keyed in
//! [`Cache`](crate::Cache) instead.
//!
//! Re-rasterizing re-points the object every sprite holds and never replaces it: a geometry
//! is re-pathed, a brush is re-surfaced. Every sprite painting with one resource therefore
//! moves together, and an entry here is a bare composition object.

use crate::id::Id;
use crate::id::Slots;
use crate::sink::{Geom, Holding, Ramp, Region, RegionId, ResId, Run};
use windows_composition::{CompositionPathGeometry, CompositionSurfaceBrush};

/// A shared resource, and the two independent claims on it.
///
/// The model disclaims when its declaration goes away and a sprite releases when it is
/// destroyed or re-declared, in either order. The entry lives until both claims are gone, so
/// a resource neither outlives its last holder nor is pulled out from under one.
#[derive(Debug)]
pub(crate) struct Res<T> {
    pub(crate) value: T,
    /// Sprites painting with it.
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

/// One resource family: the ids it is keyed by, and what it holds under them.
///
/// `F` is the model's family marker, as in `Id<Geom>`, and `T` is the composition object.
/// Naming the family in the type is what stops a geom id reaching the ramp table.
pub(crate) type ResTable<F, T> = Slots<F, Res<T>>;

impl<F, T> ResTable<F, T> {
    /// Returns the resource under `id`, or `None` if no entry is live.
    pub(crate) fn value(&self, id: Id<F>) -> Option<&T> {
        self.get(id).map(|res| &res.value)
    }

    /// Takes a sprite's hold on `id`.
    fn retain(&mut self, id: Id<F>) {
        if let Some(res) = self.get_mut(id) {
            res.rc += 1;
        }
    }

    /// Gives up a sprite's hold, dropping the entry if that was the last claim on it.
    fn release(&mut self, id: Id<F>) {
        if let Some(res) = self.get_mut(id) {
            res.rc = res.rc.saturating_sub(1);
        }
        self.collect(id);
    }

    /// Gives up the model's claim. The declaration is gone; the sprites holding it may not
    /// be.
    fn disclaim(&mut self, id: Id<F>) {
        if let Some(res) = self.get_mut(id) {
            res.claimed = false;
        }
        self.collect(id);
    }

    fn collect(&mut self, id: Id<F>) {
        if self.get(id).is_some_and(Res::unheld) {
            // The model owns the id space on the other side of the seam, so this frees the
            // row and not the id.
            self.take(id);
        }
    }
}

/// Every shared resource this scene holds.
#[derive(Default)]
pub(crate) struct Resources {
    pub(crate) geoms: ResTable<Geom, CompositionPathGeometry>,
    pub(crate) ramps: ResTable<Ramp, CompositionSurfaceBrush>,
    pub(crate) runs: ResTable<Run, CompositionSurfaceBrush>,
    /// `None` until the producer hands over its buffer, which arrives out of band as the
    /// one kernel handle that legitimately crosses from the present thread.
    pub(crate) regions: ResTable<Region, Option<CompositionSurfaceBrush>>,
}

/// Dispatches one verb to the table that holds the family a [`Holding`] names.
///
/// The single place a family maps to a table.
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

    /// Drops the model's claim on `id`, whichever family it names.
    ///
    /// A [`ResId`] carries no family, so all four tables are asked; at most one holds it.
    pub(crate) fn disclaim(&mut self, id: ResId) {
        self.geoms.disclaim(id.cast::<Geom>());
        self.ramps.disclaim(id.cast::<Ramp>());
        self.runs.disclaim(id.cast::<Run>());
        self.regions.disclaim(id.cast::<Region>());
    }

    /// Returns the buffer a region points at, or `None` before the producer hands one over.
    pub(crate) fn region(&self, id: RegionId) -> Option<&CompositionSurfaceBrush> {
        self.regions.value(id).and_then(Option::as_ref)
    }
}

impl<F> ResTable<F, CompositionSurfaceBrush> {
    /// Re-points at a freshly drawn surface, minting the brush the first time.
    pub(crate) fn point(
        &mut self,
        id: Id<F>,
        surface: &impl windows_composition::Surface,
        fresh: impl FnOnce() -> CompositionSurfaceBrush,
    ) {
        match self.get_mut(id) {
            Some(res) => res.value.set_surface(surface),
            None => self.place(id, Res::new(fresh())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resource_outlives_the_model_but_not_its_last_sprite() {
        let mut table: ResTable<(), u8> = ResTable::default();
        let id = Id::<()>::raw(1, 1);
        table.place(id, Res::new(7));

        table.retain(id);
        table.retain(id);
        // The declaration goes first, while two sprites are still painting with it.
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
        let mut table: ResTable<(), u8> = ResTable::default();
        let id = Id::<()>::raw(1, 1);
        table.place(id, Res::new(7));
        table.disclaim(id);
        assert!(table.get(id).is_none());
    }

    #[test]
    fn releasing_more_than_was_taken_cannot_drop_a_declared_resource() {
        let mut table: ResTable<(), u8> = ResTable::default();
        let id = Id::<()>::raw(1, 1);
        table.place(id, Res::new(7));
        // An app-side bug must not be able to pull a resource out from under a declaration
        // that is still standing.
        table.release(id);
        table.release(id);
        assert!(table.get(id).is_some());
    }
}
