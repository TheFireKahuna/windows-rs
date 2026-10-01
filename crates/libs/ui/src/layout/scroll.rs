//! Scroll and virtualization: the two policies that sit over the tracker.
//!
//! **Scroll is tracker-delegated, always.** The viewport does not move; it clips. The
//! content's offset and the thumb's offset are both bound to the one tracker, so the thumb
//! follows the content with no per-frame front-thread work.
//!
//! Thumb geometry and the realization window live here rather than in `windows-scene`
//! because both are shaped by the widget that consumes them. `windows-scene` supplies the
//! tracker and the binding.

use crate::GestureSettings;
use crate::build::control::ControlRow;
use crate::build::{Element, Host, Node, Ui};
use crate::gesture::{DragDecl, DragUpdate, GestureDecl, Phase};
use crate::bindings::{VK_DOWN, VK_END, VK_HOME, VK_NEXT, VK_PRIOR, VK_UP};
use crate::input::{KeyEvent, KeyKind, Report};
use crate::layout::{Edge, Layout, Len, Position, Preset, Rect, Table, anchors, probe};
use crate::role::{Metric, metric};
use crate::seam::{ScrollFront, ScrollOp};
use crate::signal::{Cell, Effect, Memo};
use crate::widget::Front;
use core::cell::RefCell;
use core::ops::Range;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{
    Affine, Anim, Axes, Bind, ControlId, GroupId, HitDecl, HitFlags, HitTable, Id, NodeId, Observed,
    Phase as TrackerPhase, Prop, SceneEvent, SpriteId, TRACKER, TrackerAxis, TrackerId,
    TrackerRequest, Tuning, Value, unpack_offset,
};

// ── the thumb ────────────────────────────────────────────────────────────────────

/// When the thumb is visible.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Reveal {
    /// While the content is moving, while the pointer is over the surface, and for a moment
    /// after either ends.
    #[default]
    OnDemand,
    /// Always, taking room from the content.
    Always,
    /// Never — for a surface whose extent is obvious from what is in it.
    Never,
}

/// How wide the thumb is.
pub const THUMB_W: f32 = 6.0;
/// How far it is inset from the right edge, and from each end of its travel.
pub const THUMB_MARGIN: f32 = 2.0;
/// The thumb's minimum height, however long the content: a thumb that shrinks to nothing
/// cannot be grabbed.
pub const THUMB_MIN_H: f32 = 24.0;
/// How far past the thumb a pointer still counts as over it. The drawn bar is 6 DIP, under
/// the system's minimum target, so the hit entry is inflated rather than the bar widened.
const GRAB_INFLATE: f32 = 8.0;
/// How much content must be out of view before the rail is worth arming, in DIPs.
///
/// Sub-pixel overflow is a rounding residue of the solve rather than something to scroll.
const OVERFLOW_FLOOR: f32 = 0.5;
/// Fraction of a viewport used by arrow keys and accessibility small scroll steps.
pub(crate) const SMALL_STEP: f32 = 0.1;
/// How long a thumb stays lit after the last reason to show it ends, in milliseconds.
///
/// The fade carries it as its own delay, so the compositor measures the wait and a reason
/// arriving inside it replaces the fade rather than racing it.
const CONCEAL_MS: u32 = 700;

/// What a scrollbar is, at one pair of extents.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ThumbGeom {
    /// How far the content can move.
    pub max_scroll: f32,
    /// How tall the thumb is.
    pub thumb_h: f32,
    /// How far the thumb itself travels, which is not how far the content does.
    pub travel: f32,
}

impl ThumbGeom {
    /// What a container the half that moves thumbs has not been told about holds.
    ///
    /// `NaN` compares unequal to every geometry including itself, so the first publication is
    /// emitted without a flag recording that none has happened.
    pub const UNSENT: Self = Self {
        max_scroll: f32::NAN,
        thumb_h: 0.0,
        travel: 0.0,
    };

    /// Returns whether there is anything to scroll and room to show a thumb in.
    ///
    /// Derived rather than stored, so the three numbers cannot disagree across the seam.
    #[must_use]
    pub fn overflow(self) -> bool {
        self.max_scroll > OVERFLOW_FLOOR && self.travel > 0.0
    }

    /// Returns how big the thumb is drawn.
    #[must_use]
    pub fn size(self) -> Vector2 {
        Vector2 {
            x: THUMB_W,
            y: self.thumb_h,
        }
    }

    /// Returns where the thumb rests in a viewport `view_w` DIPs wide, at the content's origin.
    #[must_use]
    pub fn offset(self, view_w: f32) -> Vector2 {
        Vector2 {
            x: view_w - THUMB_W - THUMB_MARGIN,
            y: THUMB_MARGIN,
        }
    }

    /// Returns the map from the tracker's position to the thumb's own offset.
    ///
    /// [`thumb_y_for_scroll`] written as the affine the compositor evaluates, so the thumb the
    /// user grabs and the value the binding renders are one function.
    #[must_use]
    pub fn affine(self) -> Affine {
        Affine {
            m: if self.max_scroll > 0.0 {
                self.travel / self.max_scroll
            } else {
                0.0
            },
            c: THUMB_MARGIN,
        }
    }
}

/// Returns the scrollbar geometry for a viewport of `viewport_h` showing `content_h` of
/// content.
#[must_use]
pub fn thumb_geom(viewport_h: f32, content_h: f32) -> ThumbGeom {
    let max_scroll = (content_h - viewport_h).max(0.0);
    let track_h = (viewport_h - 2.0 * THUMB_MARGIN).max(0.0);
    // Proportional, then floored at THUMB_MIN_H and capped at the track: a very long document
    // keeps a grabbable thumb, and the travel subtracts the floored height rather than the
    // proportional one.
    let (thumb_h, travel) = if max_scroll > 0.0 && track_h > 0.0 {
        let ratio = (viewport_h / content_h).clamp(0.0, 1.0);
        let thumb_h = (track_h * ratio).max(THUMB_MIN_H).min(track_h);
        (thumb_h, track_h - thumb_h)
    } else {
        (0.0, 0.0)
    };
    ThumbGeom {
        max_scroll,
        thumb_h,
        travel,
    }
}

/// Returns where the thumb sits when the content is at `scroll`.
///
/// The same affine map the compositor evaluates from the tracker, so a grab starts from the
/// value the binding is rendering.
#[must_use]
pub fn thumb_y_for_scroll(scroll: f32, geom: ThumbGeom) -> f32 {
    let frac = if geom.max_scroll > 0.0 {
        (scroll / geom.max_scroll).clamp(0.0, 1.0)
    } else {
        0.0
    };
    THUMB_MARGIN + frac * geom.travel
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

/// The rail: a strip down the right edge of the viewport, full height.
///
/// **The rail is the grab target, and the thumb is not.** The compositor moves the thumb, so
/// its layout rect stays where the solve put it however far the content has travelled, and a
/// hit entry on it would stop being under the drawn bar. The rail is static geometry, and
/// where inside it a press landed is answered from the reported position.
///
/// Pinned rather than scrolled: it lives inside the container it reports on and does not move
/// with it.
pub const RAIL: Layout = Layout {
    position: Position::Pin(Edge::Right),
    width: Len::dip(THUMB_W + 2.0 * THUMB_MARGIN),
    height: Len::pct(1.0),
    ..Layout::DEFAULTS[Preset::Layer as usize]
};

/// Returns the rail's hit entry: interactive, unscrolled, and inflated for touch.
pub(crate) fn grab_hit(control: ControlId) -> HitDecl {
    HitDecl {
        // Pinned: the rail lives inside the container it reports on and does not move with it,
        // so its rect must not resolve through that container's offset.
        flags: HitFlags::INTERACTIVE.union(HitFlags::UNSCROLLED),
        id: control,
        touch_inflate: Some(GRAB_INFLATE),
    }
}

/// Returns the rail's gesture declaration, so a pointer can grab the bar in it.
///
/// A hit entry and a drag, with no wash and no chrome row: the thumb's opacity is retargeted
/// from the front half, and a control the front table adopted would give that channel two
/// owners.
pub(crate) fn grab_decl() -> GestureDecl {
    GestureDecl {
        settings: GestureSettings::None,
        drag: Some(DragDecl {
            horizontal: false,
            // Below the default: a scrollbar is aimed at, so the grab should follow the first
            // pixel rather than absorb six of them.
            threshold: 1.0,
            // The content tracks the thumb rather than landing when it is let go.
            live: true,
            ..DragDecl::default()
        }),
        ..GestureDecl::default()
    }
}

// ── where the content is, and how tall it is ─────────────────────────────────────

/// Where a list's content stands and what it has been asked for, as one value.
///
/// One signal rather than seven: every field but [`Pos::band_y`] is an input to the
/// realization window, so a write to any of them has to wake it, and the window's own equality
/// gate stops the one that does not.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Pos {
    /// The content offset the tracker last reported.
    pub offset: f32,
    /// The viewport's solved height.
    pub viewport: f32,
    /// How far the band group sits below the top of the content, in DIPs.
    ///
    /// The rows are placed in the group's own space and the tracker reports the content's, so
    /// this is what carries one into the other.
    pub band_y: f32,
    /// Where inertia will rest, or `None` while nothing is in flight.
    ///
    /// An `Option` rather than a `NaN` sentinel: this value is the signal's own change gate, and
    /// `NaN != NaN` would make every write look like a move, so an unchanged list would bump the
    /// cell from inside `Host::flush` and the waker that bump raises would run the app pass
    /// again rather than letting it park.
    ///
    /// Held **beside** the offset and never in place of it: the destination is realized as
    /// soon as it is known, while the rows the offset still names stay realized too.
    pub target: Option<f32>,
    /// The extent the content is held at until the tracker goes idle, in row heights.
    ///
    /// Zero while it is idle. A measurement landing mid-interaction may lengthen the content
    /// and may never shorten it, so the maximum position climbs toward the truth and never
    /// steps back under a moving finger.
    pub held: f32,
    /// The row the list realizes wherever the content stands, under the application's own
    /// identity for it.
    ///
    /// What keeps a focused row on the tree: an unrealized row has no node, so it has no hit
    /// entry, no focus ring and nothing for the focus order to land on.
    pub pin: Option<u64>,
    /// The row to bring into view, cleared by the flush that asks for it.
    pub reveal: Option<u64>,
}

impl Pos {
    /// Returns where inertia will rest, or `None` when nothing is in flight.
    #[must_use]
    pub fn target(self) -> Option<f32> {
        self.target
    }
}

/// Where a list's content is, and what the solve has measured of it.
///
/// A tracker's own getter answers with what was last set rather than with what the compositor
/// is evaluating, so the position reported in a [`SceneEvent`] is **the only trustworthy read
/// of one**. [`observe`] writes what it was told into here, and everything above reads it as
/// an ordinary signal — the realization window is a [`Memo`] over it.
///
/// The extent table rides here too, because the container and the band group it holds are two
/// nodes and one list: the container reports where the content is, the band group states how
/// tall it is, and a second handle would be a second answer to either.
#[derive(Copy, Clone, Debug)]
pub struct ListState {
    pos: Cell<Pos>,
    rows: Cell<Rows>,
}

/// Returns a list's state: at the origin, with nothing measured and nothing in flight.
#[must_use]
pub fn list_state() -> ListState {
    ListState {
        pos: Cell::new(Pos::default()),
        rows: Cell::new(Rows::default()),
    }
}

impl ListState {
    /// Returns where the content stands and what it has been asked for.
    #[must_use]
    pub fn pos(self) -> Pos {
        self.pos.get()
    }

    /// Returns the content offset the tracker last reported.
    #[must_use]
    pub fn offset(self) -> f32 {
        self.pos.get().offset
    }

    /// Returns the viewport's height.
    #[must_use]
    pub fn viewport(self) -> f32 {
        self.pos.get().viewport
    }

    /// Records the viewport's own height, from the solved layout.
    pub fn resized(self, height: f32) {
        self.edit(|at| at.viewport = height);
    }

    /// Records how far the band group sits below the top of the content.
    pub fn banded(self, y: f32) {
        self.edit(|at| at.band_y = y);
    }

