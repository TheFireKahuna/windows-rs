//! The taffy tree, the measure seam, and pixel snapping. **App half.**
//!
//! Taffy already implements flexbox, CSS grid with auto-placement, block layout,
//! min/max/aspect-ratio, absolute positioning, gap and overflow with gutter reservation,
//! and this crate uses [`taffy::Style`] directly rather than redeclaring thirty fields and
//! a conversion between them. The one thing taffy does not have is a container that
//! classifies its own width for its subtree, and that is the whole of what is added here.
//!
//! The tree is the low-level one and not [`taffy::TaffyTree`], because the classifying
//! container *is* a `compute_child_layout` arm — and a custom tree also makes every
//! per-node table a `Vec` indexed by the node's own dense id, so no hash map appears on any
//! node path.

use crate::responsive::{Bounds, WidthClass};
use crate::sink::NodeId;
use taffy::{
    AvailableSpace, Cache, CacheTree, Display, Layout, LayoutBlockContainer,
    LayoutFlexboxContainer, LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree,
    NodeId as TaffyId, RoundTree, RunMode, Size, Style, TraversePartialTree, TraverseTree,
    compute_block_layout, compute_cached_layout, compute_flexbox_layout, compute_grid_layout,
    compute_hidden_layout, compute_leaf_layout, compute_root_layout,
};
use windows_numerics::Vector2;

/// An axis-aligned box in absolute layout DIPs.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    #[must_use]
    pub fn width(self) -> f32 {
        self.x1 - self.x0
    }

    #[must_use]
    pub fn height(self) -> f32 {
        self.y1 - self.y0
    }

    /// The smallest box containing both.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }
}

/// One node's placement, as plain data.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Solved {
    /// Absolute, window-relative, pixel-snapped, and **unscrolled** — the position layout
    /// placed the node at, before any tracker offset.
    pub rect: Rect,
    /// Offset relative to the parent group, which is what a visual's own offset is.
    pub local: Vector2,
    pub size: Vector2,
    /// Content size — the scroll extent's source.
    pub content: Vector2,
    /// Whether it clips or scrolls, and therefore confines its children.
    pub bounded: bool,
}

/// Snaps a DIP coordinate onto the physical pixel grid.
///
/// In DIP space against the physical grid, and not at the end: the root visual carries the
/// display scale, so a fractional DIP offset lands *between* physical pixels and the
/// compositor bilinear-resamples the whole surface — text and hairlines blur.
///
/// **Edges are snapped, never extents.** Snapping `x` and `x + w` keeps adjacent nodes
/// sharing an edge exactly; snapping `x` and `w` independently opens a hairline gap
/// between them at some scales and overlaps them at others.
#[must_use]
pub fn snap(v: f32, scale: f32) -> f32 {
    if !v.is_finite() {
        return 0.0;
    }
    (v * scale).round() / scale
}

/// What a node's intrinsic size comes from.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum MeasureCtx {
    /// Nothing to measure: the style is the whole answer.
    #[default]
    None,
    /// A fixed intrinsic size, in DIPs.
    Fixed(Vector2),
    /// Measured by the layer above, which owns the text engine. The key is opaque here.
    Measured(MeasureKey),
}

/// A measurement request's identity, minted and interpreted by the layer above.
///
/// This crate holds no text engine and no font ladder, so it cannot measure a string. What
/// it can do is make sure the measurement happens at the right moment and under the right
/// width class, which is the part that is easy to get wrong.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MeasureKey(pub u64);

/// What a measurement is given.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MeasureIn {
    pub key: MeasureKey,
    /// The class the enclosing responsive container resolved. Every metric a measurement
    /// depends on — type size, padding, tracking — resolves against this, which is why it
    /// is an input and not something read from ambient state.
    pub class: WidthClass,
    /// Whichever dimensions layout has already fixed.
    pub known: (Option<f32>, Option<f32>),
    /// The space available on each axis, or `None` for an indefinite one.
    pub available: (Option<f32>, Option<f32>),
}

/// Measures what this crate cannot: shaped text, and anything else whose size is content.
pub trait Measure: Send {
    fn measure(&mut self, input: MeasureIn) -> Vector2;
}

