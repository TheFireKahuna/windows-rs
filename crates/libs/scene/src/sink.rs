//! The alphabet, the id authority, the environment and the quantizers. **Both halves.**
//!
//! Everything a retained tree can draw, as plain `Send` data carrying ids and light and
//! nothing thread-affine, so the widget layer above and the patch below carry one family of
//! types with no conversion. The id arithmetic both halves agree about, the two facts of the
//! display, and the snapping that keeps a derived cache key bounded live here too.

use windows_color::{OutputTransform, Radiance, Scrgb};
use windows_numerics::{Vector2, Vector3};

pub use windows_text::{FaceId, FamilyId, FontLadder, Ink};

// ── ids ─────────────────────────────────────────────────────────────────────────────

pub const NODE: u8 = 0;
pub const GEOM: u8 = 1;
pub const RAMP: u8 = 2;
pub const RUN: u8 = 3;
pub const REGION: u8 = 4;
pub const DASH: u8 = 5;
pub const TRACKER: u8 = 6;
pub const DELAY: u8 = 7;
pub const CONTROL: u8 = 8;

/// A generational index, with its family carried as a `const` parameter.
///
/// The family is a `u8` and not a marker type, which is what lets every trait derive: a
/// `PhantomData<fn() -> T>` family makes each derive demand a bound on a `T` the id says
/// nothing about.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Id<const F: u8> {
    idx: u32,
    age: u32,
}

impl<const F: u8> Id<F> {
    /// The id nothing is minted at. Every arena leaves slot zero unoccupied, so a parentless
    /// node and an absent sibling are both this and no link needs an `Option`.
    pub const NONE: Self = Self::raw(0, 0);
    /// What a fresh authority mints first: both halves seat their root here without
    /// exchanging it.
    pub const FIRST: Self = Self::raw(1, 1);

    #[must_use]
    pub const fn raw(idx: u32, age: u32) -> Self {
        Self { idx, age }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.idx as usize
    }

    #[must_use]
    pub const fn generation(self) -> u32 {
        self.age
    }

    #[must_use]
    pub const fn is_none(self) -> bool {
        self.age == 0
    }

    /// Carries the family as a value, so one table holds every resource id.
    #[must_use]
    pub const fn erased(self) -> ResId {
        ResId {
            idx: self.idx,
            age: self.age,
            family: F,
        }
    }
}

/// A resource id with its family carried as a value, so one table holds all five and a
/// disclaim is one lookup rather than five.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResId {
    idx: u32,
    age: u32,
    family: u8,
}

impl ResId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.idx as usize
    }
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.age
    }
    #[must_use]
    pub const fn family(self) -> u8 {
        self.family
    }
}

pub type NodeId = Id<NODE>;
pub type GeomId = Id<GEOM>;
pub type RampId = Id<RAMP>;
pub type RunId = Id<RUN>;
pub type RegionId = Id<REGION>;
pub type DashId = Id<DASH>;
pub type DelayId = Id<DELAY>;
pub type ControlId = Id<CONTROL>;

/// A node that paints. The kind is a fact of the node, so this is a view of its id and not a
/// second identity.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SpriteId(pub NodeId);
/// A node that holds children.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct GroupId(pub NodeId);

/// Mints generational ids and owns the free list.
///
/// The app half is the only minter and the scene half validates what it is handed, so a slot
/// freed by a destroy is reused by a create in the same patch with no round trip.
#[derive(Default)]
pub struct Ids<const F: u8> {
    gens: Vec<u32>,
    free: Vec<u32>,
    live: u32,
}

impl<const F: u8> Ids<F> {
    pub fn mint(&mut self) -> Id<F> {
        self.live += 1;
        if let Some(idx) = self.free.pop() {
            self.gens[idx as usize] += 1;
            return Id::raw(idx, self.gens[idx as usize]);
        }
        // Slot zero is never occupied, so `Id::NONE` can never name a live row.
        if self.gens.is_empty() {
            self.gens.push(0);
        }
        self.gens.push(1);
        Id::raw(self.gens.len() as u32 - 1, 1)
    }

    /// Frees `id`'s slot, reporting whether it was live. The generation moves, so every copy
    /// of the old id reads stale from here on.
    pub fn release(&mut self, id: Id<F>) -> bool {
        if !self.is_live(id) {
            return false;
        }
        self.gens[id.index()] += 1;
        self.free.push(id.index() as u32);
        self.live -= 1;
        true
    }

    #[must_use]
    pub fn is_live(&self, id: Id<F>) -> bool {
        id.generation() != 0 && self.gens.get(id.index()) == Some(&id.generation())
    }

    /// Returns the live id at row `at`, or `Id::NONE` where that row is vacant.
    ///
    /// A row's generation is odd while it is occupied: a mint sets it to 1 or advances a freed
    /// row's even count by one, and a release advances it again. That parity is what lets a
    /// row index answer without a second liveness column beside the generations.
    #[must_use]
    pub fn id_at(&self, at: u32) -> Id<F> {
        match self.gens.get(at as usize) {
            Some(&age) if age & 1 == 1 => Id::raw(at, age),
            _ => Id::NONE,
        }
    }

    #[must_use]
    pub fn live(&self) -> usize {
        self.live as usize
    }
}

/// A dense store keyed by an [`Id`], validated by generation.
pub struct Slots<const F: u8, T> {
    slots: Vec<Option<(u32, T)>>,
}

