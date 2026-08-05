//! Scroll and virtualization: the two policies that sit over the tracker.
//!
//! **Scroll is tracker-delegated, always.** The viewport does not move; it clips. The
//! content's offset and the thumb's offset are both bound to the one tracker, so the thumb
//! follows the content with no per-frame front-thread work.
//!
//! Thumb geometry and the realization window live here rather than in `windows-scene`
//! because both are shaped by the widget that consumes them. `windows-scene` supplies the
//! tracker and the binding.

use crate::bindings::GestureSettings;
use crate::build::{Any, El, Host, IntoChildren, View};
use crate::gesture::{Commit, DragAxes, DragDecl, DragPhase, GestureDecl};
use crate::input::Report;
use crate::role::Metric;
use crate::signal::{Cell, Memo};
use crate::widget::Front;
use core::cell::RefCell;
use core::ops::Range;
use std::rc::Rc;
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{
    Anim, Bind, ControlId, GroupId, HitDecl, HitFlags, NodeId, Prop, SceneEvent, SpriteId,
    TrackerRequest, Tuning, Value,
};

use super::Preset;

/// When the thumb is visible.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Reveal {
    /// Always, taking room from the content.
    Always,
    /// Never — for a surface whose extent is obvious from what is in it.
    Never,
    /// While the content is moving, while the pointer is over the surface, and for a moment
    /// after either ends.
    #[default]
    OnDemand,
}

// ── thumb geometry ───────────────────────────────────────────────────────────────
//
// Raw DIPs: these are the scrollbar's own dimensions rather than the palette's spacing
// scale, so they carry no `Metric` and do not move with density.

/// How wide the thumb is.
pub const THUMB_W: f32 = 6.0;
/// How far it is inset from the right edge, and from each end of its travel.
pub const THUMB_MARGIN: f32 = 2.0;
/// The thumb's minimum height, however long the content: a thumb that shrinks to nothing
/// cannot be grabbed.
pub const THUMB_MIN_H: f32 = 24.0;
/// How far past the thumb a pointer still counts as over it. The drawn bar is 6 DIP, under
/// the system's minimum target, so the hit entry is inflated rather than the bar widened.
const THUMB_GRAB: f32 = 8.0;

/// How long a concealed thumb waits before fading, once the reason to show it ends.
///
/// A delay on the spring rather than a timer: the compositor holds it, so a re-reveal
/// inside the window is a retarget and the front thread never wakes for either edge.
const CONCEAL_MS: u32 = 700;

/// What a scrollbar is, at one pair of extents.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ThumbGeom {
    /// Whether there is anything to scroll. A viewport larger than its content has no
    /// thumb and no tracker travel.
    pub overflow: bool,
    /// How far the content can move.
    pub max_scroll: f32,
    /// How tall the thumb is.
    pub thumb_h: f32,
    /// How far the thumb itself travels, which is not how far the content does.
    pub travel: f32,
}

/// Returns the scrollbar geometry for a viewport of `viewport_h` showing `content_h` of
/// content.
#[must_use]
pub fn thumb_geom(viewport_h: f32, content_h: f32) -> ThumbGeom {
    let max_scroll = (content_h - viewport_h).max(0.0);
    let track_h = (viewport_h - 2.0 * THUMB_MARGIN).max(0.0);
    if max_scroll <= 0.0 || track_h <= 0.0 {
        return ThumbGeom {
            overflow: false,
            max_scroll: 0.0,
            thumb_h: 0.0,
            travel: 0.0,
        };
    }
    // Proportional, then floored at THUMB_MIN_H and capped at the track: a very long
    // document keeps a grabbable thumb, and the travel below subtracts the floored height
    // rather than the proportional one.
    let ratio = (viewport_h / content_h).clamp(0.0, 1.0);
    let thumb_h = (track_h * ratio).max(THUMB_MIN_H).min(track_h);
    ThumbGeom {
        overflow: true,
        max_scroll,
        thumb_h,
        travel: track_h - thumb_h,
    }
}

