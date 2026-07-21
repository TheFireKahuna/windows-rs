/// A live subscription — an element's size notifications, a pointer sink —
/// that ends when this drops.
///
/// Two shapes sit behind it. The WinUI backend subscribes a WinRT event on the
/// native element and hands back its `EventRevoker`. The DirectComposition
/// backend registers against an id-keyed backend registry and hands back only a
/// token: **no COM, so the holder is not pinned to the backend's thread**, which
/// is what lets app code retain one across renders (and, once the reconciler
/// moves off the UI thread, across threads).
pub struct Subscription(SubscriptionInner);

enum SubscriptionInner {
    /// WinRT event registration; revokes itself on drop.
    Winrt(#[allow(dead_code, reason = "revocation is the Drop impl")] windows_core::EventRevoker),
    /// Token in an id-keyed backend registry, removed by `remove` on drop.
    Token { token: i64, remove: fn(i64) },
}

impl Subscription {
    pub(crate) fn winrt(revoker: windows_core::EventRevoker) -> Self {
        Self(SubscriptionInner::Winrt(revoker))
    }

    pub(crate) fn token(token: i64, remove: fn(i64)) -> Self {
        Self(SubscriptionInner::Token { token, remove })
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // The WinRT arm revokes through `EventRevoker`'s own `Drop`.
        if let SubscriptionInner::Token { token, remove } = &self.0 {
            remove(*token);
        }
    }
}