    /// Keeps the row `key` names realized, or releases the one that was.
    pub fn pin(self, key: Option<u64>) {
        self.edit(|at| at.pin = key);
    }

    /// Asks the tracker to bring the row `key` names into view.
    ///
    /// The one place a list moves the content rather than reading where it is, and it is a
    /// request rather than a write: the compositor owns the position, so the scroll is
    /// animated by it and the rows are realized from what it reports back.
    pub fn reveal(self, key: u64) {
        self.edit(|at| at.reveal = Some(key));
    }

    /// Returns where the content has to stand for the revealed row to be inside a viewport of
    /// `viewport_h`, taking the request.
    ///
    /// `None` where nothing was asked for, where the row has left the list, or where it is
    /// already in view — a reveal that asked for the position the content already has would
    /// interrupt whatever the user is doing to arrive where they are.
    pub fn take_reveal(self, viewport_h: f32) -> Option<f32> {
        let at = self.pos.peek();
        let key = at.reveal?;
        self.edit(|held| held.reveal = None);
        self.rows.with(|rows| {
            let index = rows.index_of(key)?;
            let top = at.band_y + rows.offset(index);
            let bottom = top + rows.extent(index);
            if bottom > at.offset + viewport_h {
                Some((bottom - viewport_h).max(0.0))
            } else if top < at.offset {
                Some(top)
            } else {
                None
            }
        })
    }

    /// Returns the extent the content is laid out at, in row heights.
    ///
    /// The held extent wins while a manipulation is live, so a measurement landing under a
    /// moving finger lengthens the content and never shortens it.
    #[must_use]
    pub fn extent(self) -> f32 {
        self.rows.with(Rows::total_units).max(self.pos.get().held)
    }

    /// Calls `f` with the extent table, registering a dependency for the reading effect or
    /// memo.
    ///
    /// What a surface drawn beside the list resolves against: it answers for every row,
    /// realized or not, where a realized row's own box answers only while it exists.
    pub fn with_rows<R>(self, f: impl FnOnce(&Rows) -> R) -> R {
        self.rows.with(f)
    }

    /// Applies one change to the position.
    ///
    /// The one writer of [`Pos`], so a transition that has to move two fields together wakes
    /// the realization window once.
    pub(crate) fn edit(self, f: impl FnOnce(&mut Pos)) {
        let mut at = self.pos.peek();
        f(&mut at);
        self.pos.set(at);
    }
}

// ── the extent table ─────────────────────────────────────────────────────────────

/// Where a list puts every row, whether it is realized or not.
///
/// **Offsets are counted in the list's own row height**, not in DIPs, so a placement is a
/// `Len::times` of the metric the list was declared with and re-lowers with the type ramp.
/// [`Rows::unit`] carries what one of them measures, which is what turns a count back into the
/// DIPs a pointer arrives in.
///
/// A row is either **estimated** — [`ListSpec::estimate`], until the solve has reported a box
/// for it — or **measured**. A measurement is kept once taken, so a row scrolled out of the
/// window and back does not revert to the estimate and the offsets above the viewport stop
/// moving once they have been visited.
#[derive(Default, Debug)]
pub struct Rows {
    /// One row height in DIPs, at the scope the band group was solved at. Zero until the first
    /// solve has reported one.
    unit: f32,
    /// The application's identity for each row, in list order.
    keys: Vec<u64>,
    /// Row `i`'s extent in row heights, **negated while it is still the estimate**. A zero box
    /// is never a measurement, so the sign answers for every row and no parallel column of
    /// flags can fall out of step with it.
    extents: Vec<f32>,
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
        self.keys.iter().position(|&held| held == key)
    }

    /// Returns the key at `index`, or `None` past the end of the list.
    #[must_use]
    pub fn key(&self, index: usize) -> Option<u64> {
        self.keys.get(index).copied()
    }

    /// Returns row `index`'s top edge in DIPs, measured from the band group's own corner.
    #[must_use]
    pub fn offset(&self, index: usize) -> f32 {
        self.offset_units(index) * self.unit
    }

    /// Returns row `index`'s own height in DIPs.
    #[must_use]
    pub fn extent(&self, index: usize) -> f32 {
        self.extents.get(index).map_or(0.0, |e| e.abs()) * self.unit
    }

    /// Returns the whole list's height in DIPs.
    #[must_use]
    pub fn total(&self) -> f32 {
        self.total_units() * self.unit
    }

    /// Returns whether row `index`'s extent is what the solve measured rather than the
    /// estimate.
    #[must_use]
    pub fn is_measured(&self, index: usize) -> bool {
        self.extents.get(index).is_some_and(|&e| e > 0.0)
    }

    /// Returns row `index`'s offset in row heights, which is what a placement states.
    #[must_use]
    pub fn offset_units(&self, index: usize) -> f32 {
        self.prefix.get(index).copied().unwrap_or(0.0)
    }

    /// Returns the whole list's extent, in row heights.
    #[must_use]
    pub fn total_units(&self) -> f32 {
        self.prefix.last().copied().unwrap_or(0.0)
    }

    /// Returns the row the content position `y` falls in, clamped to the list.
    ///
    /// **Answers inside `0..len` at any `y`.** A tracker's position travels outside its bounds
    /// during a manipulation — the overpan is the bounce — so this is asked about positions
    /// past both ends of the content.
    #[must_use]
    pub fn at(&self, y: f32) -> usize {
        let Some(units) = self.units_at(y) else {
            return 0;
        };
        // The row is the last one whose top edge is at or before `units`, so a boundary
        // belongs to the row it opens.
        self.tops()
            .partition_point(|&top| top <= units)
            .saturating_sub(1)
    }

    /// Returns the first row whose top edge is at or below `y`, which is one past the last row
    /// a span reaching `y` shows.
    ///
    /// At least one, so a viewport with no height still realizes the row under its top edge
    /// and the list has something to measure before it has a scale.
    #[must_use]
    pub fn past(&self, y: f32) -> usize {
        let Some(units) = self.units_at(y) else {
            return usize::from(!self.keys.is_empty());
        };
        self.tops().partition_point(|&top| top < units).max(1)
    }

    /// Returns the rows' top edges, which is the prefix table without the list's own total.
    fn tops(&self) -> &[f32] {
        &self.prefix[..self.keys.len()]
    }

    /// Returns `y` in row heights, or `None` for an empty list or one with no scale yet.
    fn units_at(&self, y: f32) -> Option<f32> {
        (self.unit > 0.0 && !self.keys.is_empty()).then(|| y / self.unit)
    }

    /// Returns whether `keys` names a different list from the one held.
    #[must_use]
    pub fn differs(&self, keys: &[u64]) -> bool {
        self.keys != keys
    }

    /// Takes `keys` as the list, carrying each surviving key's measurement across.
    ///
    /// A key that was not in the list before is the estimate, and one that has left takes its
    /// measurement with it. Reordering is therefore free of re-measurement, which is what
    /// keeps a dragged row from changing height as it lands.
    pub fn take(&mut self, keys: &[u64], estimate: f32) {
        let was = core::mem::take(&mut self.extents);
        let mut cursor = 0;
        for &key in keys {
            // A reorder moves few rows, so the scan resumes where the last key was found and
            // wraps once, rather than searching the whole list for every key.
            let found = self.keys[cursor..]
                .iter()
                .position(|&held| held == key)
                .map(|at| at + cursor)
                .or_else(|| self.keys[..cursor].iter().position(|&held| held == key));
            cursor = found.map_or(cursor, |at| at + 1).min(self.keys.len());
            // Only a measurement is carried: an unmeasured row takes whatever the list now
            // guesses, so a changed estimate reaches every row that has not been visited.
            self.extents.push(
                found
                    .map(|at| was[at])
                    .filter(|&extent| extent > 0.0)
                    .unwrap_or(-estimate),
            );
        }
        self.keys.clear();
        self.keys.extend_from_slice(keys);
        self.reflow();
    }

    /// Returns whether `table` reports a scale or an extent the held table does not have.
    ///
    /// Read-only, because the write is what wakes every reader of the extents: a solve that
    /// measured nothing new must not wake them.
    #[must_use]
    pub fn needs(&self, table: &Table, unit: f32) -> bool {
        unit != self.unit
            || table.iter().any(|row| {
                matches!(
                    (row_units(row.rect, unit), self.index_of(row.key)),
                    (Some(extent), Some(at)) if self.extents[at] != extent
                )
            })
    }

    /// Writes every realized row's measured extent, and the scale they were measured at.
    ///
    /// A key the table reports and the list does not hold is dropped: the two are published by
    /// different passes of one flush, so a row can be measured on the solve that removed it
    /// from the document.
    pub fn write(&mut self, table: &Table, unit: f32) {
        self.unit = unit;
        for row in table.iter() {
            if let (Some(extent), Some(at)) = (row_units(row.rect, unit), self.index_of(row.key)) {
                self.extents[at] = extent;
            }
        }
        self.reflow();
    }

    /// Recomputes the offsets from the extents.
    ///
    /// One pass over the list and no allocation once the table has been sized: correcting a
    /// row's extent moves every row below it, and a table that answered by summing on demand
    /// would walk the list per row per placement.
    fn reflow(&mut self) {
        self.prefix.clear();
        self.prefix.push(0.0);
        let mut at = 0.0;
        for &extent in &self.extents {
            at += extent.abs();
            self.prefix.push(at);
        }
    }
}

/// Returns `rect`'s height in row heights, or `None` where nothing has been measured.
///
/// A zero box is what a table reads before its node is solved, and a row of no height would
/// stack every row below it on the same line.
fn row_units(rect: Rect, unit: f32) -> Option<f32> {
    (unit > 0.0 && rect.y1 > rect.y0).then(|| (rect.y1 - rect.y0) / unit)
}

// ── the realization window ───────────────────────────────────────────────────────

/// What a list needs to decide which rows exist.
///
/// **Variable extents.** A row's own content decides how tall it is, so a realized row is what
/// the solve measured and an unrealized one is [`ListSpec::estimate`]. Every extent is counted
/// in [`ListSpec::row_h`], which is therefore the list's unit as well as its guess.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ListSpec {
    /// The row height, as the palette's — so a list is as dense as the user asked for.
    pub row_h: Metric,
    /// What a row the solve has not measured is assumed to be, in `row_h`.
    pub estimate: f32,
    /// Rows realized beyond the viewport on each side. Two or three: enough that a row exists
    /// before it is looked at, few enough that a fling does not realize a screen it will never
    /// show.
    pub overscan: usize,
}

/// Rows realized beyond the viewport on each side unless a list states otherwise.
pub const OVERSCAN: usize = 3;

impl ListSpec {
    /// Returns a list whose rows are one `row_h` until they are measured.
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
/// The corridor covers a glance mid-flight rather than a read, so the path is sampled and the
/// rows realized for it do not scale with the distance flung.
const CORRIDOR: usize = 2;

/// How many runs a realized set holds: the live window, the destination, the corridor points
/// between them, and the pinned row. Sized so nothing a fling asks for is dropped.
const MAX_RUNS: usize = 3 + CORRIDOR;

/// Which rows are worth existing, as a bounded set of runs.
///
/// Several runs rather than one range: the resting position is known the instant inertia
/// begins, so the rows a fling lands on are realized before it arrives, and those are nowhere
/// near the ones on screen. Bounded and `Copy`, so realization allocates nothing.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Realized {
    runs: [(u32, u32); MAX_RUNS],
    n: u8,
}

impl Realized {
    /// Returns the runs, ascending and disjoint.
    pub fn runs(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.runs[..usize::from(self.n)]
            .iter()
            .map(|&(start, end)| start as usize..end as usize)
    }

    /// Appends a run, dropping an empty one and anything past [`MAX_RUNS`].
    fn push(&mut self, run: Range<usize>) {
        if run.is_empty() || usize::from(self.n) == MAX_RUNS {
            return;
        }
        self.runs[usize::from(self.n)] = (run.start as u32, run.end as u32);
        self.n += 1;
    }

