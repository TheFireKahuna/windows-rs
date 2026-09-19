//! The authored layout vocabulary: lengths, tracks, positions and the per-node [`Layout`].
//!
//! Nothing here resolves a value. A [`Len`] built from a [`Metric`] carries the index of that
//! metric's row in the host's per-class table, so the solver resolves one with an index and a
//! multiply rather than a call into the application's palette.

use crate::role::{Metric, Scope, ScopedToken, WidthClass, metric};
use core::cell::RefCell;

/// No pooled row: the node states no track template.
pub(crate) const NO_TRACKS: u32 = u32::MAX;

/// How many tracks one inline template holds.
pub const TRACK_CAP: usize = 8;

/// The widest occupancy word a grid auto-placement walks, and so the most columns a grid
/// places into.
pub const COLUMN_CAP: usize = 16;

// ── lengths ─────────────────────────────────────────────────────────────────────────

/// What a [`Len`] is measured in.
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Kind {
    /// Not authored. The niche that keeps every layout field out of an `Option`.
    Unset,
    Zero,
    /// Sized by content.
    Auto,
    /// `n` of the metric at `token`.
    Metric,
    /// `n` of the containing block.
    Pct,
    /// `n` device-independent pixels.
    Dip,
}

/// No metric is subtracted from the resolved length.
const NO_SUB: u8 = u8::MAX;

/// A length in the authoring surface.
///
/// Opaque, and eight bytes: only the solver reads one. There is no raw-DIP constructor on the
/// authoring path — [`Len::dip`] exists for the two host-owned lengths that come from the
/// window rather than from the palette — so a spacing is always the theme's.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Len {
    kind: Kind,
    /// The metric subtracted from the resolved value, or [`NO_SUB`].
    sub: u8,
    /// The metric's row in the host's per-class table, or a registered token's index.
    token: u16,
    n: f32,
}

impl Len {
    /// Exactly zero. A floor of zero is a layout instruction rather than a spacing.
    pub const ZERO: Self = Self::bare(Kind::Zero);
    /// Sized by content.
    pub const AUTO: Self = Self::bare(Kind::Auto);
    /// Not authored.
    pub const UNSET: Self = Self::bare(Kind::Unset);

    const fn bare(kind: Kind) -> Self {
        Self {
            kind,
            sub: NO_SUB,
            token: 0,
            n: 0.0,
        }
    }

    /// A fraction of the containing block, `0.0..=1.0`.
    #[must_use]
    pub const fn pct(p: f32) -> Self {
        Self {
            kind: Kind::Pct,
            sub: NO_SUB,
            token: 0,
            n: p,
        }
    }

    /// A raw length in device-independent pixels.
    ///
    /// The window's own extent and an overlay's viewport cap, which come from the display
    /// rather than from the palette.
    #[must_use]
    pub const fn dip(v: f32) -> Self {
        Self {
            kind: Kind::Dip,
            sub: NO_SUB,
            token: 0,
            n: v,
        }
    }

    /// `n` of `m`, end to end.
    ///
    /// Not a `const fn`: [`Metric::Custom`] registers its token on the way in, which the const
    /// evaluator cannot run. A `const` site states [`Len::dip`] or [`Len::pct`].
    #[must_use]
    pub fn times(m: Metric, n: f32) -> Self {
        Self {
            kind: Kind::Metric,
            sub: NO_SUB,
            token: index_of(m),
            n,
        }
    }

    /// This length less one metric, floored at zero.
    ///
    /// `m` must resolve to an index below [`NO_SUB`], which holds for every builtin metric and
    /// for the first 254 registered tokens.
    #[must_use]
    pub fn less(self, m: Metric) -> Self {
        let index = index_of(m);
        let sub = u8::try_from(index).unwrap_or(NO_SUB);
        debug_assert!(sub != NO_SUB, "a subtracted metric must index below 255");
        Self { sub, ..self }
    }

    /// Returns whether the field this length sits in was authored.
    #[must_use]
    pub const fn is_set(self) -> bool {
        !matches!(self.kind, Kind::Unset)
    }

    /// Returns whether this length is content-sized rather than stated.
    #[must_use]
    pub const fn is_auto(self) -> bool {
        matches!(self.kind, Kind::Auto | Kind::Unset)
    }

    /// Returns whether this length is a fraction of its containing block.
    #[must_use]
    pub const fn is_pct(self) -> bool {
        matches!(self.kind, Kind::Pct)
    }

