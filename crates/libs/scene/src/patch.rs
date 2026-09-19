//! The `Send` seam: `Copy` ops over typed side-buffers. **App half.**
//!
//! Every variable-length payload rides a typed side-buffer, so pooling is one `Vec` per
//! payload kind rather than one per op, and the applier's bounds check happens once where it
//! reads the span back.

use crate::hit_entry::HitEntry;
use crate::sink::*;
use windows_color::Radiance;

/// Addresses an `(offset, count)` window into one of a [`SinkPatch`]'s side-buffers.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Span {
    pub off: u32,
    pub len: u32,
}

/// Where a root attaches.
///
/// The window band holds content and the overlay band holds slot roots and the ghosts an
/// exit leaves behind, so an overlay sits above content by its position in the tree rather
/// than by an ordering every caller has to keep.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Attach {
    Node(NodeId),
    Window,
    Overlay,
}

impl Attach {
    /// The parent node, or `None` for the two band attachments.
    #[must_use]
    pub const fn node(self) -> Option<NodeId> {
        match self {
            Self::Node(id) => Some(id),
            Self::Window | Self::Overlay => None,
        }
    }
}

/// One instruction to the scene half.
///
/// `after`, not `index`: a visual collection offers insert-at-bottom, insert-above and
/// remove and no insert-at-index, so the wire speaks the platform's vocabulary and the
/// applier is a direct call with no translation. There is no reorder op, because the keyed
/// structure diff upstream already computes the minimal moves.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Op {
    New {
        id: NodeId,
        kind: NodeKind,
        parent: Attach,
        after: Option<NodeId>,
    },
    Move {
        id: NodeId,
        parent: Attach,
        after: Option<NodeId>,
    },
    /// Cascades to the subtree, so a partial destroy is unrepresentable. `origin` and
    /// `bounds` are the solved rect and clip chain the app already holds, which is what an
    /// exit's ghost is mounted and sized from.
    Drop {
        id: NodeId,
        exit: Exit,
        origin: Point,
        bounds: Option<[f32; 4]>,
    },
    Mask {
        id: SpriteId,
        mask: Mask,
    },
    Paint {
        id: SpriteId,
        paint: Paint,
        halo: Option<Halo>,
    },
    /// Addressed to a node because groups clip and have no mask or paint to carry it, and
    /// its own op because a clip's *kind* identifies rather than animates.
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
        id: TrackerId<()>,
        op: TrackerOp,
    },
    /// Whole-table replace. `index` is `(ControlId, u32)` ordered by id, built app-side, so
    /// the table's id lookup is an `extend_from_slice` and never a sort.
    Hits {
        entries: Span,
        index: Span,
    },
    /// Starts a timed reveal, or cancels the one registered under `id` with `None`.
    Delay {
        id: DelayId,
        ms: Option<u32>,
    },
}

/// One pass of app-thread decisions, as `Copy` ops over typed side-buffers.
#[derive(Default)]
pub struct SinkPatch {
    /// The environment this patch's geometry was solved under, where the emitter states one.
    pub env: Option<Env>,
    ops: Vec<Op>,
    verbs: Vec<PathVerb>,
    stops: Vec<(u16, Radiance)>,
    frames: Vec<(f32, Value, Easing)>,
    floats: Vec<f32>,
    segs: Vec<GlyphSeg>,
    glyphs: Vec<u16>,
    hits: Vec<HitEntry>,
    index: Vec<(ControlId, u32)>,
}

/// The whole proof that no composition object crossed the seam: a generated interface holds
/// a raw pointer and `windows-core` declares no `Send` for any of them.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<Op>();
    assert_send::<SinkPatch>();
};

/// One appender and one span reader per side-buffer, so the arithmetic is written once.
macro_rules! buffers {
    ($($push:ident, $read:ident, $field:ident: $ty:ty;)*) => { $(
        impl SinkPatch {
            pub fn $push(&mut self, items: &[$ty]) -> Span {
                let off = self.$field.len() as u32;
                self.$field.extend_from_slice(items);
                Span { off, len: items.len() as u32 }
            }

            /// The items `span` covers, or `&[]` where it runs past the buffer. One bounds
            /// check, at the seam: a mismatched pair presents as a missing payload rather
            /// than a panic inside a draw call.
            #[must_use]
            #[allow(
                clippy::should_implement_trait,
                reason = "the hit index's reader is named for the buffer it reads, as every                           other reader here is; it takes a span and is not an indexing operator"
            )]
            pub fn $read(&self, span: Span) -> &[$ty] {
                let (off, len) = (span.off as usize, span.len as usize);
                self.$field.get(off..off + len).unwrap_or_default()
            }
        }
    )* };
}

