//! The rest of the retained visual tree (feature `system`).
//!
//! The placement and rotation properties an animation targets, the collection operations
//! a reconciler performs, and the enumeration of a visual's children.

use super::*;

impl Visual {
    /// Sets the visual's rotation about its
    /// [center point](Visual::set_center_point), in **radians** — the unit of the
    /// `RotationAngle` property that animations and expressions drive. Composition also
    /// exposes the same rotation in degrees under a second property name; this crate
    /// surfaces only the radian one, so an animation and a setter always agree.
    pub fn set_rotation_angle(&self, radians: f32) {
        self.0.SetRotationAngle(radians).unwrap();
    }

    /// Rounds this visual's composed position to whole device pixels.
    ///
    /// This governs where the visual is composed, not what its `Offset` holds: an
    /// animation driving that offset still evaluates and writes every frame, and
    /// anything charged per write is charged just the same. Snapping keeps content crisp
    /// that would otherwise be resampled across a pixel boundary.
    pub fn set_pixel_snapping(&self, enabled: bool) {
        let visual: bindings::IVisual4 = self.0.cast().unwrap();
        visual.SetIsPixelSnappingEnabled(enabled).unwrap();
    }

    /// Binds this visual's box to its parent's, now and after every parent resize.
    ///
    /// The common case of
    /// [`set_relative_size_adjustment`](Visual::set_relative_size_adjustment), which
    /// is *additive* — `size + parent.size * adjustment` — so this pairs a `(1, 1)`
    /// adjustment with whatever `set_size` already holds. **The parent must carry a real
    /// size.** A `SpriteVisual` does so already, its size being its painted area, but a
    /// bare `ContainerVisual` left at its `(0, 0)` default resolves every child's
    /// adjustment to zero, with no error raised, and the subtree does not draw. A
    /// container that children size against is given a size explicitly.
    pub fn fill_parent(&self) {
        self.set_relative_size_adjustment(Vector2 { x: 1.0, y: 1.0 });
    }

    /// Returns this visual as a [`ContainerVisual`], or `None` if it is not one.
    ///
    /// [`VisualCollection::iter`] hands back the base [`Visual`] type, which has no
    /// children of its own, so a tree walk descends a level through this. Every visual
    /// this crate mints except a bare `Visual` is a container underneath — sprites and
    /// shape visuals both derive from `ContainerVisual` — so the cast usually succeeds.
    pub fn as_container(&self) -> Option<ContainerVisual> {
        self.0.cast().ok().map(ContainerVisual::new)
    }
}

impl SpriteVisual {
    /// Removes the brush, so the visual paints nothing.
    ///
    /// Not the same as hiding it: the compositor keeps a reference to whatever a
    /// visual is painted with, so a brush over a resource the app is about to free — a
    /// surface adopted from a handle the app is closing — must be cleared *here*, not
    /// merely dropped by the app.
    pub fn clear_brush(&self) {
        // Reached through the deref chain rather than the private `sprite` field: this
        // module is a sibling of the one that declares it, and the WinRT object is the
        // same one either way.
        let visual: &Visual = self;
        let sprite: bindings::SpriteVisual = visual.0.cast().unwrap();
        sprite.SetBrush(None).unwrap();
    }
}

impl VisualCollection {
    /// Returns every child, in z-order (bottom first).
    ///
    /// The child collection is enumerable, so a walk reads what the tree actually holds
    /// rather than a tally the app maintains alongside it.
    ///
    /// The returned iterator holds its own reference to the collection's WinRT iterator
    /// and borrows nothing; the `use<>` bound keeps edition 2024 from capturing `&self`,
    /// so a walk can enumerate a collection obtained from a temporary — which every
    /// nested level of a tree walk is.
    ///
    /// # Errors
    ///
    /// Returns an error when the collection does not answer to `IIterable` and so cannot
    /// be walked. An empty iterator there would report a populated subtree as childless.
    pub fn iter(&self) -> Result<impl Iterator<Item = Visual> + use<>> {
        let iterable: windows_collections::IIterable<bindings::Visual> = self.0.cast()?;
        Ok(iterable.into_iter().map(Visual))
    }

    /// Inserts a visual directly above `sibling` in the z-order (drawn after it, in
    /// front of it). `sibling` must already be in the collection.
    pub fn insert_above(&self, visual: &Visual, sibling: &Visual) {
        self.0.InsertAbove(&visual.0, &sibling.0).unwrap();
    }

    /// Removes a visual from the collection, reporting rather than panicking when it
    /// is not there.
    ///
    /// A caller that tracks children of its own can hold a visual that has since been
    /// detached — a parent torn down between the two operations, say. For that caller an
    /// already-detached visual is the goal state rather than an error, so the removal
    /// reports instead of panicking. [`remove`](VisualCollection::remove) suits a visual
    /// whose membership is known.
    pub fn try_remove(&self, visual: &Visual) -> Result<()> {
        self.0.Remove(&visual.0)
    }
}
