## Windows Composition

Windows Composition wraps the retained-mode Windows composition engine. Choose one Cargo feature:
`system` (default) uses `Windows.UI.Composition` and hosts a visual tree in a window; `reactor` uses
`Microsoft.UI.Composition` and hosts it in a WinUI 3 element through the
[`windows-reactor`][reactor-guide] bridge.

* [Samples](https://github.com/microsoft/windows-rs/tree/master/crates/samples)
* [Composition
  guide](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-composition.md)

```rust,no_run
use windows_composition::*;

fn build(compositor: &Compositor) -> SpriteVisual {
    let visual = compositor.create_sprite_visual();
    visual.set_size(200.0, 120.0);

    let brush = compositor.create_color_brush(Color::rgb(0, 120, 215));
    visual.set_brush(&brush);
    visual
}
```

Core types: `Compositor`, `Visual`, `ContainerVisual`, `SpriteVisual`, `ShapeVisual`, composition
brushes and shapes, and key-frame animations. On the system stack it also carries the pieces a
retained visual tree changes without being repainted: clips and paths, mask and gradient brushes,
drop shadows, drawing and virtual surfaces, springs and compositor-evaluated expressions, property
sets, and `InteractionTracker`, which drives motion from a manipulation while the application
thread stays idle. To show a tree, create a `DispatcherQueueController` on the thread, create a
`Compositor`, then host the root visual in a window with
`Compositor::create_desktop_window_target`, which takes a [`windows-window`][window-guide] `Window`.
See the [composition
guide](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-composition.md) for
the API and hosting options.

[reactor-guide]: https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-reactor.md
[window-guide]: https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-window.md
