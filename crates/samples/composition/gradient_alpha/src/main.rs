//! **Probe: can the compositor carry a gradient's ALPHA ramp?**
//!
//! The established rule (see `reactor::backend::dcomp::path_shape`'s `GlowLayer`)
//! is that the compositor may carry ALPHA but never COLOUR: alpha is `0..1`,
//! which 8 bits holds exactly, while colour on an HDR desktop needs scRGB well
//! above 1.0 and every compositor-carries-colour route clamps. The shipping glow
//! exploits that — a `DropShadow` blurs a white stroke purely as an alpha
//! generator, and that alpha masks an app-allocated `Rgba16Float` source.
//!
//! Gradients today do NOT follow that rule: `parts::rasterize` bakes colour AND
//! ramp into an FP16 surface per stop-set. This probe asks whether the ramp can
//! move to the compositor as alpha while colour stays in FP16.
//!
//! Each column below is one candidate route. Every column paints the SAME two
//! ramps:
//!
//! * **row A (ceiling)** — alpha `1.0 -> 0.0`, source scRGB `4.0` white. The top
//!   of the strip reads the route's ceiling: `~4.0` is unclamped, `1.0` is a
//!   clamp, `~3.0` is an 8-bit tint's paper-white ceiling.
//! * **row B (subtle)** — alpha `0.18 -> 0.03`, source scRGB `3.0` (paper white):
//!   the real use, a spectrum-bar body fading down a tall thin rect. Row B is
//!   the BANDING test — probe it row by row and look for repeated values.
//!
//! An **anchor** strip spans the top: a flat FP16 `4.0` fill. If the anchor does
//! not read ~4.0 the desktop is not in Advanced Color mode and NOTHING below is
//! meaningful.
//!
//! Nothing here animates and nothing redraws; the window is static so a capture
//! is deterministic.
//!
//! ## How to read it
//!
//! Run it, then probe the raw FP16 frame — an 8-bit screenshot cannot tell a
//! clamped route from an unclamped one:
//!
//! ```text
//! cargo run -p composition_gradient_alpha
//! # then, from gui/tools/guishot, with the printed --probe list:
//! guishot --pid <PID> --out probe.png --client-only --probe ... --probe ...
//! ```
//!
//! The program prints its own probe command lines (coarse and per-row) once the
//! window is up.
//!
//! ## What it measured
//!
//! On a 240-nit Advanced Color desktop (anchor read scRGB `4.0000` exactly, so
//! the pipeline is unclamped end to end). "ceiling" is the top of row A against
//! an ideal `3.970`; "ramp" is the whole of row A against `4.0 * alpha`;
//! "levels" is the count of distinct values down row B's 300 rows.
//!
//! | route | ceiling | ramp | levels | verdict |
//! |---|---|---|---|---|
//! | `raster-fp16` (control) | 3.9688 | exact | 300 | the reference |
//! | `grad-direct` (control) | 2.9570 | badly non-linear | — | clamps at paper white, as the rule says |
//! | `mask-grad` | **3.9668** | **exact** | 37 | binds, unclamped, linear — but BANDS |
//! | `mask-grad-norm` | **3.9668** | **exact** | **229** | same, banding fixed — the winner |
//! | `mask-ninegrid` | 3.9668 | exact | 37 | identical to `mask-grad`; the wrapper buys nothing |
//! | `mask-grad-linear` | 3.9668 | exact | 37 | `InterpolationSpace` changes nothing |
//! | `mask-grad-capture` | 3.9414 | non-linear | — | ceiling survives, ramp does not |
//! | `shadow-mask-grad` | 3.8926 | badly non-linear | — | worst of the mask routes |
//! | `mask-radial` | 2.9355 | n/a | — | silently ignored: paints the MASK, clamped |
//! | `opacity-stack` | 3.9355 | stepped | 32 | opacity quantizes to 8 bits; N visuals |
//!
//! Two findings carry the design:
//!
//! **A gradient brush IS a legal mask** — a linear one, at least. `Mask` is
//! documented for surface / nine-grid / effect brushes, but
//! `CompositionLinearGradientBrush` binds and works: the FP16 source comes
//! through at `3.9668` against an ideal `3.970`, and the ramp is linear-correct
//! at every depth. A radial one BINDS AND SILENTLY DOES THE WRONG THING —
//! `mask-radial` reads paper white, i.e. the mask brush degenerated to painting
//! the mask itself. It throws nothing and an 8-bit screenshot of it looks fine.
//!
//! **The compositor's alpha intermediate is 8 bits, so the ramp's RANGE decides
//! its quality.** "Alpha is `0..1`, which 8 bits holds exactly" is true of a
//! coverage value and false of a ramp: a fade authored `0.18 -> 0.03` only uses
//! 0.15 of the range, so it gets 38 levels and bands in ~9-row steps. Author the
//! mask over the FULL range (`1.0 -> 0.1667`) and put the `0.18` into the FP16
//! source's brightness instead — same composited ramp, 229 levels, steps at most
//! two rows tall. That is `mask-grad-norm`, and it is what makes the route a win
//! rather than a regression.
//!
//! The one thing a mask cannot carry on its own is a ramp whose HUE varies: a
//! mask is a single alpha channel over a single source colour. A multi-hue ramp
//! is instead a STAIRCASE of constant-colour layers whose masks partition the
//! alpha range — source-over between them is exactly the piecewise-linear
//! interpolation a multi-stop gradient describes — composited into a container
//! and captured once. That capture is safe only because what crosses it is
//! COLOUR: a capture carries colour bit-accurately and mangles a ramp carried as
//! alpha. See reactor's backend::dcomp::gradient.

