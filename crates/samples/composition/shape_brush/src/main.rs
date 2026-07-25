//! **Probe: can a `CompositionSpriteShape` be painted with an FP16 surface brush?**
//!
//! `reactor::backend::dcomp::path_shape` draws every curve by putting the
//! geometry in an OFF-TREE `ShapeVisual`, capturing that through a
//! `CompositionVisualSurface`, and using the capture as the mask of a
//! `CompositionMaskBrush` whose source is an app-allocated `Rgba16Float`
//! surface. The indirection exists for one reason: colour on an HDR desktop
//! needs scRGB above 1.0, and the routes that let the COMPOSITOR carry colour
//! clamp.
//!
//! That costs a capture render target per layer, and DWM re-renders it whenever
//! the geometry changes — which for a live curve is every frame.
//!
//! If a sprite shape can simply be FILLED or STROKED with the FP16 surface
//! brush, the mask and the capture both disappear for the flat-colour case: the
//! geometry goes on-tree and DWM draws it directly.
//!
//! Columns, left to right:
//!
//! 0. `shape-color` — CONTROL: sprite shape filled with a `CompositionColorBrush`
//!    (white). 8-bit, so it should read the paper-white ceiling (~3.0 on a
//!    240-nit desktop), NOT 4.0. Establishes that the shape route renders at all.
//! 1. `shape-surface-fill` — **THE QUESTION.** Sprite shape whose `FillBrush` is
//!    a surface brush over an `Rgba16Float` 4.0 surface. If it binds and reads
//!    ~4.0, the capture is unnecessary for flat fills.
//! 2. `shape-surface-stroke` — the same brush as `StrokeBrush` on a thick stroke,
//!    which is the case a curve actually needs.
//! 3. `mask-capture` — CONTROL: the shipping route (off-tree shape → visual
//!    surface → mask brush → FP16 source). Reads ~4.0 by construction; if it
//!    does not, the harness is wrong and nothing else here means anything.
//!
//! An anchor strip of flat FP16 4.0 spans the top. If the anchor does not read
//! ~4.0 the desktop is not in Advanced Color mode and NOTHING below is meaningful.
//!
//! ```text
//! cargo run -p composition_shape_brush
//! # then, from gui/tools/guishot, with the printed --probe list:
//! guishot --pid <PID> --out probe.png --client-only --probe ...
//! ```

// Deliberately NOT `windows_subsystem = "windows"`: the bind results and the
// probe command are printed, and a windows-subsystem process started from a
// console cannot write to it.

use windows::Win32::{
    D2D1CreateDevice, D2D_COLOR_F, D3D11CreateDevice, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SDK_VERSION, D3D_DRIVER_TYPE_HARDWARE, HINSTANCE, HWND,
    ICompositionDrawingSurfaceInterop, ICompositorDesktopInterop, ICompositorInterop, ID2D1Device,
    ID2D1DeviceContext, ID3D11Device, IDXGIDevice, POINT,
};
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use windows::UI::Color;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::Diagnostics::{
    CompositionDebugOverdrawContentKinds, CompositionDebugSettings,
};
use windows::UI::Composition::{
    CompositionBorderMode, CompositionBrush, CompositionGraphicsDevice, CompositionStretch,
    Compositor, ContainerVisual, Visual,
};
use windows::core::Interface;
use windows_core::Result;
use windows_numerics::{Matrix3x2, Vector2, Vector3};
use windows_window::{Window, run};

// ── Layout, in physical pixels (a desktop-window target composes in pixels) ──

const COL_W: i32 = 120;
const COL_GAP: i32 = 20;
const X0: i32 = 24;

const ANCHOR_Y: i32 = 4;
const ANCHOR_H: i32 = 16;

const ROW_Y: i32 = 32;
const ROW_H: i32 = 240;

const CLIENT_H: i32 = ROW_Y + ROW_H + 12;

/// Source luminance, in scRGB (`1.0` == 80 nits). Well above paper white
/// (~3.0 on a 240-nit desktop) so a clamp at 1.0 and a clamp at the 8-bit
/// ceiling are both unmistakable.
const SRC: f32 = 4.0;

/// Stroke width for the stroked column — fat enough to probe its middle without
/// catching an antialiased edge.
const STROKE_W: f32 = 40.0;

