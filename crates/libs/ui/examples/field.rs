//! Native proof of the shipping field driver.
//!
//! `--drive` checks ordinary character input and that a focused caret costs no input tick.
//! `--gates` runs the acceptance measurements on top of it: how long a keystroke takes to
//! reach shaped geometry and an applied scene, how many fields one keystroke reshapes, what
//! each of the four threads does at focused idle with and without a region presenting beside
//! the field, and what repeated focus and unmount cost in allocations and working set.
//!
//! Input methods and the touch keyboard are not measured here. This host exposes one keyboard
//! layout, and a posted `WM_CHAR` is not evidence about either.
use std::alloc::{GlobalAlloc, Layout as AllocLayout, System};
use std::cell::Cell as ThreadCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows_color::{Ictcp, Radiance};
use windows_composition::Compositor;
use windows_core::Result;
use windows_d2d::Gpu;
use windows_present::{DrawCtx, Draw, Frame, GateCtx, Queue};
use windows_scene::{BackdropSpec, Backends, quant_stop};
use windows_text::{FamilyId, FontLadder, FontSpec};
use windows_ui::driver::{UiRuntime, observe};
use windows_ui::layout::{Len, layer, scroll, stack};
use windows_ui::present::Live;
use windows_ui::role::*;
use windows_ui::signal::Cell;
use windows_ui::text_input::InputScope;
use windows_ui::widget::{button, field, label};
use windows_window::Window;

// ── what each thread allocated ──────────────────────────────────────────────────

thread_local! {
    /// This thread's allocation count. `const` init, so the first access neither allocates
    /// nor registers a destructor; either would re-enter the allocator.
    static ALLOCATIONS: ThreadCell<u64> = const { ThreadCell::new(0) };
}

/// Returns the allocations this thread has made through the Rust allocator.
///
/// DirectWrite and the compositor allocate on the process heap through COM, so a figure read
/// on the app thread is this crate's own and not the shaping engine's. The working set is what
/// carries both.
fn allocations() -> u64 {
    ALLOCATIONS.try_with(ThreadCell::get).unwrap_or(0)
}

struct Counting;

// SAFETY: every method forwards to `System` with the arguments it was given and returns what
// `System` returned, so every guarantee is `System`'s. The only added effect is an increment on
// a thread-local `Cell<u64>`, which allocates nothing and cannot unwind.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: AllocLayout) -> *mut u8 {
        ALLOCATIONS.try_with(|n| n.set(n.get() + 1)).ok();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: AllocLayout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: AllocLayout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.try_with(|n| n.set(n.get() + 1)).ok();
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn alloc_zeroed(&self, layout: AllocLayout) -> *mut u8 {
        ALLOCATIONS.try_with(|n| n.set(n.get() + 1)).ok();
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

// ── what the driving thread reads the running window through ────────────────────

/// What one input tick settled on, as the thread driving the window reads it.
///
/// Every field is written by the input tick's observer and read by the driving thread, so each
/// is one word and none of them is ordered against another: a reader takes whichever whole
/// value is current, and the sequence it belongs to is named by `ticks`.
struct Gauge {
    /// What every time below is measured from, so an instant crosses to the driving thread as
    /// one word rather than through a lock on the input thread's own path.
    epoch: Instant,
    ticks: AtomicU64,
    scene_wakes: AtomicU64,
    scene_applies: AtomicU64,
    /// When the scene thread finished its last apply, in nanoseconds from `epoch`.
    scene_applied_ns: AtomicU64,
    /// The focused field's box in screen pixels, packed as two pairs of `f32` bits, or zero
    /// while no field holds focus.
    focused_field: AtomicU64,
    focused_field_x: AtomicU64,
    app_flushes: AtomicU64,
    field_shapes: AtomicU64,
    /// Allocations the input thread had made as of the last tick.
    input_allocations: AtomicU64,
    /// Allocations the app thread had made as of the last completed edit, and how many edits
    /// have been delivered there. The count is what the allocations are divided by, so a burst
    /// that delivered none is reported as unmeasured rather than as zero.
    app_allocations: AtomicU64,
    app_commits: AtomicU64,
    /// Frames the region beside the field has drawn.
    region_frames: AtomicU64,
    /// Whether that region is asking for the display clock.
    region_running: AtomicU64,
}

impl Default for Gauge {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            ticks: AtomicU64::new(0),
            scene_wakes: AtomicU64::new(0),
            scene_applies: AtomicU64::new(0),
            scene_applied_ns: AtomicU64::new(0),
            focused_field: AtomicU64::new(0),
            focused_field_x: AtomicU64::new(0),
            app_flushes: AtomicU64::new(0),
            field_shapes: AtomicU64::new(0),
            input_allocations: AtomicU64::new(0),
            app_allocations: AtomicU64::new(0),
            app_commits: AtomicU64::new(0),
            region_frames: AtomicU64::new(0),
            region_running: AtomicU64::new(0),
        }
    }
}

/// Packs two `f32` into one word, so a pair crosses as one value rather than two that can be
/// read from different ticks.
fn pack(a: f32, b: f32) -> u64 {
    (u64::from(a.to_bits()) << 32) | u64::from(b.to_bits())
}

