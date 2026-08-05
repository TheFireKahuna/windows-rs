#![doc = include_str!("../readme.md")]

// `dead_code` covers what a generated binding brings in with a named type rather than on its
// own: the members of an enum family, and the methods of an interface. No whole binding here
// is unused.
#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::upper_case_acronyms
)]
mod bindings;
mod caption;
mod display;
mod dpi;
mod event;
mod feedback;
mod handoff;
mod pace;
mod visibility;
mod window;

// Two thread-level primitives a window's lifetime implies but does not own, kept as modules
// so their names read at the call site: `clock::wait_for_frame`, `qos::set`. Public because
// the threads that need them are not always the window's own — a present or producer thread
// waits on the same clock and tags its own scheduling class — and one decoder for the clock's
// slots and one encoder for the QoS class keep those threads agreeing with the window's.
pub mod clock;
pub mod qos;

/// What every query on a closed window answers with, so a caller reporting the condition
/// raises the error this crate already raises for it rather than a second one.
pub use bindings::E_HANDLE;
pub use caption::{
    BorderColor, CaptionButton, CaptionButtons, CaptionHit, CaptionSpec, CaptionState,
    CornerPreference,
};
pub use display::Subscription;
pub use dpi::Metrics;
pub use event::Event;
pub use feedback::{Feedback, FeedbackPolicy};
pub use handoff::Handoff;
pub use pace::{Pacer, PacerHealth, Tick, WM_FRAME, Wake};
pub use visibility::{Visibility, Watch};
pub use window::{MoveSize, Window, WindowBuilder, pump, quit, run};
pub use windows_core::Result;