impl<F: FnMut(MeasureIn) -> Vector2 + Send> Measure for F {
    fn measure(&mut self, input: MeasureIn) -> Vector2 {
        self(input)
    }
}

/// How a node lays its children out.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum LayoutKind {
    /// Whatever `style.display` says.
    #[default]
    Container,
    /// Classifies its own inline size for its subtree, then delegates.
    ///
    /// **Its inline size must be determined by its parent.** A content-sized one has
    /// nothing to classify, and that is a bug that announces itself: on the final layout
    /// call a parent-determined container arrives with a known inline size and a
    /// content-sized one does not, so the assertion fires at exactly the moment the
    /// problem exists.
    Responsive(Bounds),
}

#[derive(Debug)]
struct LayoutNode {
    style: Style,
    kind: LayoutKind,
    measure: MeasureCtx,
    children: Vec<TaffyId>,
    /// Who to walk up to when this node is dirtied. Taffy caches a node's output keyed on
    /// its *input*, so a change here that does not change a parent's input — hiding a
    /// child, re-measuring a leaf — leaves the parent's cached answer standing.
    parent: Option<TaffyId>,
    cache: Cache,
    /// The class this node last resolved, for the hysteresis band. Meaningless on anything
    /// but a responsive node.
    class: WidthClass,
    /// Hidden independently of `style.display`.
    ///
    /// Its own flag and not a written-in `Display::None`, because hiding must be
    /// reversible without knowing what the node's display *was*: a grid that is hidden and
    /// shown again is a grid, and a style re-pushed while hidden must not reveal it.
    hidden: bool,
    unrounded: Layout,
    solved: Layout,
    live: bool,
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self {
            style: Style::DEFAULT,
            kind: LayoutKind::Container,
            measure: MeasureCtx::None,
            children: Vec::new(),
            parent: None,
            cache: Cache::new(),
            class: WidthClass::default(),
            hidden: false,
            unrounded: Layout::with_order(0),
            solved: Layout::with_order(0),
            live: false,
        }
    }
}

/// The persistent layout tree.
///
/// Persistent, and that is not an optimization detail: rebuilding it per pass allocates a
/// node and a cloned style per node per frame, and throws away the cache that makes an
/// unchanged subtree cost nothing.
pub struct LayoutTree {
    nodes: Vec<LayoutNode>,
    /// The class in scope while a subtree is being laid out. Written by a responsive node
    /// before it delegates; read by every descendant's measurement.
    ambient: WidthClass,
    measure: Option<Box<dyn Measure>>,
}