fn unpack(word: u64) -> (f32, f32) {
    (
        f32::from_bits((word >> 32) as u32),
        f32::from_bits(word as u32),
    )
}

/// One reading of every counter, taken in one call so a report names one moment.
#[derive(Copy, Clone, Default, PartialEq, Debug)]
struct Reading {
    ticks: u64,
    scene_wakes: u64,
    scene_applies: u64,
    app_flushes: u64,
    field_shapes: u64,
    input_allocations: u64,
    region_frames: u64,
    /// When the scene thread finished its last apply, in nanoseconds from the gauge's epoch.
    scene_applied_ns: u64,
    /// The focused field's top and bottom in screen pixels, or `None` while none holds focus.
    focused_field: Option<(f32, f32)>,
}

impl Gauge {
    fn read(&self) -> Reading {
        Reading {
            ticks: self.ticks.load(Ordering::Acquire),
            scene_wakes: self.scene_wakes.load(Ordering::Acquire),
            scene_applies: self.scene_applies.load(Ordering::Acquire),
            app_flushes: self.app_flushes.load(Ordering::Acquire),
            field_shapes: self.field_shapes.load(Ordering::Acquire),
            input_allocations: self.input_allocations.load(Ordering::Acquire),
            region_frames: self.region_frames.load(Ordering::Acquire),
            scene_applied_ns: self.scene_applied_ns.load(Ordering::Acquire),
            focused_field: match self.focused_field.load(Ordering::Acquire) {
                0 => None,
                word => Some(unpack(word)),
            },
        }
    }

    /// Returns nanoseconds from this gauge's epoch to now.
    fn at(&self, when: Instant) -> u64 {
        when.duration_since(self.epoch).as_nanos() as u64
    }
}

/// A region that redraws every frame it is given, so the idle gate is measured with something
/// presenting beside the field rather than over a still window.
///
/// It reads and writes the same gauge the driving thread does: the present thread is the only
/// writer of the frame count and the only reader of the switch, so neither needs a seam.
struct Pulse {
    gauge: Arc<Gauge>,
}

impl Frame for Pulse {
    fn should_draw(&mut self, _: GateCtx<'_>) -> bool {
        self.gauge.region_running.load(Ordering::Acquire) != 0
    }

    fn draw(&mut self, ctx: DrawCtx<'_>, draw: &Draw<'_>) {
        let n = self.gauge.region_frames.fetch_add(1, Ordering::AcqRel);
        // Every pixel of the box, so the region can be scanned out rather than composed, and a
        // value that moves every frame, so the buffer's bytes actually differ.
        let nits = 2.0 + 0.5 * ((n % 32) as f32 / 32.0);
        draw.clear(ctx.out.apply(light(nits, 0.004, ACCENT_HUE)));
    }

    fn opaque(&self) -> bool {
        true
    }

    fn animating(&self) -> bool {
        self.gauge.region_running.load(Ordering::Acquire) != 0
    }
}

/// Records one completed edit: what the app thread had allocated by it, and the text it
/// carried.
///
/// Runs where the edit is delivered, which is the app thread, so the allocation figure is that
/// thread's own and not the input thread's. Every field reports here, so which one the tab
/// order reached does not decide what is measured.
fn probe(gauge: &Gauge, commits: &Commits, text: &str) {
    gauge
        .app_allocations
        .store(allocations(), Ordering::Release);
    gauge.app_commits.fetch_add(1, Ordering::AcqRel);
    commits.lock().unwrap().push(text.into());
}

/// The process's working set in bytes, or zero where the system declined to answer.
fn working_set() -> u64 {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak_working_set: usize,
        working_set: usize,
        peak_paged_pool: usize,
        paged_pool: usize,
        peak_nonpaged_pool: usize,
        nonpaged_pool: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    windows_core::link!("kernel32.dll" "system" fn GetCurrentProcess() -> *mut core::ffi::c_void);
    windows_core::link!("kernel32.dll" "system" fn K32GetProcessMemoryInfo(process: *mut core::ffi::c_void, counters: *mut core::ffi::c_void, size: u32) -> i32);
    let mut counters = Counters {
        cb: size_of::<Counters>() as u32,
        ..Counters::default()
    };
    // SAFETY: `counters` is a stack local of the layout the call declares, and its size is
    // stated in the same call.
    let ok = unsafe {
        K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&raw mut counters).cast(),
            size_of::<Counters>() as u32,
        )
    };
    if ok == 0 { 0 } else { counters.working_set as u64 }
}

/// Posts one message to the window under test.
fn post(hwnd: usize, message: u32, wparam: usize) {
    // SAFETY: the handle names the window this process created and has not destroyed, and
    // every message posted here carries no pointer in either parameter.
    unsafe {
        PostMessageW(hwnd as _, message, wparam, 0);
    }
}

/// Asks for one input pass and waits for the box it publishes.
///
/// The focused field's box is written by a pass, and a window with nothing to do runs none: a
/// compositor animation moves the content without waking anyone. A reading taken after one has
/// to ask for the pass that publishes where it ended up.
#[cfg(feature = "test-support")]
fn field_box(gauge: &Gauge, hwnd: usize) -> Option<(f32, f32)> {
    let before = gauge.read().ticks;
    post(hwnd, windows_window::WM_FRAME, 0);
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline && gauge.read().ticks == before {
        std::thread::yield_now();
    }
    gauge.read().focused_field
}

