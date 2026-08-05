//! The `Send` seam: `Copy` ops over typed side-buffers. **App half.**
//!
//! [`Op`] and [`SinkPatch`] are asserted `Send` at compile time. A generated COM interface
//! holds a raw pointer and declares no `Send`, so no composition object can reach the app
//! thread through a patch.
//!
//! Every variable-length payload travels as a [`Span`] into a typed side-buffer, one buffer
//! per payload kind, and the applier bounds-checks the span where it reads it back.

use crate::env::Env;
use crate::hit_build::HitEntry;
use crate::sink::*;
use windows_color::Radiance;
use windows_text::{GlyphSeg, SegBuffers};

pub use windows_text::Span;

/// Where a new node attaches.
///
/// The two parentless cases are separate variants: the front half seats a window root and a
/// detached root differently, and a single `NodeId::NONE` standing for both would name
/// neither.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Attach {
    /// An ordinary child of another node.
    Node(NodeId),
    /// The window's own container, which the front half owns. The model names what goes
    /// into the container and never the container itself.
    Window,
    /// A slot root: a flyout, popup, tooltip or ghost. Placed in absolute window
    /// DIPs by its own solve, scanned at the tail of the hit array, and above every
    /// window-attached node in z-order.
    Detached,
}

impl Attach {
    /// Returns the parent node, or `None` for [`Attach::Window`] and [`Attach::Detached`].
    #[must_use]
    pub const fn node(self) -> Option<NodeId> {
        match self {
            Self::Node(id) => Some(id),
            Self::Window | Self::Detached => None,
        }
    }
}

/// One instruction to the front half.
///
/// `Copy` throughout: no `Rc`, no COM interface and no closure can appear in a variant.
/// Event handlers stay in the app thread's own maps and cross as declarations the front
/// thread consults rather than holds.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Op {
    /// Mints a node and attaches it. `after` names the sibling to sit above, or `None` for
    /// the bottom of the collection: a visual collection offers insert-at-bottom,
    /// insert-above and remove, and no insert-at-index.
    New {
        id: NodeId,
        kind: NodeKind,
        parent: Attach,
        after: Option<NodeId>,
    },
    /// Reparents or reorders one node. The keyed structure diff upstream computes the
    /// minimal set of moves, and this op carries one of them.
    Move {
        id: NodeId,
        parent: NodeId,
        after: Option<NodeId>,
    },
    /// Destroys a node and its subtree, releasing every resource on the way down. A
    /// subtree removal is one op, and a partial destroy is not expressible.
    Drop {
        id: NodeId,
        exit: Exit,
    },
    Mask {
        id: SpriteId,
        mask: Mask,
    },
    Paint {
        id: SpriteId,
        paint: Paint,
    },
    /// Bounds what a node's subtree may draw inside.
    ///
    /// Addressed to a [`NodeId`] because groups clip and a group carries no mask or paint.
    /// The clip's kind selects which object is minted — a rectangle clip and a geometric
    /// clip are different objects — and the sides and radii are channels of that object.
    Clip {
        id: NodeId,
        clip: Clip,
    },
    Bind {
        id: NodeId,
        prop: Prop,
        bind: Bind,
    },
    Res {
        id: ResId,
        op: ResOp,
    },
    Tracker {
        id: crate::id::Id<Tracker>,
        op: TrackerOp,
    },
    /// Replaces the whole hit table. Issued when layout changed and at no other time, so
    /// this is not a per-frame path.
    Hits {
        entries: Span,
    },
    /// Starts a timed reveal, reported back as
    /// [`SceneEvent::DelayElapsed`](crate::SceneEvent::DelayElapsed).
    ///
    /// The deadline is monotonic and read on the frame clock rather than armed on a timer,
    /// so it is observed on a frame the scene was servicing anyway and wakes none of its
    /// own. Re-issuing a live id restarts the deadline.
    Delay {
        id: DelayId,
        ms: u32,
    },
    /// Cancels a delay by dropping it. A cancelled delay never reports.
    CancelDelay {
        id: DelayId,
    },
}

/// A pending patch: the ops, and the buffers their payloads live in.
///
/// The buffers are pooled: the front thread hands a drained patch back and the app thread
/// refills it, so a forty-stop ramp or a four-hundred-entry hit table allocates nothing once
/// the buffers have reached their working size.
#[derive(Debug, Default)]
pub struct SinkPatch {
    pub(crate) ops: Vec<Op>,
    pub(crate) verbs: Vec<PathVerb>,
    pub(crate) stops: Vec<(f32, Radiance)>,
    pub(crate) frames: Vec<(f32, Value, Easing)>,
    pub(crate) dashes: Vec<f32>,
    pub(crate) hits: Vec<HitEntry>,
    /// Segments and the glyph data they span, in one type, so they are pooled and cleared
    /// together: a segment resolved against a buffer it did not travel with is a span into
    /// the wrong bytes. `windows-text` appends straight into this.
    pub(crate) text: SegBuffers,
    /// The environment this patch's geometry was solved under, or `None` before it has been
    /// flushed.
    ///
    /// The applier compares it against the environment it is applying under, so geometry
    /// snapped to a different pixel grid than the one in force is visible. A mismatch is
    /// counted as [`Census::env_mismatches`](crate::Census::env_mismatches) and not refused.
    pub(crate) env: Option<Env>,
}

/// Fails to compile if a patch or an op ever gains a field that is not `Send`.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<SinkPatch>();
    assert_send::<Op>();
};

