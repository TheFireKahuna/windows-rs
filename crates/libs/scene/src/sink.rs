//! Declares the alphabet — everything a retained tree can draw — as plain `Send` data.
//! **App half.**
//!
//! The widget layer above and the patch below carry these types without conversion — they are
//! ids and light, and nothing here owns a composition object.
//!
//! A value that animates is a channel; a value that identifies is part of the declaration. A
//! trim window, a stroke width, a dash offset and a glow's blur animate, so they are bound
//! properties seeded by the mask or paint that introduced them. A cap style, a dash pattern
//! and a geometry id do not, so they live in the declaration and changing one rebuilds the
//! mask.

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
/// This crate never interprets one: it joins the flat hit array to whatever table a consumer
/// keeps beside it, and two such tables share the one id family — the app thread's handlers
/// and the front thread's chrome.
#[derive(Debug)]
pub struct Control;
/// A measurement request, minted and interpreted by the layer that owns the text engine.
///
/// Named `Measured` because [`Measure`](crate::Measure) is the trait a layout tree is handed,
/// and one of the two would shadow the other wherever both are in scope.
#[derive(Debug)]
pub struct Measured;

/// Either kind of node.
pub type NodeId = Id<Node>;
/// A path geometry resource.
pub type GeomId = Id<Geom>;
/// A rasterized gradient strip.
pub type RampId = Id<Ramp>;
/// A shaped run's coverage tile.
pub type RunId = Id<Run>;
/// A buffer the application presents itself.
pub type RegionId = Id<Region>;

/// A pending timed reveal, such as a submenu's hover-open or a tooltip's show.
///
/// A delay is a monotonic deadline compared on a frame the scene is already servicing:
/// nothing fires and nothing wakes. Its cost is the frame-clock request held open for its
/// duration.
pub type DelayId = Id<Delay>;

/// A node that paints. The newtype is the enforcement: a paint addressed to a group is a type
/// error at the model's own API.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SpriteId(pub(crate) NodeId);

/// A node that positions and clips its children, and paints nothing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct GroupId(pub(crate) NodeId);

impl SpriteId {
    /// Returns this sprite addressed as either kind of node.
    #[must_use]
    pub const fn node(self) -> NodeId {
        self.0
    }
}

impl GroupId {
    /// Returns this group addressed as either kind of node.
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

/// Which composition object a node mints.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// A `ContainerVisual`.
    Group,
    /// A `SpriteVisual`.
    Sprite,
}

/// A resource slot. Untyped because the [`ResOp`] variant beside it names the family.
pub type ResId = Id<()>;

/// A shared resource a sprite holds, with its family named.
///
/// The one join between the declaration alphabet and the resource tables: a mask or paint
/// reports what it holds through [`Mask::holds`] or [`Paint::holds`], and lifetime accounting
/// reads that.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Holding {
    Geom(GeomId),
    Ramp(RampId),
    Run(RunId),
    Region(RegionId),
}

// ── the alphabet ────────────────────────────────────────────────────────────────

/// What shape a sprite has, as *alpha coverage only, never colour*.
///
/// Colour lives on the [`Paint`]: a composition shape's brushes are 8-bit `Windows.UI.Color`,
/// which cannot express a negative component or a value above white at all, and this pipeline
/// authors both. The shape carries coverage, an FP16 surface carries light, and a mask brush
/// multiplies them — one construction rather than one per kind.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Mask {
    /// A rounded rectangle, stretched from a nine-grid atlas cell so that one raster
    /// serves any size with exact corners.
    Box { radius: Corners },
    /// One same-paint span of one shaped line, as a coverage tile. A run is one sprite
    /// whatever its glyph count, so a paragraph costs one visual per span rather than one
    /// per glyph, and one dirty region touches one visual per line.
    Run(RunId),
    /// Arbitrary geometry, in sprite-local DIPs. `stroke` picks fill or outline, and the
    /// draw-on window animates, so it is a channel. Which composition construction realizes
    /// it is this crate's choice, not the author's.
    Shape {
        geom: GeomId,
        stroke: Option<StrokeStyle>,
    },
    /// No mask: the paint's own alpha is the shape. For a paint whose surface already
    /// carries the alpha profile — a backdrop layer, a glow blob, a presented buffer.
    None,
}