/// Waits until `settled` reads the same twice over `quiet`, or `limit` has passed.
fn settle(gauge: &Gauge, quiet: Duration, limit: Duration) {
    let deadline = Instant::now() + limit;
    let mut last = gauge.read();
    while Instant::now() < deadline {
        std::thread::sleep(quiet);
        let now = gauge.read();
        if now.ticks == last.ticks && now.scene_applies == last.scene_applies {
            return;
        }
        last = now;
    }
}

/// Types one character and returns how long it took to reach an applied scene, or `None` where
/// it did not inside `limit`.
///
/// Both ends are timed by the thread that owns them: the post here, and the apply on the scene
/// thread, which carries its own reading down with its tallies. Waiting for that reading to
/// arrive is not part of the interval. The compositor's path from the apply to the display is
/// not in it either; nothing on this side can observe it.
fn keystroke(hwnd: usize, gauge: &Gauge, unit: u16, limit: Duration) -> Option<Duration> {
    let before = gauge.read();
    let at = Instant::now();
    let posted = gauge.at(at);
    post(hwnd, 0x102, unit as usize);
    let deadline = at + limit;
    while Instant::now() < deadline {
        let now = gauge.read();
        if now.field_shapes > before.field_shapes
            && now.scene_applies > before.scene_applies
            && now.scene_applied_ns > posted
        {
            return Some(Duration::from_nanos(now.scene_applied_ns - posted));
        }
        std::thread::yield_now();
    }
    None
}

/// Returns the values at `fraction` through a sorted sample.
fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let at = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[at]
}

/// What a warmed edit is allowed to cost, so a regression fails the run rather than being read
/// off a table by somebody who remembers last month's number.
///
/// Each is an order of magnitude clear of what this host measures, because the figure a loaded
/// machine produces is not the figure an idle one does and a gate that trips on scheduling
/// noise gets switched off. What they catch is a change of shape: a lock taken on the input
/// path, a buffer that stopped being reused, a keystroke that reshapes every field.
mod budget {
    use std::time::Duration;

    /// The 95th percentile of an edit reaching an applied scene.
    pub const LATENCY_P95: Duration = Duration::from_millis(4);
    /// The worst one, at one 60 Hz frame.
    pub const LATENCY_MAX: Duration = Duration::from_millis(16);
    /// Allocations the input thread makes per keystroke.
    pub const INPUT_ALLOCATIONS: f64 = 16.0;
    /// Allocations the app thread makes per completed edit, DirectWrite's own excluded.
    pub const APP_ALLOCATIONS: f64 = 24.0;
    /// Allocations and working-set growth over 200 mount, focus and unmount cycles.
    pub const CYCLE_ALLOCATIONS: u64 = 400;
    pub const CYCLE_WORKING_SET_KIB: u64 = 512;
}

/// Prints a latency sample as its median, its 95th and its worst, and reports whether it is
/// inside its budget.
fn report_latency(what: &str, mut sample: Vec<Duration>) -> bool {
    sample.sort_unstable();
    let (p95, max) = (percentile(&sample, 0.95), percentile(&sample, 1.0));
    let held = !sample.is_empty() && p95 <= budget::LATENCY_P95 && max <= budget::LATENCY_MAX;
    println!(
        "  {what:<28} n={:<4} p50 {:>7.2} ms  p95 {:>7.2} ms  max {:>7.2} ms  {}",
        sample.len(),
        percentile(&sample, 0.5).as_secs_f64() * 1e3,
        p95.as_secs_f64() * 1e3,
        max.as_secs_f64() * 1e3,
        verdict(held),
    );
    held
}

/// Returns how a figure inside or outside its budget is named in the report.
fn verdict(held: bool) -> &'static str {
    if held { "ok" } else { "OVER BUDGET" }
}

/// Reports what each thread did over a settled interval, and whether it did nothing.
fn report_idle(what: &str, before: Reading, after: Reading) -> bool {
    let (ticks, wakes) = (
        after.ticks - before.ticks,
        after.scene_wakes - before.scene_wakes,
    );
    let (applies, flushes) = (
        after.scene_applies - before.scene_applies,
        after.app_flushes - before.app_flushes,
    );
    let quiet = ticks == 0 && wakes == 0 && applies == 0 && flushes == 0;
    println!(
        "  {what:<28} input {ticks}  scene wakes {wakes}  scene applies {applies}  app flushes \
         {flushes}  region frames {}",
        after.region_frames - before.region_frames
    );
    quiet
}