buffers! {
    push_verbs, verbs, verbs: PathVerb;
    push_stops, stops, stops: (u16, Radiance);
    push_frames, frames, frames: (f32, Value, Easing);
    push_floats, floats, floats: f32;
    push_segs, segs, segs: GlyphSeg;
    push_glyphs, glyphs, glyphs: u16;
    push_hits, hits, hits: HitEntry;
    push_index, index, index: (ControlId, u32);
}

impl SinkPatch {
    pub fn push(&mut self, op: Op) {
        self.ops.push(op);
    }

    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// The whole glyph buffer. A run's segments address it absolutely, so the applier hands
    /// the pool to the rasterizer and each segment's span cuts its own window out of it.
    #[must_use]
    pub fn glyph_pool(&self) -> &[u16] {
        &self.glyphs
    }

    /// The whole float buffer, which a run's advances and offsets address absolutely.
    #[must_use]
    pub fn float_pool(&self) -> &[f32] {
        &self.floats
    }

    /// The hit array's own buffer, to build into rather than copy into.
    ///
    /// The table is replaced whole and one [`Op::Hits`] carries it, so a builder clears this
    /// and refills it, then names the whole of it through [`hits_span`](Self::hits_span). A
    /// second builder in one patch would overwrite the first.
    pub fn hits_mut(&mut self) -> &mut Vec<HitEntry> {
        &mut self.hits
    }

    /// The id index's own buffer, built alongside [`hits_mut`](Self::hits_mut) and ordered
    /// by id on the way out.
    pub fn index_mut(&mut self) -> &mut Vec<(ControlId, u32)> {
        &mut self.index
    }

    /// The span covering everything [`hits_mut`](Self::hits_mut) holds.
    #[must_use]
    pub fn hits_span(&self) -> Span {
        Span {
            off: 0,
            len: self.hits.len() as u32,
        }
    }

    /// The span covering everything [`index_mut`](Self::index_mut) holds.
    #[must_use]
    pub fn index_span(&self) -> Span {
        Span {
            off: 0,
            len: self.index.len() as u32,
        }
    }

    /// Clears every buffer and keeps every allocation, which is what makes a 40-stop ramp or
    /// a 400-entry hit table cost nothing after warm-up.
    pub fn clear(&mut self) {
        self.env = None;
        self.ops.clear();
        self.verbs.clear();
        self.stops.clear();
        self.frames.clear();
        self.floats.clear();
        self.segs.clear();
        self.glyphs.clear();
        self.hits.clear();
        self.index.clear();
    }
}

/// Returns drained patches to the app thread so neither side allocates per frame.
#[derive(Default)]
pub struct PatchPool(Vec<SinkPatch>);

impl PatchPool {
    pub fn take(&mut self) -> SinkPatch {
        self.0.pop().unwrap_or_default()
    }

    pub fn give(&mut self, mut patch: SinkPatch) {
        patch.clear();
        self.0.push(patch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_numerics::Vector2;

    #[test]
    fn a_span_reads_back_exactly_what_was_appended() {
        let mut patch = SinkPatch::default();
        let first = patch.push_floats(&[1.0, 2.0]);
        let second = patch.push_floats(&[3.0]);
        assert_eq!(patch.floats(first), &[1.0, 2.0]);
        assert_eq!(patch.floats(second), &[3.0]);
        assert_eq!(patch.floats(Span { off: 0, len: 9 }), &[] as &[f32]);
    }

    #[test]
    fn clearing_keeps_the_allocations_the_next_pass_writes_into() {
        let mut patch = SinkPatch::default();
        patch.push_verbs(&[PathVerb::Line(Vector2 { x: 1.0, y: 1.0 })]);
        patch.push(Op::Delay {
            id: DelayId::FIRST,
            ms: Some(4),
        });
        patch.env = Some(Env::new(
            96.0,
            windows_color::OutputTransform::for_display(
                windows_color::DisplayCapability::Sdr,
                203.0,
            ),
        ));
        patch.clear();
        assert!(patch.is_empty());
        assert!(patch.env.is_none());
        assert_eq!(patch.verbs(Span { off: 0, len: 1 }), &[] as &[PathVerb]);
    }

    #[test]
    fn the_pool_hands_back_a_cleared_patch() {
        let mut pool = PatchPool::default();
        let mut patch = pool.take();
        patch.push(Op::Delay {
            id: DelayId::FIRST,
            ms: None,
        });
        pool.give(patch);
        let reused = pool.take();
        assert!(reused.is_empty());
    }

    #[test]
    fn the_hit_buffers_are_built_in_place_and_named_whole() {
        let mut patch = SinkPatch::default();
        patch.hits_mut().clear();
        patch.index_mut().clear();
        assert_eq!(patch.hits_span(), Span { off: 0, len: 0 });
        patch.index_mut().push((ControlId::FIRST, 0));
        assert_eq!(patch.index_span(), Span { off: 0, len: 1 });
        assert_eq!(patch.index(patch.index_span()), &[(ControlId::FIRST, 0)]);
    }
}
