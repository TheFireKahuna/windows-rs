//! Three presentation regions on a real window, on the settled queue layout: one `Solo`
//! and two on a shared card queue.
//!
//! **The point is what the front thread does, which is nothing.** After the one call that
//! binds each surface handle it parks in `GetMessage` and never wakes again, while three
//! regions draw and present at the display's rate. The counter printed at the end is that
//! claim, measured rather than asserted — a window message arriving during the run is
//! either the user touching it or a mechanism that should not exist.
//!
//! It also reports what the presents actually did. `flipped` is the only report a buffer
//! that reached a display plane makes at all, so a run with `composed` climbing and
//! `flipped` at zero is a region being drawn by DWM — which is the difference the whole
//! design is about, and it is not visible on screen.
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

/// Every message the window sees. Read once the bindings are in, so what it counts from
/// then on is the front thread's own cost.
static FRONT_MESSAGES: AtomicU64 = AtomicU64::new(0);
static BOUND: AtomicBool = AtomicBool::new(false);

const REGIONS: [(RegionKey, Queue, f32, f32, f32); 3] = [
    (RegionKey(1), Queue::Solo, 16.0, 16.0, 0.0),
    (RegionKey(2), Queue::Shared("cards"), 16.0, 236.0, 0.33),
    (RegionKey(3), Queue::Shared("cards"), 428.0, 236.0, 0.66),
];

/// The mastering statement. It bounds the **presented** channel, which is not the authored
/// one: the transform converts Rec.2020 working light to the display's primaries first, and
/// that conversion raises a channel wherever the authored colour sits outside the display's
/// gamut. So this carries headroom over the 180 nits the bars actually author — a peak
/// declared at exactly the authored maximum trips the assertion, which is how this number
/// was arrived at.
const PEAK_NITS: f32 = 300.0;

const REGION_W: f32 = 800.0;
const HERO_H: f32 = 200.0;
const CARD_W: f32 = 396.0;
const CARD_H: f32 = 140.0;

fn main() -> Result<()> {
    // Counting every message the window receives is what makes "the front thread does
    // nothing" a measurement rather than a claim. Deliberately unfiltered: a mechanism that
    // woke this thread once a frame would show up here whatever message it chose.
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

    // The binder runs on the **present thread**, and a composition object may only be
    // touched on the thread that owns the compositor — so it does the one thing it is
    // allowed to: hand the plain data across. This is the shape a real consumer has, with
    // a post to the window's queue where this has a channel.
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
            // No data source in this example, so nothing ever bumps it — the regions are
            // animating, which is what keeps the loop paced on the display clock.
            Arc::new(Epoch::new()?),
            Arc::new(RegionInput::new()),
            move |_gpu: &Gpu| Ok(Box::new(Bars::new(phase)) as Box<dyn Frame>),
        );
    }

    // Bind each region once, as its handle arrives. A real consumer does this from its own
    // message pump; here it is a bounded wait before the pump starts, so that everything
    // the counter sees afterwards belongs to the run rather than to start-up.
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
        // The buffer is already at device resolution, so it samples one to one and is
        // never stretched. Every pixel guarantee a presented region makes rests on that.
        brush.set_stretch(Stretch::None);
        brush.set_alignment_ratio(0.0, 0.0);
        visual.set_brush(&brush);
    }
    BOUND.store(true, Ordering::Release);
    println!("bound 3 regions; front thread parking. Close the window to stop.\n");

    let start = std::time::Instant::now();
    // Dropping the sender is what ends the reporter, and a disconnect wakes it at once —
    // so closing the window exits now rather than up to a report period later. A scope
    // joins what it spawned, so a reporter that only ever slept would hang the process
    // here forever with the window already gone.
    let (stop, stopped) = channel::<()>();
    std::thread::scope(|scope| {
        // A reporter, so the numbers are readable while it runs rather than only on a
        // clean exit. It is a third thread on purpose: reporting from the front thread
        // would be the very wake this example exists to show the absence of.
        let watched = &presenter;
        scope.spawn(move || {
            while let Err(RecvTimeoutError::Timeout) =
                stopped.recv_timeout(std::time::Duration::from_secs(2))
            {
                report(watched, start.elapsed().as_secs_f64());
            }
        });
        // Blocks in `GetMessage`, which is the whole demonstration: a steady producer at
        // display rate posts nothing to this thread.
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

/// A field of bars whose heights ease continuously, so the region has a reason to present
/// every refresh without any data behind it.
struct Bars {
    phase: f32,
    /// Advanced once per `draw`, so a batch of three draws steps three frames — the eases
    /// cannot tell that the calls arrived together.
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

        // Opaque, so the first act is a full-cover clear. Claiming `opaque()` without one
        // is the single way to be eligible for a plane on paper and composed in practice.
        let ink = ctx.out.apply(Radiance::new(6.0, 7.0, 9.0, 1.0));
        draw.clear(ink);

        let (w, h) = (ctx.w(), ctx.h());
        let step = w / BARS as f32;
        for bar in 0..BARS {
            let x = bar as f32 * step;
            let wave = (t + bar as f32 * 0.22).sin() * 0.5 + 0.5;
            let top = h - (h - 8.0) * (0.15 + 0.85 * wave);
            // Above diffuse white at the peaks, which is what the FP16 buffer and the
            // output transform exist to carry.
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
