//! Scratch probe: can a **glow** carry more than one hue?
//!
//! `backend::dcomp::path_shape::GlowLayer` blurs a WHITE stroke as a pure alpha
//! generator and masks a single flat FP16 cell, so its halo is one hue by
//! construction. `backend::dcomp::gradient::RampSource` already produces a
//! multi-hue colour field as a `CompositionSurfaceBrush` (the captured
//! staircase) — exactly the type `CompositionMaskBrush::SetSource` demands. This
//! probe binds that brush into a glow's mask and reads what comes out.
//!
//! It is standalone on purpose: it builds its own compositor, window and FP16
//! sources rather than reaching into the backend, so it can be run and measured
//! without touching `path_shape.rs` or `gradient.rs`. The three cells are
//! deliberately the three points of comparison:
//!
//! * **A — flat glow.** The route the backend ships: blurred white ring alpha
//!   over one flat FP16 cell. The unclamped-ceiling control.
//! * **B — ramped glow.** The hypothesis: the same blurred ring alpha, its
//!   mask's `Source` swapped for the staircase capture.
//! * **C — bare staircase.** The colour field alone, painted straight onto a
//!   sprite with no mask at all. What B's colour would be if the mask stage were
//!   free.
//!
//! Every colour authored here is raw scRGB above SDR white (`LEVEL` = 3.5, where
//! 1.0 is 80 nits), because that is the only way a clamp is visible: a route
//! that clamps at 1.0 (effect graph) or 2.957 (compositor-carried colour) looks
//! perfectly fine in an 8-bit screenshot and is only caught by reading the raw
//! FP16 frame.
//!
//! Run, then probe the frame — the layout the run prints gives the rectangles:
//!
//! ```text
//! cargo run -p windows-reactor --example dcomp_glow_ramp_probe --features dcomp-backend --release
//! guishot --pid <pid> --keep-open --out glow.png --probe X,Y,W,H ...
//! ```

use windows_canvas::{ColorF, DrawingSession, GpuDevice, ID2D1DeviceContext, Matrix3x2, Rect};
use windows_composition::{
    AlphaMode, Brush, Color as UiColor, Compositor, CompositionBrush, CompositionGraphicsDevice,
    CompositionSurfaceBrush, ContainerVisual, DispatcherQueueController, MappingMode, PixelFormat,
    ShadowSource, SpriteVisual, Stretch, StrokeCap, BorderMode,
};
use windows_numerics::Vector2;
use windows_window::Window;

/// Client size, in the window's own pixels.
const WIN_W: i32 = 980;
const WIN_H: i32 = 360;
/// One cell's box. The ring is centred in it.
const CELL: f32 = 300.0;
/// Gap between cells and around them.
const PAD: f32 = 20.0;
/// Ring radius and stroke width, in the same space.
const RING_R: f32 = 100.0;
const RING_W: f32 = 10.0;
/// Blur sigma for the halo. Wide enough that the halo has a body to probe well
/// clear of the ring's own stroke.
const SIGMA: f32 = 14.0;
/// The scRGB level every hue is authored at — comfortably above SDR white (1.0)
/// and above the 2.957 ceiling a compositor-carried colour clamps to, so both
/// failure modes are legible in the numbers.
const LEVEL: f32 = 3.5;
/// The dim channel of a hue, so a "red" cell is red rather than red-or-nothing.
const DIM: f32 = 0.2;

fn hue_red() -> ColorF {
    ColorF::new(LEVEL, DIM, DIM, 1.0)
}
fn hue_green() -> ColorF {
    ColorF::new(DIM, LEVEL, DIM, 1.0)
}
fn hue_blue() -> ColorF {
    ColorF::new(DIM, DIM, LEVEL, 1.0)
}
fn hue_white() -> ColorF {
    ColorF::new(LEVEL, LEVEL, LEVEL, 1.0)
}

/// Everything a cell allocated, held so the compositor's property references
/// stay alive for the life of the window.
struct Cell {
    _visuals: Vec<SpriteVisual>,
    _brushes: Vec<CompositionBrush>,
    _keep: Vec<Box<dyn std::any::Any>>,
}

/// An FP16 (`Rgba16Float`) surface filled flat with `c`, as a Fill-stretch
/// brush — the same kind of app-allocated colour source
/// `parts::build_solid_surface` mints, authored here in raw scRGB with no
/// display map so the probe reads exactly what was written.
fn solid_fp16(
    gfx: &CompositionGraphicsDevice,
    comp: &Compositor,
    c: ColorF,
    keep: &mut Vec<Box<dyn std::any::Any>>,
) -> Option<CompositionSurfaceBrush> {
    const N: f32 = 8.0;
    let surface = gfx
        .create_drawing_surface_with_format(N, N, PixelFormat::Rgba16Float, AlphaMode::Premultiplied)
        .ok()?;
    let (ctx, (ox, oy)) = surface.begin_draw::<ID2D1DeviceContext>().ok()?;
    {
        let session =
            DrawingSession::from_borrowed_context(&ctx, Matrix3x2::translation(ox as f32, oy as f32));
        session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
        if let Ok(b) = session.create_solid_brush(c) {
            session.fill_rect(&Rect::from_xywh(0.0, 0.0, N, N), &b);
        }
    }
    surface.end_draw().ok()?;
    let brush = comp.create_surface_brush(&surface);
    brush.set_stretch(Stretch::Fill);
    keep.push(Box::new(surface));
    Some(brush)
}