use windows::Win32::{
    D2D1CreateDevice, D2D_COLOR_F, D2D_RECT_F, D3D11CreateDevice, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SDK_VERSION, D3D_DRIVER_TYPE_HARDWARE,
    DQTAT_COM_ASTA, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions, HINSTANCE, HWND,
    ICompositionDrawingSurfaceInterop, ICompositorDesktopInterop, ICompositorInterop, ID2D1Device,
    ID2D1DeviceContext, ID3D11Device, IDXGIDevice, POINT,
};
use windows::Win32::CreateDispatcherQueueController;
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use windows::UI::Color;
use windows::UI::Composition::{
    CompositionBrush, CompositionColorSpace, CompositionDropShadowSourcePolicy,
    CompositionGraphicsDevice, CompositionMappingMode, CompositionStretch, Compositor,
    ContainerVisual, SpriteVisual, Visual,
};
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::core::Interface;
use windows_core::Result;
use windows_numerics::{Matrix3x2, Vector2, Vector3};
use windows_window::{Window, run};

// ── Layout, in physical pixels (a desktop-window target composes in pixels) ──

const COL_W: i32 = 72;
const COL_GAP: i32 = 16;
const X0: i32 = 24;

const ANCHOR_Y: i32 = 4;
const ANCHOR_H: i32 = 16;

const ROW_A_Y: i32 = 28;
const ROW_A_H: i32 = 200;

const ROW_B_Y: i32 = 244;
const ROW_B_H: i32 = 320;

const CLIENT_H: i32 = ROW_B_Y + ROW_B_H + 12;

/// Source luminance for row A, in scRGB (`1.0` == 80 nits). Chosen well above
/// paper white (~3.0 on a 240-nit desktop) so a clamp at 1.0 and a clamp at the
/// 8-bit tint ceiling are both unmistakable.
const SRC_A: f32 = 4.0;
/// Source luminance for row B — paper white, the level a real bar body sits at.
const SRC_B: f32 = 3.0;

/// Row A alpha ramp: full coverage to none.
const A_TOP: f32 = 1.0;
const A_BOT: f32 = 0.0;
/// Row B alpha ramp: the subtle fade a spectrum bar body actually uses.
const B_TOP: f32 = 0.18;
const B_BOT: f32 = 0.03;

/// Steps in the `opacity-stack` route — one sub-visual per step.
const STACK_STEPS: i32 = 32;

/// Blocks in the `flat-opacity` / `flat-fold` pair. Fewer than [`STACK_STEPS`]
/// on purpose: these measure one flat scalar per block rather than approximating
/// a ramp, so each block wants enough rows to probe unambiguously.
const FLAT_STEPS: i32 = 16;