    /// Sorts the runs by start and coalesces the overlaps.
    ///
    /// Insertion sort over at most [`MAX_RUNS`] entries, in place.
    fn normalize(&mut self) {
        let n = usize::from(self.n);
        for i in 1..n {
            let mut j = i;
            while j > 0 && self.runs[j - 1].0 > self.runs[j].0 {
                self.runs.swap(j - 1, j);
                j -= 1;
            }
        }
        let mut out = 0;
        for i in 1..n {
            if self.runs[i].0 <= self.runs[out].1 {
                self.runs[out].1 = self.runs[out].1.max(self.runs[i].1);
            } else {
                out += 1;
                self.runs[out] = self.runs[i];
            }
        }
        self.n = if n == 0 { 0 } else { out as u8 + 1 };
    }
}

/// Returns the rows worth realizing at `scroll_y`, plus `overscan` on each side.
///
/// **The range is inside `0..rows.len()` at any `scroll_y`.** A tracker's position travels
/// outside its bounds during a manipulation — the overpan is the bounce — so this is asked
/// about positions past the end of the content. An empty list answers `0..0`.
#[must_use]
pub fn window(scroll_y: f32, viewport_h: f32, rows: &Rows, overscan: usize) -> Range<usize> {
    if rows.is_empty() {
        return 0..0;
    }
    // A position past the end answers the last row, which is what makes the overpan realize
    // the end of the list rather than nothing; the two clamps are what keep the run inside it.
    let first = rows.at(scroll_y).saturating_sub(overscan);
    let last = (rows.past(scroll_y + viewport_h.max(0.0)) + overscan).min(rows.len());
    first..last.max(first)
}

/// Returns the whole realized set: where the content is, where a fling is taking it, a fixed
/// number of samples of the path between, and the row the list was told to keep.
///
/// The corridor is sampled rather than swept, so a fling crossing three thousand rows realizes
/// two windows and two overscan bands however far it travels. Allocates nothing.
#[must_use]
pub fn realize(rows: &Rows, at: Pos, overscan: usize) -> Realized {
    // The tracker reports the content's position and the rows are placed in the band group's
    // own space, so every position asked about is pulled back by where that group starts.
    let live = at.offset - at.band_y;
    let mut out = Realized::default();
    out.push(window(live, at.viewport, rows, overscan));
    if let Some(rest) = at.target() {
        let rest = rest - at.band_y;
        for sample in 1..=CORRIDOR {
            let point = live + (rest - live) * sample as f32 / (CORRIDOR + 1) as f32;
            // Zero height, so a sample is the overscan band around a point rather than a
            // second viewport's worth of rows nobody will look at.
            out.push(window(point, 0.0, rows, overscan));
        }
        out.push(window(rest, at.viewport, rows, overscan));
    }
    if let Some(index) = at.pin.and_then(|key| rows.index_of(key)) {
        out.push(index..index + 1);
    }
    out.normalize();
    out
}

// ── the containers ───────────────────────────────────────────────────────────────

/// What a scroll container is declared with.
///
/// The state travels with the reveal policy so that a list and the mount reporting into it
/// share one [`ListState`]; a second handle would be a second answer to where the content is.
/// Ordinary containers hold no application state; their position stays in the scene-side
/// shadow shared with input hit testing.
#[derive(Copy, Clone, Debug)]
pub struct ScrollDecl {
    pub reveal: Reveal,
    pub state: Option<ListState>,
}

/// Returns a scrolling container over `children`, with the default reveal policy.
///
/// The children go into a content group of their own, because the viewport must not move: it
/// is what clips, and an offset on it would take the clip with it.
pub fn scroll<'a>(ui: &'a mut Ui<'_>, children: impl FnOnce(&mut Ui<'_>)) -> Element<'a> {
    scroll_with(ui, Reveal::default(), children)
}

/// Returns a scrolling container with an explicit thumb reveal policy.
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

/// Returns a scrolling container driving `state`, for a [`list`] and whatever else the content
/// carries.
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
/// list's length and every row's place come from. `items` supplies the data for the runs it is
/// handed, **in ascending index order**; an index it does not supply is not realized, and the
/// space the table gives that row stays open. Rows are reconciled by the same keyed `each` as
/// any other list, so a row surviving a move of the window keeps its node, its owner and
/// everything scoped to it.
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
    let spec = Memo::new(spec);
    // The band group's own boxes: one table and one signal, so a row appearing wakes the
    // measurement and a solve that moved nothing wakes neither it nor anything derived from it.
    let boxes = anchors();
    // Where the band group sits inside the content, so the window is resolved in the group's
    // own space rather than the scroller's. A leading inset is ordinary flow content above it.
    let band = probe();

    // The list's shape. The scratch buffer is held across runs, so re-keying allocates nothing
    // once the list has been its longest.
    let named = RefCell::new(Vec::<u64>::new());
    Effect::new(move || {
        let estimate = spec.get().estimate;
        let mut next = named.borrow_mut();
        next.clear();
        keys(&mut next);
        if crate::signal::untracked(|| state.with_rows(|rows| rows.differs(&next))) {
            state.rows.update(|rows| rows.take(&next, estimate));
        }
    });

    // The measurement coming back, gated: the write is what wakes every reader of the extents.
    Effect::new(move || {
        state.banded(band.get().local.y);
        let row_h = spec.get().row_h;
        let needed = boxes.with(|table| {
            table.published().is_some_and(|scope| {
                let unit = metric(row_h, scope);
                crate::signal::untracked(|| state.with_rows(|rows| rows.needs(table, unit)))
            })
        });
        if needed {
            boxes.with(|table| {
                let unit = metric(row_h, table.scope());
                state.rows.update(|rows| rows.write(table, unit));
            });
        }
    });

    let realized = Memo::new(move || {
        let at = state.pos();
        state.with_rows(|rows| realize(rows, at, spec.get().overscan))
    });

    let supplied = RefCell::new(Vec::<(usize, T)>::new());
    ui.node(Preset::Layer)
        .anchors_origin(boxes)
        .probed(band)
        .layout_from(move |layout| {
            layout.height = Len::times(spec.get().row_h, state.extent());
        })
        .children(move |ui| {
            ui.each(
                move |out: &mut Vec<(u64, T)>| {
                    let mut supplied = supplied.borrow_mut();
                    supplied.clear();
                    realized.with(|set| items(set, &mut supplied));
                    state.with_rows(|rows| {
                        out.extend(
                            supplied
                                .drain(..)
                                .filter_map(|(index, item)| Some((rows.key(index)?, item))),
                        );
                    });
                },
                |(key, _)| key,
                move |ui, (key, item)| {
                    let key = *key;
                    let node = view(ui, item);
                    if let Some(row) = ui.edit(node) {
                        // Read inside the closure, so a measurement above this row moves it
                        // without the list being rebuilt.
                        row.anchored(boxes, key).layout_from(move |layout| {
                            let at = state.with_rows(|rows| {
                                rows.index_of(key)
                                    .map_or(0.0, |index| rows.offset_units(index))
                            });
                            layout.position = Position::Band {
                                at: Len::times(spec.get().row_h, at),
                            };
                        });
                    }
                },
            );
        })
}

// ── the two halves of a container ────────────────────────────────────────────────

/// One scroll container, as the app half needs it.
///
/// Held by the host beside the mount that owns the tracker, so a container unmounting takes its
/// row with it.
pub(crate) struct ScrollRow {
    /// What crosses to the thread routing a contact over this surface.
    pub front: ScrollFront,
    /// What was last published with it: a solve that moved nothing emits nothing, and
    /// [`ThumbGeom::UNSENT`] is what makes the first publication unconditional.
    pub last: ThumbGeom,
    /// The view width the thumb was last placed against; its x offset is a function of it.
    pub last_w: f32,
    /// The viewport node, whose solved size is the view extent.
    pub node: NodeId,
    /// The group the tracker's position is bound onto, whose solved height is the extent.
    pub content: NodeId,
    /// The strip the thumb travels in, which is the static geometry a grab lands on.
    pub rail: NodeId,
    pub thumb: SpriteId,
    pub reveal: Reveal,
    /// Whether the deferred tracker creation has run.
    pub created: bool,
    /// Present only for a virtualized list; an ordinary container keeps no application state.
    pub state: Option<ListState>,
}

impl ScrollRow {
    /// Emits whatever this container's solve changed: its arrival, and its thumb geometry.
    ///
    /// The add is emitted before the geometry gate, so a container whose content fits is in the
    /// table too: its thumb still has a reveal, and a row that never arrived would leave every
    /// hover over that surface acting on nothing.
    pub fn publish(&mut self, geom: ThumbGeom, out: &mut Vec<ScrollOp>) {
        if self.last.max_scroll.is_nan() {
            out.push(ScrollOp::Add {
                front: self.front,
                thumb: (self.reveal != Reveal::Never).then_some(self.thumb),
                reveal: self.reveal,
                observe: self.state.is_some(),
            });
        }
        if geom != self.last {
            self.last = geom;
            out.push(ScrollOp::Thumb {
                viewport: self.front.viewport,
                geom,
            });
        }
    }
}

impl Ui<'_> {
    /// Mints the viewport, its content group and its scroll chrome, and runs `children` inside
    /// the content group.
    ///
    /// The children go into a content group of their own, because the viewport must not move: it
    /// is what clips, and an offset on it would take the clip with it.
    pub fn scroll(&mut self, decl: ScrollDecl, children: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        let mut content = NodeId::NONE;
        let mut viewport = self
            .node(Preset::Scroll)
            .children(|ui| content = ui.group(Preset::Stack, children).node_id());
        let node = viewport.node_id();
        let host = viewport.host();
        let scope = host.scope_of(node);
        // Minted after the content, because child order is paint order: a bar declared before
        // the rows would be drawn under whatever the list paints over them.
        let (rail, thumb) = crate::build::mount::mount_scroll_chrome(host, node);
        // Concealed until a reason reveals it: the front thread moves this channel from here,
        // and it moves it from the state the mount left it in.
        if decl.reveal == Reveal::OnDemand {
            host.bind(thumb.0, Prop::Opacity, Bind::Set(Value::Scalar(0.0)));
        }
        let hover = host.mint_control(ControlRow::blank(node, scope));
        // The viewport is where the scroll pattern hangs, so it is an element: a group, which
        // the control view carries and the content view does not, so a reader walks past it and
        // a client can still move it.
        if let Some(row) = host.control_mut(hover) {
            row.uia = crate::widget::UiaRole::Group;
        }
        // The viewport owns keyboard scrolling; the rail only accepts pointer input.
        let mut rail_control = ControlRow::blank(rail, scope);
        rail_control.tab_stop = Some(false);
        let grab = host.mint_control(rail_control);
        host.focus_ops.push(crate::seam::FocusOp::TabIndex(grab, -1));
        // The surface itself is a target so a hover can reveal its bar and a wheel notch the
        // tracker did not take reaches it. Its own box is the whole of it, so it is not
        // inflated: a control sitting near its edge would otherwise share the point.
        host.hit(
            node,
            Some(HitDecl {
                flags: HitFlags::INTERACTIVE.union(HitFlags::WHEEL),
                id: hover,
                touch_inflate: Some(0.0),
            }),
        );
        host.gestures.push((grab, grab_decl()));
        let front = ScrollFront {
            viewport: node,
            tracker: host.tracker_id(),
            hover,
            grab,
        };
        let at = host.scrolls.place(ScrollRow {
            front,
            last: ThumbGeom::UNSENT,
            last_w: f32::NAN,
            node,
            content,
            rail,
            thumb,
            reveal: decl.reveal,
            created: false,
            state: decl.state,
        });
        host.set_scroll_row(node, at);
        viewport
    }
}

impl Host {
    /// Publishes every scroll container's solve: its tracker, its thumb, and its rail's target.
    ///
    /// Two deferral gates, both contracts rather than optimisations. **A viewport with no area
    /// has not been laid out**, and a `VisualInteractionSource` takes its hit region from the
    /// viewport's size at the moment it is created: created at mount it hit-tests nothing,
    /// reports success, and the surface silently ignores every wheel notch for the life of the
    /// window. **The rail is a hit target only while there is something to scroll**; left on, it
    /// takes every press on the right edge of a surface that does not scroll, and a button
    /// sitting there cannot be clicked.
    pub(crate) fn publish_scrolls(&mut self) {
        for at in 0..self.scrolls.slots() {
            let Some(row) = self.scrolls.get(at) else {
                continue;
            };
            let (node, content) = (row.node, row.content);
            let view = self.tree.c.geom[node.index()].size;
            if view.x <= 0.0 || view.y <= 0.0 {
                continue;
            }
            let geom = thumb_geom(view.y, self.tree.c.geom[content.index()].size.y);
            self.publish_scroll(at, geom, view);
        }
    }