/// A horizontal alpha ramp, measured in fractions of whatever visual it paints —
/// `gradient::ramp_brush`, restated locally.
fn alpha_ramp(comp: &Compositor, stops: &[(f32, f32)]) -> CompositionBrush {
    let b = comp.create_linear_gradient_brush();
    b.set_mapping_mode(MappingMode::Relative);
    b.set_line(Vector2::new(0.0, 0.0), Vector2::new(1.0, 0.0));
    b.set_alpha_stops(comp, stops);
    b.as_brush()
}

/// The multi-hue staircase of `gradient::RampSource::multi_hue`, rebuilt here:
/// an opaque base of the first hue, then one layer per later stop whose source
/// is a flat FP16 cell and whose mask ramps `0 -> 1` across that stop's segment
/// and HOLDS 1 after it, so source-over between the layers is the interpolation.
/// The stack is captured once — a capture carries COLOUR bit-accurately (it is
/// ALPHA it mangles), and inside the container alpha is 1 everywhere.
fn staircase(
    gfx: &CompositionGraphicsDevice,
    comp: &Compositor,
    hues: &[(f32, ColorF)],
    size: f32,
    keep: &mut Vec<Box<dyn std::any::Any>>,
    brushes: &mut Vec<CompositionBrush>,
) -> Option<CompositionSurfaceBrush> {
    let container = comp.create_container_visual();
    container.set_size(size, size);
    container.set_border_mode(BorderMode::Soft);

    let base = comp.create_sprite_visual();
    let base_src = solid_fp16(gfx, comp, hues[0].1, keep)?;
    base.set_brush(&base_src);
    base.set_relative_size_adjustment(Vector2::new(1.0, 1.0));
    brushes.push(base_src.as_brush());
    container.children().insert_at_top(&base);
    keep.push(Box::new(base));

    for pair in hues.windows(2) {
        let (from, to) = (pair[0].0, pair[1].0);
        let src = solid_fp16(gfx, comp, pair[1].1, keep)?;
        let seg = alpha_ramp(comp, &[(0.0, 0.0), (from, 0.0), (to.max(from), 1.0), (1.0, 1.0)]);
        let mask = comp.create_mask_brush();
        mask.set_mask(&seg);
        mask.set_source(&src);
        let layer = comp.create_sprite_visual();
        layer.set_brush(&mask);
        layer.set_relative_size_adjustment(Vector2::new(1.0, 1.0));
        brushes.extend([src.as_brush(), seg, mask.as_brush()]);
        container.children().insert_at_top(&layer);
        keep.push(Box::new(layer));
    }

    let capture = comp.create_visual_surface();
    capture.set_source_visual(&container);
    capture.set_source_offset(Vector2::new(0.0, 0.0));
    capture.set_source_size(Vector2::new(size, size));
    let brush = comp.create_surface_brush(&capture);
    brush.set_stretch(Stretch::Fill);
    keep.push(Box::new(container));
    keep.push(Box::new(capture));
    Some(brush)
}

/// One glow cell: a white ring stroke, blurred by a `DropShadow` purely as
/// alpha, masking `source`. This is `GlowLayer` with its colour input made a
/// parameter — the ONLY difference between cell A and cell B is what is handed
/// in here.
fn glow_cell(
    comp: &Compositor,
    root: &ContainerVisual,
    x: f32,
    source: &CompositionSurfaceBrush,
) -> Cell {
    let mut visuals = Vec::new();
    let mut brushes = Vec::new();
    let mut keep: Vec<Box<dyn std::any::Any>> = Vec::new();

    // The white ring, off-tree: only its visual surface reads it.
    let geo = comp.create_ellipse_geometry();
    geo.set_radius(Vector2::new(RING_R, RING_R));
    let shape = comp.create_sprite_shape(&geo);
    shape.set_offset(Vector2::new(CELL / 2.0, CELL / 2.0));
    shape.set_stroke_brush(&comp.create_color_brush(UiColor::rgb(255, 255, 255)));
    shape.set_stroke_thickness(RING_W);
    shape.set_stroke_caps(StrokeCap::Round);

    let stroke_shape = comp.create_shape_visual();
    stroke_shape.shapes().append(&shape);
    stroke_shape.set_size(CELL, CELL);
    stroke_shape.set_border_mode(BorderMode::Soft);

    let stroke_surface = comp.create_visual_surface();
    stroke_surface.set_source_visual(&stroke_shape);
    stroke_surface.set_source_offset(Vector2::new(0.0, 0.0));
    stroke_surface.set_source_size(Vector2::new(CELL, CELL));

    // The shadow blurs THAT alpha at zero offset: a glow, not a shadow.
    let shadow = comp.create_drop_shadow();
    shadow.set_offset(0.0, 0.0, 0.0);
    shadow.set_opacity(1.0);
    shadow.set_color(UiColor::rgb(255, 255, 255));
    shadow.set_source(ShadowSource::Color);
    shadow.set_mask(&comp.create_surface_brush(&stroke_surface));
    shadow.set_blur_radius(SIGMA);

    let halo_sprite = comp.create_sprite_visual();
    halo_sprite.set_shadow(&shadow);
    halo_sprite.set_size(CELL, CELL);
    halo_sprite.set_border_mode(BorderMode::Soft);

    let halo_surface = comp.create_visual_surface();
    halo_surface.set_source_visual(&halo_sprite);
    halo_surface.set_source_offset(Vector2::new(0.0, 0.0));
    halo_surface.set_source_size(Vector2::new(CELL, CELL));

    // The binding under test: halo alpha as MASK, the caller's colour as SOURCE.
    let mask_brush = comp.create_mask_brush();
    mask_brush.set_mask(&comp.create_surface_brush(&halo_surface));
    mask_brush.set_source(source);

    let display = comp.create_sprite_visual();
    display.set_brush(&mask_brush);
    display.set_size(CELL, CELL);
    display.set_offset(x, PAD, 0.0);
    root.children().insert_at_top(&display);

    brushes.push(mask_brush.as_brush());
    visuals.push(display);
    visuals.push(halo_sprite);
    keep.push(Box::new(geo));
    keep.push(Box::new(shape));
    keep.push(Box::new(stroke_shape));
    keep.push(Box::new(stroke_surface));
    keep.push(Box::new(shadow));
    keep.push(Box::new(halo_surface));
    Cell { _visuals: visuals, _brushes: brushes, _keep: keep }
}

