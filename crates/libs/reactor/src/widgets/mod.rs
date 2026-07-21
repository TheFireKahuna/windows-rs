//! Built-in widget catalogue. One file per widget; each file owns the
//! widget struct, its builder methods, its `impl Widget`, any related
//! support types, and the convenience factory function (if any).

// Make the core types each widget module depends on visible via
// `use super::*;` inside the per-widget files.
pub use super::*;

/// Emit the boilerplate `kind`/`key`/`modifiers` accessors shared by
/// every `impl Widget for X`; `attached` falls through to the default
/// impl on the trait.
macro_rules! widget_header {
    ($kind:expr) => {
        fn kind(&self) -> ControlKind {
            $kind
        }
        fn key(&self) -> Option<&str> {
            self.key.as_deref()
        }
        fn modifiers(&self) -> &Modifiers {
            &self.modifiers
        }
    };
}
pub(crate) use widget_header;

/// Declare a widget submodule and re-export its public items.
macro_rules! widget_modules {
    ($($mod:ident),* $(,)?) => {
        $( pub mod $mod; )*
        $( pub use $mod::*; )*
    };
}

widget_modules! {
    auto_suggest_box,
    border,
    breadcrumb_bar,
    button,
    calendar_date_picker,
    calendar_view,
    canvas,
    check_box,
    color_picker,
    combo_box,
    command_bar,
    content_dialog,
    date_picker,
    drop_down_button,
    expander,
    flyout,
    grid,
    hyperlink_button,
    image,
    info_badge,
    info_bar,
    knob,
    list_box,
    menu_bar,
    meter,
    navigation_view,
    number_box,
    password_box,
    person_picture,
    pivot,
    progress_bar,
    progress_ring,
    radio_button,
    radio_buttons,
    rating_control,
    relative_panel,
    repeat_button,
    rich_edit_box,
    scroll_view,
    scroll_viewer,
    selector_bar,
    shape,
    slider,
    split_button,
    split_view,
    stack_panel,
    element_handle,
    pointer,
    subscription,
    surface_image_source,
    swap_chain_panel,
    tab_view,
    teaching_tip,
    text_block,
    text_box,
    time_picker,
    title_bar,
    toggle_button,
    toggle_switch,
    tree_view,
    viewbox,
    web_view2,
}

// Declared outside `widget_modules!` because it is the one widget that is not
// backend-agnostic: it hosts a lifted `Microsoft.UI.Composition` visual tree in
// a XAML element (`ElementCompositionPreview`, `XamlRoot`), so it exists only
// when the WinUI backend does. It is also the sole consumer of the lifted
// `windows-composition` stack outside `backend/winui`, and gating it is what
// lets that dependency be optional — a DComp-only build hosts the system stack
// instead, and the two stacks cannot coexist in one build.
#[cfg(feature = "winui-backend")]
pub mod composition_host;
#[cfg(feature = "winui-backend")]
pub use composition_host::*;

// The system-stack counterpart, and likewise not backend-agnostic: it hosts a
// host-owned `Windows.UI.Composition` drawing surface under a DComp node, so it
// exists only when the DComp backend does.
#[cfg(feature = "dcomp-backend")]
pub mod composition_surface;
#[cfg(feature = "dcomp-backend")]
pub use composition_surface::*;