impl SinkPatch {
    /// Returns an empty patch that has allocated nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties every buffer and keeps its allocation.
    pub fn clear(&mut self) {
        // The environment goes with the contents: a drained patch has been applied, and
        // nothing has been solved into a pooled one yet.
        self.env = None;
        self.ops.clear();
        self.verbs.clear();
        self.stops.clear();
        self.frames.clear();
        self.dashes.clear();
        self.hits.clear();
        self.text.clear();
    }

    /// Returns whether the patch instructs the front half to do anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Returns the number of ops the patch carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Returns the ops, in the order they must be applied.
    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// Returns the environment this patch's geometry was solved under, or `None` before it
    /// has been flushed.
    #[must_use]
    pub fn env(&self) -> Option<Env> {
        self.env
    }

    /// Returns the glyph buffers, for a producer appending a shaped run into them.
    ///
    /// `ShapedRun::segments` appends here and returns the span naming what it wrote, so a
    /// run reaches the patch without an intermediate copy.
    pub fn text(&mut self) -> &mut SegBuffers {
        &mut self.text
    }

    // ── appending payloads ────────────────────────────────────────────────────────
    //
    // Each returns the span naming what it appended, so a caller builds the payload and
    // the op that reads it in one expression.

    pub(crate) fn push_op(&mut self, op: Op) {
        self.ops.push(op);
    }

    pub(crate) fn push_verbs(&mut self, verbs: &[PathVerb]) -> Span {
        Self::extend(&mut self.verbs, verbs)
    }

    pub(crate) fn push_stops(&mut self, stops: &[(f32, Radiance)]) -> Span {
        Self::extend(&mut self.stops, stops)
    }

    pub(crate) fn push_frames(&mut self, frames: &[(f32, Value, Easing)]) -> Span {
        Self::extend(&mut self.frames, frames)
    }

    pub(crate) fn push_dashes(&mut self, dashes: &[f32]) -> Span {
        Self::extend(&mut self.dashes, dashes)
    }

    pub(crate) fn push_segs(&mut self, segs: &[GlyphSeg]) -> Span {
        Self::extend(&mut self.text.segs, segs)
    }

    /// Returns the hit table's buffer, for a producer writing entries straight into it.
    pub(crate) fn hits_mut(&mut self) -> &mut Vec<HitEntry> {
        &mut self.hits
    }

    /// Returns the number of entries the hit table holds.
    #[must_use]
    pub fn hits_len(&self) -> usize {
        self.hits.len()
    }

    /// Returns the whole hit table, in z-order.
    #[must_use]
    pub fn hit_entries(&self) -> &[HitEntry] {
        &self.hits
    }

    fn extend<T: Copy>(buffer: &mut Vec<T>, items: &[T]) -> Span {
        let off = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
        buffer.extend_from_slice(items);
        Span::new(off, u32::try_from(items.len()).unwrap_or(u32::MAX))
    }

    // ── reading them back, on the far side ────────────────────────────────────────

    pub(crate) fn verbs(&self, span: Span) -> &[PathVerb] {
        span.of(&self.verbs)
    }

    pub(crate) fn stops(&self, span: Span) -> &[(f32, Radiance)] {
        span.of(&self.stops)
    }

    pub(crate) fn frames(&self, span: Span) -> &[(f32, Value, Easing)] {
        span.of(&self.frames)
    }

    pub(crate) fn dashes(&self, span: Span) -> &[f32] {
        span.of(&self.dashes)
    }

    pub(crate) fn segs(&self, span: Span) -> &[GlyphSeg] {
        span.of(&self.text.segs)
    }

    pub(crate) fn hits(&self, span: Span) -> &[HitEntry] {
        span.of(&self.hits)
    }

    pub(crate) fn glyphs(&self) -> &SegBuffers {
        &self.text
    }
}

/// A pool of drained patches, so the two threads swap buffers rather than allocate them.
///
/// One patch is in flight at a time: the front thread hands a patch back as soon as it has
/// applied it.
#[derive(Debug, Default)]
pub struct PatchPool(Vec<SinkPatch>);

impl PatchPool {
    /// Returns a drained patch, from the pool or freshly allocated.
    pub fn take(&mut self) -> SinkPatch {
        self.0.pop().unwrap_or_default()
    }

    /// Takes a patch back, drains it, and holds it for reuse.
    pub fn give(&mut self, mut patch: SinkPatch) {
        patch.clear();
        // Two is the working set: one patch being filled and one being applied.
        if self.0.len() < 2 {
            self.0.push(patch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_reads_back_exactly_what_was_appended() {
        let mut patch = SinkPatch::new();
        let first = patch.push_dashes(&[1.0, 2.0]);
        let second = patch.push_dashes(&[3.0]);
        assert_eq!(patch.dashes(first), &[1.0, 2.0]);
        assert_eq!(patch.dashes(second), &[3.0]);
    }

    #[test]
    fn clearing_keeps_the_allocations() {
        let mut patch = SinkPatch::new();
        patch.push_dashes(&[1.0; 64]);
        let capacity = patch.dashes.capacity();
        patch.clear();
        assert!(patch.dashes.is_empty());
        assert_eq!(patch.dashes.capacity(), capacity);
    }

    #[test]
    fn a_pooled_patch_comes_back_empty() {
        let mut pool = PatchPool::default();
        let mut patch = pool.take();
        patch.push_op(Op::Drop {
            id: NodeId::NONE,
            exit: Exit::None,
        });
        pool.give(patch);
        assert!(pool.take().is_empty());
    }
}
