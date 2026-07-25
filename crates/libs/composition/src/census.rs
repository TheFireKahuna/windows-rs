//! A running census of the composition objects this process has minted, the
//! visuals it currently has parented, and the property traffic it pushes.
//!
//! None of this is observable through the composition API itself. Object
//! properties are write-only by design (the composition engine is asynchronous,
//! so a getter's answer can be stale the moment it returns), the visual tree
//! exposes a child `count` but no totals, and nothing at all reports how much
//! the app writes per frame. Yet those are exactly the numbers that decide what
//! the compositor process costs: DWM produces one frame per committed batch, and
//! the work in that frame scales with the number of visuals it walks rather than
//! with the pixels that changed. An app that cannot count its own visuals cannot
//! reason about that cost — it can only guess from profiler symbol names.
//!
//! So the wrapper layer counts. Every mint, every parenting change, and every
//! write to a property that live animation drives passes through exactly one
//! method here, and each bumps one relaxed atomic.
//!
//! ## What is counted, precisely
//!
//! *Mints* are monotonic totals of objects created, never decremented — their
//! rate is the question they answer ("is something rebuilding visuals per
//! frame?"), not their absolute value.
//!
//! *Parenting* is `inserts - removes` over [`VisualCollection`](crate::VisualCollection),
//! so [`Census::parented`] is the number of visuals currently in **some** tree.
//! It is not the number of visuals the compositor renders: a visual parented
//! under a detached root still counts. Treat it as a cheap running figure and an
//! authoritative walk of the tree as the ground truth; a divergence between the
//! two is itself a finding.
//!
//! *Property writes* cover the [`Visual`](crate::Visual) properties and the
//! shape/geometry properties that per-frame animation actually drives — offset,
//! size, scale, opacity, visibility, path, trim, stroke, and the assignment of a
//! brush or a clip. Two things are deliberately outside that line. The one-time
//! configuration of an animation object (key frames, durations, easing) changes
//! nothing in the tree until the animation is started, and counting it would
//! drown the per-frame signal in build-time noise. Mutating a *shared* brush or
//! clip object in place is likewise uncounted: one such write can redraw every
//! visual that references it, so attributing it as a single property write would
//! understate it — if that traffic ever matters it needs a counter of its own,
//! not a slot in this one.
//!
//! ## Cost
//!
//! One relaxed `fetch_add` per counted operation, uncontended in the ordinary
//! case because the visual tree is thread-affine. Every counted operation is a
//! cross-apartment vtable call that costs orders of magnitude more, so the
//! census is always on: a diagnostic that needs a rebuild to enable is a
//! diagnostic that is never available when the question is asked.

use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

/// The counted operations. One slot each in [`COUNTS`]; the discriminant is the
/// index, so a bump is an array store with no branching.
#[derive(Clone, Copy)]
pub(crate) enum Count {
    // ── mints ──
    ContainerVisual,
    SpriteVisual,
    ShapeVisual,
    SpriteShape,
    ContainerShape,
    Geometry,
    Brush,
    Clip,
    DrawingSurface,
    VisualSurface,
    Animation,
    // ── parenting ──
    TreeInsert,
    TreeRemove,
    // ── traffic ──
    PropertyWrite,
    AnimationStart,
    AnimationStop,
    SurfaceDraw,
}

/// One slot per [`Count`] variant. Keep in step with the enum — the last variant
/// must index inside this array, which [`Census::take`] relies on.
const SLOTS: usize = 17;

static COUNTS: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];

/// Record one counted operation.
#[inline]
pub(crate) fn bump(which: Count) {
    add(which, 1);
}

/// Record `n` counted operations at once — for the bulk removals
/// ([`VisualCollection::remove_all`](crate::VisualCollection::remove_all)) that
/// would otherwise have to loop purely to count.
#[inline]
pub(crate) fn add(which: Count, n: u64) {
    COUNTS[which as usize].fetch_add(n, Ordering::Relaxed);
}