/// Pumps the window's own queue for at most `limit`, the way a text service's message loop
/// does, and returns how many keyboard messages it carried.
///
/// Reached only from the nested-pump gate, which the framework's own test seams open.
///
/// Every message is translated and dispatched, so a key it removes takes the whole production
/// path: the pretranslation hook, whatever TSF makes of it, and the doorbell.
#[cfg(feature = "test-support")]
fn pump(limit: Duration) -> u32 {
    let deadline = Instant::now() + limit;
    let mut keys = 0;
    let mut message = Msg::default();
    while Instant::now() < deadline {
        // SAFETY: `message` is a stack local of the layout the call writes, and the range and
        // removal flag are the ones a modal loop uses.
        let taken = unsafe { PeekMessageW(&raw mut message, core::ptr::null_mut(), 0, 0, 1) };
        if taken == 0 {
            std::thread::yield_now();
            continue;
        }
        if matches!(message.message, 0x100 | 0x101 | 0x102 | 0x104 | 0x105) {
            keys += 1;
        }
        // SAFETY: `message` was written by the call above and is live for both of these.
        unsafe {
            TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }
    keys
}

/// Types `text` while a nested pump runs inside every input pass, and returns what the field
/// committed.
///
/// The pump runs at the point a text service opens one — inside the pass, with the pass on the
/// stack — so this is the ordered boundary under load rather than an argument about it. Every
/// character must arrive exactly once and in order.
#[cfg(feature = "test-support")]
fn nested_pump_gate(hwnd: usize, gauge: &Gauge, pumping: &Arc<AtomicU64>, commits: &Commits) -> bool {
    let text = "abcdefghijkl";
    commits.lock().unwrap().clear();
    pumping.store(1, Ordering::Release);
    for unit in text.encode_utf16() {
        post(hwnd, 0x102, unit as usize);
    }
    settle(gauge, Duration::from_millis(50), Duration::from_secs(5));
    pumping.store(0, Ordering::Release);
    settle(gauge, Duration::from_millis(50), Duration::from_secs(3));

    // Each completed edit names the whole field, so the last one carries every character that
    // landed. A key delivered twice shows as a doubled letter; one lost shows as a gap.
    let held = commits.lock().unwrap();
    let last = held.last().cloned().unwrap_or_default();
    let landed = last.ends_with(text);
    println!(
        "  {:<28} {} of {} characters, {} edits, {}",
        "keys through a nested pump",
        last.chars().count().min(text.len()),
        text.len(),
        held.len(),
        if landed { "in order" } else { "OUT OF ORDER" }
    );
    landed && held.len() == text.len()
}

/// Reports a docked occlusion over the bottom of the window and returns whether the focused
/// field was brought clear of it and put back afterwards.
///
/// The field under test is the last thing in a scroller, so it is already at the end of the
/// content once focus reveals it. Nothing is left to scroll: clearing the occlusion is only
/// possible if the container holds extent past the range its content has, and giving that
/// extent back is what puts the field where it was.
#[cfg(feature = "test-support")]
fn occlusion_gate(hwnd: usize, gauge: &Gauge, client_h: f32) -> bool {
    let Some(occluder) = windows_ui::driver::testing::occluder() else {
        println!("  {:<28} no window to occlude", "docked occlusion");
        return false;
    };
    // The field under test is the last one the tab order reaches: it sits at the end of a
    // scroller's content, so focusing it scrolls the container to its own limit and leaves
    // nothing to scroll. Tab does not wrap, so walking past the end of the ring lands there.
    for _ in 0..24 {
        post(hwnd, 0x100, 9);
        settle(gauge, Duration::from_millis(20), Duration::from_millis(500));
    }
    // The reveal that focus asked for is a compositor animation; the box is read once it has
    // arrived rather than while it is moving.
    std::thread::sleep(Duration::from_millis(1200));
    let Some((_, settled)) = field_box(gauge, hwnd) else {
        println!("  {:<28} no field holds focus", "docked occlusion");
        return false;
    };

    let occlusion = windows_ui::layout::Rect {
        x0: 0.0,
        y0: client_h * 0.55,
        x1: 4096.0,
        y1: client_h,
    };
    occluder.report(Some(occlusion));
    settle(gauge, Duration::from_millis(50), Duration::from_secs(3));
    std::thread::sleep(Duration::from_millis(1200));
    let occluded = field_box(gauge, hwnd).map_or(settled, |(_, y1)| y1);

    occluder.report(None);
    settle(gauge, Duration::from_millis(50), Duration::from_secs(3));
    std::thread::sleep(Duration::from_millis(1200));
    let restored = field_box(gauge, hwnd).map_or(occluded, |(_, y1)| y1);

    let lifted = settled - occluded;
    let returned = (restored - settled).abs();
    println!(
        "  {:<28} field bottom {settled:.0} px, occluded {occluded:.0}, restored {restored:.0}",
        "docked occlusion"
    );
    println!(
        "  {:<28} lifted {lifted:.0} px clear of it, back within {returned:.0} px",
        ""
    );
    lifted > 8.0 && returned < 8.0
}

/// The two gates that drive a path from inside the framework need the seams that open it, so
/// a build without them says which build to make rather than reporting a figure it did not
/// measure.
#[cfg(not(feature = "test-support"))]
fn nested_pump_gate(_: usize, _: &Gauge, _: &Arc<AtomicU64>, _: &Commits) -> bool {
    println!("  {:<28} needs --features test-support", "keys through a nested pump");
    false
}

#[cfg(not(feature = "test-support"))]
fn occlusion_gate(_: usize, _: &Gauge, _: f32) -> bool {
    println!("  {:<28} needs --features test-support", "docked occlusion");
    false
}

/// Installs the nested pump the re-entrancy gate runs inside every pass.
#[cfg(feature = "test-support")]
fn install_nested_pump(pumping: Arc<AtomicU64>) {
    windows_ui::driver::testing::on_call_out(move || {
        if pumping.load(Ordering::Acquire) != 0 {
            pump(Duration::from_millis(2));
        }
    });
}

#[cfg(not(feature = "test-support"))]
fn install_nested_pump(_: Arc<AtomicU64>) {}

/// Runs the acceptance measurements against the running window and closes it.
///
/// Every wait is bounded, so a gate that never settles reports what it saw rather than hanging
/// the run.
fn measure(
    hwnd: usize,
    gauge: &Arc<Gauge>,
    mounted: &Arc<AtomicU64>,
    pulse: &Arc<windows_present::Epoch>,
    pumping: &Arc<AtomicU64>,
    commits: &Commits,
) -> bool {
    let mut passed = true;
    // Tab into the first field, then type through it once so every buffer, every shaped run
    // and every mailbox is at the size a steady edit uses.
    post(hwnd, 0x100, 9);
    settle(gauge, Duration::from_millis(60), Duration::from_secs(3));
    for unit in "warm up the shaper".encode_utf16() {
        _ = keystroke(hwnd, gauge, unit, Duration::from_millis(500));
    }
    settle(gauge, Duration::from_millis(60), Duration::from_secs(3));

    println!("focused idle, before any edit:");
    settle(gauge, Duration::from_millis(100), Duration::from_secs(5));
    let before = gauge.read();
    std::thread::sleep(Duration::from_secs(2));
    passed &= report_idle("caret blinking", before, gauge.read());

    println!("latency, warmed:");
    let mut applied = Vec::new();
    let mut shapes_per_key = Vec::new();
    let warmed = gauge.read().input_allocations;
    let (warmed_app, warmed_commits) = (
        gauge.app_allocations.load(Ordering::Acquire),
        gauge.app_commits.load(Ordering::Acquire),
    );
    for unit in "0123456789abcdefghijklmnopqrstuvwxyz".encode_utf16() {
        let before = gauge.read();
        if let Some(elapsed) = keystroke(hwnd, gauge, unit, Duration::from_millis(500)) {
            applied.push(elapsed);
            shapes_per_key.push(gauge.read().field_shapes - before.field_shapes);
        }
    }
    let typing_allocations = gauge.read().input_allocations - warmed;
    let app_typing = gauge.app_allocations.load(Ordering::Acquire) - warmed_app;
    let app_commits = gauge.app_commits.load(Ordering::Acquire) - warmed_commits;
    passed &= report_latency("post to applied scene", applied);
    let keys = shapes_per_key.len().max(1) as u64;
    let reshaped: u64 = shapes_per_key.iter().sum();
    println!(
        "  {:<28} {:.2} per keystroke over {} keys  {}",
        "fields reshaped",
        reshaped as f64 / keys as f64,
        keys,
        verdict(reshaped <= keys),
    );
    let per_key = typing_allocations as f64 / keys as f64;
    println!(
        "  {:<28} {per_key:.2} per keystroke  {}",
        "input-thread allocations",
        verdict(per_key <= budget::INPUT_ALLOCATIONS),
    );
    passed &= per_key <= budget::INPUT_ALLOCATIONS;
    // DirectWrite allocates through COM on the process heap, so this figure is the framework's
    // own share of an edit and not the shaping engine's. The working set below is what carries
    // both. It is read where a completed edit is delivered, which is the app thread, so a burst
    // that delivered none has nothing to divide and says so.
    match app_commits {
        0 => println!(
            "  {:<28} not measured: no completed edit reached the app thread",
            "app-thread allocations"
        ),
        n => {
            let per_edit = app_typing as f64 / n as f64;
            println!(
                "  {:<28} {per_edit:.2} per completed edit over {n} edits, less DirectWrite's  {}",
                "app-thread allocations",
                verdict(per_edit <= budget::APP_ALLOCATIONS),
            );
            passed &= per_edit <= budget::APP_ALLOCATIONS;
        }
    }
    passed &= reshaped <= keys && app_commits > 0;

    println!("focused idle, after a burst of edits:");
    settle(gauge, Duration::from_millis(100), Duration::from_secs(5));
    std::thread::sleep(Duration::from_secs(2));
    let before = gauge.read();
    std::thread::sleep(Duration::from_secs(2));
    passed &= report_idle("caret blinking, no region", before, gauge.read());

    // The same interval with a region taking the display clock beside the field. The presents
    // are the present thread's; none of the other three owes a wake for them. The epoch is what
    // wakes that thread: the switch alone reaches a thread that is parked.
    gauge.region_running.store(1, Ordering::Release);
    pulse.bump();
    settle(gauge, Duration::from_millis(100), Duration::from_secs(5));
    std::thread::sleep(Duration::from_secs(2));
    let before = gauge.read();
    std::thread::sleep(Duration::from_secs(2));
    let after = gauge.read();
    let quiet = report_idle("region presenting", before, after);
    let presenting = after.region_frames > before.region_frames;
    if !presenting {
        println!("  region drew nothing; the idle reading above proves nothing");
    }
    passed &= quiet && presenting;
    gauge.region_running.store(0, Ordering::Release);
    pulse.bump();

    println!("re-entrancy:");
    passed &= nested_pump_gate(hwnd, gauge, pumping, commits);

    println!("touch occlusion:");
    passed &= occlusion_gate(hwnd, gauge, 600.0);

    println!("repeated focus and unmount:");
    settle(gauge, Duration::from_millis(100), Duration::from_secs(5));
    let (before, resident) = (gauge.read(), working_set());
    for round in 0..200u64 {
        mounted.store(round % 2, Ordering::Release);
        post(hwnd, 0x100, 9);
        std::thread::sleep(Duration::from_millis(4));
    }
    mounted.store(1, Ordering::Release);
    settle(gauge, Duration::from_millis(100), Duration::from_secs(5));
    let (after, grew) = (gauge.read(), working_set().saturating_sub(resident) / 1024);
    let allocated = after.input_allocations - before.input_allocations;
    let held = allocated <= budget::CYCLE_ALLOCATIONS && grew <= budget::CYCLE_WORKING_SET_KIB;
    println!(
        "  {:<28} {allocated} allocations, working set +{grew} KiB, 200 cycles  {}",
        "mount, focus, unmount",
        verdict(held),
    );
    passed &= held;

    post(hwnd, 0x10, 0);
    passed
}

fn main() -> Result<()> {
    let ui = UiRuntime::new(&REFERENCE, AccentId(0), Density::Comfortable);
    let commits: Commits = Arc::new(Mutex::new(Vec::<String>::new()));
    // Armed by the driving thread, run on this one: the pass makes its deepest call-out from
    // the window's own thread, which is where a text service's pump would open.
    let pumping = Arc::new(AtomicU64::new(0));
    install_nested_pump(pumping.clone());
    let gauge = Arc::new(Gauge::default());
    let mounted = Arc::new(AtomicU64::new(1));
    observe({
        let gauge = gauge.clone();
        let quiet = std::env::args().any(|a| a == "--gates");
        move |seen| {
            if !quiet && !seen.reports.is_empty() {
                println!("reports {:?}", seen.reports);
            }
            gauge.scene_wakes.store(seen.scene_wakes, Ordering::Release);
            gauge
                .scene_applies
                .store(seen.scene_applies, Ordering::Release);
            gauge.app_flushes.store(seen.app.flushes, Ordering::Release);
            gauge
                .field_shapes
                .store(seen.field_shapes, Ordering::Release);
            if let Some(at) = seen.scene_applied_at {
                gauge
                    .scene_applied_ns
                    .store(gauge.at(at), Ordering::Release);
            }
            gauge.focused_field.store(
                seen.focused_field
                    .map_or(0, |box_| pack(box_.y0, box_.y1)),
                Ordering::Release,
            );
            gauge.focused_field_x.store(
                seen.focused_field
                    .map_or(0, |box_| pack(box_.x0, box_.x1)),
                Ordering::Release,
            );
            gauge
                .input_allocations
                .store(allocations(), Ordering::Release);
            // Last, because it names the moment every other counter above belongs to.
            gauge.ticks.store(seen.ticks, Ordering::Release);
        }
    });
    let automatic = std::env::args().any(|a| a == "--drive" || a == "--gates");
    let gates = std::env::args().any(|a| a == "--gates");
    // Minted here rather than inside the tree builder, so the thread driving the window holds
    // the same epoch the region is parked on.
    let live = Live::new()?;
    let pulse = live.epoch.clone();
    let idle_ok = Arc::new(AtomicU64::new(0));
    let window = Window::new("windows-ui — fields")
        .size_dips(560.0, 600.0)
        .pointer_input()
        .quit_on_close(true)
        .on_message({
            let gauge = gauge.clone();
            let idle_ok = idle_ok.clone();
            let mounted = mounted.clone();
            let pulse = pulse.clone();
            let pumping = pumping.clone();
            let commits = commits.clone();
            let mut pending = automatic;
            move |hwnd, message, wparam, _| {
                if pending && message == 0x18 && wparam != 0 {
                    pending = false;
                    let hwnd = hwnd as usize;
                    let gauge = gauge.clone();
                    let idle_ok = idle_ok.clone();
                    let mounted = mounted.clone();
                    let pulse = pulse.clone();
                    let pumping = pumping.clone();
                    let commits = commits.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(2));
                        post(hwnd, 0x100, 9);
                        std::thread::sleep(Duration::from_millis(300));
                        for u in "a😀".encode_utf16() {
                            post(hwnd, 0x102, u as usize);
                        }
                        std::thread::sleep(Duration::from_secs(2));
                        let before = gauge.ticks.load(Ordering::Acquire);
                        std::thread::sleep(Duration::from_secs(2));
                        let after = gauge.ticks.load(Ordering::Acquire);
                        println!("focused idle input ticks: {}", after - before);
                        idle_ok.store(u64::from(after == before), Ordering::Release);
                        if gates {
                            idle_ok.store(
                                u64::from(
                                    after == before
                                        && measure(
                                            hwnd, &gauge, &mounted, &pulse, &pumping, &commits,
                                        ),
                                ),
                                Ordering::Release,
                            );
                            return;
                        }
                        post(hwnd, 0x10, 0);
                    });
                }
                None
            }
        });
    ui.run(
        window,
        || {
            let compositor = Compositor::new()?;
            let gpu = Gpu::for_window()?;
            Backends::new(
                compositor,
                &gpu,
                FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]),
            )
        },
        BackdropSpec {
            base: vec![
                (quant_stop(0.0), light(2.1, 0.004, ACCENT_HUE)),
                (quant_stop(1.0), light(2.1, 0.004, ACCENT_HUE)),
            ],
            glows: Vec::new(),
            // A flat base has no ramp to contour, so there is nothing to break up.
            dither: false,
        },
        {
            let commits_for_app = commits.clone();
            let gauge_for_app = gauge.clone();
            let mounted_for_app = mounted.clone();
            move |ui, _ctx| {
                let (gauge, mounted, commits) = (gauge_for_app, mounted_for_app, commits_for_app);
                let source = Cell::new(String::new());
                stack(ui, |ui| {
                    label(ui, "Text input proof");
                    field(
                        ui,
                        windows_ui::widget::TextSource::Dynamic(Box::new(move |out| {
                            source.with(|s| out.push_str(s))
                        })),
                    )
                    .name("Text")
                    .on_commit({
                        let (gauge, commits) = (gauge.clone(), commits.clone());
                        move |text| {
                            source.set(text.into());
                            probe(&gauge, &commits, text);
                        }
                    });
                    // Every field reports its completed edits, so the app-thread figure is
                    // measured on whichever one the tab order actually landed on.
                    field(ui, "12.5")
                        .scope(InputScope::Number)
                        .name("Number")
                        .on_commit({
                            let (gauge, commits) = (gauge.clone(), commits.clone());
                            move |text| probe(&gauge, &commits, text)
                        });
                    field(ui, "https://newapo.dev")
                        .scope(InputScope::Url)
                        .name("URL")
                        .on_commit({
                            let (gauge, commits) = (gauge.clone(), commits.clone());
                            move |text| probe(&gauge, &commits, text)
                        });
                    field(ui, "search")
                        .scope(InputScope::Search)
                        .name("Search")
                        .on_commit({
                            let (gauge, commits) = (gauge.clone(), commits.clone());
                            move |text| probe(&gauge, &commits, text)
                        });
                    field(ui, "secret")
                        .scope(InputScope::Password)
                        .name("Password")
                        .on_commit({
                            let (gauge, commits) = (gauge.clone(), commits.clone());
                            move |text| probe(&gauge, &commits, text)
                        });
                    button(ui, "Replace text from model")
                        .on_click(move || source.set("model replacement".into()));
                    // A region beside the fields, so focused idle can be measured with
                    // something taking the display clock rather than over a still window.
                    ui.region(Queue::Solo, &live, {
                        let gauge = gauge.clone();
                        move |_, _| Ok(Box::new(Pulse { gauge }))
                    })
                    .height(Len::times(Metric::RowH, 2.0))
                    .name("Pulse");
                    scroll(ui, |ui| {
                        stack(ui, |ui| {
                            label(ui, "Scroll to the field below");
                            layer(ui, |_| {}).height(Len::times(Metric::RowH, 12.0));
                            if mounted.load(Ordering::Acquire) != 0 {
                                field(ui, "scroll-contained input")
                                    .name("Scrolled field")
                                    .on_commit({
                                        let (gauge, commits) = (gauge.clone(), commits.clone());
                                        move |text| probe(&gauge, &commits, text)
                                    });
                            }
                        });
                    })
                    .grow();
                })
                .gap(Metric::SpaceSm)
                .padding(Metric::SpaceMd)
                .grow();
            }
        },
    )?;
    if gates {
        assert_eq!(
            idle_ok.load(Ordering::Acquire),
            1,
            "every gate above must hold"
        );
        return Ok(());
    }
    println!("commits: {:?}", commits.lock().unwrap());
    if automatic {
        assert_eq!(*commits.lock().unwrap(), ["a", "a😀"]);
        assert_eq!(
            idle_ok.load(Ordering::Acquire),
            1,
            "focused idle must settle"
        );
    }
    Ok(())
}
windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd: *mut core::ffi::c_void, message: u32, w: usize, l: isize) -> i32);
windows_core::link!("user32.dll" "system" fn PeekMessageW(message: *mut Msg, hwnd: *mut core::ffi::c_void, first: u32, last: u32, remove: u32) -> i32);
windows_core::link!("user32.dll" "system" fn TranslateMessage(message: *const Msg) -> i32);
windows_core::link!("user32.dll" "system" fn DispatchMessageW(message: *const Msg) -> isize);

