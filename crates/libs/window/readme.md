## Windows Window

A top-level window and its message loop, for hosting content such as
[`windows-canvas`](https://crates.io/crates/windows-canvas) swap chains, WebView2 controllers, or
Direct2D/Direct3D rendering — without pulling in the full `windows` crate or hand-rolling
`windows-bindgen` build scripts.

You work with a safe Rust type and the raw `HWND` it exposes for interop; the only Win32 detail
in the API is the raw message stream `WindowBuilder::on_message` hands back, which is the escape
hatch for everything this crate does not cover. Beyond creation and the pump, it owns what
belongs to a window's lifetime rather than to a user interface — per-monitor DPI, the display's
colour capability, whether anything drawn can be seen, a title bar the application draws, and the
frame clock.

* [Samples](https://github.com/microsoft/windows-rs/tree/master/crates/samples)

```rust,no_run
use windows_window::*;

fn main() -> Result<()> {
    let window = Window::new("Hello")
        .size(800, 600)
        .on_resize(|width, height| {
            println!("resized to {width} x {height}");
        })
        .create()?;

    // `window.hwnd()` can be handed to windows-canvas, WebView2, Direct2D, etc.
    println!("created window {:?}", window.hwnd());

    run();
    Ok(())
}
```

`run()` blocks until a quit message is posted; `quit()` posts one, and destroying a window does
too unless [`WindowBuilder::quit_on_close`] says otherwise. `pump()` dispatches what is pending
without blocking, for a caller driving the loop while it waits on something else, and once it
answers `false` it keeps answering `false`.

For animation, take a `Pacer` from `Window::pacer()` and do the frame's work in the window's
`WM_FRAME` arm. It blocks on the compositor clock and posts once per composition frame while
something holds a `Tick`, so a window with nothing to do costs nothing and never wakes.

Nothing here polls. Every state this crate can be in is entered and left on an edge — a message,
a kernel event, or the compositor clock — and a thread with nothing to do is blocked rather than
looping. A producer on another thread parks on `Window::watch()`, which is that window's answer
to "can anything I draw be seen", with a wake of its own so a window can have as many watchers as
it has threads. `clock::wait_for_frame` and `qos::set` are the same two levers, public for a
thread that is not the window's own.