    /// Brings container `at` up to the geometry its solve implies.
    fn publish_scroll(&mut self, at: u32, geom: ThumbGeom, view: Vector2) {
        let Some(row) = self.scrolls.get(at) else {
            return;
        };
        let (front, content, rail, thumb) = (row.front, row.content, row.rail, row.thumb);
        let (reveal, state, created) = (row.reveal, row.state, row.created);
        let moved = row.last != geom || row.last_w != view.x;
        if !created {
            self.create_tracker(front.tracker, GroupId(front.viewport), Axes::VERTICAL);
            self.bind(
                content,
                Prop::OffsetY,
                track(front.tracker, Affine::CONTENT),
            );
        }
        // Everything below is a function of the extents, so it is re-sent only when they move.
        if !created || moved {
            // Automation reports how far the content can travel and how much of it is shown,
            // which are these extents: a tree published before a card opened would report a
            // container that cannot scroll while the thumb beside it says otherwise.
            self.uia_stale.set(true);
            self.bind(thumb.0, Prop::OffsetY, track(front.tracker, geom.affine()));
            self.tracker_bounds(
                front.tracker,
                Vector2 { x: 0.0, y: 0.0 },
                Vector2 {
                    x: 0.0,
                    y: geom.max_scroll,
                },
            );
            let shown = reveal != Reveal::Never && geom.overflow();
            self.visual_rect(thumb, geom.offset(view.x), geom.size());
            self.hide(thumb.0, !shown);
            self.hit(rail, shown.then(|| grab_hit(front.grab)));
        }
        if let Some(state) = state {
            state.resized(view.y);
            if let Some(to) = state.take_reveal(view.y) {
                self.scroll_ops.push(ScrollOp::Reveal {
                    viewport: front.viewport,
                    to: Vector2 { x: 0.0, y: to },
                });
            }
        }
        let Self {
            scrolls,
            scroll_ops,
            ..
        } = self;
        if let Some(row) = scrolls.get_mut(at) {
            row.created = true;
            row.last_w = view.x;
            row.publish(geom, scroll_ops);
        }
    }

    /// Runs `f` against the container whose tracker `tracker` names.
    ///
    /// A linear scan rather than a map: a window holds a handful of scroll surfaces, and this is
    /// walked from every tracker report of every fling.
    pub(crate) fn scroll_by_tracker(
        &mut self,
        tracker: Id<TRACKER>,
        f: impl FnOnce(&mut ScrollRow),
    ) {
        if let Some((_, row)) = self
            .scrolls
            .iter_mut()
            .find(|(_, row)| row.front.tracker.id() == tracker)
        {
            f(row);
        }
    }
}

/// Returns the binding that drives one channel from `tracker` through `affine`.
///
/// One channel and never a composite: `Bind::Track` names a single property, and a binding
/// aimed at `Prop::Offset` is refused.
fn track(tracker: TrackerId<Observed>, affine: Affine) -> Bind {
    Bind::Track {
        tracker: tracker.erased(),
        axis: TrackerAxis::PositionY,
        affine,
    }
}

/// One scroll container, as the half that moves its thumb holds it.
struct Live {
    of: ScrollFront,
    /// The thumb's node, absent where the container declared no bar at all.
    thumb: Option<SpriteId>,
    reveal: Reveal,
    /// Whether this container's tracker reports are owed to the app thread.
    observe: bool,
    /// The geometry a grab maps through, as the app half last published it.
    last: ThumbGeom,
    /// Where the app half asked the content to go, `NaN` until the next tick asks the
    /// compositor.
    requested: f32,
    /// Where the content stood when the current thumb grab began, `NaN` when none is held.
    grab_from: f32,
    /// Whether the thumb was last retargeted to shown, so an unchanged reason emits nothing.
    ///
    /// `StartAnimation` resets the property's velocity, so a retarget per pointer sample would
    /// pin the opacity instead of ramping it.
    shown: bool,
    /// The extent this container holds past the maximum its layout stated, for as long as a
    /// docked occlusion needs it. Zero restores the stated maximum.
    ///
    /// Held beside the published geometry rather than folded into it, because the layout
    /// restates that geometry whenever its extents move and an occlusion outlives a resize.
    extended: f32,
}

impl Live {
    /// Returns the geometry this container is actually at: the one its layout published, with
    /// whatever extent an occlusion is holding added to the distance the content can travel.
    ///
    /// The rail and the thumb are solved nodes and an occlusion reflows neither, so the thumb's
    /// height and travel are the layout's. What changes is how far the content moves across
    /// that travel, which is the one number the affine and the grab both read. Deriving both
    /// from here is what keeps the thumb inside its rail while the extent is held.
    fn geom(&self) -> ThumbGeom {
        ThumbGeom {
            max_scroll: self.last.max_scroll + self.extended,
            ..self.last
        }
    }
}

/// Every scroll container, as the half that moves thumbs holds them.
///
/// Built from the edits the app half emits and never from its tables, so a reveal and a grab
/// cost a scan of a handful of rows and no hop.
#[derive(Default)]
pub(crate) struct ScrollTable {
    rows: Vec<Live>,
    reorder: Option<(NodeId, i8)>,
    /// The word each live tracker publishes its reported position into, listed from the scene.
    shadows: Vec<(NodeId, Arc<AtomicU64>)>,
    /// Whether a container has arrived or left since the shadows were listed.
    relist: bool,
    /// Containers whose thumb binding is owed a rebind, because their layout restated a
    /// geometry while an extent was held. Drained by the pass that applied the batch.
    rebind: Vec<NodeId>,
    /// What the last reveal asked of each container on the field's scroll ancestry, innermost
    /// first. Kept across reveals so a settled window plans one allocating nothing.
    steps: Vec<(usize, RevealStep)>,
}

/// What one scroll container has to do to bring a box into view.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct RevealStep {
    /// Where the content has to stand.
    to: f32,
    /// Extent owed past the container's own maximum, which is what an occlusion asks for.
    extra: f32,
}

/// Returns where `target` asks the content to stand, and the extent the occlusion asks for
/// past `max_scroll`.
///
/// `target` and `view` are the same space's vertical spans, `at` is where the content stands
/// and `bottom` is the lowest line of the viewport nothing covers. A box already inside answers
/// `None`: a request for the position the content already has cancels whatever inertia is
/// running.
///
/// The extent is what an occlusion takes off the viewport, and nothing else. How far content
/// can travel is its own length less the viewport's, so a viewport an occlusion has shortened
/// by that much can carry it that much further — which is what makes a surface at the end of
/// the content reachable. Stating it from the occlusion rather than from where the content
/// happens to stand is what makes it idempotent: an occlusion that goes away asks for none,
/// and the position clamps back into the range the content has.
fn reveal_step(
    target: (f32, f32),
    view: (f32, f32),
    bottom: f32,
    at: f32,
    max_scroll: f32,
) -> Option<RevealStep> {
    let bottom = bottom.max(view.0);
    let delta = if target.1 - at > bottom {
        target.1 - at - bottom
    } else if target.0 - at < view.0 {
        target.0 - at - view.0
    } else {
        return None;
    };
    let extra = (view.1 - bottom).max(0.0);
    // `clamp` needs an ordered range, and a container whose geometry has not been published
    // yet states `NaN` for its maximum, which orders against nothing.
    let limit = max_scroll + extra;
    let to = match limit >= 0.0 {
        true => (at + delta).clamp(0.0, limit),
        false => (at + delta).max(0.0),
    };
    Some(RevealStep { to, extra })
}

impl ScrollTable {
    pub(crate) fn reorder_scroll(
        &mut self, pointer: Option<(ControlId, windows_scene::Point, [f32; 2])>, front: &mut Front<'_>,
    ) -> Result<()> {
        let wanted = pointer.and_then(|(id, point, _)| {
            let entry = front.scene.hits().entry(id)?;
            if entry.flags.contains(HitFlags::UNSCROLLED) { return None; }
            let row = self.rows.iter().find(|row| row.of.viewport == entry.scroll_src)?;
            let view = front.scene.hits().visible_rect(row.of.hover)?;
            let direction = reorder_edge(view, point);
            (direction != 0 && row.geom().max_scroll.is_finite() && row.geom().max_scroll > 0.0)
                .then_some((entry.scroll_src, direction))
        });
        if self.reorder == wanted { return Ok(()); }
        if let Some((viewport, _)) = self.reorder.take()
            && let Some(row) = self.rows.iter().find(|row| row.of.viewport == viewport)
        {
            front.scene.request(row.of.tracker, TrackerRequest::To(front.scene.hits().offset(viewport)))?;
        }
        if let Some((viewport, direction)) = wanted {
            let row = self.rows.iter().find(|row| row.of.viewport == viewport).unwrap();
            let from = front.scene.hits().offset(viewport);
            let view = front.scene.hits().visible_rect(row.of.hover).unwrap();
            let span = pointer.unwrap().2;
            let edge = if direction < 0 { span[0] - view[1] - REORDER_EDGE }
                else { span[1] - view[3] + REORDER_EDGE };
            let to = Vector2::new(from.x, edge.clamp(0.0, row.geom().max_scroll));
            let seconds = (to.y - from.y) * f32::from(direction) / REORDER_SPEED;
            if seconds > 0.0 {
                front.scene.animate_tracker(row.of.tracker, to,
                    std::time::Duration::from_secs_f32(seconds.max(0.001)), front.back)?;
            }
            self.reorder = wanted;
        }
        Ok(())
    }

    /// Applies one batch of container edits, in the order the app half emitted them.
    ///
    /// A thumb declared [`Reveal::Always`] is opaque from its mount, so its row starts shown and
    /// the first edge that would show it again emits nothing.
    pub(crate) fn apply_ops(&mut self, ops: &mut Vec<ScrollOp>) {
        for op in ops.drain(..) {
            match op {
                ScrollOp::Add {
                    front,
                    thumb,
                    reveal,
                    observe,
                } => {
                    self.relist = true;
                    self.rows.push(Live {
                        of: front,
                        thumb,
                        reveal,
                        observe,
                        shown: reveal == Reveal::Always,
                        last: ThumbGeom::UNSENT,
                        requested: f32::NAN,
                        grab_from: f32::NAN,
                        extended: 0.0,
                    });
                }
                ScrollOp::Thumb { viewport, geom } => {
                    self.edit(viewport, |row| row.last = geom);
                    // The affine the thumb is bound through is a function of the extended
                    // geometry, so a restated layout owes a rebind while an extent is held.
                    self.rebind.push(viewport);
                }
                ScrollOp::Reveal { viewport, to } => {
                    self.edit(viewport, |row| row.requested = to.y);
                }
                ScrollOp::Drop { viewport } => {
                    self.relist = true;
                    self.rows.retain(|row| row.of.viewport != viewport);
                }
            }
        }
    }

    /// Returns whether this container's tracker reports are owed to the app thread.
    ///
    /// Only a scroll with application-owned state needs them upstream. Scene-side hit offsets,
    /// request completion and thumb reveal are already serviced, and an unknown tracker stays
    /// available to every other consumer.
    pub(crate) fn app_observes(&self, event: &SceneEvent) -> bool {
        let tracker = match *event {
            SceneEvent::TrackerValues { tracker, .. }
            | SceneEvent::TrackerPhase { tracker, .. }
            | SceneEvent::InertiaBegan { tracker, .. }
            | SceneEvent::RequestIgnored { tracker, .. } => tracker,
            _ => return true,
        };
        self.rows
            .iter()
            .find(|row| row.of.tracker.id() == tracker.id())
            .is_none_or(|row| row.observe)
    }

