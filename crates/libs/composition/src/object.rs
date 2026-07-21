use super::*;

/// The base type shared by every composition object — visuals, brushes, clips,
/// geometries, and property sets.
///
/// An [`Object`] can be turned into one via [`Object::as_object`] to bind it
/// into an [`ExpressionAnimation`](crate::ExpressionAnimation) as a named
/// reference parameter.
#[derive(Clone)]
pub struct CompositionObject(pub(crate) bindings::CompositionObject);

/// A composition object that an [`ExpressionAnimation`](crate::ExpressionAnimation)
/// can reference by name, so its properties are readable from the expression.
///
/// This trait is sealed: only the types in this crate implement it.
pub trait Object: Sealed {
    /// Returns this value as the shared [`CompositionObject`] base type.
    fn as_object(&self) -> CompositionObject;
}

/// The `IUnknown` pointer of `value`, which COM guarantees is stable and unique
/// per object — the only sound basis for comparing two interface pointers.
///
/// Used by the wrapper types' `PartialEq` impls, which is how a caller asks
/// "is this the same brush I already bound?" without the pointer itself ever
/// leaving the crate.
pub(crate) fn canonical(value: &impl Interface) -> *mut core::ffi::c_void {
    value
        .cast::<windows_core::IUnknown>()
        .map(|unknown| unknown.as_raw())
        .unwrap_or(core::ptr::null_mut())
}