/// Cell C: the colour field with no mask over it at all.
fn field_cell(
    comp: &Compositor,
    root: &ContainerVisual,
    x: f32,
    source: &CompositionSurfaceBrush,
) -> Cell {
    let display = comp.create_sprite_visual();
    display.set_brush(source);
    display.set_size(CELL, CELL);
    display.set_offset(x, PAD, 0.0);
    root.children().insert_at_top(&display);
    Cell { _visuals: vec![display], _brushes: Vec::new(), _keep: Vec::new() }
}

fn main() -> windows_core::Result<()> {
    let _queue = DispatcherQueueController::create_on_current_thread()?;
    let window = Window::new("glow ramp probe").size(WIN_W, WIN_H).create()?;
    let (cw, ch) = window.client_size();

    let gpu = GpuDevice::new_multi_threaded()?;
    let comp = Compositor::new()?;
    let gfx = comp.create_graphics_device(gpu.d2d_device())?;
    let target = comp.create_desktop_window_target(&window, false)?;
    let root = comp.create_container_visual();
    target.set_root(&root);

    // Black backdrop, so every probe reads the cell's own light and nothing else.
    let bg = comp.create_sprite_visual();
    bg.set_brush(&comp.create_color_brush(UiColor::rgb(0, 0, 0)));
    bg.set_size(cw as f32, ch as f32);
    root.children().insert_at_top(&bg);

    let mut keep: Vec<Box<dyn std::any::Any>> = Vec::new();
    let mut brushes: Vec<CompositionBrush> = Vec::new();

    let flat = solid_fp16(&gfx, &comp, hue_white(), &mut keep)
        .expect("flat FP16 source");
    let hues = [(0.0, hue_red()), (0.5, hue_green()), (1.0, hue_blue())];
    let ramp_a = staircase(&gfx, &comp, &hues, CELL, &mut keep, &mut brushes)
        .expect("staircase source for cell B");
    let ramp_b = staircase(&gfx, &comp, &hues, CELL, &mut keep, &mut brushes)
        .expect("staircase source for cell C");

    let xs = [PAD, PAD * 2.0 + CELL, PAD * 3.0 + CELL * 2.0];
    let cells = vec![
        glow_cell(&comp, &root, xs[0], &flat),
        glow_cell(&comp, &root, xs[1], &ramp_a),
        field_cell(&comp, &root, xs[2], &ramp_b),
    ];

    // The layout, so a probe can be aimed without measuring the screenshot. The
    // halo probes sit ON the ring's left and right extremes (where the blur is
    // brightest) and one sits just OUTSIDE it, in the halo's falloff.
    println!("pid {}  client {cw}x{ch}", std::process::id());
    println!("cells at x = {xs:?}, y = {PAD}, {CELL}x{CELL}; ring r={RING_R} at cell centre");
    for (name, x) in [("A flat", xs[0]), ("B ramp", xs[1]), ("C field", xs[2])] {
        let cy = PAD + CELL / 2.0;
        let (l, r) = (x + CELL / 2.0 - RING_R, x + CELL / 2.0 + RING_R);
        println!(
            "  {name}: left {},{},8,8   right {},{},8,8   outer-left {},{},8,8",
            l as i32 - 4,
            cy as i32 - 4,
            r as i32 - 4,
            cy as i32 - 4,
            (l - RING_W) as i32 - 4,
            cy as i32 - 4,
        );
    }

    windows_window::run();
    drop(cells);
    Ok(())
}