    /// Brings the field `request` names clear of whatever occludes it, through every scroll
    /// container that encloses it.
    ///
    /// Each container is named by the enclosed entry's own `scroll_src` rather than by a parent
    /// walk, because that entry already carries the surface its rect resolves through. The walk
    /// then asks the next container out to show where the field has just been placed, so a
    /// field nested two containers deep is reached by both.
    ///
    /// Every container the walk does not reach gives back whatever extent it was holding, which
    /// is what returns the content to its own range when the keyboard closes.
    ///
    /// # Errors
    ///
    /// The compositor refused a tracker request or an extent.
    /// Moves the container `hover` names to an absolute content offset.
    ///
    /// What a client asked for through the scroll pattern, resolved against the same tracker a
    /// wheel notch and a thumb drag move, so the three cannot disagree about where the content
    /// is. A control that is not a container moves nothing.
    ///
    /// # Errors
    ///
    /// The compositor refused the request.
    pub(crate) fn scroll_to(
        &mut self,
        hover: ControlId,
        to: Vector2,
        front: &mut Front<'_>,
    ) -> Result<()> {
        let Some(row) = self.rows.iter().find(|row| row.of.hover == hover) else {
            return Ok(());
        };
        let tracker = row.of.tracker;
        request(front, tracker, to.y)
    }

    fn key_request(
        &self,
        target: ControlId,
        event: KeyEvent,
        hits: &HitTable,
    ) -> Option<(TrackerId<Observed>, TrackerRequest)> {
        if event.kind != KeyKind::Down || event.mods.ctrl || event.mods.alt {
            return None;
        }
        let row = self.rows.iter().find(|row| row.of.hover == target)?;
        let hit = hits.entry(target)?;
        if !hit.flags.contains(HitFlags::INTERACTIVE.union(HitFlags::SCROLL)) {
            return None;
        }
        let page = (hit.y1 - hit.y0).max(0.0);
        let max = row.geom().max_scroll;
        if page <= 0.0 || !max.is_finite() || max <= OVERFLOW_FLOOR {
            return None;
        }
        let y = match event.key as i32 {
            VK_UP => -page * SMALL_STEP,
            VK_DOWN => page * SMALL_STEP,
            VK_PRIOR => -page,
            VK_NEXT => page,
            VK_HOME => return Some((row.of.tracker, TrackerRequest::To(Vector2 { x: 0.0, y: 0.0 }))),
            VK_END => return Some((row.of.tracker, TrackerRequest::To(Vector2 { x: 0.0, y: max }))),
            _ => return None,
        };
        // Relative requests accumulate in the tracker before its next position publication.
        Some((row.of.tracker, TrackerRequest::By(Vector2 { x: 0.0, y })))
    }

    pub(crate) fn reveal_field(
        &mut self,
        reveal: crate::text_input::Reveal,
        front: &mut Front<'_>,
    ) -> Result<()> {
        self.steps.clear();
        if let Some(id) = reveal.id
            && let Some(field) = front.scene.hits().entry(id).copied()
        {
            let column = (field.x0, field.x1);
            // The span to bring into view, stated in the space of the container being asked.
            let mut target = reveal.span.map_or((field.y0, field.y1), |(top, bottom)| {
                (field.y0 + top, field.y0 + bottom)
            });
            let mut viewport = field.scroll_src;
            while let Some(at) = self.rows.iter().position(|row| row.of.viewport == viewport) {
                let hover = self.rows[at].of.hover;
                let Some(view) = front.scene.hits().entry(hover).copied() else {
                    break;
                };
                let Some(offset) = self.shadow(viewport, front) else {
                    break;
                };
                // A candidate window or panel over the field takes room off the bottom of the
                // viewport, but only where it actually overlaps the field's own column.
                let bottom = reveal
                    .occlusion
                    .filter(|r| r.x0 < column.1 && column.0 < r.x1)
                    .map_or(view.y1, |r| view.y1.min(r.y0));
                // Against the layout's own maximum, not the extended one: the extent a reveal
                // asks for is stated afresh each time, so one that no longer needs it gives it
                // back rather than compounding it.
                let wanted = match reveal.align_top {
                    Some(true) => (target.0, target.0 + bottom - view.y0),
                    Some(false) => (target.1 - (bottom - view.y0), target.1),
                    None => target,
                };
                let step = reveal_step(
                    wanted,
                    (view.y0, view.y1),
                    bottom,
                    offset.y,
                    self.rows[at].last.max_scroll,
                );
                let stands = step.map_or(offset.y, |step| step.to);
                if let Some(step) = step {
                    self.steps.push((at, step));
                }
                // Where the field comes to rest inside this container, stated in the space the
                // container itself sits in, which is what the next container out is shown.
                let top = view.y0 + (target.0 - stands);
                target = (top.max(view.y0), (top + (target.1 - target.0)).min(view.y1));
                viewport = view.scroll_src;
            }
        }
        let mut first = Ok(());
        // Extents before positions: a request issued against the range it needs widening past
        // lands short of the occlusion it was meant to clear.
        for i in 0..self.steps.len() {
            let (at, step) = self.steps[i];
            keep(&mut first, self.extend(at, step.extra, front));
        }
        for at in 0..self.rows.len() {
            if !self.steps.iter().any(|(held, _)| *held == at) {
                keep(&mut first, self.extend(at, 0.0, front));
            }
        }
        for i in 0..self.steps.len() {
            let (at, step) = self.steps[i];
            let tracker = self.rows[at].of.tracker;
            keep(&mut first, request(front, tracker, step.to));
        }
        first
    }

    /// Holds `extra` extent on one container, moving the tracker's range and the thumb's map
    /// together so neither can describe a range the other does not have.
    ///
    /// # Errors
    ///
    /// The compositor refused the extent or the binding.
    fn extend(&mut self, at: usize, extra: f32, front: &mut Front<'_>) -> Result<()> {
        let Some(row) = self.rows.get_mut(at) else {
            return Ok(());
        };
        if row.extended == extra {
            return Ok(());
        }
        row.extended = extra;
        self.apply_extent(at, front)
    }

    /// Writes one container's extended geometry to the tracker and to the thumb.
    ///
    /// Both sides of the map are written from [`Live::geom`], so the range the content rests in
    /// and the range the thumb traverses are the same number by construction. The thumb is
    /// bound rather than sprung: it is a map from a position, not a destination.
    ///
    /// # Errors
    ///
    /// The compositor refused the extent or the binding.
    fn apply_extent(&mut self, at: usize, front: &mut Front<'_>) -> Result<()> {
        let Some(row) = self.rows.get(at) else {
            return Ok(());
        };
        let (tracker, thumb, geom) = (row.of.tracker, row.thumb, row.geom());
        front.scene.extend_bounds(
            tracker,
            Vector2 {
                x: 0.0,
                y: row.extended,
            },
        )?;
        let Some(thumb) = thumb else {
            return Ok(());
        };
        front.scene.retarget(
            thumb.0,
            Prop::OffsetY,
            track(tracker, geom.affine()),
            front.back,
        )
    }

    /// Re-applies the extent of every container whose layout restated its geometry.
    ///
    /// # Errors
    ///
    /// The compositor refused an extent or a binding. The first failure is kept and the rest of
    /// the batch still runs.
    pub(crate) fn rebind_extents(&mut self, front: &mut Front<'_>) -> Result<()> {
        let mut first = Ok(());
        while let Some(viewport) = self.rebind.pop() {
            let Some(at) = self.rows.iter().position(|row| row.of.viewport == viewport) else {
                continue;
            };
            if self.rows[at].extended == 0.0 {
                continue;
            }
            keep(&mut first, self.apply_extent(at, front));
        }
        first
    }

    /// Returns the offset the tracker bound to `viewport` last reported.
    ///
    /// The set of shadow words changes only when a tracker is created or dropped, so it is
    /// listed on those edges and read as an atomic between them.
    fn shadow(&mut self, viewport: NodeId, front: &Front<'_>) -> Option<Vector2> {
        if self.relist {
            front.scene.tracker_shadows(&mut self.shadows);
            self.relist = false;
        }
        let word = self
            .shadows
            .iter()
            .find(|(node, _)| *node == viewport)
            .map(|(_, word)| word)?;
        // acquire: pairs with the release store the scene makes when it records a reported
        // position, so both halves of the word read here are that one report's.
        let (x, y) = unpack_offset(word.load(Ordering::Acquire));
        Some(Vector2 { x, y })
    }

    /// Runs `f` against the container `viewport` names.
    ///
    /// A linear scan rather than a map: a screen has a handful of scroll surfaces, and this is
    /// walked from every tracker report of every fling.
    fn edit(&mut self, viewport: NodeId, f: impl FnOnce(&mut Live)) {
        if let Some(row) = self.rows.iter_mut().find(|row| row.of.viewport == viewport) {
            f(row);
        }
    }

    /// Returns the index of the first container `pick` accepts.
    ///
    /// An index rather than a borrow, so a caller that also has to read a shadow word does not
    /// hold this table borrowed across that read.
    fn find(&self, pick: impl Fn(&Live) -> bool) -> Option<usize> {
        self.rows.iter().position(|row| pick(row))
    }
}

const REORDER_EDGE: f32 = 24.0;
const REORDER_SPEED: f32 = 240.0;

fn reorder_edge(view: [f32; 4], point: windows_scene::Point) -> i8 {
    if !point.x.is_finite() || !point.y.is_finite()
        || point.x < view[0] || point.x > view[2] || point.y < view[1] || point.y > view[3]
    { return 0; }
    let band = REORDER_EDGE.min((view[3] - view[1]) * 0.5);
    if point.y < view[1] + band { -1 }
    else if point.y > view[3] - band { 1 }
    else { 0 }
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
            let tracker = match *event {
                SceneEvent::TrackerValues { tracker, .. }
                | SceneEvent::InertiaBegan { tracker, .. }
                | SceneEvent::TrackerPhase { tracker, .. } => tracker,
                _ => continue,
            };
            host.scroll_by_tracker(tracker.id(), |row| {
                let Some(state) = row.state else { return };
                // Read before the edit, because holding the extent needs the other signal and
                // the edit closure already holds this one.
                let span = state.with_rows(Rows::total_units);
                state.edit(|at| match *event {
                    SceneEvent::TrackerValues { position, .. } => at.offset = position.y,
                    // The resting position with snap points applied, which is the destination
                    // the content will actually reach.
                    SceneEvent::InertiaBegan { rest, .. } => at.target = Some(rest.y),
                    // Returning to idle releases the held extent and clears the destination;
                    // leaving idle holds the extent where it stands.
                    SceneEvent::TrackerPhase {
                        phase: TrackerPhase::Idle,
                        ..
                    } => {
                        at.target = None;
                        at.held = 0.0;
                    }
                    SceneEvent::TrackerPhase { .. } if at.held <= 0.0 => at.held = span,
                    _ => {}
                });
            });
        }
    });
}

/// Applies thumb reveals, redirected contacts, thumb drags and focused keyboard scrolling
/// to the compositor.
///
/// Must run **after the apply**, so a retarget never names a node the patch was about to
/// rebuild, and after the router's tick, so a grab resolves against the hit array this frame
/// published.
///
/// # Errors
///
/// The compositor refused a retarget or a tracker request. The first failure is kept and the
/// rest of the tick still runs, so a refused retarget on one surface does not leave another's
/// grab half-applied.
pub(crate) fn front(
    events: &[SceneEvent],
    reports: &[Report],
    table: &mut ScrollTable,
    front: &mut Front<'_>,
) -> Result<()> {
    if events.is_empty()
        && reports.is_empty()
        && table.rows.iter().all(|row| row.requested.is_nan())
    {
        return Ok(());
    }
    let mut failed = Ok(());
    // Taken before the events, so a reveal and a phase edge in one tick leave the content where
    // the reveal asked rather than where it stood.
    for at in 0..table.rows.len() {
        if table.rows[at].requested.is_nan() {
            continue;
        }
        let y = core::mem::replace(&mut table.rows[at].requested, f32::NAN);
        let tracker = table.rows[at].of.tracker;
        keep(&mut failed, request(front, tracker, y));
    }
    for event in events {
        let SceneEvent::TrackerPhase { tracker, phase } = *event else {
            continue;
        };
        if let Some(at) = table.find(|row| row.of.tracker.id() == tracker.id()) {
            keep(
                &mut failed,
                reveal(&mut table.rows[at], phase != TrackerPhase::Idle, front),
            );
        }
    }
    for report in reports {
        match *report {
            // A touch contact over a scroll surface is handed to the tracker, which is what
            // makes the pan compositor-side; the front thread sees no further samples of it.
            Report::Redirect { target, pointer } => {
                if let Some(at) = table.find(|row| row.of.hover == target) {
                    let tracker = table.rows[at].of.tracker;
                    keep(
                        &mut failed,
                        front.scene.redirect_manipulation(tracker, &pointer),
                    );
                }
            }
            Report::HoverChanged { from, to, .. } => {
                for (control, over) in [(from, false), (to, true)] {
                    let Some(control) = control else { continue };
                    if let Some(at) = table.find(|row| row.of.hover == control) {
                        keep(&mut failed, reveal(&mut table.rows[at], over, front));
                    }
                }
            }
            Report::Dragged { target, update, .. } => {
                keep(&mut failed, drag(table, target, update, front));
            }
            Report::Key { target: Some(target), event } => {
                if let Some((tracker, request)) = table.key_request(target, event, front.scene.hits()) {
                    keep(&mut failed, front.scene.request(tracker, request).map(|_| ()));
                }
            }
            // A released or cancelled grab forgets where it started, so the next one measures
            // from where the content actually is.
            Report::Released { target, .. } | Report::Canceled { target, .. } => {
                if let Some(at) = table.find(|row| row.of.grab == target) {
                    table.rows[at].grab_from = f32::NAN;
                }
            }
            _ => {}
        }
    }
    failed
}