impl<const F: u8, T> Default for Slots<F, T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<const F: u8, T> Slots<F, T> {
    pub fn place(&mut self, id: Id<F>, value: T) {
        if self.slots.len() <= id.index() {
            self.slots.resize_with(id.index() + 1, || None);
        }
        self.slots[id.index()] = Some((id.generation(), value));
    }

    #[must_use]
    pub fn get(&self, id: Id<F>) -> Option<&T> {
        match self.slots.get(id.index()) {
            Some(Some((generation, value))) if *generation == id.generation() => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: Id<F>) -> Option<&mut T> {
        match self.slots.get_mut(id.index()) {
            Some(Some((generation, value))) if *generation == id.generation() => Some(value),
            _ => None,
        }
    }

    pub fn take(&mut self, id: Id<F>) -> Option<T> {
        let slot = self.slots.get_mut(id.index())?;
        match slot {
            Some((generation, _)) if *generation == id.generation() => {
                slot.take().map(|(_, value)| value)
            }
            _ => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<F>, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(at, slot)| slot.as_ref().map(|(g, v)| (Id::raw(at as u32, *g), v)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<F>, &mut T)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(at, slot)| slot.as_mut().map(|(g, v)| (Id::raw(at as u32, *g), v)))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
}

// ── the environment ─────────────────────────────────────────────────────────────────

/// How many pixels a DIP is, and how authored light reaches the display.
///
/// Stated at every operation that depends on it rather than stored, so the half that snapped
/// a rect and the half that keyed a raster cannot disagree about the scale. There is no
/// `set_dpi`: a cached environment can go stale without saying so, and geometry snapped to
/// one pixel grid with rasters built for another is soft text and hairline seams.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Env {
    dpi: f32,
    output: OutputTransform,
}

impl Env {
    #[must_use]
    pub const fn new(dpi: f32, output: OutputTransform) -> Self {
        Self { dpi, output }
    }

    /// The display's DPI, raw. A draw bracket sets it on the device context; everything else
    /// takes [`scale`](Env::scale).
    #[must_use]
    pub const fn dpi(self) -> f32 {
        self.dpi
    }

    #[must_use]
    pub const fn output(self) -> OutputTransform {
        self.output
    }

    /// The DIP-to-pixel factor, canonicalized so float noise cannot fork a cache.
    #[must_use]
    pub fn scale(self) -> f32 {
        snap_scale(self.dpi / 96.0)
    }

    /// Converts authored light to display-referred output. The only such conversion, and it
    /// has no inverse, so the transform runs exactly once per colour.
    #[must_use]
    pub fn apply(self, light: Radiance) -> Scrgb {
        self.output.apply(light)
    }

    /// Whether a change from `self` to `next` invalidates rasterized geometry. Every snapped
    /// dimension is a function of the scale, so colour is untouched.
    #[must_use]
    pub fn geometry_moved(self, next: Self) -> bool {
        self.scale() != next.scale()
    }

    /// Whether a change from `self` to `next` invalidates rasterized colour. The same
    /// authored light produces a different cell on a different display.
    #[must_use]
    pub fn light_moved(self, next: Self) -> bool {
        self.output != next.output
    }
}

// ── snapping and quantization ───────────────────────────────────────────────────────

/// Extents snap to whole physical pixels: a raster cannot hold a fraction of one.
const EXTENT_STEPS_PER_PX: f32 = 1.0;

/// Radii and stroke widths snap to quarter pixels. The nine-grid stretches from the corner
/// profile, so a quarter-pixel change there is visible where the same change in a box's
/// width is not.
pub const DETAIL_STEPS_PER_PX: f32 = 4.0;

/// Steps per unit of the signed-square-root colour encoding.
const COLOR_STEPS: f32 = 4096.0;

/// Snaps a DIP length onto the physical grid at `steps_per_px` steps per pixel. Returns
/// `0.0` for a non-finite or non-positive `dip`.
#[must_use]
pub fn snap_len(dip: f32, scale: f32, steps_per_px: f32) -> f32 {
    if !dip.is_finite() || dip <= 0.0 {
        return 0.0;
    }
    let grid = (scale * steps_per_px).max(1.0e-3);
    (dip * grid).round() / grid
}

/// Snaps a detail length — a corner radius, a stroke width — onto quarter pixels.
#[must_use]
pub fn snap_detail(dip: f32, scale: f32) -> f32 {
    snap_len(dip, scale, DETAIL_STEPS_PER_PX)
}

/// Snaps an extent onto whole physical pixels.
///
/// A positive extent that rounds below one pixel snaps up to one, because a zero-sized
/// surface is an allocation failure. Returns `0.0` for a non-finite or non-positive `dip`.
#[must_use]
pub fn snap_extent(dip: f32, scale: f32) -> f32 {
    if !dip.is_finite() || dip <= 0.0 {
        return 0.0;
    }
    let grid = (scale * EXTENT_STEPS_PER_PX).max(1.0e-3);
    (dip * grid).round().max(1.0) / grid
}

/// Canonicalizes a DIP-to-pixel factor to a thousandth.
///
/// Display scales are a short list, and rounding here keeps float noise in `dpi / 96.0` from
/// forking the whole cache into two populations that differ in the last bit. Returns `1.0`
/// for a non-finite or non-positive `scale`.
#[must_use]
pub fn snap_scale(scale: f32) -> f32 {
    if !scale.is_finite() || scale <= 0.0 {
        return 1.0;
    }
    (scale * 1000.0).round() / 1000.0
}

/// The pixel extent a snapped DIP size occupies, clamped to `1..=65535` so it can size an
/// allocation.
#[must_use]
pub fn extent_px(dip: f32, scale: f32) -> u32 {
    let px = (snap_extent(dip, scale) * scale).round();
    px.clamp(1.0, f32::from(u16::MAX)) as u32
}

/// A quantized display-referred colour, and the colour a rasterizer draws with.
///
/// The encoding is a signed square root followed by a uniform step, which is what keys an
/// extended-range pipeline: a quantizer clamped to `[0, 1]` would crush both of the values
/// FP16 surfaces exist to carry — a component outside Rec.709 on a wide-gamut display, and a
/// component far above one. It is sign-symmetric and exact at zero.
///
/// The field is private and the only constructor quantizes, so every value in scope has been
/// through the encoding and [`dequant`](Q::dequant) is what it paints as.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Q([i32; 4]);

impl Q {
    /// Quantizes a colour that has already been through the output transform.
    #[must_use]
    pub fn new(c: Scrgb) -> Self {
        Self([
            quant_channel(c.r),
            quant_channel(c.g),
            quant_channel(c.b),
            quant_channel(c.a),
        ])
    }

