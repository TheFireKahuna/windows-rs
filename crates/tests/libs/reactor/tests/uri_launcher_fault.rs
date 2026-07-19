//! `style.rs` — a URI launcher that panics must be contained.
//!
//! The launcher is app code invoked from the window procedure, behind an
//! `extern "system"` boundary that cannot unwind: a panic escaping it aborts
//! the process, taking the user's unsaved state with it because a hyperlink
//! handler had a bug. So the call sits inside the same fault boundary as an
//! event handler, and this pins that.
//!
//! Its own test binary, not a second `#[test]` next to `uri_launcher.rs`: the
//! launcher is a process-global `OnceLock` and there is exactly one slot per
//! process. Two tests in one binary would race for it and the loser would pass
//! vacuously.

use windows_reactor::{launch_uri, set_uri_launcher};

#[test]
fn a_panicking_launcher_does_not_unwind_into_the_window_procedure() {
    set_uri_launcher(|_| panic!("the app's launcher blew up"));

    // Returns normally rather than unwinding — and reports "not handled",
    // because a launcher that never reached an answer did not give one. The
    // caller treats that exactly like a decline: nothing is launched, and
    // nothing else in the crate picks the URI up.
    assert!(
        !launch_uri("https://example.com"),
        "a panicking launcher reported its URI as handled"
    );

    // Still usable afterwards: the boundary catches, it does not poison.
    assert!(!launch_uri("https://example.com/second"));
}
