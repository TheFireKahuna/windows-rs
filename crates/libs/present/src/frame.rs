//! The [`Frame`] trait a consumer implements, and the plain-data objects the front thread
//! and the present thread share: a change counter, the pickable parts a region publishes,
//! and the pointer state the front thread decides.

use super::*;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::os::windows::io::{AsHandle, BorrowedHandle};
use std::sync::Mutex;

/// Signals that something changed, carrying a monotonic version and a wake.
///
/// No API in this crate takes a data parameter: there is no source trait and no union of
/// what consumers read. A renderer closes over its own reader and registers an `Epoch` as
/// the wake source for it, so a consumer with a new data need adds nothing to this crate.
///
/// The event is a wake and never the truth. It is auto-reset, so a signal is consumed by
/// whichever wait observes it — including a wait the present loop entered for the compositor
/// clock — while [`seq`](Self::seq) is monotonic and is what a [`Frame::should_draw`]
/// compares against. A manual-reset event would satisfy every subsequent wait immediately
/// and turn the present thread into a busy loop.
pub struct Epoch {
    seq: AtomicU64,
    urgent: AtomicU64,
    event: Event,
}

impl Epoch {
    /// Creates an epoch at version 0, behind an auto-reset event.
    ///
    /// # Errors
    ///
    /// Fails when the event cannot be created.
    pub fn new() -> Result<Self> {
        Ok(Self {
            seq: AtomicU64::new(0),
            urgent: AtomicU64::new(0),
            event: Event::auto_reset()?,
        })
    }

    /// Records a change and wakes whoever is parked on it. Callable from any thread.
    pub fn bump(&self) {
        // release: pairs with the acquire in `seq`, so a reader that observes the new
        // version also observes whatever the caller wrote before bumping.
        self.seq.fetch_add(1, Ordering::Release);
        self.event.signal();
    }

    /// Invalidates future scheduled frames after an edit or structural change.
    pub fn invalidate(&self) {
        // Release: the pump's acquire observes the input that invalidated its queue.
        self.urgent.fetch_add(1, Ordering::Release);
        self.bump();
    }

    pub(crate) fn urgent(&self) -> u64 {
        // Acquire: pairs with invalidation before queued frames are replaced.
        self.urgent.load(Ordering::Acquire)
    }

    /// Returns the current version. No kernel call, so this is the gate a renderer compares
    /// against every tick.
    #[must_use]
    pub fn seq(&self) -> u64 {
        // acquire: pairs with the release in `bump`.
        self.seq.load(Ordering::Acquire)
    }
}

impl AsHandle for Epoch {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.event.as_handle()
    }
}

/// Identifies a pickable part inside a region, minted by the renderer that drew it.
///
/// `u32::MAX` is reserved as the sentinel for "no part", so an `Option<SubId>` fits an
/// `AtomicU32` and the front thread publishes hover with one store.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubId(pub u32);

const NO_SUB: u32 = u32::MAX;

/// Bounds one pickable piece of a region's pixels, in region-local DIPs.
///
/// Geometry only. A part's accessible name is declared where the region is declared and
/// keyed by [`SubId`] there, so it is not republished when the renderer's mapping moves and
/// no accessibility type appears in this crate.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Part {
    /// The renderer's identity for this piece.
    pub id: SubId,
    /// The piece's box, in region-local DIPs.
    pub rect: Rect,
}

/// Holds what is pickable inside a region: published by the renderer whenever its mapping
/// changes — a range change, a band added, a resize — and read by the front thread's hit
/// test once the region's own hit entry wins.
///
/// The region is a single hit entry and declares its gestures like any other control, so a
/// part names only which piece of it a contact landed on.
///
/// The reader keeps its own copy, so the hot path is the version alone: a front thread that
/// has already seen this version does one acquire load and nothing else, and takes the lock
/// only on the rare pass where the mapping moved.
#[derive(Default)]
pub struct RegionParts {
    version: AtomicU64,
    state: Mutex<PartsState>,
}

#[derive(Default)]
struct PartsState {
    parts: Vec<Part>,
    wake: Option<windows_window::Wake>,
    pending: Option<windows_window::Tick>,
}

