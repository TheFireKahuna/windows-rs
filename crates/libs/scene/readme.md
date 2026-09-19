## Windows Scene

Windows Scene is the retained composition tree: what a user interface draws, what can be touched,
and how it moves. It knows nothing about widgets — its input is a patch of sprite declarations and
channel bindings, and its output is a live visual tree, a flat hit array and tracker state.

Five properties define the surface:

**The alphabet is closed.** A leaf is one sprite carrying a [`Mask`] (alpha — a rounded box, a
shaped run, a path, or none) and a [`Paint`] (colour — a flat radiance, a ramp, a captured subtree,
or a buffer the app presents itself). Everything else is a channel: an [`Op::Bind`] naming one of
thirty-two [`Prop`] rows. The crate holds a fixed set of kinds rather than a cross-product of kinds
and properties.

**One channel down, one channel up.** A [`SinkPatch`] of `Copy` ops over typed side-buffers carries
one pass of app-thread decisions to [`Scene::apply`]; a [`SceneEvent`] carries a tracker report, a
completion or a device event back. Solved layout crosses in neither direction: it becomes bind and
hit ops inside the patch. [`Scene`] is `!Send` and owns every composition object; nothing in the
patch is thread-affine, which a `const` assertion proves.

**The display is stated at every use, never held.** How many pixels a DIP is and how authored light
reaches the screen belong to the window and its monitor, so they arrive as an [`Env`] at every
operation rather than being pushed in and cached. Both halves are handed the same value, so the
grid layout snaps to and the grid the rasters are built for cannot disagree.

**Nothing continuous is driven by the CPU.** A property is written at an event, animated by the
compositor, or bound to an [`InteractionTracker`](windows_composition::InteractionTracker)
expression, and there is no fourth form. A window whose content is not changing publishes nothing at
all, because publishing happens when a pass ends and nothing asks for a pass.

**Colour above the draw choke is scene-referred light.** Sinks carry
[`Radiance`](windows_color::Radiance); the display transform is applied once, inside the cell that
rasterizes it, and the compositor's own 8-bit brushes only ever receive alpha.

This crate draws what it is told, when it is told. Deciding *what* to draw belongs to a widget layer
above it; the frame clock that decides *when* belongs to the window
([`Pacer`](windows_window::Pacer)); and content that changes without user input belongs in a
presentation region rather than here.
