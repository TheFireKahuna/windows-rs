//! Three presentation regions on a real window: one `Solo` and two on a shared card queue.
//!
//! The front thread does nothing while they run. After the one call that binds each surface
//! handle it parks in `GetMessage` and does not wake again, while the three regions draw and
//! present at the display's rate. The counter printed during and after the run measures that:
//! a window message arriving once the bindings are in came from the user touching the window.
//!
//! The run also reports what the presents did. `flipped` is the only report a buffer that
//! reached a display plane makes, so `composed` climbing with `flipped` at zero is a region
//! DWM is drawing — a difference that does not show on screen.
//!
//! ```text
//! cargo run -p windows-present --example regions
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};

use windows_color::{DisplayCapability, OutputTransform, Radiance};
use windows_composition::{Compositor, Stretch};
use windows_present::{
    Bound, Draw, Epoch, Extent, Frame, FrameCtx, Gpu, Presenter, Queue, Rect, RegionInput,
    RegionKey, RegionSpec, Result, Tuning,
};
use windows_window::Window;

/// Counts every message the window sees. Started once the bindings are in, so what it counts
/// is the front thread's cost during the run.
static FRONT_MESSAGES: AtomicU64 = AtomicU64::new(0);
static BOUND: AtomicBool = AtomicBool::new(false);

const REGIONS: [(RegionKey, Queue, f32, f32, f32); 3] = [
    (RegionKey(1), Queue::Solo, 16.0, 16.0, 0.0),
    (RegionKey(2), Queue::Shared("cards"), 16.0, 236.0, 0.33),
    (RegionKey(3), Queue::Shared("cards"), 428.0, 236.0, 0.66),
];

/// The mastering peak. It bounds the presented channel rather than the authored one: the
/// transform converts Rec.2020 working light to the display's primaries first, and that
/// conversion raises a channel wherever the authored colour sits outside the display's gamut.
/// It therefore carries headroom over the 180 nits the bars author; a peak declared at
/// exactly the authored maximum trips the transform's assertion.
const PEAK_NITS: f32 = 300.0;

const REGION_W: f32 = 800.0;
const HERO_H: f32 = 200.0;
const CARD_W: f32 = 396.0;
const CARD_H: f32 = 140.0;

fn main() -> Result<()> {
    // Unfiltered on purpose: anything that woke this thread once a frame is counted here
    // whatever message it chose.
    let window = Window::new("presentation regions")
        .size(840, 420)
        .on_message(|_, _, _, _| {
            if BOUND.load(Ordering::Relaxed) {
                FRONT_MESSAGES.fetch_add(1, Ordering::Relaxed);
            }
            None
        })
        .create()?;
    // No controller is minted here: creating the window already gave this thread a
    // dispatcher queue, and a second controller on one thread is an error.
    let compositor = Compositor::new()?;
    let target = compositor.create_desktop_window_target(&window, false)?;
    let root = compositor.create_container_visual();
    target.set_root(&root);

    // The binder runs on the present thread, and a composition object may only be touched on
    // the thread that owns the compositor, so it only hands the plain data across. A real
    // consumer posts to the window's queue where this sends on a channel.
    let (tx, rx) = channel::<(RegionKey, Bound)>();
    let presenter = Presenter::spawn(
        Tuning {
            statistics: true,
            ..Tuning::default()
        },
        OutputTransform::for_display(DisplayCapability::Sdr, PEAK_NITS),
        // Without this the loop keeps drawing and presenting while the window is
        // minimized, cloaked, or the display is off — and has no way to find out.
        Some(window.watch()?),
        Box::new(move |key, bound| send(&tx, key, bound)),
    )?;

    let mut visuals = Vec::new();
    for (key, queue, x, y, phase) in REGIONS {
        let (w, h) = if queue == Queue::Solo {
            (REGION_W, HERO_H)
        } else {
            (CARD_W, CARD_H)
        };
        let visual = compositor.create_sprite_visual();
        visual.set_offset(x, y, 0.0);
        visual.set_size(w, h);
        root.children().insert_at_top(&visual);
        visuals.push((key, visual));

        presenter.mount(
            RegionSpec {
                key,
                queue,
                extent: Extent::new(w, h, 96.0),
            },
            // No data source here, so nothing bumps it. The regions report themselves as
            // animating, which is what keeps the loop paced on the display clock.
            Arc::new(Epoch::new()?),
            Arc::new(RegionInput::new()),
            move |_gpu: &Gpu| Ok(Box::new(Bars::new(phase)) as Box<dyn Frame>),
        );
    }

    // Bind each region once, as its handle arrives. A real consumer does this from its own
    // message pump; here it is a bounded wait before the pump starts, so everything the
    // counter sees afterwards belongs to the run rather than to start-up.
    for _ in 0..REGIONS.len() {
        let Ok((key, bound)) = rx.recv_timeout(std::time::Duration::from_secs(5)) else {
            eprintln!("a region never bound — no presentation support on this machine?");
            return Ok(());
        };
        let Bound::Surface { handle, .. } = bound else {
            continue;
        };
        let Some((_, visual)) = visuals.iter().find(|(k, _)| *k == key) else {
            continue;
        };
        // SAFETY: the handle is live until the region unmounts, and the presenter outlives
        // the visual tree below — it is dropped after `run` returns.
        let surface = unsafe { compositor.create_surface_for_handle(handle as *mut _)? };
        let brush = compositor.create_surface_brush(&surface);
        // The buffer is already at device resolution, so it samples one to one and is never
        // stretched. Every pixel guarantee a presented region makes rests on that.
        brush.set_stretch(Stretch::None);
        brush.set_alignment_ratio(0.0, 0.0);
        visual.set_brush(&brush);
    }
    BOUND.store(true, Ordering::Release);
    println!("bound 3 regions; front thread parking. Close the window to stop.\n");

    let start = std::time::Instant::now();
    // Dropping the sender ends the reporter, and the disconnect wakes it at once, so closing
    // the window exits immediately rather than up to a report period later. A scope joins
    // what it spawned, so a reporter that only slept would hang the process here with the
    // window already gone.
    let (stop, stopped) = channel::<()>();
    std::thread::scope(|scope| {
        // A reporter, so the numbers are readable while the run continues. It is a third
        // thread because reporting from the front thread would be the wake this example
        // shows the absence of.
        let watched = &presenter;
        scope.spawn(move || {
            while let Err(RecvTimeoutError::Timeout) =
                stopped.recv_timeout(std::time::Duration::from_secs(2))
            {
                report(watched, start.elapsed().as_secs_f64());
            }
        });
        // Blocks in `GetMessage`: a steady producer at display rate posts nothing to this
        // thread.
        windows_window::run();
        drop(stop);
        report(&presenter, start.elapsed().as_secs_f64());
        Ok(())
    })
}

