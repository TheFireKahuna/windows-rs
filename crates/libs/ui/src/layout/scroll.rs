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
use crate::build::{Element, Host, Node, Ui};
use crate::gesture::{Commit, DragAxes, DragDecl, DragPhase, GestureDecl};
use crate::input::Report;
use crate::role::Metric;
use crate::seam::{ScrollFront, ScrollOp};
use crate::signal::{Cell, Effect, Memo};
use crate::widget::Front;
use core::cell::RefCell;
use core::ops::Range;
use core::sync::atomic::Ordering;
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{
    Anim, Bind, ControlId, GroupId, HitDecl, HitFlags, NodeId, Prop, SceneEvent, SpriteId,
    TrackerRequest, Tuning, Value, unpack_offset,
};

use super::{Len, Preset, Table, anchors, probe};

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

/// What a scroll container is declared with.
///
/// The state travels with the reveal policy so that a list and the mount reporting into it
/// share one [`ListState`]; a second handle would be a second answer to where the content
/// is. Ordinary containers hold no application state; their position stays in the
/// scene-side shadow shared with input hit testing.
#[derive(Copy, Clone, Debug)]
pub struct ScrollDecl {
    pub reveal: Reveal,
    pub state: Option<ListState>,
}

/// Returns a scrolling container over `children`, with the default reveal policy.
///
/// The children go into a content group of their own, because the viewport must not move:
/// it is what clips, and an offset on it would take the clip with it.
pub fn scroll<'a>(ui: &'a mut Ui<'_>, children: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    scroll_with(ui, Reveal::default(), children)
}

/// Creates a scrolling container with an explicit thumb reveal policy.
pub fn scroll_with<'a>(
    ui: &'a mut Ui<'_>,
    reveal: Reveal,
    children: impl FnOnce(&mut Ui<'_>),
) -> Element<'a> {
    ui.scroll(
        ScrollDecl {
            reveal,
            state: None,
        },
        children,
    )
}

// ── where the content is, and how tall it is ─────────────────────────────────────

/// Where a list's content is, and what the solve has measured of it.
///
/// A tracker's own getter answers with what was last set rather than with what the
/// compositor is evaluating, so the position reported in a [`SceneEvent`] is **the only
/// trustworthy read of one**. [`observe`] writes what it was told into here, and everything
/// above reads it as an ordinary signal — the realization window is a [`Memo`] over it.
///
/// The extent table rides here too, because the container and the band group it holds are
/// two nodes and one list: the container reports where the content is, the band group states
/// how tall it is, and a second handle would be a second answer to either.
#[derive(Copy, Clone, Debug)]
pub struct ListState {
    offset: Cell<f32>,
    /// Where inertia will rest, for as long as it is running.
    ///
    /// Held **beside** the offset and never in place of it: the destination is realized as
    /// soon as it is known, while the rows the offset still names stay realized too.
    target: Cell<Option<f32>>,
    /// The viewport's solved height.
    viewport: Cell<f32>,
    /// The extent the content is held at until the tracker goes idle, in row heights.
    ///
    /// Zero while it is idle. A measurement landing mid-interaction may lengthen the content
    /// and may never shorten it, so the maximum position climbs toward the truth and never
    /// steps back under a moving finger.
    held: Cell<f32>,
    /// The row the list realizes wherever the content stands, under the application's own
    /// identity for it.
    ///
    /// What keeps a focused row on the tree: an unrealized row has no node, so it has no hit
    /// entry, no focus ring and nothing for the focus order to land on.
    pinned: Cell<Option<u64>>,
    /// The row to bring into view, cleared by the flush that asks for it.
    reveal: Cell<Option<u64>>,
    /// How far the band group sits below the top of the content, in DIPs.
    ///
    /// The rows are placed in the group's own space and the tracker reports the content's, so
    /// this is what carries one into the other. Whatever the list is declared above — an
    /// inset, a heading, a command — is ordinary flow content and this is its height.
    lead: Cell<f32>,
    rows: Cell<Rows>,
}

/// Returns a list's state: at the origin, with nothing measured and nothing in flight.
#[must_use]
pub fn list_state() -> ListState {
    ListState {
        offset: Cell::new(0.0),
        target: Cell::new(None),
        viewport: Cell::new(0.0),
        held: Cell::new(0.0),
        pinned: Cell::new(None),
        reveal: Cell::new(None),
        lead: Cell::new(0.0),
        rows: Cell::new(Rows::default()),
    }
}

impl ListState {
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

    /// Holds the content's extent where it now stands, for as long as the tracker is moving.
    pub fn interacting(self) {
        if self.held.peek() <= 0.0 {
            self.held.set(self.rows.with(Rows::span));
        }
    }

    /// Clears the destination and the held extent once the tracker stops, leaving nothing
    /// ahead to realize and the measured extent free to shorten.
    pub fn settled(self) {
        self.target.set(None);
        self.held.set(0.0);
    }

    /// Keeps the row `key` names realized, or releases the one that was.
    pub fn pin(self, key: Option<u64>) {
        self.pinned.set(key);
    }

    /// Asks the tracker to bring the row `key` names into view.
    ///
    /// The one place a list moves the content rather than reading where it is, and it is a
    /// request rather than a write: the compositor owns the position, so the scroll is
    /// animated by it and the rows are realized from what it reports back.
    pub fn reveal(self, key: u64) {
        self.reveal.set(Some(key));
    }