/// Which row a strip belongs to; only the constants differ.
#[derive(Clone, Copy)]
struct Ramp {
    y: i32,
    h: i32,
    src: f32,
    top: f32,
    bot: f32,
}

const RAMP_A: Ramp = Ramp { y: ROW_A_Y, h: ROW_A_H, src: SRC_A, top: A_TOP, bot: A_BOT };
const RAMP_B: Ramp = Ramp { y: ROW_B_Y, h: ROW_B_H, src: SRC_B, top: B_TOP, bot: B_BOT };

/// The routes, in column order. The first two are CONTROLS: a known-good
/// baseline and a known-bad one, so a run that fails to reproduce them is a
/// broken harness rather than a finding.
const ROUTES: &[(&str, &str)] = &[
    ("raster-fp16", "CONTROL: app-rasterized FP16 ramp (what reactor shipped before this probe)"),
    ("grad-direct", "CONTROL/expect-clamp: gradient brush painted straight onto the sprite"),
    ("mask-grad", "MaskBrush{mask: linear gradient (white, alpha ramp), source: FP16 solid}"),
    ("mask-grad-capture", "glow pattern: gradient sprite -> VisualSurface -> mask; source FP16"),
    ("mask-ninegrid", "MaskBrush{mask: NineGrid{source: linear gradient}, source: FP16 solid}"),
    ("mask-radial", "MaskBrush{mask: radial gradient alpha ramp, source: FP16 solid}"),
    ("shadow-mask-grad", "DropShadow(blur 0).Mask = gradient -> VisualSurface -> mask; src FP16"),
    ("opacity-stack", "32 stacked FP16 sprites, ramp carried by Visual.Opacity"),
    ("mask-grad-linear", "mask-grad with InterpolationSpace = Rgb (linear) — interpolation variant"),
    ("mask-grad-norm", "mask-grad with the mask NORMALIZED to full alpha and the scale in the FP16 source"),
    ("flat-opacity", "16 flat blocks of the SHIPPING layer structure, scalar on Visual.Opacity"),
    ("flat-fold", "the same 16 blocks, scalar folded into the FP16 source instead"),
];

fn main() -> Result<()> {
    // The dispatcher queue and compositor are declared first so they outlive
    // every composition object — the engine must not be torn down while visuals
    // are still releasing.
    let _queue = create_dispatcher_queue()?;
    let compositor = Compositor::new()?;

    let n = ROUTES.len() as i32;
    let client_w = X0 * 2 + n * COL_W + (n - 1) * COL_GAP;

    let window = Window::new("gradient-alpha-probe")
        .size(client_w + 16, CLIENT_H + 39)
        .create()?;
    let (cw, ch) = window.client_size();

    let interop: ICompositorDesktopInterop = compositor.cast()?;
    let target: DesktopWindowTarget =
        unsafe { interop.CreateDesktopWindowTarget(HWND(window.hwnd()), false)? }.cast()?;
    let root = compositor.CreateContainerVisual()?;
    root.SetSize(Vector2 { x: cw as f32, y: ch as f32 })?;
    target.SetRoot(&root)?;

    // Opaque black behind everything, so a strip's alpha composites against a
    // known zero and the probed value is `alpha * source` with nothing added.
    let bg = compositor.CreateSpriteVisual()?;
    bg.SetSize(Vector2 { x: cw as f32, y: ch as f32 })?;
    bg.SetBrush(&compositor.CreateColorBrushWithColor(Color { A: 255, R: 0, G: 0, B: 0 })?)?;
    root.Children()?.InsertAtTop(&bg)?;

    let device = GraphicsDevice::new(&compositor)?;

    // Alignment + Advanced-Color gate: a flat FP16 4.0 fill across the top.
    let anchor = compositor.CreateSpriteVisual()?;
    anchor.SetOffset(Vector3 { x: X0 as f32, y: ANCHOR_Y as f32, z: 0.0 })?;
    anchor.SetSize(Vector2 { x: (cw - X0 * 2) as f32, y: ANCHOR_H as f32 })?;
    anchor.SetBrush(&device.solid_brush(&compositor, SRC_A)?)?;
    root.Children()?.InsertAtTop(&anchor)?;

    // Anything the compositor must keep alive but that never enters the tree —
    // the off-tree capture sources. Dropping them would blank their captures.
    let mut keepalive: Vec<Visual> = Vec::new();

    let mut failures: Vec<String> = Vec::new();
    for (i, (name, _)) in ROUTES.iter().enumerate() {
        let x = X0 + i as i32 * (COL_W + COL_GAP);
        for ramp in [RAMP_A, RAMP_B] {
            match build(&compositor, &device, name, ramp, &mut keepalive) {
                Ok(visual) => {
                    visual.SetOffset(Vector3 { x: x as f32, y: ramp.y as f32, z: 0.0 })?;
                    visual.SetSize(Vector2 { x: COL_W as f32, y: ramp.h as f32 })?;
                    root.Children()?.InsertAtTop(&visual)?;
                }
                Err(e) => failures.push(format!("{name} row@{}: BIND FAILED {e}", ramp.y)),
            }
        }
    }

    report(&failures, std::process::id(), cw);
    run();
    Ok(())
}