/// Returns where the thumb sits when the content is at `scroll`.
///
/// The same affine map the compositor evaluates from the tracker, so a grab starts from the
/// value the binding is rendering.
#[must_use]
pub fn thumb_y_for_scroll(scroll: f32, geom: ThumbGeom) -> f32 {
    if geom.max_scroll <= 0.0 {
        return THUMB_MARGIN;
    }
    THUMB_MARGIN + (scroll / geom.max_scroll).clamp(0.0, 1.0) * geom.travel
}

/// Returns the content offset a thumb dragged to `thumb_y` means, inverting
/// [`thumb_y_for_scroll`].
#[must_use]
pub fn scroll_for_thumb_y(thumb_y: f32, geom: ThumbGeom) -> f32 {
    if geom.travel <= 0.0 {
        return 0.0;
    }
    ((thumb_y - THUMB_MARGIN) / geom.travel).clamp(0.0, 1.0) * geom.max_scroll
}

/// Returns the rail's style: a strip down the right edge of the viewport, full height.
///
/// **The rail is the grab target, and the thumb is not.** The compositor moves the thumb, so
/// its layout rect stays where the solve put it however far the content has travelled, and a
/// hit entry on it would stop being under the drawn bar. The rail is static geometry, and
/// where inside it a press landed is answered from the reported position.
///
/// Pinned rather than scrolled: it lives inside the container it reports on and does not
/// move with it.
#[must_use]
pub fn rail_style() -> windows_scene::taffy::Style {
    use windows_scene::taffy;
    use windows_scene::taffy::style_helpers::{TaffyAuto, TaffyZero};
    taffy::Style {
        position: taffy::Position::Absolute,
        size: taffy::Size {
            width: taffy::Dimension::length(THUMB_W + 2.0 * THUMB_MARGIN),
            height: taffy::Dimension::AUTO,
        },
        inset: taffy::Rect {
            left: taffy::LengthPercentageAuto::AUTO,
            right: taffy::LengthPercentageAuto::ZERO,
            top: taffy::LengthPercentageAuto::ZERO,
            bottom: taffy::LengthPercentageAuto::ZERO,
        },
        ..taffy::Style::DEFAULT
    }
}

/// Returns the thumb's own box inside the rail: as tall as `geom` says, at the top of its
/// travel.
///
/// Built as a style rather than through the [`Over`](super::Over) vocabulary because the
/// numbers are this component's own geometry, resolved from extents the solve produced, and
/// [`Len`](super::Len) states no raw DIP. The thumb's offset is the tracker's alone: laying
/// it out would leave layout and the tracker writing one channel between them.
#[must_use]
pub fn thumb_style(geom: ThumbGeom) -> windows_scene::taffy::Style {
    use windows_scene::taffy;
    use windows_scene::taffy::style_helpers::{TaffyAuto, TaffyZero};
    taffy::Style {
        position: taffy::Position::Absolute,
        size: taffy::Size {
            width: taffy::Dimension::length(THUMB_W),
            height: taffy::Dimension::length(geom.thumb_h),
        },
        inset: taffy::Rect {
            left: taffy::LengthPercentageAuto::AUTO,
            right: taffy::LengthPercentageAuto::length(THUMB_MARGIN),
            top: taffy::LengthPercentageAuto::ZERO,
            bottom: taffy::LengthPercentageAuto::AUTO,
        },
        ..taffy::Style::DEFAULT
    }
}

/// What a scroll container is declared with.
///
/// The state travels with the reveal policy so that a list and the mount reporting into it
/// share one [`ScrollState`]; a second handle would be a second answer to where the content
/// is.
#[derive(Copy, Clone, Debug)]
pub struct ScrollDecl {
    pub reveal: Reveal,
    pub state: ScrollState,
}

/// Returns a scrolling container over `children`, with the default reveal policy.
///
/// The children go into a content group of their own, because the viewport must not move:
/// it is what clips, and an offset on it would take the clip with it.
#[must_use]
pub fn scroll(children: impl IntoChildren) -> View {
    scroll_with(Reveal::default(), children)
}

/// Returns a scrolling container with an explicit thumb reveal policy.
#[must_use]
pub fn scroll_with(reveal: Reveal, children: impl IntoChildren) -> View {
    scroll_state(
        ScrollDecl {
            reveal,
            state: ScrollState::new(),
        },
        super::stack(children),
    )
}