    /// Returns where the content has to stand for the revealed row to be inside a viewport of
    /// `viewport_h`, taking the request.
    ///
    /// `None` where nothing was asked for, where the row has left the list, or where it is
    /// already in view — a reveal that asked for the position the content already has would
    /// interrupt whatever the user is doing to arrive where they are.
    pub(crate) fn take_reveal(self, viewport_h: f32) -> Option<f32> {
        let key = self.reveal.peek()?;
        self.reveal.set(None);
        let lead = self.lead.peek();
        let now = self.offset.peek();
        self.rows.with(|rows| {
            let at = rows.index_of(key)?;
            let top = lead + rows.offset(at);
            let bottom = top + rows.extent(at);
            if bottom > now + viewport_h {
                Some((bottom - viewport_h).max(0.0))
            } else if top < now {
                Some(top)
            } else {
                None
            }
        })
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

    /// Calls `f` with the extent table, registering a dependency for the reading effect or
    /// memo.
    ///
    /// What a surface drawn beside the list resolves against: it answers for every row,
    /// realized or not, where a realized row's own box answers only while it exists.
    pub fn with_rows<R>(self, f: impl FnOnce(&Rows) -> R) -> R {
        self.rows.with(f)
    }

    /// Returns the extent the content is laid out at, in row heights.
    pub(crate) fn extent(self) -> f32 {
        self.rows.with(Rows::span).max(self.held.get())
    }
}

// ── the extent table ─────────────────────────────────────────────────────────────

/// Where a list puts every row, whether it is realized or not.
///
/// **Offsets are counted in the list's own row height**, not in DIPs, so a placement is a
/// [`Len::Times`] of the metric the list was declared with and re-lowers with the type ramp
/// like any other length. [`Rows::unit`] carries what one of them measures, which is what
/// turns a count back into the DIPs a pointer arrives in.
///
/// A row is either **estimated** — [`ListSpec::estimate`], until the solve has reported a box
/// for it — or **measured**. A measurement is kept once taken, so a row scrolled out of the
/// window and back does not revert to the estimate and the offsets above the viewport stop
/// moving once they have been visited.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rows {
    /// One row height in DIPs, at the scope the band group was solved at. Zero until the
    /// first solve has reported one.
    unit: f32,
    /// The application's identity for each row, in list order.
    keys: Vec<u64>,
    /// Each row's extent, in row heights.
    extent: Vec<f32>,
    /// Whether that extent is the solve's or the estimate.
    measured: Vec<bool>,
    /// `prefix[i]` is row `i`'s offset and `prefix[len]` is the whole list's extent, both in
    /// row heights. One entry longer than [`Rows::keys`], so both answers are a lookup.
    prefix: Vec<f32>,
}

impl Rows {
    /// Returns how many rows the list has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns whether the list has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Returns what one row height measures, in DIPs, or zero before the first solve.
    #[must_use]
    pub fn unit(&self) -> f32 {
        self.unit
    }

    /// Returns the position `key` holds in the list, or `None` where it holds none.
    #[must_use]
    pub fn index_of(&self, key: u64) -> Option<usize> {
        self.keys.iter().position(|&k| k == key)
    }

    /// Returns the key at `index`, or `None` past the end of the list.
    #[must_use]
    pub fn key(&self, index: usize) -> Option<u64> {
        self.keys.get(index).copied()
    }

    /// Returns row `index`'s top edge in DIPs, measured from the band group's own corner.
    #[must_use]
    pub fn offset(&self, index: usize) -> f32 {
        self.prefix.get(index).copied().unwrap_or(0.0) * self.unit
    }

    /// Returns row `index`'s own height in DIPs.
    #[must_use]
    pub fn extent(&self, index: usize) -> f32 {
        self.extent.get(index).copied().unwrap_or(0.0) * self.unit
    }

    /// Returns the whole list's height in DIPs.
    #[must_use]
    pub fn total(&self) -> f32 {
        self.span() * self.unit
    }

    /// Returns whether row `index`'s extent is what the solve measured rather than the
    /// estimate.
    #[must_use]
    pub fn is_measured(&self, index: usize) -> bool {
        self.measured.get(index).copied().unwrap_or(false)
    }

    /// Returns the row the content position `y` falls in, clamped to the list.
    ///
    /// **Answers inside `0..len` at any `y`.** A tracker's position travels outside its
    /// bounds during a manipulation — the overpan is the bounce — so this is asked about
    /// positions past both ends of the content.
    #[must_use]
    pub fn at(&self, y: f32) -> usize {
        if self.keys.is_empty() || self.unit <= 0.0 {
            return 0;
        }
        let u = y / self.unit;
        // `prefix` ascends, so the row is the last one whose offset is at or before `u`, and
        // the search is over the offsets alone rather than over the whole table.
        self.prefix[..self.keys.len()]
            .partition_point(|&at| at <= u)
            .saturating_sub(1)
    }

    /// Returns the first row whose top edge is at or below `y`, which is one past the last
    /// row a span reaching `y` shows.
    ///
    /// At least one, so a viewport with no height still realizes the row under its top edge
    /// and the list has something to measure before it has a scale.
    fn past(&self, y: f32) -> usize {
        let count = self.keys.len();
        if count == 0 {
            return 0;
        }
        if self.unit <= 0.0 {
            return 1;
        }
        self.prefix[..count]
            .partition_point(|&at| at < y / self.unit)
            .max(1)
    }

    /// Returns the whole list's extent, in row heights.
    fn span(&self) -> f32 {
        self.prefix.last().copied().unwrap_or(0.0)
    }

    /// Returns row `index`'s offset in row heights, which is what a placement states.
    fn units(&self, index: usize) -> f32 {
        self.prefix.get(index).copied().unwrap_or(0.0)
    }

    /// Returns whether `keys` names a different list from the one held.
    fn stale(&self, keys: &[u64]) -> bool {
        self.keys != keys
    }

    /// Takes `keys` as the list, carrying each surviving key's measurement across.
    ///
    /// A key that was not in the list before is the estimate, and one that has left takes its
    /// measurement with it. Reordering is therefore free of re-measurement, which is what
    /// keeps a dragged row from changing height as it lands.
    fn sync(&mut self, keys: &[u64], estimate: f32) {
        let carried: Vec<(f32, bool)> = keys
            .iter()
            .map(|&key| {
                self.index_of(key)
                    .filter(|&at| self.measured[at])
                    .map_or((estimate, false), |at| (self.extent[at], true))
            })
            .collect();
        self.keys.clear();
        self.keys.extend_from_slice(keys);
        self.extent.clear();
        self.measured.clear();
        for (extent, measured) in carried {
            self.extent.push(extent);
            self.measured.push(measured);
        }
        self.rebuild();
    }

