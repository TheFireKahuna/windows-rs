//! Property sets: the CPU-to-compositor frontier (feature `system`).

use super::*;

/// A named bag of animatable values owned by the compositor.
///
/// This is the crate's shared-state primitive for the compositor's own thread. An
/// [`ExpressionAnimation`] binds the set as a named reference parameter (via
/// [`Animatable::as_object`]) and reads its keys — `"set.Progress"` — as part of the
/// expression, so the compositor re-evaluates them every frame without waking the writing
/// thread. Writing one key therefore updates every animation that references it, and
/// reading one back never requires the writer to be running: the values live in the
/// composition engine, not in the app.
///
/// Create one with [`Compositor::create_property_set`].
#[derive(Clone)]
pub struct CompositionPropertySet(pub(crate) bindings::CompositionPropertySet);

impl CompositionPropertySet {
    /// Inserts (or replaces) a scalar under `name`.
    pub fn insert_scalar(&self, name: &str, value: f32) {
        self.0.InsertScalar(name, value).unwrap();
    }

    /// Inserts (or replaces) a [`Vector2`] under `name`.
    pub fn insert_vector2(&self, name: &str, value: Vector2) {
        self.0.InsertVector2(name, value).unwrap();
    }

    /// Inserts (or replaces) a [`Vector3`] under `name`.
    pub fn insert_vector3(&self, name: &str, value: Vector3) {
        self.0.InsertVector3(name, value).unwrap();
    }

    /// Returns the scalar stored under `name`, or `None` if the set has no such key or it
    /// holds another type.
    pub fn scalar(&self, name: &str) -> Option<f32> {
        let mut value = 0.0;
        let status = self.0.TryGetScalar(name, &mut value).ok()?;
        (status == bindings::CompositionGetValueStatus::Succeeded).then_some(value)
    }

    /// Returns the [`Vector2`] stored under `name`, or `None` if the set has no such key
    /// or it holds another type.
    pub fn vector2(&self, name: &str) -> Option<Vector2> {
        let mut value = Vector2 { x: 0.0, y: 0.0 };
        let status = self.0.TryGetVector2(name, &mut value).ok()?;
        (status == bindings::CompositionGetValueStatus::Succeeded).then_some(value)
    }

    /// Returns the [`Vector3`] stored under `name`, or `None` if the set has no such key
    /// or it holds another type.
    pub fn vector3(&self, name: &str) -> Option<Vector3> {
        let mut value = Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let status = self.0.TryGetVector3(name, &mut value).ok()?;
        (status == bindings::CompositionGetValueStatus::Succeeded).then_some(value)
    }
}

impl Sealed for CompositionPropertySet {}

impl Compositor {
    /// Creates an empty property set.
    pub fn create_property_set(&self) -> CompositionPropertySet {
        CompositionPropertySet(self.0.CreatePropertySet().unwrap())
    }
}
