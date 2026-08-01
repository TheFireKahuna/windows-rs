#![doc = include_str!("../readme.md")]

// `dead_code` is expected rather than allowed, and is temporary: the filter carries
// the caption, DPI, display-capability and dispatcher-queue surface that this crate's
// wrapper has yet to grow, and `--dead-code` exists precisely to report bindings
// nothing uses. When the wrapper consumes them, `expect` starts warning on its own —
// which is the signal to delete this line and let the lint prune whatever is left.
#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::upper_case_acronyms
)]
mod bindings;
mod event;
mod pace;
mod window;

pub use event::Event;
pub use pace::{Pacer, PacerHealth, Tick, WM_APP_FRAME, Wake};
pub use window::{Window, WindowBuilder, pump, quit, run, run_with};
pub use windows_core::Result;
