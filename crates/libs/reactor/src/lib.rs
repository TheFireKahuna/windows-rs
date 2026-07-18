#![doc = include_str!("../readme.md")]
#![allow(missing_docs)]

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
    clippy::missing_transmute_annotations
)]
mod system_bindings;

#[cfg(feature = "winui-backend")]
mod app;
#[cfg(feature = "winui-backend")]
mod app_shim;
mod backend;
#[cfg(feature = "winui-backend")]
mod bootstrap;
mod color;
mod diagnostics;
mod drag;
mod element;
mod engine;
mod fault;
mod generated;
mod hooks;
mod host;
mod interaction;
mod reconciler;
mod style;
mod surface;
mod widget;
mod widgets;

#[cfg(feature = "winui-backend")]
pub use app::*;
pub use backend::*;
pub use surface::{request_surface, PendingSurface, SurfaceDevice, SurfaceToken};
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