const ROUTES: &[(&str, &str)] = &[
    ("shape-color", "CONTROL: SpriteShape.FillBrush = ColorBrush(white) — expect the 8-bit ceiling"),
    ("shape-surface-fill", "SpriteShape.FillBrush = FP16 surface brush"),
    ("shape-surface-stroke", "SpriteShape.StrokeBrush = FP16 surface brush, thickness 40"),
    ("mask-capture", "CONTROL: shipping route — off-tree shape -> VisualSurface -> MaskBrush{src FP16}"),
    ("clip-ellipse-soft", "THE CANDIDATE: FP16 sprite + GeometricClip(ellipse), BorderMode Soft"),
    ("clip-ellipse-hard", "same clip, BorderMode left at Inherit — the antialiasing control"),
    ("shape-trim-ref", "REFERENCE: stroked SpriteShape, same ellipse, TrimEnd 0.5 — what trim means"),
    ("clip-trim", "THE TRIM QUESTION: the clip route over the SAME TrimEnd-0.5 geometry"),
];

/// How far round the trimmed columns run. A half turn is unmistakable: if trim
/// is honoured the column is visibly partial, and if it is ignored the column is
/// identical to the untrimmed clip two places to its left.
const TRIM_END: f32 = 0.5;

fn main() -> Result<()> {
    let _queue = create_dispatcher_queue()?;
    let compositor = Compositor::new()?;

    let n = ROUTES.len() as i32;
    let client_w = X0 * 2 + n * COL_W + (n - 1) * COL_GAP;

    let window = Window::new("shape-brush-probe")
        .size(client_w + 16, CLIENT_H + 39)
        .create()?;
    let (cw, ch) = window.client_size();

    let interop: ICompositorDesktopInterop = compositor.cast()?;
    let target: DesktopWindowTarget =
        unsafe { interop.CreateDesktopWindowTarget(HWND(window.hwnd()), false)? }.cast()?;
    let root = compositor.CreateContainerVisual()?;
    root.SetSize(Vector2 { x: cw as f32, y: ch as f32 })?;
    target.SetRoot(&root)?;

    // Opaque black behind everything, so a probed value is the route's own
    // output with nothing added.
    let bg = compositor.CreateSpriteVisual()?;
    bg.SetSize(Vector2 { x: cw as f32, y: ch as f32 })?;
    bg.SetBrush(&compositor.CreateColorBrushWithColor(Color { A: 255, R: 0, G: 0, B: 0 })?)?;
    root.Children()?.InsertAtTop(&bg)?;

    let device = GraphicsDevice::new(&compositor)?;

    // Alignment + Advanced-Color gate: a flat FP16 4.0 fill across the top.
    let anchor = compositor.CreateSpriteVisual()?;
    anchor.SetOffset(Vector3 { x: X0 as f32, y: ANCHOR_Y as f32, z: 0.0 })?;
    anchor.SetSize(Vector2 { x: (cw - X0 * 2) as f32, y: ANCHOR_H as f32 })?;
    anchor.SetBrush(&device.solid_brush(&compositor, SRC)?)?;
    root.Children()?.InsertAtTop(&anchor)?;

    // The off-tree capture source must outlive the tree that samples it.
    let mut keepalive: Vec<Visual> = Vec::new();

    let mut failures: Vec<String> = Vec::new();
    for (i, (name, _)) in ROUTES.iter().enumerate() {
        let x = X0 + i as i32 * (COL_W + COL_GAP);
        match build(&compositor, &device, name, &mut keepalive) {
            Ok(visual) => {
                visual.SetOffset(Vector3 { x: x as f32, y: ROW_Y as f32, z: 0.0 })?;
                visual.SetSize(Vector2 { x: COL_W as f32, y: ROW_H as f32 })?;
                root.Children()?.InsertAtTop(&visual)?;
            }
            Err(e) => failures.push(format!("{name}: BIND FAILED {e}")),
        }
    }

    apply_heat_map(&compositor, &root)?;
    report(&failures, std::process::id(), cw);
    run();
    Ok(())
}

