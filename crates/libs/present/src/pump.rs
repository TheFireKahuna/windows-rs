//! The present thread: one wake source, one reader, one core touched.
//!
//! Every region draws on this one thread. The draw work is under 0.2 ms per publish, while a
//! thread per surface would wake N threads and open N readers per publish, pulling N cores
//! out of deep idle.
//!
//! A steady producer at display rate posts nothing to the front thread: it presents, and the
//! compositor picks up the new buffer on its own. After the one call that binds the surface
//! handle the front thread is out of the loop, so an idle window costs zero front-thread
//! wakes while its regions run at display rate.

use super::*;
use std::os::windows::io::{AsHandle, AsRawHandle};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

/// Sets frames drawn, and buffers held, per region.
///
/// Both numbers are knees on hardware-shaped curves, so both stay tunable rather than
/// becoming constants.
#[derive(Copy, Clone, Debug)]
pub struct Tuning {
    /// Frames drawn per wake, each presented into its own scheduled slot.
    ///
    /// At one frame per wake, present and the Direct2D bracket were 87% of a 494 µs
    /// compositor frame before anything was drawn, and nearly all of that is fixed per pass
    /// rather than per frame, so it divides: 7.12% of a core at 1, 6.55% at 2, 5.00% at 3,
    /// 4.76% at 4, 4.69% at 5.
    ///
    /// The cost is freshness. Inside a batch the first frame is as current as a per-refresh
    /// pass and the last is `depth - 1` refreshes stale — 13.9 ms at 144 Hz for three, half
    /// that on average — which is what bounds the value rather than the CPU curve.
    pub depth: u32,
    /// Buffers beyond `depth`: one for what the display is showing, one of slack so the
    /// rotation never catches its own tail on a frame the queue was late to retire.
    ///
    /// Widening it is monotonically worse — 5.00% of a core at `+2`, 5.94% at `+4`, 6.13% at
    /// `+5` — so a stall under batching is not buffer starvation; a wider rotation cycles
    /// more cold memory per burst. Memory is not the constraint either way: 3 buffers to 5
    /// moved the process working set by 0.1 MB.
    pub slack: u32,
    /// Consecutive ticks with nothing to draw before the loop drops back to the zero-cost
    /// event park.
    ///
    /// At 144 Hz, 30 is about 200 ms of grace — long enough that a brief gap between
    /// publishes (a track change, a debounced edit) does not thrash park-to-paced, short
    /// enough that a stopped stream idles the thread promptly. It does not affect idle
    /// cost, which is zero wakes either way once the loop has parked; it only sets how
    /// long the tail is.
    pub quiet_ticks: u32,
    /// Whether the groups report per-present statistics.
    ///
    /// The system produces a record per present per enabled kind, and enabling them also
    /// forces the VSync interrupt on for every present — about 26 µs of CPU each — because a
    /// statistic describes a present the CPU was woken for. A run with this on therefore
    /// does not measure the cadence a run with it off has.
    pub statistics: bool,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            depth: 3,
            slack: 2,
            quiet_ticks: 30,
            statistics: false,
        }
    }
}

impl Tuning {
    /// Returns the buffer count a region allocates: `depth` plus `slack`.
    #[must_use]
    pub fn pool(&self) -> u32 {
        self.depth + self.slack
    }
}

/// Refresh interval assumed until the clock has been measured, in 100 ns units — 60 Hz.
///
/// Too long a tick spaces a batch's slots wider than the display and shows each frame twice;
/// too short bunches them and the extras are skipped. The measured average converges within
/// a handful of wakes from either side.
const DEFAULT_TICK: u64 = 166_667;
/// Bounds a measured wake interval must fall inside to be believed: 1000 Hz down to 24 Hz.
/// Outside them the interval is a stall or a preemption rather than the display's cadence,
/// and folding one into the average misplaces a batch's slots for several passes after.
const MIN_TICK: u64 = 10_000;
const MAX_TICK: u64 = 416_667;

