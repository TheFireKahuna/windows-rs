//! Static application tokens retained by identity and resolved against the current scope.

use super::Scope;
use core::{
    fmt,
    hash::{Hash, Hasher},
};

/// A typed resolver with stable identity. Declare each token as a `static`.
///
/// The resolver must be pure, bounded and allocation-free. It may read only its scope
/// and immutable application data: layout calls it while solving, including after a
/// container changes width class. It must not access the build arena or install effects.
/// A name is diagnostic; two statics with the same name remain distinct tokens.
pub struct ScopedToken<T> {
    name: &'static str,
    resolve: fn(Scope) -> T,
}

impl<T> ScopedToken<T> {
    /// Defines a token that has an answer for every scope, without registration.
    pub const fn new(name: &'static str, resolve: fn(Scope) -> T) -> Self {
        Self { name, resolve }
    }

    /// Resolves at the caller's current scope, without caching a width class.
    pub fn resolve(&self, scope: Scope) -> T {
        (self.resolve)(scope)
    }
}

impl<T> PartialEq for ScopedToken<T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}
impl<T> Eq for ScopedToken<T> {}
impl<T> Hash for ScopedToken<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::ptr::hash(self, state);
    }
}
impl<T> fmt::Debug for ScopedToken<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ScopedToken").field(&self.name).finish()
    }
}
