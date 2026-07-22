//! A/B spike: retained `CompositionPathGeometry` updated at engine-publish rate
//! versus a `CompositionDrawingSurface` repainted at the same rate.
//!
//! One binary, three modes selected by `SPIKE_MODE`:
//!
//! * `a0` — static retained path (the `dcomp_gradient_path` content, unchanged).
//!   The harness control: this must sit at ~0% CPU or nothing else measured here
//!   means anything.
//! * `a1` — the same retained path, its geometry recomputed and re-declared every
//!   tick (an animated bell set stands in for the engine's publish).
//! * `b1` — the same curve drawn with Direct2D into a live
//!   `CompositionDrawingSurface`, repainted every tick on a worker thread. This is
//!   the model the NewAPO analyzer uses today.
//!
//! `SPIKE_N` sets the sample count along the curve (default 160). A tick is one
//! **compositor frame** (`DCompositionWaitForCompositorClock`, the pacing the viz
//! compositor actually uses); `SPIKE_HZ=100` swaps that for a sleep-paced fixed
//! rate instead.
//!
//! Run with:
//!   SPIKE_MODE=a1 SPIKE_N=512 cargo run -p windows-reactor \
//!       --example spike_path_vs_surface --features dcomp-backend --release

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use windows_canvas::{ColorF, GpuDevice, GradientStop, PathBuilder, Vector2 as CVec2};
use windows_reactor::*;

/// Client size, identical for every cell so the compositor's per-present cost is
/// comparable across modes.
const WIN_W: f64 = 1200.0;
const WIN_H: f64 = 700.0;
/// Plot box, in DIPs.
const W: f64 = 1080.0;
const H: f64 = 480.0;
windows_core::link!("dcomp.dll" "system" fn DCompositionWaitForCompositorClock(count: u32, handles: *const core::ffi::c_void, timeoutinms: u32) -> u32);

/// How the update thread is paced.
///
/// The default is the **compositor clock** — the same
/// `DCompositionWaitForCompositorClock` the reactor frame pacer and the NewAPO viz
/// compositor block on, so a tick is one DWM frame (143 Hz on this display) and the
/// updates land in phase with the presents rather than beating against them. A
/// sleep-paced rate is available for a fixed engine-publish rate (`SPIKE_HZ=100`),
/// which is deliberately NOT the compositor tick.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Pace {
    CompositorClock,
    Fixed(u64),
}

fn pace() -> Pace {
    static PACE: std::sync::OnceLock<Pace> = std::sync::OnceLock::new();
    *PACE.get_or_init(|| {
        match std::env::var("SPIKE_HZ").ok().and_then(|s| s.parse::<u64>().ok()) {
            Some(h) if h >= 1 => Pace::Fixed(h),
            _ => Pace::CompositorClock,
        }
    })
}

/// Block until the next update is due. One compositor frame, or one sleep period.
fn wait_tick(next: &mut Instant) {
    match pace() {
        Pace::CompositorClock => {
            // No wait handles: return on the next compositor frame. The 1 s guard is
            // the pacer's — a compositor that stops ticking must not park forever.
            unsafe { DCompositionWaitForCompositorClock(0, core::ptr::null(), 1000) };
        }
        Pace::Fixed(h) => {
            *next += Duration::from_nanos(1_000_000_000 / h);
            let now = Instant::now();
            if *next > now {
                std::thread::sleep(*next - now);
            } else {
                *next = now;
            }
        }
    }
}

/// Process start, so the animated phase is wall-clock driven and therefore identical
/// across pacings — the curve moves at the same speed whatever the update rate.
fn phase_now() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1.6
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Mode {
    /// Static retained path.
    A0,
    /// Retained path, geometry rebuilt every tick.
    A1,
    /// Composition drawing surface, repainted every tick.
    B1,
}

fn mode() -> Mode {
    match std::env::var("SPIKE_MODE").unwrap_or_default().to_ascii_lowercase().as_str() {
        "a1" => Mode::A1,
        "b1" => Mode::B1,
        _ => Mode::A0,
    }
}

fn samples() -> usize {
    std::env::var("SPIKE_N")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n >= 2)
        .unwrap_or(160)
}

/// Frames the app actually produced — renders in A0/A1, surface draws in B1.
static FRAMES: AtomicU64 = AtomicU64::new(0);

/// A bell in log-x, the shape a peaking EQ band puts on a response plot.
fn bell(x: f64, centre: f64, width: f64, gain: f64) -> f64 {
    let t = (x - centre) / width;
    gain * (-t * t).exp()
}

