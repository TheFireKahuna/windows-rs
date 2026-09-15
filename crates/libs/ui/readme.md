## Windows UI

Windows UI is the application-facing framework layer: signals and structure, the widget set and its
layout classes, overlays, the pointer stack and gesture recognition, the rotary controller, text
services (TSF) and UI Automation providers.

Pointer input, wheel, keyboard focus order, overlay dismiss and `ElementProviderFromPoint` all
resolve through the same z-ordered hit array, so there is one hit-test authority and no parallel
path.

Compositor objects, recognisers, trackers, text stores and automation providers are reached only
through a front-thread handle that is neither `Send` nor `Sync`, so an app-thread closure that
captures one does not compile.

Components are ordinary functions taking `&mut build::Ui`. Constructors write retained
records immediately; container closures declare their children synchronously. Keep an
element's `.id()` only when a later update needs its generation-checked handle.

```rust
use windows_ui::{build::Ui, widget};

fn actions(ui: &mut Ui<'_>) {
    ui.row(|ui| {
        widget::button(ui, "Reset").on_click(|| {});
        widget::label(ui, "Ready");
    });
}
```

`driver::UiRuntime::run` receives the startup declaration callback and owns root
creation and teardown. There is no public mount step or temporary element tree.
