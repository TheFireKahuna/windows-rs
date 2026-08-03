//! The alphabet: everything a retained tree can draw. **App half.**
//!
//! One family, carried by the widget layer above and the patch below without conversion:
//! these are already plain `Send` data carrying ids and light.
//!
//! **If it animates it is a channel; if it identifies it is in the value.** A trim window,
//! a stroke width, a dash offset and a glow's blur animate, so they are bound properties
//! seeded by the mask or paint that introduced them. A cap style, a dash pattern and a
//! geometry id do not, so they live in the value and changing one rebuilds the mask. Two
//! sources of truth for one number is how a control animates the one nothing is bound to.

use crate::cache::GenMask;
use crate::id::Id;
use windows_color::Radiance;
use windows_numerics::Vector2;

/// A point in absolute layout DIPs.
pub type Point = Vector2;

// ── identity ────────────────────────────────────────────────────────────────────

/// The arena `SpriteId` and `GroupId` both index.
#[derive(Debug)]
pub struct Node;
/// Path geometry, in sprite-local DIPs.
#[derive(Debug)]
pub struct Geom;
/// A rasterized gradient strip.
#[derive(Debug)]
pub struct Ramp;
/// A shaped run's coverage tile.
#[derive(Debug)]
pub struct Run;
/// A buffer the application presents itself.
#[derive(Debug)]
pub struct Region;
/// A compositor-side interaction tracker.
#[derive(Debug)]
pub struct Tracker;
/// A pending timed reveal.
#[derive(Debug)]
pub struct Delay;
/// An interactive node, as the layer above mints it.
///
/// This crate never interprets one — it is the join between the flat hit array and whatever
/// table a consumer keeps beside it, and there are two such tables over the one family: the
/// app thread's handlers and the front thread's chrome.
#[derive(Debug)]
pub struct Control;
/// A measurement request, minted and interpreted by the layer that owns the text engine.
///
/// Named for the thing rather than the verb: `Measure` is the trait a layout tree is handed,
/// and one of the two would shadow the other wherever both are in scope.
#[derive(Debug)]
pub struct Measured;

/// Either kind of node.
pub type NodeId = Id<Node>;
pub type GeomId = Id<Geom>;
pub type RampId = Id<Ramp>;
pub type RunId = Id<Run>;
pub type RegionId = Id<Region>;

/// A pending timed reveal.
///
/// A submenu's hover-open and a tooltip's show are the only places in the system that want
/// "after N milliseconds", and a delay is a monotonic deadline compared on the frame the
/// scene is already servicing. **Not a fourth clock**: nothing fires and nothing wakes, and
/// the request that keeps the frame clock awake for its duration is the whole of its cost.
pub type DelayId = Id<Delay>;

/// A node that paints. The newtype is enforcement: a paint addressed to a group is refused
/// at the model's own API.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SpriteId(pub(crate) NodeId);

/// A node that positions and clips its children, and paints nothing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct GroupId(pub(crate) NodeId);

impl SpriteId {
    /// This sprite addressed as either kind of node.
    #[must_use]
    pub const fn node(self) -> NodeId {
        self.0
    }
}

impl GroupId {
    /// This group addressed as either kind of node.
    #[must_use]
    pub const fn node(self) -> NodeId {
        self.0
    }
}

impl From<SpriteId> for NodeId {
    fn from(id: SpriteId) -> Self {
        id.0
    }
}

impl From<GroupId> for NodeId {
    fn from(id: GroupId) -> Self {
        id.0
    }
}

/// Which composition object a node mints. The only place in the crate that branches on it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// A `ContainerVisual`.
    Group,
    /// A `SpriteVisual`.
    Sprite,
}

/// A resource slot. Untyped because the [`ResOp`] variant beside it names the family.
pub type ResId = Id<()>;

/// A shared resource a sprite is holding, with the family named.
///
/// The only join between the declaration alphabet and the resource tables: a mask or paint
/// says what it holds and lifetime accounting reads that. Adding a mask kind that holds
/// something is one arm of [`Mask::holds`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Holding {
    Geom(GeomId),
    Ramp(RampId),
    Run(RunId),
    Region(RegionId),
}

// ── the alphabet ────────────────────────────────────────────────────────────────