// ── Route construction ──────────────────────────────────────────────────────

/// Build one route's strip as a visual sized by the caller. Returns `Err` when
/// the route does not bind at all — a route that throws is as much a finding as
/// one that clamps, so the error is reported rather than swallowed.
fn build(
    compositor: &Compositor,
    device: &GraphicsDevice,
    route: &str,
    ramp: Ramp,
    keepalive: &mut Vec<Visual>,
) -> Result<ContainerVisual> {
    let host = compositor.CreateContainerVisual()?;
    match route {
        // CONTROL. What ships today: the ramp is baked into an FP16 surface by
        // the app, one row of the surface per output row, so both colour and
        // coverage carry full float precision. Everything else is measured
        // against this.
        "raster-fp16" => {
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
            sprite.SetBrush(&device.ramp_brush(compositor, ramp)?)?;
            host.Children()?.InsertAtTop(&sprite)?;
        }

        // CONTROL. The compositor carries the COLOUR. Expected to clamp at 1.0 —
        // if this reads 4.0 the whole premise (and the glow doc's table) is wrong.
        "grad-direct" => {
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
            let grad = linear_ramp(compositor, ramp, false)?;
            sprite.SetBrush(&grad)?;
            host.Children()?.InsertAtTop(&sprite)?;
        }

        // The headline question: does `CompositionMaskBrush.Mask` accept a
        // gradient brush at all, and does the FP16 source survive it?
        "mask-grad" | "mask-grad-linear" => {
            let grad = linear_ramp(compositor, ramp, route.ends_with("linear"))?;
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
            sprite.SetBrush(&mask_brush(compositor, device, &grad.cast()?, ramp.src)?)?;
            host.Children()?.InsertAtTop(&sprite)?;
        }

        // Same route, with the ramp's SCALE moved out of the mask.
        //
        // `mask-grad` authors the stops at the alphas the fade actually wants,
        // and a subtle fade wants a small slice of `0..1` — which is where the
        // compositor's 8-bit alpha intermediate has few levels to give. So
        // normalize: the mask runs the full `1.0 -> bot/top` and the FP16 source
        // carries `top` as brightness. The composited ramp is identical and the
        // quantization is finer by `1 / top`.
        "mask-grad-norm" => {
            let norm = Ramp { top: 1.0, bot: ramp.bot / ramp.top.max(f32::MIN_POSITIVE), ..ramp };
            let grad = linear_ramp(compositor, norm, false)?;
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
            sprite.SetBrush(&mask_brush(compositor, device, &grad.cast()?, ramp.src * ramp.top)?)?;
            host.Children()?.InsertAtTop(&sprite)?;
        }

        // The glow's own pattern, aimed at a gradient: paint an OFF-TREE sprite
        // with the gradient, capture it through a visual surface, and mask with
        // the capture. The indirection is the point — a surface brush is a
        // documented `Mask`, a gradient brush is not.
        "mask-grad-capture" => {
            let grad = linear_ramp(compositor, ramp, false)?;
            let source = compositor.CreateSpriteVisual()?;
            source.SetSize(Vector2 { x: COL_W as f32, y: ramp.h as f32 })?;
            source.SetBrush(&grad)?;
            let capture = capture_brush(compositor, &source, COL_W as f32, ramp.h as f32)?;
            keepalive.push(source.cast()?);

            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
            sprite.SetBrush(&mask_brush(compositor, device, &capture, ramp.src)?)?;
            host.Children()?.InsertAtTop(&sprite)?;
        }

        // A nine-grid IS a documented mask, and it takes an arbitrary brush as
        // its source — so it can launder a gradient into a legal mask without
        // the visual-surface capture's extra render pass. Insets are zero: a
        // plain stretch, no corner preservation wanted.
        "mask-ninegrid" => {
            let grad = linear_ramp(compositor, ramp, false)?;
            let nine = compositor.CreateNineGridBrush()?;
            nine.SetInsetsWithValues(0.0, 0.0, 0.0, 0.0)?;
            nine.SetSource(&grad)?;
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
            sprite.SetBrush(&mask_brush(compositor, device, &nine.cast()?, ramp.src)?)?;
            host.Children()?.InsertAtTop(&sprite)?;
        }

        // The radial case. A different brush class reaching the same `Mask`
        // slot, so it answers "is the restriction per-brush-type or per-family".
        // The ramp runs centre -> edge, so its top row is the ramp's OUTER value.
        "mask-radial" => {
            let grad = compositor.CreateRadialGradientBrush()?;
            grad.SetMappingMode(CompositionMappingMode::Relative)?;
            grad.SetEllipseCenter(Vector2 { x: 0.5, y: 0.0 })?;
            grad.SetEllipseRadius(Vector2 { x: 1.0, y: 1.0 })?;
            let stops = grad.ColorStops()?;
            stops.Append(&compositor.CreateColorGradientStopWithOffsetAndColor(0.0, white(ramp.top))?)?;
            stops.Append(&compositor.CreateColorGradientStopWithOffsetAndColor(1.0, white(ramp.bot))?)?;
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
            sprite.SetBrush(&mask_brush(compositor, device, &grad.cast()?, ramp.src)?)?;
            host.Children()?.InsertAtTop(&sprite)?;
        }

        // `DropShadow.Mask` is the slot the shipping glow feeds, so ask whether
        // it takes a gradient directly. Blur 0 makes the shadow a pure copy of
        // its mask's alpha, which is then captured and used as a mask itself —
        // the same two-stage shape the glow has, with the gradient standing in
        // for the stroke.
        "shadow-mask-grad" => {
            let grad = linear_ramp(compositor, ramp, false)?;
            let shadow = compositor.CreateDropShadow()?;
            shadow.SetBlurRadius(0.0)?;
            shadow.SetOffset(Vector3 { x: 0.0, y: 0.0, z: 0.0 })?;
            shadow.SetOpacity(1.0)?;
            shadow.SetColor(Color { A: 255, R: 255, G: 255, B: 255 })?;
            shadow.SetSourcePolicy(CompositionDropShadowSourcePolicy::Default)?;
            shadow.SetMask(&grad)?;

            let caster = compositor.CreateSpriteVisual()?;
            caster.SetSize(Vector2 { x: COL_W as f32, y: ramp.h as f32 })?;
            caster.SetShadow(&shadow)?;
            let capture = capture_brush(compositor, &caster, COL_W as f32, ramp.h as f32)?;
            keepalive.push(caster.cast()?);

            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
            sprite.SetBrush(&mask_brush(compositor, device, &capture, ramp.src)?)?;
            host.Children()?.InsertAtTop(&sprite)?;
        }

        // No mask at all: the ramp is a staircase of per-visual opacities over an
        // FP16 solid. Cheap to reason about and certain to bind — the question is
        // whether opacity multiplies in float (unclamped) and how coarse the
        // staircase reads.
        "opacity-stack" => {
            let brush = device.solid_brush(compositor, ramp.src)?;
            let step_h = ramp.h as f32 / STACK_STEPS as f32;
            for s in 0..STACK_STEPS {
                let t = (s as f32 + 0.5) / STACK_STEPS as f32;
                let seg = compositor.CreateSpriteVisual()?;
                seg.SetOffset(Vector3 { x: 0.0, y: s as f32 * step_h, z: 0.0 })?;
                seg.SetSize(Vector2 { x: COL_W as f32, y: step_h.ceil() })?;
                seg.SetBrush(&brush)?;
                seg.SetOpacity(ramp.top + (ramp.bot - ramp.top) * t)?;
                host.Children()?.InsertAtTop(&seg)?;
            }
        }

        // ── Is a FLAT opacity worth folding into the FP16 source? ──
        //
        // A different question from every route above, which all ask how finely
        // the compositor can carry a RAMP. A node's declared opacity is one
        // scalar over a whole layer, and the two ways to apply it are
        // `Visual.Opacity` (one property write, no raster) and authoring it into
        // the source surface the layer already owns (no compositor property at
        // all, but a rebuild whenever it changes).
        //
        // `opacity-stack` does NOT answer this: its 32 levels are `STACK_STEPS`
        // by construction, not a measurement, and it paints a bare surface brush
        // rather than the mask brush a real shape layer is. These two paint the
        // shipping structure — `MaskBrush{mask: opaque white, source: FP16}` —
        // and differ ONLY in where the scalar went. Both should read
        // `src * alpha` at every block; whichever tracks that more closely is
        // the one worth shipping, and if they agree the fold buys nothing and
        // should not be built.
        //
        // Blocks share the control column's alpha formula, so a horizontal probe
        // at one y compares all three columns at the same intended value.
        "flat-opacity" | "flat-fold" => {
            let fold = route == "flat-fold";
            let mask = device.solid_brush(compositor, 1.0)?;
            let step_h = ramp.h as f32 / FLAT_STEPS as f32;
            for s in 0..FLAT_STEPS {
                let t = (s as f32 + 0.5) / FLAT_STEPS as f32;
                let a = ramp.top + (ramp.bot - ramp.top) * t;
                let brush = compositor.CreateMaskBrush()?;
                brush.SetMask(&mask)?;
                brush.SetSource(&device.solid_brush_a(
                    compositor,
                    ramp.src,
                    if fold { a } else { 1.0 },
                )?)?;
                let seg = compositor.CreateSpriteVisual()?;
                seg.SetOffset(Vector3 { x: 0.0, y: s as f32 * step_h, z: 0.0 })?;
                seg.SetSize(Vector2 { x: COL_W as f32, y: step_h.ceil() })?;
                seg.SetBrush(&brush)?;
                if !fold {
                    seg.SetOpacity(a)?;
                }
                host.Children()?.InsertAtTop(&seg)?;
            }
        }

        other => panic!("unknown route {other}"),
    }
    Ok(host)
}