/// Shows or conceals a thumb, where the row is on an edge that moves one.
fn reveal(row: &mut Live, show: bool, front: &mut Front<'_>) -> Result<()> {
    let Some(thumb) = row.thumb else {
        return Ok(());
    };
    let Some(bind) = reveal_bind(row, show) else {
        return Ok(());
    };
    front
        .scene
        .retarget(thumb.0, Prop::Opacity, bind, front.back)
}

/// Decides what a thumb's opacity channel is bound to, recording the edge it crosses.
///
/// Returns `None` off an edge, and for a thumb this half does not move: [`Reveal::Always`] is
/// opaque from the mount and [`Reveal::Never`] has no thumb. `StartAnimation` resets the
/// property's velocity, so a retarget per pointer sample would pin the opacity rather than ramp
/// it.
///
/// A conceal states [`CONCEAL_MS`] as the fade's own delay, so the thumb holds its opacity for
/// that long and then ramps down. A reason arriving inside the wait binds the same channel
/// again, and starting an animation replaces the one on the property, so the pending fade is
/// gone rather than queued behind it.
fn reveal_bind(row: &mut Live, show: bool) -> Option<Bind> {
    // A grabbed thumb stays lit however the pointer wanders, and a moving one stays lit whatever
    // the pointer is doing.
    let show = show || !row.grab_from.is_nan();
    if row.reveal != Reveal::OnDemand || show == row.shown {
        return None;
    }
    row.shown = show;
    Some(Bind::Animate(Anim::Spring {
        to: Value::Scalar(if show { 1.0 } else { 0.0 }),
        tuning: Tuning::Chrome,
        delay_ms: if show { 0 } else { CONCEAL_MS },
    }))
}

/// Drives the tracker from a thumb the pointer is dragging.
///
/// The displacement is from the contact's origin, so the content position is resolved from where
/// it stood when the grab began rather than accumulated — a dropped sample then costs nothing,
/// where an accumulated one would drift for the rest of the drag.
///
/// The origin is read from the tracker's published word rather than from the container's signal,
/// which is the app half's. A tracker with no word is one the compositor has not created, and it
/// has nothing to be dragged from.
fn drag(
    table: &mut ScrollTable,
    target: ControlId,
    update: DragUpdate,
    front: &mut Front<'_>,
) -> Result<()> {
    let Some(at) = table.find(|row| row.of.grab == target) else {
        return Ok(());
    };
    if update.phase == Phase::Undecided {
        return Ok(());
    }
    if table.rows[at].grab_from.is_nan() {
        let viewport = table.rows[at].of.viewport;
        let Some(offset) = table.shadow(viewport, front) else {
            return Ok(());
        };
        table.rows[at].grab_from = offset.y;
    }
    let row = &table.rows[at];
    let geom = row.geom();
    let thumb_y = thumb_y_for_scroll(row.grab_from, geom) + update.delta.y;
    let to = scroll_for_thumb_y(thumb_y, geom);
    let tracker = row.of.tracker;
    request(front, tracker, to)
}

/// Asks a container's tracker for an absolute content position.
fn request(front: &mut Front<'_>, tracker: TrackerId<Observed>, y: f32) -> Result<()> {
    front
        .scene
        .request(tracker, TrackerRequest::To(Vector2 { x: 0.0, y }))
        .map(|_| ())
}