/// What shape a sprite has. **Alpha only, never colour.**
///
/// Colour lives on the paint: a composition shape's brushes are 8-bit `Windows.UI.Color`,
/// which cannot express a negative component or a value above white at all, and this
/// pipeline authors both. So the shape carries coverage, an FP16 surface carries light, and
/// a mask brush multiplies them — the only route to the palette, and why there is one
/// construction and not one per kind.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Mask {
    /// A rounded rectangle, stretched from a nine-grid atlas cell so that one raster
    /// serves any size with exact corners.
    Box { radius: Corners },
    /// One same-paint span of one shaped line, as a coverage tile. **One sprite per run**,
    /// whatever its glyph count: a per-glyph tree makes a paragraph cost dozens of visuals
    /// and drags all of them into every dirty region that touches the line.
    Run(RunId),
    /// Arbitrary geometry, in sprite-local DIPs. `stroke` picks fill or outline; the
    /// draw-on window animates, so it is a channel. Which construction realizes it is this
    /// crate's decision, not the author's.
    Shape {
        geom: GeomId,
        stroke: Option<StrokeStyle>,
    },
    /// No mask: the paint's own alpha is the shape. For a paint whose surface already
    /// carries the alpha profile — a backdrop layer, a glow blob, a presented buffer.
    None,
}

impl Mask {
    /// The shared resource this mask holds, if any.
    #[must_use]
    pub const fn holds(self) -> Option<Holding> {
        match self {
            Self::Run(id) => Some(Holding::Run(id)),
            Self::Shape { geom, .. } => Some(Holding::Geom(geom)),
            Self::Box { .. } | Self::None => None,
        }
    }

    /// Which invalidation generations a chain built from this mask reads.
    ///
    /// A *shared* resource reads none of them: re-rasterizing one moves the single brush
    /// every sprite already holds, so the sprite has nothing to rebuild. A shape is the
    /// exception, and it is not a shared resource that makes it one — the geometry is
    /// shared, but the **capture around it is per-sprite** and states its region in
    /// pixels, so a grid that moves leaves it describing the wrong number of them. A
    /// resize corrects that from the size itself; a DPI change carries no size, because
    /// the DIP rect did not move.
    #[must_use]
    pub const fn deps(self) -> GenMask {
        match self {
            Self::Box { .. } | Self::Shape { .. } => GenMask::GEOMETRY,
            Self::Run(_) | Self::None => GenMask::NONE,
        }
    }
}

/// What colour a sprite is: **authored scene light**, before the display transform.
///
/// Applied once, where the cell is rasterized, and inexpressible here — so the scene can
/// neither skip it nor apply it twice.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Paint {
    /// A flat colour. One small FP16 cell, shared by every sprite of that colour.
    Solid(Radiance),
    /// A gradient along an axis, as one premultiplied FP16 strip carrying colour *and*
    /// alpha. Minted per (stops, axis) and addressed by id, so a resize costs nothing.
    Ramp(RampId),
    /// A captured subtree, blurred and tinted — the glow layer. `blur` is the Gaussian's
    /// sigma in DIPs and animates, so a halo can breathe.
    Captured {
        group: GroupId,
        blur: f32,
        tint: Radiance,
    },
    /// A buffer the application renders and presents itself.
    Presented(RegionId),
}

impl Paint {
    /// The shared resource this paint holds, if any. A capture holds a *group*, whose
    /// lifetime is the node tree's.
    #[must_use]
    pub const fn holds(self) -> Option<Holding> {
        match self {
            Self::Ramp(id) => Some(Holding::Ramp(id)),
            Self::Presented(id) => Some(Holding::Region(id)),
            Self::Solid(_) | Self::Captured { .. } => None,
        }
    }

    /// Which invalidation generations a chain built from this paint reads.
    ///
    /// A solid carries authored light through the output transform, so it reads the light
    /// generation. A glow reads that *and* the grid: its tint goes through the same
    /// transform, and it is a capture, so it states a region in pixels for the reason
    /// [`Mask::deps`] gives. The two backed by a shared surface read neither.
    #[must_use]
    pub const fn deps(self) -> GenMask {
        match self {
            Self::Solid(_) => GenMask::LIGHT,
            Self::Captured { .. } => GenMask::LIGHT.union(GenMask::GEOMETRY),
            Self::Ramp(_) | Self::Presented(_) => GenMask::NONE,
        }
    }
}