/// Build one column. Every route paints the same rectangle over the column's
/// full extent, so one probe at the centre reads whatever that route produced.
fn build(
    compositor: &Compositor,
    device: &GraphicsDevice,
    route: &str,
    keepalive: &mut Vec<Visual>,
) -> Result<Visual> {
    let w = COL_W as f32;
    let h = ROW_H as f32;

    match route {
        "shape-color" => {
            let white = compositor.CreateColorBrushWithColor(Color { A: 255, R: 255, G: 255, B: 255 })?;
            shape_visual(compositor, w, h, Some(&white.cast()?), None)
        }
        "shape-surface-fill" => {
            let src = device.solid_brush(compositor, SRC)?;
            shape_visual(compositor, w, h, Some(&src), None)
        }
        "shape-surface-stroke" => {
            let src = device.solid_brush(compositor, SRC)?;
            shape_visual(compositor, w, h, None, Some(&src))
        }
        "mask-capture" => {
            // The shipping construction, verbatim: geometry off-tree, captured,
            // used as the mask of a brush whose source is the FP16 surface.
            let mask_shape = shape_visual(
                compositor,
                w,
                h,
                Some(&compositor
                    .CreateColorBrushWithColor(Color { A: 255, R: 255, G: 255, B: 255 })?
                    .cast()?),
                None,
            )?;
            mask_shape.SetSize(Vector2 { x: w, y: h })?;

            let capture = compositor.CreateVisualSurface()?;
            capture.SetSourceVisual(&mask_shape)?;
            capture.SetSourceOffset(Vector2 { x: 0.0, y: 0.0 })?;
            capture.SetSourceSize(Vector2 { x: w, y: h })?;
            keepalive.push(mask_shape);

            let mask_brush = compositor.CreateMaskBrush()?;
            mask_brush.SetMask(&compositor.CreateSurfaceBrushWithSurface(&capture)?)?;
            mask_brush.SetSource(&device.solid_brush(compositor, SRC)?)?;

            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetBrush(&mask_brush)?;
            Ok(sprite.cast()?)
        }
        // A clip is not a brush, so it does not compete for the visual's one
        // brush slot: the sprite keeps its FP16 surface brush (unclamped, drawn
        // inline) and the clip supplies the geometry. No visual surface, no mask
        // brush, nothing for DWM to capture.
        "clip-ellipse-soft" | "clip-ellipse-hard" | "clip-trim" => {
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetBrush(&device.solid_brush(compositor, SRC)?)?;
            if !route.ends_with("hard") {
                sprite.SetBorderMode(CompositionBorderMode::Soft)?;
            }

            let geo = ellipse(compositor, w, h)?;
            if route == "clip-trim" {
                geo.SetTrimEnd(TRIM_END)?;
            }
            sprite.SetClip(&compositor.CreateGeometricClipWithGeometry(&geo)?)?;

            Ok(sprite.cast()?)
        }
        // The same trimmed geometry through the route trim is KNOWN to work on,
        // so the two columns differ only in what consumes the geometry.
        "shape-trim-ref" => {
            let geo = ellipse(compositor, w, h)?;
            geo.SetTrimEnd(TRIM_END)?;

            let shape = compositor.CreateSpriteShapeWithGeometry(&geo)?;
            shape.SetStrokeBrush(
                &compositor.CreateColorBrushWithColor(Color { A: 255, R: 255, G: 255, B: 255 })?,
            )?;
            shape.SetStrokeThickness(8.0)?;

            let visual = compositor.CreateShapeVisual()?;
            visual.SetSize(Vector2 { x: w, y: h })?;
            visual.Shapes()?.Append(&shape)?;
            Ok(visual.cast()?)
        }
        _ => unreachable!("unknown route {route}"),
    }
}

/// Apply a DWM heat map over the whole tree when `PROBE_HEATMAP` asks for one.
/// `offscreen` narrows overdraw to content the compositor had to render to an
/// intermediate first — which is the whole question these routes are asked to
/// answer. Debug settings are withheld unless the machine is in developer mode.
fn apply_heat_map(compositor: &Compositor, root: &ContainerVisual) -> Result<()> {
    let Ok(spec) = std::env::var("PROBE_HEATMAP") else {
        return Ok(());
    };
    let settings = match CompositionDebugSettings::TryGetSettings(compositor) {
        Ok(s) => s,
        Err(e) => {
            println!("heat map unavailable ({e}) — developer mode is off");
            return Ok(());
        }
    };
    let maps = settings.HeatMaps()?;
    match spec.trim() {
        "offscreen" => maps.ShowOverdraw(root, CompositionDebugOverdrawContentKinds::OffscreenRendered)?,
        "overdraw" => maps.ShowOverdraw(root, CompositionDebugOverdrawContentKinds::All)?,
        "redraw" => maps.ShowRedraw(root)?,
        _ => maps.Hide(root)?,
    }
    println!("heat map: {spec}");
    Ok(())
}

/// The one geometry the clip and reference columns share, inset so its stroke
/// lands inside the column.
fn ellipse(
    compositor: &Compositor,
    w: f32,
    h: f32,
) -> Result<windows::UI::Composition::CompositionEllipseGeometry> {
    let geo = compositor.CreateEllipseGeometry()?;
    geo.SetCenter(Vector2 { x: w / 2.0, y: h / 2.0 })?;
    geo.SetRadius(Vector2 { x: w / 2.0 - 8.0, y: h / 2.0 - 8.0 })?;
    Ok(geo)
}