impl RegionParts {
    /// Creates an empty set at version 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the published set. Called on the present thread, when the renderer's
    /// mapping moves.
    pub fn publish(&self, parts: &[Part]) {
        let mut held = self.state.lock().unwrap_or_else(|p| p.into_inner());
        held.parts.clear();
        held.parts.extend_from_slice(parts);
        // release: pairs with the acquire in `version`, so the new set is in place before
        // the version that advertises it becomes visible.
        self.version.fetch_add(1, Ordering::Release);
        if held.pending.is_none() {
            held.pending = held.wake.as_ref().map(windows_window::Wake::tick);
        }
    }

    /// Requests an input refresh when geometry changes while a pointer is over the region.
    ///
    /// One input owner may watch a region. It must pass `None` when the pointer leaves or
    /// the region retires, and acknowledge each refreshed version with [`Self::refreshed`].
    pub fn watch(&self, wake: Option<windows_window::Wake>) {
        let mut held = self.state.lock().unwrap_or_else(|p| p.into_inner());
        held.wake = wake;
        if held.wake.is_none() { held.pending = None; }
    }

    /// Releases the input request only if `version` still names the published geometry.
    pub fn refreshed(&self, version: u64) {
        let mut held = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if self.version() == version { held.pending = None; }
    }

    /// Returns the published version. Compare it against the version your copy holds, and
    /// read nothing further when it has not moved.
    #[must_use]
    pub fn version(&self) -> u64 {
        // acquire: pairs with the release in `publish`.
        self.version.load(Ordering::Acquire)
    }

    /// Refreshes `out` from the published set and returns the version it now holds.
    ///
    /// Reuses `out`'s allocation, so a front thread that keeps its copy allocates only
    /// when the part count grows.
    pub fn read_into(&self, out: &mut Vec<Part>) -> u64 {
        // Read before the copy: a publish that lands between the two leaves `out` holding
        // the newer set under the older version, so the next read refreshes it again.
        // Reversed, it would leave a stale copy under the current version and never
        // refresh.
        let version = self.version();
        if let Ok(held) = self.state.lock() {
            out.clear();
            out.extend_from_slice(&held.parts);
        }
        version
    }
}

/// Carries what the front thread has decided about a region, read by
/// [`Frame::should_draw`].
///
/// The front thread writes here and bumps the region's [`Epoch`] directly, without going
/// through the app thread, so the next present carries the new pixels one display frame
/// later and a busy app thread cannot stall a gesture. The app is told afterwards.
///
/// No input is routed to the present thread and no [`Frame`] method is callable from another
/// thread: the renderer publishes geometry and reads state, and is never asked a question
/// synchronously.
///
/// Each field is stored with `Release` and loaded with `Acquire`, pairing the front thread's
/// write with the present thread's next read of it.
#[derive(Debug)]
pub struct RegionInput {
    hover: AtomicU32,
    active: AtomicU32,
    /// Region-local, raw, packed as two `f32` bit patterns. Absence is the canonical
    /// quiet-NaN pair, which no cursor position can be.
    cursor: AtomicU64,
}

const NO_POINT: u64 = ((f32::NAN.to_bits() as u64) << 32) | f32::NAN.to_bits() as u64;

impl Default for RegionInput {
    fn default() -> Self {
        Self {
            hover: AtomicU32::new(NO_SUB),
            active: AtomicU32::new(NO_SUB),
            cursor: AtomicU64::new(NO_POINT),
        }
    }
}

/// Native bounds owned by one mounted region, independent of shared telemetry input.
#[derive(Default)]
pub struct RegionGeometry {
    geometry: Mutex<Option<(Extent, Extent, bool, Option<[f32; 2]>)>>,
}

impl RegionGeometry {
    /// Publishes native local bounds and whether their animation is running.
    /// The caller must invalidate the region's epoch when this returns true.
    pub fn set_geometry(&self, extent: Extent, target: Extent, moving: bool) -> bool {
        self.set_bounds(extent, target, moving, None)
    }

    /// Publishes a rectangle whose placement must be submitted with its pixels.
    /// `origin` is in the scene's unscaled layout coordinates.
    pub fn set_bounds(&self, extent: Extent, target: Extent, moving: bool, origin: Option<[f32;2]>) -> bool {
        if !extent.w.is_finite() || !extent.h.is_finite() || !extent.dpi.is_finite()
            || extent.w < 0.0 || extent.h < 0.0 || extent.dpi <= 0.0
            || !target.w.is_finite() || !target.h.is_finite() || target.w < 0.0
            || target.h < 0.0 || target.dpi != extent.dpi
            || origin.is_some_and(|v| !v[0].is_finite() || !v[1].is_finite()) { return false; }
        let mut held = self.geometry.lock().unwrap_or_else(|e| e.into_inner());
        if *held == Some((extent, target, moving, origin)) { return false; }
        *held = Some((extent, target, moving, origin));
        true
    }