/// Where a node is and how it is transformed. Every field is a sink.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Xform {
    /// DIPs, relative to the parent group.
    pub offset: Vector2,
    /// DIPs.
    pub size: Vector2,
    /// About `center`. The sink a tracker's scale axis lands on.
    pub scale: Vector2,
    /// **Radians**, about `center` — the unit the animation name space carries. There is
    /// deliberately no degrees twin: a 57× error reads as a broken control, not a unit bug.
    pub rotation: f32,
    /// Rotation and scale centre, in sprite-local DIPs.
    pub center: Vector2,
    /// `0..=1`.
    pub opacity: f32,
    pub clip: Clip,
}

impl Default for Xform {
    fn default() -> Self {
        Self {
            offset: Vector2 { x: 0.0, y: 0.0 },
            size: Vector2 { x: 0.0, y: 0.0 },
            scale: Vector2 { x: 1.0, y: 1.0 },
            rotation: 0.0,
            center: Vector2 { x: 0.0, y: 0.0 },
            opacity: 1.0,
            clip: Clip::None,
        }
    }
}

/// What a node's subtree may draw inside.
///
/// One rectangular clip type, the *absolute* one. An inset clip carries no radii, so every
/// rounded clip-to-bounds is already a rectangle clip — and `Inset{l,t,r,b}` over a node of
/// size `(w,h)` *is* `Rect{l, t, w−r, h−b}`, which the node's shadowed size supplies. One
/// object, one animation name space, and a reveal wipe is animated sides.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum Clip {
    #[default]
    None,
    Rect {
        l: f32,
        t: f32,
        r: f32,
        b: f32,
        radius: Corners,
    },
    Geom(GeomId),
}

/// Four corner radii, clockwise from the top left.
///
/// A radius is capped by the platform at half the box on each axis, so "fully rounded" is
/// a stadium and never a circle-ended football.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Corners {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Corners {
    /// The same radius on every corner.
    #[must_use]
    pub const fn all(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }

    /// The largest of the four — what a nine-grid's insets are derived from.
    #[must_use]
    pub fn max(self) -> f32 {
        self.tl.max(self.tr).max(self.br).max(self.bl)
    }
}

/// How a [`Mask::Shape`] is outlined. `width` and the dash offset seed their channels; the
/// cap, join and dash pattern are identity and changing one changes the mask.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub cap: Cap,
    pub join: Join,
    /// Alternating dash and gap runs, as **multiples of the stroke width**, spanning the
    /// patch's dash buffer. Empty is a solid stroke.
    pub dashes: crate::Span,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            cap: Cap::Flat,
            join: Join::Miter,
            dashes: crate::Span::EMPTY,
        }
    }
}

/// How a stroke terminates, at its own ends and at each dash.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Cap {
    #[default]
    Flat,
    Square,
    Round,
    Triangle,
}

/// How a stroke turns a corner.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Join {
    #[default]
    Miter,
    MiterOrBevel,
    Round,
    Bevel,
}

/// How a [`Paint::Ramp`]'s stops spread over the box they paint.
///
/// Not a direction: [`Radial`](Self::Radial) has none. Which construction realizes a
/// spread is this crate's decision, not the author's — the four linear forms become a
/// strip and the radial one a square tile, and every one of them is stretched to fill,
/// so none carries the sprite's extent and a resize costs nothing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Spread {
    #[default]
    Horizontal,
    Vertical,
    DiagonalDown,
    DiagonalUp,
    /// Outward from the centre. Stretched to fill, so a square profile becomes the
    /// ellipse of whatever box it lands in — which is what a glow is.
    Radial,
}

impl Spread {
    /// The ramp's start and end, as fractions of the painted box.
    ///
    /// `None` for [`Radial`](Self::Radial), which is a centre and two radii rather than
    /// two points, and is the one form that cannot answer.
    #[must_use]
    pub const fn ends(self) -> Option<([f32; 2], [f32; 2])> {
        match self {
            Self::Horizontal => Some(([0.0, 0.5], [1.0, 0.5])),
            Self::Vertical => Some(([0.5, 0.0], [0.5, 1.0])),
            Self::DiagonalDown => Some(([0.0, 0.0], [1.0, 1.0])),
            Self::DiagonalUp => Some(([0.0, 1.0], [1.0, 0.0])),
            Self::Radial => None,
        }
    }
}

