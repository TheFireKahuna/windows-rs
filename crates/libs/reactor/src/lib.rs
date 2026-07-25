#![doc = include_str!("../readme.md")]
#![allow(missing_docs)]

// One backend per build. Each selects a composition stack, and
// `windows-composition` hosts exactly one stack per build, so the two backends
// inherit that crate's mutual exclusion rather than working around it.
#[cfg(all(not(feature = "winui-backend"), not(feature = "dcomp-backend")))]
compile_error!(
    "enable exactly one backend: the `winui-backend` feature (default) or the `dcomp-backend` feature"
);
#[cfg(all(feature = "winui-backend", feature = "dcomp-backend"))]
compile_error!(
    "the `winui-backend` and `dcomp-backend` backends are mutually exclusive; enable only one"
);

#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    // Whole WinRT enums are pulled by value (e.g. only one `DirectXPixelFormat`
    // variant of ~190 is used), so generated bindings always carry unused variants.
    dead_code,
    clippy::upper_case_acronyms,
    clippy::missing_transmute_annotations
)]
mod bindings;

// System (Windows.UI.Composition) bindings for the self-hosted DirectComposition
// HDR backend. Separate module so its flat short names don't collide with the
// lifted Microsoft.UI.Composition types in `bindings`.
#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::upper_case_acronyms,
    clippy::missing_transmute_annotations,
    clippy::useless_transmute,
    clippy::too_many_arguments
)]
mod system_bindings;

#[cfg(feature = "winui-backend")]
mod app;
#[cfg(feature = "winui-backend")]
mod app_shim;
mod backend;
#[cfg(feature = "winui-backend")]
mod bootstrap;
mod canvas_bridge;
mod color;
mod diagnostics;
mod drag;
mod element;
mod engine;
mod fault;
mod generated;
mod gesture;
mod hooks;
mod host;
mod interaction;
mod motion;
mod reconciler;
mod style;
mod widget;
mod widgets;

#[cfg(feature = "winui-backend")]
pub use app::*;
pub use backend::*;
pub use gesture::{ActionSlot, GestureEvent, GestureInterest, GestureOutcome};
pub use motion::reduced_motion;
// The composition-object and property-write counters, re-exported so a consumer
// can read them without taking a direct dependency on the composition crate —
// they pair with `composition_census`, which prints them alongside a walk of the
// live visual tree.
#[cfg(feature = "dcomp-backend")]
pub use windows_composition::{census as composition_counters, Census, OverdrawKinds};
#[cfg(feature = "dcomp-backend")]
pub use windows_composition::reset_census as reset_composition_counters;
pub use color::Color;
pub use bindings::AutomationHeadingLevel;
pub use bindings::AutomationLiveSetting;
pub use bindings::CommandBarDefaultLabelPosition;
pub use bindings::DispatcherQueuePriority;
pub use bindings::FlyoutPlacementMode;
pub use bindings::HorizontalAlignment;
pub use bindings::InfoBarSeverity;
pub use bindings::NavigationViewPaneDisplayMode;
pub use bindings::Orientation;
pub use bindings::PasswordRevealMode;
pub use bindings::ScrollBarVisibility;
pub use bindings::ScrollingScrollBarVisibility;
pub use bindings::Stretch;
pub use bindings::Symbol;
pub use bindings::TeachingTipPlacementMode;
pub use bindings::TextWrapping;
pub use bindings::Thickness;
pub use bindings::TreeViewSelectionMode;
pub use bindings::VerticalAlignment;
pub use bindings::VirtualKey;
pub use bindings::VirtualKeyModifiers;
#[cfg(feature = "winui-backend")]
pub use bootstrap::*;
pub use canvas_bridge::{
    CanvasImageSource, CanvasSwapChain, DrawContext, animated_canvas, animated_canvas_with_device,
};
pub use drag::*;
pub use element::*;
pub use engine::*;
pub use fault::Fault;
pub use hooks::*;
pub use host::*;
pub use interaction::*;
pub use reconciler::*;
pub use style::*;
pub use widget::*;
pub use widgets::*;
pub use windows_core::{Error, Interface, Result};
pub use windows_time::{DateTime, TimeSpan};