/// Every wait in this loop is `INFINITE`, with no guard timeout, retry interval or backoff.
///
/// Each reason the compositor clock can stop raises an edge this thread already waits on:
/// every monitor off, the session disconnected, another app owning the screen, and the
/// active desktop not being ours are the four conditions `windows-window` registers for, and
/// a minimized or cloaked window is the other half of that signal. A lost manager and a
/// removed device raise events of their own.
///
/// A cause that raised no edge would stop frames rather than wedge the thread: the command
/// handle is in every wait array, so shutdown, mounting and resizing interrupt any wait.
const _: () = ();

/// Tells the front thread what to do with a region's buffer.
#[derive(Copy, Clone, Debug)]
pub enum Bound {
    /// Bind this composition surface handle as the sink's brush, sampled 1:1 at `px`.
    ///
    /// `isize` rather than a pointer so the message is `Send`; cast it back at the binding.
    /// The region owns the handle and closes it, so a binding must be released before the
    /// region is, which [`Released`](Self::Released) asks for.
    Surface { handle: isize, px: (u32, u32) },
    /// Release the binding. The handle behind it is about to close.
    Released,
    /// Release the binding: the region is gone because its renderer panicked rather than
    /// because anything asked it to.
    ///
    /// Distinct from [`Released`](Self::Released) because a region that stops presenting
    /// looks exactly like one whose data stopped arriving, and the two call for opposite
    /// responses. The region is unmounted either way, so a panicking renderer costs its own
    /// region and not the per-frame path.
    Failed,
}

type Build = Box<dyn FnOnce(&Gpu) -> Result<Box<dyn Frame>> + Send>;
type Binder = Box<dyn FnMut(RegionKey, Bound) + Send>;

enum Cmd {
    Mount {
        spec: RegionSpec,
        epoch: Arc<Epoch>,
        input: Arc<RegionInput>,
        build: Build,
    },
    Unmount(RegionKey),
    Resize(RegionKey, Extent),
    Display(OutputTransform),
    Quit,
}