impl core::fmt::Debug for LayoutTree {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LayoutTree")
            .field("nodes", &self.nodes.len())
            .field("ambient", &self.ambient)
            .finish_non_exhaustive()
    }
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            ambient: WidthClass::default(),
            measure: None,
        }
    }

    /// Installs what measures content-sized nodes.
    pub fn on_measure(&mut self, measure: impl Measure + 'static) {
        self.measure = Some(Box::new(measure));
    }

    /// Clears `id`'s cache and every cache above it.
    ///
    /// The walk up is the whole of it, and it is not optional: taffy keys a cached layout
    /// on the input it was given, so a subtree change that leaves a parent's input alone —
    /// hiding a child, re-measuring a leaf, reordering siblings — would otherwise be
    /// answered from the parent's cache and never reach the screen.
    fn mark_dirty(&mut self, id: TaffyId) {
        let mut next = Some(id);
        while let Some(current) = next {
            let Some(node) = self.nodes.get_mut(usize::from(current)) else {
                return;
            };
            node.cache.clear();
            next = node.parent;
        }
    }

    fn slot(&mut self, node: NodeId) -> &mut LayoutNode {
        let index = node.index();
        if index >= self.nodes.len() {
            self.nodes.resize_with(index + 1, LayoutNode::default);
        }
        &mut self.nodes[index]
    }

    /// Creates a node's layout slot, or resets a reused one.
    pub fn create(&mut self, node: NodeId, kind: LayoutKind) {
        let slot = self.slot(node);
        *slot = LayoutNode {
            kind,
            live: true,
            // The `Vec` keeps its allocation: ids are dense and reused, so this slot will
            // be filled again by whatever is minted into it next.
            children: core::mem::take(&mut slot.children),
            ..LayoutNode::default()
        };
        slot.children.clear();
        self.mark_dirty(TaffyId::from(node.index()));
    }

    /// Declares how a node lays its children out, leaving everything else about it alone.
    ///
    /// Separate from [`create`](Self::create) so that becoming responsive composes in any
    /// order with styling and parenting: re-creating the slot would silently drop both.
    /// Answers whether the kind actually differed.
    pub fn set_kind(&mut self, node: NodeId, kind: LayoutKind) -> bool {
        let slot = self.slot(node);
        if slot.kind == kind {
            return false;
        }
        slot.kind = kind;
        self.mark_dirty(TaffyId::from(node.index()));
        true
    }

    /// Releases a node's slot. The `Vec` keeps its length: ids are dense and reused, so a
    /// slot is cleared rather than removed.
    pub fn destroy(&mut self, node: NodeId) {
        let id = TaffyId::from(node.index());
        self.mark_dirty(id);
        if let Some(slot) = self.nodes.get_mut(node.index()) {
            slot.children.clear();
            slot.parent = None;
            slot.live = false;
        }
    }

    /// Pushes a style. Returns whether it actually differed — the caller leaves taffy's own
    /// dirty propagation to decide what to recompute, and pushing an equal style would
    /// invalidate a subtree that did not change.
    pub fn set_style(&mut self, node: NodeId, style: &Style) -> bool {
        let slot = self.slot(node);
        if &slot.style == style {
            return false;
        }
        slot.style = style.clone();
        self.mark_dirty(TaffyId::from(node.index()));
        true
    }

    /// Hides a node without removing it, so its state and its subtree survive.
    ///
    /// The flag is the node's own, never a `Display::None` written into its style: a style
    /// carries what the node *is*, and overwriting it means a hidden grid comes back as a
    /// flexbox and a style re-pushed while hidden reveals it.
    pub fn set_hidden(&mut self, node: NodeId, hidden: bool) -> bool {
        let slot = self.slot(node);
        if slot.hidden == hidden {
            return false;
        }
        slot.hidden = hidden;
        self.mark_dirty(TaffyId::from(node.index()));
        true
    }

    pub fn set_measure(&mut self, node: NodeId, ctx: MeasureCtx) {
        let slot = self.slot(node);
        if slot.measure != ctx {
            slot.measure = ctx;
            self.mark_dirty(TaffyId::from(node.index()));
        }
    }

    /// Replaces a node's children, in paint order.
    pub fn set_children(&mut self, node: NodeId, children: &[NodeId]) {
        let parent = TaffyId::from(node.index());
        let ids: Vec<TaffyId> = children.iter().map(|c| TaffyId::from(c.index())).collect();
        let slot = self.slot(node);
        if slot.children == ids {
            return;
        }
        slot.children = ids;
        for &child in children {
            self.slot(child).parent = Some(parent);
        }
        self.mark_dirty(parent);
    }

    /// Solves the tree under `root` against a window of `size` DIPs, and writes each live
    /// node's placement into `out`, indexed by node id.
    ///
    /// Snapping happens here, at the end, in one place — and against the *absolute* rect,
    /// because that is the space edges have to agree in.
    pub fn solve(&mut self, root: NodeId, size: Vector2, scale: f32, out: &mut Vec<Solved>) {
        self.ambient = WidthClass::default();
        let taffy_root = TaffyId::from(root.index());
        compute_root_layout(
            self,
            taffy_root,
            Size {
                width: AvailableSpace::Definite(size.x),
                height: AvailableSpace::Definite(size.y),
            },
        );

        out.clear();
        out.resize(self.nodes.len(), Solved::default());
        self.gather(taffy_root, 0.0, 0.0, scale, out);
    }

    fn gather(&self, id: TaffyId, ox: f32, oy: f32, scale: f32, out: &mut Vec<Solved>) {
        let index = usize::from(id);
        let Some(node) = self.nodes.get(index) else {
            return;
        };
        let layout = node.solved;
        let (x, y) = (ox + layout.location.x, oy + layout.location.y);
        let (x0, y0) = (snap(x, scale), snap(y, scale));
        let (x1, y1) = (
            snap(x + layout.size.width, scale),
            snap(y + layout.size.height, scale),
        );
        let overflow = node.style.overflow;
        out[index] = Solved {
            rect: Rect::new(x0, y0, x1, y1),
            local: Vector2 {
                x: snap(layout.location.x, scale),
                y: snap(layout.location.y, scale),
            },
            size: Vector2 {
                x: x1 - x0,
                y: y1 - y0,
            },
            content: Vector2 {
                x: layout.content_size.width,
                y: layout.content_size.height,
            },
            bounded: overflow.x != taffy::Overflow::Visible
                || overflow.y != taffy::Overflow::Visible,
        };
        for &child in &node.children {
            self.gather(child, x, y, scale, out);
        }
    }

    fn node(&self, id: TaffyId) -> &LayoutNode {
        &self.nodes[usize::from(id)]
    }

    fn node_mut(&mut self, id: TaffyId) -> &mut LayoutNode {
        &mut self.nodes[usize::from(id)]
    }

    /// Clears every cache below `id`, which is what a class flip costs.
    ///
    /// Taffy's cache is keyed on the layout *input*, and the ambient class is not in that
    /// key — so a descendant whose own inputs did not change keeps the measurement it took
    /// under the previous class. The symptom is stale geometry only on the frame a
    /// threshold is crossed, which is the hardest kind of bug to catch by eye.
    fn clear_subtree_cache(&mut self, id: TaffyId) {
        self.node_mut(id).cache.clear();
        for index in 0..self.node(id).children.len() {
            let child = self.node(id).children[index];
            self.clear_subtree_cache(child);
        }
    }

    fn measure_leaf(&mut self, id: TaffyId, inputs: LayoutInput) -> LayoutOutput {
        let (style, ctx, class) = {
            let node = self.node(id);
            (node.style.clone(), node.measure, self.ambient)
        };
        // The measurement closure lives in this tree, and `compute_leaf_layout` borrows
        // the tree's style at the same time — so it is taken out for the call and put
        // back, rather than the whole tree being borrowed twice.
        let mut measure = self.measure.take();
        let output = compute_leaf_layout(
            inputs,
            &style,
            |_, _| 0.0,
            |known, available| {
                let size = match ctx {
                    MeasureCtx::None => Vector2 { x: 0.0, y: 0.0 },
                    MeasureCtx::Fixed(size) => size,
                    MeasureCtx::Measured(key) => {
                        measure.as_mut().map_or(Vector2 { x: 0.0, y: 0.0 }, |m| {
                            m.measure(MeasureIn {
                                key,
                                class,
                                known: (known.width, known.height),
                                available: (
                                    available.width.into_option(),
                                    available.height.into_option(),
                                ),
                            })
                        })
                    }
                };
                Size {
                    width: known.width.unwrap_or(size.x),
                    height: known.height.unwrap_or(size.y),
                }
            },
        );
        self.measure = measure;
        output
    }

    fn layout_responsive(
        &mut self,
        id: TaffyId,
        inputs: LayoutInput,
        bounds: Bounds,
    ) -> LayoutOutput {
        let definite = inputs
            .known_dimensions
            .width
            .or(match inputs.available_space.width {
                AvailableSpace::Definite(w) => Some(w),
                _ => None,
            });
        debug_assert!(
            definite.is_some() || inputs.run_mode != RunMode::PerformLayout,
            "a responsive container's inline size must be determined by its parent"
        );

        let previous = self.node(id).class;
        let class = definite.map_or(previous, |w| bounds.reclassify(w, previous));
        if class != previous {
            for index in 0..self.node(id).children.len() {
                let child = self.node(id).children[index];
                self.clear_subtree_cache(child);
            }
        }
        self.node_mut(id).class = class;

        // Scoped to the subtree: a sibling laid out afterwards sees whatever *its* own
        // enclosing container resolved, not this one's. Saving and restoring is what makes
        // that true without threading the class through every call.
        let enclosing = core::mem::replace(&mut self.ambient, class);
        let output = self.delegate(id, inputs);
        self.ambient = enclosing;
        output
    }

    fn delegate(&mut self, id: TaffyId, inputs: LayoutInput) -> LayoutOutput {
        match self.node(id).style.display {
            Display::Grid => compute_grid_layout(self, id, inputs),
            Display::Block => compute_block_layout(self, id, inputs, None),
            Display::None => compute_hidden_layout(self, id),
            _ => compute_flexbox_layout(self, id, inputs),
        }
    }
}