/// A snapshot of the census, taken atomically per counter (but not across
/// counters — a snapshot taken mid-frame can catch an insert without its
/// matching write, which matters only if you are diffing single frames).
#[derive(Clone, Copy, Debug, Default)]
pub struct Census {
    pub container_visuals: u64,
    pub sprite_visuals: u64,
    pub shape_visuals: u64,
    pub sprite_shapes: u64,
    pub container_shapes: u64,
    pub geometries: u64,
    pub brushes: u64,
    pub clips: u64,
    pub drawing_surfaces: u64,
    pub visual_surfaces: u64,
    pub animations: u64,
    pub tree_inserts: u64,
    pub tree_removes: u64,
    pub property_writes: u64,
    pub animations_started: u64,
    pub animations_stopped: u64,
    pub surface_draws: u64,
}

impl Census {
    fn take() -> Self {
        let at = |c: Count| COUNTS[c as usize].load(Ordering::Relaxed);
        Self {
            container_visuals: at(Count::ContainerVisual),
            sprite_visuals: at(Count::SpriteVisual),
            shape_visuals: at(Count::ShapeVisual),
            sprite_shapes: at(Count::SpriteShape),
            container_shapes: at(Count::ContainerShape),
            geometries: at(Count::Geometry),
            brushes: at(Count::Brush),
            clips: at(Count::Clip),
            drawing_surfaces: at(Count::DrawingSurface),
            visual_surfaces: at(Count::VisualSurface),
            animations: at(Count::Animation),
            tree_inserts: at(Count::TreeInsert),
            tree_removes: at(Count::TreeRemove),
            property_writes: at(Count::PropertyWrite),
            animations_started: at(Count::AnimationStart),
            animations_stopped: at(Count::AnimationStop),
            surface_draws: at(Count::SurfaceDraw),
        }
    }

    /// Visuals currently parented somewhere — `inserts - removes`. Signed
    /// because a re-parenting sequence observed mid-flight can transiently show
    /// more removes than inserts, and clamping that to zero would hide it.
    pub fn parented(&self) -> i64 {
        self.tree_inserts as i64 - self.tree_removes as i64
    }

    /// Every visual this process has ever minted, of any kind.
    pub fn visuals_minted(&self) -> u64 {
        self.container_visuals + self.sprite_visuals + self.shape_visuals
    }

    /// Field-by-field difference from an earlier snapshot, for rate reporting.
    /// Saturating, so a counter reset between the two reads as zero rather than
    /// wrapping to something enormous.
    pub fn since(&self, earlier: &Self) -> Self {
        macro_rules! d {
            ($($f:ident),* $(,)?) => { Self { $($f: self.$f.saturating_sub(earlier.$f)),* } };
        }
        d!(
            container_visuals,
            sprite_visuals,
            shape_visuals,
            sprite_shapes,
            container_shapes,
            geometries,
            brushes,
            clips,
            drawing_surfaces,
            visual_surfaces,
            animations,
            tree_inserts,
            tree_removes,
            property_writes,
            animations_started,
            animations_stopped,
            surface_draws,
        )
    }
}

impl fmt::Display for Census {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "  visuals minted   container {:>7}  sprite {:>7}  shape {:>7}  (total {})",
            self.container_visuals,
            self.sprite_visuals,
            self.shape_visuals,
            self.visuals_minted(),
        )?;
        writeln!(
            f,
            "  shapes minted    sprite    {:>7}  group  {:>7}  geometry {:>5}",
            self.sprite_shapes, self.container_shapes, self.geometries,
        )?;
        writeln!(
            f,
            "  other minted     brush     {:>7}  clip   {:>7}  animation {:>4}",
            self.brushes, self.clips, self.animations,
        )?;
        writeln!(
            f,
            "  surfaces minted  drawing   {:>7}  visual {:>7}  draws {:>8}",
            self.drawing_surfaces, self.visual_surfaces, self.surface_draws,
        )?;
        writeln!(
            f,
            "  tree             inserts   {:>7}  removes{:>7}  parented {:>5}",
            self.tree_inserts,
            self.tree_removes,
            self.parented(),
        )?;
        write!(
            f,
            "  traffic          writes    {:>7}  anim start {:>4}  stop {:>7}",
            self.property_writes, self.animations_started, self.animations_stopped,
        )
    }
}

/// Snapshot every counter.
pub fn census() -> Census {
    Census::take()
}

/// Zero every counter — for measuring one interval without arithmetic against a
/// baseline. Racy against concurrent composition work by construction; it exists
/// for "reset, do the thing, read", not for use while the tree is being built.
pub fn reset_census() {
    for c in &COUNTS {
        c.store(0, Ordering::Relaxed);
    }
}