    /// The value the raster is painted in: the key is round-tripped before drawing, so the
    /// cached surface matches its key exactly.
    #[must_use]
    pub fn dequant(self) -> Scrgb {
        Scrgb {
            r: dequant_channel(self.0[0]),
            g: dequant_channel(self.0[1]),
            b: dequant_channel(self.0[2]),
            a: dequant_channel(self.0[3]),
        }
    }

    /// Whether the quantized alpha is fully opaque, which decides a cell's alpha mode.
    #[must_use]
    pub fn is_opaque(self) -> bool {
        self.0[3] >= COLOR_STEPS as i32
    }
}

impl From<Scrgb> for Q {
    fn from(c: Scrgb) -> Self {
        Self::new(c)
    }
}

fn quant_channel(v: f32) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    (v.abs().sqrt().copysign(v) * COLOR_STEPS).round() as i32
}

fn dequant_channel(q: i32) -> f32 {
    let e = q as f32 / COLOR_STEPS;
    (e * e).copysign(e)
}

/// Quantizes a gradient stop's position to 1/65536 of the ramp.
///
/// A ramp is rasterized into a few hundred texels, so a stop resolved finer than this cannot
/// move a texel and would only fork the identity that keys them.
#[must_use]
pub fn quant_stop(at: f32) -> u16 {
    if !at.is_finite() {
        return 0;
    }
    (at.clamp(0.0, 1.0) * f32::from(u16::MAX)).round() as u16
}

/// The fraction of the ramp [`quant_stop`] encoded.
#[must_use]
pub fn stop_fraction(at: u16) -> f32 {
    f32::from(at) / f32::from(u16::MAX)
}

// ── invalidation ────────────────────────────────────────────────────────────────────

/// Three independent counters rather than one epoch, so a theme flip leaves every glyph tile
/// standing and a DPI change leaves colour alone.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Gen {
    /// Device loss, which invalidates every cell.
    pub device: u32,
    /// A DPI change: every snapped dimension moves, so geometry and text re-rasterize.
    pub dpi: u32,
    /// A display-capability change or a theme flip: the output transform moved.
    pub color: u32,
}

/// Selects the generations a rasterized cell's freshness depends on.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct GenMask(u8);

impl GenMask {
    /// Content whose realized object survives all three, so a sprite holding it never
    /// rebinds. What a shared resource reads.
    pub const NONE: Self = Self(0);
    /// Rasterized shapes and coverage, whose colour comes from elsewhere.
    pub const GEOMETRY: Self = Self(0b011);
    /// Cells whose whole content is a colour already through the output transform.
    pub const LIGHT: Self = Self(0b101);

    /// A mask reading every generation either side reads. A sprite's chain is a mask and a
    /// paint, and is fresh only while both are.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether content built at `built` is still fresh under `now`.
    #[must_use]
    pub fn fresh(self, built: Gen, now: Gen) -> bool {
        (self.0 & 1 == 0 || built.device == now.device)
            && (self.0 & 2 == 0 || built.dpi == now.dpi)
            && (self.0 & 4 == 0 || built.color == now.color)
    }
}