impl TraversePartialTree for LayoutTree {
    type ChildIter<'a> = core::iter::Copied<core::slice::Iter<'a, TaffyId>>;

    fn child_ids(&self, parent: TaffyId) -> Self::ChildIter<'_> {
        self.node(parent).children.iter().copied()
    }

    fn child_count(&self, parent: TaffyId) -> usize {
        self.node(parent).children.len()
    }

    fn get_child_id(&self, parent: TaffyId, index: usize) -> TaffyId {
        self.node(parent).children[index]
    }
}

impl TraverseTree for LayoutTree {}

impl CacheTree for LayoutTree {
    fn cache_get(&self, id: TaffyId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.node(id).cache.get(inputs)
    }

    fn cache_store(&mut self, id: TaffyId, inputs: &LayoutInput, output: LayoutOutput) {
        self.node_mut(id).cache.store(inputs, output);
    }

    fn cache_clear(&mut self, id: TaffyId) {
        self.node_mut(id).cache.clear();
    }
}

impl LayoutPartialTree for LayoutTree {
    type CoreContainerStyle<'a> = &'a Style;
    type CustomIdent = String;

    fn get_core_container_style(&self, id: TaffyId) -> Self::CoreContainerStyle<'_> {
        &self.node(id).style
    }

    fn set_unrounded_layout(&mut self, id: TaffyId, layout: &Layout) {
        let node = self.node_mut(id);
        node.unrounded = *layout;
        // Rounding is this crate's own — taffy's rounds to whole *DIPs*, where snapping has
        // to land on whole physical pixels, which at 1.5× is a third of a DIP.
        node.solved = *layout;
    }

    fn compute_child_layout(&mut self, id: TaffyId, inputs: LayoutInput) -> LayoutOutput {
        compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            let node = tree.node(id);
            if node.hidden || node.style.display == Display::None {
                return compute_hidden_layout(tree, id);
            }
            if tree.child_count(id) == 0 {
                return tree.measure_leaf(id, inputs);
            }
            match tree.node(id).kind {
                LayoutKind::Responsive(bounds) => tree.layout_responsive(id, inputs, bounds),
                LayoutKind::Container => tree.delegate(id, inputs),
            }
        })
    }
}