    pub(crate) fn geometry(&self) -> Option<(Extent, Extent, bool, Option<[f32;2]>)> {
        *self.geometry.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Rebases both observed and target dimensions onto a new output DPI.
    pub fn set_geometry_dpi(&self, dpi: f32) -> bool {
        if !dpi.is_finite() || dpi <= 0.0 { return false; }
        let mut held = self.geometry.lock().unwrap_or_else(|e| e.into_inner());
        let Some((extent, target, _, _)) = held.as_mut() else { return false; };
        if extent.dpi == dpi { return false; }
        extent.dpi = dpi;
        target.dpi = dpi;
        true
    }
}

impl RegionInput {
    /// Creates an input with no hover, no active part and no cursor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the part under the pointer, or `None` when the pointer is over none of them.
    #[must_use]
    pub fn hover(&self) -> Option<SubId> {
        sub(self.hover.load(Ordering::Acquire))
    }

    /// Returns the part under an in-flight gesture, or `None` when no gesture is running.
    #[must_use]
    pub fn active(&self) -> Option<SubId> {
        sub(self.active.load(Ordering::Acquire))
    }

    /// Returns the pointer position in region-local DIPs, or `None` when the pointer is
    /// outside the region.
    #[must_use]
    pub fn cursor(&self) -> Option<(f32, f32)> {
        let packed = self.cursor.load(Ordering::Acquire);
        let (x, y) = (
            f32::from_bits((packed >> 32) as u32),
            f32::from_bits(packed as u32),
        );
        (!x.is_nan() && !y.is_nan()).then_some((x, y))
    }

    /// Publishes the part under the pointer.
    pub fn set_hover(&self, id: Option<SubId>) {
        self.hover.store(raw_sub(id), Ordering::Release);
    }

    /// Publishes the part under an in-flight gesture.
    pub fn set_active(&self, id: Option<SubId>) {
        self.active.store(raw_sub(id), Ordering::Release);
    }

    /// Publishes the pointer position, in region-local DIPs. A NaN coordinate publishes as
    /// absence, which is the same value `None` publishes.
    pub fn set_cursor(&self, at: Option<(f32, f32)>) {
        let packed = match at {
            Some((x, y)) if !x.is_nan() && !y.is_nan() => {
                ((x.to_bits() as u64) << 32) | y.to_bits() as u64
            }
            _ => NO_POINT,
        };
        self.cursor.store(packed, Ordering::Release);
    }
}

fn sub(raw: u32) -> Option<SubId> {
    (raw != NO_SUB).then_some(SubId(raw))
}

fn raw_sub(id: Option<SubId>) -> u32 {
    match id {
        Some(SubId(v)) if v != NO_SUB => v,
        _ => NO_SUB,
    }
}

/// Carries the region's inputs and scheduled time.
/// Offscreen rasterization must use the pass supplied to [`Frame::prepare`].
#[derive(Copy, Clone)]
pub struct GateCtx<'a> {
    /// The region's DIP box, and the display it is solved for. Every number a renderer
    /// lays out against comes from here, so the buffer's allocation and the coordinates
    /// drawn into it cannot disagree.
    pub extent: Extent,
    /// A monotonic present-thread counter. Increments once per *pass*, not once per frame
    /// of a batch — it identifies the wake, which is what a version gate wants.
    pub tick: u64,
    /// Scheduled presentation time on the monotonic clock.
    pub at: std::time::Instant,
    /// Measured display interval.
    pub interval: std::time::Duration,
    /// The region's own device. Build every brush, geometry and text layout from it;
    /// resources from another `Gpu` do not bind here.
    pub device: &'a Gpu,
    /// The transform every colour drawn this frame passes through, the same value the
    /// retained path uses.
    ///
    /// A `Frame` holds `Radiance`, `windows-d2d` accepts only `Scrgb`, and
    /// `OutputTransform::apply` is the only conversion between them, so a colour reaches the
    /// target through the transform exactly once.
    pub out: OutputTransform,
    /// What the front thread has decided about this region since the last frame.
    pub input: &'a RegionInput,
}