impl Mask {
    /// Returns the shared resource this mask holds, or `None` if it holds none.
    #[must_use]
    pub const fn holds(self) -> Option<Holding> {
        match self {
            Self::Run(id) => Some(Holding::Run(id)),
            Self::Shape { geom, .. } => Some(Holding::Geom(geom)),
            Self::Box { .. } | Self::None => None,
        }
    }

    /// Returns which invalidation generations a chain built from this mask reads.
    ///
    /// A *shared* resource reads none of them: re-rasterizing one moves the single brush
    /// every sprite already holds, so the sprite has nothing to rebuild. A shape reads the
    /// geometry generation not because its geometry is shared but because the capture around
    /// it is per-sprite and states its region in pixels, so a grid that moves leaves it
    /// describing the wrong number of them. A resize corrects that from the size itself; a
    /// DPI change carries no size, because the DIP rect did not move.
    #[must_use]
    pub const fn deps(self) -> GenMask {
        match self {
            Self::Box { .. } | Self::Shape { .. } => GenMask::GEOMETRY,
            Self::Run(_) | Self::None => GenMask::NONE,
        }
    }
}

/// What colour a sprite is, as *authored scene light* before the display transform.
///
/// The transform is applied once, where the cell is rasterized, and cannot be stated here, so
/// the scene neither skips it nor applies it twice.
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
    /// Returns the shared resource this paint holds, or `None` if it holds none. A capture
    /// holds a *group*, whose lifetime is the node tree's.
    #[must_use]
    pub const fn holds(self) -> Option<Holding> {
        match self {
            Self::Ramp(id) => Some(Holding::Ramp(id)),
            Self::Presented(id) => Some(Holding::Region(id)),
            Self::Solid(_) | Self::Captured { .. } => None,
        }
    }

    /// Returns which invalidation generations a chain built from this paint reads.
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
    /// *Radians*, about `center` — the unit the animation name space carries. There is no
    /// degrees form.
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
/// The one rectangular form is the *absolute* one. An inset clip carries no radii, so every
/// rounded clip-to-bounds is already a rectangle clip, and `Inset{l,t,r,b}` over a node of
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
/// A radius is capped by the platform at half the box on each axis, so a fully rounded box
/// renders as a stadium.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Corners {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Corners {
    /// Returns the same radius on every corner.
    #[must_use]
    pub const fn all(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }

    /// Returns the largest of the four radii, which a nine-grid's insets are derived from.
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
/// Not a direction: [`Radial`](Self::Radial) has none. The four linear forms rasterize to a
/// strip and the radial one to a square tile, and every one of them is stretched to fill, so
/// none carries the sprite's extent and a resize costs nothing.
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
    /// Returns the ramp's start and end, as fractions of the painted box.
    ///
    /// `None` for [`Radial`](Self::Radial), which is a centre and two radii rather than two
    /// points.
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
/// Geometry is re-emitted when the sprite's box changes rather than stretched: a non-uniform
/// stretch distorts stroke width, corner radii and dash phase, so a hairline comes out one DIP
/// on one axis and three on the other. Re-emission runs at event rate, not per frame.
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

/// One animatable channel, as a caller names it. The rows behind these names, with their
/// owners and animation paths, are the property table in `prop.rs`.
///
/// Variants are grouped by which composition object holds the channel. Corner radii appear
/// only as per-channel scalars, because the underlying animation names are
/// DirectComposition's (`TopLeftRadiusX`, …) rather than the WinRT projection's `Vector2`:
/// the platform rejects the vector name and its `.X` subchannel alike.
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

/// Which of the two forms a channel accepts. A mismatch is refused at the seam, because the
/// platform answers a mismatched animation type and a misspelt property name with the same
/// error and so cannot tell a caller which it was.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Scalar,
    Vec2,
}

impl Value {
    /// Returns which form this value carries.
    #[must_use]
    pub const fn kind(self) -> ValueKind {
        match self {
            Self::Scalar(_) => ValueKind::Scalar,
            Self::Vec2(_) => ValueKind::Vec2,
        }
    }
}

