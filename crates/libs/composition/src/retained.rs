//! The rest of the retained visual tree (feature `system`).
//!
//! What a tree needs to be *driven* rather than merely built: the placement and
//! rotation properties an animation targets, the collection operations a reconciler
//! performs, and the one part of composition state that can be read back exactly.

use super::*;

impl Visual {
    /// Sets the visual's rotation about its
    /// [center point](Visual::set_center_point), in **radians** — the unit of the
    /// `RotationAngle` property that animations and expressions drive. (Composition
    /// also exposes the same rotation in degrees as a separate property, which this
    /// crate does not surface: two names for one value invites animating the one
    /// nothing is bound to.)
    pub fn set_rotation_angle(&self, radians: f32) {
        self.0.SetRotationAngle(radians).unwrap();
    }

    /// Rounds this visual's composed position to whole device pixels.
    ///
    /// This governs where the visual is placed, not what its `Offset` holds: an
    /// animation driving that offset still evaluates and writes every frame, and
    /// anything charged per write is charged just the same. Snapping is for crispness
    /// — content that would otherwise be resampled across a pixel boundary — and
    /// pairs with a step-eased animation rather than substituting for one.
    pub fn set_pixel_snapping(&self, enabled: bool) {
        let visual: bindings::IVisual4 = self.0.cast().unwrap();
        visual.SetIsPixelSnappingEnabled(enabled).unwrap();
    }

    /// Binds this visual's box to its parent's, now and after every parent resize.
    ///
    /// The common case of
    /// [`set_relative_size_adjustment`](Visual::set_relative_size_adjustment), which
    /// is *additive* — `size + parent.size * adjustment` — so this pairs a `(1, 1)`
    /// adjustment with whatever `set_size` already holds. **The parent must carry a
    /// real size.** For a `SpriteVisual` that is automatic, since size IS its painted
    /// area, but a bare `ContainerVisual` left at its `(0, 0)` default silently
    /// resolves every child's adjustment to nothing: nothing throws, nothing logs, and
    /// the subtree simply does not draw. A container children measure against is a
    /// size anchor and has to be sized deliberately.
    pub fn fill_parent(&self) {
        self.set_relative_size_adjustment(Vector2 { x: 1.0, y: 1.0 });
    }

    /// Views this visual as a container, if it is one.
    ///
    /// [`VisualCollection::iter`] hands back the base [`Visual`] type, which has no
    /// children of its own, so without this a tree walk could only ever see one level.
    /// Every visual this crate mints except a bare `Visual` is a container underneath
    /// — sprites and shape visuals both derive from `ContainerVisual` — so the cast
    /// usually succeeds.
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
    /// Every child, in z-order (bottom first).
    ///
    /// The child collection is enumerable, which makes the visual tree the one part of
    /// composition state an app can read back exactly rather than infer. That is what
    /// an authoritative visual census needs: a running insert/remove tally can drift,
    /// a walk cannot.
    ///
    /// Fallible because the enumeration is a `QueryInterface` away — a collection that
    /// does not answer to `IIterable` cannot be walked, and inventing an empty iterator
    /// for that case would report a populated subtree as childless.
    ///
    /// The `use<>` bound is load-bearing: the iterator holds its own reference to the
    /// collection's WinRT iterator and borrows nothing, so without it edition 2024
    /// would capture `&self` and a walk could not enumerate a collection obtained from
    /// a temporary — which is every nested level of a tree walk.
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
    /// detached — a parent torn down between the two operations, say. For that caller
    /// "already gone" is the goal state rather than an error, so the removal is
    /// fallible instead of a panic. Prefer [`remove`](VisualCollection::remove) when
    /// the visual's membership is known.
    pub fn try_remove(&self, visual: &Visual) -> Result<()> {
        self.0.Remove(&visual.0)
    }
}