    /// Returns whether `table` reports a scale or an extent the held table does not have.
    fn differs(&self, table: &Table, unit: f32) -> bool {
        if self.unit != unit {
            return true;
        }
        table.iter().any(|entry| {
            self.index_of(entry.key).is_some_and(|at| {
                measured_units(entry.rect, unit).is_some_and(|extent| self.extent[at] != extent)
            })
        })
    }

    /// Writes every realized row's measured extent, and the scale they were measured at.
    ///
    /// A key the table reports and the list does not hold is dropped: the two are published
    /// by different passes of one flush, so a row can be measured on the solve that removed
    /// it from the document.
    fn absorb(&mut self, table: &Table, unit: f32) {
        self.unit = unit;
        for entry in table.iter() {
            let Some(at) = self.index_of(entry.key) else {
                continue;
            };
            let Some(extent) = measured_units(entry.rect, unit) else {
                continue;
            };
            self.extent[at] = extent;
            self.measured[at] = true;
        }
        self.rebuild();
    }

    /// Recomputes the offsets from the extents.
    ///
    /// One pass over the list and no allocation once the table has been sized: correcting a
    /// row's extent moves every row below it, and a table that answered by summing on demand
    /// would walk the list per row per placement.
    fn rebuild(&mut self) {
        let (extent, prefix) = (&self.extent, &mut self.prefix);
        prefix.clear();
        prefix.push(0.0);
        let mut at = 0.0;
        for &each in extent {
            at += each;
            prefix.push(at);
        }
    }
}

/// Returns `rect`'s height in row heights, or `None` where nothing has been measured.
///
/// A zero box is what a table reads before its node is solved, and a row of no height would
/// stack every row below it on the same line.
fn measured_units(rect: windows_scene::Rect, unit: f32) -> Option<f32> {
    (unit > 0.0 && rect.y1 > rect.y0).then(|| (rect.y1 - rect.y0) / unit)
}

// ── virtualization ───────────────────────────────────────────────────────────────

/// What a list needs to decide which rows exist.
///
/// **Variable extents.** A row's own content decides how tall it is, so a realized row is
/// what the solve measured and an unrealized one is [`ListSpec::estimate`]. Every extent is
/// counted in [`ListSpec::row_h`], which is therefore the list's unit as well as its guess.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ListSpec {
    /// The row height, as the palette's — so a list is as dense as the user asked for.
    pub row_h: Metric,
    /// What a row the solve has not measured is assumed to be, in `row_h`.
    pub estimate: f32,
    /// Rows realized beyond the viewport on each side. Two or three: enough that a row
    /// exists before it is looked at, few enough that a fling does not realize a screen it
    /// will never show.
    pub overscan: usize,
}

/// Rows realized beyond the viewport on each side unless a list states otherwise.
const OVERSCAN: usize = 3;

impl ListSpec {
    /// Returns a list whose rows are one [`Metric::RowH`](crate::role::Metric) until they are
    /// measured.
    ///
    /// The height is a metric rather than a length, so the list is as dense as the user asked
    /// for and re-lowers with the type ramp.
    #[must_use]
    pub const fn new(row_h: Metric) -> Self {
        Self {
            row_h,
            estimate: 1.0,
            overscan: OVERSCAN,
        }
    }

    /// Returns the same list guessing `estimate` row heights for a row it has not measured.
    ///
    /// Worth stating where the rows are known to be taller than the metric they are counted
    /// in: the guess is what the scrollbar reports until a row has been visited.
    #[must_use]
    pub const fn estimate(mut self, estimate: f32) -> Self {
        self.estimate = estimate;
        self
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

/// How many runs a realized set holds: the live window, the destination, the corridor
/// between them, and the pinned row.
const MAX_RUNS: usize = 3 + CORRIDOR;

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

/// Returns the rows worth realizing at `scroll_y`, plus `overscan` on each side.
///
/// **The range is inside `0..rows.len()` at any `scroll_y`.** A tracker's position travels
/// outside its bounds during a manipulation — the overpan is the bounce — so this is asked
/// about positions past the end of the content. An empty list answers `0..0`.
#[must_use]
pub fn window(scroll_y: f32, viewport_h: f32, rows: &Rows, overscan: usize) -> Range<usize> {
    let count = rows.len();
    if count == 0 {
        return 0..0;
    }
    // A position past the end answers the last row, which is what makes the overpan realize
    // the end of the list rather than nothing; the two clamps below are what keep the run
    // inside it.
    let first = rows.at(scroll_y).saturating_sub(overscan);
    let last = (rows.past(scroll_y + viewport_h.max(0.0)) + overscan).min(count);
    first..last.max(first)
}

/// Returns the whole realized set: where the content is, where a fling is taking it, a fixed
/// number of samples of the path between, and the row the list was told to keep.
///
/// The corridor is sampled rather than swept, so a fling crossing three thousand rows
/// realizes two windows and two overscan bands however far it travels. Allocates nothing.
#[must_use]
pub fn realize(
    offset: f32,
    target: Option<f32>,
    viewport_h: f32,
    pinned: Option<usize>,
    rows: &Rows,
    spec: &ListSpec,
) -> Realized {
    let mut out = Realized::EMPTY;
    out.push(window(offset, viewport_h, rows, spec.overscan));
    if let Some(target) = target {
        for step in 1..=CORRIDOR {
            let at = offset + (target - offset) * (step as f32) / ((CORRIDOR + 1) as f32);
            // Zero height, so a sample is the overscan band around a point rather than a
            // second viewport's worth of rows nobody will look at.
            out.push(window(at, 0.0, rows, spec.overscan));
        }
        out.push(window(target, viewport_h, rows, spec.overscan));
    }
    if let Some(pinned) = pinned.filter(|&at| at < rows.len()) {
        out.push(pinned..pinned + 1);
    }
    out.merge();
    out
}

/// Returns a scrolling container driving `state`, for a [`list`] and whatever else the
/// content carries.
///
/// The band group is one child among several, so a leading inset, a trailing command and the
/// figures drawn over the rows stay ordinary flow content of one scroller.
pub fn scroll_list<'a>(
    ui: &'a mut Ui<'_>,
    state: ListState,
    children: impl FnOnce(&mut Ui<'_>),
) -> Element<'a> {
    ui.scroll(
        ScrollDecl {
            reveal: Reveal::default(),
            state: Some(state),
        },
        children,
    )
}

/// Returns a virtualized band group: the rows near the viewport, each at its own measured
/// offset, inside a box as tall as the whole list.
///
/// Rows are **placed rather than laid out** — each is absolute at the offset the extent table
/// gives its key, and the group's own height is the whole list's — so the scroll extent does
/// not move when the realized window does and the realized set can be several disjoint runs
/// rather than one contiguous span.
///
/// `keys` names every row, in order, under the application's own identity; it is what the
/// list's length and every row's place come from. `items` supplies the data for the runs it
/// is handed, **in ascending index order**; an index it does not supply is not realized, and
/// the space the table gives that row stays open. Rows are reconciled by the same keyed
/// `each` as any other list, so a row surviving a move of the window keeps its node, its
/// owner and everything scoped to it.
///
/// Each realized row's box is measured and written back into the table, which corrects the
/// group's height progressively. The tracker's position is never written from here: a
/// correction moves the maximum position and leaves the content where the compositor has it.
pub fn list<'a, T: 'static, K: 'static>(
    ui: &'a mut Ui<'_>,
    state: ListState,
    spec: impl Fn() -> ListSpec + 'static,
    keys: impl Fn(&mut Vec<u64>) + 'static,
    items: impl Fn(&Realized, &mut Vec<(usize, T)>) + 'static,
    view: impl Fn(&mut Ui<'_>, &T) -> Node<K> + 'static,
) -> Element<'a> {
    let rows = state.rows;
    let spec = Memo::new(spec);
    let row_metric = Memo::new(move || spec.get().row_h);
    // The band group's own boxes, which is how a row's measured height comes back. One table
    // and one signal, so a row appearing wakes the measurement and a solve that moved nothing
    // wakes neither it nor anything derived from the extents.
    let boxes = anchors();
    // Where the band group sits inside the content, so the window is resolved in the group's
    // own space rather than the scroller's. A leading inset is ordinary flow content above it.
    let lead = probe();