/// Keeps the first failure of a tick, so the rest of it still runs.
fn keep(first: &mut Result<()>, result: Result<()>) {
    if first.is_ok() {
        *first = result;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Anchored;

    /// Returns the geometry for a viewport showing `content_h` of content.
    fn geom(viewport_h: f32, content_h: f32) -> ThumbGeom {
        thumb_geom(viewport_h, content_h)
    }

    /// A 400-DIP viewport over 1000 DIPs of content: 600 of travel, and a field at 940..980.
    const TRAVEL: f32 = 600.0;

    /// A held extent moves the range the content rests in and the map the thumb is drawn
    /// through together. The thumb is a map from a position, so the two cannot be written
    /// apart: at the end of the extended range it is at the end of its own travel, and a grab
    /// there asks for the position it is showing.
    #[test]
    fn a_held_extent_keeps_the_thumb_and_the_content_on_one_range() {
        let published = thumb_geom(400.0, 1000.0);
        let mut row = Live {
            of: ScrollFront {
                viewport: NodeId::NONE,
                tracker: TrackerId::new(Id::raw(0, 1)),
                hover: ControlId::NONE,
                grab: ControlId::NONE,
            },
            thumb: None,
            reveal: Reveal::OnDemand,
            observe: false,
            shown: false,
            last: published,
            requested: f32::NAN,
            grab_from: f32::NAN,
            extended: 0.0,
        };
        assert_eq!(row.geom(), published, "with nothing held, the layout's own");

        row.extended = 230.0;
        let geom = row.geom();
        assert_eq!(geom.max_scroll, published.max_scroll + 230.0);
        assert_eq!(
            (geom.thumb_h, geom.travel),
            (published.thumb_h, published.travel),
            "an occlusion reflows neither the rail nor the thumb"
        );
        let end = thumb_y_for_scroll(geom.max_scroll, geom);
        assert_eq!(
            end,
            THUMB_MARGIN + geom.travel,
            "the extended end of the content is the end of the thumb's travel"
        );
        assert!(
            (scroll_for_thumb_y(end, geom) - geom.max_scroll).abs() < 1e-3,
            "and a grab there asks for the position the thumb is showing"
        );
        assert!(
            thumb_y_for_scroll(published.max_scroll, geom) < end,
            "the layout's own maximum no longer reaches the end of the rail"
        );

        row.extended = 0.0;
        assert_eq!(row.geom(), published, "clearing it restores the layout's");
    }

    #[test]
    fn a_box_already_inside_the_viewport_asks_for_nothing() {
        assert_eq!(
            reveal_step((100.0, 140.0), (0.0, 400.0), 400.0, 0.0, TRAVEL),
            None
        );
        assert_eq!(
            reveal_step((700.0, 740.0), (0.0, 400.0), 400.0, 400.0, TRAVEL),
            None,
            "already inside once the content has travelled"
        );
    }

    #[test]
    fn a_box_below_the_fold_comes_to_rest_against_the_bottom_of_the_viewport() {
        let step = reveal_step((940.0, 980.0), (0.0, 400.0), 400.0, 0.0, TRAVEL).unwrap();
        assert_eq!(step.to, 580.0, "980 - 400");
        assert_eq!(step.extra, 0.0, "nothing occludes, so nothing is owed");
    }

    #[test]
    fn a_box_above_the_viewport_comes_to_rest_against_its_top() {
        let step = reveal_step((100.0, 140.0), (0.0, 400.0), 400.0, 500.0, TRAVEL).unwrap();
        assert_eq!(step.to, 100.0);
        assert_eq!(step.extra, 0.0);
    }

    /// A docked keyboard takes the bottom 250 DIPs off a 400-DIP viewport. The last field in
    /// the content has to stand 230 DIPs past the maximum the content's own length states.
    /// A keyboard takes the bottom 250 DIPs of a 400-DIP viewport. The content can travel 250
    /// further than its own length allows for as long as it does, which is exactly what the
    /// last field in that content needs to be seen.
    #[test]
    fn a_docked_occlusion_lends_the_content_what_it_took_off_the_viewport() {
        let step = reveal_step((940.0, 980.0), (0.0, 400.0), 150.0, 0.0, TRAVEL).unwrap();
        assert_eq!(step.extra, 250.0, "400 - 150");
        assert_eq!(
            step.to, 830.0,
            "980 - 150, and inside the 850 now reachable"
        );
    }

    /// The extent is the occlusion's, so the same occlusion asks for the same extent wherever
    /// the content stands. Without that a reveal would compound its own last answer.
    #[test]
    fn the_extent_is_the_occlusions_and_not_the_contents_position() {
        for at in [0.0, 300.0, 600.0, 850.0] {
            let step = reveal_step((940.0, 980.0), (0.0, 400.0), 150.0, at, TRAVEL);
            assert!(step.is_none_or(|step| step.extra == 250.0), "at {at}");
        }
    }

    /// An occlusion that goes away lends nothing, so a content standing past its own maximum is
    /// asked for a position inside that maximum rather than for the one that cleared the
    /// occlusion.
    ///
    /// A box the lift left fully inside the viewport asks for nothing at all; what brings the
    /// content back there is the extent being given up, which shortens the range the tracker
    /// rests in.
    #[test]
    fn clearing_an_occlusion_clamps_what_it_asks_for_into_the_contents_own_range() {
        let lifted = reveal_step((940.0, 980.0), (0.0, 400.0), 400.0, 960.0, TRAVEL).unwrap();
        assert_eq!(lifted.extra, 0.0);
        assert_eq!(lifted.to, TRAVEL, "not the 940 that would clear nothing");
        assert_eq!(
            reveal_step((940.0, 980.0), (0.0, 400.0), 400.0, 850.0, TRAVEL),
            None,
            "a box the lift left inside asks for nothing"
        );
    }

    #[test]
    fn an_occlusion_covering_the_whole_viewport_still_rests_against_its_top() {
        let step = reveal_step((940.0, 980.0), (0.0, 400.0), -50.0, 0.0, TRAVEL).unwrap();
        assert_eq!(step.extra, 400.0, "the whole viewport is owed");
        assert_eq!(
            step.to, 980.0,
            "the occluded bottom cannot rise above the top"
        );
    }

    #[test]
    fn a_container_whose_geometry_is_unpublished_is_not_clamped_into_a_range_it_has_not_stated() {
        let step = reveal_step((940.0, 980.0), (0.0, 400.0), 400.0, 0.0, f32::NAN).unwrap();
        assert_eq!(step.extra, 0.0);
        assert_eq!(step.to, 580.0);
    }

    #[test]
    fn a_reveal_never_asks_for_a_position_above_the_content() {
        let step = reveal_step((0.0, 40.0), (100.0, 400.0), 400.0, 0.0, TRAVEL).unwrap();
        assert_eq!(step.to, 0.0, "clamped, rather than asking for -100");
    }

    /// A viewport bigger than its content has no thumb, no travel and nothing to scroll.
    #[test]
    fn content_that_fits_has_no_scrollbar() {
        let g = geom(400.0, 200.0);
        assert!(!g.overflow());
        assert_eq!((g.max_scroll, g.thumb_h, g.travel), (0.0, 0.0, 0.0));
    }

    /// A very long document still gets a thumb big enough to grab, and its travel is corrected
    /// for that floor rather than running past the end of the track.
    #[test]
    fn a_long_document_keeps_a_grabbable_thumb_inside_its_track() {
        let g = geom(400.0, 100_000.0);
        assert!(g.overflow());
        assert!((g.thumb_h - THUMB_MIN_H).abs() < 1.0e-3);
        let track = 400.0 - 2.0 * THUMB_MARGIN;
        assert!(g.travel >= 0.0 && g.thumb_h + g.travel <= track + 1.0e-3);
    }

    /// The two thumb functions are inverses over the whole travel, which is what lets a grab
    /// start from the value the compositor is already rendering.
    #[test]
    fn the_thumb_maps_both_ways() {
        let g = geom(400.0, 2000.0);
        for step in 0..=10u8 {
            let scroll = g.max_scroll * f32::from(step) / 10.0;
            let back = scroll_for_thumb_y(thumb_y_for_scroll(scroll, g), g);
            assert!((back - scroll).abs() < 0.01, "{scroll} → {back}");
        }
    }

    /// The affine the compositor evaluates agrees with the function a grab is resolved through
    /// at every point of the travel.
    #[test]
    fn the_thumb_affine_is_the_map_a_grab_uses() {
        let g = geom(400.0, 2000.0);
        let affine = g.affine();
        for step in 0..=10u8 {
            let scroll = g.max_scroll * f32::from(step) / 10.0;
            let bound = scroll * affine.m + affine.c;
            assert!(
                (bound - thumb_y_for_scroll(scroll, g)).abs() < 1.0e-3,
                "{scroll}"
            );
        }
        assert_eq!(g.size().x, THUMB_W);
        assert_eq!(g.size().y, g.thumb_h);
        assert_eq!(g.offset(300.0).y, THUMB_MARGIN);
        assert_eq!(g.offset(300.0).x, 300.0 - THUMB_W - THUMB_MARGIN);
    }

    /// With nothing to scroll, a dragged thumb asks for nothing rather than dividing by its own
    /// zero travel.
    #[test]
    fn a_thumb_with_no_travel_maps_to_the_top() {
        let g = geom(400.0, 200.0);
        assert_eq!(thumb_y_for_scroll(50.0, g), THUMB_MARGIN);
        assert_eq!(scroll_for_thumb_y(300.0, g), 0.0);
        assert_eq!(g.affine().m, 0.0);
    }

    /// An unsent geometry compares unequal to every real one, including itself, so the first
    /// publication needs no flag of its own.
    #[test]
    fn an_unsent_geometry_matches_nothing() {
        assert!(ThumbGeom::UNSENT != ThumbGeom::UNSENT);
        assert!(ThumbGeom::UNSENT != geom(400.0, 2000.0));
        assert!(ThumbGeom::UNSENT.max_scroll.is_nan());
    }

    /// Returns a hundred-row table, every row `extent` row heights tall and measured.
    fn uniform(extent: f32) -> Rows {
        table(&vec![extent; 100])
    }

    /// Returns a table of the extents given, in row heights, at a 20-DIP row.
    fn table(extents: &[f32]) -> Rows {
        let mut rows = Rows {
            unit: 20.0,
            ..Rows::default()
        };
        rows.take(&(0..extents.len() as u64).collect::<Vec<_>>(), 1.0);
        rows.extents.clear();
        rows.extents.extend_from_slice(extents);
        rows.reflow();
        rows
    }

    /// Returns a measured anchor table naming one row.
    fn measured(key: u64, top: f32, bottom: f32) -> Table {
        let mut table = Table::default();
        table.boxes.push(Anchored {
            key,
            rect: Rect {
                x0: 0.0,
                y0: top,
                x1: 100.0,
                y1: bottom,
            },
        });
        table
    }

    /// Returns how many rows a set realizes.
    fn realized_rows(set: &Realized) -> usize {
        set.runs().map(|run| run.len()).sum()
    }

    /// Returns whether a set realizes `index`.
    fn holds(set: &Realized, index: usize) -> bool {
        set.runs().any(|run| run.contains(&index))
    }

    /// Returns a position at `offset` in a viewport of `viewport_h`.
    fn at(offset: f32, viewport_h: f32) -> Pos {
        Pos {
            offset,
            viewport: viewport_h,
            ..Pos::default()
        }
    }

    /// Offsets accumulate the extents ahead of each row, and the search inverts them.
    ///
    /// Answered from the prefix table rather than by summing, so a list of mixed extents costs
    /// one lookup per row and one search per position however long it is.
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
        // Each row's own band answers itself at both edges, and the boundary belongs to the row
        // it opens.
        for (index, y) in [
            (0, 0.0),
            (0, 19.0),
            (1, 20.0),
            (1, 79.0),
            (2, 80.0),
            (3, 90.0),
        ] {
            assert_eq!(rows.at(y), index, "position {y}");
        }
        assert_eq!(
            rows.at(-50.0),
            0,
            "an overpan above the list answers its first row"
        );
        assert_eq!(rows.at(400.0), 3, "an overpan below it answers its last");
    }

    /// A measurement replaces the estimate, moves every row below it, and stays taken.
    #[test]
    fn a_measurement_moves_the_rows_below_it_and_is_kept() {
        let mut rows = Rows {
            unit: 20.0,
            ..Rows::default()
        };
        rows.take(&[0, 1, 2], 1.0);
        let one = measured(1, 20.0, 80.0);
        assert!(rows.needs(&one, 20.0));
        rows.write(&one, 20.0);
        assert!(!rows.needs(&one, 20.0), "the measurement did not land");
        assert!(rows.is_measured(1) && !rows.is_measured(0));
        assert_eq!(
            rows.offset(2),
            80.0,
            "the row below kept the estimate's offset"
        );
        assert_eq!(rows.total(), 100.0);
        // A key the table reports and the list does not hold is not an index into it.
        assert!(!rows.needs(&measured(99, 0.0, 40.0), 20.0));
    }

    /// A row that survives a reorder carries its measurement to its new place, and a row that
    /// arrives is the estimate.
    #[test]
    fn a_reorder_carries_each_measurement_with_its_row() {
        let mut rows = table(&[1.0, 3.0]);
        rows.take(&[1, 7, 0], 2.0);
        assert!(rows.is_measured(0) && rows.is_measured(2));
        assert!(
            !rows.is_measured(1),
            "a row that was not in the list was measured"
        );
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

    /// The window covers the viewport, plus the overscan on each side, and never runs past the
    /// ends.
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
        assert_eq!(
            window(60.0, 100.0, &rows, 0),
            3..4,
            "the open row did not take the viewport"
        );
        // The same viewport above it shows four.
        assert_eq!(window(0.0, 100.0, &rows, 0), 0..4);
        // And below it, where the rows are shut again, five.
        assert_eq!(window(260.0, 100.0, &rows, 0), 9..14);
    }

    /// A tracker overpans past the end of the content, and the window stays inside the list.
    ///
    /// The overpan is the bounce, so this position is reached by ordinary use. A window running
    /// past the last row underflows a row count: a debug panic, and in release a placement far
    /// past the end of the list.
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
        assert_eq!(realized_rows(&realize(&empty, at(0.0, 100.0), 2)), 0);
        let unsolved = Rows {
            unit: 0.0,
            ..table(&[1.0; 10])
        };
        assert_eq!(
            unsolved.at(500.0),
            0,
            "an unmeasured scale answers the first row"
        );
    }

    /// At rest the realized set is exactly the live window: one run, no corridor, nothing
    /// realized ahead of a fling that is not happening.
    #[test]
    fn a_resting_list_realizes_one_run() {
        let set = realize(&uniform(1.0), at(400.0, 100.0), SPEC.overscan);
        assert_eq!(set.runs().count(), 1);
        assert_eq!(set.runs().next().expect("one run"), 18..27);
    }

    /// A fling realizes where it lands as well as where it is, and the two are disjoint, which
    /// is why the set is not one range.
    #[test]
    fn a_long_fling_realizes_its_destination_and_a_bounded_corridor() {
        let flung = realize(
            &uniform(1.0),
            Pos {
                target: Some(1900.0),
                ..at(0.0, 100.0)
            },
            SPEC.overscan,
        );
        assert!(holds(&flung, 0), "where the content still is");
        assert!(holds(&flung, 99), "where it is going");
        assert!(!holds(&flung, 50), "and not the whole path between");
        assert!(flung.runs().count() > 1);
        // Two windows and two bands, and nothing that scales with the distance flung.
        assert!(realized_rows(&flung) <= 4 * (100 / 20 + 2 * SPEC.overscan + 2));
    }

    /// A short fling's destination overlaps the live window, and the two coalesce rather than
    /// realizing the same rows twice.
    #[test]
    fn a_short_fling_coalesces_into_one_run() {
        let nudged = realize(
            &uniform(1.0),
            Pos {
                target: Some(440.0),
                ..at(400.0, 100.0)
            },
            SPEC.overscan,
        );
        assert_eq!(nudged.runs().count(), 1);
        assert_eq!(nudged.runs().next().expect("one run"), 18..29);
    }

    /// A pinned row is realized wherever the content stands, and is its own run.
    ///
    /// What keeps a focused row on the tree: an unrealized row has no node, so it has nothing
    /// for the focus order to land on and no ring to draw.
    #[test]
    fn a_pinned_row_is_realized_from_anywhere() {
        let pinned = realize(
            &uniform(1.0),
            Pos {
                pin: Some(2),
                ..at(1900.0, 100.0)
            },
            SPEC.overscan,
        );
        assert!(holds(&pinned, 2), "the pinned row was left unrealized");
        assert!(holds(&pinned, 99), "the live window was dropped for it");
        assert_eq!(pinned.runs().count(), 2);
        let past = realize(
            &uniform(1.0),
            Pos {
                pin: Some(500),
                ..at(0.0, 100.0)
            },
            SPEC.overscan,
        );
        assert_eq!(
            past.runs().count(),
            1,
            "a pin past the end realized a row that is not there"
        );
    }

    /// The runs come out ascending and disjoint however they went in, because the fill walks
    /// them in order and a supplied item is matched by a single forward scan.
    #[test]
    fn the_runs_are_ascending_and_disjoint() {
        let flung = realize(
            &uniform(1.0),
            Pos {
                target: Some(0.0),
                pin: Some(50),
                ..at(1900.0, 100.0)
            },
            SPEC.overscan,
        );
        let mut last = 0;
        for run in flung.runs() {
            assert!(run.start >= last, "{run:?} after {last}");
            assert!(run.end > run.start);
            last = run.end;
        }
    }

    /// The band group's own offset is subtracted before the window is resolved, so a list under
    /// a heading realizes the rows the viewport is actually over.
    #[test]
    fn the_band_offset_moves_the_window_with_the_group() {
        let rows = uniform(1.0);
        let banded = realize(
            &rows,
            Pos {
                band_y: 400.0,
                ..at(400.0, 100.0)
            },
            SPEC.overscan,
        );
        assert_eq!(
            banded.runs().next().expect("one run"),
            window(0.0, 100.0, &rows, SPEC.overscan),
            "the group's own offset was not taken off the reported position"
        );
    }

    /// A reveal asks for the nearest position that brings the row inside the viewport, and asks
    /// for nothing where the row is already in it.
    #[test]
    fn a_reveal_moves_the_content_only_when_the_row_is_out_of_view() {
        let state = list_state();
        state.rows.update(|rows| {
            rows.unit = 20.0;
            rows.take(&(0..100).collect::<Vec<_>>(), 1.0);
        });
        state.edit(|at| at.offset = 400.0);

        state.reveal(50);
        assert_eq!(
            state.take_reveal(100.0),
            Some(920.0),
            "a row below the viewport comes to its bottom edge"
        );
        state.reveal(5);
        assert_eq!(
            state.take_reveal(100.0),
            Some(100.0),
            "a row above it comes to the top edge"
        );
        state.reveal(21);
        assert_eq!(
            state.take_reveal(100.0),
            None,
            "a visible row moves nothing"
        );
        assert_eq!(state.take_reveal(100.0), None, "the request was not taken");
        state.reveal(1_000);
        assert_eq!(
            state.take_reveal(100.0),
            None,
            "a row that has left the list moved the content"
        );
    }

    /// The extent a list is laid out at climbs while a manipulation is live and never steps
    /// back under a moving finger.
    #[test]
    fn a_held_extent_never_shortens_under_an_interaction() {
        let state = list_state();
        state.rows.update(|rows| {
            rows.unit = 20.0;
            rows.take(&(0..10).collect::<Vec<_>>(), 1.0);
        });
        assert_eq!(state.extent(), 10.0);
        state.edit(|at| at.held = 10.0);
        state
            .rows
            .update(|rows| rows.take(&(0..4).collect::<Vec<_>>(), 1.0));
        assert_eq!(
            state.extent(),
            10.0,
            "the content shortened mid-interaction"
        );
        state.edit(|at| at.held = 0.0);
        assert_eq!(state.extent(), 4.0, "the hold outlived the interaction");
    }

    /// A flush that re-states the viewport it already holds moves the position signal nothing.
    ///
    /// The write is what wakes the realization window, and that wake is what keeps the app
    /// thread from parking: a `Pos` whose `NaN` target compared unequal to itself would bump on
    /// every flush forever.
    #[test]
    fn restating_the_same_viewport_does_not_move_the_position() {
        let state = list_state();
        assert!(state.pos().target.is_none(), "a fresh list is at rest");
        state.resized(400.0);
        let moved = state.pos.version();
        state.resized(400.0);
        assert_eq!(
            state.pos.version(),
            moved,
            "an unchanged viewport bumped the position signal, which wakes the app on every flush"
        );
        state.resized(401.0);
        assert_ne!(
            state.pos.version(),
            moved,
            "a viewport that really moved did not reach the signal"
        );
    }

    // ── the two halves of a container ────────────────────────────────────────────

    /// Returns a container's app-side row, with `observe` deciding whether its tracker's
    /// reports are owed upstream.
    fn row(index: u32, observed: bool) -> ScrollRow {
        let node = NodeId::raw(index, 1);
        ScrollRow {
            front: ScrollFront {
                viewport: node,
                tracker: TrackerId::new(Id::raw(index, 1)),
                hover: ControlId::raw(index, 1),
                grab: ControlId::raw(index + 100, 1),
            },
            last: ThumbGeom::UNSENT,
            last_w: f32::NAN,
            node,
            content: NodeId::raw(index + 200, 1),
            rail: NodeId::raw(index + 300, 1),
            thumb: SpriteId(NodeId::raw(index + 100, 1)),
            reveal: Reveal::OnDemand,
            created: true,
            state: observed.then(list_state),
        }
    }

    #[test]
    fn keyboard_scroll_uses_the_focused_viewport_and_keeps_repeats_relative() {
        let mut table = ScrollTable::default();
        let mut ops = Vec::new();
        row(1, false).publish(geom(200.0, 1200.0), &mut ops);
        row(2, false).publish(geom(400.0, 2400.0), &mut ops);
        table.apply_ops(&mut ops);
        let target = table.rows[1].of.hover;
        let tracker = table.rows[1].of.tracker;
        let entry = windows_scene::HitEntry {
            x0: 0.0, y0: 70.0, x1: 300.0, y1: 470.0,
            touch_inflate: 0.0,
            clip_parent: windows_scene::NO_ENTRY,
            parent: windows_scene::NO_ENTRY,
            flags: HitFlags::INTERACTIVE.union(HitFlags::SCROLL),
            scroll_src: NodeId::NONE,
            id: target,
        };
        let mut hits = HitTable::default();
        hits.replace(&[entry], &[(target, 0)]);
        let event = KeyEvent { kind: KeyKind::Down, key: VK_DOWN as u16, repeat: false, mods: Default::default() };
        for (key, y) in [(VK_UP, -40.0), (VK_DOWN, 40.0), (VK_PRIOR, -400.0), (VK_NEXT, 400.0)] {
            for repeat in [false, true] {
                assert_eq!(table.key_request(target, KeyEvent { key: key as u16, repeat, ..event }, &hits),
                    Some((tracker, TrackerRequest::By(Vector2 { x: 0.0, y }))));
            }
        }
        for (key, y) in [(VK_HOME, 0.0), (VK_END, 2000.0)] {
            assert_eq!(table.key_request(target, KeyEvent { key: key as u16, ..event }, &hits),
                Some((tracker, TrackerRequest::To(Vector2 { x: 0.0, y }))));
        }
        table.rows[1].extended = 50.0;
        assert_eq!(table.key_request(target, KeyEvent { key: VK_END as u16, ..event }, &hits),
            Some((tracker, TrackerRequest::To(Vector2 { x: 0.0, y: 2050.0 }))));
        for kind in [KeyKind::Up, KeyKind::Char] {
            assert_eq!(table.key_request(target, KeyEvent { kind, ..event }, &hits), None);
        }
        for mods in [crate::input::Mods { ctrl: true, ..Default::default() }, crate::input::Mods { alt: true, ..Default::default() }] {
            assert_eq!(table.key_request(target, KeyEvent { mods, ..event }, &hits), None);
        }
        assert_eq!(table.key_request(table.rows[0].of.hover, event, &hits), None);
        assert_eq!(table.key_request(table.rows[1].of.grab, event, &hits), None);
        hits.replace(&[windows_scene::HitEntry { flags: HitFlags::SCROLL, ..entry }], &[(target, 0)]);
        assert_eq!(table.key_request(target, event, &hits), None);
        hits.replace(&[entry], &[(target, 0)]);
        table.rows[1].last = geom(400.0, 300.0);
        table.rows[1].extended = 0.0;
        assert_eq!(table.key_request(target, event, &hits), None);
        table.rows.clear();
        assert_eq!(table.key_request(target, event, &hits), None);
    }

    /// A container reaches the table that moves its thumb once, and leaves it when it unmounts.
    ///
    /// The add is emitted before the geometry gate, so a container whose content fits is in the
    /// table too: its thumb still has a reveal, and a row that never arrived would leave every
    /// hover over that surface acting on nothing.
    #[test]
    fn a_container_is_added_once_and_dropped_when_it_unmounts() {
        let mut held = row(1, false);
        let mut table = ScrollTable::default();
        let mut ops = Vec::new();

        held.publish(geom(400.0, 200.0), &mut ops);
        assert_eq!(
            ops.len(),
            2,
            "the arrival and its geometry did not both cross"
        );
        assert!(
            matches!(ops[0], ScrollOp::Add { .. }),
            "the add was not first"
        );
        table.apply_ops(&mut ops);
        assert_eq!(table.rows.len(), 1);
        assert!(ops.is_empty(), "the batch was not drained");

        held.publish(geom(400.0, 200.0), &mut ops);
        assert!(ops.is_empty(), "a settled container emitted an op");
        table.apply_ops(&mut ops);
        assert_eq!(table.rows.len(), 1, "a settled container was added twice");

        held.publish(geom(200.0, 4000.0), &mut ops);
        assert_eq!(
            ops.len(),
            1,
            "a moved extent emitted more than its geometry"
        );
        table.apply_ops(&mut ops);
        assert_eq!(
            table.rows[0].last,
            geom(200.0, 4000.0),
            "the table kept the geometry the old viewport gave it"
        );

        ops.push(ScrollOp::Drop {
            viewport: held.front.viewport,
        });
        table.apply_ops(&mut ops);
        assert!(
            table.rows.is_empty(),
            "the unmounted container kept its row"
        );
    }

    /// Only a scroll with application-owned state needs its tracker's reports upstream.
    ///
    /// Scene-side hit offsets, request completion and thumb reveal are already serviced, and an
    /// unknown tracker stays available to every other consumer.
    #[test]
    fn only_virtual_lists_forward_tracker_reports_to_the_app() {
        let mut table = ScrollTable::default();
        let mut ops = Vec::new();
        row(1, false).publish(geom(200.0, 4000.0), &mut ops);
        table.apply_ops(&mut ops);
        let quiet = table.rows[0].of.tracker.id();
        let unknown = TrackerId::<Observed>::new(Id::raw(9, 1)).id();

        let events = |tracker| {
            [
                SceneEvent::TrackerValues {
                    tracker: TrackerId::new(tracker),
                    position: Vector2 { x: 0.0, y: 100.0 },
                    scale: 1.0,
                },
                SceneEvent::TrackerPhase {
                    tracker: TrackerId::new(tracker),
                    phase: TrackerPhase::Idle,
                },
                SceneEvent::InertiaBegan {
                    tracker: TrackerId::new(tracker),
                    rest: Vector2 { x: 0.0, y: 0.0 },
                    from_wheel: false,
                },
                SceneEvent::RequestIgnored {
                    tracker: TrackerId::new(tracker),
                    request: 1,
                },
            ]
        };
        assert!(events(quiet).iter().all(|e| !table.app_observes(e)));
        assert!(
            events(unknown).iter().all(|e| table.app_observes(e)),
            "an unknown tracker was withheld from its other consumers"
        );
        assert!(table.app_observes(&SceneEvent::DeviceRebuilt));

        let mut ops = Vec::new();
        let mut virtualized = row(2, true);
        virtualized.publish(geom(200.0, 4000.0), &mut ops);
        table.apply_ops(&mut ops);
        let watched = table.rows[1].of.tracker.id();
        assert!(events(watched).iter().all(|e| table.app_observes(e)));
    }

    /// A thumb declared `Always` is opaque from its mount, so the first edge that would show it
    /// again emits nothing.
    #[test]
    fn an_always_shown_thumb_starts_shown() {
        let mut table = ScrollTable::default();
        let mut ops = Vec::new();
        let mut always = row(1, false);
        always.reveal = Reveal::Always;
        always.publish(geom(200.0, 4000.0), &mut ops);
        table.apply_ops(&mut ops);
        assert!(table.rows[0].shown);

        let mut on_demand = row(2, false);
        on_demand.publish(geom(200.0, 4000.0), &mut ops);
        table.apply_ops(&mut ops);
        assert!(!table.rows[1].shown, "an on-demand thumb started shown");
    }

    /// The reason to show a thumb ending does not take the thumb with it: the fade carries the
    /// hold, and a reason arriving inside the hold binds the channel again, which replaces the
    /// fade rather than letting it run.
    #[test]
    fn a_concealed_thumb_holds_before_it_fades_and_a_new_reason_replaces_the_fade() {
        let mut table = ScrollTable::default();
        let mut ops = Vec::new();
        let mut held = row(1, false);
        held.publish(geom(200.0, 4000.0), &mut ops);
        table.apply_ops(&mut ops);
        let live = &mut table.rows[0];

        assert_eq!(
            reveal_bind(live, true),
            Some(Bind::Animate(Anim::Spring {
                to: Value::Scalar(1.0),
                tuning: Tuning::Chrome,
                delay_ms: 0,
            })),
            "a thumb was not lit the moment its reason arrived"
        );
        assert_eq!(
            reveal_bind(live, false),
            Some(Bind::Animate(Anim::Spring {
                to: Value::Scalar(0.0),
                tuning: Tuning::Chrome,
                delay_ms: CONCEAL_MS,
            })),
            "the fade carried no hold, so the thumb left with its reason"
        );
        assert_eq!(
            reveal_bind(live, false),
            None,
            "a second conceal restarted the hold"
        );
        assert_eq!(
            reveal_bind(live, true),
            Some(Bind::Animate(Anim::Spring {
                to: Value::Scalar(1.0),
                tuning: Tuning::Chrome,
                delay_ms: 0,
            })),
            "a reason inside the hold did not replace the pending fade"
        );
    }

    /// A grab outlives the pointer leaving the thumb, so the reason it holds is not the hover's.
    #[test]
    fn a_grabbed_thumb_states_no_fade_however_the_pointer_wanders() {
        let mut table = ScrollTable::default();
        let mut ops = Vec::new();
        let mut held = row(1, false);
        held.publish(geom(200.0, 4000.0), &mut ops);
        table.apply_ops(&mut ops);
        let live = &mut table.rows[0];

        _ = reveal_bind(live, true);
        live.grab_from = 0.0;
        assert_eq!(reveal_bind(live, false), None, "a grabbed thumb faded");
        live.grab_from = f32::NAN;
        assert!(
            matches!(
                reveal_bind(live, false),
                Some(Bind::Animate(Anim::Spring {
                    delay_ms: CONCEAL_MS,
                    ..
                }))
            ),
            "a released grab did not leave the thumb on its hold"
        );
    }
}