impl LayoutFlexboxContainer for LayoutTree {
    type FlexboxContainerStyle<'a> = &'a Style;
    type FlexboxItemStyle<'a> = &'a Style;

    fn get_flexbox_container_style(&self, id: TaffyId) -> Self::FlexboxContainerStyle<'_> {
        &self.node(id).style
    }

    fn get_flexbox_child_style(&self, id: TaffyId) -> Self::FlexboxItemStyle<'_> {
        &self.node(id).style
    }
}

impl LayoutGridContainer for LayoutTree {
    type GridContainerStyle<'a> = &'a Style;
    type GridItemStyle<'a> = &'a Style;

    fn get_grid_container_style(&self, id: TaffyId) -> Self::GridContainerStyle<'_> {
        &self.node(id).style
    }

    fn get_grid_child_style(&self, id: TaffyId) -> Self::GridItemStyle<'_> {
        &self.node(id).style
    }
}

impl LayoutBlockContainer for LayoutTree {
    type BlockContainerStyle<'a> = &'a Style;
    type BlockItemStyle<'a> = &'a Style;

    fn get_block_container_style(&self, id: TaffyId) -> Self::BlockContainerStyle<'_> {
        &self.node(id).style
    }

    fn get_block_child_style(&self, id: TaffyId) -> Self::BlockItemStyle<'_> {
        &self.node(id).style
    }
}