    /// Resolves to DIPs, or `None` where nothing here states a number.
    ///
    /// `basis` is the containing block's extent on this axis, and a non-finite one leaves a
    /// percentage unresolved. `rows` is the host's metric table and `scope` the scope a
    /// registered token resolves against; a builtin metric is one index and one multiply.
    #[must_use]
    pub(crate) fn resolve(
        self,
        rows: &[[f32; crate::role::BUILTIN_METRICS]; 3],
        class: WidthClass,
        scope: Scope,
        basis: f32,
    ) -> Option<f32> {
        let base = match self.kind {
            Kind::Unset | Kind::Auto => return None,
            Kind::Zero => 0.0,
            Kind::Dip => self.n,
            Kind::Pct => {
                if !basis.is_finite() {
                    return None;
                }
                basis * self.n
            }
            Kind::Metric => row(rows, class, scope, self.token) * self.n,
        };
        if self.sub == NO_SUB {
            return Some(base);
        }
        Some((base - row(rows, class, scope, u16::from(self.sub))).max(0.0))
    }

    /// Resolves this length in DIPs against `scope`, away from the solve.
    ///
    /// The authority path: a metric resolves through the palette rather than out of the host's
    /// cached table, so a caller holding a scope and no solve answers the same number. A
    /// percentage has no containing block here and a content-sized length has no content, so
    /// both answer zero.
    #[must_use]
    pub fn dips(self, scope: Scope) -> f32 {
        let base = match self.kind {
            Kind::Unset | Kind::Auto | Kind::Pct | Kind::Zero => 0.0,
            Kind::Dip => self.n,
            Kind::Metric => named(self.token).map_or(0.0, |m| metric(m, scope)) * self.n,
        };
        if self.sub == NO_SUB {
            return base;
        }
        let less = named(u16::from(self.sub)).map_or(0.0, |m| metric(m, scope));
        (base - less).max(0.0)
    }
}

/// Returns the metric `index` names, or `None` where nothing is registered under it.
///
/// Only [`index_of`] mints an index, so the absent case is unreachable and answers nothing
/// rather than a plausible substitute.
fn named(index: u16) -> Option<Metric> {
    let index = usize::from(index);
    if index < crate::role::BUILTIN_METRICS {
        return Some(Metric::BUILTIN[index]);
    }
    let at = index - crate::role::BUILTIN_METRICS;
    let held = CUSTOM.with_borrow(|held| held.get(at).copied());
    debug_assert!(
        held.is_some(),
        "a Len names a metric that was never registered"
    );
    held.map(Metric::Custom)
}

impl From<Metric> for Len {
    fn from(m: Metric) -> Self {
        Self::times(m, 1.0)
    }
}

/// Returns the metric at `index` in DIPs.
///
/// An index below [`crate::role::BUILTIN_METRICS`] reads the host's table, which `set_theme`
/// fills by calling [`metric`] itself; anything above it is a registered token, which is
/// called here for the same reason the table exists — the palette is the authority and the
/// table is its cache.
fn row(
    rows: &[[f32; crate::role::BUILTIN_METRICS]; 3],
    class: WidthClass,
    scope: Scope,
    index: u16,
) -> f32 {
    let index = usize::from(index);
    if index < crate::role::BUILTIN_METRICS {
        return rows[class as usize][index];
    }
    custom(index - crate::role::BUILTIN_METRICS, scope.at_width(class))
}

thread_local! {
    /// The tokens a [`Len`] names by index, in registration order.
    ///
    /// Thread-local because a host is: a `Layout` is authored and solved on the one app
    /// thread, and an index minted on another would name a different token.
    static CUSTOM: RefCell<Vec<&'static ScopedToken<f32>>> = const { RefCell::new(Vec::new()) };
}

/// Returns `token`'s index, registering it the first time it is seen.
///
/// Idempotent by pointer identity, and stable for the life of the thread's host. The scan is
/// linear over the application's distinct tokens, and it runs where a length is authored
/// rather than where one is solved.
fn custom_index(token: &'static ScopedToken<f32>) -> u16 {
    CUSTOM.with_borrow_mut(|held| {
        let at = held
            .iter()
            .position(|other| core::ptr::eq(*other, token))
            .unwrap_or_else(|| {
                held.push(token);
                held.len() - 1
            });
        let index = crate::role::BUILTIN_METRICS + at;
        u16::try_from(index).expect("a host registers fewer than 65536 custom metrics")
    })
}

