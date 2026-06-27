//! Demo of the windows-canvas `surface_image` bridge: a `SurfaceImageSource`
//! hosted in an `Image`, drawn with high-level canvas primitives instead of raw
//! Direct2D calls.
//!
//! Unlike `surface_image_source.rs` (which drives `SurfaceImageSource` and
//! Direct2D by hand against the app-wide shared device), `surface_image` owns
//! its own `GpuDevice`, tracks the element size and window DPI, and re-runs the
//! `draw` closure whenever either changes or the device is lost.

use windows_canvas::{ColorF, Ellipse, Rect, TextAlignment, TextFormat, Vector2};
use windows_reactor::*;

/// Sample page: canvas primitives (clear, ellipse, text) rendered into a
/// `SurfaceImageSource` via the reactive [`windows_canvas::surface_image`]
/// helper, which keeps the surface correctly sized and DPI-crisp.
pub fn canvas_surface_sample(_: &(), cx: &mut RenderCx) -> Element {
    let canvas = windows_canvas::surface_image(cx, |ctx| {
        // Background.
        ctx.clear(ColorF::CORNFLOWER_BLUE);

        // A gold ellipse centered in, and sized from, the current surface.
        let center = Vector2::new(ctx.width / 2.0, ctx.height / 2.0);
        let radius_x = (ctx.width / 2.0 - 24.0).max(1.0);
        let radius_y = (ctx.height / 2.0 - 24.0).max(1.0);
        let gold = ColorF::rgb(1.0, 0.78, 0.0);
        if let Ok(brush) = ctx.create_solid_brush(gold) {
            ctx.fill_ellipse(&Ellipse::new(center, radius_x, radius_y), &brush);
        }

        // A centered label, proving text renders into the surface too.
        if let (Ok(format), Ok(text_brush)) = (
            TextFormat::new_bold("Segoe UI", 24.0).map(|f| f.with_alignment(TextAlignment::Center)),
            ctx.create_solid_brush(ColorF::WHITE),
        ) {
            let rect = Rect::from_xywh(0.0, ctx.height / 2.0 - 16.0, ctx.width, 32.0);
            ctx.draw_text("SurfaceImage", &format, &rect, &text_brush);
        }
    });

    // The canvas needs bounds: give it a stretched, star-sized grid cell beneath
    // a heading.
    grid((
        Element::from(text_block("Image backed by a windows-canvas SurfaceImage:").grid_row(0)),
        Element::from(
            border(canvas)
                .border_thickness(Thickness::uniform(1.0))
                .margin(Thickness {
                    left: 0.0,
                    top: 8.0,
                    right: 0.0,
                    bottom: 0.0,
                })
                .grid_row(1),
        ),
    ))
    .rows([GridLength::Auto, GridLength::STAR])
    .columns([GridLength::STAR])
    .margin(Thickness::uniform(16.0))
    .into()
}
