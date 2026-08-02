//! The `Send` seam: `Copy` ops over typed side-buffers. **App half.**
//!
//! The app thread never touches a composition object, and one `const` assertion proves it:
//! a generated interface holds a raw pointer and nothing in the generated surface declares
//! itself `Send`, so `Send` here means precisely "no COM rode the wire".
//!
//! Every variable-length payload travels as a [`Span`] into a typed side-buffer. Pooling is
//! then one buffer per payload *kind* rather than one per op, the applier's bounds check
//! happens once at the seam, and a thread-affine value has nowhere to hide.

use crate::env::Env;
use crate::hit_build::HitEntry;
use crate::sink::*;
use windows_color::Radiance;
use windows_text::{GlyphSeg, SegBuffers};

pub use windows_text::Span;

/// One instruction to the front half.
///
/// `Copy`, unconditionally. No `Rc`, no COM interface, no closure can enter — event
/// handlers stay in the app thread's own maps and cross as *declarations* that the front
/// thread consults without ever holding them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Op {
    /// Mints a node and parents it. `after` is the sibling to sit above, or `None` for the
    /// bottom of the collection — the platform's own vocabulary, since a visual collection
    /// offers insert-at-bottom, insert-above and remove and no insert-at-index.
    New {
        id: NodeId,
        kind: NodeKind,
        parent: NodeId,
        after: Option<NodeId>,
    },
    /// Reparents or reorders. The keyed structure diff upstream already computed the
    /// minimal set of moves, so this carries its *result*.
    Move {
        id: NodeId,
        parent: NodeId,
        after: Option<NodeId>,
    },
    /// Destroys a node **and its subtree**, decrementing every resource on the way down.
    /// A subtree removal is one op, and a partial destroy is not expressible.
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
    /// What a node's subtree may draw inside.
    ///
    /// Its own op because a clip's *kind* identifies rather than animates — a rectangle
    /// and a geometry are different objects, not different values of one — and because it
    /// is addressed to a `NodeId`: groups clip, and a group has no mask or paint to carry
    /// it on. The sides and radii are channels of whatever this mints.
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
    /// Replaces the whole hit table. Not a per-frame path: the array is rebuilt when
    /// layout changed and at no other time.
    Hits {
        entries: Span,
    },
}

/// A pending patch: the ops, and the buffers their payloads live in.
///
/// Buffers are **pooled**. The front thread hands the drained patch back and the app
/// thread refills it, so a forty-stop ramp or a four-hundred-entry hit table costs zero
/// allocations after warm-up.
#[derive(Debug, Default)]
pub struct SinkPatch {
    pub(crate) ops: Vec<Op>,
    pub(crate) verbs: Vec<PathVerb>,
    pub(crate) stops: Vec<(f32, Radiance)>,
    pub(crate) frames: Vec<(f32, Value, Easing)>,
    pub(crate) dashes: Vec<f32>,
    pub(crate) hits: Vec<HitEntry>,
    /// Segments *and* the glyph data they span, in one type. They are pooled together and
    /// cleared together because a segment addressing a buffer it did not travel with is a
    /// span into the wrong bytes; `windows-text` appends straight into this.
    pub(crate) text: SegBuffers,
    /// The environment this patch's geometry was solved under. `None` on a patch that has
    /// not been flushed.
    ///
    /// Carried so the far side can *notice* when it is applying geometry snapped to one
    /// pixel grid — the whole failure the [`Env`] seam exists to prevent, one level up
    /// where the two halves meet. It is not a rule the applier enforces, because a
    /// mismatch is not necessarily wrong: see [`Census::env_mismatches`].
    pub(crate) env: Option<Env>,
}

/// The whole `Send` invariant, and it is the whole proof.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<SinkPatch>();
    assert_send::<Op>();
};

impl SinkPatch {
    /// An empty patch with no allocations yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties every buffer, keeping the allocations. This is the whole of the pooling.
    pub fn clear(&mut self) {
        // The stamp goes with the contents: a drained patch has been applied, and a
        // pooled one has not been solved for anything yet.
        self.env = None;
        self.ops.clear();
        self.verbs.clear();
        self.stops.clear();
        self.frames.clear();
        self.dashes.clear();
        self.hits.clear();
        self.text.clear();
    }

    /// Whether it instructs the front half to do anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// How many ops it carries. The census counts against this.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// The ops, in the order they must be applied.
    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// The environment this patch's geometry was solved under, once it has been flushed.
    #[must_use]
    pub fn env(&self) -> Option<Env> {
        self.env
    }

    /// The glyph buffers, for a producer appending a shaped run into them.
    ///
    /// `ShapedRun::segments` appends here and returns the span naming what it wrote, so a
    /// run reaches the patch without a copy in between.
    pub fn text(&mut self) -> &mut SegBuffers {
        &mut self.text
    }

    // ── appending payloads ────────────────────────────────────────────────────────
    //
    // Each returns the span naming what it appended, so a caller builds the payload and
    // the op that reads it in one expression and cannot pair the wrong two.

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

    /// The hit table's buffer, written straight into rather than through an intermediate.
    pub(crate) fn hits_mut(&mut self) -> &mut Vec<HitEntry> {
        &mut self.hits
    }

    /// How many entries the table holds.
    #[must_use]
    pub fn hits_len(&self) -> usize {
        self.hits.len()
    }

    /// The whole hit table, in z-order.
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
/// Not a channel: a channel would impose a queue discipline on something that is one
/// buffer in flight at a time, and the front thread hands a patch back the moment it has
/// applied it.
#[derive(Debug, Default)]
pub struct PatchPool(Vec<SinkPatch>);

impl PatchPool {
    /// A drained patch, from the pool or fresh.
    pub fn take(&mut self) -> SinkPatch {
        self.0.pop().unwrap_or_default()
    }

    /// Returns a patch, drained, for reuse.
    pub fn give(&mut self, mut patch: SinkPatch) {
        patch.clear();
        // Two is the working set: one being filled and one being applied. A third would be
        // memory held against a burst that the pool's own growth already absorbs.
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