// ── the alphabet ────────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum NodeKind {
    Group,
    Sprite,
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Corners {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Corners {
    #[must_use]
    pub const fn all(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }

    #[must_use]
    pub fn max(self) -> f32 {
        self.tl.max(self.tr).max(self.br).max(self.bl)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Side {
    Left,
    Top,
    Right,
    Bottom,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Cap {
    Flat,
    Round,
    Square,
    Triangle,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Join {
    Miter,
    MiterOrBevel,
    Bevel,
    Round,
}

/// `width` and the dash offset seed their channels; the cap, join and dash pattern identify
/// and change the mask. A value that animates is a bound channel; a value that identifies is
/// part of the declaration, and never both.
///
/// An unbroken stroke names [`DashId::NONE`], as every other absent id here does: an
/// `Option<DashId>` has no niche to pack into, and one on this row widens [`Mask`] by eight
/// bytes on the wire and on every painted sprite.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct StrokeStyle {
    pub width: f32,
    pub cap: Cap,
    pub join: Join,
    pub dash: DashId,
}

/// Supplies the sprite's shape. Carries alpha only, never colour.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Mask {
    Box {
        radius: Corners,
    },
    Outline {
        radius: Corners,
        width: f32,
        open: Option<Side>,
    },
    Run(RunId),
    Shape {
        geom: GeomId,
        stroke: Option<StrokeStyle>,
        space: PathSpace,
    },
    None,
}

/// Supplies the sprite's colour, as authored scene light. The display transform is applied
/// once, where the cell is rasterized, and not here.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Paint {
    Solid(Radiance),
    Ramp(RampId),
    /// Blurs already transformed content behind the sprite; sigma must be in 0..=250 DIPs.
    /// The sprite must use `Mask::None` and no halo to preserve HDR effect output through composition.
    Backdrop { sigma: f32 },
    Captured {
        group: GroupId,
        sigma: f32,
        tint: Radiance,
    },
    /// Samples unscaled pixels from a finite, nonnegative source origin in DIPs.
    Presented { region: RegionId, origin: Vector2 },
    PresentedView { region: RegionId, view: RegionView },
    None,
}

/// A fixed rectangle in a presented source, in DIPs.
///
/// The producer must keep its packing fixed while views are mounted. Pixel views must
/// have destination bounds no larger than the source rectangle.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct RegionView {
    pub rect: [f32; 4],
    pub sampling: RegionSampling,
}

impl RegionView {
    /// Returns whether the source rectangle has finite coordinates and positive area.
    #[must_use]
    pub fn is_valid(self) -> bool {
        let [left, top, right, bottom] = self.rect;
        self.rect.into_iter().all(f32::is_finite)
            && left >= 0.0 && top >= 0.0 && right > left && bottom > top
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RegionSampling {
    /// Preserves source pixels, including text coverage and stroke widths.
    Pixels,
    /// Fits image content to the visual's live size. Text must not use this mode.
    Fit,
}

/// A blurred copy of the sprite's own silhouette, cast behind it.
///
/// The halo is an effect graph the compositor evaluates: a Gaussian of sigma `sigma` DIPs
/// over the sprite's brush alpha, multiplied by the tint. The construction costs the
/// census four visuals ([16 §10](16-SCENE-INTERNALS.md) of the GUI spec records why), and
/// the sigma animates as a channel like any other.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Halo {
    /// Gaussian sigma, in DIPs. The capture that feeds the blur is sized at `3·sigma +
    /// |offset|` past the node's box on every side, fixed at the construction.
    pub sigma: f32,
    pub tint: Radiance,
    pub offset: Vector2,
}

/// Clips to live visual bounds, an explicit rounded rectangle, or a geometry.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub enum Clip {
    #[default]
    None,
    /// Follows the visual's animated size through a zero-inset native clip.
    Bounds,
    /// Follows the visual's animated size with fixed DIP corner radii.
    RoundedBounds(Corners),
    Rect {
        l: f32,
        t: f32,
        r: f32,
        b: f32,
        radius: Corners,
    },
    Geom(GeomId),
}

/// How a ramp's stops spread over the box they paint.
///
/// Not a direction: the radial form has none. Axial forms rasterize to strips and the rest
/// to square tiles; all stretch to fill and carry no sprite extent, so a DIP-sized feather is
/// expressed by updating its normalized edge fraction when the consumer's width changes. The
/// feather is baked into the strip's texels, because the compositor's own gradient brush
/// carries 8-bit stops and a narrow alpha ramp would quantize to almost nothing.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Spread {
    Horizontal,
    HorizontalFeathered {
        edge: f32,
        /// Transparent margin at each side, as a fraction of the box width.
        inset: f32,
    },
    Vertical,
    VerticalFeathered {
        edge: f32,
    },
    DiagonalDown,
    DiagonalUp,
    /// Outward from the centre. Stretched to fill, so a square profile becomes the ellipse
    /// of whatever box it lands in, which is what a glow is.
    Radial,
    /// Clockwise around a normalized centre, starting at `start` radians.
    Conic {
        center: [f32; 2],
        start: f32,
    },
}

impl Spread {
    /// The ramp's start and end as fractions of the box, or `None` for the two forms that
    /// are a centre rather than two points.
    #[must_use]
    pub const fn ends(self) -> Option<([f32; 2], [f32; 2])> {
        match self {
            Self::Horizontal | Self::HorizontalFeathered { .. } => Some(([0.0, 0.5], [1.0, 0.5])),
            Self::Vertical | Self::VerticalFeathered { .. } => Some(([0.5, 0.0], [0.5, 1.0])),
            Self::DiagonalDown => Some(([0.0, 0.0], [1.0, 1.0])),
            Self::DiagonalUp => Some(([0.0, 1.0], [1.0, 0.0])),
            Self::Radial | Self::Conic { .. } => None,
        }
    }

    /// The fraction of each end a taper occupies, or `None` where nothing tapers.
    #[must_use]
    pub const fn edge(self) -> Option<f32> {
        match self {
            Self::VerticalFeathered { edge } | Self::HorizontalFeathered { edge, .. } => Some(edge),
            _ => None,
        }
    }
}

/// Selects the coordinate space of a shape's geometry.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum PathSpace {
    /// Uses sprite-local DIPs; layout-dependent geometry is re-emitted at event rate.
    #[default]
    Local,
    /// Maps the unit box to the visual's live size, preserving DIP stroke thickness.
    /// Geometric radii scale with the box. Text must not use this space.
    Unit,
}