/// A top-to-bottom linear gradient whose STOPS vary only in alpha — white
/// throughout, so nothing about the colour channel is being asked of the
/// compositor. `linear_space` swaps the interpolation space, which is the only
/// knob that could change how finely the ramp is evaluated between stops.
fn linear_ramp(
    compositor: &Compositor,
    ramp: Ramp,
    linear_space: bool,
) -> Result<windows::UI::Composition::CompositionLinearGradientBrush> {
    let grad = compositor.CreateLinearGradientBrush()?;
    grad.SetMappingMode(CompositionMappingMode::Relative)?;
    grad.SetStartPoint(Vector2 { x: 0.0, y: 0.0 })?;
    grad.SetEndPoint(Vector2 { x: 0.0, y: 1.0 })?;
    grad.SetInterpolationSpace(if linear_space {
        CompositionColorSpace::Rgb
    } else {
        CompositionColorSpace::Auto
    })?;
    let stops = grad.ColorStops()?;
    stops.Append(&compositor.CreateColorGradientStopWithOffsetAndColor(0.0, white(ramp.top))?)?;
    stops.Append(&compositor.CreateColorGradientStopWithOffsetAndColor(1.0, white(ramp.bot))?)?;
    Ok(grad)
}

/// White at `alpha`. A mask reads only the alpha channel, but the colour is kept
/// white so the same brush is legible when painted directly (the `grad-direct`
/// control) and so no interpolation-space hue behaviour muddies the ramp.
fn white(alpha: f32) -> Color {
    Color { A: (alpha * 255.0).round().clamp(0.0, 255.0) as u8, R: 255, G: 255, B: 255 }
}

