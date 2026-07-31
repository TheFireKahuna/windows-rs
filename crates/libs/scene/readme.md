## Windows Scene

Windows Scene is the retained composition tree: what a user interface draws, where it sits, what
can be touched, and how it moves. It knows nothing about widgets — its input is sprites and layout
styles, and its output is a live visual tree, a flat hit array and tracker state.

Five things shape the whole surface:

**The alphabet is closed.** A leaf is one `SpriteVisual` carrying a `Mask` (alpha — a rounded box,
a shaped run, a path, or none), a `Paint` (colour — a flat radiance, a ramp, a captured subtree, or
a buffer the app presents itself) and an `Xform`. Everything drawable is a composition of those,
which is what keeps the crate a fixed size rather than a cross-product of kinds and properties.

**There are two root types, one per thread.** [`Model`] is `Send`, owns no COM, and does the
measuring, solving and emitting; [`Scene`] is `!Send` and owns every composition object. The only
channel down is a [`SinkPatch`] of `Copy` ops over typed side-buffers, and the only channel up is a
[`SceneEvent`]. The `Send` half — snapping, the responsive classification, hit-array construction —
is unit-testable with no device at all.

**The display is stated at every use, never held.** How many pixels a DIP is and how authored light
reaches the screen belong to the window and its monitor, so they arrive as an [`Env`] on
[`Model::flush`] and [`Scene::apply`] rather than being pushed in and cached. Both halves are handed
the same value, so the grid layout snaps to and the grid the rasters are built for cannot disagree —
and a display that moved cannot be applied against stale content, because there is no way to apply
without saying what display it is for.

**Nothing continuous is driven by the CPU.** A property is written at an event, animated by the
compositor, or bound to an [`InteractionTracker`](windows_composition::InteractionTracker)
expression, and there is no fourth form. A window whose content is not changing costs no publishing
at all, because publishing happens when a tick ends and nothing asks for a tick.

**Colour above the draw choke is scene-referred light.** Sinks carry
[`Radiance`](windows_color::Radiance); the display transform is applied once, inside the cell that
rasterizes it, and the compositor's own 8-bit brushes only ever receive alpha.

It draws what it is told, when it is told. Deciding what to draw belongs to a widget layer above
it; the frame clock that decides *when* belongs to the window
([`Pacer`](windows_window::Pacer)); and content that changes without user input belongs in a
presentation region rather than here.