/// How a channel is driven: written, animated or bound to a tracker — and released.
///
/// The three driving forms are closed, so the compiler checks the set. All three address one
/// property set the same way, so there is one validity check and one re-issue loop after
/// device loss.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Bind {
    /// An event-rate property write. The default.
    Set(Value),
    /// The compositor plays it to completion while the CPU sleeps.
    Animate(Anim),
    /// The compositor evaluates it from a tracker, every vblank. *Permanent*: it owns the
    /// channel until [`Bind::Stop`], and a set on it is refused rather than applied.
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

/// The two spring tunings. A caller names one rather than stating a period; the period is
/// derived from the tuning and the travel the spring has to cover.
///
/// The distinction is behavioural: a scroll surface carries momentum, where an indicator
/// reports a choice already made.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tuning {
    /// Every indicator, ink, pill glide and trim — and nothing else.
    Chrome,
    /// Where the motion carries momentum.
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
/// There is no step easing: `CreateStepEasingFunction` takes the segment's *end* value
/// immediately, so a pair meant to hold a value and then jump instead jumps at the start. A
/// level is held with an explicit frame at the held value.
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
/// `O` records whether the tracker was created with an owner, and so whether anything can
/// observe it. An owner is supplied at construction with no per-callback subscription, so a
/// tracker needing one event pays for all six — measured at ~19× the cost of an ownerless
/// one. Carrying that in the type is what lets [`Scene::request`](crate::Scene::request)
/// accept only an observed tracker.
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

/// Whether a tracker is created with an owner. Sealed: [`Observed`] and [`Passive`] are the
/// only implementors.
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

    /// Returns the tracker addressed without its observability.
    #[must_use]
    pub const fn erased(self) -> TrackerId<()> {
        TrackerId::new(self.raw)
    }

    /// Returns the identity a [`SceneEvent`](crate::SceneEvent) names this tracker by, so a
    /// consumer holding typed ids can match a report to one.
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
/// A tracker's position starts at zero and is in no visual's coordinate space, and it
/// increases for *up and left*, so the content binding is `m = -1` ([`Affine::CONTENT`]) and
/// the opposite sign scrolls backwards.
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
/// A request that arrives in the wrong state is dropped without an error, so a caller holds
/// each request against its id until a
/// [`TrackerValues`](crate::SceneEvent::TrackerValues) supersedes it or a
/// [`RequestIgnored`](crate::SceneEvent::RequestIgnored) names it.
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
    /// Builds a tracker sourced from `viewport`.
    ///
    /// Carried as an op because the ordering decides whether it works: a
    /// `VisualInteractionSource` takes its hit region from the visual's size at the moment it
    /// is created, and a zero-size one hit-tests nothing while returning success. Creation
    /// therefore has to follow the placement ops that size the viewport, and the patch is
    /// what orders it against them.
    Create {
        viewport: NodeId,
        axes: Axes,
        /// Whether an owner is attached. The guarantee that a passive tracker cannot be
        /// asked to move is on [`TrackerId`]'s parameter, where a caller meets it.
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
    /// Path geometry, spanning the patch's verb buffer. Re-pointing moves *every* sprite
    /// sharing the id, whichever construction each uses, so a curve's fill, stroke and glow
    /// cannot diverge.
    Geom { verbs: crate::Span },
    /// Gradient stops, spanning the patch's stop buffer, and how they spread over the box.
    Ramp { stops: crate::Span, spread: Spread },
    /// A shaped run: fallback segments spanning the patch's segment buffer, and the tile
    /// they occupy.
    ///
    /// The tile is stated in *DIPs*, like every other extent the model hands over; the pixel
    /// grid is applied where it is rasterized, by the half that holds the scale.
    Run {
        segs: crate::Span,
        ink: windows_text::Ink,
    },
    /// Declares a region slot. The buffer itself arrives out of band, as the one kernel
    /// handle that legitimately crosses from the present thread.
    Region,
    /// Releases the model's own claim on the slot. Sprites refcount the resource, so it
    /// lives until the last sprite painting with it is destroyed or re-declares.
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

    /// [`Spread::ends`] answers `None` for the radial form, which has no two points.
    #[test]
    fn a_radial_ramp_has_no_ends() {
        assert!(Spread::Radial.ends().is_none());
    }

    /// Everything realized through a *capture* reads the pixel grid.
    ///
    /// A capture states its region in pixels, so a monitor DPI change leaves it describing
    /// the wrong number of them while carrying no size change to correct it from.
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