/// Wraps `content` in a viewport declared by `decl`.
///
/// The content never shrinks to its viewport: a flex child squeezed back to its parent never
/// overflows, and a container with no overflow has no travel and no thumb.
fn scroll_state(decl: ScrollDecl, content: El<Any>) -> View {
    El::<Any>::viewport(decl, content.no_shrink())
}

// ── where the content is ─────────────────────────────────────────────────────────

/// Where a scroll container's content is, as values rather than as events.
///
/// A tracker's own getter answers with what was last set rather than with what the
/// compositor is evaluating, so the position reported in a [`SceneEvent`] is **the only
/// trustworthy read of one**. [`observe`] writes what it was told into here, and everything
/// above reads it as an ordinary signal — the realization window is a [`Memo`] over it.
#[derive(Copy, Clone, Debug)]
pub struct ScrollState {
    offset: Cell<f32>,
    /// Where inertia will rest, for as long as it is running.
    ///
    /// Held **beside** the offset and never in place of it: the destination is realized as
    /// soon as it is known, while the rows the offset still names stay realized too.
    target: Cell<Option<f32>>,
    /// The viewport's solved height.
    viewport: Cell<f32>,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    /// Returns a state at the origin, with no viewport height and nothing in flight.
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset: Cell::new(0.0),
            target: Cell::new(None),
            viewport: Cell::new(0.0),
        }
    }

    /// Records the content offset the tracker reported.
    pub fn moved(self, y: f32) {
        self.offset.set(y);
    }

    /// Records where inertia will rest.
    ///
    /// `y` must be the **modified** destination: snap points are applied to that one and not
    /// to the natural one.
    pub fn flinging_to(self, y: f32) {
        self.target.set(Some(y));
    }

    /// Clears the destination once inertia ends, leaving nothing ahead to realize.
    pub fn settled(self) {
        self.target.set(None);
    }

    /// Records the viewport's own height, from the solved layout.
    pub fn resized(self, height: f32) {
        self.viewport.set(height);
    }

    /// Returns the content offset the tracker last reported.
    #[must_use]
    pub fn offset(self) -> f32 {
        self.offset.get()
    }

    /// Returns where inertia will rest, or `None` when nothing is in flight.
    #[must_use]
    pub fn target(self) -> Option<f32> {
        self.target.get()
    }

    /// Returns the viewport's height.
    #[must_use]
    pub fn viewport(self) -> f32 {
        self.viewport.get()
    }
}

// ── virtualization ───────────────────────────────────────────────────────────────

/// What a list needs to decide which rows exist.
///
/// **Uniform extents.** With a fixed row height a row's offset is affine in its index and
/// the maximum scroll position is a constant, so no estimate is corrected as rows realize
/// and the maximum never shifts mid-fling.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ListSpec {
    /// How many rows the list has.
    pub count: usize,
    /// The row height, as the palette's — so a list is as dense as the user asked for.
    pub row_h: Metric,
    /// Rows realized beyond the viewport on each side. Two or three: enough that a row
    /// exists before it is looked at, few enough that a fling does not realize a screen it
    /// will never show.
    pub overscan: usize,
}

/// Rows realized beyond the viewport on each side unless a list states otherwise.
const OVERSCAN: usize = 3;

impl ListSpec {
    /// Returns a list of `count` rows, each one [`Metric::RowH`](crate::role::Metric) tall.
    ///
    /// The height is a metric rather than a length, so the list is as dense as the user
    /// asked for and re-lowers with the type ramp.
    #[must_use]
    pub const fn uniform(count: usize, row_h: Metric) -> Self {
        Self {
            count,
            row_h,
            overscan: OVERSCAN,
        }
    }

    /// Returns the same list realizing `overscan` rows past each edge of the viewport.
    #[must_use]
    pub const fn overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }
}

/// How many points are sampled between where a fling started and where it lands.
///
/// The corridor covers a glance mid-flight rather than a read, so the path is sampled and
/// the rows realized for it do not scale with the distance flung.
const CORRIDOR: usize = 2;