/// `mask` supplies coverage, an FP16 surface at `src` supplies colour.
fn mask_brush(
    compositor: &Compositor,
    device: &GraphicsDevice,
    mask: &CompositionBrush,
    src: f32,
) -> Result<CompositionBrush> {
    let brush = compositor.CreateMaskBrush()?;
    brush.SetMask(mask)?;
    brush.SetSource(&device.solid_brush(compositor, src)?)?;
    brush.cast()
}

/// Capture an off-tree visual through a `CompositionVisualSurface` and hand back
/// a surface brush over it — the indirection the glow layer relies on.
fn capture_brush(
    compositor: &Compositor,
    source: &SpriteVisual,
    w: f32,
    h: f32,
) -> Result<CompositionBrush> {
    let surface = compositor.CreateVisualSurface()?;
    surface.SetSourceVisual(source)?;
    surface.SetSourceOffset(Vector2 { x: 0.0, y: 0.0 })?;
    surface.SetSourceSize(Vector2 { x: w, y: h })?;
    let brush = compositor.CreateSurfaceBrushWithSurface(&surface)?;
    brush.SetStretch(CompositionStretch::Fill)?;
    brush.cast()
}

// ── The FP16 source surfaces ────────────────────────────────────────────────

/// The app's Direct2D device wired to the compositor, and the surfaces drawn
/// through it. Every colour in this probe is written here, in `Rgba16Float`.
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

    /// A flat FP16 surface at scRGB `v`, stretched to fill whatever paints with
    /// it. This is the COLOUR half of every mask route.
    fn solid_brush(&self, compositor: &Compositor, v: f32) -> Result<CompositionBrush> {
        self.solid_brush_a(compositor, v, 1.0)
    }

    /// A flat FP16 surface at scRGB `v` and alpha `a` — [`solid_brush`] with the
    /// coverage left to the caller.
    ///
    /// The FOLD half of the flat-opacity comparison: authoring `a` here puts the
    /// fade in the app's own `Rgba16Float` buffer instead of on the visual, so
    /// nothing about it passes through a compositor property. Written as
    /// straight alpha exactly as [`ramp_brush`] writes each of its rows — D2D
    /// premultiplies into the target — so the two are the same construction and
    /// a difference between them cannot be an authoring difference.
    fn solid_brush_a(&self, compositor: &Compositor, v: f32, a: f32) -> Result<CompositionBrush> {
        let surface = self.surface(8, 8)?;
        self.draw(&surface, |ctx| unsafe {
            ctx.Clear(Some(&D2D_COLOR_F { r: v, g: v, b: v, a }));
        })?;
        let brush = compositor.CreateSurfaceBrushWithSurface(&surface)?;
        brush.SetStretch(CompositionStretch::Fill)?;
        brush.cast()
    }

    /// The CONTROL surface: one pixel row per output row, each filled with the
    /// exact scRGB colour and alpha that row's ramp calls for. Rows are filled
    /// individually rather than through a D2D gradient so the reference ramp is
    /// float-exact by construction and nothing about D2D's own interpolation is
    /// under test.
    fn ramp_brush(&self, compositor: &Compositor, ramp: Ramp) -> Result<CompositionBrush> {
        let surface = self.surface(COL_W, ramp.h)?;
        self.draw(&surface, |ctx| unsafe {
            ctx.Clear(Some(&D2D_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            let brush = ctx
                .CreateSolidColorBrush(&D2D_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }, None)
                .unwrap();
            for y in 0..ramp.h {
                let t = (y as f32 + 0.5) / ramp.h as f32;
                let a = ramp.top + (ramp.bot - ramp.top) * t;
                brush.SetColor(&D2D_COLOR_F { r: ramp.src, g: ramp.src, b: ramp.src, a });
                ctx.FillRectangle(
                    &D2D_RECT_F {
                        left: 0.0,
                        top: y as f32,
                        right: COL_W as f32,
                        bottom: (y + 1) as f32,
                    },
                    &brush,
                );
            }
        })?;
        let brush = compositor.CreateSurfaceBrushWithSurface(&surface)?;
        brush.SetStretch(CompositionStretch::Fill)?;
        brush.cast()
    }

    fn surface(&self, w: i32, h: i32) -> Result<windows::UI::Composition::CompositionDrawingSurface> {
        self.device.CreateDrawingSurface2(
            windows::Graphics::SizeInt32 { Width: w, Height: h },
            DirectXPixelFormat::R16G16B16A16Float,
            DirectXAlphaMode::Premultiplied,
        )
    }

    fn draw(
        &self,
        surface: &windows::UI::Composition::CompositionDrawingSurface,
        f: impl FnOnce(&ID2D1DeviceContext),
    ) -> Result<()> {
        let interop: ICompositionDrawingSurfaceInterop = surface.cast()?;
        let mut offset = POINT::default();
        let ctx: ID2D1DeviceContext = unsafe { interop.BeginDraw(None, &mut offset)? };
        unsafe {
            ctx.SetTransform(&Matrix3x2::translation(offset.x as f32, offset.y as f32));
        }
        f(&ctx);
        unsafe { interop.EndDraw().ok()? };
        Ok(())
    }
}

