//! Demo of the windows-canvas `virtual_surface_image` bridge: a
//! `VirtualSurfaceImageSource` whose content is far larger than the viewport.
//!
//! The framework virtualizes the surface and calls the `draw` closure for each
//! region as it scrolls into view. The closure keys its work off
//! [`DrawContext::update_rect`](windows_canvas::DrawContext::update_rect) — the
//! dirty region in surface-local DIPs — so panning the host `ScrollViewer`
//! reveals freshly drawn content with per-region coordinate labels.

use windows_canvas::{ColorF, Rect, TextAlignment, TextFormat, Vector2};
use windows_reactor::*;

/// Logical content size of the virtual surface, in DIPs.
const CONTENT: f32 = 2000.0;
/// Spacing of the background grid, in DIPs.
const CELL: f32 = 100.0;

/// Sample page: a 2000x2000 DIP canvas drawn on demand through
/// [`windows_canvas::virtual_surface_image`], hosted in a `ScrollViewer` so it
/// can be panned in both directions.
pub fn canvas_virtual_sample(_: &(), cx: &mut RenderCx) -> Element {
    let canvas = windows_canvas::virtual_surface_image(cx, CONTENT, CONTENT, |ctx| {
        let update = ctx.update_rect();

        // Paint the region background so newly revealed tiles are opaque.
        if let Ok(bg) = ctx.create_solid_brush(ColorF::DARK_SLATE_BLUE) {
            ctx.fill_rect(&update, &bg);
        }

        // Grid lines, clipped to the region being (re)drawn for efficiency.
        if let Ok(line) = ctx.create_solid_brush(ColorF::rgb(0.3, 0.4, 0.6)) {
            let first_x = (update.left / CELL).floor() * CELL;
            let mut x = first_x;
            while x <= update.right {
                ctx.draw_line(
                    Vector2::new(x, update.top),
                    Vector2::new(x, update.bottom),
                    &line,
                    1.0,
                );
                x += CELL;
            }

            let first_y = (update.top / CELL).floor() * CELL;
            let mut y = first_y;
            while y <= update.bottom {
                ctx.draw_line(
                    Vector2::new(update.left, y),
                    Vector2::new(update.right, y),
                    &line,
                    1.0,
                );
                y += CELL;
            }
        }

        // Label each grid intersection with its coordinates, so panning to a
        // different region shows visibly different content.
        if let (Ok(format), Ok(text_brush)) = (
            TextFormat::new("Segoe UI", 12.0).map(|f| f.with_alignment(TextAlignment::Leading)),
            ctx.create_solid_brush(ColorF::rgb(0.7, 0.8, 1.0)),
        ) {
            let first_x = (update.left / CELL).floor() * CELL;
            let first_y = (update.top / CELL).floor() * CELL;
            let mut gy = first_y;
            while gy <= update.bottom {
                let mut gx = first_x;
                while gx <= update.right {
                    let label = format!("{},{}", gx as i32, gy as i32);
                    let rect = Rect::from_xywh(gx + 4.0, gy + 2.0, CELL, 16.0);
                    ctx.draw_text(&label, &format, &rect, &text_brush);
                    gx += CELL;
                }
                gy += CELL;
            }
        }
    });

    grid((
        Element::from(
            text_block("VirtualSurfaceImage (2000x2000 DIPs) — scroll to pan:").grid_row(0),
        ),
        Element::from(
            scroll_viewer(canvas)
                .horizontal_scroll_bar_visibility(ScrollBarVisibility::Visible)
                .vertical_scroll_bar_visibility(ScrollBarVisibility::Visible)
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
