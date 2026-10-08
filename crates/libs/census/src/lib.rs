#![doc = include_str!("../readme.md")]
#![forbid(unsafe_code)]

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Whether this build counts.
pub const ENABLED: bool = cfg!(feature = "enabled");

/// What a site's value means.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Hits added since the process started.
    Count,
    /// A running total the owner keeps and publishes whole.
    Total,
    /// An instantaneous value, such as a live-object count.
    Level,
    /// Nanoseconds spent inside a [`span!`], summed over its entries.
    Time,
}

impl Kind {
    /// The lowercase name readers print.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Total => "total",
            Self::Level => "level",
            Self::Time => "ns",
        }
    }
}

/// One counter, owned by the `static` a macro declares at its call site.
pub struct Site {
    name: &'static str,
    kind: Kind,
    value: AtomicU64,
    linked: AtomicBool,
}

impl Site {
    #[doc(hidden)]
    pub const fn new(name: &'static str, kind: Kind) -> Self {
        Self { name, kind, value: AtomicU64::new(0), linked: AtomicBool::new(false) }
    }
}

/// Every site hit so far. Taken once per site, on its first hit, and by [`read`].
static SITES: Mutex<Vec<&'static Site>> = Mutex::new(Vec::new());

#[cold]
fn link(site: &'static Site) {
    let mut sites = SITES.lock().unwrap_or_else(|e| e.into_inner());
    // Under the lock, so two first hits on two threads link the site once.
    if !site.linked.swap(true, Ordering::Relaxed) {
        sites.push(site);
    }
}

#[doc(hidden)]
#[inline(always)]
pub fn add(site: &'static Site, n: u64) {
    if ENABLED {
        // Relaxed throughout: a value is a tally read for reporting, and orders no other memory.
        if !site.linked.load(Ordering::Relaxed) {
            link(site);
        }
        site.value.fetch_add(n, Ordering::Relaxed);
    }
}

#[doc(hidden)]
#[inline(always)]
pub fn store(site: &'static Site, v: u64) {
    if ENABLED {
        if !site.linked.load(Ordering::Relaxed) {
            link(site);
        }
        site.value.store(v, Ordering::Relaxed);
    }
}

/// Calls `f` with the name, kind and current value of every site hit so far.
///
/// Sites sharing a name are reported once each; a reader sums them. Reports nothing
/// unless the `enabled` feature is on.
pub fn read(mut f: impl FnMut(&'static str, Kind, u64)) {
    if !ENABLED {
        return;
    }
    let sites = SITES.lock().unwrap_or_else(|e| e.into_inner());
    for site in sites.iter() {
        f(site.name, site.kind, site.value.load(Ordering::Relaxed));
    }
}

/// Times one entry of a [`span!`] until it drops.
#[must_use = "a span times the scope it is bound in; `_` drops it at once"]
pub struct Span(Option<(&'static Site, std::time::Instant)>);

impl Span {
    #[doc(hidden)]
    #[inline(always)]
    pub fn open(time: &'static Site, hits: &'static Site) -> Self {
        if ENABLED {
            add(hits, 1);
            Self(Some((time, std::time::Instant::now())))
        } else {
            Self(None)
        }
    }
}

impl Drop for Span {
    #[inline(always)]
    fn drop(&mut self) {
        if let Some((time, at)) = self.0 {
            add(time, at.elapsed().as_nanos() as u64);
        }
    }
}

/// Adds one, or `n`, to the counter `name`.
#[macro_export]
macro_rules! count {
    ($name:literal) => {
        $crate::count!($name, 1u64)
    };
    ($name:literal, $n:expr) => {{
        static SITE: $crate::Site = $crate::Site::new($name, $crate::Kind::Count);
        $crate::add(&SITE, ($n) as u64);
    }};
}

/// Publishes the instantaneous value of `name`.
#[macro_export]
macro_rules! level {
    ($name:literal, $v:expr) => {{
        static SITE: $crate::Site = $crate::Site::new($name, $crate::Kind::Level);
        $crate::store(&SITE, ($v) as u64);
    }};
}

/// Publishes a running total of `name` that the caller keeps.
#[macro_export]
macro_rules! total {
    ($name:literal, $v:expr) => {{
        static SITE: $crate::Site = $crate::Site::new($name, $crate::Kind::Total);
        $crate::store(&SITE, ($v) as u64);
    }};
}

/// Times the rest of the enclosing scope as `name`: wall nanoseconds as a
/// [`Kind::Time`] site and entries as a [`Kind::Count`] site of the same name.
///
/// The returned guard must be bound to a named variable; `let _ = span!(..)` times nothing.
#[macro_export]
macro_rules! span {
    ($name:literal) => {{
        static TIME: $crate::Site = $crate::Site::new($name, $crate::Kind::Time);
        static HITS: $crate::Site = $crate::Site::new($name, $crate::Kind::Count);
        $crate::Span::open(&TIME, &HITS)
    }};
}

#[cfg(all(test, feature = "enabled"))]
mod tests {
    #[test]
    fn sites_sum_by_hit_and_report_once() {
        for _ in 0..3 {
            crate::count!("test.hits");
        }
        crate::count!("test.bulk", 5);
        crate::level!("test.level", 7);
        {
            let _span = crate::span!("test.span");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let mut seen = Vec::new();
        crate::read(|name, kind, v| {
            if name.starts_with("test.") {
                seen.push((name, kind.name(), v));
            }
        });
        seen.sort();
        let spent = seen.iter().find(|row| row.0 == "test.span" && row.1 == "ns").map(|row| row.2);
        assert!(spent.is_some_and(|ns| ns >= 1_000_000), "the span timed {spent:?}");
        seen.retain(|row| row.1 != "ns");
        assert_eq!(seen, [
            ("test.bulk", "count", 5), ("test.hits", "count", 3), ("test.level", "level", 7), ("test.span", "count", 1),
        ]);
    }
}