/// How many runs a realized set holds: the live window, the destination, and the corridor
/// between them.
const MAX_RUNS: usize = 2 + CORRIDOR;

/// Which rows are worth existing, as a bounded set of runs.
///
/// Several runs rather than one range: the resting position is known the instant inertia
/// begins, so the rows a fling lands on are realized before it arrives, and those are
/// nowhere near the ones on screen. Bounded and `Copy`, so realization allocates nothing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Realized {
    runs: [(usize, usize); MAX_RUNS],
    len: usize,
}

impl Realized {
    const EMPTY: Self = Self {
        runs: [(0, 0); MAX_RUNS],
        len: 0,
    };

    /// Returns the runs, ascending and disjoint.
    pub fn runs(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.runs[..self.len].iter().map(|&(start, end)| start..end)
    }

    /// Returns how many rows the set realizes.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.runs[..self.len]
            .iter()
            .map(|&(start, end)| end - start)
            .sum()
    }

    /// Returns whether `index` falls in one of the runs.
    #[must_use]
    pub fn contains(&self, index: usize) -> bool {
        self.runs[..self.len]
            .iter()
            .any(|&(start, end)| index >= start && index < end)
    }

    /// Appends a run, dropping an empty one and anything past `MAX_RUNS`.
    fn push(&mut self, run: Range<usize>) {
        if run.is_empty() || self.len == MAX_RUNS {
            return;
        }
        self.runs[self.len] = (run.start, run.end);
        self.len += 1;
    }

    /// Sorts the runs by start and coalesces the overlaps. Insertion sort over at most
    /// `MAX_RUNS` entries, in place.
    fn merge(&mut self) {
        for i in 1..self.len {
            let mut j = i;
            while j > 0 && self.runs[j - 1].0 > self.runs[j].0 {
                self.runs.swap(j - 1, j);
                j -= 1;
            }
        }
        let mut write = 0;
        for read in 1..self.len {
            if self.runs[read].0 <= self.runs[write].1 {
                self.runs[write].1 = self.runs[write].1.max(self.runs[read].1);
            } else {
                write += 1;
                self.runs[write] = self.runs[read];
            }
        }
        self.len = self.len.min(write + 1);
    }
}

/// Returns the rows worth realizing at `scroll_y`, plus `spec.overscan` on each side.
///
/// **The range is inside `0..spec.count` at any `scroll_y`.** A tracker's position travels
/// outside its bounds during a manipulation — the overpan is the bounce — so this is asked
/// about positions past the end of the content. A non-positive `row_h` or an empty list
/// answers `0..0`.
#[must_use]
pub fn window(scroll_y: f32, viewport_h: f32, row_h: f32, spec: &ListSpec) -> Range<usize> {
    if row_h <= 0.0 || spec.count == 0 {
        return 0..0;
    }
    let first = (((scroll_y / row_h).floor() as isize - spec.overscan as isize).max(0) as usize)
        .min(spec.count);
    let last = (((scroll_y + viewport_h) / row_h).ceil() as usize + spec.overscan).min(spec.count);
    first..last.max(first)
}

/// Returns the whole realized set: where the content is, where a fling is taking it, and a
/// fixed number of samples of the path between.
///
/// The corridor is sampled rather than swept, so a fling crossing three thousand rows
/// realizes two windows and two overscan bands however far it travels. Allocates nothing.
#[must_use]
pub fn realize(
    offset: f32,
    target: Option<f32>,
    viewport_h: f32,
    row_h: f32,
    spec: &ListSpec,
) -> Realized {
    let mut out = Realized::EMPTY;
    out.push(window(offset, viewport_h, row_h, spec));
    if let Some(target) = target {
        for step in 1..=CORRIDOR {
            let at = offset + (target - offset) * (step as f32) / ((CORRIDOR + 1) as f32);
            // Zero height, so a sample is the overscan band around a point rather than a
            // second viewport's worth of rows nobody will look at.
            out.push(window(at, 0.0, row_h, spec));
        }
        out.push(window(target, viewport_h, row_h, spec));
    }
    out.merge();
    out
}

