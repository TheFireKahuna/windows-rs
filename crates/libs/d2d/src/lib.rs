#![doc = include_str!("../readme.md")]

#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments
)]
mod bindings;

// The safe surface — device, drawing session, geometry, brushes, bitmaps — replaces
// this blanket re-export as each module lands. Until then the generated bindings are
// the crate's public surface, which is what lets sibling crates be written against
// it. A COM object reaching this crate from another generated crate arrives as
// `&impl Interface` and is cast on the way in; no generated struct crosses a crate
// boundary.
pub use bindings::*;
pub use windows_core::Result;
