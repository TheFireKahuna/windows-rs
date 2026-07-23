//! Scratch probe: one full-box path fill carrying a single-hue vertical ramp,
//! over a black card, so the composited ramp can be read row by row with no
//! curve, glow or baseline anywhere near it.
//!
//! Run, then probe the raw FP16 frame with `guishot --pid <pid> --probe X,Y,1,1`.

fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    const W: f64 = 300.0;
    const H: f64 = 400.0;

    fn app(_cx: &mut RenderCx) -> Element {
        // A plain rectangle as a PATH, so it takes the gradient path layer.
        let area = ShapePath::with_capacity(5)
            .move_to(0.0, 0.0)
            .line_to(W, 0.0)
            .line_to(W, H)
            .line_to(0.0, H)
            .close()
            .build();

        let fill = Shape::path(area)
            .fill(Color::rgba(0xFF, 0xFF, 0xFF, 0xFF))
            .fill_gradient(vec![
                (0.0, Color::rgba(0xFF, 0xFF, 0xFF, 0xFF)),
                (1.0, Color::rgba(0xFF, 0xFF, 0xFF, 0x00)),
            ])
            .width(W)
            .height(H)
            .canvas_left(0.0)
            .canvas_top(0.0);

        let plot: Element = Canvas::new(vec![Element::from(fill)]).width(W).height(H).into();

        border(plot)
            .background(Color::rgb(0x00, 0x00, 0x00))
            .padding(Thickness::uniform(0.0))
            .margin(Thickness::uniform(0.0))
            .into()
    }

    DCompHost::render("Ramp probe", app)
}