    // The list's length and order. Read untracked and written only on a difference, so the
    // extents this effect owns cannot wake it.
    let named = RefCell::new(Vec::<u64>::new());
    Effect::new(move || {
        let estimate = spec.get().estimate;
        let mut next = named.borrow_mut();
        next.clear();
        keys(&mut next);
        if crate::signal::untracked(|| rows.with(|held| held.stale(&next))) {
            rows.update(|held| held.sync(&next, estimate));
        }
    });

    // Every realized row's measured height, at the scale the group was solved at, and how far
    // the group itself sits below the top of the content.
    Effect::new(move || {
        state.lead.set(lead.get().local.y);
        let metric = row_metric.get();
        let moved = boxes.with(|table| {
            table.scope.is_some_and(|scope| {
                let unit = crate::role::metric(metric, scope);
                crate::signal::untracked(|| rows.with(|held| held.differs(table, unit)))
            })
        });
        if moved {
            boxes.with(|table| {
                let unit = crate::role::metric(metric, table.scope.expect("a measured table"));
                rows.update(|held| held.absorb(table, unit));
            });
        }
    });

    let realized = Memo::new(move || {
        let spec = spec.get();
        let above = state.lead.get();
        rows.with(|held| {
            realize(
                state.offset() - above,
                state.target().map(|at| at - above),
                state.viewport(),
                state.pinned.get().and_then(|key| held.index_of(key)),
                held,
                &spec,
            )
        })
    });
    let supplied = RefCell::new(Vec::<(usize, T)>::new());
    ui.node(Preset::Bare)
        .no_shrink()
        .anchors_origin(boxes)
        .probed(lead)
        .layout_from(move |layout| {
            layout.height = Some(Len::Times(row_metric.get(), state.extent()));
        })
        .children(move |ui| {
            ui.each(
                move |out: &mut Vec<(u64, T)>| {
                    let realized = realized.get();
                    let mut supplied = supplied.borrow_mut();
                    supplied.clear();
                    items(&realized, &mut supplied);
                    rows.with(|held| {
                        for (index, item) in supplied.drain(..) {
                            if let Some(key) = held.key(index) {
                                out.push((key, item));
                            }
                        }
                    });
                },
                |(key, _)| key,
                move |ui, (key, item)| {
                    let key = *key;
                    let node = view(ui, item);
                    ui.effect(move |ui| {
                        let (metric, at) = (row_metric.get(), rows.with(|held| {
                            held.index_of(key).map_or(0.0, |index| held.units(index))
                        }));
                        if let Some(row) = ui.edit(node) {
                            row.anchored(boxes, key).layout(move |layout| {
                                layout.position = Some(super::Position::Band {
                                    at: Len::Times(metric, at),
                                });
                            });
                        }
                    });
                },
            );
        })
}

// ── the front thread's half ──────────────────────────────────────────────────────

/// One scroll container, as the app half needs it. Held by the host beside the mount that
/// owns the tracker, so a container unmounting takes its row with it.
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
    pub state: Option<ListState>,
    /// What was last published, so a solve that moved nothing emits nothing.
    pub last: ThumbGeom,
    /// Whether the half that moves the thumb has been told this container exists.
    pub front_added: bool,
}

// ── the table the thumb is moved from ────────────────────────────────────────────

/// One scroll container, as the half that moves its thumb holds it.
struct ScrollLive {
    front: ScrollFront,
    /// Where the app half asked the content to go, until the next tick asks the compositor.
    to: Option<f32>,
    /// Where the content stood when the current thumb grab began.
    grabbed_at: Option<f32>,
    /// What the thumb's opacity was last retargeted to, so an unchanged reason emits nothing.
    shown: bool,
}

/// Every scroll container, as the half that moves thumbs holds them.
///
/// Built from the edits the app half emits and never from its tables, so a reveal and a grab
/// cost a scan of a handful of rows and no hop.
#[derive(Default)]
pub(crate) struct ScrollTable {
    rows: Vec<ScrollLive>,
}