/// Returns a virtualized list: a scroll container whose rows exist only near the viewport.
///
/// Rows are **placed rather than laid out** — each is absolute at its own index's offset, and
/// the content group's height is the whole list's. So the scroll extent does not move when
/// the realized window does, the maximum position stays the constant a uniform row height
/// gives, and the realized set can be several disjoint runs rather than one contiguous span.
///
/// Rows are keyed by index and reconciled by the same keyed `each` as any other list, so a
/// row's index is fixed for its life and its placement is written once. `items` must push
/// the items it has for the runs it is handed **in ascending index order**; a realized index
/// it does not supply gets a placeholder of the right height rather than a gap.
#[must_use]
pub fn list<T, V>(
    spec: impl Fn() -> ListSpec + 'static,
    items: impl Fn(&Realized, &mut Vec<(usize, T)>) + 'static,
    view: impl Fn(&T) -> El<V> + 'static,
) -> View
where
    T: 'static,
    V: 'static,
{
    let state = ScrollState::new();
    let spec = Memo::new(spec);
    // Its own memo, so a row's placement re-lowers on a density change and on nothing else —
    // not when the count moves, which is most of what `spec` reports.
    let row_metric = Memo::new(move || spec.get().row_h);
    let realized = Memo::new(move || {
        let spec = spec.get();
        // The root scope: a row height varies with density, a root axis, and not with the
        // elevation or width a surface pushes.
        let row_h = crate::role::metric(spec.row_h, Host::with(|h| h.root_scope));
        realize(
            state.offset(),
            state.target(),
            state.viewport(),
            row_h,
            &spec,
        )
    });

    // Reused across reconciles, so the fill path allocates only until its high-water mark.
    let supplied: Rc<RefCell<Vec<(usize, T)>>> = Rc::new(RefCell::new(Vec::new()));
    // Keyed by index, and carrying it: the key is what recycles a row and the value is what
    // places it, and a row's placement has to survive the reconcile that moved it.
    let rows = crate::build::each_into(
        move |out: &mut Vec<(usize, Option<T>)>| {
            let realized = realized.get();
            let mut supplied = supplied.borrow_mut();
            supplied.clear();
            items(&realized, &mut supplied);
            let mut supplied = supplied.drain(..).peekable();
            for run in realized.runs() {
                for index in run {
                    while supplied.peek().is_some_and(|&(at, _)| at < index) {
                        supplied.next();
                    }
                    let item = match supplied.peek() {
                        Some(&(at, _)) if at == index => supplied.next().map(|(_, item)| item),
                        _ => None,
                    };
                    out.push((index, item));
                }
            }
        },
        // The index is the key and also what places the row: one field, projected.
        |(index, _): &(usize, Option<T>)| index,
        // A realized index the caller did not supply gets its space and nothing in it: the
        // row is where it will be when the data arrives, and nothing invented stands in for
        // the data.
        move |(index, item): &(usize, Option<T>)| {
            let at = *index as f32;
            match item {
                Some(item) => view(item).band_rows(at, move || row_metric.get()).erase(),
                None => El::<Any>::seed_bare().band_rows(at, move || row_metric.get()),
            }
        },
    );

    scroll_state(
        ScrollDecl {
            reveal: Reveal::default(),
            state,
        },
        El::<Any>::seed_bare()
            .height_rows(move || spec.get().row_h, move || spec.get().count as f32)
            .contain(Preset::Bare, rows),
    )
}

// ── the front thread's half ──────────────────────────────────────────────────────

/// One scroll container, as the tick needs it. Held by the host beside the mount that owns
/// the tracker, so a container unmounting takes its row with it.
pub(crate) struct ScrollRow {
    pub tracker: windows_scene::TrackerId<windows_scene::Observed>,
    pub viewport: NodeId,
    pub content: NodeId,
    pub thumb: Option<SpriteId>,
    /// The strip the thumb travels in, which is the static geometry a grab lands on.
    pub rail: Option<GroupId>,
    /// The viewport's own control, which is what a hover names.
    pub control: Option<ControlId>,
    /// The rail's, which is what a grab names. Minted only where there is a thumb.
    pub grab: Option<ControlId>,
    pub reveal: Reveal,
    pub state: ScrollState,
    /// What was last published, so a solve that moved nothing emits nothing.
    pub last: ThumbGeom,
    /// Where the content stood when the current thumb grab began.
    pub grabbed_at: Option<f32>,
    /// What the thumb's opacity was last retargeted to, so an unchanged reason emits nothing.
    pub shown: bool,
}

