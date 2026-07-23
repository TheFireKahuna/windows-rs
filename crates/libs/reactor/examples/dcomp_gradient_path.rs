//! Gradient path layers on the DirectComposition backend: a response curve whose
//! UNDERFILL fades down the plot while its LINE colours along it.
//!
//! The two ramps are the point. A path's fill and stroke are independent layers
//! with independent colour sources, so each carries its own stops and its own
//! axis — an area fading away from the line, and a line coloured by where it sits
//! on the x axis. Sharing one ramp would force one reading on both.
//!
//! Everything here is retained, and nothing about either ramp is rasterized: the
//! geometry is a `CompositionPathGeometry`, and each ramp lives in compositor
//! gradient brushes carrying ALPHA over flat FP16 colour sources. The underfill
//! is one hue fading, so it is one gradient mask over one FP16 cell; the line
//! changes hue, so it is a staircase of constant-colour layers whose alphas
//! partition — source-over between them IS the interpolation. The app is idle at
//! rest and a resize costs no draw at all.
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_gradient_path

fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    /// Plot box, in DIPs.
    const W: f64 = 520.0;
    const H: f64 = 220.0;
    /// Samples along the curve. Enough that the spline reads smooth at this width.
    const N: usize = 160;

    /// A bell in log-x, the shape a peaking EQ band puts on a response plot.
    fn bell(x: f64, centre: f64, width: f64, gain: f64) -> f64 {
        let t = (x - centre) / width;
        gain * (-t * t).exp()
    }

    /// The curve's y at fraction `t` across the box: two peaks and a dip, in dB,
    /// mapped onto the plot's ±12 dB window.
    fn curve_y(t: f64) -> f64 {
        let db = bell(t, 0.22, 0.10, 7.5) + bell(t, 0.55, 0.07, -5.0) + bell(t, 0.82, 0.12, 4.0);
        H * 0.5 - (db / 12.0) * (H * 0.5 - 12.0)
    }

    fn app(_cx: &mut RenderCx) -> Element {
        // The line, as a polyline over the plot box. Local DIPs: the geometry is
        // the host's own coordinates, so the node needs no origin arithmetic.
        let line = ShapePath::with_capacity(N)
            .polyline((0..N).map(|i| {
                let t = i as f64 / (N - 1) as f64;
                (t * W, curve_y(t))
            }))
            .build();

        // The same run, closed down to the baseline — a fillable area rather than
        // an open line. Two geometries because the two layers describe two shapes.
        let mut area = ShapePath::with_capacity(N + 3).move_to(0.0, H);
        for i in 0..N {
            let t = i as f64 / (N - 1) as f64;
            area = area.line_to(t * W, curve_y(t));
        }
        let area = area.line_to(W, H).close().build();

        let accent = Color::rgb(0x38, 0xBD, 0xF8);

        // The underfill: vertical, opaque at the line and gone by the baseline.
        // `fill_gradient` is already this axis — an area fill under a curve fades
        // DOWN — so it needs no axis stated.
        let underfill = Shape::path(area)
            .fill_gradient(vec![
                (0.0, Color::rgba(0x38, 0xBD, 0xF8, 0x66)),
                (1.0, Color::rgba(0x38, 0xBD, 0xF8, 0x00)),
            ])
            .width(W)
            .height(H)
            .canvas_left(0.0)
            .canvas_top(0.0);

        // The line: horizontal, so the colour tracks the x axis. On a real plot
        // that axis is frequency, and this is what "colour by frequency" means —
        // the ramp is a raster over the BOX, so a curve that doubles back still
        // reads left to right.
        let response = Shape::path(line)
            .stroke(accent)
            .stroke_thickness(2.5)
            .stroke_gradient(vec![
                (0.00, Color::rgb(0x34, 0xD3, 0x99)),
                (0.35, Color::rgb(0x38, 0xBD, 0xF8)),
                (0.70, Color::rgb(0xA7, 0x8B, 0xFA)),
                (1.00, Color::rgb(0xFB, 0x71, 0x85)),
            ])
            .glow(Color::rgba(0x38, 0xBD, 0xF8, 0x88), 7.0)
            .width(W)
            .height(H)
            .canvas_left(0.0)
            .canvas_top(0.0);

        // The 0 dB reference, so the fill has something to read against.
        let baseline = Shape::rectangle()
            .fill(Color::rgba(0xFF, 0xFF, 0xFF, 0x14))
            .width(W)
            .height(1.0)
            .canvas_left(0.0)
            .canvas_top(H * 0.5);

        let layers: Vec<Element> = vec![baseline.into(), underfill.into(), response.into()];
        let plot: Element = Canvas::new(layers).width(W).height(H).into();

        let card = vstack((
            text_block("Gradient path layers").font_size(22.0).semibold(),
            text_block("fill ramp runs DOWN · stroke ramp runs ACROSS")
                .font_size(13.0)
                .foreground(Color::rgb(0x9A, 0x9A, 0xA2)),
            plot,
        ))
        .spacing(14.0);

        border(card)
            .background(Color::rgb(0x18, 0x18, 0x1C))
            .corner_radius(14.0)
            .padding(Thickness::uniform(24.0))
            .margin(Thickness::uniform(28.0))
            .into()
    }

    DCompHost::render("Gradient path layers", app)
}