impl ScrollTable {
    pub(crate) fn reveal_field(
        &mut self,
        request: crate::text_input::Reveal,
        front: &mut Front<'_>,
    ) -> Result<()> {
        let Some(entry) = front.scene.hits().entry(request.id).copied() else {
            return Ok(());
        };
        let Some(row) = self
            .rows
            .iter()
            .find(|row| row.front.viewport == entry.scroll_src)
        else {
            return Ok(());
        };
        let Some(viewport) = front
            .scene
            .hits()
            .entries()
            .iter()
            .find(|h| Some(h.id) == row.front.control)
        else {
            return Ok(());
        };
        let top = viewport.y0;
        let bottom = viewport.y1;
        let bottom = request
            .occlusion
            .filter(|r| r.x0 < entry.x1 && entry.x0 < r.x1)
            .map_or(bottom, |r| bottom.min(r.y0));
        let Some(shadow) = front.scene.tracker_shadow(row.front.tracker) else {
            return Ok(());
        };
        let (x, y) = unpack_offset(shadow.load(Ordering::Acquire));
        let delta = if entry.y1 - y > bottom {
            entry.y1 - y - bottom
        } else if entry.y0 - y < top {
            entry.y0 - y - top
        } else {
            0.0
        };
        if delta != 0.0 {
            front.scene.request(
                row.front.tracker,
                TrackerRequest::To(Vector2 {
                    x,
                    y: (y + delta).max(0.0),
                }),
            )?;
        }
        Ok(())
    }

    /// Applies one batch of container edits, in the order the app half emitted them.
    ///
    /// A thumb declared [`Reveal::Always`] is opaque from its mount, so its row starts shown
    /// and the first edge that would show it again emits nothing.
    pub(crate) fn apply_ops(&mut self, ops: &mut Vec<ScrollOp>) {
        for op in ops.drain(..) {
            match op {
                ScrollOp::Add(front) => {
                    let shown = front.reveal == Reveal::Always;
                    self.rows.push(ScrollLive {
                        front,
                        to: None,
                        grabbed_at: None,
                        shown,
                    });
                }
                ScrollOp::Geom { id, geom } => {
                    if let Some(row) = self.rows.iter_mut().find(|row| row.front.id == id) {
                        row.front.last = geom;
                    }
                }
                ScrollOp::To { id, y } => {
                    if let Some(row) = self.rows.iter_mut().find(|row| row.front.id == id) {
                        row.to = Some(y);
                    }
                }
                ScrollOp::Drop(id) => self.rows.retain(|row| row.front.id != id),
            }
        }
    }

    /// Only a scroll with application-owned state needs its tracker reports upstream.
    /// Scene-side hit offsets, request completion and thumb reveal are already serviced.
    pub(crate) fn app_observes(&self, event: &SceneEvent) -> bool {
        let tracker = match *event {
            SceneEvent::TrackerValues { tracker, .. }
            | SceneEvent::TrackerPhase { tracker, .. }
            | SceneEvent::InertiaStarting { tracker, .. }
            | SceneEvent::RequestIgnored { tracker, .. } => tracker,
            _ => return true,
        };
        self.rows
            .iter()
            .find(|row| row.front.tracker.id() == tracker)
            .is_none_or(|row| row.front.observe)
    }

    /// Returns how many containers the table holds.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns the first container's thumb geometry, which is what a grab maps through.
    #[cfg(test)]
    fn geom_of_first(&self) -> Option<ThumbGeom> {
        self.rows.first().map(|row| row.front.last)
    }

    /// Runs `f` against the first container `pick` accepts.
    ///
    /// A linear scan rather than a map: a screen has a handful of scroll surfaces, and this
    /// is walked from every tracker report of every fling.
    fn where_(&mut self, pick: impl Fn(&ScrollFront) -> bool, f: impl FnOnce(&mut ScrollLive)) {
        if let Some(row) = self.rows.iter_mut().find(|row| pick(&row.front)) {
            f(row);
        }
    }
}

impl ScrollRow {
    /// Returns what the half that moves this container's thumb needs of it.
    ///
    /// `id` is the row's own, which is the name every later edit to this container carries.
    pub(crate) fn describe(&self, id: crate::build::ScrollId) -> ScrollFront {
        ScrollFront {
            viewport: self.viewport,
            id,
            tracker: self.tracker,
            thumb: self.thumb,
            control: self.control,
            grab: self.grab,
            reveal: self.reveal,
            last: self.last,
            observe: self.state.is_some(),
        }
    }
}