/// Drives a running present thread.
///
/// Every method sends a message: the thread owns the device, the groups, the regions and
/// every [`Frame`], because all of them are thread-affine and none of them is `Send`. A
/// `Frame` is built on the present thread by a factory this hands over, which is what lets
/// it hold `!Send` state.
pub struct Presenter {
    /// Behind a lock so the handle is `Sync`, not because sending contends. Mounting,
    /// unmounting and resizing happen on structural events that both the app thread and the
    /// front thread raise, so both must be able to hold this handle.
    tx: Mutex<Sender<Cmd>>,
    wake: Arc<Event>,
    tally: Arc<Mutex<PresentTally>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Presenter {
    /// Starts the present thread and returns the handle that drives it.
    ///
    /// `on_bind` runs on the present thread and is how a surface handle reaches the front
    /// thread. Post from it rather than touching the scene: a composition object may only be
    /// used on the thread that owns the compositor.
    ///
    /// `visibility` is the window's watch, from [`Window::watch`](windows_window::Window).
    /// Without it the loop cannot learn that the window went off screen and keeps drawing
    /// and presenting frames nobody can see. `None` is for a headless producer, which has no
    /// window to be hidden.
    ///
    /// # Errors
    ///
    /// Fails when the wake event or the thread itself cannot be created.
    pub fn spawn(
        tuning: Tuning,
        out: OutputTransform,
        visibility: Option<Watch>,
        on_bind: Binder,
    ) -> Result<Self> {
        let (tx, rx) = channel();
        let wake = Arc::new(Event::auto_reset()?);
        let tally = Arc::new(Mutex::new(PresentTally::default()));
        let (thread_wake, thread_tally) = (wake.clone(), tally.clone());
        let thread = std::thread::Builder::new()
            .name("present".into())
            .spawn(move || {
                run(
                    tuning,
                    out,
                    on_bind,
                    &thread_wake,
                    &thread_tally,
                    visibility,
                    &rx,
                );
            })
            .map_err(|_| windows_core::Error::from_hresult(E_FAIL))?;
        Ok(Self {
            tx: Mutex::new(tx),
            wake,
            tally,
            thread: Some(thread),
        })
    }

    /// Returns what this thread's presents have done, as of the last pass.
    ///
    /// Zero unless [`Tuning::statistics`] is on, because nothing is reported otherwise.
    /// Republished once per pass rather than once per present, so reading it costs an
    /// uncontended lock on a path that runs at wake rate.
    #[must_use]
    pub fn tally(&self) -> PresentTally {
        self.tally.lock().map(|t| *t).unwrap_or_default()
    }

    /// Mounts a region at its solved box, and hands its surface handle to the binder once it
    /// exists.
    ///
    /// `build` runs on the present thread with that thread's `Gpu`, so the [`Frame`] it
    /// returns may hold device resources and anything else `!Send`.
    ///
    /// Region count is data-dependent — a list of twelve rows with four expanded mounts four
    /// surfaces — so mounting and unmounting follow the structure on screen and are ordinary
    /// operations rather than start-up.
    pub fn mount(
        &self,
        spec: RegionSpec,
        epoch: Arc<Epoch>,
        input: Arc<RegionInput>,
        build: impl FnOnce(&Gpu) -> Result<Box<dyn Frame>> + Send + 'static,
    ) {
        self.send(Cmd::Mount {
            spec,
            epoch,
            input,
            build: Box::new(build),
        });
    }

    /// Destroys a region and frees its buffers, after telling the binder to release.
    pub fn unmount(&self, key: RegionKey) {
        self.send(Cmd::Unmount(key));
    }

    /// Resizes a region in place. The surface handle survives, so the front thread's binding
    /// is untouched; unmounting and remounting instead would drop frames, reallocate buffers
    /// and re-issue a handle that is already bound.
    pub fn resize(&self, key: RegionKey, extent: Extent) {
        self.send(Cmd::Resize(key, extent));
    }

    /// Replaces the output transform every region draws through, after a display-capability
    /// change.
    pub fn set_output_transform(&self, out: OutputTransform) {
        self.send(Cmd::Display(out));
    }

    fn send(&self, cmd: Cmd) {
        if self.tx.lock().is_ok_and(|tx| tx.send(cmd).is_ok()) {
            self.wake.signal();
        }
    }
}

impl Drop for Presenter {
    fn drop(&mut self) {
        self.send(Cmd::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ── the thread ──────────────────────────────────────────────────────────────────────

/// Identifies the queue a group serves. `Solo` is keyed by the region, so asking for one
/// always yields a group of one.
#[derive(Copy, Clone, PartialEq, Eq)]
enum GroupKey {
    Solo(RegionKey),
    Shared(&'static str),
}

impl GroupKey {
    fn of(queue: Queue, key: RegionKey) -> Self {
        match queue {
            Queue::Solo => Self::Solo(key),
            Queue::Shared(name) => Self::Shared(name),
        }
    }
}

struct Mounted {
    spec: RegionSpec,
    group: GroupKey,
    epoch: Arc<Epoch>,
    input: Arc<RegionInput>,
    frame: Box<dyn Frame>,
    region: PresentationRegion,
    /// Read once, at allocation, because it decides the allocation.
    opaque: bool,
}

/// Runs the present thread until a [`Cmd::Quit`] arrives.
///
/// Initializes no COM apartment. Everything this thread builds is reached through a plain
/// export — `D3D11CreateDevice`, `D2D1CreateFactory`, `CreatePresentationFactory`,
/// `DCompositionCreateSurfaceHandle` — and none of them is activated, so nothing on the
/// presented path needs one.
fn run(
    tuning: Tuning,
    out: OutputTransform,
    on_bind: Binder,
    wake: &Event,
    tally: &Arc<Mutex<PresentTally>>,
    visibility: Option<Watch>,
    rx: &Receiver<Cmd>,
) {
    match Pump::new(tuning, out, on_bind, tally.clone(), visibility) {
        Ok(mut pump) => pump.drive(wake, rx),
        // Presentation support is unconditional on this stack's floor, so a failure here is
        // a machine with no graphics stack and there is no fallback to take. Every mounted
        // region never binds.
        Err(_) => drain_until_quit(rx, wake),
    }
}

/// Accepts and discards commands until [`Cmd::Quit`], so a caller's `Drop` still joins.
fn drain_until_quit(rx: &Receiver<Cmd>, wake: &Event) {
    loop {
        while let Ok(cmd) = rx.try_recv() {
            if matches!(cmd, Cmd::Quit) {
                return;
            }
        }
        wake.wait(INFINITE);
    }
}

struct Pump {
    device: PresentationDevice,
    groups: Vec<(GroupKey, PresentationGroup)>,
    mounted: Vec<Mounted>,
    on_bind: Binder,
    tuning: Tuning,
    out: OutputTransform,
    tally: PresentTally,
    /// The same numbers, republished once per pass for whoever holds the [`Presenter`].
    published: Arc<Mutex<PresentTally>>,
    /// Whether anything drawn can be seen. `None` when the caller attached no window: the
    /// loop then has no way to learn it is invisible and treats itself as visible.
    visibility: Option<Watch>,
    speed: Speed,
    /// Increments once per pass — it identifies the wake, which is what a version gate
    /// wants, rather than once per frame of a batch.
    tick_count: u64,
    // Reused across wakes so a pass allocates nothing.
    handles: Vec<HANDLE>,
    due: Vec<usize>,
    slots: Vec<u64>,
    poisoned: Vec<RegionKey>,
}

/// Runs one call into a renderer, and returns `None` if it panicked.
///
/// A [`Frame`] is application code on the thread that owns every region's clock, so an
/// unwind out of one would stop the whole per-frame path and freeze every other region.
/// Catching it costs the panicking renderer its own region and nothing else.
///
/// Unwind safety is asserted: the frame is destroyed immediately afterwards and its region
/// with it, so nothing the panic left half-updated is read again, and the pass's bracket
/// closes on its own `Drop`.
fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

impl Pump {
    fn new(
        tuning: Tuning,
        out: OutputTransform,
        on_bind: Binder,
        published: Arc<Mutex<PresentTally>>,
        visibility: Option<Watch>,
    ) -> Result<Self> {
        Ok(Self {
            device: PresentationDevice::new()?,
            groups: Vec::new(),
            mounted: Vec::new(),
            on_bind,
            tuning,
            out,
            tally: PresentTally::default(),
            published,
            visibility,
            speed: Speed::Full,
            tick_count: 0,
            handles: Vec::new(),
            due: Vec::new(),
            slots: Vec::new(),
            poisoned: Vec::new(),
        })
    }

    fn drive(&mut self, wake: &Event, rx: &Receiver<Cmd>) {
        // `true` while data flows or an ease runs: the loop then paces off the compositor
        // clock, one wake per display refresh. Otherwise it parks on the events alone and
        // an idle producer costs zero wakes.
        let (mut paced, mut quiet) = (false, 0u32);
        // Latched false only by `WAIT_FAILED`: this session has no compositor clock at all
        // (headless, or remote). A display that is merely off is transient and is handled
        // without latching.
        let mut clock_ok = true;
        // Latched by the clock's occluded return, cleared by one probe after an edge.
        let mut dark = false;
        let mut tick = DEFAULT_TICK;
        let mut last_wake = 0u64;
        // Wakes remaining to sit out because a batch already drew their frames.
        let mut skip = 0u32;

        self.rebuild_handles(wake);
        loop {
            // Nothing this thread draws can be seen: the window is off screen, or the
            // display cannot show it. Either way there is nothing to gate on and nothing
            // to present, so drop to EcoQoS and park until an edge says otherwise.
            if dark || self.visibility.as_ref().is_some_and(|v| v.is_hidden()) {
                self.qos(Speed::Eco);
                self.park();
                if self.commands(rx, wake) {
                    return;
                }
                // Only the clock reports whether the display is back, so leaving the parked
                // state is a single probe on the next iteration rather than state this
                // thread tracks itself.
                dark = false;
                continue;
            }
            self.qos(Speed::Full);
            match self.wait(paced, clock_ok) {
                Waken::Failed => {
                    clock_ok = false;
                    continue;
                }
                Waken::Occluded => {
                    dark = true;
                    continue;
                }
                Waken::Ready => {}
            }
            if self.commands(rx, wake) {
                return;
            }
            self.recover();

            // Measure the interval the display is actually running at, from the clock we
            // are already woken by: no second source, and it follows a mode change on its
            // own. Skipped wakes are excluded because their spacing is the batch's, not
            // the display's.
            let now = interrupt_time_now();
            if last_wake != 0 && skip == 0 {
                let delta = now.saturating_sub(last_wake);
                if (MIN_TICK..=MAX_TICK).contains(&delta) {
                    tick = (tick * 7 + delta) / 8;
                }
            }
            last_wake = now;

            // A previous batch already drew this wake's frame. The wake still happens; it
            // costs the gate and no draw, bracket or present.
            if skip > 0 {
                skip -= 1;
                continue;
            }

            let drew = self.pass(now, tick);
            if drew {
                skip = self.tuning.depth.saturating_sub(1);
            }

            // A redraw or a mid-flight ease keeps the loop on the display clock; enough
            // consecutive empty ticks fall back to the zero-cost park.
            if drew || self.mounted.iter().any(|m| m.frame.animating()) {
                paced = true;
                quiet = 0;
            } else if paced {
                quiet += 1;
                if quiet >= self.tuning.quiet_ticks {
                    paced = false;
                    quiet = 0;
                }
            }
        }
    }

    /// Runs one pass: gate every region, draw the whole batch inside one bracket, then bind
    /// and show. Returns `true` when anything was drawn.
    fn pass(&mut self, now: u64, tick: u64) -> bool {
        self.tick_count += 1;

        // Ahead of issuing this pass's presents: the queue is finite and retires its oldest
        // entries, so a producer that presents first and reads later reads a queue that has
        // already dropped the answer.
        let mut read = 0;
        for (_, group) in &self.groups {
            read += group.drain_statistics(&mut self.tally);
        }
        if read > 0
            && let Ok(mut published) = self.published.lock()
        {
            *published = self.tally;
        }

        // Which regions this pass is for, decided once. `should_draw` both tests and commits
        // its version stamp, so asking it per slot would consume the change on the batch's
        // first frame and report "nothing moved" for the rest.
        let Self {
            device,
            mounted,
            due,
            slots,
            poisoned,
            out,
            tick_count,
            ..
        } = self;
        due.clear();
        poisoned.clear();
        for (i, m) in mounted.iter_mut().enumerate() {
            let ctx = FrameCtx {
                extent: m.region.extent(),
                tick: *tick_count,
                device: device.gpu(),
                out: *out,
                input: &m.input,
            };
            match guarded(|| m.frame.should_draw(ctx)) {
                Some(true) => due.push(i),
                Some(false) => {}
                None => poisoned.push(m.spec.key),
            }
        }
        if due.is_empty() {
            self.retire_poisoned();
            return false;
        }

        // The frames this pass will draw, at the times they are meant to be shown: one per
        // refresh, starting at the one this wake is for.
        slots.clear();
        for k in 0..self.tuning.depth {
            slots.push(now + u64::from(k) * tick);
        }

        // ── draw ────────────────────────────────────────────────────────────────────
        // Every slot of every due region, before anything binds. The bracket opens here
        // and spans the whole batch; its cost is fixed per pair and independent of what
        // was drawn, so a batch of `depth` frames pays it once instead of `depth` times.
        let Ok(mut pass) = device.pass() else {
            return false;
        };
        for _ in 0..slots.len() {
            for &i in &*due {
                let m = &mut mounted[i];
                let Ok(Some(target)) = m.region.acquire() else {
                    continue;
                };
                debug_assert_eq!(
                    m.frame.opaque(),
                    m.opaque,
                    "a Frame's opacity decides its allocation and may not change after it"
                );
                let ctx = FrameCtx {
                    extent: m.region.extent(),
                    tick: *tick_count,
                    device: device.gpu(),
                    out: *out,
                    input: &m.input,
                };
                // Once per retarget rather than once per call: a latched error discards the
                // rest of the batch, and the tag names the region that latched it.
                pass.tag(m.spec.key.0);
                let draw = pass.draw(target);
                if guarded(|| m.frame.draw(ctx, &draw)).is_none() {
                    poisoned.push(m.spec.key);
                }
            }
        }
        let flushed = match Flushed::end(pass) {
            Ok(flushed) => flushed,
            Err(error) => {
                self.failed(error);
                self.retire_poisoned();
                return true;
            }
        };

        // ── bind and show ───────────────────────────────────────────────────────────
        // Slot outer, group inner. On a shared queue every member must bind its slot-k
        // buffer before any of them presents slot k, or the queue issues one present per
        // region instead of one per slot — n times the most expensive call in the process.
        // A solo queue is a group of one, so the same shape is correct for both and there
        // is no second ordering to pick between.
        let last = slots.len().saturating_sub(1);
        for (k, at) in slots.iter().enumerate() {
            for (key, group) in &self.groups {
                let mut bound = false;
                for &i in &*due {
                    if mounted[i].group == *key {
                        bound |= mounted[i].region.submit(&flushed).unwrap_or(false);
                    }
                }
                if !bound {
                    continue;
                }
                // Only the frame the next pass waits behind has to wake the CPU when it is
                // shown; nothing reads the earlier ones' buffer-available events or
                // statistics before that wake.
                let interrupt = if k == last {
                    Interrupt::Raise
                } else {
                    Interrupt::Defer
                };
                let _ = group.present_at(*at, interrupt);
            }
        }
        // After the presents, not before: a region that panicked mid-batch still has
        // earlier slots drawn and bound, and showing them is better than a frozen box.
        self.retire_poisoned();
        true
    }

    /// Takes every renderer that panicked this pass out of service.
    ///
    /// Reported as [`Bound::Failed`] rather than as an ordinary release: a region that stops
    /// presenting looks exactly like one whose data stopped arriving, and the two call for
    /// opposite responses.
    fn retire_poisoned(&mut self) {
        while let Some(key) = self.poisoned.pop() {
            self.unmount_as(key, Bound::Failed);
        }
    }

    /// Handles an error latched by the pass. A device that is gone is rebuilt whole;
    /// anything else was one region's bad draw, and the next pass opens a fresh bracket.
    fn failed(&mut self, error: PassError) {
        if error.loss == Loss::DeviceRemoved {
            self.rebuild_device();
        }
    }

    // ── commands ────────────────────────────────────────────────────────────────────

    /// Drains the command queue. Returns `true` when the thread must quit.
    fn commands(&mut self, rx: &Receiver<Cmd>, wake: &Event) -> bool {
        let mut moved = false;
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Cmd::Quit => {
                    self.unmount_all();
                    return true;
                }
                Cmd::Mount {
                    spec,
                    epoch,
                    input,
                    build,
                } => {
                    self.unmount(spec.key);
                    let _ = self.mount(spec, epoch, input, build);
                    moved = true;
                }
                Cmd::Unmount(key) => {
                    self.unmount(key);
                    moved = true;
                }
                Cmd::Resize(key, extent) => {
                    if let Some(m) = self.mounted.iter_mut().find(|m| m.spec.key == key) {
                        m.spec.extent = extent;
                        // In place: the surface handle survives, so the front thread's
                        // binding does not move and no other region in the group is
                        // disturbed. Only the pixel extent it samples changes.
                        if m.region.resize(extent).is_ok() {
                            let px = m.region.size_px();
                            let handle = m.region.surface_handle() as isize;
                            (self.on_bind)(key, Bound::Surface { handle, px });
                        }
                    }
                }
                Cmd::Display(out) => self.out = out,
            }
        }
        if moved {
            self.rebuild_handles(wake);
        }
        false
    }

    fn mount(
        &mut self,
        spec: RegionSpec,
        epoch: Arc<Epoch>,
        input: Arc<RegionInput>,
        build: Build,
    ) -> Result<()> {
        let frame = build(self.device.gpu())?;
        // Read once, here: it decides the surface's alpha mode, the Direct2D target's alpha
        // mode and the displayable allocation, all three together.
        let opaque = frame.opaque();
        let group = GroupKey::of(spec.queue, spec.key);
        let handle = self.group(group)?;
        let region = PresentationRegion::new(
            &self.device,
            handle,
            spec.extent,
            if opaque {
                Opacity::Opaque
            } else {
                Opacity::Translucent
            },
            spec.key,
            self.tuning.pool(),
        )?;
        let px = region.size_px();
        let raw = region.surface_handle() as isize;
        self.mounted.push(Mounted {
            spec,
            group,
            epoch,
            input,
            frame,
            region,
            opaque,
        });
        (self.on_bind)(spec.key, Bound::Surface { handle: raw, px });
        Ok(())
    }

    fn unmount(&mut self, key: RegionKey) {
        self.unmount_as(key, Bound::Released);
    }

    fn unmount_as(&mut self, key: RegionKey, reason: Bound) {
        let Some(at) = self.mounted.iter().position(|m| m.spec.key == key) else {
            return;
        };
        // Told before it goes: the compositor holds a reference to whatever a visual
        // paints with, so a brush over a handle this region is about to close has to leave
        // the tree first.
        (self.on_bind)(key, reason);
        let gone = self.mounted.remove(at);
        let group = gone.group;
        drop(gone);
        // A group whose last region left has nothing to present and nothing to report.
        if !self.mounted.iter().any(|m| m.group == group) {
            self.groups.retain(|(key, _)| *key != group);
        }
    }

    fn unmount_all(&mut self) {
        while let Some(m) = self.mounted.last() {
            self.unmount(m.spec.key);
        }
    }

    fn group(&mut self, key: GroupKey) -> Result<PresentationGroup> {
        if let Some((_, group)) = self.groups.iter().find(|(k, _)| *k == key) {
            return Ok(group.clone());
        }
        let group = self.device.create_group(self.tuning.statistics)?;
        self.groups.push((key, group.clone()));
        Ok(group)
    }

    // ── device loss ─────────────────────────────────────────────────────────────────

    /// Rebuilds every region whose group has lost its manager.
    ///
    /// A lost manager costs its group and the regions in it; a lost device costs every
    /// region. Neither costs the retained visual tree.
    fn recover(&mut self) {
        let lost: Vec<GroupKey> = self
            .groups
            .iter()
            .filter(|(_, group)| group.is_lost())
            .map(|(key, _)| *key)
            .collect();
        for key in lost {
            self.groups.retain(|(k, _)| *k != key);
            self.remake(|m| m.group == key);
        }
    }

    fn rebuild_device(&mut self) {
        let Ok(device) = PresentationDevice::new() else {
            return;
        };
        self.device = device;
        self.groups.clear();
        self.remake(|_| true);
    }

    /// Rebuilds every region matching `which` onto a freshly created group, and tells each
    /// frame to drop the device resources it cached.
    fn remake(&mut self, which: impl Fn(&Mounted) -> bool) {
        for i in 0..self.mounted.len() {
            if !which(&self.mounted[i]) {
                continue;
            }
            let (spec, group) = (self.mounted[i].spec, self.mounted[i].group);
            (self.on_bind)(spec.key, Bound::Released);
            let Ok(handle) = self.group(group) else {
                continue;
            };
            let opacity = if self.mounted[i].opaque {
                Opacity::Opaque
            } else {
                Opacity::Translucent
            };
            let region = PresentationRegion::new(
                &self.device,
                handle,
                spec.extent,
                opacity,
                spec.key,
                self.tuning.pool(),
            );
            let Ok(region) = region else { continue };
            let px = region.size_px();
            let raw = region.surface_handle() as isize;
            self.mounted[i].region = region;
            // Before its next draw, not after: the device those resources came from is
            // gone, and a brush built on it does not bind.
            self.mounted[i].frame.device_reset();
            (self.on_bind)(spec.key, Bound::Surface { handle: raw, px });
        }
    }

    // ── waiting ─────────────────────────────────────────────────────────────────────

    /// Rebuilds the wait array: handle 0 is the command event, then the visibility watch
    /// when a window is attached, then each distinct epoch. Called when the mounted set
    /// changes, never per pass.
    fn rebuild_handles(&mut self, wake: &Event) {
        // Raw, because a `BorrowedHandle`'s lifetime cannot be named in the field's type.
        // The owners outlive every entry: `wake` is the `Arc` held for this thread, and the
        // visibility and epoch `Arc`s are fields of this struct.
        self.handles.clear();
        self.handles.push(wake.as_handle().as_raw_handle());
        // Slot 1 when a window is attached: the edge that says the window came back on
        // screen, or that the system's occlusion status moved. It is in both the paced wait
        // and the park, because it is the only handle that can leave the parked state.
        if let Some(visibility) = &self.visibility {
            self.handles.push(visibility.as_handle().as_raw_handle());
        }
        for m in &self.mounted {
            let raw = m.epoch.as_handle().as_raw_handle();
            if !self.handles.contains(&raw) {
                self.handles.push(raw);
            }
        }
    }

    /// Waits once, either paced off the compositor clock or on this thread's own edges
    /// alone.
    ///
    /// `INFINITE` in both arms, with no guard timeout: nothing downstream of this thread
    /// drives itself from the tick, so a stalled clock has nothing to degrade, and every
    /// reason the loop should move again is already an edge in the wait array.
    fn wait(&self, paced: bool, clock_ok: bool) -> Waken {
        let count = self.handles.len() as u32;
        let ptr = self.handles.as_ptr();
        if paced && clock_ok {
            // SAFETY: every handle is a live kernel object owned by this pump, by the
            // window's watch, or by a mounted region's epoch, and the list is a field of the
            // stated length.
            return match unsafe { clock::wait_for_frame_raw(ptr, count, clock::INFINITE) } {
                Observed::Occluded => Waken::Occluded,
                Observed::NoClock => Waken::Failed,
                // A frame, one of this thread's own edges, or a stalled clock: all three mean
                // look at the mounted set, and what to draw comes from epoch stamps rather
                // than from which handle woke this.
                _ => Waken::Ready,
            };
        }
        // SAFETY: as above.
        match unsafe { WaitForMultipleObjects(count, ptr, false.into(), INFINITE) } {
            WAIT_FAILED => Waken::Failed,
            _ => Waken::Ready,
        }
    }

    /// Parks until something can be seen again.
    ///
    /// Waits on the command handle and the visibility watch, and not on the data epochs.
    /// Ignoring data that arrives while nothing is displayed loses no state:
    /// [`Frame::should_draw`] compares against a stored stamp, so any number of accumulated
    /// changes produce exactly one redraw on the way back.
    fn park(&self) {
        let count = if self.visibility.is_some() { 2 } else { 1 };
        // SAFETY: both handles are live kernel objects owned by this pump and by the
        // window's visibility; the list is a field at least `count` long.
        unsafe {
            WaitForMultipleObjects(count, self.handles.as_ptr(), false.into(), INFINITE);
        }
    }

    /// Sets this thread's scheduling class, and only when it changes.
    ///
    /// [`Speed::Eco`] rather than [`Speed::Managed`], which is what a window thread asks for
    /// when it goes quiet: Windows demotes a fully-occluded window-owning process on its
    /// own. This thread's reason for stopping — a display that is off, or a producer with
    /// nothing to publish — appears in no window state, so nothing infers it.
    fn qos(&mut self, speed: Speed) {
        if self.speed == speed {
            return;
        }
        self.speed = speed;
        qos::set(speed);
    }
}

/// Reports why the loop woke.
enum Waken {
    /// Run the pass. A clock tick, a data epoch, a command, or a stalled clock — the loop
    /// gates on [`Frame::should_draw`] either way and does not need them told apart.
    Ready,
    /// The display is off, and the call returned immediately.
    Occluded,
    /// This session has no compositor clock. Latched: the loop then waits on its own edges
    /// alone.
    Failed,
}

impl Drop for Pump {
    fn drop(&mut self) {
        self.unmount_all();
    }
}