/// One segment of authored path geometry, in sprite-local DIPs.
///
/// Authored in the sprite's own space and re-emitted when its box changes. A non-uniform
/// stretch distorts stroke width, corner radii and dash phase — a hairline comes out one
/// DIP on one axis and three on the other — and a resize is event rate, so re-emission
/// costs nothing on the frames that matter.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PathVerb {
    /// Starts a figure. `filled` decides whether it contributes to the fill region.
    Move {
        to: Vector2,
        filled: bool,
    },
    Line(Vector2),
    Cubic {
        c1: Vector2,
        c2: Vector2,
        to: Vector2,
    },
    /// Ends the current figure, closing it back to the start or leaving it open.
    End {
        closed: bool,
    },
}

// ── binding ─────────────────────────────────────────────────────────────────────

/// One animatable channel. The rows, their owners and their animation paths are the
/// property table in `prop.rs`; this is the name a caller uses.
///
/// Grouped by which composition object holds it. **Corner radii appear only as per-channel
/// scalars**, because the underlying animation names are DirectComposition's
/// (`TopLeftRadiusX`, …) rather than the WinRT projection's `Vector2` — naming the vector,
/// or its `.X` subchannel, is rejected.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Prop {
    // the visual
    Offset,
    OffsetX,
    OffsetY,
    Size,
    SizeX,
    SizeY,
    Scale,
    ScaleX,
    ScaleY,
    Opacity,
    RotationAngle,
    Center,
    CenterX,
    CenterY,
    // the clip
    ClipL,
    ClipT,
    ClipR,
    ClipB,
    CornerTopLeftX,
    CornerTopLeftY,
    CornerTopRightX,
    CornerTopRightY,
    CornerBottomRightX,
    CornerBottomRightY,
    CornerBottomLeftX,
    CornerBottomLeftY,
    // the shape mask
    TrimStart,
    TrimEnd,
    StrokeThickness,
    DashOffset,
    // the glow
    BlurRadius,
    ShadowOpacity,
}

/// What a channel carries.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Scalar(f32),
    Vec2(Vector2),
}

/// Which of the two a channel accepts. A mismatch is refused at the seam, because the
/// platform answers a mismatched animation type and a misspelt property name with the same
/// error and so cannot tell a caller which it was.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Scalar,
    Vec2,
}

impl Value {
    #[must_use]
    pub const fn kind(self) -> ValueKind {
        match self {
            Self::Scalar(_) => ValueKind::Scalar,
            Self::Vec2(_) => ValueKind::Vec2,
        }
    }
}

/// The three binding forms and no fourth, closed so the compiler checks it. All three
/// address one property set the same way, so there is one validity check and one re-issue
/// loop after device loss.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Bind {
    /// An event-rate property write. The default.
    Set(Value),
    /// The compositor plays it to completion while the CPU sleeps.
    Animate(Anim),
    /// The compositor evaluates it from a tracker, every vblank. **Permanent**: it owns
    /// the channel until [`Bind::Stop`], and a set on it is refused rather than applied.
    Track {
        tracker: TrackerId,
        axis: TrackerAxis,
        affine: Affine,
    },
    /// Hands the property back, leaving it wherever it had reached.
    Stop,
}

/// What the compositor plays.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Anim {
    /// Retargetable, interruptible motion. The spring object is shared per (value kind ×
    /// tuning) for the whole process, so starting one allocates nothing.
    Spring {
        to: Value,
        tuning: Tuning,
        delay_ms: u32,
    },
    /// A curve the app authored, spanning the patch's frame buffer.
    Frames {
        frames: crate::Span,
        duration_ms: u32,
        iterations: Iterations,
    },
}

/// The two spring tunings, **named and not numbered**: neither is derivable from the other,
/// and a call site holding a period eventually derives one.
///
/// The distinction is behavioural — a scroll surface *carries momentum*, where an indicator
/// reports a choice already made and should simply be there.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tuning {
    /// Every indicator, ink, pill glide and trim — and nothing else.
    Chrome,
    /// Where carrying momentum is the point.
    Scroll,
}

/// How many times a key-frame animation runs.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Iterations {
    Count(u32),
    Forever,
}

