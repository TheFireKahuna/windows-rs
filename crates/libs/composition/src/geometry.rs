use super::*;

/// A geometry that can be filled or stroked by a
/// [`CompositionSpriteShape`](crate::CompositionSpriteShape), or used to clip a
/// subtree through a [`CompositionGeometricClip`](crate::CompositionGeometricClip).
///
/// This trait is sealed: only the geometry types in this crate implement it.
pub trait Geometry: Sealed {
    /// Returns this geometry as the shared [`CompositionGeometry`] base type.
    fn as_geometry(&self) -> CompositionGeometry;
}

impl Sealed for CompositionEllipseGeometry {}

impl Geometry for CompositionEllipseGeometry {
    fn as_geometry(&self) -> CompositionGeometry {
        // The inherent method of the same name, which predates this trait and is what
        // `shape.rs` reaches for internally.
        Self::as_geometry(self)
    }
}

// `CompositionGeometry`'s trim, and every geometry beyond the ellipse, are carried for
// the system stack only and live in `path.rs`. The trait itself is not, because
// `Compositor::create_sprite_shape` takes it and that factory exists on both stacks.
