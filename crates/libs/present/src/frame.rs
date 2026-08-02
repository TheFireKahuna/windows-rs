//! The framework's entire knowledge of a consumer, and the two plain-data objects that
//! sit between the front thread and one.

use super::*;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::os::windows::io::{AsHandle, BorrowedHandle};
use std::sync::Mutex;

/// A payload-free "something changed": a version and a wake.
///
/// **There is no data parameter anywhere in this crate.** No `src: &dyn AnySource`, no
/// facet traits, no union of what consumers read — a renderer closes over its own reader
/// and data arrival is a wake source it registers. The prior stack's god-trait
/// (`VizSource: OverlaySource + MeterSource + DynamicsSource + CurveSource`) meant that
/// adding a consumer with a new data need required editing the framework, and it was
/// already leaking: two sibling facet traits existed outside the union. Adding a consumer
/// here is zero framework edits.
///
/// The event is a wake and **never the truth**. It is auto-reset, so a signal is consumed
/// by whichever wait happens to observe it — including a wait the present loop entered
/// for the compositor clock — while `seq` is monotonic and is what a
/// [`Frame::should_draw`] compares against. A manual-reset event would satisfy every
/// subsequent wait immediately and turn the present thread into a busy loop, which is the
/// difference between parking at idle and spending a core reporting that nothing
/// happened.
pub struct Epoch {
    seq: AtomicU64,
    event: Event,
}

impl Epoch {
    pub fn new() -> Result<Self> {
        Ok(Self {
            seq: AtomicU64::new(0),
            event: Event::auto_reset()?,
        })
    }

    /// Records a change and wakes whoever is parked on it. Callable from any thread.
    pub fn bump(&self) {
        self.seq.fetch_add(1, Ordering::Release);
        self.event.signal();
    }

    /// The current version — a cheap gate, with no kernel call in it.
    #[must_use]
    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }
}

impl AsHandle for Epoch {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.event.as_handle()
    }
}

/// A pickable part inside a region, identified by the renderer that drew it.
///
/// `u32::MAX` is reserved as the niche for "no part", so an `Option<SubId>` fits an
/// `AtomicU32` and the front thread can publish hover with one store.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubId(pub u32);

const NO_SUB: u32 = u32::MAX;

/// One pickable region of a region's pixels, in **region-local DIPs**.
///
/// Geometry only. What a part *means* to a screen reader is declared once where the
/// region is declared, and is keyed by [`SubId`] there — it is not republished with the
/// geometry, because the geometry moves whenever the renderer's mapping does and an
/// accessible name does not. That also keeps this crate free of any accessibility type,
/// which lives a layer up.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Part {
    pub id: SubId,
    pub rect: Rect,
}

/// What is pickable inside a region: published by the renderer whenever its **mapping**
/// changes — a range change, a band added, a resize — and read by the front thread's hit
/// test after the region's own entry wins.
///
/// The region is one hit entry and declares its gestures like any control, so the
/// recogniser pool, capture, cancel and inertia are unchanged; what is new is only *which
/// part* the contact landed on.
///
/// **The reader owns the second buffer, which is why it never blocks.** A publish is rare
/// and a hover read is frequent, so the version is the only thing on the hot path: a
/// front thread that has already seen this version does nothing at all — one acquire load
/// — and takes the lock solely to refresh its own copy on the rare pass where the mapping
/// actually moved. Double-buffering it the other way round, with the reader copying out
/// of a slot the writer is cycling, buys the same thing and costs a cutover protocol to
/// get wrong.
#[derive(Default)]
pub struct RegionParts {
    version: AtomicU64,
    parts: Mutex<Vec<Part>>,
}

impl RegionParts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the published set. Present thread; rare.
    pub fn publish(&self, parts: &[Part]) {
        if let Ok(mut held) = self.parts.lock() {
            held.clear();
            held.extend_from_slice(parts);
        }
        self.version.fetch_add(1, Ordering::Release);
    }

    /// The published version. Front thread, per hover — compare it against the one whose
    /// copy you hold, and read nothing further when it has not moved.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// Refreshes `out` from the published set, answering the version it now holds.
    ///
    /// Reuses `out`'s allocation, so a front thread that keeps its copy allocates only
    /// when the part count grows.
    pub fn read_into(&self, out: &mut Vec<Part>) -> u64 {
        // Read before the copy: a publish that lands between the two leaves `out` holding
        // the newer set under the older version, so the next read refreshes it again.
        // Reversed, it would leave a stale copy under the current version and never
        // refresh.
        let version = self.version();
        if let Ok(held) = self.parts.lock() {
            out.clear();
            out.extend_from_slice(&held);
        }
        version
    }
}

/// What the front thread has decided about a region, read by [`Frame::should_draw`].
///
/// The front thread writes here and bumps the region's [`Epoch`] **directly** — it does
/// not go through the app thread. The next present carries the new pixels: one display
/// frame, the same latency the retained path has, and a busy app thread can never stall a
/// gesture. The app is told *afterwards*, once the pixels are committed to.
///
/// No input is routed to the present thread and no [`Frame`] method is callable from
/// another one. The renderer publishes geometry and consumes state; it is never asked a
/// question synchronously.
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