/// Returns what the token registered at `at` resolves to in `scope`.
fn custom(at: usize, scope: Scope) -> f32 {
    CUSTOM.with_borrow(|held| {
        held.get(at)
            .map_or(0.0, |token| metric(Metric::Custom(token), scope))
    })
}

/// Returns `m`'s index: its row in the host's metric table, or a registered token's.
fn index_of(m: Metric) -> u16 {
    match m.row() {
        Some(row) => row as u16,
        None => match m {
            Metric::Custom(token) => custom_index(token),
            _ => unreachable!("only a custom metric has no cache row"),
        },
    }
}

// ── tracks ──────────────────────────────────────────────────────────────────────────

/// How large a track may grow.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TrackMax {
    /// A stated length.
    Len(Len),
    /// A share of what is left, weighted.
    Fr(f32),
    /// The widest its content wants.
    MaxContent,
    /// The narrowest its content can be.
    MinContent,
}

/// One grid track: what it cannot go below, and what it may grow to.
///
/// Separate from [`Len`] because `fr` is a length only inside a track.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Track {
    pub min: Len,
    pub max: TrackMax,
}

impl Track {
    /// A track of exactly `len`.
    #[must_use]
    pub const fn fixed(len: Len) -> Self {
        Self {
            min: len,
            max: TrackMax::Len(len),
        }
    }

    /// A share of the leftover space, never less than its content's minimum. A track that may
    /// go below its content states [`Track::min_fr`] with [`Len::ZERO`].
    #[must_use]
    pub const fn fr(n: f32) -> Self {
        Self {
            min: Len::AUTO,
            max: TrackMax::Fr(n),
        }
    }

    /// At least `min`, and a share of what is left.
    #[must_use]
    pub const fn min_fr(min: Len, n: f32) -> Self {
        Self {
            min,
            max: TrackMax::Fr(n),
        }
    }

    /// At least `min` and at most `max`. The minimum wins where the maximum resolves below it.
    #[must_use]
    pub const fn bounded(min: Len, max: Len) -> Self {
        Self {
            min,
            max: TrackMax::Len(max),
        }
    }

    /// Sized by content: at least its narrowest, at most its widest.
    pub const AUTO: Self = Self {
        min: Len::AUTO,
        max: TrackMax::MaxContent,
    };
    /// As narrow as its content can be.
    pub const MIN: Self = Self {
        min: Len::AUTO,
        max: TrackMax::MinContent,
    };
    /// As wide as its content wants.
    pub const MAX: Self = Self {
        min: Len::AUTO,
        max: TrackMax::MaxContent,
    };
}

impl From<Metric> for Track {
    fn from(m: Metric) -> Self {
        Self::fixed(Len::from(m))
    }
}

impl From<Len> for Track {
    fn from(l: Len) -> Self {
        Self::fixed(l)
    }
}

/// One axis's track list.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Template {
    Inline {
        len: u8,
        tracks: [Track; TRACK_CAP],
    },
    /// A list the enclosing scope resolves, so a column count that changes with width class
    /// is one token rather than a row of variants.
    Ladder(&'static ScopedToken<&'static [Track]>),
}

impl Template {
    /// The template a container with no track list carries.
    pub const NONE: Self = Self::Inline {
        len: 0,
        tracks: [Track::AUTO; TRACK_CAP],
    };

    /// Calls `f` with this template's tracks, resolved through `scope`.
    pub(crate) fn with<R>(&self, scope: Scope, f: impl FnOnce(&[Track]) -> R) -> R {
        match self {
            Self::Inline { len, tracks } => f(&tracks[..usize::from(*len)]),
            Self::Ladder(token) => f(token.resolve(scope)),
        }
    }
}

/// Both axes' track lists, pooled behind [`Layout::tracks`].
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Templates {
    pub cols: Template,
    pub rows: Template,
}

impl Templates {
    const NONE: Self = Self {
        cols: Template::NONE,
        rows: Template::NONE,
    };

    /// Calls `f` with the templates `held` names, or answers `None` where it names none.
    pub(crate) fn with<R>(held: u32, f: impl FnOnce(&Self) -> R) -> Option<R> {
        if held == NO_TRACKS {
            return None;
        }
        POOL.with_borrow(|pool| pool.templates.get(held as usize).map(f))
    }
}