/// Carries what a renderer is handed to draw one frame: the same numbers as [`GateCtx`],
/// without the device.
#[derive(Copy, Clone)]
pub struct DrawCtx<'a> {
    /// The region's DIP box, and the display it is solved for.
    pub extent: Extent,
    /// The pass counter this frame belongs to.
    pub tick: u64,
    /// Scheduled presentation time on the monotonic clock.
    pub at: std::time::Instant,
    /// Measured display interval.
    pub interval: std::time::Duration,
    /// The transform every colour drawn this frame passes through.
    pub out: OutputTransform,
    /// What the front thread has decided about this region since the last frame.
    pub input: &'a RegionInput,
}

impl<'a> GateCtx<'a> {
    /// Returns the draw context for this frame: everything but the device.
    #[must_use]
    pub fn draw_ctx(&self) -> DrawCtx<'a> {
        DrawCtx {
            extent: self.extent,
            tick: self.tick,
            at: self.at,
            interval: self.interval,
            out: self.out,
            input: self.input,
        }
    }
}

/// Answers a region's box in the units a renderer lays out in.
macro_rules! box_accessors {
    ($t:ident) => {
        impl $t<'_> {
            /// Returns the region's width in DIPs.
            #[must_use]
            pub fn w(&self) -> f32 {
                self.extent.w
            }

            /// Returns the region's height in DIPs.
            #[must_use]
            pub fn h(&self) -> f32 {
                self.extent.h
            }

            /// Returns the DIP-to-pixel factor, the scale a realization or a cached raster
            /// is keyed on.
            #[must_use]
            pub fn scale(&self) -> f32 {
                self.extent.scale()
            }
        }
    };
}

box_accessors!(GateCtx);
box_accessors!(DrawCtx);

/// Draws one region's content, once per frame.
///
/// Built on the present thread by the factory passed to
/// [`Presenter::mount`](crate::Presenter::mount), so an implementation may hold `!Send`
/// state — which it must, because every device resource it draws with belongs to that
/// thread.
pub trait Frame {
    /// Reports whether rendering follows every observed animated size.
    ///
    /// Returning `false` holds the previous render extent during native layout motion.
    /// Placement and source clipping still follow observed bounds without scaling pixels;
    /// content beyond those bounds is clipped, and newly exposed space must be cleared.
    /// Settlement and DPI changes adopt the observed extent and force a redraw.
    /// Read once at mount. Renderers must clear the full acquired target.
    fn follows_animated_extent(&self) -> bool { true }