/// Supplies geometry in the coordinate space selected by its shape mask.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PathVerb {
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
    RoundRect {
        origin: Vector2,
        size: Vector2,
        radius: f32,
    },
    Segment {
        from: Vector2,
        to: Vector2,
    },
    End {
        closed: bool,
    },
}

/// What a resource identity names. Carried as a value so one table holds every family.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Holding {
    Geom(GeomId),
    Ramp(RampId),
    Run(RunId),
    Region(RegionId),
    Dash(DashId),
}

impl Holding {
    #[must_use]
    pub const fn id(self) -> ResId {
        match self {
            Self::Geom(id) => id.erased(),
            Self::Ramp(id) => id.erased(),
            Self::Run(id) => id.erased(),
            Self::Region(id) => id.erased(),
            Self::Dash(id) => id.erased(),
        }
    }
}

impl Mask {
    /// The resource this mask holds, which the applier retains and releases.
    #[must_use]
    pub const fn holds(self) -> Option<Holding> {
        match self {
            Self::Run(id) => Some(Holding::Run(id)),
            Self::Shape { geom, .. } => Some(Holding::Geom(geom)),
            _ => None,
        }
    }

    /// The dash pattern a stroked shape holds, which is a second claim.
    ///
    /// A stroked shape holds two resources — the geometry it is drawn from and the pattern
    /// it is broken by — and the applier refcounts both. Without this one a `ResOp::Drop` on
    /// the pattern frees the row under a sprite still stroking with it, and the next rebind
    /// draws an unbroken line.
    #[must_use]
    pub const fn holds_dash(self) -> Option<Holding> {
        match self {
            Self::Shape {
                stroke: Some(stroke),
                ..
            } if !stroke.dash.is_none() => Some(Holding::Dash(stroke.dash)),
            _ => None,
        }
    }

    /// The generations a realized chain over this mask reads.
    #[must_use]
    pub const fn deps(self) -> GenMask {
        match self {
            Self::Box { .. } | Self::Outline { .. } | Self::Shape { .. } => GenMask::GEOMETRY,
            _ => GenMask::NONE,
        }
    }
}

impl Paint {
    #[must_use]
    pub const fn holds(self) -> Option<Holding> {
        match self {
            Self::Ramp(id) => Some(Holding::Ramp(id)),
            Self::Presented { region: id, .. } | Self::PresentedView { region: id, .. } => Some(Holding::Region(id)),
            _ => None,
        }
    }

    /// Everything realized through a capture reads the pixel grid, because a capture states
    /// its region in pixels and a DPI change carries no size change to correct it from.
    #[must_use]
    pub const fn deps(self) -> GenMask {
        match self {
            Self::Solid(_) => GenMask::LIGHT,
            Self::Captured { .. } => GenMask::LIGHT.union(GenMask::GEOMETRY),
            Self::Presented { .. } | Self::PresentedView { .. } => GenMask::GEOMETRY,
            _ => GenMask::NONE,
        }
    }
}

// ── channels ────────────────────────────────────────────────────────────────────────

/// One animatable channel as a caller names it.
///
/// Corner radii appear only as per-channel scalars, because the underlying animation names
/// are DirectComposition's — naming the WinRT `Vector2` or its `.X` subchannel returns
/// `E_INVALIDARG`.
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Prop {
    Offset,
    OffsetX,
    OffsetY,
    Size,
    SizeX,
    SizeY,
    Scale,
    ScaleX,
    ScaleY,
    RotationAngle,
    Center,
    CenterX,
    CenterY,
    Opacity,
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
    TrimStart,
    TrimEnd,
    StrokeThickness,
    DashOffset,
    /// The glow's Gaussian sigma, in DIPs. The channel drives the effect property, whose
    /// own name is the platform's `"blur.BlurAmount"`.
    GlowSigma,
    /// The glow's opacity, on the halo sprite alone.
    GlowOpacity,
    /// Horizontal anchor fraction, independent of the layout offset.
    AnchorX,
    /// Vertical anchor fraction, independent of the layout offset.
    AnchorY,
    /// Horizontal local translation in DIPs, independent of layout offset and anchor.
    TranslationX,
    /// Vertical local translation in DIPs, independent of layout offset and anchor.
    TranslationY,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Value {
    Scalar(f32),
    Vec2(Vector2),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ValueKind {
    Scalar,
    Vec2,
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Self::Scalar(v)
    }
}

impl From<Vector2> for Value {
    fn from(v: Vector2) -> Self {
        Self::Vec2(v)
    }
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

/// Selects a shared natural-motion curve and its travel-scaling policy.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Tuning {
    Chrome,
    Scroll,
    /// Uses the chrome curve without distance scaling for related layout bounds.
    Layout,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Iterations {
    Count(u32),
    Forever,
}

/// How one key-frame segment interpolates.
///
/// There is no step easing: `CreateStepEasingFunction` takes the segment's *end* value
/// immediately, so a pair meant to hold a value and then jump instead jumps at the start. A
/// level is held with an explicit frame at the held value.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Easing {
    Linear,
    /// The CSS `cubic-bezier()` convention: two control points, each in `0..=1`.
    Cubic(Vector2, Vector2),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Anim {
    /// Retargetable, interruptible motion. The spring object is shared per (value kind ×
    /// tuning) for the whole process, so starting one allocates nothing.
    ///
    /// `delay_ms` holds the channel where it stands before the motion runs. The compositor
    /// measures the wait, so nothing on this side wakes for it, and a retarget arriving
    /// inside the wait replaces the whole animation rather than queueing behind it.
    Spring {
        to: Value,
        tuning: Tuning,
        delay_ms: u32,
    },
    /// A curve the app authored, spanning the patch's frame buffer.
    Frames {
        frames: Span,
        duration_ms: u32,
        iterations: Iterations,
    },
}

/// Drives one property: a set, an animation, a tracker or offset expression, plus the stop
/// that releases one. Closed, so "no fifth binding form" is a match the compiler checks.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Bind {
    /// An event-rate property write. The default.
    Set(Value),
    /// The compositor plays it to completion while the CPU sleeps.
    Animate(Anim),
    /// The compositor evaluates it from a tracker, every vblank. *Permanent*: it owns the
    /// channel until [`Bind::Stop`], and a set on it is refused rather than applied.
    Track {
        tracker: TrackerId<()>,
        axis: TrackerAxis,
        affine: Affine,
    },
    /// Derives trim or opacity from another visual's animated offset, clamped to a range.
    FollowOffset {
        source: NodeId,
        vertical: bool,
        affine: Affine,
        /// Divides the mapped offset by this visual's live axial size minus the inset.
        /// A non-positive extent maps to zero before clamping.
        extent: Option<(NodeId, f32)>,
        clamp: [f32; 2],
    },
    /// Hands the property back, leaving it wherever it had reached.
    Stop,
}

/// How a destroyed subtree leaves.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub enum Exit {
    #[default]
    None,
    /// Clips the retained subtree closed under its existing parent with the layout spring.
    Collapse,
    Fade {
        ms: u32,
    },
    Scale {
        to: f32,
        ms: u32,
    },
    /// Moves the flattened subtree by a multiple of its own size.
    Slide {
        by: Vector2,
        ms: u32,
        easing: Easing,
    },
}