/// The templates, interned by content: a node re-authoring the same tracks names the row it
/// already had, so a template outlives the node that stated it and the table grows with the
/// application's distinct templates rather than with its node count.
#[derive(Default)]
struct Pool {
    templates: Vec<Templates>,
}

thread_local! {
    static POOL: RefCell<Pool> = RefCell::new(Pool::default());
}

/// Returns the handle for `held`'s templates with `write` applied.
fn edit(held: u32, write: impl FnOnce(&mut Templates)) -> u32 {
    POOL.with_borrow_mut(|pool| {
        let mut next = pool
            .templates
            .get(held as usize)
            .copied()
            .unwrap_or(Templates::NONE);
        write(&mut next);
        if let Some(at) = pool.templates.iter().position(|held| *held == next) {
            return at as u32;
        }
        pool.templates.push(next);
        (pool.templates.len() - 1) as u32
    })
}

impl Template {
    /// Returns `tracks` as an inline list. At most [`TRACK_CAP`] are kept; a longer list is
    /// truncated.
    fn inline(tracks: impl IntoIterator<Item = Track>) -> Self {
        let mut held = [Track::AUTO; TRACK_CAP];
        let mut len = 0u8;
        for track in tracks {
            let at = usize::from(len);
            if at == TRACK_CAP {
                debug_assert!(false, "a track template holds at most {TRACK_CAP} tracks");
                break;
            }
            held[at] = track;
            len += 1;
        }
        Self::Inline { len, tracks: held }
    }
}

impl Layout {
    /// States this grid's columns.
    pub fn set_cols(&mut self, tracks: impl IntoIterator<Item = Track>) {
        let cols = Template::inline(tracks);
        self.tracks = edit(self.tracks, |t| t.cols = cols);
    }

    /// States this grid's rows.
    pub fn set_rows(&mut self, tracks: impl IntoIterator<Item = Track>) {
        let rows = Template::inline(tracks);
        self.tracks = edit(self.tracks, |t| t.rows = rows);
    }

    /// States this grid's columns as `token`'s ladder, resolved through the scope at solve.
    pub fn set_cols_by(&mut self, token: &'static ScopedToken<&'static [Track]>) {
        self.tracks = edit(self.tracks, |t| t.cols = Template::Ladder(token));
    }

    /// States this grid's rows as `token`'s ladder.
    pub fn set_rows_by(&mut self, token: &'static ScopedToken<&'static [Track]>) {
        self.tracks = edit(self.tracks, |t| t.rows = Template::Ladder(token));
    }
}

// ── geometry vocabulary ─────────────────────────────────────────────────────────────

/// An axis-aligned box in absolute layout DIPs.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    /// Returns the box with those four edges.
    #[must_use]
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    /// Returns the distance between the left and right edges.
    #[must_use]
    pub fn width(self) -> f32 {
        self.x1 - self.x0
    }

    /// Returns the distance between the top and bottom edges.
    #[must_use]
    pub fn height(self) -> f32 {
        self.y1 - self.y0
    }

    /// Returns the smallest box containing both.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    /// Returns whether `p` lies inside, right and bottom edges excluded.
    #[must_use]
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    /// Returns this box in `origin`'s own space.
    #[must_use]
    pub fn rebased(self, origin: Self) -> Self {
        Self {
            x0: self.x0 - origin.x0,
            y0: self.y0 - origin.y0,
            x1: self.x1 - origin.x0,
            y1: self.y1 - origin.y0,
        }
    }
}

/// One edge of a box.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Top,
    Right,
    Bottom,
}

impl Edge {
    /// Returns whether pinning to this edge takes the inline axis.
    #[must_use]
    pub const fn horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// How a container aligns all of its children.
///
/// Alignment is a container property; the per-child escape is
/// [`Element::align_self`](crate::build::Element::align_self), where [`Align::Stretch`] is the
/// value that defers to the container. A child that wants to span a cross axis its container
/// does not stretch states a percentage extent instead.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

impl Align {
    /// Returns the fraction of a node's own extent that lies before the point it aligns to.
    ///
    /// `None` for [`Align::Stretch`], which pins both edges and has no single point.
    #[must_use]
    pub const fn fraction(self) -> Option<f32> {
        match self {
            Self::Start => Some(0.0),
            Self::Center => Some(0.5),
            Self::End => Some(1.0),
            Self::Stretch => None,
        }
    }

