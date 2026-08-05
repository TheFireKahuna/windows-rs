//! The [`Geometry`] trait, which lets a sprite shape or a geometric clip take any
//! composition geometry.

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
        // `Self::` selects the inherent method of the same name, the one `shape.rs` calls
        // internally, rather than recursing into this trait method.
        Self::as_geometry(self)
    }
}

// `CompositionGeometry`'s trim, and every geometry beyond the ellipse, are carried for
// the system stack only and live in `path.rs`. The trait itself is not, because
// `Compositor::create_sprite_shape` takes it and that factory exists on both stacks.