/// Records what the trackers reported into each container's [`ListState`].
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
                } => host.scroll_by_tracker(tracker, |row| {
                    if let Some(state) = row.state {
                        state.moved(position.y);
                    }
                }),
                SceneEvent::InertiaStarting {
                    tracker, modified, ..
                } => host.scroll_by_tracker(tracker, |row| {
                    if let Some(state) = row.state {
                        state.flinging_to(modified.y);
                    }
                }),
                SceneEvent::TrackerPhase { tracker, phase } => {
                    host.scroll_by_tracker(tracker, |row| {
                        if let Some(state) = row.state {
                            if phase == windows_scene::Phase::Idle {
                                state.settled();
                            } else {
                                state.interacting();
                            }
                        }
                    });
                }
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
pub(crate) fn front(
    events: &[SceneEvent],
    reports: &[Report],
    table: &mut ScrollTable,
    front: &mut Front<'_>,
) -> Result<()> {
    if events.is_empty() && reports.is_empty() && table.rows.iter().all(|row| row.to.is_none()) {
        return Ok(());
    }
    // The first failure is kept and the rest of the tick still runs: a refused retarget on
    // one surface must not leave another's grab half-applied.
    let mut failed: Option<windows_core::Error> = None;
    // Taken before the events, so a reveal and a phase edge in one tick leave the content
    // where the reveal asked rather than where it stood.
    for row in &mut table.rows {
        if let Some(y) = row.to.take() {
            if let Err(error) = front
                .scene
                .request(row.front.tracker, TrackerRequest::To(Vector2 { x: 0.0, y }))
                .map(|_| ())
            {
                failed.get_or_insert(error);
            }
        }
    }
    for event in events {
        let (tracker, moving) = match *event {
            SceneEvent::TrackerPhase { tracker, phase } => {
                (tracker, phase != windows_scene::Phase::Idle)
            }
            _ => continue,
        };
        table.where_(
            |row| row.tracker.id() == tracker,
            |row| {
                if let Err(error) = reveal(row, moving, front) {
                    failed.get_or_insert(error);
                }
            },
        );
    }
    for report in reports {
        match *report {
            Report::Redirect { target, pointer } => table.where_(
                |row| row.control == Some(target),
                |row| {
                    if let Err(error) = front
                        .scene
                        .redirect_manipulation(row.front.tracker, &pointer)
                    {
                        failed.get_or_insert(error);
                    }
                },
            ),
            Report::HoverChanged { from, to, .. } => {
                for (id, over) in [(from, false), (to, true)] {
                    let Some(id) = id else { continue };
                    table.where_(
                        |row| row.control == Some(id),
                        |row| {
                            if let Err(error) = reveal(row, over, front) {
                                failed.get_or_insert(error);
                            }
                        },
                    );
                }
            }
            Report::Dragged { target, update, .. } => table.where_(
                |row| row.grab == Some(target),
                |row| {
                    if let Err(error) = drag(row, update.phase, update.delta.y, front) {
                        failed.get_or_insert(error);
                    }
                },
            ),
            // A released or cancelled grab forgets where it started, so the next one
            // measures from where the content actually is.
            Report::Released { target, .. } | Report::Canceled { target, .. } => {
                table.where_(|row| row.grab == Some(target), |row| row.grabbed_at = None);
            }
            _ => {}
        }
    }
    failed.map_or(Ok(()), Err)
}

