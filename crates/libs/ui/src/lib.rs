#![doc = include_str!("../readme.md")]

#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments
)]
mod bindings;

// The safe surface — signals, widgets, the pointer stack, overlays, UI Automation
// and text services — replaces this blanket re-export as each module lands.
pub use bindings::*;
pub use windows_core::Result;