/// Records what the trackers reported into each container's [`ScrollState`].
///
/// **Writes signals only**, and runs before the flush, so the realization window a reported
/// position implies is resolved in the tick that position arrived in.
pub fn observe(events: &[SceneEvent]) {
    if events.is_empty() {
        return;
    }
    Host::with(|host| {
        for event in events {
            match *event {
                SceneEvent::TrackerValues {
                    tracker, position, ..
                } => host.scroll_by_tracker(tracker, |row| row.state.moved(position.y)),
                SceneEvent::InertiaStarting {
                    tracker, modified, ..
                } => host.scroll_by_tracker(tracker, |row| row.state.flinging_to(modified.y)),
                SceneEvent::TrackerPhase {
                    tracker,
                    phase: windows_scene::Phase::Idle,
                } => host.scroll_by_tracker(tracker, |row| row.state.settled()),
                _ => {}
            }
        }
    });
}

/// Applies a tick's events and reports to the compositor: a thumb's reveal, and a thumb
/// being dragged.
///
/// Must run **after the apply**, so a retarget never names a node the patch was about to
/// rebuild, and after the router's tick, so a grab resolves against the hit array this frame
/// published.
///
/// # Errors
///
/// The compositor refused a retarget or a tracker request. The first failure is returned;
/// the rest of the tick still runs.
pub fn front(events: &[SceneEvent], reports: &[Report], front: &mut Front<'_>) -> Result<()> {
    if events.is_empty() && reports.is_empty() {
        return Ok(());
    }
    // The first failure is kept and the rest of the tick still runs: a refused retarget on
    // one surface must not leave another's grab half-applied.
    let mut failed: Option<windows_core::Error> = None;
    Host::with(|host| {
        for event in events {
            let (tracker, moving) = match *event {
                SceneEvent::TrackerPhase { tracker, phase } => {
                    (tracker, phase != windows_scene::Phase::Idle)
                }
                _ => continue,
            };
            host.scroll_by_tracker(tracker, |row| {
                if let Err(error) = reveal(row, moving, front) {
                    failed.get_or_insert(error);
                }
            });
        }
        for report in reports {
            match *report {
                Report::HoverChanged { from, to, .. } => {
                    for (id, over) in [(from, false), (to, true)] {
                        let Some(id) = id else { continue };
                        host.scroll_by_control(id, |row| {
                            if let Err(error) = reveal(row, over, front) {
                                failed.get_or_insert(error);
                            }
                        });
                    }
                }
                Report::Dragged { target, update, .. } => host.scroll_by_grab(target, |row| {
                    if let Err(error) = drag(row, update.phase, update.delta.y, front) {
                        failed.get_or_insert(error);
                    }
                }),
                // A released or cancelled grab forgets where it started, so the next one
                // measures from where the content actually is.
                Report::Released { target, .. } | Report::Canceled { target, .. } => {
                    host.scroll_by_grab(target, |row| row.grabbed_at = None);
                }
                _ => {}
            }
        }
    });
    failed.map_or(Ok(()), Err)
}

/// Shows or conceals a thumb.
///
/// Only [`Reveal::OnDemand`] retargets, and only on an edge: `Always` is opaque from the
/// mount and `Never` has no thumb, so neither reaches the compositor here.
fn reveal(row: &mut ScrollRow, show: bool, front: &mut Front<'_>) -> Result<()> {
    let Some(thumb) = row.thumb else {
        return Ok(());
    };
    // A grabbed thumb stays lit however the pointer wanders, and a moving one stays lit
    // whatever the pointer is doing.
    let show = show || row.grabbed_at.is_some();
    if row.reveal != Reveal::OnDemand || show == row.shown {
        return Ok(());
    }
    row.shown = show;
    let (to, delay_ms) = if show { (1.0, 0) } else { (0.0, CONCEAL_MS) };
    front.scene.retarget(
        thumb.node(),
        Prop::Opacity,
        Bind::Animate(Anim::Spring {
            to: Value::Scalar(to),
            tuning: Tuning::Chrome,
            delay_ms,
        }),
        front.back,
        front.env,
    )
}

