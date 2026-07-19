//! Repro / proof for the surface-fill fix: a `surface_painter` now fills the
//! width of its container the same way a `Shape::rectangle` does, because the
//! surface is hosted in a layout-driven `Border` (the reactor port of how Win2D's
//! `CanvasControl` hosts its surface in a `UserControl`) and the surface is sized
//! from the *host's* `SizeChanged`, not the content-sized inner `Image`.
//!
//! Before the fix, cases 2–4 were blank: size was tracked on the `Image`, which
//! adopts its (null) `Source`'s 0-width natural size and never reported a width.
//!
//! Run: `cargo run -p reactor_samples --example painter_factor_repro`

use windows_canvas::{ColorF, surface_painter};
use windows_reactor::*;

fn label(t: &str) -> Element {
    text_block(t).foreground(Color::rgb(220, 220, 220)).into()
}

/// `grid` with a single `Star` column and `Auto` row holding `child` — the
/// column hands the child the full available width.
fn star_col(child: Element) -> Element {
    grid((child.grid_row(0),))
        .columns([GridLength::Star(1.0)])
        .rows([GridLength::Auto])
        .into()
}

fn app(cx: &mut RenderCx) -> Element {
    // Each painter just clears to a flat color — enough to see its bounds.
    let blue = surface_painter(cx)
        .clear_color(ColorF::rgb(0.24, 0.47, 0.69))
        .draw(|ctx| eprintln!("DRAW case2 (min_height) at {}x{}", ctx.width, ctx.height));
    let green = surface_painter(cx)
        .clear_color(ColorF::rgb(0.35, 0.70, 0.45))
        .draw(|ctx| eprintln!("DRAW case3 (height) at {}x{}", ctx.width, ctx.height));
    let amber = surface_painter(cx)
        .clear_color(ColorF::rgb(0.85, 0.65, 0.25))
        .draw(|ctx| eprintln!("DRAW case4 (stretch) at {}x{}", ctx.width, ctx.height));

    vstack((
        label("1. Shape::rectangle (control) in grid[Star col] — baseline:"),
        star_col(
            Shape::rectangle()
                .fill(Color::rgb(60, 120, 175))
                .height(40.0)
                .into(),
        ),
        label("2. painter in grid[Star col], min_height(40) — fills width:"),
        star_col(blue.element().min_height(40.0)),
        label("3. painter in grid[Star col], height(40) — fills width, exact height:"),
        star_col(green.element().height(40.0)),
        label("4. painter in a stretched parent, min_height(40) — fills width:"),
        amber
            .element()
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .min_height(40.0),
    ))
    .spacing(8.0)
    .margin(Thickness::uniform(16.0))
    .into()
}

fn main() -> Result<()> {
    reactor_samples::run("Painter fill (fixed)", app)
}