/// A `ShapeVisual` holding one rectangle sprite shape, filled and/or stroked
/// with the given brushes. A stroke is drawn inset by half its width so the
/// whole stroke lands inside the column.
fn shape_visual(
    compositor: &Compositor,
    w: f32,
    h: f32,
    fill: Option<&CompositionBrush>,
    stroke: Option<&CompositionBrush>,
) -> Result<Visual> {
    let inset = if stroke.is_some() { STROKE_W / 2.0 } else { 0.0 };
    let geo = compositor.CreateRectangleGeometry()?;
    geo.SetOffset(Vector2 { x: inset, y: inset })?;
    geo.SetSize(Vector2 { x: w - inset * 2.0, y: h - inset * 2.0 })?;

    let shape = compositor.CreateSpriteShapeWithGeometry(&geo)?;
    if let Some(f) = fill {
        shape.SetFillBrush(f)?;
    }
    if let Some(s) = stroke {
        shape.SetStrokeBrush(s)?;
        shape.SetStrokeThickness(STROKE_W)?;
    }

    let visual = compositor.CreateShapeVisual()?;
    visual.SetSize(Vector2 { x: w, y: h })?;
    visual.Shapes()?.Append(&shape)?;
    Ok(visual.cast()?)
}

// ── The FP16 source surfaces ────────────────────────────────────────────────

struct GraphicsDevice {
    device: CompositionGraphicsDevice,
}

impl GraphicsDevice {
    fn new(compositor: &Compositor) -> Result<Self> {
        let mut d3d: Option<ID3D11Device> = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HINSTANCE(core::ptr::null_mut()),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT as u32,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )
            .ok()?;
        }
        let dxgi: IDXGIDevice = d3d.unwrap().cast()?;
        let d2d: ID2D1Device = unsafe { D2D1CreateDevice(&dxgi, None)? };
        let interop: ICompositorInterop = compositor.cast()?;
        let device: CompositionGraphicsDevice =
            unsafe { interop.CreateGraphicsDevice(&d2d)? }.cast()?;
        Ok(Self { device })
    }

    /// A flat FP16 surface at scRGB `v`, stretched to fill whatever paints with it.
    fn solid_brush(&self, compositor: &Compositor, v: f32) -> Result<CompositionBrush> {
        let surface = self.device.CreateDrawingSurface2(
            windows::Graphics::SizeInt32 { Width: 8, Height: 8 },
            DirectXPixelFormat::R16G16B16A16Float,
            DirectXAlphaMode::Premultiplied,
        )?;
        let interop: ICompositionDrawingSurfaceInterop = surface.cast()?;
        let mut offset = POINT::default();
        let ctx: ID2D1DeviceContext = unsafe { interop.BeginDraw(None, &mut offset)? };
        unsafe {
            ctx.SetTransform(&Matrix3x2::translation(offset.x as f32, offset.y as f32));
            ctx.Clear(Some(&D2D_COLOR_F { r: v, g: v, b: v, a: 1.0 }));
            interop.EndDraw().ok()?;
        }
        let brush = compositor.CreateSurfaceBrushWithSurface(&surface)?;
        brush.SetStretch(CompositionStretch::Fill)?;
        brush.cast()
    }
}

fn create_dispatcher_queue() -> Result<windows::System::DispatcherQueueController> {
    use windows::Win32::{
        CreateDispatcherQueueController, DQTAT_COM_ASTA, DQTYPE_THREAD_CURRENT,
        DispatcherQueueOptions,
    };
    let options = DispatcherQueueOptions {
        dwSize: core::mem::size_of::<DispatcherQueueOptions>() as u32,
        threadType: DQTYPE_THREAD_CURRENT,
        apartmentType: DQTAT_COM_ASTA,
    };
    let controller = unsafe { CreateDispatcherQueueController(options)? };
    controller.cast()
}

fn report(failures: &[String], pid: u32, client_w: i32) {
    println!("shape-brush probe — pid {pid}, client {client_w}x{CLIENT_H}");
    println!();
    for (i, (name, what)) in ROUTES.iter().enumerate() {
        let x = X0 + i as i32 * (COL_W + COL_GAP);
        println!("  col {i} x={x:<4} {name:<22} {what}");
    }
    println!();
    if failures.is_empty() {
        println!("all routes bound");
    } else {
        println!("BIND FAILURES:");
        for f in failures {
            println!("  {f}");
        }
    }
    println!();

    let mut probes = format!(" --probe {},{},{},{}", X0, ANCHOR_Y + 4, client_w - X0 * 2, 8);
    for (i, _) in ROUTES.iter().enumerate() {
        // Centre of the column. For the stroked route that is the middle of the
        // stroke's left edge, so probe a band that the stroke actually covers.
        let x = X0 + i as i32 * (COL_W + COL_GAP) + 4;
        probes.push_str(&format!(" --probe {},{},{},{}", x, ROW_Y + 4, 24, 24));
    }
    println!("guishot --pid {pid} --out probe.png --client-only{probes}");
}
