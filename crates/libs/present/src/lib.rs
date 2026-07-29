#![doc = include_str!("../readme.md")]

#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments
)]
mod bindings;

// The safe surface — the presentation device, its buffer pool and its regions —
// replaces this blanket re-export as each module lands. A buffer's texture reaches
// a drawing crate as `&impl Interface` and is cast once when the pool is built,
// never per frame.
pub use bindings::*;
pub use windows_core::Result;