/// How one key-frame segment interpolates.
///
/// There is deliberately no step easing. `CreateStepEasingFunction` takes the segment's
/// **end** value immediately, so a pair intended to hold a value and then jump instead
/// jumps at the start — hold a level with an explicit frame at the held value instead.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Easing {
    Linear,
    /// The CSS `cubic-bezier()` convention: two control points, each in `0..=1`.
    Cubic {
        c1: Vector2,
        c2: Vector2,
    },
}

/// How a destroyed subtree leaves.
///
/// A subtree with an exit is flattened into one capture and animated as a ghost, so a dying
/// panel of sixty visuals fades as one. The ghost is released on the batch's own completion
/// signal, never on a timer.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum Exit {
    #[default]
    None,
    Fade {
        ms: u32,
    },
    Scale {
        to: f32,
        ms: u32,
    },
}

// ── trackers ────────────────────────────────────────────────────────────────────

/// A compositor-side interaction tracker.
///
/// The parameter says whether it was created with an owner, and so whether anything can
/// observe it. An owner is supplied at construction with no per-callback subscription, so a
/// tracker needing one event pays for all six — measured at ~19× an ownerless one. In the
/// type, "which surfaces qualify" is answerable by the compiler.
pub struct TrackerId<O = Observed> {
    pub(crate) raw: Id<Tracker>,
    _observed: core::marker::PhantomData<fn() -> O>,
}

/// A tracker whose motion something reconciles against: a virtualized list, or anything
/// driven by explicit position requests.
#[derive(Debug)]
pub struct Observed;
/// A tracker nothing observes — wheel and touch only, not virtualized. The cheap form, and
/// the default.
#[derive(Debug)]
pub struct Passive;

/// Whether a tracker is created with an owner. Sealed: the two markers above are the only
/// answers.
pub trait Observe: sealed::Sealed {
    /// Whether construction attaches `IInteractionTrackerOwner`.
    const OWNED: bool;
}

impl Observe for Observed {
    const OWNED: bool = true;
}
impl Observe for Passive {
    const OWNED: bool = false;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Observed {}
    impl Sealed for super::Passive {}
}

impl<O> TrackerId<O> {
    pub(crate) const fn new(raw: Id<Tracker>) -> Self {
        Self {
            raw,
            _observed: core::marker::PhantomData,
        }
    }

    /// The tracker addressed without its observability.
    #[must_use]
    pub const fn erased(self) -> TrackerId<()> {
        TrackerId::new(self.raw)
    }

    /// The identity a [`SceneEvent`](crate::SceneEvent) names it by, so a consumer holding
    /// typed ids can match a report against them.
    #[must_use]
    pub const fn id(self) -> Id<Tracker> {
        self.raw
    }
}

impl<O> Copy for TrackerId<O> {}
impl<O> Clone for TrackerId<O> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<O> PartialEq for TrackerId<O> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<O> Eq for TrackerId<O> {}
impl<O> core::fmt::Debug for TrackerId<O> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "tracker{:?}", self.raw)
    }
}

/// Which of a tracker's values a binding reads.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrackerAxis {
    PositionX,
    PositionY,
    Scale,
}

/// How a tracker's value maps onto a sink: `value * m + c`.
///
/// The mapping is ours: a tracker's position starts at zero and is in no visual's
/// coordinate space. Position increases for **up and left**, so the canonical content
/// binding is `m = -1`, and the wrong sign scrolls backwards.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Affine {
    pub m: f32,
    pub c: f32,
}

impl Affine {
    /// The content binding: the negation, with no offset.
    pub const CONTENT: Self = Self { m: -1.0, c: 0.0 };
}

/// Which axes a tracker's source drives.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Axes {
    pub x: bool,
    pub y: bool,
    pub scale: bool,
}

impl Axes {
    /// Vertical only — a scrolling list.
    pub const VERTICAL: Self = Self {
        x: false,
        y: true,
        scale: false,
    };
}

/// What the front thread asks a tracker to do.
///
/// **Never assume one applied.** A request arriving in the wrong state is dropped silently
/// by design, so every request is held against its id until a values change supersedes it
/// or a rejection names it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TrackerRequest {
    /// Move to an absolute position. Ignored outright while the user is interacting.
    To(Vector2),
    /// Move by a delta. The mouse-drag path, since a mouse contact cannot be redirected
    /// into a manipulation at all.
    By(Vector2),
    /// Hand a fling to the compositor, in DIPs per second, and stop participating.
    Fling(Vector2),
}

