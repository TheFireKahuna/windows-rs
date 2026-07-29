#![doc = include_str!("../readme.md")]

#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments
)]
mod bindings;

// The safe surface — the text engine, shaped runs and the run coverage rasterizer —
// replaces this blanket re-export as each module lands. A shaped run leaves this
// crate as plain data (glyph indices, advances, offsets), never as a DirectWrite
// struct, so whichever crate draws it assembles the FFI struct from its own bindings.
pub use bindings::*;
pub use windows_core::Result;