/// The queued message a nested pump takes, in the layout the system writes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Msg {
    hwnd: *mut core::ffi::c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    x: i32,
    y: i32,
    private: u32,
}

/// The completed edits the fields reported, in order.
type Commits = Arc<Mutex<Vec<String>>>;

struct Reference;
static REFERENCE: Reference = Reference;

const SURFACE_NITS: [f32; 4] = [2.1, 3.7, 6.4, 11.1];
const TEXT_NITS: [f32; 4] = [30.0, 96.0, 160.0, 244.0];
const ACCENT_HUE: f32 = 250.0;

fn light(nits: f32, chroma: f32, hue: f32) -> Radiance {
    Ictcp::polar(nits, chroma, hue).to_radiance(1.0)
}

impl Palette for Reference {
    fn text(&self, role: Text, scope: Scope) -> Radiance {
        let rung = |i: usize| match scope.polarity {
            Polarity::Dark => TEXT_NITS[i],
            Polarity::Light => TEXT_NITS[TEXT_NITS.len() - 1 - i],
        };
        match role {
            Text::Disabled => light(rung(0), 0.0, 0.0),
            Text::Tertiary => light(rung(1), 0.0, 0.0),
            Text::Secondary => light(rung(2), 0.0, 0.0),
            Text::Primary | Text::OnAccent => light(rung(3), 0.0, 0.0),
            Text::Accent => light(107.0, 0.06, ACCENT_HUE),
        }
    }