/// What a patch does to a tracker's configuration.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TrackerOp {
    /// Builds it, sourced from `viewport`.
    ///
    /// **An op rather than a call, because the order is the whole of it.** A
    /// `VisualInteractionSource` takes its hit region from the visual's size at the moment
    /// it is created, and a zero-size one hit-tests nothing while returning success — so
    /// creating a tracker has to happen after the placement ops that size its viewport, and
    /// the one thing in this system that is ordered against those is the patch.
    Create {
        viewport: NodeId,
        axes: Axes,
        /// Whether an owner is attached. Carried as data because the op stream is data; the
        /// guarantee that a passive tracker cannot be asked to move is on
        /// [`TrackerId`]'s parameter, where a caller meets it.
        owned: bool,
    },
    /// The range it rests inside. The position may travel outside during a manipulation or
    /// inertia — that overpan is the bounce, and it is wanted.
    Bounds { min: Vector2, max: Vector2 },
    /// How fast inertia decays per axis, in `0..=1`, or the system default.
    Decay(Option<Vector2>),
    /// Destroys it.
    Drop,
}

/// What a patch does to a shared resource.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ResOp {
    /// Path geometry, spanning the patch's verb buffer. Re-pointing moves **every** sprite
    /// sharing the id, whichever construction each uses, so a curve's fill, stroke and glow
    /// cannot diverge.
    Geom { verbs: crate::Span },
    /// Gradient stops, spanning the patch's stop buffer, and how they spread over the box.
    Ramp { stops: crate::Span, spread: Spread },
    /// A shaped run: fallback segments spanning the patch's segment buffer, and the tile
    /// they occupy.
    ///
    /// The tile is stated in **DIPs**, like every other extent the model hands over; the
    /// pixel grid is applied where it is rasterized, by the half that holds the scale.
    Run {
        segs: crate::Span,
        ink: windows_text::Ink,
    },
    /// Declares a region slot. The buffer itself arrives out of band, as the one kernel
    /// handle that legitimately crosses from the present thread.
    Region,
    /// Releases the slot. A resource cannot outlive its last sprite, and cannot be leaked
    /// by an app-side bug, because sprites refcount it — this only drops the model's own
    /// claim.
    Drop,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_linear_ramps_ends_span_the_box() {
        for spread in [
            Spread::Horizontal,
            Spread::Vertical,
            Spread::DiagonalDown,
            Spread::DiagonalUp,
        ] {
            let (from, to) = spread.ends().expect("a linear spread has ends");
            let travelled = (to[0] - from[0]).abs() + (to[1] - from[1]).abs();
            assert!(travelled >= 1.0, "{spread:?} travels {travelled}");
        }
    }

    /// The radial form is the one that cannot answer, and saying so is what keeps a
    /// caller from inventing two points for it.
    #[test]
    fn a_radial_ramp_has_no_ends() {
        assert!(Spread::Radial.ends().is_none());
    }

    /// Everything realized through a **capture** reads the pixel grid.
    ///
    /// The regression this closes has no visible trigger: a capture states its region in
    /// pixels, so a monitor DPI change leaves it describing the wrong number of them — and
    /// it carries no size, because the DIP rect did not move. Nothing else would notice.
    #[test]
    fn a_capture_reads_the_pixel_grid() {
        let shape = Mask::Shape {
            geom: GeomId::FIRST,
            stroke: None,
        };
        assert_eq!(shape.deps(), GenMask::GEOMETRY);
        let glow = Paint::Captured {
            group: GroupId::default(),
            blur: 4.0,
            tint: Radiance::new(1.0, 1.0, 1.0, 1.0),
        };
        assert_eq!(glow.deps(), GenMask::LIGHT.union(GenMask::GEOMETRY));
        // And a shared resource still reads nothing: re-rasterizing one moves the brush
        // every sprite already holds.
        assert_eq!(Mask::Run(RunId::FIRST).deps(), GenMask::NONE);
        assert_eq!(Paint::Ramp(RampId::FIRST).deps(), GenMask::NONE);
    }

    #[test]
    fn an_xform_defaults_to_visible_and_untransformed() {
        let x = Xform::default();
        assert_eq!(x.opacity, 1.0);
        assert_eq!(x.scale, Vector2 { x: 1.0, y: 1.0 });
        assert_eq!(x.rotation, 0.0);
        assert_eq!(x.clip, Clip::None);
    }
}