/// Drives the tracker from a thumb the pointer is dragging.
///
/// The displacement is from the contact's origin, so the content position is resolved from
/// where it stood when the grab began rather than accumulated — a dropped sample then costs
/// nothing, where an accumulated one would drift for the rest of the drag.
fn drag(row: &mut ScrollRow, phase: DragPhase, dy: f32, front: &mut Front<'_>) -> Result<()> {
    if phase == DragPhase::Undecided {
        return Ok(());
    }
    let from = *row.grabbed_at.get_or_insert_with(|| row.state.offset());
    let thumb_y = thumb_y_for_scroll(from, row.last) + dy;
    let to = scroll_for_thumb_y(thumb_y, row.last);
    front
        .scene
        .request(row.tracker, TrackerRequest::To(Vector2 { x: 0.0, y: to }))
        .map(|_| ())
}

/// Returns the rail's gesture declaration, so a pointer can grab the bar in it.
///
/// A hit entry and a drag, with no wash and no chrome row: the thumb's opacity is retargeted
/// here, and a control the front table adopted would give that channel two owners.
pub(crate) fn grab_decl() -> GestureDecl {
    GestureDecl {
        settings: GestureSettings::None,
        drag: Some(DragDecl {
            axes: DragAxes::Vertical,
            // Below the default: a scrollbar is aimed at, so the grab should follow the
            // first pixel rather than absorb six of them.
            threshold: 1.0,
            commit: Commit::Live,
            ..DragDecl::default()
        }),
        ..GestureDecl::default()
    }
}

