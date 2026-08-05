#![doc = include_str!("../readme.md")]

#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::too_many_arguments
)]
mod bindings;

// The signal graph, the structural combinators over it, and the role layer a widget
// resolves its appearance through. None of the three contains `unsafe`: the graph is an
// arena of generational indices, and the only Windows surface any of it needs is one
// waitable event, owned by `windows-window`.
#[deny(unsafe_code)]
pub mod role;
#[deny(unsafe_code)]
pub mod signal;
#[deny(unsafe_code)]
pub mod structure;

// Measures what the modules below claim about allocation. Outside their `deny(unsafe_code)`
// because a global allocator is `unsafe` by definition, and test-only because it exists to
// answer one question the type system cannot.
#[cfg(test)]
mod counting;

// The authoring surface: the build arena and the element, the length vocabulary and the
// style presets, and the widget seeds over both. No `unsafe`, and no Windows surface at
// all — it declares into `windows-scene` and resolves through `role`, and both of those
// are somebody else's problem by the time a widget runs.
#[deny(unsafe_code)]
pub mod build;
#[deny(unsafe_code)]
pub mod layout;
#[deny(unsafe_code)]
pub mod widget;

// The window's own three commands. Two functions over the hit array and the control table
// the layers above already own, which is why a title bar is a declaration here and not an
// implementation in every application that draws one.
#[deny(unsafe_code)]
pub mod caption;

// Start-up and the frame. The ordered tick and the five process-wide installs, for the same
// reason the caption is here: every application on this stack needs exactly one of each, the
// order is a correctness rule at every seam, and a second transcription of either is a second
// chance to get it wrong. No `unsafe` — every Windows surface it reaches belongs to the
// window, the scene or the router, and it holds none of them open.
#[deny(unsafe_code)]
pub mod driver;

// Flyouts, popups, menus and tooltips. Positioned against an anchor rather than by a
// parent's layout, above everything, with a defined way to go away — and no `unsafe` and no
// Windows surface, because every mechanism it needs is one the layers below already own: a
// detached root in `windows-scene`, a blocker entry in the one hit array, a focus scope in
// the router, and a scoped batch for a delay.
#[deny(unsafe_code)]
pub mod overlay;

// The pointer stack and what it feeds. The gesture layer and the drag policy contain no
// `unsafe` either — the recognisers are the platform's and every call into them is a safe
// projection — so only `input` and `rotary` reach the raw surface, and both are confined to
// reading a pointer and creating a controller.
#[deny(unsafe_code)]
pub mod gesture;
pub mod input;
pub mod rotary;

// UI Automation. Reaches the raw surface — a provider is a COM object and a `VARIANT` is a
// union — but nothing it touches above that boundary is: the tree it publishes is plain
// data and every mutable field beside it is an atomic, which is what lets a client read it
// from its own thread without the window's thread being involved at all.
pub mod uia;

mod front;

pub use front::FrontHandle;

// The rest of the safe surface — the widget set, overlays, UI Automation and text services —
// replaces this blanket re-export as each module lands.
pub use bindings::*;
pub use windows_core::Result;
