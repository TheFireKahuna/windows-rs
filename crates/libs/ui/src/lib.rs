#![doc = include_str!("../readme.md")]

#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::too_many_arguments
)]
mod bindings;

// The signal graph, the structural combinators over it, and the role layer a widget
// resolves its appearance through. The graph is an arena of generational indices, and the
// only Windows surface the three need is one waitable event owned by `windows-window`.
#[deny(unsafe_code)]
pub mod role;
#[deny(unsafe_code)]
pub mod signal;
#[deny(unsafe_code)]
pub mod structure;

// Counts allocations, so a zero-allocation path can be measured. A global allocator is
// `unsafe` to implement, so this sits outside the `deny(unsafe_code)` of the modules it
// measures. Test-only; nothing here is compiled into the library.
#[cfg(test)]
mod counting;

// The authoring surface: the element, the length vocabulary and the style presets, and the
// widget seeds over both. It declares into `windows-scene` and resolves through `role`, and
// reaches no Windows surface of its own.
#[deny(unsafe_code)]
pub mod build;
#[deny(unsafe_code)]
pub mod layout;
pub mod text_input;
#[deny(unsafe_code)]
pub mod widget;

// The window's three commands: minimize, maximize and close. Two functions over the hit array
// and the control table the layers above already own, so an application declares a title bar
// rather than implementing one. An application names a command through `Element::caption`; the
// two answers here are the driver's.
#[deny(unsafe_code)]
pub(crate) mod caption;

// Start-up and the frame: the ordered tick and the six process-wide installs. An
// application takes one of each, and the order within the tick is a correctness rule at
// every seam it crosses. Every Windows surface it reaches belongs to the window, the scene
// or the router, and it holds none of them open.
#[deny(unsafe_code)]
pub mod driver;

// The thread seams: single-slot mailboxes over buffers allocated once, and the doorbells the
// input, scene and app threads park on. Nothing here allocates once the buffers exist, and no
// thread waits on another. A mailbox hands a box across threads through one atomic pointer,
// which is why this is not under `deny(unsafe_code)`.
pub(crate) mod seam;

// Presentation regions: the one per-frame path, declared as a node. The buffers, the thread
// and the renderer trait belong to `windows-present` and the brush to `windows-scene`; what
// is here is the declaration and the lifecycle that follows the node. Binding a surface
// handle is raw, which is why this is not under the driver's `deny(unsafe_code)`.
pub mod present;
pub mod correlation;

// Flyouts, popups, menus and tooltips: positioned against an anchor rather than by a
// parent's layout, drawn above everything, and dismissed on a defined trigger. Each
// mechanism belongs to a layer below — a detached root in `windows-scene`, a blocker entry
// in the one hit array, a focus scope in the router, and a scoped batch for a delay.
#[deny(unsafe_code)]
pub mod overlay;

// The pointer stack and what it feeds. `gesture` drives the platform recognisers through
// safe projections; the raw surface is confined to `input`, which reads a pointer, and
// `rotary`, which creates a controller.
#[deny(unsafe_code)]
pub mod gesture;
pub mod input;
pub mod rotary;

// UI Automation. A provider is a COM object and a `VARIANT` is a union, so this reaches the
// raw surface. The tree it publishes is plain data and every mutable field beside it is an
// atomic, so a client reads it from its own thread without involving the window's thread.
#[expect(non_upper_case_globals, reason = "the property and pattern ids are matched by their bound names")]
pub mod uia;
pub(crate) mod wheel;

mod front;

pub use front::FrontHandle;

// The generated Windows bindings this crate's public types are expressed in.
pub use bindings::*;
pub use windows_core::Result;

pub use build::{Element, Node, Ui};