// ── Reporting ───────────────────────────────────────────────────────────────

/// Print the legend and the two probe command lines: a coarse one (a band per
/// route per row, for ceilings) and a per-row one down a single column (for
/// banding). Written out rather than described so the measurement is copy-paste
/// reproducible.
fn report(failures: &[String], pid: u32, client_w: i32) {
    println!("gradient-alpha probe — pid {pid}, client {client_w}x{CLIENT_H}");
    println!();
    for (i, (name, what)) in ROUTES.iter().enumerate() {
        let x = X0 + i as i32 * (COL_W + COL_GAP);
        println!("  col {i} x={x:<4} {name:<18} {what}");
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

    // Coarse: the top 6 rows of each strip (the ceiling) and a mid band.
    let mut coarse = String::new();
    coarse.push_str(&format!(" --probe {},{},{},{}", X0, ANCHOR_Y + 4, client_w - X0 * 2, 8));
    for (i, _) in ROUTES.iter().enumerate() {
        let x = X0 + i as i32 * (COL_W + COL_GAP) + 8;
        let w = COL_W - 16;
        coarse.push_str(&format!(" --probe {},{},{},{}", x, ROW_A_Y + 2, w, 6));
        coarse.push_str(&format!(" --probe {},{},{},{}", x, ROW_B_Y + 2, w, 6));
    }
    println!("# ceilings — anchor first, then (rowA top, rowB top) per route, in column order");
    println!("guishot --pid {pid} --client-only --out ceilings.png{coarse}");
    println!();

    // Per-row down row B, one route at a time: the banding profile.
    println!("# banding — one 1px row at a time down row B of column N (N = 0..{})", ROUTES.len() - 1);
    println!("#   set N, then run; each probe line is one output row, top to bottom");
    let mut rows = String::new();
    for y in 0..ROW_B_H {
        rows.push_str(&format!(" --probe $x,{},{},1", ROW_B_Y + y, COL_W - 16));
    }
    println!("$N=2; $x={X0}+$N*{}+8  # PowerShell: compute x, then", COL_W + COL_GAP);
    println!("guishot --pid {pid} --client-only --out band.png{rows}");
}

fn create_dispatcher_queue() -> Result<windows::System::DispatcherQueueController> {
    let options = DispatcherQueueOptions {
        dwSize: size_of::<DispatcherQueueOptions>() as u32,
        threadType: DQTYPE_THREAD_CURRENT,
        apartmentType: DQTAT_COM_ASTA,
    };
    let controller = unsafe { CreateDispatcherQueueController(options)? };
    controller.cast()
}