impl RoundTree for LayoutTree {
    fn get_unrounded_layout(&self, id: TaffyId) -> Layout {
        self.node(id).unrounded
    }

    fn set_final_layout(&mut self, id: TaffyId, layout: &Layout) {
        self.node_mut(id).solved = *layout;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::prelude::{TaffyAuto, length, percent};

    #[test]
    fn snapping_keeps_adjacent_edges_exactly_shared() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            // Two boxes meeting at a fractional edge. Snapping the shared coordinate — not
            // each box's extent — is what keeps them meeting.
            let edge = snap(37.3, scale);
            assert_eq!(snap(37.3, scale), edge);
            let px = edge * scale;
            assert!((px - px.round()).abs() < 1.0e-3);
        }
    }

    fn tree_with_a_row() -> (LayoutTree, NodeId, [NodeId; 3]) {
        let mut tree = LayoutTree::new();
        let ids = crate::id::Ids::new();
        let _ = ids;
        let root = NodeId::raw(1, 1);
        let kids = [NodeId::raw(2, 1), NodeId::raw(3, 1), NodeId::raw(4, 1)];
        tree.create(root, LayoutKind::Container);
        for kid in kids {
            tree.create(kid, LayoutKind::Container);
            tree.set_style(
                kid,
                &Style {
                    size: Size {
                        width: length(100.0_f32),
                        height: length(20.0_f32),
                    },
                    ..Style::DEFAULT
                },
            );
        }
        tree.set_style(
            root,
            &Style {
                size: Size {
                    width: percent(1.0_f32),
                    height: percent(1.0_f32),
                },
                ..Style::DEFAULT
            },
        );
        tree.set_children(root, &kids);
        (tree, root, kids)
    }

    #[test]
    fn a_solve_places_children_in_order_and_absolutely() {
        let (mut tree, root, kids) = tree_with_a_row();
        let mut out = Vec::new();
        tree.solve(root, Vector2 { x: 600.0, y: 400.0 }, 1.0, &mut out);
        let rects: Vec<Rect> = kids.iter().map(|k| out[k.index()].rect).collect();
        assert_eq!(rects[0].x0, 0.0);
        assert_eq!(rects[1].x0, 100.0);
        assert_eq!(rects[2].x0, 200.0);
        assert!(rects.iter().all(|r| (r.height() - 20.0).abs() < 0.01));
    }

    #[test]
    fn a_hidden_node_keeps_its_slot_and_takes_no_space() {
        let (mut tree, root, kids) = tree_with_a_row();
        assert!(tree.set_hidden(kids[1], true));
        let mut out = Vec::new();
        tree.solve(root, Vector2 { x: 600.0, y: 400.0 }, 1.0, &mut out);
        assert_eq!(out[kids[2].index()].rect.x0, 100.0);
        // Hidden, not removed: unhiding restores it without anything being rebuilt.
        assert!(tree.set_hidden(kids[1], false));
        tree.solve(root, Vector2 { x: 600.0, y: 400.0 }, 1.0, &mut out);
        assert_eq!(out[kids[2].index()].rect.x0, 200.0);
    }

    #[test]
    fn hiding_a_node_does_not_rewrite_what_it_is() {
        // A style says what a node *is*; hiding says whether it is shown. Folding the
        // second into the first means a grid comes back from a hide as a flexbox.
        let (mut tree, _, kids) = tree_with_a_row();
        let grid = kids[0];
        tree.set_style(
            grid,
            &Style {
                display: Display::Grid,
                ..Style::DEFAULT
            },
        );
        tree.set_hidden(grid, true);
        tree.set_hidden(grid, false);
        assert_eq!(
            tree.node(TaffyId::from(grid.index())).style.display,
            Display::Grid,
            "hiding and showing rewrote the node's display"
        );
    }

    #[test]
    fn a_style_re_pushed_while_hidden_does_not_reveal_the_node() {
        // A widget layer re-states a node's style on every rebuild. If hidden-ness lives
        // in the style, one of those re-statements silently shows the node.
        let (mut tree, root, kids) = tree_with_a_row();
        let hidden = kids[1];
        assert!(tree.set_hidden(hidden, true));
        tree.set_style(
            hidden,
            &Style {
                size: Size {
                    width: length(20.0_f32),
                    height: length(10.0_f32),
                },
                ..Style::DEFAULT
            },
        );
        let mut out = Vec::new();
        tree.solve(root, Vector2 { x: 600.0, y: 400.0 }, 1.0, &mut out);
        assert_eq!(
            out[kids[2].index()].rect.x0,
            100.0,
            "a re-pushed style revealed a hidden node"
        );
    }

    #[test]
    fn becoming_responsive_keeps_the_style_and_the_children() {
        // The kind is a declaration about how a node lays out, so it has to compose in any
        // order with the other two. Re-creating the slot to set it drops both, and the
        // symptom is a subtree that silently stops being laid out at all.
        let (mut tree, root, kids) = tree_with_a_row();
        assert!(tree.set_kind(root, LayoutKind::Responsive(Bounds([600.0, 1000.0]))));
        let mut out = Vec::new();
        tree.solve(root, Vector2 { x: 600.0, y: 400.0 }, 1.0, &mut out);
        assert_eq!(
            out[kids[2].index()].rect.x0,
            200.0,
            "the children were dropped"
        );
        assert_eq!(
            out[root.index()].size.x,
            600.0,
            "the root's own style was dropped"
        );
        // And it is idempotent, so a re-declaration is not a layout pass.
        assert!(!tree.set_kind(root, LayoutKind::Responsive(Bounds([600.0, 1000.0]))));
    }

    #[test]
    fn a_class_flip_re_measures_a_leaf_whose_own_inputs_did_not_change() {
        // The exact case taffy's cache cannot see: a fixed-width leaf gets the same layout
        // input at every width, so only clearing the subtree's caches re-measures it.
        let mut tree = LayoutTree::new();
        let root = NodeId::raw(1, 1);
        let card = NodeId::raw(2, 1);
        let leaf = NodeId::raw(3, 1);
        tree.create(root, LayoutKind::Container);
        tree.create(card, LayoutKind::Responsive(Bounds([600.0, 1000.0])));
        tree.create(leaf, LayoutKind::Container);
        // Cross-axis stretch is flexbox's default, and it would make every height the
        // container's — so the measured height, which is what this test is about, would
        // never be visible.
        let top_aligned = Some(taffy::AlignItems::FLEX_START);
        tree.set_style(
            root,
            &Style {
                size: Size {
                    width: percent(1.0_f32),
                    height: percent(1.0_f32),
                },
                align_items: top_aligned,
                ..Style::DEFAULT
            },
        );
        tree.set_style(
            card,
            &Style {
                size: Size {
                    width: percent(1.0_f32),
                    height: taffy::Dimension::AUTO,
                },
                align_items: top_aligned,
                ..Style::DEFAULT
            },
        );
        tree.set_style(
            leaf,
            &Style {
                size: Size {
                    width: length(120.0_f32),
                    height: taffy::Dimension::AUTO,
                },
                ..Style::DEFAULT
            },
        );
        tree.set_measure(leaf, MeasureCtx::Measured(MeasureKey(7)));
        tree.set_children(root, &[card]);
        tree.set_children(card, &[leaf]);
        tree.on_measure(|input: MeasureIn| Vector2 {
            x: 120.0,
            y: match input.class {
                WidthClass::Wide => 20.0,
                WidthClass::Medium => 40.0,
                WidthClass::Narrow => 60.0,
            },
        });

        let mut out = Vec::new();
        tree.solve(
            root,
            Vector2 {
                x: 1400.0,
                y: 400.0,
            },
            1.0,
            &mut out,
        );
        assert_eq!(out[leaf.index()].size.y, 20.0, "wide");

        tree.solve(root, Vector2 { x: 480.0, y: 400.0 }, 1.0, &mut out);
        assert_eq!(out[leaf.index()].size.y, 60.0, "narrow, after a class flip");
    }
}
