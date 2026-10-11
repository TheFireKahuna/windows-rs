//! Debug-build trace notes.
//!
//! A note is one stderr line naming a failure the framework carries instead of raising: a
//! loss-classified `HRESULT` the draw bracket swallows, a rasterization a lost device
//! dropped, a glow that lost its silhouette. Errors that propagate need no note — the
//! driver records a propagated failure, closes the window, and the application's `run`
//! returns it.
//!
//! Notes are off unless `NEWAPO_NOTES=1`, read once for the run. Off is what makes a note
//! free: nothing is formatted and nothing is written, so under a test harness the captured
//! stderr buffer never grows and a measured path stays allocation-free.

use std::sync::OnceLock;

/// Returns whether trace notes are enabled, resolved once from `NEWAPO_NOTES=1`.
pub fn note_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("NEWAPO_NOTES").is_some_and(|v| v == "1"))
}

/// Emits a trace note to stderr when enabled, in debug builds only.
///
/// Release builds keep the statement only behind a constant `false`: the arguments are
/// type-checked against the format string and count as used, and none is evaluated, so a
/// note costs nothing where it is not wanted. An argument must therefore be the expression
/// that computes the value, not a binding made for the note, or release computes it anyway.
/// `tag` and the format string must be literals; `tag` prefixes the line as `[tag]`. The
/// enable check short-circuits before the arguments are formatted, so a disabled note
/// allocates nothing.
#[macro_export]
macro_rules! note {
    ($tag:literal, $format:literal $(, $arg:expr)* $(,)?) => {
        if cfg!(debug_assertions) && $crate::note_enabled() {
            eprintln!(concat!("[", $tag, "] ", $format) $(, $arg)*);
        }
    };
}