/// The curve's y at fraction `t` across the box, in DIPs. `phase` moves the bells
/// so the geometry genuinely differs from tick to tick.
fn curve_y(t: f64, phase: f64) -> f64 {
    let db = bell(t, 0.22 + 0.06 * (phase).sin(), 0.10, 7.5 + 3.0 * (phase * 1.7).sin())
        + bell(t, 0.55, 0.07, -5.0 + 2.5 * (phase * 0.9).sin())
        + bell(t, 0.82 + 0.05 * (phase * 1.3).cos(), 0.12, 4.0 + 2.0 * (phase * 0.6).cos());
    H * 0.5 - (db / 12.0) * (H * 0.5 - 12.0)
}

/// Report the achieved frame rate every 5 s, so a cell that silently fell short of
/// `HZ` is visible in the log rather than hidden in the CPU number.
fn spawn_rate_report(tag: &'static str) {
    std::thread::spawn(move || {
        let mut last = FRAMES.load(Ordering::Relaxed);
        let mut at = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let now = FRAMES.load(Ordering::Relaxed);
            let dt = at.elapsed().as_secs_f64();
            println!("[{tag}] {:.1} frames/s ({} total)", (now - last) as f64 / dt, now);
            last = now;
            at = Instant::now();
        }
    });
}

/// A `GpuDevice` on its way to the drawing thread.
///
/// SAFETY: the device comes from the backend, which builds it with a
/// multi-threaded Direct2D factory (`GpuDevice::new_multi_threaded`); Direct2D
/// serializes access across threads. It is moved — not shared — to the one worker
/// that draws with it. `Sync` is deliberately not asserted.
struct DeviceForWorker(GpuDevice);
unsafe impl Send for DeviceForWorker {}
impl DeviceForWorker {
    fn take(self) -> GpuDevice {
        self.0
    }
}

// ── A0 / A1: retained sprite path ────────────────────────────────────────────

/// The retained-path plot: an underfill with a vertical ramp, the line with a
/// horizontal ramp and a compositor glow. Identical content in A0 and A1; only
/// `phase` differs (A0 pins it at 0).
fn retained_plot(n: usize, phase: f64) -> Element {
    let line = ShapePath::with_capacity(n)
        .polyline((0..n).map(|i| {
            let t = i as f64 / (n - 1) as f64;
            (t * W, curve_y(t, phase))
        }))
        .build();

    let mut area = ShapePath::with_capacity(n + 3).move_to(0.0, H);
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        area = area.line_to(t * W, curve_y(t, phase));
    }
    let area = area.line_to(W, H).close().build();

    let accent = Color::rgb(0x38, 0xBD, 0xF8);

    let underfill = Shape::path(area)
        .fill_gradient(vec![
            (0.0, Color::rgba(0x38, 0xBD, 0xF8, 0x66)),
            (1.0, Color::rgba(0x38, 0xBD, 0xF8, 0x00)),
        ])
        .width(W)
        .height(H)
        .canvas_left(0.0)
        .canvas_top(0.0);

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

    let baseline = Shape::rectangle()
        .fill(Color::rgba(0xFF, 0xFF, 0xFF, 0x14))
        .width(W)
        .height(1.0)
        .canvas_left(0.0)
        .canvas_top(H * 0.5);

    let layers: Vec<Element> = vec![baseline.into(), underfill.into(), response.into()];
    Canvas::new(layers).width(W).height(H).into()
}

// ── B1: Direct2D into a live composition surface ─────────────────────────────

/// The surface's brushes, built once and reused — the production draw keeps them in
/// its `DrawKit`, and a gradient stop collection rebuilt per frame would charge B1
/// for work the real code does not do.
struct Kit {
    baseline: windows_canvas::Brush,
    under: windows_canvas::LinearGradient,
    halo: windows_canvas::Brush,
    line: windows_canvas::LinearGradient,
}