/// Shows or conceals a thumb.
///
/// Only [`Reveal::OnDemand`] retargets, and only on an edge: `Always` is opaque from the
/// mount and `Never` has no thumb, so neither reaches the compositor here.
fn reveal(row: &mut ScrollLive, show: bool, front: &mut Front<'_>) -> Result<()> {
    let Some(thumb) = row.front.thumb else {
        return Ok(());
    };
    // A grabbed thumb stays lit however the pointer wanders, and a moving one stays lit
    // whatever the pointer is doing.
    let show = show || row.grabbed_at.is_some();
    if row.front.reveal != Reveal::OnDemand || show == row.shown {
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
///
/// The origin is read from the tracker's published word rather than from the container's
/// signal, which is the app half's. A tracker with no word is one the compositor has not
/// created, and it has nothing to be dragged from.
fn drag(row: &mut ScrollLive, phase: DragPhase, dy: f32, front: &mut Front<'_>) -> Result<()> {
    if phase == DragPhase::Undecided {
        return Ok(());
    }
    let from = if let Some(from) = row.grabbed_at {
        from
    } else {
        let Some(shadow) = front.scene.tracker_shadow(row.front.tracker) else {
            return Ok(());
        };
        // acquire: pairs with the release store the scene makes when it records a reported
        // position, so both halves of the word read here are that one report's.
        let (_, y) = unpack_offset(shadow.load(Ordering::Acquire));
        *row.grabbed_at.insert(y)
    };
    let thumb_y = thumb_y_for_scroll(from, row.front.last) + dy;
    let to = scroll_for_thumb_y(thumb_y, row.front.last);
    front
        .scene
        .request(
            row.front.tracker,
            TrackerRequest::To(Vector2 { x: 0.0, y: to }),
        )
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
    use crate::build::tests::fixture;
    use crate::driver::testing::LayoutDriver;
    use crate::layout::Len;

    #[test]
    fn a_scroll_replaced_before_its_first_solve_never_creates_a_retired_tracker() {
        let mut patch = fixture();
        let mount = || LayoutDriver::create(|ui| {
            scroll(ui, |ui| { ui.node(Preset::Bare).height(Len::Times(Metric::RowH, 200.0)); });
        });
        let retired = mount();
        drop(retired);
        let mut live = mount();
        live.flush(&mut patch);
        assert_eq!(patch.ops().iter().filter(|op| matches!(op,
            windows_scene::Op::Tracker { op: windows_scene::TrackerOp::Create { .. }, .. }
        )).count(), 1, "only the surviving viewport may create a tracker");
    }

    #[test]
    fn only_virtual_lists_forward_tracker_reports_to_the_app() {
        let mut patch = fixture();
        let mut down = crate::seam::Down::default();
        let mut table = ScrollTable::default();
        let ordinary = LayoutDriver::create(|ui| {
            scroll(ui, |ui| {
                ui.node(Preset::Bare)
                    .height(Len::Times(Metric::RowH, 200.0));
            });
        });
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        table.apply_ops(&mut down.scrolls);
        let tracker = table.rows[0].front.tracker.id();
        let events = [
            SceneEvent::TrackerValues {
                tracker,
                position: Vector2 { x: 0.0, y: 100.0 },
                scale: 1.0,
            },
            SceneEvent::TrackerPhase {
                tracker,
                phase: windows_scene::Phase::Idle,
            },
            SceneEvent::InertiaStarting {
                tracker,
                natural: Vector2::default(),
                modified: Vector2::default(),
                from_impulse: false,
            },
            SceneEvent::RequestIgnored {
                tracker,
                request: 1,
            },
        ];
        assert!(events.iter().all(|e| !table.app_observes(e)));
        assert!(table.app_observes(&SceneEvent::DeviceRebuilt));
        drop(ordinary);
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        table.apply_ops(&mut down.scrolls);
        assert!(
            events.iter().all(|e| table.app_observes(e)),
            "unknown trackers remain available to other consumers"
        );
        let _list = LayoutDriver::create(|ui| {
            let state = list_state();
            scroll_list(ui, state, move |ui| {
                list(
                    ui,
                    state,
                    || SPEC,
                    |out| out.extend(0..100u64),
                    |runs, items| {
                        for run in runs.runs() {
                            for i in run {
                                items.push((i, i));
                            }
                        }
                    },
                    |ui, _| ui.node(Preset::Bare).id(),
                );
            });
        });
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        table.apply_ops(&mut down.scrolls);
        let tracker = table.rows[0].front.tracker.id();
        let moved = SceneEvent::TrackerValues {
            tracker,
            position: Vector2 { x: 0.0, y: 400.0 },
            scale: 1.0,
        };
        assert!(table.app_observes(&moved));
        observe(&[moved]);
        Host::with(|h| {
            h.scroll_by_tracker(tracker, |row| {
                assert_eq!(
                    row.state
                        .expect("virtualization owns scroll state")
                        .offset
                        .get(),
                    400.0
                );
            })
        });
    }

    /// A container reaches the table that moves its thumb once, and leaves it when it
    /// unmounts.
    ///
    /// The add is emitted before the geometry gate, so a container whose content fits is in
    /// the table too: its thumb still has a reveal, and a row that never arrived would leave
    /// every hover over that surface acting on nothing.
    #[test]
    fn a_container_is_added_once_and_dropped_when_it_unmounts() {
        let mut patch = fixture();
        let mut down = crate::seam::Down::default();
        let mut table = ScrollTable::default();

        let held = LayoutDriver::create(|ui| {
            scroll(ui, |ui| {
                ui.node(Preset::Bare)
                    .height(Len::Times(Metric::RowH, 200.0));
            });
        });
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        assert!(
            down.scrolls
                .iter()
                .filter(|op| matches!(op, ScrollOp::Add(_)))
                .count()
                == 1,
            "a container was added other than once"
        );
        table.apply_ops(&mut down.scrolls);
        assert_eq!(table.len(), 1);

        Host::flush(&mut patch);

        Host::with(|h| {
            h.fill(&mut down);
        });
        table.apply_ops(&mut down.scrolls);
        assert_eq!(table.len(), 1, "a settled container was added twice");

        drop(held);
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        table.apply_ops(&mut down.scrolls);
        assert_eq!(table.len(), 0, "the unmounted container kept its row");
    }

    /// A container whose extents moved emits the new thumb geometry, and the table takes it.
    #[test]
    fn a_moved_extent_emits_the_geometry_the_table_grabs_against() {
        let mut patch = fixture();
        let mut down = crate::seam::Down::default();
        let mut table = ScrollTable::default();

        let _held = LayoutDriver::create(|ui| {
            scroll(ui, |ui| {
                ui.node(Preset::Bare)
                    .height(Len::Times(Metric::RowH, 200.0));
            });
        });
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        table.apply_ops(&mut down.scrolls);
        let first = table.geom_of_first().expect("one container");
        assert!(
            first.overflow,
            "4000 DIP of content in a window that is not"
        );

        Host::with(|h| h.set_window(Vector2 { x: 400.0, y: 200.0 }));
        Host::flush(&mut patch);
        Host::with(|h| {
            h.fill(&mut down);
        });
        assert!(
            down.scrolls
                .iter()
                .any(|op| matches!(op, ScrollOp::Geom { .. })),
            "a shorter viewport moved no thumb geometry"
        );
        table.apply_ops(&mut down.scrolls);
        assert_ne!(
            table.geom_of_first().expect("one container"),
            first,
            "the table kept the geometry the old viewport gave it"
        );
    }

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

    /// Returns a hundred-row table, every row `extent` row heights tall and measured.
    fn uniform(extent: f32) -> Rows {
        table(&(0..100).map(|_| extent).collect::<Vec<_>>())
    }

    /// Returns a table of the extents given, in row heights, at a 20-DIP row.
    fn table(extents: &[f32]) -> Rows {
        let mut rows = Rows {
            unit: 20.0,
            ..Rows::default()
        };
        rows.sync(&(0..extents.len() as u64).collect::<Vec<_>>(), 1.0);
        rows.extent.clear();
        rows.extent.extend_from_slice(extents);
        rows.measured.iter_mut().for_each(|at| *at = true);
        rows.rebuild();
        rows
    }

    /// Offsets accumulate the extents ahead of each row, and the search inverts them.
    ///
    /// Answered from the prefix table rather than by summing, so a list of mixed extents
    /// costs one lookup per row and one search per position however long it is.
    #[test]
    fn a_mixed_table_answers_both_directions_without_a_scan() {
        let rows = table(&[1.0, 3.0, 0.5, 2.0]);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows.offset(0), 0.0);
        assert_eq!(rows.offset(1), 20.0);
        assert_eq!(rows.offset(2), 80.0);
        assert_eq!(rows.offset(3), 90.0);
        assert_eq!(rows.total(), 130.0);
        assert_eq!(rows.extent(1), 60.0);
        // Each row's own band answers itself at both edges, and the boundary belongs to the
        // row it opens.
        for (index, y) in [(0, 0.0), (0, 19.0), (1, 20.0), (1, 79.0), (2, 80.0), (3, 90.0)] {
            assert_eq!(rows.at(y), index, "position {y}");
        }
        assert_eq!(rows.at(-50.0), 0, "an overpan above the list answers its first row");
        assert_eq!(rows.at(400.0), 3, "an overpan below it answers its last");
    }

    /// A measurement replaces the estimate, moves every row below it, and stays taken.
    #[test]
    fn a_measurement_moves_the_rows_below_it_and_is_kept() {
        let mut rows = table(&[1.0, 1.0, 1.0]);
        rows.measured.iter_mut().for_each(|at| *at = false);
        let mut measured = Table::default();
        measured.boxes.push(crate::layout::Anchored {
            key: 1,
            rect: windows_scene::Rect::new(0.0, 20.0, 100.0, 80.0),
        });
        assert!(rows.differs(&measured, 20.0));
        rows.absorb(&measured, 20.0);
        assert!(!rows.differs(&measured, 20.0), "the measurement did not land");
        assert!(rows.is_measured(1) && !rows.is_measured(0));
        assert_eq!(rows.offset(2), 80.0, "the row below kept the estimate's offset");
        assert_eq!(rows.total(), 100.0);
        // A key the table reports and the list does not hold is not an index into it.
        let mut stray = Table::default();
        stray.boxes.push(crate::layout::Anchored {
            key: 99,
            rect: windows_scene::Rect::new(0.0, 0.0, 100.0, 40.0),
        });
        assert!(!rows.differs(&stray, 20.0));
    }

    /// A row that survives a reorder carries its measurement to its new place, and a row that
    /// arrives is the estimate.
    #[test]
    fn a_reorder_carries_each_measurement_with_its_row() {
        let mut rows = table(&[1.0, 3.0]);
        rows.sync(&[1, 7, 0], 2.0);
        assert!(rows.is_measured(0) && rows.is_measured(2));
        assert!(!rows.is_measured(1), "a row that was not in the list was measured");
        assert_eq!(rows.extent(0), 60.0, "the tall row lost its measurement");
        assert_eq!(rows.extent(1), 40.0, "the new row is not the estimate");
        assert_eq!(rows.extent(2), 20.0);
        assert_eq!(rows.total(), 120.0);
    }

    const SPEC: ListSpec = ListSpec {
        row_h: Metric::RowH,
        estimate: 1.0,
        overscan: 2,
    };

    /// The window covers the viewport, plus the overscan on each side, and never runs past
    /// the ends.
    #[test]
    fn the_realization_window_covers_the_viewport_and_is_clamped_at_both_ends() {
        let rows = uniform(1.0);
        let top = window(0.0, 100.0, &rows, SPEC.overscan);
        assert_eq!(top.start, 0, "the overscan cannot go negative");
        assert!(top.end >= 5 && top.end <= 8);

        let middle = window(400.0, 100.0, &rows, SPEC.overscan);
        assert_eq!(middle.start, 18, "twenty rows in, less two of overscan");
        assert_eq!(middle.end, 27, "five visible, plus two either side");

        let bottom = window(1900.0, 100.0, &rows, SPEC.overscan);
        assert_eq!(bottom.end, 100, "the overscan cannot go past the last row");
    }

    /// The window follows the extents rather than an index, so a tall row shows fewer.
    #[test]
    fn the_window_over_mixed_extents_follows_the_measurements() {
        // Three shut rows, one five deep, then shut rows again: 20, 20, 20, 100, 20…
        let mut extents = vec![1.0; 100];
        extents[3] = 5.0;
        let rows = table(&extents);
        // A viewport of 100 DIPs opening at the tall row shows that row and one more.
        let over = window(60.0, 100.0, &rows, 0);
        assert_eq!(over, 3..4, "the open row did not take the viewport");
        // The same viewport above it shows four.
        assert_eq!(window(0.0, 100.0, &rows, 0), 0..4);
        // And below it, where the rows are shut again, five.
        assert_eq!(window(260.0, 100.0, &rows, 0), 9..14);
    }

    /// A tracker overpans past the end of the content, and the window stays inside the list.
    ///
    /// The overpan is the bounce, so this position is reached by ordinary use. A window
    /// running past the last row underflows a row count: a debug panic, and in release a
    /// placement far past the end of the list.
    #[test]
    fn an_overpanned_window_stays_inside_the_list() {
        let rows = uniform(1.0);
        let bounced = window(2100.0, 100.0, &rows, SPEC.overscan);
        assert!(bounced.start <= rows.len() && bounced.end <= rows.len());
        assert_eq!(bounced, 97..100, "the overpan realizes the end of the list");
    }

    /// An empty list realizes nothing, and an unmeasured scale does not divide by itself.
    #[test]
    fn a_degenerate_list_realizes_nothing() {
        let empty = Rows::default();
        assert!(window(0.0, 100.0, &empty, 2).is_empty());
        assert_eq!(realize(0.0, None, 100.0, None, &empty, &SPEC).rows(), 0);
        let unsolved = Rows {
            unit: 0.0,
            ..table(&[1.0; 10])
        };
        assert_eq!(unsolved.at(500.0), 0, "an unmeasured scale answers the first row");
    }

    /// At rest the realized set is exactly the live window: one run, no corridor, nothing
    /// realized ahead of a fling that is not happening.
    #[test]
    fn a_resting_list_realizes_one_run() {
        let at_rest = realize(400.0, None, 100.0, None, &uniform(1.0), &SPEC);
        assert_eq!(at_rest.runs().count(), 1);
        assert_eq!(at_rest.runs().next().unwrap(), 18..27);
    }

    /// A fling realizes where it lands as well as where it is, and the two are disjoint,
    /// which is why the set is not one range.
    #[test]
    fn a_long_fling_realizes_its_destination_and_a_bounded_corridor() {
        let flung = realize(0.0, Some(1900.0), 100.0, None, &uniform(1.0), &SPEC);
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
        let nudged = realize(400.0, Some(440.0), 100.0, None, &uniform(1.0), &SPEC);
        assert_eq!(nudged.runs().count(), 1);
        let run = nudged.runs().next().unwrap();
        assert_eq!(run, 18..29);
    }

    /// A pinned row is realized wherever the content stands, and is its own run.
    ///
    /// What keeps a focused row on the tree: an unrealized row has no node, so it has
    /// nothing for the focus order to land on and no ring to draw.
    #[test]
    fn a_pinned_row_is_realized_from_anywhere() {
        let pinned = realize(1900.0, None, 100.0, Some(2), &uniform(1.0), &SPEC);
        assert!(pinned.contains(2), "the pinned row was left unrealized");
        assert!(pinned.contains(99), "the live window was dropped for it");
        assert_eq!(pinned.runs().count(), 2);
        let past = realize(0.0, None, 100.0, Some(500), &uniform(1.0), &SPEC);
        assert_eq!(past.runs().count(), 1, "a pin past the end realized a row that is not there");
    }

    /// The runs come out ascending and disjoint however they went in, because the fill walks
    /// them in order and a supplied item is matched by a single forward scan.
    #[test]
    fn the_runs_are_ascending_and_disjoint() {
        let flung = realize(1900.0, Some(0.0), 100.0, Some(50), &uniform(1.0), &SPEC);
        let mut last = 0;
        for run in flung.runs() {
            assert!(run.start >= last, "{run:?} after {last}");
            assert!(run.end > run.start);
            last = run.end;
        }
    }
}