impl RegionInput {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The part under the pointer.
    #[must_use]
    pub fn hover(&self) -> Option<SubId> {
        sub(self.hover.load(Ordering::Acquire))
    }

    /// The part under an in-flight gesture.
    #[must_use]
    pub fn active(&self) -> Option<SubId> {
        sub(self.active.load(Ordering::Acquire))
    }

    /// The pointer, in region-local DIPs.
    #[must_use]
    pub fn cursor(&self) -> Option<(f32, f32)> {
        let packed = self.cursor.load(Ordering::Acquire);
        let (x, y) = (
            f32::from_bits((packed >> 32) as u32),
            f32::from_bits(packed as u32),
        );
        (!x.is_nan() && !y.is_nan()).then_some((x, y))
    }

    pub fn set_hover(&self, id: Option<SubId>) {
        self.hover.store(raw_sub(id), Ordering::Release);
    }

    pub fn set_active(&self, id: Option<SubId>) {
        self.active.store(raw_sub(id), Ordering::Release);
    }

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

/// Everything a renderer is handed for one frame.
#[derive(Copy, Clone)]
pub struct FrameCtx<'a> {
    /// The region's DIP box, and the display it is solved for. Every number a renderer
    /// lays out against comes from here, so the buffer's allocation and the coordinates
    /// drawn into it cannot disagree.
    pub extent: Extent,
    /// A monotonic present-thread counter. Increments once per *pass*, not once per frame
    /// of a batch — it identifies the wake, which is what a version gate wants.
    pub tick: u64,
    /// The region's own device. Build every brush, geometry and text layout from it;
    /// resources from another `Gpu` do not bind here.
    pub device: &'a Gpu,
    /// The draw choke, the same value the retained path uses.
    ///
    /// A `Frame` holds `Radiance`, `windows-d2d` accepts only `Scrgb`, and
    /// `OutputTransform::apply` is the only bridge — so a colour that skips the transform
    /// does not compile and one that takes it twice is unrepresentable. "Exactly once"
    /// needs no discipline here.
    pub out: OutputTransform,
    /// What the front thread has decided about this region since the last frame.
    pub input: &'a RegionInput,
}

impl FrameCtx<'_> {
    /// The region's DIP width.
    #[must_use]
    pub fn w(&self) -> f32 {
        self.extent.w
    }

    /// The region's DIP height.
    #[must_use]
    pub fn h(&self) -> f32 {
        self.extent.h
    }

    /// DIP-to-pixel factor — the scale a realization or a cached raster is keyed on.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.extent.scale()
    }
}

/// One consumer of the per-frame path.
///
/// Built **on the present thread** by a factory, so it may hold `!Send` state — which it
/// must, because everything it draws with belongs to the region's own device.
pub trait Frame {
    /// Advances live state and reports whether this frame would **differ** from the one
    /// already on screen. `false` skips the tick entirely: no draw, and — more to the
    /// point — no present, so the compositor is not woken.
    ///
    /// Allocation-free, and called every tick whether or not `draw` follows. It is asked
    /// **once per pass and not once per slot**: it both tests and *commits* its version
    /// stamp, so asking it per slot would consume the change on a batch's first frame and
    /// report "nothing moved" for the rest of it.
    fn should_draw(&mut self, ctx: FrameCtx<'_>) -> bool;

    /// Draws one frame.
    ///
    /// The target is bound in DIPs with the region's origin at `(0, 0)`, and its contents
    /// are **undefined** on entry — so this must clear or cover the whole box.
    ///
    /// Called `depth` times per pass, once per frame the batch will show, in the order
    /// they will be shown. An ease stepped inside it cannot tell that the calls arrived
    /// together: a batch draws exactly the frames that will appear, so the motion is
    /// identical to a pass per refresh.
    ///
    /// It must never do anything that can wait. A later region's buffer acquisition can
    /// block with this pass's bracket already open, which is correct and deliberate — the
    /// alternative is a bracket per region — but it is why blocking here compounds.
    fn draw(&mut self, ctx: FrameCtx<'_>, draw: &Draw<'_>);

    /// True when **every pixel** of the box is covered opaquely.
    ///
    /// This is the difference between DWM drawing the region and the display controller
    /// scanning it out, and it decides the alpha mode and the displayable allocation
    /// together. A renderer whose first act is not a full-cover clear or fill must answer
    /// `false`: claiming opacity while leaving pixels through shows whatever the buffer
    /// happened to hold, and the region is composed anyway.
    ///
    /// **Opacity is work, not a flag.** A region earns it by drawing the chrome inside its
    /// own box rather than leaving gaps for what sits underneath — which is what took the
    /// one region DWM still composed to independent flip.
    ///
    /// Read **once**, when the region is allocated, because it decides the allocation.
    /// Debug builds check it has not changed since.
    fn opaque(&self) -> bool {
        false
    }

    /// True while something this renderer draws is still moving, so the present thread
    /// keeps display-clock pacing alive rather than parking.
    fn animating(&self) -> bool {
        false
    }

    /// Drop every cached device resource: the region was rebuilt and the device those
    /// resources came from is gone. Called before the next [`draw`](Self::draw).
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

        // The niche and the sentinel are not values a caller can smuggle a `Some` through.
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
}