    /// Returns where a node of `extent` sits in `room` under this alignment.
    #[must_use]
    pub fn offset(self, room: f32, extent: f32) -> f32 {
        match self {
            Self::Start | Self::Stretch => 0.0,
            Self::Center => (room - extent) * 0.5,
            Self::End => room - extent,
        }
    }
}

/// Where a node sits in its container. One field, so a grid cell and an edge pin cannot
/// both be stated.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Position {
    Flow,
    Cell {
        row: u16,
        col: u16,
        row_span: u16,
        col_span: u16,
    },
    /// Out of flow against one edge of the parent box, stretched across the other axis.
    Pin(Edge),
    /// Out of flow, spanning the parent's inline axis, with its leading block edge `at`
    /// below the parent's.
    Band {
        at: Len,
    },
    /// Out of flow over a normalized region of the parent box, aligned by the node's own
    /// measured extent.
    ///
    /// `at` is `[x0, y0, x1, y1]` as fractions of the parent's box; a point is the two edges
    /// of an axis stated equal. [`Align::Stretch`] spans the region and every other alignment
    /// keeps the measured extent and puts that much of it before the point.
    Anchor {
        at: [f32; 4],
        align: [Align; 2],
    },
}

impl Position {
    /// Returns whether this node takes room from its siblings.
    #[must_use]
    pub const fn in_flow(self) -> bool {
        matches!(self, Self::Flow | Self::Cell { .. })
    }
}

// ── the authored layout ─────────────────────────────────────────────────────────────

/// How a container arranges its children.
///
/// A preset is a row of [`Layout::DEFAULTS`] rather than a code path: a constructor writes
/// that row and every field stays overridable from the call site.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Preset {
    #[default]
    Stack,
    Row,
    Wrap,
    Grid,
    Layer,
    Scroll,
    Figure,
    Text,
}

impl Preset {
    /// Every preset, in the order [`Layout::DEFAULTS`] holds them.
    pub const ALL: [Self; 8] = [
        Self::Stack,
        Self::Row,
        Self::Wrap,
        Self::Grid,
        Self::Layer,
        Self::Scroll,
        Self::Figure,
        Self::Text,
    ];

    /// Returns whether children run along the inline axis.
    #[must_use]
    pub const fn inline_main(self) -> bool {
        matches!(self, Self::Row | Self::Wrap)
    }

    /// Returns the node flags this preset declares.
    ///
    /// `HIDDEN`, `CLIP`, `SCROLL` and `RESPONSIVE` have one home — the node's own flag word —
    /// so a preset that means one of them is mirrored there when the node is created, and the
    /// authored [`Layout`] carries none of the four.
    #[must_use]
    pub const fn node_flags(self) -> crate::build::tree::Bits {
        use crate::build::tree::{CLIP, SCROLL};
        match self {
            Self::Scroll => SCROLL | CLIP,
            _ => 0,
        }
    }
}

/// One node's authored layout.
///
/// `Copy`, `Send` and heap-free, so a setter writes a field and nothing allocates. Its
/// starting value is its preset's row of [`Layout::DEFAULTS`]; authored setters replace
/// fields in it, and that is the whole resolution order.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Layout {
    pub preset: Preset,
    /// [`Layout::NO_STRETCH`], and nothing else.
    ///
    /// Hiding, clipping, scrolling and classifying live in the node's own flag word, which is
    /// their one home and their one writer.
    pub flags: u16,
    /// How children are placed across the container's cross axis.
    pub align: Align,
    /// How they are placed along its main axis.
    pub justify: Align,
    /// This node's own exception to its container's `align`.
    ///
    /// [`Align::Stretch`] defers to the container, which is what makes the field statable
    /// without a second "was it authored" bit beside it.
    pub align_self: Align,
    /// The class at and below which a row runs the stack arm.
    ///
    /// [`WidthClass::Wide`] states no threshold, because a row that stacks at every class is
    /// a stack and is authored as one.
    pub stack_below: WidthClass,
    /// `[narrow_max, medium_max]`, meaningful where the node carries the responsive flag.
    pub bounds: [f32; 2],
    /// This node's share of the surplus, by weight. Zero, so growing is stated.
    pub grow: f32,
    /// Width over height, deriving whichever axis is unset. Zero states none.
    pub aspect: f32,
    /// The pooled [`Templates`] handle, or [`NO_TRACKS`].
    pub tracks: u32,
    pub position: Position,
    pub width: Len,
    pub height: Len,
    pub min_width: Len,
    pub min_height: Len,
    pub max_width: Len,
    pub max_height: Len,
    /// The accessible minimum a recipe gives a control on the block axis.
    ///
    /// Apart from `min_height` so the two can be told apart: a floor says how short this kind
    /// of control may be when nobody has said, and it applies only where neither `height` nor
    /// `min_height` was stated.
    pub floor: Len,
    pub gap: Len,
    /// `[inline, block]`, applied on both sides of each axis.
    pub padding: [Len; 2],
}