// ── trackers ────────────────────────────────────────────────────────────────────────

/// A compositor-side interaction tracker.
///
/// `O` records whether the tracker was created with an owner, and so whether anything can
/// observe it. An owner is supplied at construction with no per-callback subscription, so a
/// tracker needing one event pays for all six. Carrying that in the type is what lets
/// [`Scene::request`](crate::Scene::request) accept only an observed tracker.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TrackerId<O = Observed> {
    raw: Id<TRACKER>,
    _observed: core::marker::PhantomData<fn() -> O>,
}

/// A tracker whose motion something reconciles against: a virtualized list, or anything
/// driven by explicit position requests.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Observed;
/// A tracker nothing observes — wheel and touch only. The cheap form.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Passive;

impl<O> TrackerId<O> {
    #[must_use]
    pub const fn new(raw: Id<TRACKER>) -> Self {
        Self {
            raw,
            _observed: core::marker::PhantomData,
        }
    }

    #[must_use]
    pub const fn erased(self) -> TrackerId<()> {
        TrackerId::new(self.raw)
    }

    #[must_use]
    pub const fn id(self) -> Id<TRACKER> {
        self.raw
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TrackerAxis {
    PositionX,
    PositionY,
    Scale,
}

/// A tracker axis mapped onto a sink as `value * m + c`. Its position starts at zero and is
/// in no visual's coordinate space, so the mapping is ours.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Affine {
    pub m: f32,
    pub c: f32,
}

impl Affine {
    /// Position increases for up/left motion, so the content binding is the negated axis. A
    /// wrong sign scrolls the content backwards.
    pub const CONTENT: Self = Self { m: -1.0, c: 0.0 };
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Axes {
    pub x: bool,
    pub y: bool,
    pub scale: bool,
}

impl Axes {
    pub const VERTICAL: Self = Self {
        x: false,
        y: true,
        scale: false,
    };
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TrackerRequest {
    By(Vector2),
    To(Vector2),
    Fling(Vector2),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TrackerOp {
    /// Carried as an op because the ordering decides whether it works: a source takes its
    /// hit region from the visual's size at the moment it is created, and a zero-size one
    /// hit-tests nothing while returning success.
    Create {
        viewport: GroupId,
        axes: Axes,
        owned: bool,
    },
    /// The range it rests inside. The position may travel outside during a manipulation or
    /// inertia; that overpan is the bounce, and it is wanted.
    Bounds {
        min: Vector2,
        max: Vector2,
    },
    /// How fast inertia decays per axis, in `0..=1`, or the system default.
    Decay(Option<Vector2>),
    Drop,
}

/// The phase a tracker's last reported transition put it in.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Phase {
    #[default]
    Idle,
    Interacting,
    Inertia,
    CustomAnimation,
}

// ── resource ops ────────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ResOp {
    /// Path geometry, spanning the patch's verb buffer. Re-pointing moves *every* sprite
    /// sharing the id, whichever construction each uses, so a curve's fill, stroke and glow
    /// cannot diverge.
    Geom { verbs: Span },
    /// Gradient stops, spanning the patch's stop buffer, and how they spread over the box.
    Ramp { stops: Span, spread: Spread },
    /// A shaped run: fallback segments spanning the patch's segment buffer, and the tile
    /// they occupy, in DIPs.
    Run { segs: Span, ink: Ink },
    /// A dash pattern, spanning the patch's float buffer.
    Dash { runs: Span },
    /// Declares a region slot. The buffer itself arrives out of band, through
    /// [`Scene::set_region`](crate::Scene::set_region), as the one kernel handle that
    /// legitimately crosses from the present thread.
    Region,
    /// Releases the model's own claim on the slot. Sprites refcount the resource, so it
    /// lives until the last sprite painting with it is destroyed or re-declares.
    Drop,
}

/// One shaped segment of a run.
///
/// Font fallback splits a line across faces, so a run is a list of these: a single-segment
/// wire fails to render CJK and emoji outright. It names a [`FaceId`] and not a family,
/// because the face fallback chose is not one the requested family names, and a glyph index
/// is an index into a face.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct GlyphSeg {
    /// The face every glyph index here is an index into, against the shared ladder.
    pub face: FaceId,
    /// Em size in DIPs, as shaped.
    pub em: f32,
    /// Bidi embedding level; odd means the segment advances leftward from `origin`.
    pub bidi: u32,
    /// Baseline origin relative to the tile's top-left, in DIPs. Carried rather than folded
    /// from advances, so a bidi line, where visual order and advance order disagree, is
    /// placed by the same rule as any other.
    pub origin: Vector2,
    /// Glyph indices, in the patch's glyph buffer.
    pub glyphs: Span,
    /// Advance per glyph, in the patch's float buffer.
    pub advances: Span,
    /// Displacement per glyph, in the patch's float buffer, two floats per glyph.
    pub offsets: Span,
}

pub use crate::patch::Span;

pub type Point = Vector2;

/// Zero in the third component: the compositor's offset, scale and centre are three-vectors
/// and the sink alphabet is two-dimensional.
#[must_use]
pub fn v3(v: Vector2) -> Vector3 {
    Vector3 {
        x: v.x,
        y: v.y,
        z: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_color::DisplayCapability;

    const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

    fn env(dpi: f32) -> Env {
        Env::new(
            dpi,
            OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
        )
    }

    #[test]
    fn a_fresh_authority_mints_the_id_both_halves_seat_their_root_at() {
        let mut ids: Ids<NODE> = Ids::default();
        assert_eq!(ids.mint(), NodeId::FIRST);
        assert!(!NodeId::FIRST.is_none());
        assert!(NodeId::NONE.is_none());
    }

    #[test]
    fn a_released_slot_is_reused_and_every_copy_of_the_old_id_reads_stale() {
        let mut ids: Ids<NODE> = Ids::default();
        let first = ids.mint();
        assert!(ids.release(first));
        assert!(!ids.is_live(first));
        assert!(!ids.release(first), "a double release is not a mint");
        let reused = ids.mint();
        assert_eq!(reused.index(), first.index());
        assert_ne!(reused.generation(), first.generation());
        assert!(ids.is_live(reused));
        assert_eq!(ids.live(), 1);
    }

    #[test]
    fn a_slot_answers_for_the_generation_that_placed_it_and_no_other() {
        let mut slots: Slots<NODE, u8> = Slots::default();
        let id = NodeId::raw(3, 1);
        slots.place(id, 7);
        assert_eq!(slots.get(id), Some(&7));
        assert_eq!(slots.get(NodeId::raw(3, 2)), None, "a stale id reads none");
        assert_eq!(slots.len(), 1);
        assert_eq!(slots.take(NodeId::raw(3, 2)), None);
        assert_eq!(slots.take(id), Some(7));
        assert!(slots.is_empty());
    }

    #[test]
    fn an_erased_id_carries_its_family_so_one_table_holds_them_all() {
        let geom = GeomId::raw(4, 2).erased();
        let ramp = RampId::raw(4, 2).erased();
        assert_ne!(geom, ramp, "two families collided in one index space");
        assert_eq!(geom.family(), GEOM);
        assert_eq!(geom.index(), 4);
        assert_eq!(geom.generation(), 2);
    }

    #[test]
    fn the_scale_is_canonicalized_so_float_noise_cannot_fork_a_cache() {
        assert_eq!(env(144.0).scale(), 1.5);
        assert_eq!(env(120.0).scale(), 1.25);
        assert_eq!(env(96.0).scale(), 1.0);
    }

    #[test]
    fn a_dpi_move_invalidates_geometry_and_leaves_light_alone() {
        let (before, after) = (env(96.0), env(144.0));
        assert!(before.geometry_moved(after));
        assert!(!before.light_moved(after));
    }

    #[test]
    fn a_display_move_invalidates_light_and_leaves_geometry_alone() {
        let before = env(96.0);
        let after = Env::new(
            96.0,
            OutputTransform::for_display(
                DisplayCapability::HighDynamicRange {
                    gamut: windows_color::Gamut::REC2020,
                    white_nits: 203.0,
                    peak_nits: 1000.0,
                },
                600.0,
            ),
        );
        assert!(before.light_moved(after));
        assert!(!before.geometry_moved(after));
    }

    #[test]
    fn a_dpi_that_snaps_to_the_same_scale_invalidates_nothing() {
        let before = env(96.0);
        let after = env(96.02);
        assert!(!before.geometry_moved(after));
        assert!(!before.light_moved(after));
    }

    #[test]
    fn a_non_finite_input_collapses_rather_than_minting_a_key() {
        for scale in SCALES {
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
                assert_eq!(snap_extent(bad, scale), 0.0);
                assert_eq!(snap_detail(bad, scale), 0.0);
            }
        }
        assert_eq!(snap_scale(f32::NAN), 1.0);
        assert_eq!(snap_scale(0.0), 1.0);
        assert_eq!(quant_stop(f32::NAN), 0);
    }

    #[test]
    fn a_positive_extent_never_collapses_to_nothing() {
        for scale in SCALES {
            let snapped = snap_extent(0.1, scale);
            assert!(snapped > 0.0, "0.1 DIP at {scale}x snapped to {snapped}");
            assert_eq!(extent_px(0.1, scale), 1);
        }
    }

    #[test]
    fn snapping_lands_on_the_physical_grid() {
        for scale in SCALES {
            for dip in [1.0_f32, 7.3, 12.9, 100.4] {
                let px = snap_extent(dip, scale) * scale;
                assert!(
                    (px - px.round()).abs() < 1.0e-3,
                    "{dip} DIP at {scale}x is {px} px"
                );
                let detail = snap_detail(dip, scale) * scale * DETAIL_STEPS_PER_PX;
                assert!((detail - detail.round()).abs() < 1.0e-3);
            }
        }
    }

    #[test]
    fn quantization_is_sign_symmetric_and_exact_at_zero() {
        let zero = Q::new(Scrgb::TRANSPARENT).dequant();
        assert_eq!((zero.r, zero.g, zero.b, zero.a), (0.0, 0.0, 0.0, 0.0));
        for v in [0.25_f32, 1.0, 4.0, 12.0] {
            let of = |r: f32| {
                Q::new(Scrgb {
                    r,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                })
                .dequant()
                .r
            };
            assert!((of(v) + of(-v)).abs() < 1.0e-4, "{v}");
        }
    }

    #[test]
    fn quantization_clips_neither_end_of_the_extended_range() {
        let wild = Scrgb {
            r: -0.4,
            g: 12.0,
            b: 0.5,
            a: 1.0,
        };
        let back = Q::new(wild).dequant();
        assert!(back.r < 0.0, "a negative component survived as {}", back.r);
        assert!(
            back.g > 11.9,
            "an above-white component survived as {}",
            back.g
        );
    }

    #[test]
    fn quantization_is_finer_than_sixteen_bits_at_white() {
        let of = |r: f32| {
            Q::new(Scrgb {
                r,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            })
        };
        assert_ne!(of(1.0), of(1.0 + 1.0 / 2048.0));
    }

    #[test]
    fn opacity_comes_from_the_quantized_alpha_and_not_the_authored_one() {
        let alpha = |a: f32| {
            Q::new(Scrgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a,
            })
            .is_opaque()
        };
        assert!(alpha(1.0));
        assert!(!alpha(0.999));
    }

