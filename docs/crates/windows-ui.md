# windows-ui

> The application-facing framework layer: signals and structure, widgets, input, overlays, text services and UI Automation.

- Not published to crates.io
- [Getting started](../../crates/libs/ui/readme.md)
- [Source](https://github.com/microsoft/windows-rs/tree/master/crates/libs/ui)

`windows-ui` reads the input a window opts into, recognises gestures, drives the rotary controller
and the touch keyboard, hosts a TSF text store, and answers UI Automation.

Pointer input, wheel, keyboard focus order, overlay dismiss and `ElementProviderFromPoint` all
resolve through one z-ordered hit array, so there is a single hit-test authority and no parallel
path that can disagree with it.

Compositor objects, recognisers, trackers, text stores and automation providers are reached only
through a front-thread handle that is neither `Send` nor `Sync`, so an app-thread closure that
captures one does not compile.

Creating the window and opting it into pointer input belong to [`windows-window`](windows-window.md);
this crate reads what those opt-ins deliver.