fn make_kit(session: &windows_canvas::DrawingSession<'_>) -> Option<Kit> {
    Some(Kit {
        baseline: session.create_solid_brush(ColorF::from_rgba8(0xFF, 0xFF, 0xFF, 0x14)).ok()?,
        under: session
            .create_linear_gradient(
                CVec2::new(0.0, 0.0),
                CVec2::new(0.0, H as f32),
                &[
                    GradientStop::new(0.0, ColorF::from_rgba8(0x38, 0xBD, 0xF8, 0x66)),
                    GradientStop::new(1.0, ColorF::from_rgba8(0x38, 0xBD, 0xF8, 0x00)),
                ],
            )
            .ok()?,
        halo: session.create_solid_brush(ColorF::from_rgba8(0x38, 0xBD, 0xF8, 0x44)).ok()?,
        line: session
            .create_linear_gradient(
                CVec2::new(0.0, 0.0),
                CVec2::new(W as f32, 0.0),
                &[
                    GradientStop::new(0.00, ColorF::from_rgba8(0x34, 0xD3, 0x99, 0xFF)),
                    GradientStop::new(0.35, ColorF::from_rgba8(0x38, 0xBD, 0xF8, 0xFF)),
                    GradientStop::new(0.70, ColorF::from_rgba8(0xA7, 0x8B, 0xFA, 0xFF)),
                    GradientStop::new(1.00, ColorF::from_rgba8(0xFB, 0x71, 0x85, 0xFF)),
                ],
            )
            .ok()?,
    })
}

/// One frame of the same curve, drawn with Direct2D in DIP space. Mirrors what a
/// viz surface draw does: clear, fill the area under the curve with a vertical
/// ramp, lay a soft halo under the line, stroke the line with a horizontal ramp.
fn draw_curve(
    session: &windows_canvas::DrawingSession<'_>,
    device: &GpuDevice,
    kit: &mut Option<Kit>,
    n: usize,
    phase: f64,
) {
    session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
    if kit.is_none() {
        *kit = make_kit(session);
    }
    let Some(kit) = kit.as_ref() else { return };

    // The closed area under the curve.
    let Ok(builder) = PathBuilder::new(device) else { return };
    let mut fig = builder.begin(CVec2::new(0.0, H as f32));
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        fig = fig.line_to(CVec2::new((t * W) as f32, curve_y(t, phase) as f32));
    }
    let Ok(area) = fig.line_to(CVec2::new(W as f32, H as f32)).close().build() else { return };

    // The open line.
    let Ok(builder) = PathBuilder::new(device) else { return };
    let mut fig = builder.begin_hollow(CVec2::new(0.0, curve_y(0.0, phase) as f32));
    for i in 1..n {
        let t = i as f64 / (n - 1) as f64;
        fig = fig.line_to(CVec2::new((t * W) as f32, curve_y(t, phase) as f32));
    }
    let Ok(line) = fig.end_open().build() else { return };

    // 0 dB reference.
    session.fill_rect(
        &windows_canvas::Rect {
            left: 0.0,
            top: (H * 0.5) as f32,
            right: W as f32,
            bottom: (H * 0.5) as f32 + 1.0,
        },
        &kit.baseline,
    );

    // Underfill: vertical ramp, opaque at the line and gone by the baseline.
    session.fill_path(&area, &kit.under);

    // Halo. The retained path gets this from a compositor DropShadow; on a surface
    // it is a wide translucent stroke, which is what the drawn code does today.
    session.draw_path(&line, &kit.halo, 7.0);

    // The line: horizontal ramp, so the colour tracks the x axis.
    session.draw_path(&line, &kit.line, 2.5);
}