impl Layout {
    /// Takes its measured extent rather than its container's, however the container aligns.
    pub const NO_STRETCH: u16 = 1 << 0;

    /// The empty value the `layout` column is seated at.
    pub const DEFAULT: Self = Self::of(Preset::Layer);

    /// Returns `preset`'s row.
    #[must_use]
    pub const fn of(preset: Preset) -> Self {
        Self::DEFAULTS[preset as usize]
    }

    /// The window root's own layout. Its extent is written from window input.
    #[must_use]
    pub const fn window() -> Self {
        Self {
            width: Len::dip(0.0),
            height: Len::dip(0.0),
            ..Self::of(Preset::Stack)
        }
    }

    /// Returns whether `bit` is set.
    #[must_use]
    pub const fn has(self, bit: u16) -> bool {
        self.flags & bit != 0
    }

    /// Returns whether a row of this declaration runs the stack arm at `class`.
    #[must_use]
    pub const fn stacks_at(self, class: WidthClass) -> bool {
        !matches!(self.stack_below, WidthClass::Wide) && (class as u8) <= (self.stack_below as u8)
    }

    /// Returns the cross-axis alignment this node takes inside a container aligning
    /// `container`.
    #[must_use]
    pub const fn align_self_or(self, container: Align) -> Align {
        if matches!(self.align_self, Align::Stretch) {
            return container;
        }
        self.align_self
    }

    /// The preset rows, indexed by [`Preset`].
    ///
    /// **No preset states a gap, a padding or a minimum.** Space between children is the
    /// container's own declaration every time, so a container that means zero says so.
    pub const DEFAULTS: [Self; 8] = {
        const BARE: Layout = Layout {
            preset: Preset::Stack,
            flags: 0,
            align: Align::Stretch,
            justify: Align::Start,
            align_self: Align::Stretch,
            stack_below: WidthClass::Wide,
            bounds: [0.0, 0.0],
            grow: 0.0,
            aspect: 0.0,
            tracks: NO_TRACKS,
            position: Position::Flow,
            width: Len::UNSET,
            height: Len::UNSET,
            min_width: Len::UNSET,
            min_height: Len::UNSET,
            max_width: Len::UNSET,
            max_height: Len::UNSET,
            floor: Len::UNSET,
            gap: Len::UNSET,
            padding: [Len::UNSET, Len::UNSET],
        };
        [
            // Stack
            Layout { ..BARE },
            // Row
            Layout {
                preset: Preset::Row,
                align: Align::Center,
                justify: Align::Start,
                ..BARE
            },
            // Wrap
            Layout {
                preset: Preset::Wrap,
                align: Align::Center,
                justify: Align::Start,
                ..BARE
            },
            // Grid
            Layout {
                preset: Preset::Grid,
                align: Align::Stretch,
                justify: Align::Stretch,
                ..BARE
            },
            // Layer
            Layout {
                preset: Preset::Layer,
                align: Align::Stretch,
                justify: Align::Stretch,
                ..BARE
            },
            // Scroll. What it clips and what it scrolls are node flags, which
            // `Preset::node_flags` states and creation mirrors.
            Layout {
                preset: Preset::Scroll,
                ..BARE
            },
            // Figure
            Layout {
                preset: Preset::Figure,
                width: Len::pct(1.0),
                height: Len::pct(1.0),
                ..BARE
            },
            // Text
            Layout {
                preset: Preset::Text,
                flags: Layout::NO_STRETCH,
                ..BARE
            },
        ]
    };
}

impl Default for Layout {
    fn default() -> Self {
        Self::DEFAULT
    }
}

const _: () = {
    assert!(size_of::<Len>() == 8);
    assert!(size_of::<Layout>() <= 128);
};