    fn fill(&self, role: Fill, scope: Scope) -> Radiance {
        let base = SURFACE_NITS[scope.elevation as usize];
        match role {
            Fill::Surface => light(base, 0.004, ACCENT_HUE),
            Fill::Hover => light(base * 1.18, 0.004, ACCENT_HUE),
            Fill::Pressed => light(base * 0.86, 0.004, ACCENT_HUE),
            Fill::Selected => light(base * 1.32, 0.010, ACCENT_HUE),
            Fill::Accent => light(72.0, 0.09, ACCENT_HUE),
            Fill::AccentSubtle => light(base * 1.6, 0.03, ACCENT_HUE),
        }
    }

    fn stroke(&self, role: Stroke, scope: Scope) -> Radiance {
        let base = SURFACE_NITS[scope.elevation as usize];
        match role {
            Stroke::Subtle => light(base * 1.5, 0.002, ACCENT_HUE),
            Stroke::Default => light(base * 2.4, 0.002, ACCENT_HUE),
            Stroke::Focus => light(107.0, 0.08, ACCENT_HUE),
            Stroke::Accent => light(72.0, 0.09, ACCENT_HUE),
        }
    }

    fn data(&self, role: DataRole) -> Radiance {
        light(84.0, 0.12, f32::from(role.0) * 31.0 % 360.0)
    }