    #[test]
    fn a_stop_round_trips_through_its_quantized_form() {
        for at in [0.0_f32, 0.25, 0.5, 1.0] {
            assert!((stop_fraction(quant_stop(at)) - at).abs() < 1.0e-4, "{at}");
        }
        assert_eq!(quant_stop(-1.0), 0);
        assert_eq!(quant_stop(2.0), u16::MAX);
    }

    #[test]
    fn a_path_verb_stays_at_the_width_the_wire_is_sized_for() {
        assert_eq!(size_of::<PathVerb>(), 28);
    }

    #[test]
    fn every_linear_spread_has_two_ends_and_the_centred_ones_have_none() {
        for spread in [
            Spread::Horizontal,
            Spread::HorizontalFeathered { edge: 0.1, inset: 0.0 },
            Spread::Vertical,
            Spread::VerticalFeathered { edge: 0.1 },
            Spread::DiagonalDown,
            Spread::DiagonalUp,
        ] {
            assert!(spread.ends().is_some(), "{spread:?}");
        }
        assert!(Spread::Radial.ends().is_none());
        assert!(
            Spread::Conic {
                center: [0.5, 0.5],
                start: 0.0
            }
            .ends()
            .is_none()
        );
        assert_eq!(
            Spread::HorizontalFeathered { edge: 0.25, inset: 0.0 }.edge(),
            Some(0.25)
        );
        assert_eq!(Spread::Horizontal.edge(), None);
    }