/// Returns the rail's hit entry: interactive, unscrolled, and inflated for touch.
pub(crate) fn grab_hit(id: ControlId) -> HitDecl {
    HitDecl {
        // Pinned: the rail lives inside the container it reports on and does not move with
        // it, so its rect must not resolve through that container's offset.
        flags: HitFlags::INTERACTIVE | HitFlags::UNSCROLLED,
        id,
        touch_inflate: Some(THUMB_GRAB),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: ListSpec = ListSpec {
        count: 100,
        row_h: Metric::RowH,
        overscan: 2,
    };

    /// A viewport bigger than its content has no thumb, no travel and nothing to scroll.
    #[test]
    fn content_that_fits_has_no_scrollbar() {
        let g = thumb_geom(400.0, 200.0);
        assert!(!g.overflow);
        assert_eq!((g.max_scroll, g.thumb_h, g.travel), (0.0, 0.0, 0.0));
    }

    /// A very long document still gets a thumb big enough to grab, and its travel is
    /// corrected for that floor rather than running past the end of the track.
    #[test]
    fn a_long_document_keeps_a_grabbable_thumb_inside_its_track() {
        let g = thumb_geom(400.0, 100_000.0);
        assert!(g.overflow);
        assert!((g.thumb_h - THUMB_MIN_H).abs() < 1.0e-3);
        let track = 400.0 - 2.0 * THUMB_MARGIN;
        assert!(g.travel >= 0.0 && g.thumb_h + g.travel <= track + 1.0e-3);
    }

    /// The two thumb functions are inverses over the whole travel, which is what lets a grab
    /// start from the value the compositor is already rendering.
    #[test]
    fn the_thumb_maps_both_ways() {
        let g = thumb_geom(400.0, 2000.0);
        for step in 0..=10u8 {
            let scroll = g.max_scroll * f32::from(step) / 10.0;
            let back = scroll_for_thumb_y(thumb_y_for_scroll(scroll, g), g);
            assert!((back - scroll).abs() < 0.01, "{scroll} → {back}");
        }
    }

    /// With nothing to scroll, a dragged thumb asks for nothing rather than dividing by its
    /// own zero travel.
    #[test]
    fn a_thumb_with_no_travel_maps_to_the_top() {
        let g = thumb_geom(400.0, 200.0);
        assert_eq!(thumb_y_for_scroll(50.0, g), THUMB_MARGIN);
        assert_eq!(scroll_for_thumb_y(300.0, g), 0.0);
    }

    /// The window covers the viewport, plus the overscan on each side, and never runs past
    /// the ends.
    #[test]
    fn the_realization_window_covers_the_viewport_and_is_clamped_at_both_ends() {
        let top = window(0.0, 100.0, 20.0, &SPEC);
        assert_eq!(top.start, 0, "the overscan cannot go negative");
        assert!(top.end >= 5 && top.end <= 8);

        let middle = window(400.0, 100.0, 20.0, &SPEC);
        assert_eq!(middle.start, 18, "twenty rows in, less two of overscan");
        assert_eq!(middle.end, 27, "five visible, plus two either side");

        let bottom = window(1900.0, 100.0, 20.0, &SPEC);
        assert_eq!(bottom.end, 100, "the overscan cannot go past the last row");
    }

    /// A tracker overpans past the end of the content, and the window stays inside the list.
    ///
    /// The overpan is the bounce, so this position is reached by ordinary use. A window
    /// running past the last row underflows a row count: a debug panic, and in release a
    /// placement far past the end of the list.
    #[test]
    fn an_overpanned_window_stays_inside_the_list() {
        let bounced = window(2100.0, 100.0, 20.0, &SPEC);
        assert!(bounced.start <= SPEC.count && bounced.end <= SPEC.count);
        assert!(
            bounced.is_empty(),
            "nothing past the end is worth realizing"
        );
    }

    /// An empty list realizes nothing, and a zero row height does not divide by itself.
    #[test]
    fn a_degenerate_list_realizes_nothing() {
        let spec = ListSpec { count: 0, ..SPEC };
        assert!(window(0.0, 100.0, 20.0, &spec).is_empty());
        assert!(window(0.0, 100.0, 0.0, &ListSpec { count: 10, ..spec }).is_empty());
        assert_eq!(realize(0.0, None, 100.0, 20.0, &spec).rows(), 0);
    }

    /// At rest the realized set is exactly the live window: one run, no corridor, nothing
    /// realized ahead of a fling that is not happening.
    #[test]
    fn a_resting_list_realizes_one_run() {
        let at_rest = realize(400.0, None, 100.0, 20.0, &SPEC);
        assert_eq!(at_rest.runs().count(), 1);
        assert_eq!(at_rest.runs().next().unwrap(), 18..27);
    }

    /// A fling realizes where it lands as well as where it is, and the two are disjoint,
    /// which is why the set is not one range.
    #[test]
    fn a_long_fling_realizes_its_destination_and_a_bounded_corridor() {
        let flung = realize(0.0, Some(1900.0), 100.0, 20.0, &SPEC);
        assert!(flung.contains(0), "where the content still is");
        assert!(flung.contains(99), "where it is going");
        assert!(!flung.contains(50), "and not the whole path between");
        assert!(flung.runs().count() > 1);
        // Two windows and two bands, and nothing that scales with the distance flung.
        assert!(flung.rows() <= 4 * (100 / 20 + 2 * SPEC.overscan + 2));
    }

    /// A short fling's destination overlaps the live window, and the two coalesce rather than
    /// realizing the same rows twice.
    #[test]
    fn a_short_fling_coalesces_into_one_run() {
        let nudged = realize(400.0, Some(440.0), 100.0, 20.0, &SPEC);
        assert_eq!(nudged.runs().count(), 1);
        let run = nudged.runs().next().unwrap();
        assert_eq!(run, 18..29);
    }

    /// The runs come out ascending and disjoint however they went in, because the fill walks
    /// them in order and a supplied item is matched by a single forward scan.
    #[test]
    fn the_runs_are_ascending_and_disjoint() {
        let flung = realize(1900.0, Some(0.0), 100.0, 20.0, &SPEC);
        let mut last = 0;
        for run in flung.runs() {
            assert!(run.start >= last, "{run:?} after {last}");
            assert!(run.end > run.start);
            last = run.end;
        }
    }
}