/// Mount a live composition surface and drive it from a worker thread at `HZ`.
/// A trimmed copy of the NewAPO viz host: request the surface from the backend,
/// take the `Send` drawing half, draw on a worker. No resize / device-loss paths —
/// the spike window never resizes.
fn surface_plot(cx: &mut RenderCx, n: usize) -> Element {
    let dpi = cx.use_dpi() as f32;
    let scale = (dpi / 96.0).max(0.5);
    let (size, set_size) = cx.use_state::<(u32, u32)>((0, 0));
    let (generation, bump) = cx.use_async_state::<u32>(0);

    let element = cx.use_ref::<Option<ElementHandle>>(None);
    let size_revoker = cx.use_ref::<Option<Subscription>>(None);
    let pending = cx.use_ref::<Option<PendingSurface>>(None);
    let live = cx.use_ref::<Option<PendingSurface>>(None);

    cx.use_effect((size, generation), {
        let (element, pending, live) = (element.clone(), pending.clone(), live.clone());
        let bump_ready = bump.clone();
        move || {
            let (w, h) = size;
            if w == 0 || h == 0 || live.borrow().is_some() {
                return;
            }
            let Some(element) = element.borrow().clone() else { return };
            let Some((dev, dev_gen)) = backend_gpu_device() else { return };

            let ready = pending.borrow().as_ref().and_then(|p| p.take());
            let Some(draw_surface) = ready else {
                if pending.borrow().is_none() {
                    let pixel = ((w as f32 * scale) as i32, (h as f32 * scale) as i32);
                    let Ok(device) = SurfaceDevice::new(dev.d2d_device(), dev_gen) else { return };
                    let next = generation.wrapping_add(1);
                    let bump = bump_ready.clone();
                    *pending.borrow_mut() = Some(request_surface(
                        element.id(),
                        device,
                        pixel,
                        (w as f32, h as f32),
                        false,
                        move || bump.call(next),
                    ));
                }
                return;
            };
            let hosted = pending.borrow_mut().take().expect("pending was just read");

            let target = CompositionDrawTarget::new(draw_surface);
            let dev = DeviceForWorker(dev);
            std::thread::spawn(move || {
                let dev = dev.take();
                let mut kit: Option<Kit> = None;
                let mut next = Instant::now();
                loop {
                    let phase = phase_now();
                    if target
                        .draw(scale, |session| draw_curve(session, &dev, &mut kit, n, phase))
                        .is_err()
                    {
                        eprintln!("[b1] surface draw failed");
                    }
                    FRAMES.fetch_add(1, Ordering::Relaxed);
                    wait_tick(&mut next);
                }
            });
            *live.borrow_mut() = Some(hosted);
        }
    });

    border(text_block(""))
        .width(W)
        .height(H)
        .on_mounted({
            let (element, size_revoker) = (element.clone(), size_revoker.clone());
            move |handle| {
                *element.borrow_mut() = Some(handle.clone());
                let set_size = set_size.clone();
                if let Ok(rev) =
                    handle.on_size_changed(move |w, h| {
                        set_size.call((w.round().max(0.0) as u32, h.round().max(0.0) as u32));
                    })
                {
                    *size_revoker.borrow_mut() = Some(rev);
                }
            }
        })
        .into()
}

fn main() -> windows_reactor::Result<()> {
    let mode = mode();
    let n = samples();
    println!(
        "spike: mode={mode:?} N={n} window={WIN_W}x{WIN_H} plot={W}x{H} pace={:?}",
        pace()
    );
    spawn_rate_report(match mode {
        Mode::A0 => "a0",
        Mode::A1 => "a1",
        Mode::B1 => "b1",
    });

    let app = move |cx: &mut RenderCx| -> Element {
        // Hooks are unconditional; only the element output branches on the mode.
        let (tick, bump_tick) = cx.use_async_state::<u32>(0);
        let started = cx.use_ref::<bool>(false);

        cx.use_effect((), {
            let started = started.clone();
            move || {
                if mode != Mode::A1 || *started.borrow() {
                    return;
                }
                *started.borrow_mut() = true;
                std::thread::spawn(move || {
                    let mut next = Instant::now();
                    let mut t: u32 = 0;
                    loop {
                        t = t.wrapping_add(1);
                        bump_tick.call(t);
                        wait_tick(&mut next);
                    }
                });
            }
        });

        let plot = match mode {
            Mode::A0 => retained_plot(n, 0.0),
            Mode::A1 => {
                FRAMES.fetch_add(1, Ordering::Relaxed);
                // `tick` is what makes the state change (and so the render happen);
                // the geometry itself follows wall-clock, like B1's.
                let _ = tick;
                retained_plot(n, phase_now())
            }
            Mode::B1 => surface_plot(cx, n),
        };

        let card = vstack((
            text_block(match mode {
                Mode::A0 => "A0 — retained path, static",
                Mode::A1 => "A1 — retained path, rebuilt per tick",
                Mode::B1 => "B1 — composition surface, repainted per tick",
            })
            .font_size(22.0)
            .semibold(),
            text_block(format!("N = {n} samples · pace {:?}", pace()))
                .font_size(13.0)
                .foreground(Color::rgb(0x9A, 0x9A, 0xA2)),
            plot,
        ))
        .spacing(10.0);

        border(card)
            .background(Color::rgb(0x18, 0x18, 0x1C))
            .corner_radius(14.0)
            .padding(Thickness::uniform(16.0))
            .margin(Thickness::uniform(12.0))
            .into()
    };

    // One title per mode+N so a screenshot can attach to the right window.
    let title = format!("spike {:?} N={n}", mode);
    DCompHost::render_sized(title, WIN_W, WIN_H, app)
}
