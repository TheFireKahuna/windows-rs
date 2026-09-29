//! Debug-build trace notes.
//!
//! A note is one stderr line naming a failure the framework carries instead of raising: a
//! loss-classified `HRESULT` the draw bracket swallows, a rasterization a lost device
//! dropped, a glow that lost its silhouette. Errors that propagate need no note — the
//! driver records a propagated failure, closes the window, and the application's `run`
//! returns it.

/// Emits a trace note to stderr, in debug builds only.
///
/// Release builds remove the statement before code generation: the [`cfg`] gates the block
/// on `debug_assertions`, so a note costs nothing where it is not wanted. `tag` and the
/// format string must be literals; `tag` prefixes the line as `[tag]`.
#[macro_export]
macro_rules! note {
    ($tag:literal, $format:literal $(, $arg:expr)* $(,)?) => {
        #[cfg(debug_assertions)]
        {
            eprintln!(concat!("[", $tag, "] ", $format) $(, $arg)*);
        }
    };
}