fn report(presenter: &Presenter, seconds: f64) {
    let messages = FRONT_MESSAGES.load(Ordering::Acquire);
    let t = presenter.tally();
    println!(
        "{seconds:5.1}s  front-thread messages {messages:5} ({:5.1}/s)   \
         queued {:6} skipped {:5} | flip {:6} scanout {:6} composed {:6}",
        messages as f64 / seconds,
        t.queued,
        t.skipped,
        t.flipped,
        t.scanout,
        t.composed,
    );
}

fn send(tx: &Sender<(RegionKey, Bound)>, key: RegionKey, bound: Bound) {
    let _ = tx.send((key, bound));
}

/// Draws a field of bars whose heights ease continuously, so the region presents every
/// refresh with no data behind it.
struct Bars {
    /// Offsets this region's waveform from its neighbours'.
    phase: f32,
    /// Advanced once per `draw`, so a batch of three draws steps three frames and the eases
    /// run as though the calls arrived one per refresh.
    frame: u32,
}

impl Bars {
    fn new(phase: f32) -> Self {
        Self { phase, frame: 0 }
    }
}

const BARS: usize = 48;

impl Frame for Bars {
    fn should_draw(&mut self, _ctx: FrameCtx<'_>) -> bool {
        true
    }

    fn draw(&mut self, ctx: FrameCtx<'_>, draw: &Draw<'_>) {
        let t = self.frame as f32 / 60.0 + self.phase * 10.0;
        self.frame = self.frame.wrapping_add(1);

        // `opaque()` answers true, so the first act is a full-cover clear. Claiming opacity
        // without one leaves the region eligible for a plane on paper and composed in fact.
        let ink = ctx.out.apply(Radiance::new(6.0, 7.0, 9.0, 1.0));
        draw.clear(ink);

        let (w, h) = (ctx.w(), ctx.h());
        let step = w / BARS as f32;
        for bar in 0..BARS {
            let x = bar as f32 * step;
            let wave = (t + bar as f32 * 0.22).sin() * 0.5 + 0.5;
            let top = h - (h - 8.0) * (0.15 + 0.85 * wave);
            // Above diffuse white at the peaks, which the FP16 buffer and the output
            // transform carry.
            let lit = Radiance::new(30.0 + 150.0 * wave, 60.0 + 90.0 * wave, 180.0, 1.0);
            let Ok(brush) = ctx.device.solid(ctx.out.apply(lit)) else {
                return;
            };
            let rect = Rect::new(x + 1.0, top, x + step - 1.0, h - 4.0);
            draw.fill(draw.snap_rect(rect), &brush);
        }
    }

    fn opaque(&self) -> bool {
        true
    }

    fn animating(&self) -> bool {
        true
    }
}