    /// No light at any rung. A reference palette states an appearance and nothing about
    /// what emits — a glow is the application's claim about its own data.
    fn emission(&self, _role: Role, _scope: Scope) -> Emission {
        Emission::NONE
    }

    fn shadow(&self, _scope: Scope) -> Shadow {
        Shadow {
            sigma: 18.0,
            offset: 14.0,
            light: Radiance::new(0.0, 0.0, 0.0, 0.45),
        }
    }

    fn typography(&self, role: TypeRole, scope: Scope) -> FontSpec {
        let size = match role {
            TypeRole::Custom(token) => return token.resolve(scope),
            TypeRole::Display => 32.0,
            TypeRole::Title => 20.0,
            TypeRole::Body | TypeRole::BodyStrong | TypeRole::Mono => 14.0,
            TypeRole::Caption | TypeRole::Label => 12.0,
            TypeRole::Micro => 10.0,
        };
        let size = match scope.density {
            Density::Comfortable => size,
            Density::Compact => size - 1.0,
        };
        let weight = if matches!(role, TypeRole::Title | TypeRole::BodyStrong) {
            600
        } else {
            400
        };
        FontSpec::new(FamilyId(u16::from(role == TypeRole::Mono)), size).weight(weight)
    }

    fn metric(&self, metric: Metric, scope: Scope) -> f32 {
        let tight = match (scope.density, scope.width) {
            (Density::Compact, WidthClass::Narrow) => 0.75,
            (Density::Compact, _) | (_, WidthClass::Narrow) => 0.875,
            _ => 1.0,
        };
        match metric {
            Metric::Custom(token) => token.resolve(scope),
            Metric::Space3xs => 1.0,
            Metric::Space2xs => 2.0,
            Metric::SpaceXs => 4.0 * tight,
            Metric::SpaceXsSm => 6.0 * tight,
            Metric::SpaceSm => 8.0 * tight,
            Metric::SpaceSmMd => 10.0 * tight,
            Metric::SpaceMd => 12.0 * tight,
            Metric::SpaceMdLg => 16.0 * tight,
            Metric::SpaceLg => 20.0 * tight,
            Metric::SpaceXl => 28.0 * tight,
            Metric::Space2xl => 40.0 * tight,
            Metric::Radius | Metric::RadiusSurface | Metric::RadiusPill => 8.0,
            Metric::RowH => (32.0 * tight).max(24.0),
            Metric::TrackH => 20.0 * tight,
            Metric::BorderW => 1.0,
            Metric::HairlineW => 0.5,
            Metric::CardMinW => 240.0,
            Metric::CardMinH => 160.0,
            Metric::SliderRailH => 5.0 * tight,
            Metric::SliderThumb => 13.0 * tight,
        }
    }

    fn content_peak_nits(&self, _gamut: &windows_color::Gamut, _scope: Scope) -> f32 {
        290.0
    }
}
