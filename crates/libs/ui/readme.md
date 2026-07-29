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