    /// Advances live state and reports whether this frame would differ from the one already
    /// on screen. `false` skips the tick entirely: no draw and no present, so the compositor
    /// is not woken.
    ///
    /// Called once per pass whether or not [`draw`](Self::draw) follows, and not once per
    /// slot of a batch: an implementation both tests and commits its version stamp, so a
    /// per-slot call would consume the change on the batch's first frame and report "nothing
    /// moved" for the rest of it.
    ///
    /// Must not rasterize resources. Cached content is prepared through [`Self::prepare`].
    fn should_draw(&mut self, ctx: GateCtx<'_>) -> bool;

    /// Prepares a scheduled frame through the batch's open drawing bracket.
    /// Resource rasterization must use `pass`; it must not open another bracket.
    fn prepare(&mut self, _ctx: GateCtx<'_>, _pass: &mut Pass<'_>) -> Result<()> {
        Ok(())
    }

    /// Commits prepared resources after a successful flush, or invalidates them on failure.
    fn finish_batch(&mut self, success: bool) {
        if !success { self.device_reset(); }
    }

    /// Draws one frame.
    ///
    /// The target is bound in DIPs with the region's origin at `(0, 0)`, and its contents
    /// are undefined on entry, so an implementation must clear or cover the whole box.
    ///
    /// Called once per frame the batch will show, in the order they will be shown: `depth`
    /// times while [`animating`](Self::animating) reports `true`, and once otherwise, since
    /// a frame that does not move would show the same pixels at every slot. A batch draws
    /// exactly the frames that will appear, so an ease stepped once per call produces the
    /// same motion as a pass per refresh.
    ///
    /// Must not block. A later region's buffer acquisition can block with this pass's
    /// bracket already open, so time spent waiting here delays every region in the pass.
    fn draw(&mut self, ctx: DrawCtx<'_>, draw: &Draw<'_>);

    /// Returns `true` when every pixel of the region's box is covered opaquely.
    ///
    /// This decides the surface's alpha mode, the Direct2D target's alpha mode and whether
    /// the buffers are requested displayable — together, the difference between DWM drawing
    /// the region and the display controller scanning it out. An implementation whose first
    /// act is not a full-cover clear or fill must answer `false`: claiming opacity while
    /// leaving pixels through shows whatever the buffer happened to hold, and the region is
    /// composed anyway.
    ///
    /// A region reaches opacity by drawing the chrome inside its own box rather than leaving
    /// gaps for what sits underneath it.
    ///
    /// Read once, when the region is allocated, because it decides the allocation. Debug
    /// builds assert it has not changed since.
    fn opaque(&self) -> bool {
        false
    }

    /// Returns `true` while something this renderer draws is still moving, which keeps the
    /// present thread paced off the display clock rather than parked.
    ///
    /// Read after [`should_draw`](Self::should_draw) reports a change, and it decides how
    /// many slots of the batch [`draw`](Self::draw) fills: a renderer whose pixels depend on
    /// the slot's time must report `true` for as long as they do.
    fn animating(&self) -> bool {
        false
    }

    /// Receives region visibility transitions without discarding GPU resources.
    fn suspended(&mut self, _suspended: bool) {}

    /// Drops every cached device resource.
    ///
    /// Called after the region is rebuilt on a new device and before the next
    /// [`draw`](Self::draw); a resource built on the previous device does not bind.
    fn device_reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_input_round_trips() {
        let input = RegionInput::new();
        assert_eq!(input.hover(), None);
        assert_eq!(input.cursor(), None);

        input.set_hover(Some(SubId(7)));
        input.set_active(Some(SubId(0)));
        input.set_cursor(Some((12.5, -3.25)));
        assert_eq!(input.hover(), Some(SubId(7)));
        assert_eq!(input.active(), Some(SubId(0)));
        assert_eq!(input.cursor(), Some((12.5, -3.25)));

        // `u32::MAX` and a NaN coordinate are the absence sentinels, so neither round-trips
        // as `Some`.
        input.set_hover(Some(SubId(u32::MAX)));
        assert_eq!(input.hover(), None);
        input.set_cursor(Some((f32::NAN, 0.0)));
        assert_eq!(input.cursor(), None);

        input.set_hover(None);
        input.set_cursor(None);
        assert_eq!(input.hover(), None);
        assert_eq!(input.cursor(), None);
    }

    #[test]
    fn parts_refresh_only_when_the_version_moves() {
        let parts = RegionParts::new();
        let mut mine = Vec::new();
        let mut seen = parts.read_into(&mut mine);
        assert!(mine.is_empty());

        parts.publish(&[Part {
            id: SubId(1),
            rect: Rect::new(0.0, 0.0, 10.0, 4.0),
        }]);
        assert_ne!(parts.version(), seen);
        seen = parts.read_into(&mut mine);
        assert_eq!(mine.len(), 1);
        assert_eq!(parts.version(), seen);
    }

    #[test]
    fn watched_parts_coalesce_and_a_stale_ack_cannot_lose_a_publish() {
        let window = windows_window::Window::new("parts wake").create().unwrap();
        let pacer = window.pacer().unwrap();
        let wake = pacer.wake();
        let parts = RegionParts::new();
        let part = Part { id: SubId(1), rect: Rect::new(0.0, 0.0, 10.0, 4.0) };
        parts.publish(&[part]);
        assert_eq!(wake.requesters(), 0);
        parts.watch(Some(wake.clone()));
        parts.publish(&[part]);
        assert_eq!(wake.requesters(), 1);
        let mut mine = Vec::new();
        let read = parts.read_into(&mut mine);
        assert_eq!(wake.requesters(), 1, "another reader cannot acknowledge input's request");
        parts.publish(&[]);
        parts.refreshed(read);
        assert_eq!(wake.requesters(), 1, "the newer geometry must still wake input");
        let read = parts.read_into(&mut mine);
        assert!(mine.is_empty());
        parts.refreshed(read);
        assert_eq!(wake.requesters(), 0);
        for _ in 0..32 { parts.publish(&[part]); }
        assert_eq!(wake.requesters(), 1);
        parts.watch(None);
        assert_eq!(wake.requesters(), 0);
        parts.publish(&[]);
        assert_eq!(wake.requesters(), 0);
    }
}