    #[test]
    fn a_mask_and_a_paint_read_only_the_generations_they_depend_on() {
        let built = Gen::default();
        let dpi = Gen { dpi: 1, ..built };
        let color = Gen { color: 1, ..built };
        assert!(
            !Mask::Box {
                radius: Corners::all(4.0)
            }
            .deps()
            .fresh(built, dpi)
        );
        assert!(
            Mask::Box {
                radius: Corners::all(4.0)
            }
            .deps()
            .fresh(built, color)
        );
        assert!(Paint::Solid(Radiance::TRANSPARENT).deps().fresh(built, dpi));
        assert!(
            !Paint::Solid(Radiance::TRANSPARENT)
                .deps()
                .fresh(built, color)
        );
        assert!(Mask::Run(RunId::FIRST).deps().fresh(built, dpi));
    }

    #[test]
    fn a_mask_and_a_paint_name_the_resource_the_applier_refcounts() {
        assert_eq!(
            Mask::Run(RunId::FIRST).holds(),
            Some(Holding::Run(RunId::FIRST))
        );
        assert_eq!(
            Mask::Shape {
                geom: GeomId::FIRST,
                stroke: None,
                space: PathSpace::Local,
            }
            .holds(),
            Some(Holding::Geom(GeomId::FIRST))
        );
        assert_eq!(Mask::None.holds(), None);
        assert_eq!(
            Paint::Presented { region: RegionId::FIRST, origin: Vector2::zero() }.holds(),
            Some(Holding::Region(RegionId::FIRST))
        );
        assert_eq!(Paint::Solid(Radiance::TRANSPARENT).holds(), None);
        assert_eq!(
            Holding::Geom(GeomId::FIRST).id().family(),
            GEOM,
            "a holding must name the family its row is filed under"
        );
    }
}
